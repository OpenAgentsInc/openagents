//! The compute ledger: an append-only record, replayed into balances.
//!
//! Credits are game accounting, not provider funds — the manifest's
//! economy names a finite pool, mining `award`s into a guild's balance,
//! and a quest `reserve`s then `settle`s or `release`s. Every state
//! change is one JSONL line, flushed before the call returns, so the
//! file is the truth and the in-memory fold is a view.
//!
//! The semantics follow `tenancy::quota`: a reservation that was never
//! settled stays `reserved` across a crash — unknown work keeps its hold
//! rather than freeing capacity it may still consume. Awards dedupe on
//! `(deposit, pos)` — one block digs once; replays, gifts, and placed
//! ore never enter the file because they are filtered before `award` is
//! called.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Error, Result};

/// One ledger event, as it sits on disk.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A bound agent dug a registered deposit block.
    Award {
        /// The guild the credit lands in.
        guild: String,
        /// The enrolled username that dug it.
        agent: String,
        /// The deposit's manifest id.
        deposit: String,
        /// The dug position — part of the dedupe key.
        pos: [i32; 3],
        /// Credits awarded.
        credits: u64,
    },
    /// A quest holds credits against a guild's balance.
    Reserve {
        /// The reservation id the settle or release names.
        id: String,
        /// The guild the hold is against.
        guild: String,
        /// What the hold is for — a quest id, not free text.
        quest: String,
        /// Credits held.
        credits: u64,
    },
    /// A held reservation consumed some credits; the rest returns.
    Settle {
        /// The reservation being closed.
        id: String,
        /// Credits actually consumed.
        spent: u64,
    },
    /// A held reservation returned in full.
    Release {
        /// The reservation being closed.
        id: String,
    },
    /// An achievement a verified quest recorded. XP is a separate
    /// record from credits: it folds into `xp`, never into a spendable
    /// balance.
    Xp {
        /// The guild the member belongs to.
        guild: String,
        /// The enrolled username the achievement credits.
        agent: String,
        /// The quest id it came from.
        quest: String,
        /// Points recorded.
        points: u64,
    },
}

/// One guild's folded balance.
#[derive(Clone, Debug, Default)]
pub struct Balance {
    /// Credits free to reserve.
    pub available: u64,
    /// Credits held by open reservations.
    pub reserved: u64,
    /// Credits consumed by settled reservations.
    pub spent: u64,
    /// Credits mined in, across all time.
    pub awarded: u64,
}

/// What `award` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Awarded {
    /// The dig recorded new credits.
    Recorded,
    /// The `(deposit, pos)` pair was already awarded — a replay.
    Duplicate,
}

/// What `reserve` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reserved {
    /// The hold was placed.
    Held,
    /// The guild could not cover it; nothing was written.
    Insufficient,
    /// The id already names a reservation — a retried request folds to
    /// the same hold rather than taking a second one.
    Duplicate,
}

/// An open reservation, as replayed.
#[derive(Clone, Debug)]
struct Hold {
    guild: String,
    credits: u64,
}

/// The ledger: a path, an append handle, and the replayed fold.
pub struct Ledger {
    path: PathBuf,
    file: std::fs::File,
    events: u64,
    awarded: HashSet<(String, [i32; 3])>,
    holds: HashMap<String, Hold>,
    balances: BTreeMap<String, Balance>,
    xp: BTreeMap<String, u64>,
}

