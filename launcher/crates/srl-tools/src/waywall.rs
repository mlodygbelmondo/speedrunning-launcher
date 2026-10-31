//! waywall (Linux): a nested Wayland compositor that runs the game inside its own window, for
//! instant resizing, mirrors (EyeZoom), key remaps and a floating Ninjabrain Bot. Its config is
//! the player's `~/.config/waywall/init.lua`.
//!
//! The launcher never installs it (its packages need root and bring the patched GLFW); it finds
//! `waywall` on `PATH` and, when the mode has it on, starts the game the way waywall's setup
//! guide (`doc/00_setup.md`) has Prism do it:
//!
//! - **The wrapper:** `waywall wrap -- <java> <args>`. waywall starts the command with
//!   `WAYLAND_DISPLAY` pointing at itself and exits when it exits; the game's output still goes
//!   to the launcher's log.
//! - **1.16.1 (LWJGL 3.2):** the patched GLFW, through `-Dorg.lwjgl.glfw.libname=<path>` (the
//!   MultiMC way; Prism's "use system GLFW" passes the same property). The waywall packages put
//!   it in `/usr/local/lib64/waywall-glfw/libglfw.so`; Nix in `~/.nix-profile/lib/libglfw.so`.
//!   Rules A.8.4.c and A.8.18 allow it only when the build is in legal-dlls.csv, which lists the
//!   builds from waywall's releases; a self-built one is used, with a warning.
//! - **26.x (Draftout):** no GLFW patch; `-DMC_DEBUG_ENABLED -DMC_DEBUG_PREFER_WAYLAND` instead,
//!   the two debug properties rule A.8.18 allows.
//! - **Environment:** `__GL_THREADED_OPTIMIZATIONS=0`, which the guide requires on NVIDIA (it
//!   fixes GLFW error 65544) and other drivers ignore.
//! - **A Wayland session:** waywall is a Wayland client of the desktop; without
//!   `WAYLAND_DISPLAY` it can't start, so the game starts without it, with a warning.
//!
//! waywall prints no version (`waywall --help` has none), so the status has no version.
use std::path::{Path, PathBuf};

use srl_core::Mode;
use srl_game::LaunchCommand;

use crate::{
    legal_dlls::LegalDlls,
    status::{InstalledTool, ToolStatus},
};

/// Where the patched GLFW is looked for, in order. `~` is the home directory.
pub const GLFW_PATHS: &[&str] = &[
    "/usr/local/lib64/waywall-glfw/libglfw.so",
    "/usr/local/lib/waywall-glfw/libglfw.so",
    "/usr/lib64/waywall-glfw/libglfw.so",
    "/usr/lib/waywall-glfw/libglfw.so",
    "~/.nix-profile/lib/libglfw.so",
    "~/.local/lib64/waywall-glfw/libglfw.so",
    "~/.local/lib/waywall-glfw/libglfw.so",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waywall {
    pub exe: PathBuf,
}

/// The patched GLFW for 1.16.1 and whether legal-dlls.csv lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glfw {
    pub path: PathBuf,
    /// The legal-dlls.csv version label (`3.4-linux`), `None` when it isn't listed (or there
    /// was no list to check it against).
    pub legal_as: Option<String>,
    /// False when no legal-dlls.csv was available, so `legal_as` says nothing.
    pub checked: bool,
}

/// `waywall` on `PATH` (an executable file), else `None`.
pub fn find() -> Option<Waywall> {
    let path = std::env::var_os("PATH")?;
    find_in(std::env::split_paths(&path))
}

