//! Builds a `LaunchPlan` from a Forge descriptor and its vanilla parent. Pure and synchronous.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use super::{
    DomainError, LaunchPlan, OfflineAccount, Settings,
    version::{
        Argument, ArgumentValue, Arguments, Library, OsRule, Rule, RuleAction, VersionDescriptor,
    },
};

pub const LAUNCHER_NAME: &str = "PanasRPGLauncher";
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    Windows,
    Linux,
    MacOs,
}

impl HostOs {
    /// Name used by Mojang rules and natives maps.
    pub fn rule_name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::MacOs => "osx",
        }
    }

    pub fn classpath_separator(self) -> &'static str {
        match self {
            Self::Windows => ";",
            Self::Linux | Self::MacOs => ":",
        }
    }
}

/// Machine the game runs on. Injected by the composition root so the domain stays deterministic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEnvironment {
    pub os: HostOs,
    /// Rust-style architecture name: `x86_64`, `x86`, `aarch64`...
    pub arch: String,
    pub os_version: Option<String>,
}

impl HostEnvironment {
    pub fn new(os: HostOs, arch: impl Into<String>) -> Self {
        Self {
            os,
            arch: arch.into(),
            os_version: None,
        }
    }

    fn matches(&self, rule: &OsRule) -> bool {
        let name_ok = rule
            .name
            .as_deref()
            .is_none_or(|name| name == self.os.rule_name());
        let arch_ok = rule
            .arch
            .as_deref()
            .is_none_or(|arch| self.arch_matches(arch));
        let version_ok = rule.version.as_deref().is_none_or(|pattern| {
            self.os_version
                .as_deref()
                .is_some_and(|version| simple_regex_find(pattern, version))
        });
        name_ok && arch_ok && version_ok
    }

    fn arch_matches(&self, rule_arch: &str) -> bool {
        let host = match self.arch.as_str() {
            "x86_64" | "amd64" => "x86_64",
            "x86" | "i386" | "i686" => "x86",
            "aarch64" | "arm64" => "arm64",
            other => other,
        };
        match rule_arch {
            "x86" => host == "x86",
            "x86_64" | "amd64" => host == "x86_64",
            "arm64" | "aarch64" => host == "arm64",
            other => host == other,
        }
    }
}

/// Feature flags Mojang rules may test. Unknown features count as `false`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchFeatures(HashMap<String, bool>);

impl LaunchFeatures {
    pub fn get(&self, name: &str) -> bool {
        self.0.get(name).copied().unwrap_or(false)
    }
}

impl Default for LaunchFeatures {
    fn default() -> Self {
        Self(HashMap::from([
            ("is_demo_user".to_owned(), false),
            ("has_custom_resolution".to_owned(), true),
            ("has_quick_plays_support".to_owned(), false),
            ("is_quick_play_singleplayer".to_owned(), false),
            ("is_quick_play_multiplayer".to_owned(), false),
            ("is_quick_play_realms".to_owned(), false),
        ]))
    }
}

/// Mojang semantics: no rules means allowed; otherwise the last matching rule decides, default deny.
pub fn rules_allow(rules: &[Rule], host: &HostEnvironment, features: &LaunchFeatures) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, host, features) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, host: &HostEnvironment, features: &LaunchFeatures) -> bool {
    let os_ok = rule.os.as_ref().is_none_or(|os| host.matches(os));
    let features_ok = rule.features.as_ref().is_none_or(|wanted| {
        wanted
            .iter()
            .all(|(name, value)| features.get(name) == *value)
    });
    os_ok && features_ok
}

/// Unanchored search supporting `^`, `$`, `.` and backslash escapes, which covers the patterns Mojang ships
/// (e.g. `^10\.`). Patterns with other metacharacters never match rather than matching wrongly.
fn simple_regex_find(pattern: &str, text: &str) -> bool {
    let anchored_start = pattern.starts_with('^');
    let body = pattern.strip_prefix('^').unwrap_or(pattern);
    let anchored_end = body.ends_with('$') && !body.ends_with("\\$");
    let body = if anchored_end {
        &body[..body.len() - 1]
    } else {
        body
    };

    let mut tokens: Vec<Option<char>> = Vec::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(escaped) => tokens.push(Some(escaped)),
                None => return false,
            },
            '.' => tokens.push(None),
            '*' | '+' | '?' | '[' | ']' | '(' | ')' | '{' | '}' | '|' => return false,
            literal => tokens.push(Some(literal)),
        }
    }

    let text: Vec<char> = text.chars().collect();
    let matches_at = |start: usize| {
        let end = start + tokens.len();
        end <= text.len()
            && (!anchored_end || end == text.len())
            && tokens
                .iter()
                .zip(&text[start..end])
                .all(|(token, c)| token.is_none_or(|t| t == *c))
    };
    if anchored_start {
        matches_at(0)
    } else {
        (0..=text.len()).any(matches_at)
    }
}

