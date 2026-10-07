//! Talking to a villager (`docs/verse/generative-agents.md`, item 5,
//! phase E2): what it says, and what it remembers of the player.
//!
//! A villager says a fixed line keyed by the player's current quest step
//! first, as Bram does. Otherwise it answers with one small-model reply,
//! only for a player with a configured provider and under the roster's
//! `replies_per_player_per_day`; anyone else, or a player past the cap,
//! gets a fixed line: a rumor it knows and hasn't told this player, or one
//! of its own lines. The reply is written from the character card, the
//! villager's top memories of this player, and the rumors it knows
//! ([`prompt`]). The model sits behind [`Answerer`]; tests use a fake.
//!
//! Each villager keeps a bounded [`memory_stream::Stream`] of encounters
//! with each player, ranked by recency and importance, in the player's
//! [`Save`], with the cap's counter keyed by the town day. The save is
//! plain JSON with no I/O here: the desktop keeps it in a file beside the
//! player's other Verse files.

use std::collections::BTreeMap;

use memory_stream::{Memory, Stream};
use serde::{Deserialize, Serialize};
use town_clock::TownTime;

use crate::rumor::Rumor;
use crate::{Budgets, Npc, quest};

/// The save's schema.
pub const SAVE_SCHEMA: &str = "openagents.verse-townsfolk-save.v1";
/// The most encounters a villager keeps of one player.
pub const MEMORIES_PER_VILLAGER: usize = 24;
/// The most memories a reply's prompt carries.
pub const PROMPT_MEMORIES: usize = 5;
/// The most rumors a reply's prompt carries.
pub const PROMPT_RUMORS: usize = 4;
/// The longest note an encounter keeps, characters.
pub const MAX_NOTE: usize = 160;
/// The longest reply a bubble shows, characters.
pub const MAX_REPLY: usize = 280;
/// What a villager says with no line of its own.
pub const GREETING: &str = "Good day to you.";

/// What kind of encounter a villager remembers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The first time they met.
    Greeted,
    /// A conversation.
    Talked,
    /// The player reached a quest step near it.
    StepReached,
    /// The player gave it something.
    Gift,
}

impl Kind {
    /// The importance its memory starts with, 1 to 10, by rule.
    #[must_use]
    pub const fn importance(self) -> f64 {
        match self {
            Self::Greeted => 4.0,
            Self::Talked => 3.0,
            Self::StepReached => 6.0,
            Self::Gift => 7.0,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Greeted => "greeted",
            Self::Talked => "talked",
            Self::StepReached => "step reached",
            Self::Gift => "gift",
        }
    }
}

/// One encounter with the player.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Encounter {
    pub kind: Kind,
    /// What happened, in a few words.
    pub note: String,
    /// The rumor the villager told, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rumor: Option<String>,
}

impl Encounter {
    #[must_use]
    pub fn new(kind: Kind, note: &str) -> Self {
        Self {
            kind,
            note: clip(note, MAX_NOTE),
            rumor: None,
        }
    }
}

/// One stored memory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub encounter: Encounter,
    /// Unix seconds.
    pub created: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_access: Option<u64>,
    pub importance: f64,
}

/// Model replies spent on one town day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replies {
    pub day: i64,
    pub count: u32,
}

/// One player's townsfolk save: each villager's memories of the player,
/// and the replies spent today.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub schema: String,
    /// By villager ID.
    #[serde(default)]
    pub villagers: BTreeMap<String, Vec<Saved>>,
    #[serde(default)]
    pub replies: Replies,
}

