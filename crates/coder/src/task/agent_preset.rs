//! Crew presets: the definition, charter, and look a new crew member
//! starts with (`docs/verse/crew.md`).
//!
//! Every crew member runs on the same machinery: the record, key,
//! journal, engrams, steering loop, policy, and Coder session are the
//! workshop agent's, parameterized by name. A preset only fills the
//! private record a new agent gets, so `studio.agent.new bob` and
//! `openagents agent new bob` make Bob with Bob's charter, and
//! `openagents agent new NAME --preset bob` makes another agent from
//! Bob's. A name with no preset gets the host's defaults: [`CHARTER`], a
//! look named after it, and sentences that refer to it by name.
//!
//! The presets follow Buzz's persona idea (NIP-AP), reimplemented here as
//! host-local records; nothing in a preset is published.

use serde::{Deserialize, Serialize};

use super::agent::{DEFAULT_CHARTER, DEFAULT_LOOK, Definition};

/// How the host's sentences refer to an agent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pronouns {
    /// By name: "Carol has no key", "Carol's key". The default, which
    /// keeps every sentence third person singular.
    #[default]
    Name,
    /// "she has no key", "her key".
    She,
    /// "he has no key", "his key".
    He,
}

impl Pronouns {
    #[must_use]
    pub fn is_name(&self) -> bool {
        *self == Self::Name
    }
}

/// The words a sentence about one agent uses: [`Pronouns`] resolved
/// against its display name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refer {
    subject: String,
    object: String,
    possessive: String,
    absolute: String,
}

impl Refer {
    /// The words for `pronouns`, with `display_name` for [`Pronouns::Name`].
    #[must_use]
    pub fn new(pronouns: Pronouns, display_name: &str) -> Self {
        let (subject, object, possessive, absolute) = match pronouns {
            Pronouns::She => ("she".into(), "her".into(), "her".into(), "hers".into()),
            Pronouns::He => ("he".into(), "him".into(), "his".into(), "his".into()),
            Pronouns::Name => (
                display_name.to_string(),
                display_name.to_string(),
                format!("{display_name}'s"),
                format!("{display_name}'s"),
            ),
        };
        Self {
            subject,
            object,
            possessive,
            absolute,
        }
    }

    /// The words for agent `name` before its record is read: its preset's
    /// pronouns, else by name.
    #[must_use]
    pub fn for_name(name: &str) -> Self {
        let pronouns = preset(name).map_or(Pronouns::Name, |p| p.pronouns);
        Self::new(pronouns, &Definition::default_for(name).display_name)
    }

    /// The subject, object, and possessive, for a `format!` that names
    /// them `they`, `them`, and `their`.
    #[must_use]
    pub fn words(&self) -> (&str, &str, &str) {
        (&self.subject, &self.object, &self.possessive)
    }

    /// The possessive that stands alone: `hers`, `his`, or `Name's`.
    #[must_use]
    pub fn theirs(&self) -> &str {
        &self.absolute
    }

    /// The subject: `she`, `he`, or the name.
    #[must_use]
    pub fn they(&self) -> &str {
        &self.subject
    }

    /// The subject at the start of a sentence: `She`, `He`, or the name.
    #[must_use]
    pub fn they_cap(&self) -> String {
        capitalize(&self.subject)
    }

    /// The object: `her`, `him`, or the name.
    #[must_use]
    pub fn them(&self) -> &str {
        &self.object
    }

    /// The possessive: `her`, `his`, or `Name's`.
    #[must_use]
    pub fn their(&self) -> &str {
        &self.possessive
    }

    /// The possessive at the start of a sentence: `Her`, `His`, or
    /// `Name's`.
    #[must_use]
    pub fn their_cap(&self) -> String {
        capitalize(&self.possessive)
    }
}

/// `text` with its first letter upper case.
#[must_use]
pub fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

/// One crew member's starting record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    /// The agent name it is for, such as `bob`.
    pub name: &'static str,
    /// The name people see.
    pub display_name: &'static str,
    pub pronouns: Pronouns,
    /// The character look a view draws it with.
    pub look: &'static str,
    /// What it may do, in words. It only narrows the host's permit.
    pub charter: &'static str,
    /// How it speaks; empty is plain and brief.
    pub voice: &'static str,
    /// Its job on the crew, in a few words.
    pub role: &'static str,
    pub job_role: Option<coder_host::access::crew::JobRole>,
}

/// The charter an agent without a preset starts with: the default
/// charter, about no one in particular.
pub const CHARTER: &str = "Terminal mode may run read-only commands anywhere in the \
     workspace without asking; any other command waits for the owner's CONFIRM or REJECT. Task \
     mode changes files only in the agent's own worktree, and the owner merges at the Merge \
     station. Never push, publish, pay, or read credentials.";

