//! Talking to Everglade's townsfolk on the desktop
//! (`docs/verse/generative-agents.md`, item 5, phase E2).
//!
//! `F` next to a villager asks `townsfolk::talk` what it says: its line for
//! the player's quest step, else one model reply through the agent's door
//! ([`crate::brain::Voice`]) under the roster's per-player daily cap, else a
//! fixed line. A machine with no door, such as a stranger's, never calls a
//! model. The player's townsfolk save, each villager's memories of the
//! player and the day's reply count, lives in
//! `PROFILE-townsfolk.json` beside the player's other Verse files. The web
//! and phone builds have no talk path yet.

use std::path::{Path, PathBuf};

use town_clock::TownTime;
use townsfolk::rumor::Rumor;
use townsfolk::talk::{self, Plan, Save, Talk};
use townsfolk::{Budgets, Npc};

use crate::brain::Voice;

/// The largest save file read, bytes.
const MAX_SAVE: u64 = 1 << 20;

/// A model reply in flight.
struct Waiting {
    npc: Npc,
    rumors: Vec<Rumor>,
    budgets: Budgets,
    time: TownTime,
    now: u64,
}

/// The desktop's conversations with villagers.
pub struct TownTalk {
    player: String,
    save: Save,
    path: Option<PathBuf>,
    voice: Option<Voice>,
    waiting: Option<Waiting>,
}

/// The save file for `profile` under `home`.
#[must_use]
pub fn save_path(home: &Path, profile: &str) -> PathBuf {
    let name: String = profile
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(32)
        .collect();
    let name = if name.is_empty() { "player" } else { &name };
    home.join(format!("{name}-townsfolk.json"))
}

/// The save at `path`, or a new one when there is none or it doesn't read.
#[must_use]
pub fn load(path: &Path) -> Save {
    let fits = std::fs::metadata(path).is_ok_and(|m| m.len() <= MAX_SAVE);
    fits.then(|| std::fs::read_to_string(path).ok())
        .flatten()
        .and_then(|text| Save::parse(&text).ok())
        .unwrap_or_else(Save::new)
}

fn write(path: &Path, save: &Save) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, save.to_json()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl TownTalk {
    /// Conversations for `player`, saved at `path` (memory only when
    /// `None`), with a model only when `voice` is given.
    #[must_use]
    pub fn new(player: &str, path: Option<PathBuf>, voice: Option<Voice>) -> Self {
        let save = path.as_deref().map_or_else(Save::new, load);
        Self {
            player: player.to_owned(),
            save,
            path,
            voice,
            waiting: None,
        }
    }

    /// Whether a model reply is in flight.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.waiting.is_some()
    }

    fn keep(&self) {
        if let Some(path) = &self.path
            && let Err(e) = write(path, &self.save)
        {
            eprintln!("verse: the townsfolk save at {}: {e}", path.display());
        }
    }

    /// Talks to `npc`, who knows `rumors`, at town `time`: the words to show
    /// now, or `None` while a model reply is on its way ([`Self::poll`]).
    pub fn talk(
        &mut self,
        npc: &Npc,
        rumors: &[&Rumor],
        budgets: Budgets,
        time: TownTime,
    ) -> Option<String> {
        if self.waiting.is_some() {
            return None;
        }
        let now = unix_now();
        let setting = Talk {
            villager: npc,
            player: &self.player,
            step: None,
            rumors,
            provider: self.voice.is_some(),
            budgets,
            time,
            now,
        };
        match talk::plan(&mut self.save, &setting) {
            Plan::Fixed { text, rumor, .. } => {
                talk::finish(&mut self.save, &setting, &text, rumor);
                self.keep();
                Some(text)
            }
            Plan::Ask(prompt) => {
                let asked = self
                    .voice
                    .as_ref()
                    .is_some_and(|v| v.ask(prompt.system, prompt.user));
                let waiting = Waiting {
                    npc: npc.clone(),
                    rumors: rumors.iter().map(|r| (*r).clone()).collect(),
                    budgets,
                    time,
                    now,
                };
                self.keep();
                if asked {
                    self.waiting = Some(waiting);
                    None
                } else {
                    Some(self.settle(&waiting, Err("no voice".into())))
                }
            }
        }
    }

    /// The villager's ID and words once a model reply came.
    pub fn poll(&mut self) -> Option<(String, String)> {
        let result = self.voice.as_ref()?.poll()?;
        let waiting = self.waiting.take()?;
        let text = self.settle(&waiting, result);
        Some((waiting.npc.id, text))
    }

    fn settle(&mut self, waiting: &Waiting, result: Result<String, String>) -> String {
        let rumors: Vec<&Rumor> = waiting.rumors.iter().collect();
        let setting = Talk {
            villager: &waiting.npc,
            player: &self.player,
            step: None,
            rumors: &rumors,
            provider: true,
            budgets: waiting.budgets,
            time: waiting.time,
            now: waiting.now,
        };
        let (text, rumor) = match result {
            Ok(reply) if !reply.trim().is_empty() => (talk::clean(&reply), None),
            Ok(_) => talk::fallback(&self.save, &setting),
            Err(e) => {
                eprintln!("verse: {} couldn't answer: {e}", waiting.npc.name);
                talk::fallback(&self.save, &setting)
            }
        };
        talk::finish(&mut self.save, &setting, &text, rumor);
        self.keep();
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_voice_a_villager_says_fixed_lines_and_remembers_the_player() {
        let temp = tempfile::tempdir().unwrap();
        let path = save_path(temp.path(), "kiki/../x");
        assert_eq!(path.file_name().unwrap(), "kikix-townsfolk.json");
        let (roster, _) = crate::zones::everglade::townsfolk::roster();
        let mira = &roster.villager("mira-baker").unwrap().npc;
        let time = TownTime::at_hour(3, 12.0);
        let mut talk = TownTalk::new("kiki", Some(path.clone()), None);
        let said = talk.talk(mira, &[], roster.town.budgets, time).unwrap();
        assert!(mira.lines.iter().any(|l| l.text() == said));
        assert!(!talk.waiting());
        let back = load(&path);
        assert!(back.met("mira-baker"));
        assert_eq!(back.replies_on(3), 0, "no model, no reply spent");
    }
}