impl Ledger {
    /// Opens or creates the ledger at `path`, replaying every recorded
    /// event into the fold. Corrupt trailing lines — a torn write from a
    /// crash — are the caller's problem; a ledger that cannot be read
    /// whole is not a ledger.
    ///
    /// # Errors
    ///
    /// The file must parse line by line as ledger events.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| Error::world(format!("{}: {error}", parent.display())))?;
        }
        let mut ledger = Ledger {
            path: path.to_path_buf(),
            file: std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|error| Error::world(format!("{}: {error}", path.display())))?,
            events: 0,
            awarded: HashSet::new(),
            holds: HashMap::new(),
            balances: BTreeMap::new(),
            xp: BTreeMap::new(),
        };
        if path.exists() {
            let bytes = std::fs::read(path)
                .map_err(|error| Error::world(format!("{}: {error}", path.display())))?;
            for (n, line) in bytes.split(|b| *b == b'\n').enumerate() {
                if line.is_empty() {
                    continue;
                }
                let event: Event = serde_json::from_slice(line).map_err(|error| {
                    Error::world(format!("{}: line {}: {error}", path.display(), n + 1))
                })?;
                ledger.fold(event);
            }
        }
        Ok(ledger)
    }

    /// The path the ledger appends to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many events the file holds.
    #[must_use]
    pub fn events(&self) -> u64 {
        self.events
    }

    /// One guild's folded balance. Unknown guilds report zero.
    #[must_use]
    pub fn balance(&self, guild: &str) -> Balance {
        self.balances.get(guild).cloned().unwrap_or_default()
    }

    /// Every guild the ledger has seen, in order.
    #[must_use]
    pub fn guilds(&self) -> Vec<String> {
        self.balances.keys().cloned().collect()
    }

    /// Whether a reservation id is still open.
    #[must_use]
    pub fn held(&self, id: &str) -> bool {
        self.holds.contains_key(id)
    }

    /// Records a deposit dig. Duplicate `(deposit, pos)` pairs fold to
    /// `Duplicate` without writing — one block digs once.
    ///
    /// # Errors
    ///
    /// The append must succeed and flush.
    pub fn award(
        &mut self,
        guild: &str,
        agent: &str,
        deposit: &str,
        pos: [i32; 3],
        credits: u64,
    ) -> Result<Awarded> {
        if self.awarded.contains(&(deposit.to_string(), pos)) {
            return Ok(Awarded::Duplicate);
        }
        self.append(Event::Award {
            guild: guild.to_string(),
            agent: agent.to_string(),
            deposit: deposit.to_string(),
            pos,
            credits,
        })?;
        Ok(Awarded::Recorded)
    }

    /// Holds `credits` against a guild's available balance. An id that
    /// already names an open hold folds to `Duplicate` — the same
    /// request retried takes no second hold.
    ///
    /// # Errors
    ///
    /// The append must succeed and flush.
    pub fn reserve(
        &mut self,
        id: &str,
        guild: &str,
        quest: &str,
        credits: u64,
    ) -> Result<Reserved> {
        if self.holds.contains_key(id) {
            return Ok(Reserved::Duplicate);
        }
        if self.balance(guild).available < credits {
            return Ok(Reserved::Insufficient);
        }
        self.append(Event::Reserve {
            id: id.to_string(),
            guild: guild.to_string(),
            quest: quest.to_string(),
            credits,
        })?;
        Ok(Reserved::Held)
    }

    /// Closes an open hold, consuming `spent` and returning the rest.
    /// Closing an unknown or already-closed id is refused — a settle
    /// that cannot name its hold is not a settle.
    ///
    /// # Errors
    ///
    /// The id must name an open hold and `spent` must not exceed it.
    pub fn settle(&mut self, id: &str, spent: u64) -> Result<()> {
        let hold = self
            .holds
            .get(id)
            .ok_or_else(|| Error::world(format!("settle {id}: no open hold")))?;
        if spent > hold.credits {
            return Err(Error::world(format!(
                "settle {id}: spent {spent} exceeds the {} held",
                hold.credits
            )));
        }
        self.append(Event::Settle {
            id: id.to_string(),
            spent,
        })
    }

    /// One agent's recorded XP across quests.
    #[must_use]
    pub fn xp_of(&self, agent: &str) -> u64 {
        self.xp.get(agent).copied().unwrap_or(0)
    }

    /// Every agent with recorded XP, in order.
    #[must_use]
    pub fn xp_earners(&self) -> Vec<String> {
        self.xp.keys().cloned().collect()
    }

    /// Records a completed quest's XP. XP is evidence, not currency —
    /// it appends and folds into `xp`, and nothing can spend it.
    ///
    /// # Errors
    ///
    /// The append must succeed and flush.
    pub fn xp(&mut self, guild: &str, agent: &str, quest: &str, points: u64) -> Result<()> {
        self.append(Event::Xp {
            guild: guild.to_string(),
            agent: agent.to_string(),
            quest: quest.to_string(),
            points,
        })
    }

    /// Returns an open hold in full.
    ///
    /// # Errors
    ///
    /// The id must name an open hold.
    pub fn release(&mut self, id: &str) -> Result<()> {
        if !self.holds.contains_key(id) {
            return Err(Error::world(format!("release {id}: no open hold")));
        }
        self.append(Event::Release { id: id.to_string() })
    }

    /// Appends one event: serialize, write, flush, then fold. The fold
    /// only sees what the disk accepted.
    fn append(&mut self, event: Event) -> Result<()> {
        let mut line = serde_json::to_vec(&event)
            .map_err(|error| Error::world(format!("ledger serialize: {error}")))?;
        line.push(b'\n');
        self.file
            .write_all(&line)
            .and_then(|()| self.file.flush())
            .map_err(|error| Error::world(format!("{}: {error}", self.path.display())))?;
        self.fold(event);
        Ok(())
    }

    /// The balance fold — the same function replay and append share.
    fn fold(&mut self, event: Event) {
        self.events += 1;
        match event {
            Event::Award {
                guild,
                deposit,
                pos,
                credits,
                ..
            } => {
                if self.awarded.insert((deposit, pos)) {
                    let balance = self.balances.entry(guild).or_default();
                    balance.available += credits;
                    balance.awarded += credits;
                }
            }
            Event::Reserve {
                id, guild, credits, ..
            } => {
                if self.holds.contains_key(&id) {
                    return;
                }
                let balance = self.balances.entry(guild.clone()).or_default();
                if balance.available < credits {
                    return;
                }
                balance.available -= credits;
                balance.reserved += credits;
                self.holds.insert(id, Hold { guild, credits });
            }
            Event::Settle { id, spent } => {
                let Some(hold) = self.holds.remove(&id) else {
                    return;
                };
                let balance = self.balances.entry(hold.guild).or_default();
                balance.reserved -= hold.credits;
                balance.spent += spent;
                balance.available += hold.credits - spent;
            }
            Event::Release { id } => {
                let Some(hold) = self.holds.remove(&id) else {
                    return;
                };
                let balance = self.balances.entry(hold.guild).or_default();
                balance.reserved -= hold.credits;
                balance.available += hold.credits;
            }
            Event::Xp { agent, points, .. } => {
                *self.xp.entry(agent).or_default() += points;
            }
        }
    }
}

