//! Toolscreen (Windows): a DLL that hooks the game's OpenGL (and, from 1.5.0, Vulkan) output for
//! instant resizing, EyeZoom, mirrors and overlays. Its settings open with Ctrl+I in the game and
//! live in `%USERPROFILE%\.config\toolscreen`, shared with any other launcher's install.
//!
//! # How it attaches (from its MIT sources: Toolscreen 92c7e9b, EasyInjectBundled f59501e)
//!
//! The release assets are installers (`Toolscreen-<v>-double-click-me.jar` / `.exe`, built by
//! EasyInjectBundled) that carry `dlls/Toolscreen.dll` (or `Toolscreen_x64.dll` and
//! `Toolscreen_arm64.dll` from 1.4.5) and `dlls/liblogger_<arch>.dll`. Prism and MultiMC run the
//! jar as a pre-launch command; MCSR Launcher, which has Toolscreen built in, runs
//! `java -jar Toolscreen.jar` in the instance folder with its environment before it starts the
//! game, after checking the jar's hash against its meta. Either way the jar then starts a
//! watcher that polls for a Java process whose working directory is the instance, and injects
//! **liblogger first, then Toolscreen.dll**, into that JVM with `CreateRemoteThread` +
//! `LoadLibraryW`, as soon as it finds it ("before native graphics initialization can race
//! ahead of the injector"). In launcher mode the jar also checks GitHub for a newer Toolscreen
//! and can replace itself.
//!
//! We do the injection step ourselves instead of running the installer jar:
//!
//! - we start the JVM, so we know its pid at once and need no watcher or polling;
//! - the bytes we inject are the ones we checked: the DLLs come out of the pinned jar (sha256),
//!   each must match its pinned sha512 **and** be in `legal-dlls.csv` (rule A.8.16 allows only
//!   listed DLLs, and only with a listed liblogger), and they are hashed again before every
//!   launch;
//! - the installer's own updater can't swap in a build that isn't legal or pinned.
//!
//! The injection itself (`win::inject`) follows EasyInjectBundled's `DllInjector` (MIT).
//!
//! # Versions
//!
//! Pinned below with the jar's GitHub digest and the DLL hashes inside, as read on 2026-10-26:
//!
//! | Release | GitHub | Toolscreen.dll | liblogger |
//! |---|---|---|---|
//! | v1.4.7 | release | x64: = legal-dlls `Toolscreen.dll 1.4.4` | x64 1.0.2 |
//! | v1.5.0 | prerelease | x64: `1.5.0-x64`, arm64: `1.5.0-arm64` | x64 1.1.0, arm64 1.1.0 |
//!
//! v1.4.5 to v1.4.7 carry the 1.4.4 DLL unchanged, which is why legal-dlls.csv has no 1.4.5 to
//! 1.4.7 rows. [`choose`] takes the newest release that is not a prerelease and whose two DLLs
//! for this CPU are both in the current legal-dlls.csv, and a prerelease only when no release
//! qualifies: x64 gets v1.4.7, ARM64 (which only 1.5.0 supports) v1.5.0.
use std::{
    io::Read as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha512};
use srl_core::{Download, Downloader, Hash, Paths, ProgressFn, Stage};
use srl_game::rules::Arch;

use crate::{
    ToolId,
    legal_dlls::LegalDlls,
    status::{InstalledTool, ToolStatus},
};

/// The release x64 Windows gets today (see [`choose`]).
pub const STABLE_VERSION: &str = "1.4.7";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: &'static str,
    pub prerelease: bool,
    pub jar_url: &'static str,
    /// GitHub's asset digest.
    pub jar_sha256: &'static str,
    pub jar_size: u64,
    pub builds: &'static [Build],
}

/// The two DLLs of one CPU architecture inside a release jar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Build {
    pub arch: Arch,
    /// Zip entries.
    pub dll_entry: &'static str,
    pub dll_sha512: &'static str,
    pub logger_entry: &'static str,
    pub logger_sha512: &'static str,
}

impl Build {
    fn arch_name(&self) -> &'static str {
        arch_name(self.arch)
    }

    /// Both DLLs listed in legal-dlls.csv.
    pub fn is_legal(&self, legal: &LegalDlls) -> bool {
        legal.contains(self.dll_sha512) && legal.contains(self.logger_sha512)
    }
}

