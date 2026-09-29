//! The XP ledger: what a reader derives from NIP-XP quests, awards, and
//! revocations (`nips/openagents/NIP-XP.md`).
//!
//! XP is evidence of accepted outcomes, never a balance: nothing here
//! spends, transfers, or converts it, and it is computed fresh from signed
//! events every time. Only awards signed by referees the reader trusts
//! count, each is re-checked against the entry and evidence it names, a
//! revoked award is dropped, and a uniqueness key with more than one live
//! award counts for nobody until its referee revokes the extra ones. Levels
//! and titles are the client's reading of the totals.
//!
//! Signing and the relay connection are the caller's; nothing here opens a
//! socket. The crate is small on purpose: the phone links it through
//! Verse's read-only XP reader, and it must not pull in the knowledge
//! base's model and embedding clients. `knowledge` re-exports it as
//! `knowledge::xp`, and its entry parser as `knowledge::Entry`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use nostr::domain::Event;
use nostr::kb;
use nostr::xp::{self, Award};
use serde::Serialize;

pub mod adopt;
pub mod defaults;
pub mod entry;
pub mod eval;
pub mod front;
pub mod trainers;

pub use trainers::{Profile, Trainers};

use crate::entry::{Entry, task_of};

/// `~/.openagents/nostr/knowledge-key`, the secret key entries are signed
/// with. It's created on first use with mode 0600.
#[must_use]
pub fn key_file() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/nostr/knowledge-key"))
}

/// The public key of the secret key in `path`, without creating one.
#[must_use]
pub fn own_pubkey(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let signer = nostr::domain::RelaySigner::from_secret_hex(text.trim()).ok()?;
    Some(signer.pubkey().to_string())
}

/// A hex public key as an `npub`.
#[must_use]
pub fn npub(pubkey: &str) -> String {
    let mut bytes = [0u8; 32];
    for (i, pair) in pubkey.as_bytes().chunks(2).take(32).enumerate() {
        bytes[i] = std::str::from_utf8(pair)
            .ok()
            .and_then(|p| u8::from_str_radix(p, 16).ok())
            .unwrap_or(0);
    }
    nostr::nip19::encode_npub(&bytes)
}

/// A public key written as an `npub` or 64 lowercase hex characters, as hex.
#[must_use]
pub fn parse_key(text: &str) -> Option<String> {
    let text = text.trim();
    if let Ok(bytes) = nostr::nip19::decode_npub(text) {
        return Some(bytes.iter().map(|b| format!("{b:02x}")).collect());
    }
    (text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    .then(|| text.to_string())
}

/// `~/.openagents/knowledge/xp-trust.json`.
#[must_use]
pub fn trust_file() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".openagents/knowledge/xp-trust.json"))
}

/// `~/.openagents/nostr/referee-key`, the secret key quests, awards, and
/// revocations are signed with. It's created on first use with mode 0600.
#[must_use]
pub fn referee_key_file() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/nostr/referee-key"))
}

/// `~/.openagents/nostr/playtest-referee-key`, the separate key that signs
/// `playtest` quests and awards. It is never created on first use:
/// `microcoder xp playtest-keygen` makes it, once, on the owner's machine.
#[must_use]
pub fn playtest_referee_key_file() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".openagents/nostr/playtest-referee-key"))
}

/// Whose awards a reader counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct XpTrust {
    /// Hex public keys of the referees whose awards count.
    pub referees: BTreeSet<String>,
    /// Hex public keys of the runners whose evidence counts. Empty means
    /// any runner a trusted referee accepted.
    pub runners: BTreeSet<String>,
}

impl XpTrust {
    /// Reads `{"referees": ["npub1…"], "runners": ["npub1…"]}` from
    /// `path`. A missing file trusts no one.
    ///
    /// # Errors
    ///
    /// A file that exists but doesn't parse, or a key that isn't a public
    /// key.
    pub fn read(path: &Path) -> Result<Self, String> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(XpTrust::default());
        };
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut trust = XpTrust::default();
        for (field, set) in [
            ("referees", &mut trust.referees),
            ("runners", &mut trust.runners),
        ] {
            for key in value[field].as_array().cloned().unwrap_or_default() {
                let text = key.as_str().unwrap_or_default();
                set.insert(parse_key(text).ok_or(format!(
                    "{}: {text} isn't an npub or a hex public key",
                    path.display()
                ))?);
            }
        }
        Ok(trust)
    }
}