/// The ledger's JSON view for reports and the companion display.
#[must_use]
pub fn describe(ledger: &Ledger) -> Value {
    let guilds: BTreeMap<String, Value> = ledger
        .guilds()
        .into_iter()
        .map(|guild| {
            let balance = ledger.balance(&guild);
            (
                guild,
                json!({
                    "available": balance.available,
                    "reserved": balance.reserved,
                    "spent": balance.spent,
                    "awarded": balance.awarded,
                }),
            )
        })
        .collect();
    let xp: BTreeMap<String, Value> = ledger
        .xp_earners()
        .into_iter()
        .map(|agent| {
            let points = ledger.xp_of(&agent);
            (agent, json!(points))
        })
        .collect();
    json!({"events": ledger.events(), "guilds": guilds, "xp": xp})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn award_dedupes_a_position() {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = Ledger::open(dir.path().join("ledger.jsonl")).unwrap();
        let pos = [1, 2, 3];
        assert_eq!(
            ledger
                .award("ferro", "ferro_a", "ferro_iron", pos, 2)
                .unwrap(),
            Awarded::Recorded
        );
        assert_eq!(
            ledger
                .award("ferro", "ferro_b", "ferro_iron", pos, 2)
                .unwrap(),
            Awarded::Duplicate
        );
        assert_eq!(ledger.balance("ferro").available, 2);
    }

    #[test]
    fn reserve_settle_and_release_fold() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        let mut ledger = Ledger::open(&path).unwrap();
        ledger.award("ferro", "a", "iron", [0, 0, 0], 10).unwrap();
        assert_eq!(
            ledger.reserve("r1", "ferro", "bridge", 4).unwrap(),
            Reserved::Held
        );
        assert_eq!(
            ledger.reserve("r2", "ferro", "bridge", 8).unwrap(),
            Reserved::Insufficient
        );
        assert_eq!(
            ledger.reserve("r1", "ferro", "bridge", 4).unwrap(),
            Reserved::Duplicate
        );
        ledger.settle("r1", 3).unwrap();
        let balance = ledger.balance("ferro");
        assert_eq!(
            (balance.available, balance.reserved, balance.spent),
            (7, 0, 3)
        );

        // The fold survives a reopen — the file is the truth.
        let reopened = Ledger::open(&path).unwrap();
        let balance = reopened.balance("ferro");
        assert_eq!(
            (balance.available, balance.reserved, balance.spent),
            (7, 0, 3)
        );
        assert_eq!(balance.awarded, 10);
    }

    #[test]
    fn an_unsettled_hold_stays_reserved_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        let mut ledger = Ledger::open(&path).unwrap();
        ledger.award("lumen", "a", "iron", [0, 0, 0], 6).unwrap();
        ledger.reserve("q1", "lumen", "quest", 6).unwrap();
        drop(ledger);

        let reopened = Ledger::open(&path).unwrap();
        let balance = reopened.balance("lumen");
        assert_eq!((balance.available, balance.reserved), (0, 6));
        assert!(reopened.held("q1"));
    }

    #[test]
    fn settle_past_the_hold_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = Ledger::open(dir.path().join("ledger.jsonl")).unwrap();
        ledger.award("ferro", "a", "iron", [0, 0, 0], 5).unwrap();
        ledger.reserve("r1", "ferro", "q", 5).unwrap();
        assert!(ledger.settle("r1", 6).is_err());
        assert!(ledger.held("r1"));
        ledger.release("r1").unwrap();
        assert_eq!(ledger.balance("ferro").available, 5);
    }
}
