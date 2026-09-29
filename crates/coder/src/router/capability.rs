//! The admitted-capability set: what the chat can do on this turn, as a
//! typed set the `capability` question is asked over (#9960).
//!
//! Chat is pure chat plus the capabilities present. The set for a turn is
//! three lists, deduplicated by id:
//!
//! - the chat's **built-in** capabilities ([`builtin`]): the knowledge
//!   base, Coder dispatch, `openagents` command offers, the wallet, the
//!   account, and the Gym, each with the route that serves it;
//! - the **catalog** ([`of_tool`]): the tool notes the Gym seam lists
//!   (`knowledge/openagents/openagents.tool-*.md`), usable in a Coder run;
//! - the **adoptions** ([`of_adoption`]): what the latest `coder-defaults`
//!   release admitted for everyone, read by [`crate::gym_kb`] from the
//!   release's signed manifest and its admissions.
//!
//! Each entry has an id (the `capability` question's option), a kind
//! ([`Kind`]: program, plugin, skill, or knowledge, the four contributor
//! kinds of [the vocabulary](https://github.com/OpenAgentsInc/openagents/issues/9957)),
//! one plain line, and a reach ([`Reach`]: usable from Cloud chat, or only
//! in a Coder run). The `capability` question offers every entry plus
//! `none` (a capability request none covers) and
//! `not-a-capability-request`, and [`super::policy::decide`] acts on the
//! typed reading: no code here reads message text.

use serde_json::{Value, json};

use super::RouteId;
use super::gym::{AdoptionRecord, Tool};

/// The `capability` question's option for a request that calls for a
/// capability none of the admitted ones covers.
pub const NONE: &str = "none";

/// The `capability` question's option for a message that asks for no
/// capability at all.
pub const NOT_A_REQUEST: &str = "not-a-capability-request";

/// A capability's contributor kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A program (NIP-PRG): composes work.
    Program,
    /// A plugin: a Wasm guest with one bounded operation.
    Plugin,
    /// A skill: guidance.
    Skill,
    /// A knowledge entry (NIP-KB).
    Knowledge,
}

impl Kind {
    /// The word the wire and the question carry.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Kind::Program => "program",
            Kind::Plugin => "plugin",
            Kind::Skill => "skill",
            Kind::Knowledge => "knowledge",
        }
    }
}

/// Where a capability can be used from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// From Cloud chat, on this turn.
    Chat,
    /// Only in a Coder run on a connected computer.
    Coder,
}

impl Reach {
    /// The word the wire and the question carry.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Reach::Chat => "chat",
            Reach::Coder => "coder",
        }
    }

    /// The NIP-CJ word, the same.
    #[must_use]
    pub fn cj(self) -> nostr::cj_conversation::Reach {
        match self {
            Reach::Chat => nostr::cj_conversation::Reach::Chat,
            Reach::Coder => nostr::cj_conversation::Reach::Coder,
        }
    }
}

/// One admitted capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capability {
    /// The `capability` question's option: `chat.coder`, a tool note's id
    /// (`openagents.tool-project-map`), or `adopted:<release id>`.
    pub id: String,
    pub kind: Kind,
    /// Its name on screen.
    pub name: String,
    /// What it does, in one plain line.
    pub line: String,
    pub reach: Reach,
    /// The route that serves it from chat, for a built-in.
    pub route: Option<RouteId>,
    /// Where it is described: a repository path or a release id.
    pub source: String,
}

impl Capability {
    /// What Jev reads for this entry: its name, kind, reach, and line, as
    /// labeled structure.
    #[must_use]
    pub fn criterion(&self) -> Value {
        json!({
            "what": format!("{}: {}", self.name, self.line),
            "kind": self.kind.word(),
            "usable_from": match self.reach {
                Reach::Chat => "this chat",
                Reach::Coder => "a Coder run on a connected computer",
            },
        })
    }
}

/// The id of the built-in Coder dispatch capability.
pub const CODER: &str = "chat.coder";

/// The chat's built-in capabilities, in a fixed order.
#[must_use]
pub fn builtin() -> Vec<Capability> {
    let entry = |id: &str, kind, name: &str, line: &str, route| Capability {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        line: line.to_string(),
        reach: Reach::Chat,
        route: Some(route),
        source: "crates/coder/src/router/capability.rs".to_string(),
    };
    vec![
        entry(
            "chat.knowledge",
            Kind::Knowledge,
            "Knowledge base",
            "Answers how the OpenAgents app, its features, and its code work, from our documentation.",
            RouteId::ProductKb,
        ),
        entry(
            CODER,
            Kind::Program,
            "Coder",
            "Works on the user's code, repository, files, or machine: changes, fixes, builds, tests, and looks through code, on a computer the user connected.",
            RouteId::WorkDispatch,
        ),
        entry(
            "chat.cli",
            Kind::Program,
            "Command offers",
            "Runs an openagents command for the user's own account or devices after they confirm it: their computers, tasks, sessions, XP, quests, the knowledge base, and what is published on a relay.",
            RouteId::Cli,
        ),
        entry(
            "chat.wallet",
            Kind::Program,
            "Wallet",
            "Explains the user's OpenAgents wallet, bitcoin amounts, receiving, sending, and backups, and opens the wallet for them.",
            RouteId::Wallet,
        ),
        entry(
            "chat.account",
            Kind::Program,
            "Account",
            "Explains and opens account settings: identity keys, connecting or removing computers, playtest sessions, and reporting a problem.",
            RouteId::Account,
        ),
        entry(
            "chat.gym",
            Kind::Program,
            "The Gym",
            "Tests a capability on Coder, makes a new one and its tests with the user, checks another trainer's result, and reports what their work earned.",
            RouteId::EvalRun,
        ),
    ]
}

