//! `microcoder xp`: quests, awards, revocations, and the XP ledger over a
//! Nostr relay (`nips/openagents/NIP-XP.md`).
//!
//! A referee publishes frozen quest versions, checks a completion against
//! the quest's rule before signing its award, and revokes an award it got
//! wrong. A reader derives XP from the awards of the referees it trusts,
//! re-checking each one. Events are built and checked by `nostr::xp`, the
//! ledger is `knowledge::xp`, and the relay connection is the one
//! `kb publish` and `kb sync` use. Quests, awards, and revocations are
//! signed with `~/.openagents/nostr/referee-key`, created on first use with
//! mode 0600; the key is never printed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use coder::relay::Identity;
use knowledge::remote::{self, npub, parse_author};
use knowledge::xp::{self as ledger_xp, Ledger, XpTrust};
use nostr::domain::Event;
use nostr::{kb, xp};
use serde_json::{Value, json};

use crate::kbnet::{LIMIT, Relay};

/// The `xp` command's help.
pub const USAGE: &str = "usage: microcoder xp <command> --relay URL [options]

Refereeing (signed with the referee key):
  quest <file.json>        publish a frozen quest version (NIP-XP kind 30193);
                           a version is never rewritten, so raise it to change one
  award --quest ID@VERSION --evidence EVENT-ID [--label VALUE]...
                           check a completion against the quest's rule, then
                           publish its award (kind 3193) and any achievement
                           labels (NIP-32); a quest version pays once
  revoke <award-event-id> --reason TEXT
                           revoke one of your awards (kind 3194)

Reading:
  ledger [--referee KEY]... [--runner KEY]... [--json]
                           derive XP per public key from the awards of the
                           referees you trust, re-checking every award

Options:
  --relay URL   the relay; there's no default
  --key PATH    the referee key (default ~/.openagents/nostr/referee-key,
                created on first use)

The ledger trusts the referees in ~/.openagents/knowledge/xp-trust.json
({\"referees\": [\"npub1...\"], \"runners\": [\"npub1...\"]}), those named with
--referee, and your own referee key. With runners listed, only evidence from
those runners counts. XP is never spent, transferred, or converted.";

/// Every option `xp` takes.
#[derive(Debug, Default)]
pub struct XpOptions {
    pub relay: Option<String>,
    pub key: Option<PathBuf>,
    pub quest: Option<String>,
    pub evidence: Option<String>,
    pub labels: Vec<String>,
    pub reason: Option<String>,
    pub referees: Vec<String>,
    pub runners: Vec<String>,
    pub json: bool,
    /// Words that aren't options, in order.
    pub words: Vec<String>,
}

/// Parses the options after the command.
///
/// # Errors
///
/// An unknown option or one without its value.
pub fn parse(args: &[String]) -> Result<XpOptions, String> {
    let mut o = XpOptions::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = || rest.next().cloned().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--relay" => o.relay = Some(value()?),
            "--key" => o.key = Some(PathBuf::from(value()?)),
            "--quest" => o.quest = Some(value()?),
            "--evidence" => o.evidence = Some(value()?),
            "--label" => o.labels.push(value()?),
            "--reason" => o.reason = Some(value()?),
            "--referee" => o.referees.push(value()?),
            "--runner" => o.runners.push(value()?),
            "--json" => o.json = true,
            "-h" | "--help" => return Err(USAGE.to_string()),
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            word => o.words.push(word.to_string()),
        }
    }
    Ok(o)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn short(hex: &str) -> &str {
    &hex[..hex.len().min(12)]
}

fn sign(identity: &Identity, parts: kb::Unsigned) -> Event {
    identity
        .signer()
        .sign(now(), parts.kind, parts.tags, parts.content)
}

fn relay_url(o: &XpOptions) -> Result<&str, String> {
    o.relay.as_deref().ok_or(
        "name the relay with --relay, such as --relay ws://127.0.0.1:7447; there's no default \
relay"
            .to_string(),
    )
}

fn referee_key(o: &XpOptions) -> Result<PathBuf, String> {
    o.key
        .clone()
        .or_else(ledger_xp::referee_key_file)
        .ok_or("HOME isn't set, so there's no key file: pass --key".to_string())
}

fn load_key(key: &Path, role: &str) -> Result<Identity, String> {
    let identity = Identity::load_from(key)?;
    println!(
        "{role} {} (key in {})",
        npub(identity.pubkey()),
        key.display()
    );
    Ok(identity)
}

/// The reader's trust: the trust file, `--referee` and `--runner`, and the
/// reader's own referee key when it exists.
///
/// # Errors
///
/// A bad trust file or key.
pub fn trust_for(o: &XpOptions) -> Result<XpTrust, String> {
    let mut trust = match ledger_xp::trust_file() {
        Some(path) => XpTrust::read(&path)?,
        None => XpTrust::default(),
    };
    for (keys, set) in [
        (&o.referees, &mut trust.referees),
        (&o.runners, &mut trust.runners),
    ] {
        for key in keys {
            set.insert(
                parse_author(key).ok_or(format!("{key} isn't an npub or a hex public key"))?,
            );
        }
    }
    if let Some(own) = referee_key(o).ok().and_then(|k| remote::own_pubkey(&k)) {
        trust.referees.insert(own);
    }
    Ok(trust)
}

/// Runs `xp <command>` with `args`, the command first. Returns the exit
/// code: 0 on success, 1 when something was refused, 2 on bad usage or a
/// relay that can't be reached.
pub async fn main(args: &[String]) -> u8 {
    let result = async {
        let command = args.first().ok_or(USAGE)?;
        let o = parse(&args[1..])?;
        match command.as_str() {
            "quest" => quest(&o, &referee_key(&o)?).await,
            "award" => award(&o, &referee_key(&o)?).await,
            "revoke" => revoke(&o, &referee_key(&o)?).await,
            "ledger" => {
                let key = remote::key_file().ok_or("HOME isn't set, so there's no key file")?;
                ledger(&o, &key, &trust_for(&o)?).await
            }
            "-h" | "--help" | "help" => Err(USAGE.to_string()),
            other => Err(format!("unknown command {other}\n\n{USAGE}")),
        }
    }
    .await;
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            2
        }
    }
}

