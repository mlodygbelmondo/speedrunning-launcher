//! OS, architecture and feature rules on libraries and arguments.
//!
//! Rule evaluation is adapted from Pandora Launcher (MIT, commit 2d83d6f),
//! `crates/backend/src/launch/mod.rs` (`LaunchRuleContext::check_rules`/`check_rule`). See
//! THIRD_PARTY_NOTICES.md. Changes: the platform is a value instead of `std::env::consts`, so
//! tests can evaluate Windows and macOS rules on Linux; features are a generic map; and the
//! 26.x `versionRange` condition is supported.
use std::sync::OnceLock;

use crate::schema::{Rule, RuleAction, RuleOs, VersionRange};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    MacOs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    X86,
    Aarch64,
}

/// The machine the game runs on, as the rules see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
    /// E.g. `10.0.22631` on Windows 11, `6.8.0-45-generic` on Linux. Empty if unknown.
    pub os_version: String,
}

impl Platform {
    /// The platform this binary was built for, with the running OS version.
    pub fn current() -> Platform {
        static CURRENT: OnceLock<Platform> = OnceLock::new();
        CURRENT
            .get_or_init(|| Platform {
                os: if cfg!(windows) {
                    Os::Windows
                } else if cfg!(target_os = "macos") {
                    Os::MacOs
                } else {
                    Os::Linux
                },
                arch: if cfg!(target_arch = "aarch64") {
                    Arch::Aarch64
                } else if cfg!(target_arch = "x86") {
                    Arch::X86
                } else {
                    Arch::X86_64
                },
                os_version: os_version(),
            })
            .clone()
    }

    /// The `os.name` used in version JSONs.
    pub fn mojang_os_name(&self) -> &'static str {
        match self.os {
            Os::Windows => "windows",
            Os::Linux => "linux",
            Os::MacOs => "osx",
        }
    }

    /// The value of `${arch}` in old native classifiers (`natives-windows-${arch}`).
    pub fn bits(&self) -> &'static str {
        match self.arch {
            Arch::X86 => "32",
            _ => "64",
        }
    }

    /// The classpath separator of the target OS.
    pub fn classpath_separator(&self) -> &'static str {
        match self.os {
            Os::Windows => ";",
            _ => ":",
        }
    }
}

/// Feature flags the game JSONs test. Quick Play is never used by us, so its flags stay false.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features {
    pub is_demo_user: bool,
    pub has_custom_resolution: bool,
}

impl Features {
    fn get(&self, name: &str) -> bool {
        match name {
            "is_demo_user" => self.is_demo_user,
            "has_custom_resolution" => self.has_custom_resolution,
            // has_quick_plays_support, is_quick_play_* and anything new.
            _ => false,
        }
    }
}

/// Mojang's semantics: no rules means allowed; otherwise start from "disallowed" and let every
/// matching rule set the result, so the last matching rule wins.
pub fn rules_allow(rules: &[Rule], platform: &Platform, features: &Features) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, platform, features) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

/// A rule matches when all of its conditions match.
pub fn rule_matches(rule: &Rule, platform: &Platform, features: &Features) -> bool {
    if let Some(wanted) = &rule.features
        && wanted
            .iter()
            .any(|(name, value)| features.get(name) != *value)
    {
        return false;
    }
    match &rule.os {
        Some(os) => os_matches(os, platform),
        None => true,
    }
}