fn arch_name(arch: Arch) -> &'static str {
    match arch {
        Arch::Aarch64 => "arm64",
        Arch::X86 => "x86",
        Arch::X86_64 => "x64",
    }
}

/// Newest first.
pub static RELEASES: &[Release] = &[
    Release {
        version: "1.5.0",
        prerelease: true,
        jar_url: "https://github.com/jojoe77777/Toolscreen/releases/download/v1.5.0/Toolscreen-1.5.0-double-click-me.jar",
        jar_sha256: "fc19cdb87885e8bcaf24ccdbad1ae1c2b259f2c0424578660aea0b4446fe822e",
        jar_size: 16_007_430,
        builds: &[
            Build {
                arch: Arch::X86_64,
                dll_entry: "dlls/Toolscreen_x64.dll",
                dll_sha512: "1c9db0e0dab168f31f0ad41adccca451ac4a150b079a35bdcc3aacfae687fe7b505dc524238afa4141ebaec60b0f86b277e90ec9e5568cfc94010a4e2d4b53ec",
                logger_entry: "dlls/liblogger_x64.dll",
                logger_sha512: "02a1396f69c836e44a4bc5ad2dc1440807c70026c5d6cb24ec4afad54caea503e6b221a45619fed75d0e266d32452d34497e4892a6a0727f16934be5ed6d3768",
            },
            Build {
                arch: Arch::Aarch64,
                dll_entry: "dlls/Toolscreen_arm64.dll",
                dll_sha512: "c6b37cd3aeacc64100d1a4ab44d478a16d47253438377b297a752fab353129d180930b57fe91405ed861ac3c3e8ff201a3df3f7b947be6751d52570675b678ef",
                logger_entry: "dlls/liblogger_arm64.dll",
                logger_sha512: "3503fc8af5aefe7c3fa5fa53c977b29f014d09ff0f9c3f840d1e57e24261006b668da8e6651c65d7145ce4aa2b8b183c97607b1301bd889a289996b626ecff01",
            },
        ],
    },
    Release {
        version: "1.4.7",
        prerelease: false,
        jar_url: "https://github.com/jojoe77777/Toolscreen/releases/download/v1.4.7/Toolscreen-1.4.7-double-click-me.jar",
        jar_sha256: "bfadb6219e67280f59675724de4cd72c19dc5f5b294954c8c7b8c5053646cc3e",
        jar_size: 9_363_156,
        builds: &[Build {
            arch: Arch::X86_64,
            dll_entry: "dlls/Toolscreen.dll",
            dll_sha512: "833a6b0c0df01df057f17a6fb7ee0c2dc486a7de4d93fc04fc550c4270d6717695eec84d028e69bf5a67de3212f2c4cc15508c299fb421e8c3be983657c7d088",
            logger_entry: "dlls/liblogger_x64.dll",
            logger_sha512: "946849cde44b52247534661eb798163c42b2a9e8e9e8311bb533457d2817ee47229ad793d0264902c0e9220fc543597836d37c043454bbd34d7e0e4ea5bd4834",
        }],
    },
];

/// The release to use on `arch`: the newest non-prerelease whose build for `arch` is legal, else
/// the newest legal prerelease. `None` when nothing pinned is legal for `arch`.
pub fn choose<'a>(
    releases: &'a [Release],
    arch: Arch,
    legal: &LegalDlls,
) -> Option<(&'a Release, &'a Build)> {
    let legal_build = |r: &'a Release| {
        r.builds
            .iter()
            .find(|b| b.arch == arch && b.is_legal(legal))
            .map(|b| (r, b))
    };
    let newest = |pre: bool| {
        releases
            .iter()
            .filter(|r| r.prerelease == pre)
            .filter_map(legal_build)
            .max_by(|a, b| version_key(a.0.version).cmp(&version_key(b.0.version)))
    };
    newest(false).or_else(|| newest(true))
}

fn version_key(v: &str) -> Vec<u64> {
    v.split(['.', '-'])
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// The DLLs of an installed release, in injection order: liblogger, then Toolscreen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Injection {
    pub version: String,
    pub dlls: Vec<PathBuf>,
}

pub fn dir(paths: &Paths) -> PathBuf {
    paths.tools().join(ToolId::Toolscreen.id())
}

/// `tools/toolscreen/<version>/<arch>/`.
pub fn build_dir(paths: &Paths, release: &Release, build: &Build) -> PathBuf {
    dir(paths).join(release.version).join(build.arch_name())
}