fn clip(text: &str, max: usize) -> String {
    let clean: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    if clean.chars().count() <= max {
        return clean;
    }
    let mut out: String = clean.chars().take(max.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

impl Save {
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema: SAVE_SCHEMA.into(),
            ..Self::default()
        }
    }

    /// Reads a save, bounding what it holds.
    ///
    /// # Errors
    ///
    /// When the text isn't a save.
    pub fn parse(json: &str) -> Result<Self, String> {
        let mut save: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if save.schema != SAVE_SCHEMA {
            return Err(format!("is {:?}, not {SAVE_SCHEMA}", save.schema));
        }
        save.bound();
        Ok(save)
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        crate::pretty(self)
    }

    /// Keeps at most [`MEMORIES_PER_VILLAGER`] memories a villager, each
    /// note within [`MAX_NOTE`], and at most the villager ceiling, dropping
    /// the villagers met longest ago.
    pub fn bound(&mut self) {
        for memories in self.villagers.values_mut() {
            memories.sort_by_key(|m| m.created);
            let extra = memories.len().saturating_sub(MEMORIES_PER_VILLAGER);
            memories.drain(..extra);
            for m in memories.iter_mut() {
                m.encounter.note = clip(&m.encounter.note, MAX_NOTE);
            }
        }
        self.villagers.retain(|_, m| !m.is_empty());
        let most = Budgets::CEILING.villagers as usize;
        while self.villagers.len() > most {
            let oldest = self
                .villagers
                .iter()
                .min_by_key(|(_, m)| m.iter().map(|s| s.created).max().unwrap_or(0))
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                self.villagers.remove(&id);
            }
        }
    }

    /// `villager`'s memories of this player as a stream.
    #[must_use]
    pub fn stream(&self, villager: &str) -> Stream<Encounter> {
        let memories = self
            .villagers
            .get(villager)
            .map(|saved| {
                saved
                    .iter()
                    .map(|s| Memory {
                        item: s.encounter.clone(),
                        created: s.created,
                        last_access: s.last_access,
                        importance: s.importance,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Stream::from_memories(MEMORIES_PER_VILLAGER, memories)
    }

    fn store(&mut self, villager: &str, stream: Stream<Encounter>) {
        let saved: Vec<Saved> = stream
            .into_memories()
            .into_iter()
            .map(|m| Saved {
                encounter: m.item,
                created: m.created,
                last_access: m.last_access,
                importance: m.importance,
            })
            .collect();
        self.villagers.insert(villager.to_owned(), saved);
        self.bound();
    }

    /// Writes an encounter into `villager`'s memory of this player at
    /// `now`, Unix seconds, with its kind's importance.
    pub fn remember(&mut self, villager: &str, encounter: Encounter, now: u64) {
        let mut stream = self.stream(villager);
        let importance = encounter.kind.importance();
        stream.push(encounter, now, importance);
        self.store(villager, stream);
    }

    /// `villager`'s `limit` best memories of this player at `now`, best
    /// first; retrieval marks them accessed, as the stream does.
    pub fn recall(&mut self, villager: &str, now: u64, limit: usize) -> Vec<(Encounter, u64)> {
        let mut stream = self.stream(villager);
        let created: Vec<(Encounter, u64)> = stream
            .memories()
            .iter()
            .map(|m| (m.item.clone(), m.created))
            .collect();
        let picked: Vec<Encounter> = stream
            .retrieve(now, limit, |_| 0.0)
            .into_iter()
            .map(|(e, _)| e.clone())
            .collect();
        if !picked.is_empty() {
            self.store(villager, stream);
        }
        picked
            .into_iter()
            .map(|e| {
                let at = created
                    .iter()
                    .find(|(c, _)| *c == e)
                    .map_or(now, |(_, t)| *t);
                (e, at)
            })
            .collect()
    }

    /// Whether `villager` has met this player.
    #[must_use]
    pub fn met(&self, villager: &str) -> bool {
        self.villagers.get(villager).is_some_and(|m| !m.is_empty())
    }

    /// Whether `villager` already told this player `rumor`.
    #[must_use]
    pub fn told(&self, villager: &str, rumor: &str) -> bool {
        self.villagers.get(villager).is_some_and(|m| {
            m.iter()
                .any(|s| s.encounter.rumor.as_deref() == Some(rumor))
        })
    }

    /// Model replies spent on town day `day`.
    #[must_use]
    pub fn replies_on(&self, day: i64) -> u32 {
        if self.replies.day == day {
            self.replies.count
        } else {
            0
        }
    }

    fn spend(&mut self, day: i64) {
        let count = self.replies_on(day) + 1;
        self.replies = Replies { day, count };
    }
}

/// A conversation's setting.
pub struct Talk<'a> {
    pub villager: &'a Npc,
    /// The player's name, as the villager calls them.
    pub player: &'a str,
    /// The player's current quest step, when the quest line reports one.
    pub step: Option<&'a str>,
    /// The rumors the villager knows now.
    pub rumors: &'a [&'a Rumor],
    /// Whether the player configured a model provider.
    pub provider: bool,
    pub budgets: Budgets,
    pub time: TownTime,
    /// Unix seconds.
    pub now: u64,
}

/// The text of a model reply's request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub system: String,
    pub user: String,
}

/// Why a villager said a fixed line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixed {
    /// Its line for the player's quest step.
    Step,
    /// The player has no model provider.
    NoProvider,
    /// The player spent today's replies.
    Cap,
    /// The model call failed.
    Failed,
}

/// What a villager will say.
#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// A fixed line, and the rumor it tells, if any.
    Fixed {
        text: String,
        why: Fixed,
        rumor: Option<String>,
    },
    /// Ask the model; today's reply is already spent.
    Ask(Prompt),
}

/// Writes the reply a villager gives, from a prompt.
pub trait Answerer {
    /// # Errors
    ///
    /// Why there is no reply, such as a failed call.
    fn answer(&mut self, prompt: &Prompt) -> Result<String, String>;
}

/// An answerer that always says one thing, for tests and captures.
pub struct Canned(pub String);

impl Answerer for Canned {
    fn answer(&mut self, _: &Prompt) -> Result<String, String> {
        Ok(self.0.clone())
    }
}

fn ago(now: u64, then: u64) -> String {
    let hours = now.saturating_sub(then) / 3_600;
    match hours {
        0 => "just now".into(),
        1 => "an hour ago".into(),
        h if h < 48 => format!("{h} hours ago"),
        h => format!("{} days ago", h / 24),
    }
}

