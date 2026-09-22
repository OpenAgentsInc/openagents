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
    /// Who the bot joins as. Single-agent worlds use this; multi-agent
    /// worlds list `agents` instead.
    pub agent: Agent,
    /// Every embodied agent the episode runs, with its guild and pubkey.
    /// Empty means the single `agent` plays alone.
    pub agents: Vec<Member>,
    /// Registered minable deposits. A block only earns when a bound agent
    /// digs a listed position — placed ore, gifts, and replays cannot.
    pub deposits: Vec<Deposit>,
    /// The finite compute budget the episode works under.
    pub economy: Economy,
    /// Named console commands a verified quest may run — the allowlisted
    /// world effects. A quest names an effect; it never writes commands.
    pub effects: BTreeMap<String, String>,
    /// Guild channels and the decision door. `None` means no relay runs.
    pub relay: Option<RelaySection>,
    /// The coding quest the arena posts. `None` means no quest runs.
    pub quest: Option<QuestSection>,
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

/// An enrolled agent: an offline-mode player bound to a guild and a Nostr
/// pubkey. The binding is the manifest's claim; the episode refuses a bot
/// whose username is not enrolled.
#[derive(Clone, Debug, Deserialize)]
pub struct Member {
    /// The offline-mode username the bot joins as.
    pub username: String,
    /// The guild it mines and spends for.
    pub guild: String,
    /// The Nostr pubkey (hex) the agent's messages sign under. Empty in
    /// phase 1 — the field exists so the binding lands before the wire.
    #[serde(default)]
    pub pubkey: String,
    /// Where the agent stands when idle: `[x, y, z]` of its camp.
    #[serde(default)]
    pub camp: Option<[f64; 3]>,
}

/// A registered deposit: a named set of block positions worth credits
/// when a bound agent digs them. `guild` scopes a deposit to one guild;
/// absent means contested — any guild may earn it.
#[derive(Clone, Debug, Deserialize)]
pub struct Deposit {
    /// Its name in ledger records, such as `ferro_iron` or `contested_diamond`.
    pub id: String,
    /// The guild that may earn it, or `None` for contested.
    #[serde(default)]
    pub guild: Option<String>,
    /// The block positions that make it up: `[x, y, z]` integer triples.
    pub blocks: Vec<[i32; 3]>,
    /// Credits one dug block of this deposit awards.
    #[serde(default = "default_award")]
    pub award: u64,
    /// The block kind the position must still hold, such as
    /// `minecraft:iron_ore`. A dug block of the wrong kind does not award —
    /// placed blocks and substitutions are not mining.
    #[serde(default)]
    pub kind: String,
}

/// The `economy` section: how mining converts to compute.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Economy {
    /// Credits each guild holds before the first block is dug.
    #[serde(default)]
    pub starting_credits: u64,
    /// The most credits one quest may reserve at once.
    #[serde(default)]
    pub max_reservation: u64,
}

/// The `relay` section: guild channels and a decision door. Present
/// means the episode spawns a local `nostr-relay` and the members speak
/// in NIP-29 groups.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RelaySection {
    /// The port the episode's relay listens on.
    #[serde(default = "default_relay_port")]
    pub port: u16,
    /// The `POST /v1/systemone` endpoint for decisions — a local
    /// `kev-serve`, or a live door. Absent means no model calls.
    #[serde(default)]
    pub decision_url: Option<String>,
    /// The model decision requests name, such as `kev-latest`.
    #[serde(default)]
    pub decision_model: Option<String>,
}

/// The `quest` section: the coding quest a guild may take. The fixture
/// it names is the solver's world — a bounded Rust project the quest
/// copies into the run directory before a patch may touch it.
#[derive(Clone, Debug, Deserialize)]
pub struct QuestSection {
    /// The quest id the ledger, the label, and the trace name.
    pub id: String,
    /// The fixture directory, relative to the repository root — it
    /// holds `fixture/` plus a `protected/` the solver never sees.
    pub fixture: String,
    /// Credits the reservation holds while the quest runs.
    pub cost: u64,
    /// The most patch attempts the hold pays for.
    #[serde(default = "default_attempts")]
    pub attempts: u32,
    /// The manifest effect a verified patch buys — a name in
    /// `effects`, never a command.
    pub effect: String,
    /// Block positions the effect must leave standing.
    pub verify_blocks: Vec<[i32; 3]>,
    /// The block kind those positions must hold, such as `oak_planks`.
    pub verify_kind: String,
    /// XP a completed quest records — separate from credit balances.
    pub xp: u64,
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
fn default_award() -> u64 {
    1
}
fn default_relay_port() -> u16 {
    7447
}
fn default_attempts() -> u32 {
    3
}

#[derive(Deserialize)]
struct Manifest {
    kind: String,
    name: String,
    minecraft: Minecraft,
    #[serde(default)]
    agent: Agent,
    #[serde(default)]
    agents: Vec<Member>,
    #[serde(default)]
    deposits: Vec<Deposit>,
    #[serde(default)]
    economy: Economy,
    #[serde(default)]
    effects: BTreeMap<String, String>,
    #[serde(default)]
    relay: Option<RelaySection>,
    #[serde(default)]
    quest: Option<QuestSection>,
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
        let world = World {
            path: path.to_path_buf(),
            digest,
            name: manifest.name,
            minecraft: manifest.minecraft,
            agent: manifest.agent,
            agents: manifest.agents,
            deposits: manifest.deposits,
            economy: manifest.economy,
            effects: manifest.effects,
            relay: manifest.relay,
            quest: manifest.quest,
            episode: manifest.episode,
        };
        world.check()?;
        Ok(world)
    }

