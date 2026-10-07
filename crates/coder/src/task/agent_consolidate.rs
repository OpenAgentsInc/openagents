//! A workshop agent's memory consolidation into `core`
//! (`docs/verse/agent-identity-and-engrams.md`, "Engrams", Consolidation).
//!
//! After a nightly reflection, one model call proposes a new `core`
//! profile ([`propose`]): it keeps her charter and rules, adds standing
//! facts that her active insights support, and links each memory it relies
//! on with a NIP-AE `[[slug]]` reference. Code checks the reply before
//! anything is stored ([`check`]): it differs from her `core`, keeps her
//! charter, fits the 10 KiB `core` cap, passes the secret screen, and links
//! only live, active entries and insights. A proposal that passes is the
//! `mem/proposal/core` engram ([`PROPOSAL_SLUG`]), holding the profile and
//! `base`, the SHA-256 of the `core` it was written against.
//!
//! The owner decides it at F2 like a preference ([`decide`]), as memory
//! row [`PROPOSAL_ID`]. Accepting writes `core` only when her current
//! `core` still hashes to `base`, a compare-and-swap: when `core` changed
//! since, the write is refused, journaled, and the stale proposal is
//! discarded. Rejecting discards it. Nothing else changes `core`.
//!
//! **Fail closed.** A store that cannot be read, or an agent who keeps no
//! engrams, never proposes and never writes `core`.
//!
//! The base-hash compare-and-swap follows Buzz's `mem patch --base-hash`,
//! reimplemented here.

use std::sync::{Arc, Mutex};

use nostr::engram::{Body, Slug};
use serde::{Deserialize, Serialize};

use super::agent::{Entry, Kind, Store};
use super::agent_engrams::{self, CORE_MAX, CORE_SCHEMA, EngramStore, Opened};
use super::agent_memory::{Memory, MemoryKind, MemoryState};
use super::agent_reflect::{Reply, Writer};

/// The slug of the proposal waiting for the owner.
pub const PROPOSAL_SLUG: &str = "mem/proposal/core";
/// The proposal value's schema.
pub const PROPOSAL_SCHEMA: &str = "openagents.agent-core-proposal.v1";
/// The memory row ID the proposal shows as at F2. Memory entry IDs start
/// at 1, so 0 names no entry.
pub const PROPOSAL_ID: u64 = 0;
/// How a consolidation's journal lines start.
pub const RUN_PREFIX: &str = "core proposal";
/// The most memory text one proposal call reads, bytes.
pub const PROMPT_MAX: usize = 32 * 1024;

/// Only one `core` write at a time in this process, so the hash check and
/// the write it guards don't interleave with another decision.
static CORE_WRITE: Mutex<()> = Mutex::new(());

/// A `core` proposal (`openagents.agent-core-proposal.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub schema: String,
    pub v: u32,
    /// The SHA-256 of the `core` profile it was written against
    /// (`agent_engrams::core_hash`).
    pub base: String,
    pub profile: String,
    /// Unix seconds.
    pub at: u64,
    /// The model that wrote it.
    #[serde(default)]
    pub model: String,
}

fn proposal_slug() -> Slug {
    Slug::parse(PROPOSAL_SLUG).expect("the proposal slug is a slug")
}

/// The proposal waiting in `engrams`, when there is one that reads.
#[must_use]
pub fn pending(engrams: &EngramStore) -> Option<Proposal> {
    let text = engrams.value(&proposal_slug())?;
    serde_json::from_str::<Proposal>(text)
        .ok()
        .filter(|p| p.schema == PROPOSAL_SCHEMA && p.v == 1)
}

/// Makes the model a proposal is written with.
pub type WriterFactory = Arc<dyn Fn(&Store) -> Result<Box<dyn Writer>, String> + Send + Sync>;

/// The agent's live model, or, in a unit test, a refusal: no model runs
/// there.
#[must_use]
pub fn default_factory() -> WriterFactory {
    if cfg!(test) {
        Arc::new(|_: &Store| Err("no consolidation model runs in a unit test".to_string()))
    } else {
        Arc::new(|_: &Store| {
            super::agent::LiveModel::new().map(|model| Box::new(model) as Box<dyn Writer>)
        })
    }
}