pub fn find_in(dirs: impl IntoIterator<Item = PathBuf>) -> Option<Waywall> {
    dirs.into_iter()
        .map(|d| d.join("waywall"))
        .find(|p| is_executable(p))
        .map(|exe| Waywall { exe })
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

pub fn status() -> ToolStatus {
    if !cfg!(target_os = "linux") {
        return ToolStatus::NotAvailable;
    }
    match find() {
        Some(w) => ToolStatus::Installed(InstalledTool {
            version: None,
            path: w.exe,
            managed: false,
        }),
        None => ToolStatus::NotInstalled,
    }
}

/// The patched GLFW: `custom` if given, else the first of [`GLFW_PATHS`] that exists. Symlinks
/// are followed before hashing, since the packages ship `libglfw.so` as a link.
pub fn find_glfw(
    custom: Option<&Path>,
    home: Option<&Path>,
    legal: Option<&LegalDlls>,
) -> Option<Glfw> {
    let candidates: Vec<PathBuf> = match custom {
        Some(p) => vec![p.to_path_buf()],
        None => GLFW_PATHS
            .iter()
            .filter_map(|p| match p.strip_prefix("~/") {
                Some(rest) => home.map(|h| h.join(rest)),
                None => Some(PathBuf::from(p)),
            })
            .collect(),
    };
    let path = candidates.into_iter().find(|p| p.is_file())?;
    let legal_as = legal.and_then(|l| {
        let real = std::fs::canonicalize(&path).ok()?;
        let sha = srl_core::download::file_sha512(&real).ok()?;
        l.get(&sha).map(|row| row.version.clone())
    });
    Some(Glfw {
        path,
        legal_as,
        checked: legal.is_some(),
    })
}

/// Problems that keep waywall out of a launch, or that the player should know about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WrapIssue {
    NotFound,
    NoWaylandSession,
    /// 1.16.1 needs the patched GLFW and none was found.
    NoGlfw,
    /// Found, but legal-dlls.csv doesn't list it: runs with it may be rejected.
    GlfwNotLegal(PathBuf),
    /// Found, but there was no legal-dlls.csv to check it against (never fetched, offline).
    GlfwUnchecked(PathBuf),
}

/// True for the modes that need the patched GLFW (1.16.1, LWJGL 3.2).
pub fn needs_glfw(mode: Mode) -> bool {
    mode.minecraft_version().starts_with("1.")
}

/// `cmd` started through waywall for `mode`. `glfw` is required for the 1.16.1 modes.
pub fn wrap(
    cmd: &LaunchCommand,
    waywall: &Waywall,
    mode: Mode,
    glfw: Option<&Glfw>,
) -> LaunchCommand {
    let mut jvm: Vec<String> = Vec::new();
    if mode.minecraft_version().starts_with("1.") {
        if let Some(g) = glfw {
            jvm.push(format!("-Dorg.lwjgl.glfw.libname={}", g.path.display()));
        }
    } else {
        jvm.push("-DMC_DEBUG_ENABLED".into());
        jvm.push("-DMC_DEBUG_PREFER_WAYLAND".into());
    }
    let mut args = vec![
        "wrap".to_string(),
        "--".to_string(),
        cmd.program.display().to_string(),
    ];
    // JVM options go before everything else: the main class comes later in `cmd.args`.
    args.extend(jvm);
    args.extend(cmd.args.iter().cloned());
    let mut env = cmd.env.clone();
    env.push(("__GL_THREADED_OPTIMIZATIONS".into(), "0".into()));
    LaunchCommand {
        program: waywall.exe.clone(),
        args,
        cwd: cmd.cwd.clone(),
        env,
    }
}

/// What a launch with waywall would need, checked in order. `Ok(None)` when everything is fine;
/// `Err` means the game must start without waywall.
pub fn check(
    waywall: Option<&Waywall>,
    wayland_display: bool,
    mode: Mode,
    glfw: Option<&Glfw>,
) -> Result<Option<WrapIssue>, WrapIssue> {
    if waywall.is_none() {
        return Err(WrapIssue::NotFound);
    }
    if !wayland_display {
        return Err(WrapIssue::NoWaylandSession);
    }
    if mode.minecraft_version().starts_with("1.") {
        match glfw {
            None => return Err(WrapIssue::NoGlfw),
            Some(g) if !g.checked => {
                return Ok(Some(WrapIssue::GlfwUnchecked(g.path.clone())));
            }
            Some(g) if g.legal_as.is_none() => {
                return Ok(Some(WrapIssue::GlfwNotLegal(g.path.clone())));
            }
            Some(_) => {}
        }
    }
    Ok(None)
}

