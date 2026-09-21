//! A world is a manifest, not a directory.
//!
//! One file under `worlds/` names the Minecraft version, the seed, the
//! rules the world runs under, and how long an episode may take. The
//! manifest's SHA-256 is the world's identity: two runs against the same
//! file name the same world, and a run against an edited file names a new
//! one. That is the same digest convention the Gym applies to suites —
//! what a result was measured against is part of the result.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// The `kind` a world manifest must declare.
pub const WORLD_KIND: &str = "voyager.world/v1";

/// A loaded world manifest and the digest that names it.
#[derive(Clone, Debug)]
pub struct World {
    /// The file it came from.
    pub path: PathBuf,
    /// `sha256:` of the file's bytes.
    pub digest: String,
    /// The manifest's name for itself.
    pub name: String,
    /// How the server is configured.
    pub minecraft: Minecraft,
    /// Who the bot joins as.
    pub agent: Agent,
    /// How long one episode may run.
    pub episode: Bounds,
}

/// The `minecraft` section: what the server is told at boot.
#[derive(Clone, Debug, Deserialize)]
pub struct Minecraft {
    /// The Minecraft version, such as `1.21.11`. The server jar for it is
    /// fetched by `scripts/fetch-mc-server.sh`, not by this crate.
    pub version: String,
    /// `level-seed` in `server.properties`; empty lets the server pick.
    #[serde(default)]
    pub seed: String,
    /// `level-type`, such as `minecraft:normal` or `minecraft:flat`.
    #[serde(default = "default_level_type")]
    pub level_type: String,
    /// `difficulty`: `peaceful`, `easy`, `normal`, or `hard`.
    #[serde(default = "default_difficulty")]
    pub difficulty: String,
    /// `gamemode`: `survival`, `creative`, `adventure`, or `spectator`.
    #[serde(default = "default_gamemode")]
    pub gamemode: String,
    /// `generator-settings` for flat and custom world types, as a JSON
    /// string the server reads verbatim.
    #[serde(default)]
    pub generator_settings: Option<String>,
    /// Gamerules applied through the console once the server is ready:
    /// `{"doDaylightCycle": "false"}` runs `gamerule doDaylightCycle
    /// false`. World rules live in the manifest so a custom ruleset is a
    /// file, not a rebuild.
    #[serde(default)]
    pub gamerules: BTreeMap<String, String>,
    /// Console commands run once, in order, after the gamerules — for
    /// example `time set day` or `weather clear`.
    #[serde(default)]
    pub setup_commands: Vec<String>,
    /// `view-distance` in chunks.
    #[serde(default = "default_view_distance")]
    pub view_distance: u16,
}

/// The `agent` section: the account the bot joins with.
#[derive(Clone, Debug, Deserialize)]
pub struct Agent {
    /// The offline-mode username. Local servers only.
    #[serde(default = "default_username")]
    pub username: String,
}

/// The `episode` section: the bounds an episode may not cross.
#[derive(Clone, Debug, Deserialize)]
pub struct Bounds {
    /// The most bridge calls one episode may make.
    #[serde(default = "default_max_actions")]
    pub max_actions: u32,
    /// The most wall seconds one episode may take.
    #[serde(default = "default_max_seconds")]
    pub max_seconds: u64,
}

fn default_level_type() -> String {
    "minecraft:normal".to_string()
}
fn default_difficulty() -> String {
    "peaceful".to_string()
}
fn default_gamemode() -> String {
    "survival".to_string()
}
fn default_username() -> String {
    "voyager".to_string()
}
fn default_view_distance() -> u16 {
    10
}
fn default_max_actions() -> u32 {
    24
}
fn default_max_seconds() -> u64 {
    600
}

#[derive(Deserialize)]
struct Manifest {
    kind: String,
    name: String,
    minecraft: Minecraft,
    #[serde(default)]
    agent: Agent,
    #[serde(default)]
    episode: Bounds,
}

impl Default for Agent {
    fn default() -> Self {
        Agent {
            username: default_username(),
        }
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            max_actions: default_max_actions(),
            max_seconds: default_max_seconds(),
        }
    }
}