/// One memory a proposal may link: its slug and entry.
struct Linkable {
    slug: String,
    kind: MemoryKind,
    text: String,
    sources: Vec<String>,
}

/// The active entries and insights a proposal may link, by slug. Outcomes
/// and project notes stay in the stream; candidates and rejected entries
/// are never linked.
fn linkable(engrams: &EngramStore) -> Vec<Linkable> {
    engrams
        .entry_heads()
        .into_values()
        .filter_map(|(head, entry)| {
            let entry = entry?;
            (entry.state == MemoryState::Active
                && matches!(
                    entry.kind,
                    MemoryKind::Insight | MemoryKind::Preference | MemoryKind::Note
                ))
            .then(|| Linkable {
                slug: head.slug().as_str().to_string(),
                kind: entry.kind,
                text: entry.text,
                sources: entry.sources,
            })
        })
        .collect()
}

/// The proposal call's system text and prompt.
#[must_use]
fn prompt(agent: &str, charter: &str, core: &str, memories: &[Linkable]) -> (String, String) {
    let system = format!(
        "You are {agent}, the owner's workshop agent, consolidating your memory into your core \
         profile. Run no commands: finish on this step and put only the JSON asked for in your \
         reply."
    );
    let mut lines = String::new();
    for m in memories.iter().rev() {
        let cites = if m.sources.is_empty() {
            String::new()
        } else {
            format!(" (cites {})", m.sources.join(", "))
        };
        let line = format!(
            "[[{}]] ({}) {}{cites}\n",
            m.slug,
            m.kind.word(),
            m.text.replace('\n', " ")
        );
        if lines.len() + line.len() > PROMPT_MAX {
            break;
        }
        lines.push_str(&line);
    }
    let user = format!(
        "Your core profile now:\n---\n{core}\n---\nYour charter, which the profile must keep word \
         for word: {charter}\n\nYour active memories, each with its slug:\n{lines}\nWrite a new \
         core profile: keep who you are, your charter, and your rules; add the standing facts \
         these memories support, each followed by the [[slug]] of every memory it relies on, \
         using only slugs listed above. Keep it under {CORE_MAX} bytes. The owner reviews it \
         before it replaces your core. Reply with JSON: {{\"profile\": \"...\"}}"
    );
    (system, user)
}

/// The profile in a proposal call's reply.
///
/// # Errors
/// When the reply isn't the JSON asked for.
pub fn parse_profile(text: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Reply {
        profile: String,
    }
    let start = text.find('{').ok_or("the reply holds no JSON object")?;
    let end = text.rfind('}').ok_or("the reply holds no JSON object")?;
    if end < start {
        return Err("the reply holds no JSON object".into());
    }
    let reply: Reply = serde_json::from_str(&text[start..=end])
        .map_err(|e| format!("the reply isn't a profile: {e}"))?;
    Ok(super::agent::ascii(reply.profile.trim()))
}

/// Checks a proposed `profile` against her current `core`, her `charter`,
/// and the memories it may link: it differs from `core`, keeps the
/// charter, fits [`CORE_MAX`] as a `core` body, passes `screen`, and every
/// `[[slug]]` names a linkable memory.
///
/// # Errors
/// Why the proposal is dropped.
fn check(
    profile: &str,
    core: Option<&str>,
    charter: &str,
    may_link: &[Linkable],
    screen: &secret_screen::Screen,
) -> Result<(), String> {
    if profile.trim().is_empty() {
        return Err("it is empty".into());
    }
    if core == Some(profile) {
        return Err("it is her core unchanged".into());
    }
    if !profile.contains(charter.trim()) {
        return Err("it drops her charter".into());
    }
    if core_body(profile, "").to_json().len() > CORE_MAX {
        return Err(format!("it is over the {CORE_MAX}-byte core cap"));
    }
    if let Err(why) = screen.check(profile) {
        return Err(format!("the secret screen refused it: {why}"));
    }
    for link in nostr::engram::wiki_links(profile) {
        if !may_link.iter().any(|m| m.slug == link.as_str()) {
            return Err(format!(
                "it links [[{}]], which is no active entry or insight",
                link.as_str()
            ));
        }
    }
    Ok(())
}