/// True if the player's waywall config starts Ninjabrain Bot itself, so the launcher shouldn't
/// start a second one outside waywall (where it gets no clipboard updates while waywall has
/// focus, and where the config's own "is it running?" check would find it and never start its
/// floating one).
///
/// Configs often keep the jar path in a variable (`local nb_path = ... "Ninjabrain-Bot-1.5.2.jar"`
/// in one place, `waywall.exec("java -jar " .. nb_path)` in another, maybe in another file), so
/// the whole config counts: some `.lua` file in the directory (or one level below) calls
/// `waywall.exec`, and some names a Ninjabrain jar. Comments don't count.
pub fn config_starts_ninjabrain(config_dir: &Path) -> bool {
    let mut execs = false;
    let mut names_jar = false;
    for path in lua_files(config_dir, 1) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let code = strip_lua_comments(&text).to_ascii_lowercase();
        execs |= code.contains("waywall.exec");
        names_jar |= names_ninjabrain_jar(&code);
        if execs && names_jar {
            return true;
        }
    }
    false
}

/// `.lua` files in `dir` and, `depth` levels down, in its subdirectories.
fn lua_files(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            if depth > 0 {
                out.extend(lua_files(&p, depth - 1));
            }
        } else if p.extension().is_some_and(|x| x == "lua") {
            out.push(p);
        }
    }
    out
}

/// A `ninjabrain…jar` file name in lower-case code: `ninjabrain` followed by `.jar` within the
/// same path-like token (no quote, space, bracket or comma in between).
fn names_ninjabrain_jar(code: &str) -> bool {
    code.match_indices("ninjabrain").any(|(i, _)| {
        let rest = &code[i..];
        let token = rest
            .find(|c: char| c.is_whitespace() || "\"'()[]{},;".contains(c))
            .map_or(rest, |end| &rest[..end]);
        token.contains(".jar")
    })
}