/// The tasks an entry version was written from, which never count as
/// evidence for it: its `provenance.written_from` runs, as tasks.
///
/// # Errors
///
/// When the event isn't a valid `3190` or its document doesn't parse.
pub fn excluded_tasks(entry: &Event) -> Result<Vec<String>, String> {
    let version = kb::parse_entry(entry).map_err(|e| e.to_string())?;
    let parsed = Entry::parse(&version.document)?;
    let tasks: BTreeSet<String> = parsed
        .written_from
        .iter()
        .filter(|w| w.as_str() != "reference")
        .map(|w| task_of(w))
        .collect();
    Ok(tasks.into_iter().collect())
}

/// Checks one `kb-transfer` award completely: the award alone, its quest,
/// and the quest's rule over the entry and evidence it names.
///
/// # Errors
///
/// A message naming the first check that failed.
pub fn verify(
    award_event: &Event,
    quest: &Event,
    entry: &Event,
    evidence: &Event,
) -> Result<(Award, xp::Quest), String> {
    let award = xp::parse_award(award_event).map_err(|e| e.to_string())?;
    let parsed = xp::bind_quest(&award, quest).map_err(|e| e.to_string())?;
    let excluded = excluded_tasks(entry)?;
    xp::bind_evidence(&award, &parsed, entry, evidence, &excluded).map_err(|e| e.to_string())?;
    Ok((award, parsed))
}

/// Checks one `reproduce` award completely: the award alone, its quest,
/// and the rule over the claim and reproduction it names.
///
/// # Errors
///
/// A message naming the first check that failed.
pub fn verify_reproduction(
    award_event: &Event,
    quest: &Event,
    claim: &Event,
    reproduction: &Event,
) -> Result<(Award, xp::Quest), String> {
    let award = xp::parse_award(award_event).map_err(|e| e.to_string())?;
    let parsed = xp::bind_quest(&award, quest).map_err(|e| e.to_string())?;
    xp::bind_reproduction(&award, &parsed, claim, reproduction).map_err(|e| e.to_string())?;
    Ok((award, parsed))
}

/// Checks one `playtest` award completely: the award alone, its quest,
/// its rule-derived key, and the rule over the tester's report and, for a
/// moderated or group session, the moderator's session record.
///
/// # Errors
///
/// A message naming the first check that failed.
pub fn verify_playtest(
    award_event: &Event,
    quest: &Event,
    report: &Event,
    session: Option<&Event>,
) -> Result<(Award, xp::Quest), String> {
    let award = xp::parse_award(award_event).map_err(|e| e.to_string())?;
    let parsed = xp::bind_quest(&award, quest).map_err(|e| e.to_string())?;
    xp::bind_playtest(&award, &parsed, report, session).map_err(|e| e.to_string())?;
    Ok((award, parsed))
}

/// One awardee's share of one counted award.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Credit {
    /// The `3193` event ID.
    pub award: String,
    pub referee: String,
    /// The quest version's address, `<id>@<version>`.
    pub quest: String,
    pub title: String,
    pub season: String,
    /// The quest's rule: `kb-transfer`, `reproduce`, `playtest`,
    /// `eval-check`, or `eval-adopt`.
    pub rule: String,
    /// `author` or `runner` under `kb-transfer`; `claimant` or
    /// `reproducer` under `reproduce`; `tester` or `triager` under
    /// `playtest`; `checker`, `evaluator`, or `suite-author` under
    /// `eval-check`; `extension-author`, `suite-author`, or `evaluator`
    /// under `eval-adopt`.
    pub role: String,
    pub pubkey: String,
    pub xp: u64,
    /// The event IDs the award names as evidence, in its order.
    pub evidence: Vec<String>,
}

