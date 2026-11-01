//! What the computer already records about where the player's launchers are, so a launcher is
//! found wherever it was unpacked and whatever its folder is called:
//!
//! - **Windows' list of programs run** (Windows only): every exe started is a value name in
//!   `HKCU\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\
//!   Store` (the full path) and in `HKCU\Software\Classes\Local Settings\Software\Microsoft\
//!   Windows\Shell\MuiCache` (the path plus `.FriendlyAppName` or `.ApplicationCompany`). The
//!   folders of `MultiMC.exe`, `prismlauncher.exe` and `polymc.exe` there are launchers.
//! - **Jingle** (the speedrunners' macro tool) keeps every game directory it has seen in
//!   `~/.config/Jingle/options.json`, `seenPaths` (path to the time it was last seen, in ms).
//!
//! A record may be stale or point through a link (a junction in Downloads to the real folder);
//! `detect` resolves it and checks what is really there.
use std::path::{Path, PathBuf};

use crate::detect::read_text;

/// At most this many recorded paths are looked at.
pub(crate) const MAX_RECORDED: usize = 200;

/// The exe file names of the launchers whose folder is the launcher's own.
#[cfg_attr(not(windows), allow(dead_code))]
const LAUNCHER_EXES: [&str; 3] = ["multimc.exe", "prismlauncher.exe", "polymc.exe"];

/// The game directories in a Jingle `options.json`, most recently seen first.
pub(crate) fn jingle_paths(options: &Path) -> Vec<PathBuf> {
    let Some(v) =
        read_text(options).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return Vec::new();
    };
    let Some(seen) = v["seenPaths"].as_object() else {
        return Vec::new();
    };
    let mut paths: Vec<(u64, PathBuf)> = seen
        .iter()
        .take(MAX_RECORDED)
        .filter(|(p, _)| !p.trim().is_empty())
        .map(|(p, t)| (t.as_u64().unwrap_or(0), PathBuf::from(p.trim())))
        .filter(|(_, p)| p.is_absolute())
        .collect();
    paths.sort_by(|a, b| b.0.cmp(&a.0));
    paths.into_iter().map(|(_, p)| p).collect()
}

/// The folder of a launcher's exe in one of Windows' value names: `C:\MMC\MultiMC.exe` or
/// `C:\MMC\MultiMC.exe.FriendlyAppName`.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn launcher_folder(value_name: &str) -> Option<PathBuf> {
    let lower = value_name.to_ascii_lowercase();
    let end = lower.find(".exe")? + ".exe".len();
    let exe = Path::new(value_name.get(..end)?);
    let file = exe.file_name()?.to_string_lossy().to_ascii_lowercase();
    if !LAUNCHER_EXES.contains(&file.as_str()) || !exe.is_absolute() {
        return None;
    }
    exe.parent().map(Path::to_path_buf)
}

/// The launcher folders Windows has a record of running, without repeats.
#[cfg(windows)]
pub(crate) fn launched_launchers() -> Vec<PathBuf> {
    const KEYS: [&str; 2] = [
        r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Store",
        r"Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache",
    ];
    let mut out: Vec<PathBuf> = Vec::new();
    for key in KEYS {
        for name in registry::value_names(key) {
            if let Some(dir) = launcher_folder(&name) {
                let lower = dir.to_string_lossy().to_lowercase();
                if !out
                    .iter()
                    .any(|d| d.to_string_lossy().to_lowercase() == lower)
                {
                    out.push(dir);
                }
            }
        }
    }
    out.truncate(MAX_RECORDED);
    out
}

#[cfg(not(windows))]
pub(crate) fn launched_launchers() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(windows)]
mod registry {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegEnumValueW, RegOpenKeyExW,
    };

    /// The most value names read from one key; these keys hold one per program ever run.
    const MAX_VALUES: u32 = 20_000;
    /// The longest value name the registry allows, in UTF-16 units, plus the NUL.
    const MAX_NAME: usize = 16_384;
    const ERROR_SUCCESS: u32 = 0;
    const ERROR_NO_MORE_ITEMS: u32 = 259;

    /// The value names of `HKCU\<path>`; none if the key is missing or unreadable.
    pub(super) fn value_names(path: &str) -> Vec<String> {
        let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: `wide` is NUL-terminated and outlives the call; `key` receives the handle.
        let opened =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide.as_ptr(), 0, KEY_READ, &mut key) };
        if opened != ERROR_SUCCESS {
            return Vec::new();
        }
        let mut names = Vec::new();
        let mut buf = vec![0u16; MAX_NAME];
        for index in 0..MAX_VALUES {
            let mut len = buf.len() as u32;
            // SAFETY: `key` is open; `buf` holds `len` units; no type or data is asked for.
            let r = unsafe {
                RegEnumValueW(
                    key,
                    index,
                    buf.as_mut_ptr(),
                    &mut len,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            match r {
                ERROR_SUCCESS => names.push(String::from_utf16_lossy(&buf[..len as usize])),
                ERROR_NO_MORE_ITEMS => break,
                // A name that changed under us or can't be read: skip it.
                _ => continue,
            }
        }
        // SAFETY: `key` was opened above and is closed once.
        unsafe { RegCloseKey(key) };
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_folders_from_value_names() {
        let dir = |s: &str| launcher_folder(s).map(|p| p.to_string_lossy().into_owned());
        if cfg!(windows) {
            assert_eq!(
                dir(r"C:\Games\MultiMC\MultiMC.exe").as_deref(),
                Some(r"C:\Games\MultiMC")
            );
            assert_eq!(
                dir(r"D:\mc\Prism\prismlauncher.exe.FriendlyAppName").as_deref(),
                Some(r"D:\mc\Prism")
            );
        } else {
            assert_eq!(dir("/g/MMC/MultiMC.exe").as_deref(), Some("/g/MMC"));
            assert_eq!(
                dir("/g/Prism/prismlauncher.exe.ApplicationCompany").as_deref(),
                Some("/g/Prism")
            );
        }
        assert_eq!(dir("/g/Other/javaw.exe"), None);
        assert_eq!(dir("MultiMC.exe"), None);
        assert_eq!(dir("@%SystemRoot%\\system32\\shell32.dll,-1"), None);
    }
}