/// Bob's charter: the town builder's limits from `docs/verse/crew.md`,
/// over the default charter's.
pub const BOB_CHARTER: &str = "Terminal mode may run read-only commands anywhere in the \
     workspace without asking; any other command waits for the owner's CONFIRM or REJECT. Task \
     mode changes files only in his own worktree, and only the town's villager, routine, and \
     placement tables in the verse-zone-everglade crate and assets/verse/; the owner merges at \
     the Merge station. Never write the Everglade pack's pin, give a villager a job that claims \
     real work, or place licensed content. Never push, publish, pay, or read credentials.";

/// The crew members with a preset.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "alice",
        display_name: "Alice",
        pronouns: Pronouns::She,
        look: DEFAULT_LOOK,
        charter: DEFAULT_CHARTER,
        voice: "",
        role: "workshop agent",
        job_role: None,
    },
    Preset {
        name: "bob",
        display_name: "Bob",
        pronouns: Pronouns::He,
        look: "bob",
        charter: BOB_CHARTER,
        voice: "Practical and concrete; talks about the town in places, people, and hours.",
        role: "town builder",
        job_role: None,
    },
    Preset {
        name: "paul",
        display_name: "Paul",
        pronouns: Pronouns::Name,
        look: "paul",
        charter: super::agent_crew::SALES_CHARTER,
        voice: "Concrete, brief, and explicit about missing evidence.",
        role: "sales crew member",
        job_role: Some(coder_host::access::crew::JobRole::SalesLead),
    },
    Preset {
        name: "erin",
        display_name: "Erin",
        pronouns: Pronouns::Name,
        look: "erin",
        charter: super::agent_crew::SALES_CHARTER,
        voice: "Concrete, brief, and explicit about missing evidence.",
        role: "sales crew member",
        job_role: Some(coder_host::access::crew::JobRole::SalesResearcher),
    },
    Preset {
        name: "frank",
        display_name: "Frank",
        pronouns: Pronouns::Name,
        look: "frank",
        charter: super::agent_crew::SALES_CHARTER,
        voice: "Concrete, brief, and explicit about missing evidence.",
        role: "sales crew member",
        job_role: Some(coder_host::access::crew::JobRole::SalesProspector),
    },
    Preset {
        name: "pat",
        display_name: "Pat",
        pronouns: Pronouns::Name,
        look: "pat",
        charter: super::agent_crew::SALES_CHARTER,
        voice: "Concrete, brief, and explicit about missing evidence.",
        role: "sales crew member",
        job_role: Some(coder_host::access::crew::JobRole::SalesDemo),
    },
    Preset {
        name: "arthur",
        display_name: "Arthur",
        pronouns: Pronouns::Name,
        look: "arthur",
        charter: super::agent_crew::SALES_CHARTER,
        voice: "Concrete, brief, and explicit about missing evidence.",
        role: "sales crew member",
        job_role: Some(coder_host::access::crew::JobRole::SalesPartner),
    },
    Preset {
        name: "vanna",
        display_name: "Vanna",
        pronouns: Pronouns::Name,
        look: "vanna",
        charter: super::agent_crew::SALES_CHARTER,
        voice: "Concrete, brief, and explicit about missing evidence.",
        role: "sales crew member",
        job_role: Some(coder_host::access::crew::JobRole::SalesAffiliate),
    },
];

/// The preset named `name`, when there is one.
#[must_use]
pub fn preset(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

impl Preset {
    /// The definition an agent made from this preset starts with.
    #[must_use]
    pub fn definition(&self) -> Definition {
        Definition {
            display_name: self.display_name.into(),
            voice: self.voice.into(),
            system_prompt: String::new(),
            route: String::new(),
            respond_to: super::agent::RESPOND_TO_OWNER.into(),
            pronouns: Some(self.pronouns),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alice_is_she_bob_is_he_and_anyone_else_is_their_name() {
        let alice = Refer::for_name("alice");
        assert_eq!(
            (alice.they(), alice.them(), alice.their()),
            ("she", "her", "her")
        );
        let bob = Refer::for_name("bob");
        assert_eq!((bob.they(), bob.them(), bob.their()), ("he", "him", "his"));
        let carol = Refer::for_name("carol");
        assert_eq!(
            (carol.they(), carol.them(), carol.their()),
            ("Carol", "Carol", "Carol's")
        );
        assert_eq!(bob.their_cap(), "His");
    }

    #[test]
    fn alices_preset_is_the_hosts_defaults() {
        let alice = preset("alice").unwrap();
        assert_eq!(alice.charter, DEFAULT_CHARTER);
        assert_eq!(alice.look, DEFAULT_LOOK);
        assert!(alice.voice.is_empty());
        let bob = preset("bob").unwrap();
        assert!(
            !bob.charter
                .split_whitespace()
                .any(|w| w == "her" || w == "she")
        );
        assert!(!bob.charter.to_lowercase().contains("alice"));
    }
}