fn os_matches(os: &RuleOs, platform: &Platform) -> bool {
    if let Some(name) = &os.name
        && name != platform.mojang_os_name()
    {
        return false;
    }
    if let Some(arch) = &os.arch {
        let ok = match arch.as_str() {
            "x86" => platform.arch == Arch::X86,
            "x86_64" | "x64" | "amd64" => platform.arch == Arch::X86_64,
            "arm64" | "aarch64" => platform.arch == Arch::Aarch64,
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    if let Some(pattern) = &os.version {
        // An invalid regex can't match anything we know, so treat it as not matching.
        match regex::Regex::new(pattern) {
            Ok(re) if re.is_match(&platform.os_version) => {}
            _ => return false,
        }
    }
    if let Some(range) = &os.version_range
        && !in_range(&platform.os_version, range)
    {
        return false;
    }
    true
}

/// `min` is inclusive and `max` exclusive: 26.3 pairs `{min: 10.0.17134}` (ZGC) with
/// `{max: 10.0.17134}` (G1), which must not both apply to the same Windows build.
fn in_range(version: &str, range: &VersionRange) -> bool {
    let v = parse_version(version);
    if v.is_empty() {
        // Unknown OS version: assume a current one, so `min` passes and `max` fails.
        return range.max.is_none();
    }
    if let Some(min) = &range.min
        && v < parse_version(min)
    {
        return false;
    }
    if let Some(max) = &range.max
        && v >= parse_version(max)
    {
        return false;
    }
    true
}

/// Leading numeric components: `6.8.0-45-generic` -> [6, 8, 0].
pub(crate) fn parse_version(s: &str) -> Vec<u64> {
    let mut out = Vec::new();
    for part in s.split(['.', '-', '+']) {
        match part.parse() {
            Ok(n) => out.push(n),
            Err(_) => break,
        }
    }
    out
}

#[cfg(windows)]
fn os_version() -> String {
    // RtlGetVersion tells the truth; GetVersionEx lies to programs without a manifest.
    use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    // SAFETY: OSVERSIONINFOW is plain data; RtlGetVersion only writes into it.
    unsafe {
        let mut info: OSVERSIONINFOW = std::mem::zeroed();
        info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
        if RtlGetVersion(&mut info) == 0 {
            format!(
                "{}.{}.{}",
                info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
            )
        } else {
            String::new()
        }
    }
}

#[cfg(target_os = "linux")]
fn os_version() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn os_version() -> String {
    // macOS: only pre-2013 versions have OS version rules, none of which we launch.
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform(os: Os, arch: Arch, version: &str) -> Platform {
        Platform {
            os,
            arch,
            os_version: version.into(),
        }
    }

    fn rules(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn no_rules_means_allowed() {
        let linux = platform(Os::Linux, Arch::X86_64, "6.8.0");
        assert!(rules_allow(&[], &linux, &Features::default()));
    }

    #[test]
    fn last_matching_rule_wins() {
        // 1.16.1's LWJGL 3.2.2: everywhere but macOS.
        let r = rules(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
        let f = Features::default();
        assert!(rules_allow(&r, &platform(Os::Linux, Arch::X86_64, ""), &f));
        assert!(rules_allow(
            &r,
            &platform(Os::Windows, Arch::X86_64, ""),
            &f
        ));
        assert!(!rules_allow(
            &r,
            &platform(Os::MacOs, Arch::Aarch64, ""),
            &f
        ));
        // A rule list where nothing matches is a disallow.
        let osx_only = rules(r#"[{"action":"allow","os":{"name":"osx"}}]"#);
        assert!(!rules_allow(
            &osx_only,
            &platform(Os::Linux, Arch::X86_64, ""),
            &f
        ));
    }

    #[test]
    fn arch_and_version_regex() {
        let f = Features::default();
        let x86 = rules(r#"[{"action":"allow","os":{"arch":"x86"}}]"#);
        assert!(!rules_allow(
            &x86,
            &platform(Os::Windows, Arch::X86_64, ""),
            &f
        ));
        assert!(rules_allow(&x86, &platform(Os::Windows, Arch::X86, ""), &f));
        let win10 = rules(r#"[{"action":"allow","os":{"name":"windows","version":"^10\\."}}]"#);
        assert!(rules_allow(
            &win10,
            &platform(Os::Windows, Arch::X86_64, "10.0.22631"),
            &f
        ));
        assert!(!rules_allow(
            &win10,
            &platform(Os::Windows, Arch::X86_64, "6.1.7601"),
            &f
        ));
    }

    #[test]
    fn version_range_is_half_open() {
        let f = Features::default();
        let zgc = rules(
            r#"[{"action":"allow","os":{"name":"windows","versionRange":{"min":"10.0.17134"}}}]"#,
        );
        let g1 = rules(
            r#"[{"action":"allow","os":{"name":"windows","versionRange":{"max":"10.0.17134"}}}]"#,
        );
        for (version, want_zgc) in [
            ("10.0.22631", true),
            ("10.0.17134", true),
            ("10.0.16299", false),
        ] {
            let p = platform(Os::Windows, Arch::X86_64, version);
            assert_eq!(rules_allow(&zgc, &p, &f), want_zgc, "{version}");
            assert_eq!(rules_allow(&g1, &p, &f), !want_zgc, "{version}");
        }
        // Unknown version: assume a modern OS.
        let unknown = platform(Os::Windows, Arch::X86_64, "");
        assert!(rules_allow(&zgc, &unknown, &f));
        assert!(!rules_allow(&g1, &unknown, &f));
    }

    #[test]
    fn features() {
        let r = rules(r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#);
        let p = platform(Os::Linux, Arch::X86_64, "");
        assert!(!rules_allow(&r, &p, &Features::default()));
        let custom = Features {
            has_custom_resolution: true,
            ..Features::default()
        };
        assert!(rules_allow(&r, &p, &custom));
        // Quick Play is never on.
        let qp = rules(r#"[{"action":"allow","features":{"is_quick_play_singleplayer":true}}]"#);
        assert!(!rules_allow(&qp, &p, &custom));
    }
}