/// The `core` body for `profile`, recording the `base` it replaced.
fn core_body(profile: &str, base: &str) -> Body {
    Body::Core {
        profile: profile.to_string(),
        extra: serde_json::Map::from_iter([
            ("schema".to_string(), CORE_SCHEMA.into()),
            ("v".to_string(), 1.into()),
            ("base".to_string(), base.into()),
        ]),
    }
}

/// What [`propose`] did.
#[derive(Clone, Debug, PartialEq)]
pub enum Proposed {
    /// A proposal waits for the owner.
    Waiting { links: usize, bytes: usize },
    /// Nothing to propose, with why.
    Skipped(String),
    /// The model's proposal failed a check, with why.
    Dropped(String),
}

fn journal(store: &Store, now: u64, text: &str) {
    let _ = store.append(&Entry::new(
        now,
        Kind::Memory,
        &format!("{RUN_PREFIX} {text}"),
    ));
}

fn ready(memory: &Memory, now: u64) -> Result<EngramStore, String> {
    match EngramStore::open(memory.store(), memory.screen(), now) {
        Opened::Ready(engrams) => Ok(engrams),
        Opened::Skipped(why) => Err(format!("she keeps no engrams: {why}")),
        Opened::Unreadable(why) => Err(format!(
            "her engram store cannot be read, so nothing is proposed or written: {why}"
        )),
    }
}

/// Runs one consolidation for `memory`'s agent with `writer` at `now`:
/// proposes a new `core` when an active insight is not yet reachable from
/// her `core` and no proposal is waiting. Every outcome is journaled, and
/// the reply's cost comes back with it.
///
/// # Errors
/// When the store cannot be read or she keeps no engrams (fail closed:
/// nothing is proposed), or no model answered.
pub fn propose(
    memory: &Memory,
    writer: &mut dyn Writer,
    now: u64,
) -> Result<(Proposed, Option<Reply>), String> {
    let store = memory.store();
    let mut engrams = ready(memory, now)?;
    let record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    if pending(&engrams).is_some() {
        let why = "one is already waiting for the owner".to_string();
        journal(store, now, &format!("skipped: {why}"));
        return Ok((Proposed::Skipped(why), None));
    }
    let may_link = linkable(&engrams);
    let reach = engrams.reach();
    let fresh = may_link
        .iter()
        .any(|m| m.kind == MemoryKind::Insight && !reach.reachable.contains(&m.slug));
    if !fresh {
        let why = "no active insight is new to her core".to_string();
        journal(store, now, &format!("skipped: {why}"));
        return Ok((Proposed::Skipped(why), None));
    }
    let core = engrams.core().map(str::to_string);
    let (system, user) = prompt(
        store.name(),
        &record.charter,
        core.as_deref().unwrap_or(""),
        &may_link,
    );
    let reply = writer.write(&system, &user)?;
    let checked = parse_profile(&reply.text).and_then(|profile| {
        check(
            &profile,
            core.as_deref(),
            &record.charter,
            &may_link,
            memory.screen(),
        )
        .map(|()| profile)
    });
    let profile = match checked {
        Ok(profile) => profile,
        Err(why) => {
            journal(store, now, &format!("dropped: {why}"));
            return Ok((Proposed::Dropped(why), Some(reply)));
        }
    };
    let proposal = Proposal {
        schema: PROPOSAL_SCHEMA.into(),
        v: 1,
        base: agent_engrams::core_hash(core.as_deref()),
        profile,
        at: now,
        model: reply.model.clone(),
    };
    let value = serde_json::to_string(&proposal).map_err(|e| e.to_string())?;
    let written = Body::memory(proposal_slug(), value)
        .and_then(|b| b.with_extra("schema", PROPOSAL_SCHEMA.into()))
        .and_then(|b| b.with_extra("v", 1.into()))
        .map_err(|e| e.to_string())
        .and_then(|body| engrams.put(body, now));
    if let Err(why) = written {
        journal(store, now, &format!("dropped: {why}"));
        return Ok((Proposed::Dropped(why), Some(reply)));
    }
    let links = nostr::engram::wiki_links(&proposal.profile).len();
    let bytes = proposal.profile.len();
    journal(
        store,
        now,
        &format!(
            "waiting for the owner: {bytes} bytes linking {links} memories, against core {}",
            short(&proposal.base)
        ),
    );
    Ok((Proposed::Waiting { links, bytes }, Some(reply)))
}

