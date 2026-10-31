//! The JSON documents we read: Mojang's version manifest, version JSON, asset index and Java
//! runtime manifests, and Fabric's loader list and profile JSON.
//!
//! Adapted from Pandora Launcher (MIT, commit 2d83d6f), `crates/schema/src/version.rs`,
//! `version_manifest.rs`, `assets_index.rs`, `java_runtimes.rs` and `java_runtime_component.rs`.
//! See THIRD_PARTY_NOTICES.md. Changes: plain `String`s instead of `Ustr`, every field we don't
//! need is left out (unknown fields are ignored, so a new Mojang field never breaks a launch),
//! Fabric-style libraries (`url` + `sha1` + `size`) share the vanilla `Library` type, and rules
//! understand the `versionRange` condition that the 26.x `default-user-jvm` arguments use.
use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const JAVA_RUNTIMES_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
pub const ASSETS_URL: &str = "https://resources.download.minecraft.net";
pub const MOJANG_LIBRARIES_URL: &str = "https://libraries.minecraft.net/";
pub const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";

// --- Version manifest -----------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionLink>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VersionLink {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    /// Hash of the version JSON itself.
    pub sha1: String,
}

// --- Version JSON (vanilla, and the Fabric profile that inherits from it) --------------------

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    /// Set on loader profiles (Fabric): the vanilla version they extend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    /// One space-separated string of game arguments. Only versions before 1.13 (17w43a).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndexLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<VersionDownloads>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
    /// What the official launcher pre-fills as the user's JVM arguments (26.x: heap size, ZGC,
    /// compact object headers). Used only when the user gives no JVM arguments of their own.
    #[serde(
        default,
        rename = "default-user-jvm",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub default_user_jvm: Vec<Argument>,
}

/// A plain string, or values that apply only when their rules allow them.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Ruled {
        #[serde(default)]
        rules: Vec<Rule>,
        value: ArgumentValue,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    One(String),
    Many(Vec<String>),
}

impl ArgumentValue {
    pub fn values(&self) -> &[String] {
        match self {
            ArgumentValue::One(v) => std::slice::from_ref(v),
            ArgumentValue::Many(v) => v,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<RuleOs>,
    /// Feature flags such as `is_demo_user` or `has_custom_resolution`. The rule matches when
    /// every listed flag has the listed value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<BTreeMap<String, bool>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleOs {
    /// `windows`, `osx` or `linux`. Kept as a string so an unknown name simply never matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Only `x86` (32-bit) appears in Mojang's JSONs; `arm64` appears in some third-party ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    /// Regex on the OS version, e.g. `^10\\.` (1.16.1 and older).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// `{min, max}` OS version range (26.x `default-user-jvm`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_range: Option<VersionRange>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct VersionRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexLink {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VersionDownloads {
    pub client: FileLink,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileLink {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

/// A library entry. Vanilla entries carry `downloads`; Fabric entries carry a Maven `name`, a
/// repository `url` and (for most of them) `sha1` and `size`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Library {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    /// OS name to classifier, e.g. `{"linux": "natives-linux"}`. Only versions before 1.19
    /// (22w19a); newer ones list each native jar as its own library with an OS rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<ExtractRules>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LibraryDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Artifact {
    /// Maven-layout path under `libraries/`.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Logging {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<LoggingTarget>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoggingTarget {
    /// E.g. `-Dlog4j.configurationFile=${path}`.
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

// --- Asset index ----------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    /// 1.7 and older: objects are copied to `assets/virtual/legacy/<path>`.
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    /// 1.5 and older: objects are copied into `<gameDir>/resources/`.
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

// --- Java runtimes --------------------------------------------------------------------------

/// `all.json`: platform -> component -> list of builds (empty when Mojang has none).
pub type JavaRuntimes = HashMap<String, HashMap<String, Vec<JavaRuntimeBuild>>>;

#[derive(Clone, Debug, Deserialize)]
pub struct JavaRuntimeBuild {
    pub manifest: FileLink,
    pub version: JavaRuntimeVersion,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JavaRuntimeVersion {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JavaRuntimeManifest {
    pub files: BTreeMap<String, JavaRuntimeFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JavaRuntimeFile {
    Directory,
    File {
        #[serde(default)]
        executable: bool,
        downloads: JavaRuntimeFileDownloads,
    },
    Link {
        target: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct JavaRuntimeFileDownloads {
    /// We download the raw file: LZMA saves only 5 to 30 % here and would need a decoder.
    pub raw: FileLink,
}

// --- Fabric ---------------------------------------------------------------------------------

/// One entry of `/v2/versions/loader/<game>`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FabricLoaderEntry {
    pub loader: FabricLoaderVersion,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FabricLoaderVersion {
    pub version: String,
    pub stable: bool,
}