/// This referee's quests at `address`.
async fn my_quests(relay: &mut Relay, me: &str, address: &str) -> Result<Vec<Event>, String> {
    let events = relay
        .query(json!({"kinds": [xp::QUEST_KIND], "authors": [me], "#d": [address], "limit": LIMIT}))
        .await?;
    Ok(events
        .into_iter()
        .filter(|e| xp::parse_quest(e).is_ok())
        .collect())
}

/// `xp quest`: signs the quest spec in the file named by the first word
/// and publishes it, unless its address already holds this version.
///
/// # Errors
///
/// A bad usage, file, key, or relay.
pub async fn quest(o: &XpOptions, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let [file] = o.words.as_slice() else {
        return Err("name one quest file: microcoder xp quest <file.json> --relay URL".into());
    };
    let text = std::fs::read_to_string(file).map_err(|e| format!("can't read {file}: {e}"))?;
    let spec: Value = serde_json::from_str(&text).map_err(|e| format!("{file}: {e}"))?;
    let parts = xp::quest(&spec).map_err(|e| format!("{file}: {e}"))?;
    let identity = load_key(key, "signing as")?;
    let event = sign(&identity, parts);
    let parsed = xp::parse_quest(&event).map_err(|e| e.to_string())?;
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    let existing = my_quests(&mut relay, identity.pubkey(), &parsed.address).await?;
    if let Some(existing) = existing.first() {
        if xp::parse_quest(existing).ok().as_ref() == Some(&parsed) {
            println!(
                "{}: already published as {}",
                parsed.address,
                short(&existing.id)
            );
            return Ok(0);
        }
        println!(
            "{}: this version is already published with other content ({}); a quest version is \
frozen, so raise its version and publish again",
            parsed.address,
            short(&existing.id)
        );
        return Ok(1);
    }
    relay.publish(&event).await?;
    println!(
        "{}: quest {} published; {} XP ({}), season {}",
        parsed.address,
        short(&event.id),
        parsed.total(),
        parsed
            .award
            .iter()
            .map(|(role, xp)| format!("{role} {xp}"))
            .collect::<Vec<_>>()
            .join(", "),
        parsed.season.id
    );
    Ok(0)
}