/// Lua source without its comments (`-- line` and `--[[ block ]]`). Strings are kept, and a `--`
/// inside a string is left alone.
fn strip_lua_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut quote: Option<char> = None;
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        if let Some(q) = quote {
            out.push(c);
            if c == '\\' {
                if let Some((_, escaped)) = chars.next() {
                    out.push(escaped);
                }
            } else if c == q || c == '\n' {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
        } else if c == '-' && text[i..].starts_with("--") {
            let rest = &text[i + 2..];
            // Where the comment ends: after `]]` for a block, before the line break for a line.
            let end = if rest.starts_with("[[") {
                rest.find("]]").map_or(text.len(), |e| i + 2 + e + 2)
            } else {
                rest.find('\n').map_or(text.len(), |e| i + 2 + e)
            };
            out.push(' ');
            while chars.as_str().len() > text.len() - end {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `$XDG_CONFIG_HOME/waywall`, else `~/.config/waywall`.
pub fn config_dir() -> Option<PathBuf> {
    match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(x) => Some(PathBuf::from(x).join("waywall")),
        None => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join("waywall")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> LaunchCommand {
        LaunchCommand {
            program: "/srl/java/java-runtime-delta-21.0.7/bin/java".into(),
            args: vec![
                "-Xmx2048M".into(),
                "-cp".into(),
                "a.jar:b.jar".into(),
                "net.fabricmc.loader.impl.launch.knot.KnotClient".into(),
                "--gameDir".into(),
                "/srl/instances/rsg".into(),
            ],
            cwd: "/srl/instances/rsg".into(),
            env: vec![],
        }
    }

    #[test]
    fn wraps_1_16_1_with_glfw() {
        let ww = Waywall {
            exe: "/usr/bin/waywall".into(),
        };
        let glfw = Glfw {
            path: "/usr/local/lib64/waywall-glfw/libglfw.so".into(),
            legal_as: Some("3.4-linux".into()),
            checked: true,
        };
        let cmd = wrap(&game(), &ww, Mode::Rsg, Some(&glfw));
        assert_eq!(cmd.program, Path::new("/usr/bin/waywall"));
        assert_eq!(
            cmd.args,
            [
                "wrap",
                "--",
                "/srl/java/java-runtime-delta-21.0.7/bin/java",
                "-Dorg.lwjgl.glfw.libname=/usr/local/lib64/waywall-glfw/libglfw.so",
                "-Xmx2048M",
                "-cp",
                "a.jar:b.jar",
                "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "--gameDir",
                "/srl/instances/rsg",
            ]
        );
        assert_eq!(cmd.cwd, Path::new("/srl/instances/rsg"));
        assert_eq!(
            cmd.env,
            [("__GL_THREADED_OPTIMIZATIONS".to_string(), "0".to_string())]
        );
    }

    #[test]
    fn wraps_26_x_with_debug_properties() {
        let ww = Waywall {
            exe: "/usr/bin/waywall".into(),
        };
        let cmd = wrap(&game(), &ww, Mode::Draftout, None);
        assert_eq!(
            &cmd.args[3..5],
            ["-DMC_DEBUG_ENABLED", "-DMC_DEBUG_PREFER_WAYLAND"]
        );
        assert!(!cmd.args.iter().any(|a| a.contains("glfw")));
    }

    #[test]
    fn checks_in_order() {
        let ww = Waywall {
            exe: "/usr/bin/waywall".into(),
        };
        let legal = Glfw {
            path: "/g".into(),
            legal_as: Some("3.4-linux".into()),
            checked: true,
        };
        let own = Glfw {
            path: "/own/libglfw.so".into(),
            legal_as: None,
            checked: true,
        };
        let unknown = Glfw {
            path: "/g".into(),
            legal_as: None,
            checked: false,
        };
        // No legal list to check against: a separate note, never "not legal".
        assert_eq!(
            check(Some(&ww), true, Mode::Rsg, Some(&unknown)),
            Ok(Some(WrapIssue::GlfwUnchecked("/g".into())))
        );
        assert_eq!(
            check(None, true, Mode::Rsg, Some(&legal)),
            Err(WrapIssue::NotFound)
        );
        assert_eq!(
            check(Some(&ww), false, Mode::Rsg, Some(&legal)),
            Err(WrapIssue::NoWaylandSession)
        );
        assert_eq!(
            check(Some(&ww), true, Mode::Rsg, None),
            Err(WrapIssue::NoGlfw)
        );
        assert_eq!(
            check(Some(&ww), true, Mode::Rsg, Some(&own)),
            Ok(Some(WrapIssue::GlfwNotLegal("/own/libglfw.so".into())))
        );
        assert_eq!(check(Some(&ww), true, Mode::Rsg, Some(&legal)), Ok(None));
        assert_eq!(check(Some(&ww), true, Mode::Draftout, None), Ok(None));
    }

    #[test]
    fn finds_executables_and_glfw() {
        let root = std::env::temp_dir().join(format!("srl-tools-ww-{}", std::process::id()));
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        assert_eq!(find_in([bin.clone()]), None);
        let exe = bin.join("waywall");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(find_in([bin.clone()]), None, "not executable yet");
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(
            find_in([root.join("nope"), bin.clone()]),
            Some(Waywall { exe: exe.clone() })
        );

        let home = root.join("home");
        let lib = home.join(".nix-profile/lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("libglfw.so"), b"glfw").unwrap();
        let sha = hex::encode(<sha2::Sha512 as sha2::Digest>::digest(b"glfw"));
        let csv = format!("filename,version,sha512\nlibglfw.so,3.4-linux,{sha}\n");
        let legal = LegalDlls::parse(&csv).unwrap();
        let g = find_glfw(None, Some(&home), Some(&legal)).unwrap();
        assert_eq!(g.path, lib.join("libglfw.so"));
        assert_eq!(g.legal_as.as_deref(), Some("3.4-linux"));
        let other = root.join("mine.so");
        std::fs::write(&other, b"other").unwrap();
        let g = find_glfw(Some(&other), Some(&home), Some(&legal)).unwrap();
        assert_eq!((g.path, g.legal_as, g.checked), (other.clone(), None, true));
        let g = find_glfw(Some(&other), Some(&home), None).unwrap();
        assert_eq!((g.legal_as, g.checked), (None, false));

        let cfg = root.join("cfg");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("init.lua"),
            "-- waywall.exec(\"java -jar ninjabrain.jar\")\n",
        )
        .unwrap();
        assert!(!config_starts_ninjabrain(&cfg));
        std::fs::write(
            cfg.join("init.lua"),
            "    [\"Ctrl-Shift-N\"] = function() waywall.exec(\"java -jar /x/Ninjabrain-Bot-1.5.2.jar\") end,\n",
        )
        .unwrap();
        assert!(config_starts_ninjabrain(&cfg));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The pattern of arjuncgore/waywall_generic_config (main.lua): the jar path in a variable,
    /// the exec line without the word "ninjabrain". Also split over two files.
    #[test]
    fn finds_ninjabrain_started_through_a_variable() {
        let root = std::env::temp_dir().join(format!("srl-tools-wwcfg-{}", std::process::id()));
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let generic = r#"local waywall = require("waywall")
local nb_path = waywall_config_path .. "resources/Ninjabrain-Bot-1.5.2.jar"
local is_ninb_running = function()
    local handle = io.popen("ps aux | grep '[N]injabrain-Bot.*\\.jar'")
end
-- waywall.exec("java -jar old.jar")
local toggle = function()
    if not is_ninb_running() then
        waywall.exec("java -Dawt.useSystemAAFontSettings=on -jar " .. nb_path)
    end
end
"#;
        std::fs::write(root.join("main.lua"), generic).unwrap();
        assert!(config_starts_ninjabrain(&root));

        // The path in one file, the exec in a module one level down.
        std::fs::remove_file(root.join("main.lua")).unwrap();
        std::fs::write(
            root.join("init.lua"),
            "local nb = \"/home/a/Ninjabrain-Bot-1.5.1.jar\"\n",
        )
        .unwrap();
        assert!(!config_starts_ninjabrain(&root), "no exec anywhere yet");
        std::fs::write(
            root.join("lib/keys.lua"),
            "return function(p) waywall.exec(\"java -jar \" .. p) end\n",
        )
        .unwrap();
        assert!(config_starts_ninjabrain(&root));

        // Only comments name the jar: not started by the config.
        std::fs::write(
            root.join("init.lua"),
            "--[[ local nb = \"/home/a/Ninjabrain-Bot-1.5.1.jar\" ]]\n-- Ninjabrain-Bot.jar\n",
        )
        .unwrap();
        assert!(!config_starts_ninjabrain(&root));
        // "ninjabrain" without a jar (a window name for a mirror) doesn't count either.
        std::fs::write(
            root.join("init.lua"),
            "local nb_title = \"Ninjabrain Bot\" -- the jar: x.jar\n",
        )
        .unwrap();
        assert!(!config_starts_ninjabrain(&root));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn strips_lua_comments_only() {
        assert_eq!(strip_lua_comments("a -- b\nc"), "a  \nc");
        assert_eq!(strip_lua_comments("a --[[ b\nb ]] c"), "a   c");
        assert_eq!(
            strip_lua_comments("x = \"--not\" -- yes"),
            "x = \"--not\"  "
        );
        assert_eq!(strip_lua_comments("s = 'it\\'s' -- c"), "s = 'it\\'s'  ");
    }
}