fn logger_name(build: &Build) -> String {
    format!("liblogger_{}.dll", build.arch_name())
}

/// The installed files of a build, logger first.
pub fn files(paths: &Paths, release: &Release, build: &Build) -> [(PathBuf, &'static str); 2] {
    let d = build_dir(paths, release, build);
    [
        (d.join(logger_name(build)), build.logger_sha512),
        (d.join("Toolscreen.dll"), build.dll_sha512),
    ]
}

/// From the files on disk and the cached legal-dlls.csv (no network, no hashing).
pub fn status(paths: &Paths, arch: Arch, legal: Option<&LegalDlls>) -> ToolStatus {
    let any_legal;
    let legal = match legal {
        Some(l) => l,
        None => {
            any_legal = pinned_as_legal();
            &any_legal
        }
    };
    let Some((release, build)) = choose(RELEASES, arch, legal) else {
        return ToolStatus::NotAvailable;
    };
    let present = |r: &Release, b: &Build| files(paths, r, b).iter().all(|(p, _)| p.is_file());
    if present(release, build) {
        return ToolStatus::Installed(InstalledTool {
            version: Some(release.version.into()),
            path: build_dir(paths, release, build),
            managed: true,
        });
    }
    let older = RELEASES
        .iter()
        .filter(|r| r.version != release.version)
        .find(|r| r.builds.iter().any(|b| b.arch == arch && present(r, b)));
    match older {
        Some(r) => ToolStatus::UpdateAvailable {
            installed: r.version.into(),
            latest: release.version.into(),
        },
        None => ToolStatus::NotInstalled,
    }
}

/// The release an install picks for `arch` (with the cached legal-dlls.csv, else the pins), and
/// the bytes its jar downloads. `None` when nothing pinned is legal for `arch`.
pub fn offer(arch: Arch, legal: Option<&LegalDlls>) -> Option<(&'static str, u64)> {
    let pinned;
    let legal = match legal {
        Some(l) => l,
        None => {
            pinned = pinned_as_legal();
            &pinned
        }
    };
    choose(RELEASES, arch, legal).map(|(r, _)| (r.version, r.jar_size))
}

/// Every pinned hash, as if legal-dlls.csv listed them all: the pins were legal when made, so
/// `status` uses this before the first install fetched the real list.
fn pinned_as_legal() -> LegalDlls {
    let mut csv = String::from("filename,version,sha512\n");
    for r in RELEASES {
        for b in r.builds {
            csv.push_str(&format!("Toolscreen.dll,{},{}\n", r.version, b.dll_sha512));
            csv.push_str(&format!(
                "liblogger.dll,{},{}\n",
                r.version, b.logger_sha512
            ));
        }
    }
    LegalDlls::parse(&csv).expect("pins are well-formed")
}

/// Installs the release [`choose`] picks for `arch`: downloads its jar (sha256), takes the two
/// DLLs out of it, checks each against its pinned sha512 and legal-dlls.csv, and removes the jar
/// and older releases. Runs on any OS, so the download and the checks are tested on Linux too;
/// only the injection is Windows-only.
pub async fn install_for(
    paths: &Paths,
    downloader: &Downloader,
    arch: Arch,
    legal: &LegalDlls,
    progress: Option<ProgressFn>,
) -> Result<Injection> {
    let Some((release, build)) = choose(RELEASES, arch, legal) else {
        bail!(
            "no pinned Toolscreen build for {} is in legal-dlls.csv",
            arch_name(arch)
        );
    };
    let files = files(paths, release, build);
    if verify_files(&files, legal).await.is_ok() {
        remove_others(paths, release).await;
        return Ok(injection(release, &files));
    }
    let version_dir = dir(paths).join(release.version);
    let jar = version_dir.join(format!(
        "Toolscreen-{}-double-click-me.jar",
        release.version
    ));
    downloader
        .download_all(
            vec![Download {
                url: release.jar_url.into(),
                dest: jar.clone(),
                hash: Hash::Sha256(release.jar_sha256.into()),
                size: Some(release.jar_size),
            }],
            Stage::Tools,
            progress,
        )
        .await
        .with_context(|| format!("downloading Toolscreen {}", release.version))?;
    {
        let jar = jar.clone();
        let build = *build;
        let out: Vec<(PathBuf, &'static str, &'static str)> = vec![
            (files[0].0.clone(), build.logger_entry, build.logger_sha512),
            (files[1].0.clone(), build.dll_entry, build.dll_sha512),
        ];
        tokio::task::spawn_blocking(move || extract(&jar, &out)).await??;
    }
    let _ = tokio::fs::remove_file(&jar).await;
    verify_files(&files, legal).await?;
    remove_others(paths, release).await;
    Ok(injection(release, &files))
}

fn injection(release: &Release, files: &[(PathBuf, &'static str); 2]) -> Injection {
    Injection {
        version: release.version.into(),
        dlls: files.iter().map(|(p, _)| p.clone()).collect(),
    }
}

/// Older releases' folders. On Windows a DLL a running game loaded can't be deleted; that folder
/// stays until the next install.
async fn remove_others(paths: &Paths, keep: &Release) {
    let Ok(mut entries) = tokio::fs::read_dir(dir(paths)).await else {
        return;
    };
    while let Ok(Some(e)) = entries.next_entry().await {
        if e.file_name().to_str() != Some(keep.version) {
            let _ = tokio::fs::remove_dir_all(e.path()).await;
        }
    }
}

/// Copies zip entries out of `jar`, each to `<dest>.part`, checks its sha512, then renames it.
fn extract(jar: &Path, out: &[(PathBuf, &'static str, &'static str)]) -> Result<()> {
    let file = std::fs::File::open(jar).with_context(|| format!("opening {}", jar.display()))?;
    let mut zip =
        zip::ZipArchive::new(file).with_context(|| format!("reading {}", jar.display()))?;
    for (dest, entry, sha512) in out {
        let mut data = Vec::new();
        zip.by_name(entry)
            .with_context(|| format!("{} has no {entry}", jar.display()))?
            .read_to_end(&mut data)?;
        let got = hex::encode(Sha512::digest(&data));
        if !got.eq_ignore_ascii_case(sha512) {
            bail!(
                "{entry} in {}: sha512 {got}, pinned {sha512}",
                jar.display()
            );
        }
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d)?;
        }
        let part = dest.with_extension("dll.part");
        std::fs::write(&part, &data)?;
        std::fs::rename(&part, dest)?;
    }
    Ok(())
}

/// Each file exists, has its pinned sha512, and that sha512 is in legal-dlls.csv.
pub async fn verify_files(files: &[(PathBuf, &'static str)], legal: &LegalDlls) -> Result<()> {
    for (path, sha512) in files {
        let p = path.clone();
        let got = tokio::task::spawn_blocking(move || srl_core::download::file_sha512(&p))
            .await?
            .with_context(|| format!("hashing {}", path.display()))?;
        if !got.eq_ignore_ascii_case(sha512) {
            bail!(
                "{} was changed on disk (sha512 {got}, expected {sha512})",
                path.display()
            );
        }
        if !legal.contains(&got) {
            bail!(
                "{} (sha512 {got}) is not in legal-dlls.csv any more",
                path.display()
            );
        }
    }
    Ok(())
}

/// What [`crate::Tools`] injects before a launch: the chosen build's files, verified against
/// their pins and `legal` right now.
pub async fn for_launch(paths: &Paths, arch: Arch, legal: &LegalDlls) -> Result<Injection> {
    let Some((release, build)) = choose(RELEASES, arch, legal) else {
        bail!(
            "no pinned Toolscreen build for {} is in legal-dlls.csv",
            arch_name(arch)
        );
    };
    let files = files(paths, release, build);
    if !files.iter().all(|(p, _)| p.is_file()) {
        bail!("Toolscreen {} is not installed", release.version);
    }
    verify_files(&files, legal).await?;
    Ok(injection(release, &files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legal_dlls::tests::SAMPLE;

    fn legal() -> LegalDlls {
        LegalDlls::parse(SAMPLE).unwrap()
    }

    #[test]
    fn version_choice() {
        let l = legal();
        let (r, b) = choose(RELEASES, Arch::X86_64, &l).unwrap();
        assert_eq!((r.version, r.prerelease), ("1.4.7", false));
        assert_eq!(b.dll_entry, "dlls/Toolscreen.dll");
        assert_eq!(r.version, STABLE_VERSION);
        // ARM64 exists only in the 1.5.0 prerelease.
        let (r, b) = choose(RELEASES, Arch::Aarch64, &l).unwrap();
        assert_eq!((r.version, b.arch), ("1.5.0", Arch::Aarch64));
        assert!(choose(RELEASES, Arch::X86, &l).is_none());

        // If the 1.4.4 DLL were taken off the list, x64 would move to the legal prerelease.
        let without_144: String = SAMPLE
            .lines()
            .filter(|l| !l.contains(",1.4.4,"))
            .map(|l| format!("{l}\n"))
            .collect();
        let l2 = LegalDlls::parse(&without_144).unwrap();
        assert_eq!(
            choose(RELEASES, Arch::X86_64, &l2).unwrap().0.version,
            "1.5.0"
        );
        // An illegal liblogger makes the build illegal too (rule A.8.16).
        let without_logger: String = SAMPLE
            .lines()
            .filter(|l| !l.starts_with("liblogger_x64.dll,1.0.2"))
            .map(|l| format!("{l}\n"))
            .collect();
        let l3 = LegalDlls::parse(&without_logger).unwrap();
        assert_eq!(
            choose(RELEASES, Arch::X86_64, &l3).unwrap().0.version,
            "1.5.0"
        );
        // Nothing legal: nothing chosen.
        let l4 = LegalDlls::parse("filename,version,sha512\nx,1,00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\n").unwrap();
        assert!(choose(RELEASES, Arch::X86_64, &l4).is_none());
    }

    #[test]
    fn newer_stable_wins() {
        static FAKE: &[Release] = &[
            Release {
                version: "1.4.10",
                prerelease: false,
                jar_url: "",
                jar_sha256: "",
                jar_size: 0,
                builds: RELEASES[1].builds,
            },
            Release {
                version: "1.4.9",
                prerelease: false,
                jar_url: "",
                jar_sha256: "",
                jar_size: 0,
                builds: RELEASES[1].builds,
            },
        ];
        assert_eq!(
            choose(FAKE, Arch::X86_64, &legal()).unwrap().0.version,
            "1.4.10"
        );
    }

    #[test]
    fn pins_are_well_formed() {
        for r in RELEASES {
            assert!(r.jar_url.ends_with(&format!(
                "/v{0}/Toolscreen-{0}-double-click-me.jar",
                r.version
            )));
            assert_eq!(r.jar_sha256.len(), 64);
            for b in r.builds {
                assert_eq!(b.dll_sha512.len(), 128);
                assert_eq!(b.logger_sha512.len(), 128);
                assert!(b.is_legal(&legal()), "{} {:?}", r.version, b.arch);
            }
        }
    }

    /// A jar with the right entry names but other bytes is refused, and nothing is left behind.
    #[test]
    fn extract_checks_hashes() {
        let root = std::env::temp_dir().join(format!("srl-tools-ts-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let jar = root.join("fake.jar");
        {
            let f = std::fs::File::create(&jar).unwrap();
            let mut w = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            w.start_file("dlls/Toolscreen.dll", opts).unwrap();
            std::io::Write::write_all(&mut w, b"not the real dll").unwrap();
            w.finish().unwrap();
        }
        let dest = root.join("out").join("Toolscreen.dll");
        let b = &RELEASES[1].builds[0];
        let err = extract(&jar, &[(dest.clone(), b.dll_entry, b.dll_sha512)]).unwrap_err();
        assert!(err.to_string().contains("sha512"), "{err}");
        assert!(!dest.exists());
        // With the hash of what is really there, it extracts.
        let real = hex::encode(Sha512::digest(b"not the real dll"));
        let real: &'static str = Box::leak(real.into_boxed_str());
        extract(&jar, &[(dest.clone(), b.dll_entry, real)]).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"not the real dll");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn layout() {
        let paths = Paths::new("/srl");
        let (r, b) = (&RELEASES[1], &RELEASES[1].builds[0]);
        let f = files(&paths, r, b);
        assert_eq!(
            f[0].0,
            Path::new("/srl/tools/toolscreen/1.4.7/x64/liblogger_x64.dll")
        );
        assert_eq!(
            f[1].0,
            Path::new("/srl/tools/toolscreen/1.4.7/x64/Toolscreen.dll")
        );
        assert_eq!(
            status(&paths, Arch::X86_64, Some(&legal())),
            ToolStatus::NotInstalled
        );
        assert_eq!(status(&paths, Arch::X86, None), ToolStatus::NotAvailable);
    }
}