async fn by_id(relay: &mut Relay, id: &str, kind: u16, what: &str) -> Result<Event, String> {
    let events = relay.query(json!({"ids": [id], "limit": 1})).await?;
    events
        .into_iter()
        .find(|e| e.id == id && e.kind == kind)
        .ok_or(format!("the relay has no {what} {}", short(id)))
}

/// `xp award`: fetches the quest, the evidence, and the entry the evidence
/// is about; checks the quest's rule; refuses when this quest version
/// already has a live award; and publishes the award and any labels.
///
/// # Errors
///
/// A bad usage, key, or relay, or events the relay doesn't have.
pub async fn award(o: &XpOptions, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let address = o
        .quest
        .as_deref()
        .ok_or("name the quest version with --quest ID@VERSION")?;
    let evidence_id = o
        .evidence
        .as_deref()
        .ok_or("name the evidence with --evidence EVENT-ID (a kind 3189 event)")?;
    let identity = load_key(key, "signing as")?;
    let me = identity.pubkey().to_string();
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    let quests = my_quests(&mut relay, &me, address).await?;
    let quest = match quests.as_slice() {
        [one] => one.clone(),
        [] => return Err(format!("you have no quest {address} on this relay")),
        _ => return Err(format!("{address} has conflicting versions on this relay")),
    };
    let parsed = xp::parse_quest(&quest).map_err(|e| e.to_string())?;
    let evidence = by_id(&mut relay, evidence_id, kb::EVIDENCE_KIND, "evidence").await?;
    let published = kb::parse_evidence(&evidence).map_err(|e| format!("the evidence: {e}"))?;
    let subject = published
        .subject
        .event
        .ok_or("the evidence names no entry event")?;
    let entry = by_id(&mut relay, &subject.id, kb::ENTRY_KIND, "entry").await?;
    let excluded = ledger_xp::excluded_tasks(&entry)?;
    if let Err(error) = xp::check_transfer(&parsed, &entry, &evidence, &excluded) {
        println!("{address}: not accepted: {error}");
        return Ok(1);
    }
    let coordinate = xp::coordinate(&me, &parsed.address);
    let existing = relay
        .query(json!({
            "kinds": [xp::AWARD_KIND, xp::REVOCATION_KIND], "authors": [&me],
            "#a": [&coordinate], "limit": LIMIT,
        }))
        .await?;
    let revoked: BTreeSet<String> = existing
        .iter()
        .filter_map(|e| xp::parse_revocation(e).ok())
        .map(|r| r.award.id)
        .collect();
    if let Some(live) = existing
        .iter()
        .find(|e| xp::parse_award(e).is_ok() && !revoked.contains(&e.id))
    {
        println!(
            "{address}: already awarded as {}; a quest version pays once. Revoke that award \
first to replace it.",
            short(&live.id)
        );
        return Ok(1);
    }
    // A replacement is later than every award it replaces, so it never
    // shares an event ID with a revoked one.
    let accepted_at = existing
        .iter()
        .filter_map(|e| xp::parse_award(e).ok())
        .map(|a| a.accepted_at + 1)
        .fold(now(), u64::max);
    let parts =
        xp::award(&quest, &entry, &evidence, &excluded, accepted_at).map_err(|e| e.to_string())?;
    let signed = sign(&identity, parts);
    relay.publish(&signed).await?;
    let parsed_award = xp::parse_award(&signed).map_err(|e| e.to_string())?;
    println!(
        "{address}: award {} published: {}",
        short(&signed.id),
        parsed_award
            .awardees
            .iter()
            .map(|a| format!("{} {} {} XP", a.role, npub(&a.pubkey), a.xp))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut refused = 0;
    for value in &o.labels {
        let parts = xp::achievement(&signed, value).map_err(|e| format!("label {value}: {e}"))?;
        let label = sign(&identity, parts);
        match relay.publish(&label).await {
            Ok(()) => println!("label {value}: {} published", short(&label.id)),
            Err(error) => {
                refused += 1;
                println!("label {value}: {error}");
            }
        }
    }
    Ok(u8::from(refused > 0))
}

/// `xp revoke`: revokes one of this referee's awards.
///
/// # Errors
///
/// A bad usage, key, or relay, or an award that isn't this referee's.
pub async fn revoke(o: &XpOptions, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let [id] = o.words.as_slice() else {
        return Err("name one award: microcoder xp revoke <award-event-id> --reason TEXT".into());
    };
    let reason = o
        .reason
        .as_deref()
        .ok_or("say why with --reason TEXT; readers see it")?;
    let identity = load_key(key, "signing as")?;
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    let award = by_id(&mut relay, id, xp::AWARD_KIND, "award").await?;
    if award.pubkey != identity.pubkey() {
        return Err(format!(
            "award {} was signed by {}; only its referee revokes it",
            short(id),
            npub(&award.pubkey)
        ));
    }
    let parts = xp::revocation(&award, reason).map_err(|e| e.to_string())?;
    let signed = sign(&identity, parts);
    relay.publish(&signed).await?;
    println!(
        "award {}: revocation {} published",
        short(id),
        short(&signed.id)
    );
    Ok(0)
}

/// `xp ledger`: fetches the trusted referees' quests, awards, and
/// revocations, and the entries and evidence the awards name, then
/// derives and prints XP per public key. `key` answers the relay's
/// authentication challenge.
///
/// # Errors
///
/// A bad usage, key, or relay.
pub async fn ledger(o: &XpOptions, key: &Path, trust: &XpTrust) -> Result<u8, String> {
    let url = relay_url(o)?;
    if trust.referees.is_empty() {
        return Err(
            "you trust no referee: name one with --referee KEY or list them in \
~/.openagents/knowledge/xp-trust.json"
                .to_string(),
        );
    }
    let identity = if o.json {
        Identity::load_from(key)?
    } else {
        load_key(key, "authenticating to the relay as")?
    };
    let mut relay = Relay::open(url, &identity).await?;
    let referees: Vec<&String> = trust.referees.iter().collect();
    let mut events = relay
        .query(json!({
            "kinds": [xp::QUEST_KIND, xp::AWARD_KIND, xp::REVOCATION_KIND],
            "authors": referees, "limit": LIMIT,
        }))
        .await?;
    let have: BTreeSet<String> = events.iter().map(|e| e.id.clone()).collect();
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    for award in events.iter().filter_map(|e| xp::parse_award(e).ok()) {
        wanted.insert(award.quest.id);
        wanted.insert(award.entry.id);
        wanted.extend(award.evidence.into_iter().map(|e| e.id));
    }
    let missing: Vec<String> = wanted.difference(&have).cloned().collect();
    for chunk in missing.chunks(LIMIT) {
        events.extend(
            relay
                .query(json!({"ids": chunk, "limit": chunk.len()}))
                .await?,
        );
    }
    let derived = ledger_xp::derive(&events, trust);
    if o.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&derived).map_err(|e| e.to_string())?
        );
    } else {
        print_ledger(url, trust, &derived);
    }
    Ok(u8::from(
        !derived.refused.is_empty() || !derived.conflicts.is_empty(),
    ))
}

