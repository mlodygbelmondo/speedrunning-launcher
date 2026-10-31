//! Putting a staged release in place of the running launcher.
//!
//! Windows doesn't let a running exe be replaced or deleted, but it can be renamed (the process
//! keeps running from the renamed file). So the swap is three steps in the exe's folder:
//!
//! 1. copy the staged exe next to the running one as `<name>.new` (a copy, not a rename: the
//!    staging folder may be on another drive). Nothing has changed yet if this fails;
//! 2. rename the running exe to `<stem>.old.exe`;
//! 3. rename `<name>.new` to the exe's name. If this fails, step 2 is undone.
//!
//! Then the release's other files (README.txt, the licences, VERSION.txt) are copied over the old
//! ones; a failure there is only noted. The new exe starts on the next start (or Restart), and
//! that start deletes the `.old.exe` (`cleanup_old`). The exe's path is a parameter, so all of
//! this runs on Linux in a temporary folder too; the file operations are behind `Ops`, so a test
//! can make any one of them fail.
use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{EXE_NAME, Staged};

/// The file operations of a swap.
pub trait Ops {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
    fn copy(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::copy(from, to).map(|_| ())
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }
}

/// The real file system.
pub struct Fs;

impl Ops for Fs {}

/// A swap that went through.
#[derive(Debug, PartialEq, Eq)]
pub struct Installed {
    /// Where the previous exe is now (deleted on the next start).
    pub old: PathBuf,
    /// Other files of the release that could not be copied, in English.
    pub notes: Vec<String>,
}

#[derive(Debug)]
pub enum SwapError {
    /// The exe's folder can't be written (Program Files, a read-only drive). Nothing changed;
    /// the player has to download the release by hand.
    NotWritable(String),
    /// A step failed and the previous exe is back in place.
    Failed(String),
    /// The new exe couldn't be put in place and the previous one couldn't be put back either:
    /// the launcher's file is now `old`. Should never happen; logged loudly.
    RolledBackFailed { detail: String, old: PathBuf },
}

impl std::fmt::Display for SwapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwapError::NotWritable(s) => write!(f, "the launcher's folder can't be written: {s}"),
            SwapError::Failed(s) => write!(f, "{s} (the previous version is back in place)"),
            SwapError::RolledBackFailed { detail, old } => write!(
                f,
                "{detail}; putting the previous version back failed too, it is at {}",
                old.display()
            ),
        }
    }
}

impl std::error::Error for SwapError {}

/// Whether files can be created in `dir`: writes and removes a probe file.
pub fn writable(dir: &Path) -> io::Result<()> {
    let probe = dir.join(format!(".srl-update-probe-{}", std::process::id()));
    std::fs::write(&probe, b"ok")?;
    std::fs::remove_file(&probe)
}

/// `<stem>.old<suffix>.<ext>`: `speedrunning-launcher.old.exe`, or `.old-2.exe` and on when an
/// older one is still there (an earlier swap whose old process still runs).
fn old_name(exe: &Path, n: u32) -> PathBuf {
    let stem = exe.file_stem().unwrap_or_default().to_string_lossy();
    let ext = exe
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let suffix = if n == 0 {
        String::new()
    } else {
        format!("-{n}")
    };
    exe.with_file_name(format!("{stem}.old{suffix}{ext}"))
}

fn new_name(exe: &Path) -> PathBuf {
    let mut name = exe.file_name().unwrap_or_default().to_os_string();
    name.push(".new");
    exe.with_file_name(name)
}

/// Swaps `staged` in for the exe at `exe`. See the module's comment for the steps.
pub fn install(staged: &Staged, exe: &Path) -> Result<Installed, SwapError> {
    install_with(&Fs, staged, exe)
}