/// What a reader derives.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Ledger {
    /// XP per hex public key.
    pub totals: BTreeMap<String, u64>,
    pub credits: Vec<Credit>,
    /// Valid quests from trusted referees, by address.
    pub quests: BTreeMap<String, String>,
    /// Award IDs their referee revoked.
    pub revoked: Vec<String>,
    /// Awards that failed a check, with why.
    pub refused: Vec<String>,
    /// Uniqueness keys and quest addresses with conflicting records; they
    /// count for no one.
    pub conflicts: Vec<String>,
    /// Awards signed by referees the reader doesn't trust.
    pub untrusted: usize,
}

/// A verified award, its quest, and its event ID.
type Checked = (Award, xp::Quest, String);

fn short(hex: &str) -> &str {
    &hex[..hex.len().min(12)]
}

/// Derives the ledger from `events`: quests, awards, and revocations,
/// plus the `3190` entries and `3189` evidence the awards name. Events may
/// come from several relays and repeat. When the reader lists runners, a
/// `kb-transfer` award counts only when its runner is listed, and a
/// `reproduce` award only when its reproducer is; the runner list doesn't
/// apply to `playtest`, whose evidence is a tester's report.
///
/// A `playtest` award's key is derived by its rule, and a `per-awardee`
/// award's key names its keyed awardee, so each distinct reproducer is
/// paid once. A quest version with more live awards than its `max_awards`
/// counts none of them.
///
/// `eval-adopt` awards also need the `coder-defaults` manifest and
/// admission bytes their release pins; [`derive`] holds none, so it
/// refuses them, and [`derive_with`] takes them.
#[must_use]
pub fn derive(events: &[Event], trust: &XpTrust) -> Ledger {
    derive_with(events, &eval::Documents::new(), trust)
}