/// Maven coordinates `group:artifact:version[:classifier][@extension]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenCoordinates {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl MavenCoordinates {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let invalid =
            || DomainError::InvalidVersionDescriptor(format!("bad maven coordinates: {raw}"));
        let (coords, extension) = match raw.split_once('@') {
            Some((coords, ext)) => (coords, ext.to_owned()),
            None => (raw, "jar".to_owned()),
        };
        let parts: Vec<&str> = coords.split(':').collect();
        if !(3..=4).contains(&parts.len()) || parts.iter().any(|p| p.is_empty()) {
            return Err(invalid());
        }
        Ok(Self {
            group: parts[0].to_owned(),
            artifact: parts[1].to_owned(),
            version: parts[2].to_owned(),
            classifier: parts.get(3).map(|c| (*c).to_owned()),
            extension,
        })
    }

    /// Relative path inside a Maven repository, always with `/`.
    pub fn repository_path(&self) -> String {
        let classifier = self
            .classifier
            .as_ref()
            .map(|c| format!("-{c}"))
            .unwrap_or_default();
        format!(
            "{}/{}/{}/{}-{}{}.{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            self.artifact,
            self.version,
            classifier,
            self.extension
        )
    }

    /// Identity used to deduplicate libraries across versions: `group:artifact[:classifier]`.
    pub fn identity(&self) -> String {
        match &self.classifier {
            Some(classifier) => format!("{}:{}:{}", self.group, self.artifact, classifier),
            None => format!("{}:{}", self.group, self.artifact),
        }
    }
}

fn library_identity(library: &Library) -> String {
    MavenCoordinates::parse(&library.name)
        .map(|c| c.identity())
        .unwrap_or_else(|_| library.name.clone())
}