pub fn install_with(ops: &dyn Ops, staged: &Staged, exe: &Path) -> Result<Installed, SwapError> {
    let dir = exe
        .parent()
        .ok_or_else(|| SwapError::Failed(format!("{} has no folder", exe.display())))?;
    let new = new_name(exe);
    // 1. The new exe next to the running one, under a temporary name.
    if let Err(e) = ops.copy(&staged.dir.join(EXE_NAME), &new) {
        let _ = ops.remove(&new);
        let detail = format!("copying the new exe to {}: {e}", new.display());
        return Err(
            if e.kind() == io::ErrorKind::PermissionDenied
                || e.kind() == io::ErrorKind::ReadOnlyFilesystem
            {
                SwapError::NotWritable(detail)
            } else {
                SwapError::Failed(detail)
            },
        );
    }
    // The running exe's permissions (on Linux its execute bits; a zip entry carries none).
    if let Ok(meta) = std::fs::metadata(exe) {
        let _ = std::fs::set_permissions(&new, meta.permissions());
    }
    // 2. The running exe out of the way. A `.old.exe` left by an earlier swap is removed, or
    // the next free name taken when it can't be (its process may still run).
    let old = (0..100)
        .map(|n| old_name(exe, n))
        .find(|p| !p.exists() || ops.remove(p).is_ok())
        .unwrap_or_else(|| old_name(exe, 100));
    if let Err(e) = ops.rename(exe, &old) {
        let _ = ops.remove(&new);
        return Err(SwapError::Failed(format!(
            "renaming {} to {}: {e}",
            exe.display(),
            old.display()
        )));
    }
    // 3. The new exe in its place, or the old one back.
    if let Err(e) = ops.rename(&new, exe) {
        let detail = format!("renaming {} to {}: {e}", new.display(), exe.display());
        let _ = ops.remove(&new);
        return match ops.rename(&old, exe) {
            Ok(()) => Err(SwapError::Failed(detail)),
            Err(back) => Err(SwapError::RolledBackFailed {
                detail: format!("{detail}; {back}"),
                old,
            }),
        };
    }
    // The rest of the release, over the previous copies. Only what the zip has.
    let mut notes = Vec::new();
    for rel in &staged.files {
        if rel.as_os_str() == EXE_NAME {
            continue;
        }
        let to = dir.join(rel);
        let copied = to
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| ops.copy(&staged.dir.join(rel), &to));
        if let Err(e) = copied {
            notes.push(format!("{}: {e}", to.display()));
        }
    }
    Ok(Installed { old, notes })
}