fn short(hash: &str) -> &str {
    &hash[..hash.len().min(12)]
}

/// Accepts or rejects the waiting proposal. Accepting writes `core` only
/// when her `core` still hashes to the proposal's `base`; otherwise the
/// write is refused, journaled as a conflict, and the stale proposal is
/// discarded. Either way a decided proposal is tombstoned. Returns the new
/// `core` hash on accept, or the discarded proposal's base on reject.
///
/// # Errors
/// The store cannot be read (nothing is written), no proposal waits, the
/// base changed (a conflict), or the write fails.
pub fn decide(memory: &Memory, accept: bool, now: u64) -> Result<String, String> {
    let store = memory.store();
    let _held = CORE_WRITE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut engrams = ready(memory, now)?;
    let proposal = pending(&engrams).ok_or("no core proposal is waiting")?;
    let discard = |engrams: &mut EngramStore| {
        Body::tombstone(proposal_slug())
            .and_then(|b| b.with_extra("schema", PROPOSAL_SCHEMA.into()))
            .and_then(|b| b.with_extra("v", 1.into()))
            .map_err(|e| e.to_string())
            .and_then(|body| engrams.put(body, now))
    };
    if !accept {
        discard(&mut engrams)?;
        journal(store, now, "rejected by the owner and discarded");
        return Ok(proposal.base);
    }
    let current = agent_engrams::core_hash(engrams.core());
    if current != proposal.base {
        let why = format!(
            "refused: her core changed since it was proposed (base {}, now {}); the proposal was \
             discarded",
            short(&proposal.base),
            short(&current)
        );
        let _ = discard(&mut engrams);
        journal(store, now, &why);
        return Err(format!("{RUN_PREFIX} {why}"));
    }
    engrams.put(core_body(&proposal.profile, &proposal.base), now)?;
    discard(&mut engrams)?;
    let hash = agent_engrams::core_hash(Some(&proposal.profile));
    journal(
        store,
        now,
        &format!("accepted by the owner: core is now {}", short(&hash)),
    );
    Ok(hash)
}

/// The memory rows F2 shows ahead of her entries, in the existing row
/// shape: the waiting proposal as row [`PROPOSAL_ID`] (`core`,
/// `candidate`), then a `reach` row with the orphan and dangling-reference
/// counts when there are any, or an `engrams` row when the store cannot be
/// read.
#[must_use]
pub fn rows(
    store: &Store,
    screen: &secret_screen::Screen,
) -> Vec<coder_host::access::agent::MemoryRow> {
    let row =
        |kind: &str, state: &str, text: String, at: u64| coder_host::access::agent::MemoryRow {
            id: PROPOSAL_ID,
            kind: kind.into(),
            state: state.into(),
            text,
            at,
        };
    let engrams = match EngramStore::read(store, screen) {
        Opened::Ready(engrams) => engrams,
        Opened::Skipped(_) => return Vec::new(),
        Opened::Unreadable(why) => {
            return vec![row(
                "engrams",
                "unreadable",
                format!(
                    "Her engram store cannot be read, so nothing is proposed or written: {why}"
                ),
                0,
            )];
        }
    };
    let mut out = Vec::new();
    if let Some(proposal) = pending(&engrams) {
        out.push(row(
            "core",
            "candidate",
            format!(
                "A new core profile, waiting for you: `accept 0` writes it, `reject 0` discards \
                 it. {}",
                proposal.profile.replace('\n', " / ")
            ),
            proposal.at,
        ));
    }
    let reach = engrams.reach();
    if !reach.orphans.is_empty() || !reach.dangling.is_empty() {
        let mut text = format!(
            "{} memories are orphans, not linked from her core; none is deleted",
            reach.orphans.len()
        );
        if !reach.dangling.is_empty() {
            text.push_str(&format!(
                "; {} links name a missing memory",
                reach.dangling.len()
            ));
        }
        text.push_str(&format!(
            ". List them with `openagents agent memory {} engrams --orphans`.",
            store.name()
        ));
        out.push(row("reach", "active", text, 0));
    }
    out
}

#[cfg(test)]
#[path = "agent_consolidate_tests.rs"]
mod tests;