fn print_ledger(url: &str, trust: &XpTrust, ledger: &Ledger) {
    let referees = trust.referees.len();
    println!(
        "connected to {url}; trusting {referees} {}{}",
        if referees == 1 { "referee" } else { "referees" },
        if trust.runners.is_empty() {
            String::new()
        } else {
            format!(" and {} listed runners", trust.runners.len())
        }
    );
    let counted: BTreeSet<&str> = ledger.credits.iter().map(|c| c.award.as_str()).collect();
    println!(
        "{} quests; {} awards counted, {} revoked, {} refused, {} conflicts, {} from untrusted \
referees",
        ledger.quests.len(),
        counted.len(),
        ledger.revoked.len(),
        ledger.refused.len(),
        ledger.conflicts.len(),
        ledger.untrusted
    );
    let mut totals: Vec<(&String, &u64)> = ledger.totals.iter().collect();
    totals.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (pubkey, xp) in totals {
        println!("{:>6} XP  {}", xp, npub(pubkey));
    }
    for award in counted {
        let credits: Vec<_> = ledger.credits.iter().filter(|c| c.award == award).collect();
        let first = credits[0];
        println!(
            "- {} \"{}\" ({}), award {}: {}",
            first.quest,
            first.title,
            first.season,
            short(award),
            credits
                .iter()
                .map(|c| format!("{} {} {}", c.role, npub(&c.pubkey), c.xp))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for why in &ledger.refused {
        println!("- refused {why}");
    }
    for why in &ledger.conflicts {
        println!("- conflict {why}");
    }
}

#[cfg(test)]
mod tests;
