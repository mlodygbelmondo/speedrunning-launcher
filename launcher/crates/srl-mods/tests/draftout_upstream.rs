//! Whether the newest Draftout still matches what the launcher assumes about it. Ignored by
//! default (network); CI runs it daily (`.github/workflows/draftout-check.yml`):
//!
//! ```text
//! srl-cpu cargo test -p srl-mods --test draftout_upstream -- --ignored --nocapture
//! ```
//!
//! Draftout refuses to start with a mod outside the `ALLOWED_MODS` of its
//! `DisallowedModsChecker`, so `allowed_only(Mode::Draftout)` has to be that list exactly: a mod
//! it newly allows is one the Draftout set could have, and one it dropped would stop the game.
//! It also has to run on the Minecraft version the launcher installs for Draftout.
use std::{collections::BTreeSet, io::Read};

use serde::Deserialize;
use srl_core::{Downloader, Mode};
use srl_mods::allowed_only;

const VERSIONS: &str = "https://api.modrinth.com/v2/project/draftout/version";
const CHECKER: &str = "com/draftoutmc/draftout/DisallowedModsChecker.class";

#[derive(Deserialize)]
struct Version {
    version_number: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<File>,
}

#[derive(Deserialize)]
struct File {
    url: String,
    primary: bool,
}

/// The string literals of a class file (the `CONSTANT_String` entries of its constant pool).
fn string_literals(class: &[u8]) -> Vec<String> {
    let u16_at = |i: usize| u16::from_be_bytes([class[i], class[i + 1]]) as usize;
    assert_eq!(class[..4], [0xCA, 0xFE, 0xBA, 0xBE], "not a class file");
    let count = u16_at(8);
    let (mut utf8, mut strings) = (vec![None; count], Vec::new());
    let (mut i, mut index) = (10, 1);
    while index < count {
        let tag = class[i];
        i += 1;
        match tag {
            1 => {
                let len = u16_at(i);
                utf8[index] =
                    Some(String::from_utf8_lossy(&class[i + 2..i + 2 + len]).into_owned());
                i += 2 + len;
            }
            8 => {
                strings.push(u16_at(i));
                i += 2;
            }
            3 | 4 => i += 4,
            // Longs and doubles take two pool slots.
            5 | 6 => {
                i += 8;
                index += 1;
            }
            7 | 16 | 19 | 20 => i += 2,
            9..=12 | 17 | 18 => i += 4,
            15 => i += 3,
            other => panic!("constant pool tag {other} at byte {i}"),
        }
        index += 1;
    }
    strings
        .into_iter()
        .filter_map(|s| utf8.get(s).cloned().flatten())
        .collect()
}

/// The mod ids among the literals: Draftout's `ALLOWED_MODS` (`Set.of("draftout", "sodium", …)`).
/// Its other literals are the folder name `mods`, a separator and the crash message.
fn allowed_mods(class: &[u8]) -> BTreeSet<String> {
    string_literals(class)
        .into_iter()
        .filter(|s| {
            s != "mods"
                && !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        })
        .collect()
}

#[tokio::test]
#[ignore = "network: asks Modrinth for the newest Draftout and downloads its jar"]
async fn newest_draftout_matches_the_launcher() {
    let dl = Downloader::new().unwrap();
    let versions: Vec<Version> = dl.fetch_json(VERSIONS).await.unwrap();
    let newest = versions
        .iter()
        .find(|v| v.loaders.iter().any(|l| l == "fabric"))
        .expect("a Fabric build of Draftout");
    let mc = Mode::Draftout.minecraft_version();
    assert!(
        newest.game_versions.iter().any(|v| v == mc),
        "Draftout {} is for Minecraft {:?}; the launcher installs {mc} for Draftout",
        newest.version_number,
        newest.game_versions
    );
    let file = newest
        .files
        .iter()
        .find(|f| f.primary)
        .or(newest.files.first())
        .expect("a jar");
    let jar = dl.fetch_bytes(&file.url).await.unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(jar)).unwrap();
    let mut class = Vec::new();
    zip.by_name(CHECKER)
        .unwrap_or_else(|e| panic!("Draftout {} has no {CHECKER}: {e}", newest.version_number))
        .read_to_end(&mut class)
        .unwrap();

    let upstream = allowed_mods(&class);
    let ours: BTreeSet<String> = allowed_only(Mode::Draftout)
        .unwrap()
        .iter()
        .map(|s| s.to_string())
        .collect();
    println!("Draftout {}: {upstream:?}", newest.version_number);
    assert_eq!(
        upstream,
        ours,
        "Draftout {}'s allowed mods changed: it now allows {:?} and no longer allows {:?}. \
         Update `allowed_only` in crates/srl-mods/src/resolve.rs (and the extras, if one is new).",
        newest.version_number,
        upstream.difference(&ours).collect::<Vec<_>>(),
        ours.difference(&upstream).collect::<Vec<_>>()
    );
}

#[test]
fn literals_of_a_tiny_class() {
    // Magic, version, a pool of: 1 Utf8 "sodium", 2 String #1, 3 Utf8 "mods", 4 String #3,
    // 5 Long (two slots), 7 Utf8 "Not A Mod", 8 String #7.
    let mut c = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65, 0, 9];
    let utf8 = |c: &mut Vec<u8>, s: &str| {
        c.push(1);
        c.extend((s.len() as u16).to_be_bytes());
        c.extend(s.as_bytes());
    };
    utf8(&mut c, "sodium");
    c.extend([8, 0, 1]);
    utf8(&mut c, "mods");
    c.extend([8, 0, 3]);
    c.extend([5, 0, 0, 0, 0, 0, 0, 0, 7]);
    utf8(&mut c, "Not A Mod");
    c.extend([8, 0, 7]);
    assert_eq!(string_literals(&c), ["sodium", "mods", "Not A Mod"]);
    assert_eq!(allowed_mods(&c), BTreeSet::from(["sodium".to_string()]));
}