    /// The enrollment and deposit rules a manifest must keep, checked
    /// once at load so a run never discovers them mid-episode.
    fn check(&self) -> Result<()> {
        let mut usernames = std::collections::HashSet::new();
        for member in &self.agents {
            if !usernames.insert(member.username.as_str()) {
                return Err(Error::world(format!(
                    "{}: duplicate agent username {:?}",
                    self.path.display(),
                    member.username
                )));
            }
        }
        let mut positions = std::collections::HashSet::new();
        for deposit in &self.deposits {
            if deposit.id.trim().is_empty() {
                return Err(Error::world(format!(
                    "{}: a deposit id is empty",
                    self.path.display()
                )));
            }
            for block in &deposit.blocks {
                if !positions.insert(block) {
                    return Err(Error::world(format!(
                        "{}: deposit {:?} shares a block position",
                        self.path.display(),
                        deposit.id
                    )));
                }
            }
        }
        if let Some(quest) = &self.quest {
            if !self.effects.contains_key(&quest.effect) {
                return Err(Error::world(format!(
                    "{}: quest {:?} names effect {:?}, which is not in effects",
                    self.path.display(),
                    quest.id,
                    quest.effect
                )));
            }
            if quest.attempts == 0 || quest.cost == 0 {
                return Err(Error::world(format!(
                    "{}: quest {:?} needs nonzero cost and attempts",
                    self.path.display(),
                    quest.id
                )));
            }
        }
        Ok(())
    }

    /// The registered deposit covering `pos`, if any. One position may
    /// sit in at most one deposit — the loader enforces it.
    #[must_use]
    pub fn deposit_at(&self, pos: [i32; 3]) -> Option<&Deposit> {
        self.deposits.iter().find(|d| d.blocks.contains(&pos))
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

    const ARENA: &str = r#"{
        "kind": "voyager.world/v1",
        "name": "arena",
        "minecraft": {"version": "1.21.11"},
        "agents": [
            {"username": "ferro_1", "guild": "ferro", "pubkey": "aa"},
            {"username": "lumen_1", "guild": "lumen", "pubkey": "bb", "camp": [20.5, 0.0, 0.5]}
        ],
        "deposits": [
            {"id": "ferro_iron", "guild": "ferro", "kind": "minecraft:iron_ore",
             "award": 2, "blocks": [[-24, 0, -1], [-24, 0, 0]]},
            {"id": "contested", "kind": "minecraft:diamond_ore",
             "award": 5, "blocks": [[0, 0, -6]]}
        ],
        "economy": {"starting_credits": 0, "max_reservation": 12},
        "effects": {"open_bridge": "fill -1 0 -11 1 0 -10 minecraft:oak_planks"}
    }"#;

    #[test]
    fn agents_deposits_and_effects_parse() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arena.json");
        std::fs::write(&path, ARENA).unwrap();
        let world = World::load(&path).unwrap();
        assert_eq!(world.agents.len(), 2);
        assert_eq!(world.agents[1].camp, Some([20.5, 0.0, 0.5]));
        assert_eq!(world.deposits.len(), 2);
        let contested = world.deposit_at([0, 0, -6]).unwrap();
        assert_eq!(contested.id, "contested");
        assert!(contested.guild.is_none());
        assert!(world.deposit_at([9, 9, 9]).is_none());
        assert_eq!(
            world.effects.get("open_bridge").map(String::as_str),
            Some("fill -1 0 -11 1 0 -10 minecraft:oak_planks")
        );
    }

    #[test]
    fn duplicate_agent_usernames_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arena.json");
        std::fs::write(&path, ARENA.replace("\"lumen_1\"", "\"ferro_1\"")).unwrap();
        assert!(matches!(World::load(&path), Err(Error::World(_))));
    }

    #[test]
    fn deposits_sharing_a_position_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arena.json");
        std::fs::write(&path, ARENA.replace("[[0, 0, -6]]", "[[-24, 0, -1]]")).unwrap();
        assert!(matches!(World::load(&path), Err(Error::World(_))));
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