/// Deletes what earlier swaps left next to `exe`: `<stem>.old*.exe` and `<name>.new`. Returns
/// each file it tried and how that went; one still in use (the previous launcher hasn't quit
/// yet) fails on Windows and is tried again on a later call.
pub fn cleanup_old(exe: &Path) -> Vec<(PathBuf, io::Result<()>)> {
    let (Some(dir), Some(stem)) = (exe.parent(), exe.file_stem()) else {
        return Vec::new();
    };
    let stem = stem.to_string_lossy().to_lowercase();
    let ext = exe
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let new = new_name(exe)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            // Windows names compare without case.
            let name = e.file_name().to_string_lossy().to_lowercase();
            let old = name
                .strip_prefix(&format!("{stem}.old"))
                .and_then(|rest| rest.strip_suffix(ext.as_str()))
                .is_some_and(|mid| {
                    mid.is_empty()
                        || mid
                            .strip_prefix('-')
                            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                });
            old || Some(&name) == new.as_ref()
        })
        .map(|e| {
            let p = e.path();
            let r = std::fs::remove_file(&p);
            (p, r)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::archive::tests::temp_dir;

    /// The real file system, except that the `fail`-th rename (counting from 1) fails.
    struct FailRename {
        fail: u32,
        n: Cell<u32>,
    }

    impl Ops for FailRename {
        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            self.n.set(self.n.get() + 1);
            if self.n.get() == self.fail {
                return Err(io::Error::other("simulated"));
            }
            std::fs::rename(from, to)
        }
    }

    /// An install folder with the running exe and an old README, and a staged release.
    fn setup(tag: &str) -> (PathBuf, PathBuf, Staged) {
        let root = temp_dir(tag);
        let install = root.join("install");
        std::fs::create_dir_all(&install).unwrap();
        let exe = install.join(EXE_NAME);
        std::fs::write(&exe, b"MZ old").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(install.join("README.txt"), b"old readme").unwrap();
        let staged = root.join("staged");
        std::fs::create_dir_all(staged.join("licenses")).unwrap();
        std::fs::write(staged.join(EXE_NAME), b"MZ new").unwrap();
        std::fs::write(staged.join("README.txt"), b"new readme").unwrap();
        std::fs::write(staged.join("licenses/OFL.txt"), b"ofl").unwrap();
        let staged = Staged {
            version: semver::Version::new(0, 2, 0),
            dir: staged,
            files: vec![
                PathBuf::from(EXE_NAME),
                PathBuf::from("README.txt"),
                Path::new("licenses").join("OFL.txt"),
            ],
        };
        (root, exe, staged)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn swaps_the_exe_and_copies_the_rest() {
        let (root, exe, staged) = setup("swap");
        let done = install(&staged, &exe).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"MZ new");
        assert_eq!(
            done.old,
            exe.with_file_name("speedrunning-launcher.old.exe")
        );
        assert_eq!(std::fs::read(&done.old).unwrap(), b"MZ old");
        assert!(done.notes.is_empty(), "{:?}", done.notes);
        let dir = exe.parent().unwrap();
        assert_eq!(
            std::fs::read(dir.join("README.txt")).unwrap(),
            b"new readme"
        );
        assert_eq!(std::fs::read(dir.join("licenses/OFL.txt")).unwrap(), b"ofl");
        // The next start removes the old exe, and nothing else.
        let removed = cleanup_old(&exe);
        assert_eq!(removed.len(), 1);
        assert!(removed[0].1.is_ok());
        assert_eq!(
            names(dir),
            ["README.txt", "licenses", "speedrunning-launcher.exe"]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_failed_swap_puts_the_old_exe_back() {
        // The new exe can't take the exe's name (the second rename).
        let (root, exe, staged) = setup("rollback");
        let ops = FailRename {
            fail: 2,
            n: Cell::new(0),
        };
        let err = install_with(&ops, &staged, &exe).unwrap_err();
        assert!(matches!(err, SwapError::Failed(_)), "{err}");
        assert_eq!(std::fs::read(&exe).unwrap(), b"MZ old");
        let dir = exe.parent().unwrap();
        // No `.new` or `.old.exe` left behind, and the README untouched.
        assert_eq!(names(dir), ["README.txt", "speedrunning-launcher.exe"]);
        assert_eq!(
            std::fs::read(dir.join("README.txt")).unwrap(),
            b"old readme"
        );

        // The running exe can't be renamed (the first rename): nothing changes.
        let ops = FailRename {
            fail: 1,
            n: Cell::new(0),
        };
        assert!(matches!(
            install_with(&ops, &staged, &exe),
            Err(SwapError::Failed(_))
        ));
        assert_eq!(std::fs::read(&exe).unwrap(), b"MZ old");
        assert_eq!(names(dir), ["README.txt", "speedrunning-launcher.exe"]);

        // Both the swap and the way back fail: the error says where the old exe is.
        struct Neither;
        impl Ops for Neither {
            fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
                if from.extension().is_some_and(|e| e == "new")
                    || to == from.with_file_name(EXE_NAME)
                {
                    return Err(io::Error::other("simulated"));
                }
                std::fs::rename(from, to)
            }
        }
        match install_with(&Neither, &staged, &exe) {
            Err(SwapError::RolledBackFailed { old, .. }) => {
                assert_eq!(std::fs::read(&old).unwrap(), b"MZ old");
            }
            other => panic!("{other:?}"),
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_missing_staged_exe_changes_nothing() {
        let (root, exe, mut staged) = setup("missing");
        staged.dir = root.join("nowhere");
        assert!(matches!(install(&staged, &exe), Err(SwapError::Failed(_))));
        assert_eq!(std::fs::read(&exe).unwrap(), b"MZ old");
        assert_eq!(
            names(exe.parent().unwrap()),
            ["README.txt", "speedrunning-launcher.exe"]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_old_exe_still_in_use_gets_another_name() {
        let (root, exe, staged) = setup("inuse");
        let dir = exe.parent().unwrap().to_path_buf();
        // A directory can't be removed with remove_file: stands in for an exe Windows holds.
        std::fs::create_dir(dir.join("speedrunning-launcher.old.exe")).unwrap();
        let done = install(&staged, &exe).unwrap();
        assert_eq!(done.old, dir.join("speedrunning-launcher.old-1.exe"));
        // Cleanup takes the files it knows and leaves everything else.
        std::fs::write(dir.join("speedrunning-launcher.old-x.exe"), b"keep").unwrap();
        std::fs::write(dir.join("speedrunning-launcher.exe.new"), b"stale").unwrap();
        std::fs::write(dir.join("SPEEDRUNNING-LAUNCHER.OLD-7.EXE"), b"x").unwrap();
        let mut removed: Vec<String> = cleanup_old(&exe)
            .into_iter()
            .map(|(p, r)| {
                assert!(r.is_ok());
                p.file_name().unwrap().to_string_lossy().into_owned()
            })
            .collect();
        removed.sort();
        assert_eq!(
            removed,
            [
                "SPEEDRUNNING-LAUNCHER.OLD-7.EXE",
                "speedrunning-launcher.exe.new",
                "speedrunning-launcher.old-1.exe"
            ]
        );
        assert!(dir.join("speedrunning-launcher.old-x.exe").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_folder_that_refuses_writes() {
        let root = temp_dir("probe");
        assert!(writable(&root).is_ok());
        assert!(writable(&root.join("missing")).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