/// Merges a child descriptor (Forge) over its parent (vanilla). The child wins on duplicated libraries
/// and scalar fields; arguments are concatenated parent first.
pub fn merge_descriptors(
    child: &VersionDescriptor,
    parent: &VersionDescriptor,
) -> VersionDescriptor {
    let child_ids: HashSet<String> = child.libraries.iter().map(library_identity).collect();
    let libraries = child
        .libraries
        .iter()
        .cloned()
        .chain(
            parent
                .libraries
                .iter()
                .filter(|lib| !child_ids.contains(&library_identity(lib)))
                .cloned(),
        )
        .collect();

    let arguments = match (&parent.arguments, &child.arguments) {
        (None, None) => None,
        (parent_args, child_args) => {
            let parent_args = parent_args.clone().unwrap_or_default();
            let child_args = child_args.clone().unwrap_or_default();
            Some(Arguments {
                game: parent_args
                    .game
                    .into_iter()
                    .chain(child_args.game)
                    .collect(),
                jvm: parent_args.jvm.into_iter().chain(child_args.jvm).collect(),
            })
        }
    };

    VersionDescriptor {
        id: child.id.clone(),
        inherits_from: None,
        main_class: child
            .main_class
            .clone()
            .or_else(|| parent.main_class.clone()),
        libraries,
        arguments,
        minecraft_arguments: child
            .minecraft_arguments
            .clone()
            .or_else(|| parent.minecraft_arguments.clone()),
        asset_index: child
            .asset_index
            .clone()
            .or_else(|| parent.asset_index.clone()),
        assets: child.assets.clone().or_else(|| parent.assets.clone()),
        downloads: child.downloads.clone().or_else(|| parent.downloads.clone()),
        java_version: child
            .java_version
            .clone()
            .or_else(|| parent.java_version.clone()),
        kind: child.kind.clone().or_else(|| parent.kind.clone()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPaths {
    /// Shared files: `versions`, `libraries`, `assets`.
    pub game_root: PathBuf,
    pub instance_dir: PathBuf,
    pub java: PathBuf,
}

pub struct LaunchInput<'a> {
    pub child: &'a VersionDescriptor,
    /// `None` when the child does not inherit from anything.
    pub parent: Option<&'a VersionDescriptor>,
    pub account: &'a OfflineAccount,
    pub settings: &'a Settings,
    pub paths: &'a LaunchPaths,
    pub host: &'a HostEnvironment,
}

pub fn build_launch_plan(input: &LaunchInput<'_>) -> Result<LaunchPlan, DomainError> {
    let merged = match input.parent {
        Some(parent) => merge_descriptors(input.child, parent),
        None => input.child.clone(),
    };
    let main_class = merged.main_class.clone().ok_or_else(|| {
        DomainError::InvalidVersionDescriptor(format!("{} has no main class", merged.id))
    })?;
    let client_version = input
        .child
        .inherits_from
        .clone()
        .or_else(|| input.parent.map(|p| p.id.clone()))
        .unwrap_or(merged.id.clone());

    let features = LaunchFeatures::default();
    let host = input.host;
    let paths = input.paths;
    let separator = host.os.classpath_separator();
    let libraries_dir = paths.game_root.join("libraries");
    let client_jar_name = format!("{client_version}.jar");
    let client_jar = paths
        .game_root
        .join("versions")
        .join(&client_version)
        .join(&client_jar_name);

    let classpath = build_classpath(&merged, &libraries_dir, &client_jar, host, &features)?;
    let natives_dir = paths
        .game_root
        .join("versions")
        .join(&merged.id)
        .join("natives");
    let assets_index = merged
        .asset_index
        .as_ref()
        .map(|i| i.id.clone())
        .or(merged.assets.clone())
        .unwrap_or_default();
    let settings = input.settings;

    let vars: HashMap<&str, String> = HashMap::from([
        (
            "auth_player_name",
            input.account.username().as_str().to_owned(),
        ),
        ("auth_uuid", input.account.uuid().simple().to_string()),
        ("auth_access_token", "0".to_owned()),
        ("auth_session", "0".to_owned()),
        ("auth_xuid", "0".to_owned()),
        ("clientid", "0".to_owned()),
        ("user_type", "legacy".to_owned()),
        ("user_properties", "{}".to_owned()),
        ("version_name", merged.id.clone()),
        (
            "version_type",
            merged.kind.clone().unwrap_or_else(|| "release".to_owned()),
        ),
        ("game_directory", path_string(&paths.instance_dir)),
        ("assets_root", path_string(&paths.game_root.join("assets"))),
        ("game_assets", path_string(&paths.game_root.join("assets"))),
        ("assets_index_name", assets_index),
        ("launcher_name", LAUNCHER_NAME.to_owned()),
        ("launcher_version", LAUNCHER_VERSION.to_owned()),
        ("natives_directory", path_string(&natives_dir)),
        ("classpath", classpath.join(separator)),
        ("library_directory", path_string(&libraries_dir)),
        ("classpath_separator", separator.to_owned()),
        ("resolution_width", settings.window_width.to_string()),
        ("resolution_height", settings.window_height.to_string()),
    ]);

    let (raw_jvm, raw_game) = match (&merged.arguments, &merged.minecraft_arguments) {
        (None, Some(legacy)) => (
            legacy_jvm_arguments(),
            legacy
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        ),
        (arguments, _) => {
            let arguments = arguments.clone().unwrap_or_default();
            (
                evaluate_arguments(&arguments.jvm, host, &features),
                evaluate_arguments(&arguments.game, host, &features),
            )
        }
    };

    let mut jvm_args = vec![
        format!("-Xms{}M", settings.min_memory_mb),
        format!("-Xmx{}M", settings.max_memory_mb),
    ];
    jvm_args.extend(
        raw_jvm
            .iter()
            .map(|arg| substitute(arg, &vars))
            .map(|arg| ignore_client_jar(arg, &client_jar_name)),
    );
    jvm_args.extend(
        settings
            .extra_jvm_args
            .split_whitespace()
            .map(str::to_owned),
    );

    let mut game_args: Vec<String> = raw_game.iter().map(|arg| substitute(arg, &vars)).collect();
    if merged.arguments.is_none() && features.get("has_custom_resolution") {
        game_args.extend([
            "--width".to_owned(),
            vars["resolution_width"].clone(),
            "--height".to_owned(),
            vars["resolution_height"].clone(),
        ]);
    }
    if settings.fullscreen {
        game_args.push("--fullscreen".to_owned());
    }

    Ok(LaunchPlan {
        java: paths.java.clone(),
        jvm_args,
        main_class,
        game_args,
        working_dir: paths.instance_dir.clone(),
    })
}

fn legacy_jvm_arguments() -> Vec<String> {
    [
        "-Djava.library.path=${natives_directory}",
        "-cp",
        "${classpath}",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// Forge's bootstrap skips classpath jars whose name starts with an `ignoreList` entry and expects the
/// client jar to be named after the Forge version. Here the client jar keeps its vanilla name, so it must
/// be added explicitly or it would be loaded as a second `minecraft` module.
fn ignore_client_jar(arg: String, client_jar_name: &str) -> String {
    match arg.strip_prefix("-DignoreList=") {
        Some(list)
            if !list
                .split(',')
                .any(|entry| client_jar_name.starts_with(entry)) =>
        {
            format!("{arg},{client_jar_name}")
        }
        _ => arg,
    }
}

fn evaluate_arguments(
    arguments: &[Argument],
    host: &HostEnvironment,
    features: &LaunchFeatures,
) -> Vec<String> {
    arguments
        .iter()
        .flat_map(|argument| match argument {
            Argument::Plain(value) => vec![value.clone()],
            Argument::Conditional { rules, value } if rules_allow(rules, host, features) => {
                match value {
                    ArgumentValue::One(value) => vec![value.clone()],
                    ArgumentValue::Many(values) => values.clone(),
                }
            }
            Argument::Conditional { .. } => Vec::new(),
        })
        .collect()
}

fn build_classpath(
    merged: &VersionDescriptor,
    libraries_dir: &Path,
    client_jar: &Path,
    host: &HostEnvironment,
    features: &LaunchFeatures,
) -> Result<Vec<String>, DomainError> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for library in &merged.libraries {
        if !library
            .rules
            .as_deref()
            .is_none_or(|rules| rules_allow(rules, host, features))
        {
            continue;
        }
        // Legacy natives-only entries are extracted, never put on the classpath.
        if library.natives.is_some()
            && library
                .downloads
                .as_ref()
                .is_none_or(|d| d.artifact.is_none())
        {
            continue;
        }
        let relative = match library
            .downloads
            .as_ref()
            .and_then(|d| d.artifact.as_ref())
            .and_then(|a| a.path.clone())
        {
            Some(path) => path,
            None => MavenCoordinates::parse(&library.name)?.repository_path(),
        };
        let entry = path_string(&join_relative(libraries_dir, &relative));
        if seen.insert(entry.clone()) {
            entries.push(entry);
        }
    }
    let client = path_string(client_jar);
    if seen.insert(client.clone()) {
        entries.push(client);
    }
    Ok(entries)
}

fn join_relative(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Replaces every `${name}` with its value; unknown placeholders are kept verbatim like the vanilla launcher.
fn substitute(template: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                match vars.get(name) {
                    Some(value) => out.push_str(value),
                    None => out.push_str(&rest[start..start + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Username;

    const VANILLA_1_20_1: &str = r#"{
      "id": "1.20.1",
      "type": "release",
      "mainClass": "net.minecraft.client.main.Main",
      "assets": "5",
      "assetIndex": {"id": "5", "sha1": "a6e5a4e1d6b5b1e4bd3a2c7e10dcef4b4b6b8d68", "size": 410889, "totalSize": 622360473,
        "url": "https://piston-meta.mojang.com/v1/packages/a6e5a4e1d6b5b1e4bd3a2c7e10dcef4b4b6b8d68/5.json"},
      "downloads": {"client": {"sha1": "0c3ec587af28e5a785c0b4a7b8a30f9a8f78f838", "size": 23028853,
        "url": "https://piston-data.mojang.com/v1/objects/0c3ec587af28e5a785c0b4a7b8a30f9a8f78f838/client.jar"}},
      "javaVersion": {"component": "java-runtime-gamma", "majorVersion": 17},
      "libraries": [
        {"name": "com.mojang:logging:1.1.1", "downloads": {"artifact": {"path": "com/mojang/logging/1.1.1/logging-1.1.1.jar",
          "sha1": "832b8e6674a9b325a5175a3a6267dfaf34c85139", "size": 15343,
          "url": "https://libraries.minecraft.net/com/mojang/logging/1.1.1/logging-1.1.1.jar"}}},
        {"name": "org.ow2.asm:asm:9.3", "downloads": {"artifact": {"path": "org/ow2/asm/asm/9.3/asm-9.3.jar",
          "sha1": "8e6300ef51c1d801a7ed62d07cd221aca3a90640", "size": 122004,
          "url": "https://libraries.minecraft.net/org/ow2/asm/asm/9.3/asm-9.3.jar"}}},
        {"name": "org.lwjgl:lwjgl:3.3.1", "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar",
          "sha1": "ae58664f88e18a9bb2c77b063833ca7aaec484cb", "size": 724243,
          "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar"}}},
        {"name": "org.lwjgl:lwjgl:3.3.1:natives-linux", "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar",
          "sha1": "1de885aba434f934201b99f2f1afb142036ac189", "size": 110704,
          "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar"}},
          "rules": [{"action": "allow", "os": {"name": "linux"}}]},
        {"name": "org.lwjgl:lwjgl:3.3.1:natives-windows", "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar",
          "sha1": "0036c37f16ab611b3aa11f3bcf80b1d509b4ce6b", "size": 159361,
          "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"}},
          "rules": [{"action": "allow", "os": {"name": "windows"}}]},
        {"name": "org.lwjgl:lwjgl:3.3.1:natives-windows-x86", "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows-x86.jar",
          "sha1": "3b14f4beae9dd39791ec9e12190a9380cd8a3ce6", "size": 134695,
          "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows-x86.jar"}},
          "rules": [{"action": "allow", "os": {"name": "windows"}}]},
        {"name": "org.lwjgl:lwjgl:3.3.1:natives-macos", "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos.jar",
          "sha1": "2d76a1ffd6a5d8e9b7a6b4b1b1b4e4d1c2f4e5a6", "size": 41487,
          "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos.jar"}},
          "rules": [{"action": "allow", "os": {"name": "osx"}}]}
      ],
      "arguments": {
        "game": [
          "--username", "${auth_player_name}", "--version", "${version_name}", "--gameDir", "${game_directory}",
          "--assetsDir", "${assets_root}", "--assetIndex", "${assets_index_name}", "--uuid", "${auth_uuid}",
          "--accessToken", "${auth_access_token}", "--clientId", "${clientid}", "--xuid", "${auth_xuid}",
          "--userType", "${user_type}", "--versionType", "${version_type}",
          {"rules": [{"action": "allow", "features": {"is_demo_user": true}}], "value": "--demo"},
          {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}],
           "value": ["--width", "${resolution_width}", "--height", "${resolution_height}"]},
          {"rules": [{"action": "allow", "features": {"has_quick_plays_support": true}}],
           "value": ["--quickPlayPath", "${quickPlayPath}"]}
        ],
        "jvm": [
          {"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
          {"rules": [{"action": "allow", "os": {"name": "windows"}}],
           "value": "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"},
          {"rules": [{"action": "allow", "os": {"name": "windows", "version": "^10\\."}}],
           "value": ["-Dos.name=Windows 10", "-Dos.version=10.0"]},
          {"rules": [{"action": "allow", "os": {"arch": "x86"}}], "value": "-Xss1M"},
          "-Djava.library.path=${natives_directory}",
          "-Djna.tmpdir=${natives_directory}",
          "-Dminecraft.launcher.brand=${launcher_name}",
          "-Dminecraft.launcher.version=${launcher_version}",
          "-cp",
          "${classpath}"
        ]
      }
    }"#;

    const FORGE_47_4_10: &str = r#"{
      "id": "1.20.1-forge-47.4.10",
      "inheritsFrom": "1.20.1",
      "type": "release",
      "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
      "arguments": {
        "game": ["--launchTarget", "forgeclient", "--fml.forgeVersion", "47.4.10", "--fml.mcVersion", "1.20.1",
                 "--fml.forgeGroup", "net.minecraftforge", "--fml.mcpVersion", "20230612.114412"],
        "jvm": [
          "-Djava.net.preferIPv6Addresses=system",
          "-DignoreList=bootstraplauncher,securejarhandler,asm-commons,asm-util,asm-analysis,asm-tree,asm,JarJarFileSystems,client-extra,fmlcore,javafmllanguage,lowcodelanguage,mclanguage,forge-,${version_name}.jar",
          "-DmergeModules=jna-5.10.0.jar,jna-platform-5.10.0.jar",
          "-DlibraryDirectory=${library_directory}",
          "-p", "${library_directory}/cpw/mods/bootstraplauncher/1.1.2/bootstraplauncher-1.1.2.jar${classpath_separator}${library_directory}/cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar",
          "--add-modules", "ALL-MODULE-PATH",
          "--add-opens", "java.base/java.util.jar=cpw.mods.securejarhandler"
        ]
      },
      "libraries": [
        {"name": "cpw.mods:securejarhandler:2.1.10", "downloads": {"artifact": {"path": "cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar",
          "url": "https://maven.minecraftforge.net/cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar",
          "sha1": "51e6a22c6c716beb11e244bf5b8be480f51dd6b5", "size": 88749}}},
        {"name": "org.ow2.asm:asm:9.7.1", "downloads": {"artifact": {"path": "org/ow2/asm/asm/9.7.1/asm-9.7.1.jar",
          "url": "https://maven.minecraftforge.net/org/ow2/asm/asm/9.7.1/asm-9.7.1.jar",
          "sha1": "f0ed132a49244b042cd0e15702ab9f2ce3cc8436", "size": 126093}}},
        {"name": "net.minecraftforge:fmlloader:1.20.1-47.4.10", "downloads": {"artifact": {
          "path": "net/minecraftforge/fmlloader/1.20.1-47.4.10/fmlloader-1.20.1-47.4.10.jar",
          "url": "https://maven.minecraftforge.net/net/minecraftforge/fmlloader/1.20.1-47.4.10/fmlloader-1.20.1-47.4.10.jar",
          "sha1": "c2d5b6cfd8a6f2b8e0b3c0f9c9d5c7e6b0a1b2c3", "size": 210000}}},
        {"name": "net.minecraftforge:forge:1.20.1-47.4.10:client", "downloads": {"artifact": {
          "path": "net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-client.jar", "url": "",
          "sha1": "d3b4e2f1a0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5", "size": 0}}}
      ]
    }"#;

    fn parse(json: &str) -> VersionDescriptor {
        serde_json::from_str(json).expect("fixture parses")
    }

    fn account() -> OfflineAccount {
        OfflineAccount::new(Username::parse("Notch").unwrap())
    }

    fn paths() -> LaunchPaths {
        LaunchPaths {
            game_root: PathBuf::from("/data/game"),
            instance_dir: PathBuf::from("/data/instance"),
            java: PathBuf::from("/data/runtimes/java"),
        }
    }

    fn linux() -> HostEnvironment {
        HostEnvironment::new(HostOs::Linux, "x86_64")
    }

    fn plan_for(host: &HostEnvironment, settings: &Settings) -> LaunchPlan {
        let child = parse(FORGE_47_4_10);
        let parent = parse(VANILLA_1_20_1);
        let account = account();
        let paths = paths();
        build_launch_plan(&LaunchInput {
            child: &child,
            parent: Some(&parent),
            account: &account,
            settings,
            paths: &paths,
            host,
        })
        .unwrap()
    }

    fn classpath(plan: &LaunchPlan) -> Vec<String> {
        let index = plan.jvm_args.iter().position(|a| a == "-cp").unwrap();
        plan.jvm_args[index + 1]
            .split(':')
            .map(str::to_owned)
            .collect()
    }

    fn value_after<'a>(args: &'a [String], flag: &str) -> &'a str {
        let index = args
            .iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("{flag} missing"));
        &args[index + 1]
    }

    #[test]
    fn merge_child_wins_on_duplicates_and_inherits_missing_fields() {
        let merged = merge_descriptors(&parse(FORGE_47_4_10), &parse(VANILLA_1_20_1));

        assert_eq!(merged.id, "1.20.1-forge-47.4.10");
        assert_eq!(
            merged.main_class.as_deref(),
            Some("cpw.mods.bootstraplauncher.BootstrapLauncher")
        );
        assert_eq!(merged.asset_index.as_ref().unwrap().id, "5");
        assert_eq!(
            merged.java_version.as_ref().unwrap().component,
            "java-runtime-gamma"
        );
        assert!(merged.downloads.as_ref().unwrap().client.is_some());
        assert!(merged.inherits_from.is_none());

        let asm: Vec<&str> = merged
            .libraries
            .iter()
            .filter(|l| l.name.starts_with("org.ow2.asm:asm:"))
            .map(|l| l.name.as_str())
            .collect();
        assert_eq!(asm, ["org.ow2.asm:asm:9.7.1"]);
        let lwjgl_variants = merged
            .libraries
            .iter()
            .filter(|l| l.name.starts_with("org.lwjgl:lwjgl:"))
            .count();
        assert_eq!(lwjgl_variants, 5, "classifiers are distinct identities");

        let arguments = merged.arguments.unwrap();
        assert_eq!(arguments.game[0], Argument::Plain("--username".into()));
        assert_eq!(
            arguments.game.last(),
            Some(&Argument::Plain("20230612.114412".into()))
        );
        assert_eq!(
            arguments.jvm.last(),
            Some(&Argument::Plain(
                "java.base/java.util.jar=cpw.mods.securejarhandler".into()
            ))
        );
    }

    #[test]
    fn rules_follow_os_arch_version_and_features() {
        let features = LaunchFeatures::default();
        let windows_rule: Vec<Rule> = serde_json::from_str(
            r#"[{"action":"allow","os":{"name":"windows","version":"^10\\."}}]"#,
        )
        .unwrap();
        let mut win10 = HostEnvironment::new(HostOs::Windows, "x86_64");
        win10.os_version = Some("10.0".into());
        assert!(rules_allow(&windows_rule, &win10, &features));
        win10.os_version = Some("6.1".into());
        assert!(!rules_allow(&windows_rule, &win10, &features));
        assert!(!rules_allow(&windows_rule, &linux(), &features));

        let not_osx: Vec<Rule> = serde_json::from_str(
            r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#,
        )
        .unwrap();
        assert!(rules_allow(&not_osx, &linux(), &features));
        assert!(!rules_allow(
            &not_osx,
            &HostEnvironment::new(HostOs::MacOs, "aarch64"),
            &features
        ));

        let x86: Vec<Rule> =
            serde_json::from_str(r#"[{"action":"allow","os":{"arch":"x86"}}]"#).unwrap();
        assert!(!rules_allow(&x86, &linux(), &features));
        assert!(rules_allow(
            &x86,
            &HostEnvironment::new(HostOs::Windows, "x86"),
            &features
        ));

        let demo: Vec<Rule> =
            serde_json::from_str(r#"[{"action":"allow","features":{"is_demo_user":true}}]"#)
                .unwrap();
        assert!(!rules_allow(&demo, &linux(), &features));
        assert!(rules_allow(&[], &linux(), &features));
    }

    #[test]
    fn plan_filters_natives_and_os_arguments_per_host() {
        let settings = Settings::default();
        let linux_plan = plan_for(&linux(), &settings);
        let cp = classpath(&linux_plan);
        assert!(
            cp.iter()
                .any(|e| e.ends_with("lwjgl-3.3.1-natives-linux.jar"))
        );
        assert!(
            !cp.iter()
                .any(|e| e.contains("natives-windows") || e.contains("natives-macos"))
        );
        assert!(
            !linux_plan
                .jvm_args
                .iter()
                .any(|a| a == "-XstartOnFirstThread" || a == "-Xss1M")
        );

        let mac_plan = plan_for(&HostEnvironment::new(HostOs::MacOs, "aarch64"), &settings);
        assert!(
            mac_plan
                .jvm_args
                .iter()
                .any(|a| a == "-XstartOnFirstThread")
        );

        let windows_plan = plan_for(&HostEnvironment::new(HostOs::Windows, "x86_64"), &settings);
        let windows_cp = value_after(&windows_plan.jvm_args, "-cp");
        assert!(windows_cp.contains(';'));
        assert!(windows_cp.contains("natives-windows.jar"));
        assert!(
            windows_plan
                .jvm_args
                .iter()
                .any(|a| a.starts_with("-XX:HeapDumpPath="))
        );
        assert!(
            !windows_plan
                .jvm_args
                .iter()
                .any(|a| a == "-Dos.name=Windows 10"),
            "unknown os version never matches"
        );
    }

    #[test]
    fn classpath_uses_artifact_paths_maven_coordinates_and_client_jar_without_duplicates() {
        let mut child = parse(FORGE_47_4_10);
        child.libraries.push(Library {
            name: "net.minecraftforge:JarJarFileSystems:0.3.19".into(),
            ..Library::default()
        });
        child.libraries.push(Library {
            name: "org.example:thing:1.0:extra@zip".into(),
            ..Library::default()
        });
        child.libraries.push(child.libraries[0].clone());
        let parent = parse(VANILLA_1_20_1);
        let account = account();
        let paths = paths();
        let settings = Settings::default();
        let plan = build_launch_plan(&LaunchInput {
            child: &child,
            parent: Some(&parent),
            account: &account,
            settings: &settings,
            paths: &paths,
            host: &linux(),
        })
        .unwrap();
        let cp = classpath(&plan);

        assert!(cp.contains(
            &"/data/game/libraries/net/minecraftforge/JarJarFileSystems/0.3.19/JarJarFileSystems-0.3.19.jar".to_owned()
        ));
        assert!(cp.contains(
            &"/data/game/libraries/org/example/thing/1.0/thing-1.0-extra.zip".to_owned()
        ));
        assert!(
            cp.contains(&"/data/game/libraries/org/ow2/asm/asm/9.7.1/asm-9.7.1.jar".to_owned())
        );
        assert!(!cp.iter().any(|e| e.ends_with("asm-9.3.jar")));
        assert_eq!(cp.last().unwrap(), "/data/game/versions/1.20.1/1.20.1.jar");
        let unique: HashSet<&String> = cp.iter().collect();
        assert_eq!(unique.len(), cp.len());
    }

    #[test]
    fn placeholders_memory_and_window_settings_are_applied() {
        let settings = Settings {
            min_memory_mb: 2048,
            max_memory_mb: 8192,
            extra_jvm_args: "  -XX:+UseG1GC   -Dfoo=bar ".into(),
            window_width: 1600,
            window_height: 900,
            fullscreen: true,
            ..Settings::default()
        };
        let plan = plan_for(&linux(), &settings);
        let game = &plan.game_args;

        assert_eq!(plan.java, PathBuf::from("/data/runtimes/java"));
        assert_eq!(plan.working_dir, PathBuf::from("/data/instance"));
        assert_eq!(
            plan.main_class,
            "cpw.mods.bootstraplauncher.BootstrapLauncher"
        );
        assert_eq!(&plan.jvm_args[..2], ["-Xms2048M", "-Xmx8192M"]);
        assert_eq!(
            &plan.jvm_args[plan.jvm_args.len() - 2..],
            ["-XX:+UseG1GC", "-Dfoo=bar"]
        );

        assert_eq!(value_after(game, "--username"), "Notch");
        assert_eq!(
            value_after(game, "--uuid"),
            "b50ad385829d3141a2167e7d7539ba7f"
        );
        assert_eq!(value_after(game, "--accessToken"), "0");
        assert_eq!(value_after(game, "--userType"), "legacy");
        assert_eq!(value_after(game, "--version"), "1.20.1-forge-47.4.10");
        assert_eq!(value_after(game, "--versionType"), "release");
        assert_eq!(value_after(game, "--gameDir"), "/data/instance");
        assert_eq!(value_after(game, "--assetsDir"), "/data/game/assets");
        assert_eq!(value_after(game, "--assetIndex"), "5");
        assert_eq!(value_after(game, "--width"), "1600");
        assert_eq!(value_after(game, "--height"), "900");
        assert_eq!(value_after(game, "--launchTarget"), "forgeclient");
        assert!(game.contains(&"--fullscreen".to_owned()));
        assert!(!game.iter().any(|a| a == "--demo" || a == "--quickPlayPath"));

        let jvm = &plan.jvm_args;
        assert!(jvm.contains(
            &"-Djava.library.path=/data/game/versions/1.20.1-forge-47.4.10/natives".to_owned()
        ));
        assert!(jvm.contains(&format!("-Dminecraft.launcher.brand={LAUNCHER_NAME}")));
        assert!(jvm.contains(&"-DlibraryDirectory=/data/game/libraries".to_owned()));
        assert_eq!(
            value_after(jvm, "-p"),
            "/data/game/libraries/cpw/mods/bootstraplauncher/1.1.2/bootstraplauncher-1.1.2.jar:/data/game/libraries/cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar"
        );
        let ignore = jvm.iter().find(|a| a.starts_with("-DignoreList=")).unwrap();
        assert!(ignore.ends_with(",1.20.1-forge-47.4.10.jar,1.20.1.jar"));
        assert!(!jvm.iter().chain(game).any(|a| a.contains("${")));
    }

    #[test]
    fn legacy_minecraft_arguments_are_used_when_arguments_are_missing() {
        let child: VersionDescriptor = serde_json::from_str(
            r#"{"id": "1.12.2-forge-14.23.5.2860", "inheritsFrom": "1.12.2",
                "mainClass": "net.minecraft.launchwrapper.Launch",
                "minecraftArguments": "--username ${auth_player_name} --version ${version_name} --gameDir ${game_directory} --uuid ${auth_uuid} --accessToken ${auth_access_token} --userType ${user_type} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
                "libraries": [{"name": "net.minecraft:launchwrapper:1.12"}]}"#,
        )
        .unwrap();
        let parent: VersionDescriptor = serde_json::from_str(
            r#"{"id": "1.12.2", "mainClass": "net.minecraft.client.main.Main", "assets": "1.12",
                "minecraftArguments": "--username ${auth_player_name}", "libraries": []}"#,
        )
        .unwrap();
        let account = account();
        let paths = paths();
        let settings = Settings::default();
        let plan = build_launch_plan(&LaunchInput {
            child: &child,
            parent: Some(&parent),
            account: &account,
            settings: &settings,
            paths: &paths,
            host: &linux(),
        })
        .unwrap();

        assert_eq!(plan.main_class, "net.minecraft.launchwrapper.Launch");
        assert_eq!(
            value_after(&plan.game_args, "--tweakClass"),
            "net.minecraftforge.fml.common.launcher.FMLTweaker"
        );
        assert_eq!(value_after(&plan.game_args, "--username"), "Notch");
        assert_eq!(value_after(&plan.game_args, "--width"), "1280");
        assert_eq!(
            value_after(&plan.jvm_args, "-cp"),
            "/data/game/libraries/net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar:/data/game/versions/1.12.2/1.12.2.jar"
        );
        assert!(
            plan.jvm_args
                .iter()
                .any(|a| a.starts_with("-Djava.library.path="))
        );
    }

    #[test]
    fn missing_main_class_is_an_error() {
        let child = VersionDescriptor {
            id: "broken".into(),
            ..VersionDescriptor::default()
        };
        let account = account();
        let paths = paths();
        let settings = Settings::default();
        let result = build_launch_plan(&LaunchInput {
            child: &child,
            parent: None,
            account: &account,
            settings: &settings,
            paths: &paths,
            host: &linux(),
        });
        assert!(matches!(
            result,
            Err(DomainError::InvalidVersionDescriptor(_))
        ));
    }

    #[test]
    fn simple_regex_and_substitution_edge_cases() {
        assert!(simple_regex_find("^10\\.", "10.0"));
        assert!(!simple_regex_find("^10\\.", "100"));
        assert!(simple_regex_find("0$", "10.0"));
        assert!(!simple_regex_find("^1[0-9]", "10"));
        let vars = HashMap::from([("a", "x".to_owned())]);
        assert_eq!(substitute("${a}-${b}-${a", &vars), "x-${b}-${a");
    }
}