impl World {
    /// Reads and validates a world manifest.
    ///
    /// # Errors
    ///
    /// The file must exist, parse, and declare [`WORLD_KIND`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .map_err(|error| Error::world(format!("{}: {error}", path.display())))?;
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|error| Error::world(format!("{}: {error}", path.display())))?;
        if manifest.kind != WORLD_KIND {
            return Err(Error::world(format!(
                "{}: kind {:?} is not {WORLD_KIND:?}",
                path.display(),
                manifest.kind
            )));
        }
        if manifest.name.trim().is_empty() {
            return Err(Error::world(format!("{}: name is empty", path.display())));
        }
        let digest = format!("sha256:{:x}", Sha256::digest(&bytes));
        Ok(World {
            path: path.to_path_buf(),
            digest,
            name: manifest.name,
            minecraft: manifest.minecraft,
            agent: manifest.agent,
            episode: manifest.episode,
        })
    }

    /// The `server.properties` body the server reads at boot.
    ///
    /// Only properties the file format actually carries are written here;
    /// gamerules and one-shot setup are console commands the harness sends
    /// after the world is ready.
    #[must_use]
    pub fn properties(&self, port: u16) -> String {
        let mut body = String::new();
        let mc = &self.minecraft;
        body.push_str("# generated by voyager; edit the world manifest, not this file\n");
        body.push_str(&format!("server-port={port}\n"));
        body.push_str("online-mode=false\n");
        body.push_str("enforce-secure-profile=false\n");
        body.push_str("spawn-protection=0\n");
        body.push_str(&format!("view-distance={}\n", mc.view_distance));
        body.push_str(&format!("difficulty={}\n", mc.difficulty));
        body.push_str(&format!("gamemode={}\n", mc.gamemode));
        body.push_str("force-gamemode=true\n");
        body.push_str(&format!("level-type={}\n", mc.level_type));
        if !mc.seed.is_empty() {
            body.push_str(&format!("level-seed={}\n", mc.seed));
        }
        if let Some(settings) = &mc.generator_settings {
            body.push_str(&format!(
                "generator-settings={}\n",
                settings.replace('\n', "")
            ));
        }
        body.push_str("white-list=false\n");
        body.push_str("enable-command-block=false\n");
        body.push_str(&format!("motd=voyager:{}\n", self.name));
        body
    }

    /// The console commands to run once the server reports ready:
    /// gamerules first, then the manifest's setup commands.
    #[must_use]
    pub fn boot_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();
        for (rule, value) in &self.minecraft.gamerules {
            commands.push(format!("gamerule {rule} {value}"));
        }
        commands.extend(self.minecraft.setup_commands.iter().cloned());
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
        "kind": "voyager.world/v1",
        "name": "meadow",
        "minecraft": {
            "version": "1.21.11",
            "seed": "openagents",
            "difficulty": "peaceful",
            "gamerules": {"doDaylightCycle": "false"},
            "setup_commands": ["time set day"]
        },
        "agent": {"username": "voyager"},
        "episode": {"max_actions": 12, "max_seconds": 300}
    }"#;

    #[test]
    fn load_parses_and_digests() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meadow.json");
        std::fs::write(&path, MANIFEST).unwrap();
        let world = World::load(&path).unwrap();
        assert_eq!(world.name, "meadow");
        assert!(world.digest.starts_with("sha256:"));
        assert_eq!(world.minecraft.version, "1.21.11");
        assert_eq!(world.episode.max_actions, 12);
        assert_eq!(world.agent.username, "voyager");
    }

    #[test]
    fn properties_cover_the_server_contract() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meadow.json");
        std::fs::write(&path, MANIFEST).unwrap();
        let world = World::load(&path).unwrap();
        let body = world.properties(25565);
        assert!(body.contains("online-mode=false"));
        assert!(body.contains("level-seed=openagents"));
        assert!(body.contains("difficulty=peaceful"));
        assert!(body.contains("server-port=25565"));
    }

    #[test]
    fn boot_commands_order_rules_then_setup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meadow.json");
        std::fs::write(&path, MANIFEST).unwrap();
        let world = World::load(&path).unwrap();
        let commands = world.boot_commands();
        assert_eq!(commands[0], "gamerule doDaylightCycle false");
        assert_eq!(commands[1], "time set day");
    }

    #[test]
    fn wrong_kind_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("other.json");
        std::fs::write(
            &path,
            r#"{"kind": "other", "name": "x", "minecraft": {"version": "1"}}"#,
        )
        .unwrap();
        assert!(matches!(World::load(&path), Err(Error::World(_))));
    }
}
