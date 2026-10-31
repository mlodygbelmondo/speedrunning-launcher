//! What each mode needs from the game side: Java, the oldest Fabric Loader its mods accept, and
//! JVM flags. The Minecraft version itself is `Mode::minecraft_version` in srl-core.
//!
//! **Java.** We pick the runtime per mode and ignore the version JSON's `javaVersion`, because
//! that field describes vanilla, not a modded speedrun instance (research/02 §6, research/03
//! §2.3):
//!
//! - **1.16.1 modes (Ranked, RSG, FSG, SSG): Mojang `java-runtime-delta`, Java 21.** 1.16.1's
//!   JSON asks for Java 8 (`jre-legacy`), but modern Java is the speedrun standard: the MCSR
//!   Ranked FAQ "highly recommends" Java 17+, the Ranked mod bundles Java 16 bytecode, and the
//!   Voyager mod exists to fix the MC-149777 crash on modern Java. The MCSR settings guide
//!   recommends Java 17 to 22 and warns against 23+ (from 24 on, generational ZGC can't be
//!   turned off, which hurts SeedQueue). Of Mojang's runtimes that leaves gamma (17) and delta
//!   (21); 21 is the newer LTS and the guide's default.
//! - **Draftout (26.3): Mojang `java-runtime-epsilon`, Java 25.** Draftout requires Java 25+, and
//!   26.3's own JSON asks for epsilon.
//!
//! Mojang's runtimes are used because they are listed file by file with a sha1 for each, which
//! fits our verified downloads; Adoptium or GraalVM can come later as an option.
use anyhow::{Result, bail};
use srl_core::{Instance, Mode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeProfile {
    pub minecraft_version: &'static str,
    /// Component name in Mojang's Java runtime manifest.
    pub java_component: &'static str,
    pub java_major: u32,
    /// The oldest Fabric Loader the mode's mods accept. We install the newest stable loader at
    /// or above it.
    pub min_fabric_loader: &'static str,
    /// Always passed, before the user's own JVM arguments.
    pub jvm_args: &'static [&'static str],
    /// The default `-Xmx`, in MB.
    pub default_memory_mb: u32,
    /// The least `-Xmx` the mode runs with, in MB, whatever the player's setting: one memory
    /// setting covers every mode, and 2 GB that suits RSG is below what Draftout asks for.
    pub min_memory_mb: u32,
}

/// The mode's game-side requirements.
pub fn profile(mode: Mode) -> ModeProfile {
    match mode {
        Mode::Ranked | Mode::Rsg | Mode::Fsg | Mode::Ssg => ModeProfile {
            minecraft_version: mode.minecraft_version(),
            java_component: "java-runtime-delta",
            java_major: 21,
            // SeedQueue and the Ranked mod need Fabric Loader 0.16+.
            min_fabric_loader: "0.16.0",
            jvm_args: &[],
            // research/02 §6: about 2000 to 2500 MB plus ~250 MB per SeedQueue queued seed.
            default_memory_mb: 2048,
            min_memory_mb: 2048,
        },
        Mode::Draftout => ModeProfile {
            minecraft_version: mode.minecraft_version(),
            java_component: "java-runtime-epsilon",
            java_major: 25,
            // draftoutmc.com/download: Fabric Loader 0.19.5 or newer.
            min_fabric_loader: "0.19.5",
            // Draftout's Java 25 crash workaround (draftoutmc.com/download). Its other flag,
            // -XX:StackShadowPages=32, is already in 26.3's own JVM arguments.
            jvm_args: &[
                "-XX:CompileCommand=exclude,io/netty/util/internal/ReferenceCountUpdater,retryRelease0",
            ],
            // Draftout asks for at least 3 GB; 26.3's default is 4 GB.
            default_memory_mb: 4096,
            min_memory_mb: 3072,
        },
    }
}

/// Mojang's Java runtime component for each Java feature version it ships (the components named
/// in its `java-runtime` list and in version JSONs' `javaVersion`).
const JAVA_COMPONENTS: [(u32, &str); 5] = [
    (8, "jre-legacy"),
    (16, "java-runtime-alpha"),
    (17, "java-runtime-gamma"),
    (21, "java-runtime-delta"),
    (25, "java-runtime-epsilon"),
];

/// The Mojang runtime component of Java `major`, if Mojang ships one.
pub fn java_component(major: u32) -> Option<&'static str> {
    JAVA_COMPONENTS
        .iter()
        .find(|(m, _)| *m == major)
        .map(|(_, c)| *c)
}

/// What [`crate::GameInstaller::install_target`] installs and starts: a game directory (one of the
/// modes, or the practice instance) and the game it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub instance: Instance,
    pub profile: ModeProfile,
}

impl Target {
    /// A mode, with its own profile ([`profile`]).
    pub fn mode(mode: Mode) -> Self {
        Self {
            instance: mode.into(),
            profile: profile(mode),
        }
    }

    /// The practice instance (`Paths::practice_instance`). Its game comes from the caller (srl-
    /// practice's `PRACTICE` profile), so srl-game needn't depend on srl-practice. No JVM flags
    /// of its own and the 1.16.1 modes' memory: practice maps are 1.16.1 worlds played the same
    /// way.
    pub fn practice(
        minecraft_version: &'static str,
        java_major: u32,
        min_fabric_loader: &'static str,
    ) -> Result<Self> {
        let Some(java_component) = java_component(java_major) else {
            bail!("Mojang ships no Java {java_major} runtime");
        };
        Ok(Self {
            instance: Instance::Practice,
            profile: ModeProfile {
                minecraft_version,
                java_component,
                java_major,
                min_fabric_loader,
                jvm_args: &[],
                default_memory_mb: 2048,
                min_memory_mb: 2048,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn practice_target_and_java_components() {
        for mode in Mode::ALL {
            let p = profile(mode);
            assert_eq!(java_component(p.java_major), Some(p.java_component));
            assert_eq!(Target::mode(mode).instance, Instance::Mode(mode));
        }
        let t = Target::practice("1.16.1", 21, "0.16.0").unwrap();
        assert_eq!(t.instance, Instance::Practice);
        assert_eq!(t.profile.java_component, "java-runtime-delta");
        assert!(t.profile.jvm_args.is_empty());
        assert!(Target::practice("1.16.1", 22, "0.16.0").is_err());
    }

    #[test]
    fn java_per_mode() {
        for mode in [Mode::Ranked, Mode::Rsg, Mode::Fsg, Mode::Ssg] {
            assert_eq!(profile(mode).java_major, 21);
            assert_eq!(profile(mode).minecraft_version, "1.16.1");
        }
        assert_eq!(
            profile(Mode::Draftout).java_component,
            "java-runtime-epsilon"
        );
    }
}