/// The prompt for a reply: the character card, the villager's top
/// memories of the player with how long ago, and the rumors it knows with
/// the quest step each points at.
#[must_use]
pub fn prompt(talk: &Talk, memories: &[(Encounter, u64)]) -> Prompt {
    let npc = talk.villager;
    let player = talk.player;
    let mut system = format!(
        "You are {name}, a character in Everglade, a town in the game Verse. You are the \
         town's {role}. {about}\n\
         Stay in character. Reply in one to three short sentences of plain text, with no lists \
         and no markdown. Speak only of the town, your work, what you remember of {player}, and \
         the news below. Never invent quests, rewards, or places; pass news on as you heard it.\n",
        name = npc.name,
        role = npc.card.role,
        about = npc.card.about,
    );
    if memories.is_empty() {
        system.push_str(&format!("\nYou have not met {player} before.\n"));
    } else {
        system.push_str(&format!("\nWhat you remember of {player}:\n"));
        for (m, at) in memories {
            system.push_str(&format!(
                "- {} ({}): {}\n",
                ago(talk.now, *at),
                m.kind.as_str(),
                m.note
            ));
        }
    }
    if !talk.rumors.is_empty() {
        system.push_str("\nNews you have heard:\n");
        for r in talk.rumors.iter().take(PROMPT_RUMORS) {
            match r.quest_step() {
                Some(step) => system.push_str(&format!(
                    "- {} (this concerns \"{}\" at {})\n",
                    r.fact, step.title, step.place
                )),
                None => system.push_str(&format!("- {}\n", r.fact)),
            }
        }
    }
    Prompt {
        system,
        user: format!("{player} walks up to you and says hello."),
    }
}

/// A fixed line for when there is no model reply: a rumor the villager
/// knows and hasn't told this player, else one of its own lines in turn,
/// else [`GREETING`].
#[must_use]
pub fn fallback(save: &Save, talk: &Talk) -> (String, Option<String>) {
    let id = &talk.villager.id;
    if let Some(r) = talk.rumors.iter().find(|r| !save.told(id, &r.id)) {
        return (format!("Have you heard? {}", r.fact), Some(r.id.clone()));
    }
    let plain: Vec<&str> = talk
        .villager
        .lines
        .iter()
        .filter(|l| l.step().is_none())
        .map(crate::Line::text)
        .collect();
    if plain.is_empty() {
        return (GREETING.to_owned(), None);
    }
    let turn = save.villagers.get(id).map_or(0, Vec::len);
    (plain[turn % plain.len()].to_owned(), None)
}

/// What the villager will say: its line for the current quest step, else
/// a model reply for a player with a provider under the daily cap (which
/// this spends), else [`fallback`].
pub fn plan(save: &mut Save, talk: &Talk) -> Plan {
    if let Some(step) = talk.step
        && let Some(line) = talk
            .villager
            .lines
            .iter()
            .find(|l| l.step().is_some_and(|key| quest::matches(key, step)))
    {
        return Plan::Fixed {
            text: line.text().to_owned(),
            why: Fixed::Step,
            rumor: None,
        };
    }
    let why = if !talk.provider {
        Some(Fixed::NoProvider)
    } else if save.replies_on(talk.time.day) >= talk.budgets.replies_per_player_per_day {
        Some(Fixed::Cap)
    } else {
        None
    };
    if let Some(why) = why {
        let (text, rumor) = fallback(save, talk);
        return Plan::Fixed { text, why, rumor };
    }
    save.spend(talk.time.day);
    let memories = save.recall(&talk.villager.id, talk.now, PROMPT_MEMORIES);
    Plan::Ask(prompt(talk, &memories))
}

/// A model reply cleaned for a bubble: one line, within [`MAX_REPLY`].
#[must_use]
pub fn clean(reply: &str) -> String {
    clip(reply.trim().trim_matches('"'), MAX_REPLY)
}

/// Records that the villager said `text` to the player, and told `rumor`:
/// a first meeting is remembered as one.
pub fn finish(save: &mut Save, talk: &Talk, text: &str, rumor: Option<String>) {
    let id = &talk.villager.id;
    let kind = if save.met(id) {
        Kind::Talked
    } else {
        Kind::Greeted
    };
    let mut encounter = Encounter::new(kind, &format!("I said: {text}"));
    encounter.rumor = rumor;
    save.remember(id, encounter, talk.now);
}

/// What a villager said, and how.
#[derive(Clone, Debug, PartialEq)]
pub struct Said {
    pub text: String,
    /// `None` for a model reply.
    pub fixed: Option<Fixed>,
}

/// One whole conversation turn: [`plan`], the model through `answerer`
/// when the plan asks, and [`finish`]. A failed call says the fallback.
pub fn talk(save: &mut Save, talk: &Talk, answerer: &mut dyn Answerer) -> Said {
    let (text, fixed, rumor) = match plan(save, talk) {
        Plan::Fixed { text, why, rumor } => (text, Some(why), rumor),
        Plan::Ask(prompt) => match answerer.answer(&prompt) {
            Ok(reply) if !reply.trim().is_empty() => (clean(&reply), None, None),
            _ => {
                let (text, rumor) = fallback(save, talk);
                (text, Some(Fixed::Failed), rumor)
            }
        },
    };
    finish(save, talk, &text, rumor);
    Said { text, fixed }
}