/// A catalog tool as a capability, usable in a Coder run. Its kind is
/// what the note's tags say (`skill`, `program`), else a plugin.
#[must_use]
pub fn of_tool(tool: &Tool) -> Capability {
    let kind = if tool.slugs.iter().any(|slug| slug == "skill") {
        Kind::Skill
    } else if tool.slugs.iter().any(|slug| slug == "program") {
        Kind::Program
    } else {
        Kind::Plugin
    };
    Capability {
        id: tool.id.clone(),
        kind,
        name: tool.name.clone(),
        line: tool.line.clone(),
        reach: Reach::Coder,
        route: None,
        source: tool.source.clone(),
    }
}

/// An adoption as a capability: what Coder's defaults admitted for
/// everyone. One that is a catalog tool is that tool's entry; one that is
/// not is named by its release.
#[must_use]
pub fn of_adoption(adoption: &AdoptionRecord, tools: &[Tool]) -> Capability {
    if let Some(tool) = adoption
        .tool
        .as_deref()
        .and_then(|id| tools.iter().find(|tool| tool.id == id))
    {
        return of_tool(tool);
    }
    Capability {
        id: format!("adopted:{}", adoption.release.id),
        kind: Kind::Plugin,
        name: adoption.tool_name.clone(),
        line: "A capability Coder adopted for everyone, from a published extension release."
            .to_string(),
        reach: Reach::Coder,
        route: None,
        source: adoption.release.id.clone(),
    }
}

/// The admitted set for a turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Admitted {
    pub entries: Vec<Capability>,
}

impl Admitted {
    /// The built-ins, then the catalog, then the adoptions not already in
    /// it, each id once.
    #[must_use]
    pub fn of(tools: &[Tool], adoptions: &[AdoptionRecord]) -> Self {
        let mut entries = builtin();
        let mut push = |entry: Capability| {
            if !entries.iter().any(|known| known.id == entry.id) {
                entries.push(entry);
            }
        };
        for tool in tools {
            push(of_tool(tool));
        }
        for adoption in adoptions {
            push(of_adoption(adoption, tools));
        }
        Admitted { entries }
    }

    /// The built-ins alone: what a worker with no Gym seam admits.
    #[must_use]
    pub fn builtin() -> Self {
        Admitted::of(&[], &[])
    }

    /// The entry with `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::gym::EventPointer;
    use crate::router::gym::fixtures::tool;

    /// The set is the built-ins, the catalog, and the adoptions, each id
    /// once, with a kind and a reach; an adopted catalog tool is the
    /// tool's own entry.
    #[test]
    fn the_admitted_set_is_built_ins_catalog_and_adoptions_once_each() {
        let tools = [tool("project-map", "Project map")];
        let adopted = AdoptionRecord {
            release: EventPointer {
                id: "aa".repeat(32),
                pubkey: "bb".repeat(32),
                kind: 3184,
            },
            tool: Some("openagents.tool-project-map".into()),
            tool_name: "Project map".into(),
            at: 1,
        };
        let other = AdoptionRecord {
            release: EventPointer {
                id: "cc".repeat(32),
                pubkey: "bb".repeat(32),
                kind: 3184,
            },
            tool: None,
            tool_name: "changelog writer".into(),
            at: 2,
        };
        let admitted = Admitted::of(&tools, &[adopted, other]);
        let ids: Vec<&str> = admitted
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "chat.knowledge",
                CODER,
                "chat.cli",
                "chat.wallet",
                "chat.account",
                "chat.gym",
                "openagents.tool-project-map",
                &format!("adopted:{}", "cc".repeat(32)),
            ]
        );
        let map = admitted.get("openagents.tool-project-map").unwrap();
        assert_eq!((map.kind, map.reach), (Kind::Plugin, Reach::Coder));
        assert_eq!(
            map.criterion()["usable_from"],
            "a Coder run on a connected computer"
        );
        let coder = admitted.get(CODER).unwrap();
        assert_eq!(
            (coder.reach, coder.route),
            (Reach::Chat, Some(RouteId::WorkDispatch))
        );
        assert_eq!(Admitted::builtin().entries.len(), 6);
        assert!(Admitted::builtin().get(NONE).is_none());
        for entry in &admitted.entries {
            assert!(
                !entry.line.is_empty() && !entry.name.is_empty(),
                "{}",
                entry.id
            );
            assert!(!entry.id.contains(' '), "{}", entry.id);
        }
    }
}