/// [`derive`], with the document bytes `eval-adopt` awards are checked
/// against.
#[must_use]
pub fn derive_with(events: &[Event], documents: &eval::Documents, trust: &XpTrust) -> Ledger {
    let mut ledger = Ledger::default();
    let publications: Vec<Event> = eval::publications(events)
        .into_values()
        .map(|(event, _)| event)
        .collect();
    let requests = eval::requests(events);
    let mut by_id: BTreeMap<&str, &Event> = BTreeMap::new();
    for event in events {
        by_id.entry(event.id.as_str()).or_insert(event);
    }
    let trusted = |event: &Event| trust.referees.contains(&event.pubkey);

    // Quests: one event per address. Two different events at one address
    // are a referee rewriting a frozen version, so neither counts.
    let mut addresses: BTreeMap<(String, String), BTreeSet<&str>> = BTreeMap::new();
    for event in by_id
        .values()
        .filter(|e| e.kind == xp::QUEST_KIND && trusted(e))
    {
        if let Ok(quest) = xp::parse_quest(event) {
            addresses
                .entry((event.pubkey.clone(), quest.address.clone()))
                .or_default()
                .insert(event.id.as_str());
            ledger.quests.insert(quest.address, quest.title);
        }
    }
    let rewritten: BTreeSet<String> = addresses
        .iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|((referee, address), _)| xp::coordinate(referee, address))
        .collect();
    for coordinate in &rewritten {
        ledger.conflicts.push(format!(
            "{coordinate}: the quest version was published twice with different content"
        ));
    }

    // Revocations: only the award's own referee's count.
    let mut revoked: BTreeSet<String> = BTreeSet::new();
    for event in by_id
        .values()
        .filter(|e| e.kind == xp::REVOCATION_KIND && trusted(e))
    {
        if let Ok(revocation) = xp::parse_revocation(event) {
            revoked.insert(revocation.award.id);
        }
    }

    let mut live: BTreeMap<(String, String), Vec<Checked>> = BTreeMap::new();
    for event in by_id.values().filter(|e| e.kind == xp::AWARD_KIND) {
        if !trusted(event) {
            ledger.untrusted += 1;
            continue;
        }
        if revoked.contains(&event.id) {
            ledger.revoked.push(event.id.clone());
            continue;
        }
        let checked = (|| {
            let award = xp::parse_award(event).map_err(|e| e.to_string())?;
            if rewritten.contains(&award.coordinate) {
                return Err("its quest version was rewritten".to_string());
            }
            let find = |id: &str, what: &str| {
                by_id
                    .get(id)
                    .copied()
                    .ok_or(format!("the {what} {} isn't available", short(id)))
            };
            let quest = find(&award.quest.id, "quest")?;
            if award.rule == xp::EVAL_CHECK {
                let result = find(&award.evidence[0].id, "result")?;
                let check = find(&award.evidence[1].id, "check")?;
                return eval::verify_eval_check(event, quest, result, check, &requests);
            }
            if award.rule == xp::EVAL_ADOPT {
                let release = find(&award.evidence[0].id, "defaults release")?;
                return eval::verify_eval_adopt(
                    event,
                    quest,
                    release,
                    &publications,
                    &requests,
                    documents,
                );
            }
            if award.rule == xp::PLAYTEST {
                let report = find(&award.evidence[0].id, "report")?;
                let session = match award.evidence.get(1) {
                    Some(record) => Some(find(&record.id, "session record")?),
                    None => None,
                };
                return verify_playtest(event, quest, report, session);
            }
            let (award, quest) = if award.rule == xp::REPRODUCE {
                let claim = find(&award.evidence[0].id, "claim")?;
                let reproduction = find(&award.evidence[1].id, "reproduction")?;
                verify_reproduction(event, quest, claim, reproduction)?
            } else {
                let entry_id = &award.entry.as_ref().ok_or("no entry")?.id;
                let entry = find(entry_id, "entry")?;
                let evidence = find(&award.evidence[0].id, "evidence")?;
                verify(event, quest, entry, evidence)?
            };
            let role = if award.rule == xp::REPRODUCE {
                "reproducer"
            } else {
                "runner"
            };
            let runner = &award.role(role).ok_or("no runner")?.pubkey;
            if !trust.runners.is_empty() && !trust.runners.contains(runner) {
                return Err(format!("the runner {} isn't trusted", short(runner)));
            }
            Ok((award, quest))
        })();
        match checked {
            Ok((award, quest)) => live
                .entry((event.pubkey.clone(), award.key.clone()))
                .or_default()
                .push((award, quest, event.id.clone())),
            Err(why) => ledger
                .refused
                .push(format!("award {}: {why}", short(&event.id))),
        }
    }

    // A `per-awardee` or `playtest` quest version states the most awards
    // it pays; more live awards than that is the referee overissuing, and
    // none of them count.
    let mut per_quest: BTreeMap<(String, String), (u64, usize)> = BTreeMap::new();
    for ((referee, _), awards) in &live {
        for (award, quest, _) in awards {
            if let Some(limit) = quest.award_limit() {
                per_quest
                    .entry((referee.clone(), award.coordinate.clone()))
                    .or_insert((limit, 0))
                    .1 += 1;
            }
        }
    }
    let overissued: BTreeSet<(String, String)> = per_quest
        .into_iter()
        .filter(|(_, (max, n))| (*n as u64) > *max)
        .map(|(quest, (max, n))| {
            ledger.conflicts.push(format!(
                "{}: {n} live awards over its max_awards of {max}; none counts until the \
referee revokes the extra ones",
                quest.1
            ));
            quest
        })
        .collect();
    for ((referee, key), awards) in live {
        if awards.first().is_some_and(|(award, _, _)| {
            overissued.contains(&(referee.clone(), award.coordinate.clone()))
        }) {
            continue;
        }
        if awards.len() > 1 {
            let ids: Vec<&str> = awards.iter().map(|(_, _, id)| short(id)).collect();
            ledger.conflicts.push(format!(
                "{key}: {} live awards ({}); a uniqueness key pays once, so none counts until \
the referee revokes the extra ones",
                awards.len(),
                ids.join(", ")
            ));
            continue;
        }
        let (award, quest, id) = &awards[0];
        for awardee in &award.awardees {
            if awardee.xp == 0 {
                continue;
            }
            *ledger.totals.entry(awardee.pubkey.clone()).or_default() += awardee.xp;
            ledger.credits.push(Credit {
                award: id.clone(),
                referee: referee.clone(),
                quest: quest.address.clone(),
                title: quest.title.clone(),
                season: quest.season.id.clone(),
                rule: award.rule.clone(),
                role: awardee.role.clone(),
                pubkey: awardee.pubkey.clone(),
                xp: awardee.xp,
                evidence: award.evidence.iter().map(|e| e.id.clone()).collect(),
            });
        }
    }
    ledger
}

#[cfg(test)]
mod tests;
