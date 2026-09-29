//! `microcoder xp`: quests, awards, revocations, and the XP ledger over a
//! Nostr relay (`nips/openagents/NIP-XP.md`).
//!
//! A referee publishes frozen quest versions, checks a completion against
//! the quest's rule before signing its award, and revokes an award it got
//! wrong. A reader derives XP from the awards of the referees it trusts,
//! re-checking each one. Events are built and checked by `nostr::xp`, the
//! ledger is `xp_ledger` (as `knowledge::xp`), and the relay connection is the one
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

pub mod eval;

/// The `xp` command's help.
pub const USAGE: &str = "usage: microcoder xp <command> --relay URL [options]

Refereeing (signed with the referee key):
  quest <file.json>        publish a frozen quest version (NIP-XP kind 30193);
                           a version is never rewritten, so raise it to change one
  award --quest ID@VERSION --evidence EVENT-ID [--record summary.json]
        [--label VALUE]...
                           check a completion against the quest's rule, then
                           publish its award (kind 3193) and any achievement
                           labels (NIP-32); a quest version pays once, or
                           each reproducer once under per-awardee. A
                           reproduce quest needs the reproducer's run record
                           (--record), which must match the digest the
                           reproduction names
  revoke <award-event-id> --reason TEXT
                           revoke one of your awards (kind 3194)

Playtesting (signed with the playtest referee key; see
docs/game/playtesting.md):
  playtest-keygen          create the playtest referee key, once, at --key or
                           ~/.openagents/nostr/playtest-referee-key (mode
                           0600); prints only its public key
  quest <file.json>        a quest whose rule is playtest is signed with the
                           playtest referee key
  award --quest ID@VERSION --evidence REPORT-ID --triager KEY
        [--issue OWNER/REPO#N] [--severity pN] [--commit SHA]
        [--session RECORD-ID] [--label VALUE]...
                           accept a tester's playtest report (kind 3197): the
                           issue records the acceptance; a moderated or group
                           session also names the moderator's record
  playtest-session --tester KEY --script NAME --format moderated|group
        --build BUILD
                           as a moderator (your knowledge key, or --key),
                           publish the record of a completed session (3196)

Run evidence (signed with your knowledge key, or --key):
  claim --record RUN [--benchmark NAME --benchmark-version V]
                           publish a graded run as a claim: run evidence
                           (kind 3189) whose recipe a reproduce quest can pin
  reproduce --quest ID@VERSION --referee KEY --record RUN
                           publish your rerun of a reproduce quest's claim as
                           a reproduction that cites it; then send the run's
                           summary.json to the referee
  link --trainer KEY | --unlink
                           link this key to your trainer key (NIP-XP 13195),
                           so readers sum its XP into the trainer's once the
                           trainer's profile lists it too (OpenAgents app:
                           Account > Trainer > Link a key); --unlink
                           withdraws it

Extension evaluation credit (docs/extensions/evaluation.md):
  referee [--quests DIR] [--documents DIR] [--queue FILE] [--state FILE]
                           one pass of the automated referee job: publish
                           the quest versions confirmed checks and
                           coder-defaults releases need, and sign each
                           eval-check and eval-adopt award the rules accept,
                           once per key; write the adoption queue. Signs
                           only with an existing referee key
  adopt [--subject RELEASE-ID] [--expires-days N] [--package-dir DIR]
                           list the tools that are candidates for Coder's
                           defaults (Better, confirmed by checks from three
                           distinct trainers), or adopt one: an operator's
                           decision, signed with the coder-defaults key
  defaults-keygen          create the coder-defaults key, once, at --key or
                           ~/.openagents/nostr/coder-defaults-key (mode 0600)

Reading:
  ledger [--referee KEY]... [--runner KEY]... [--json]
                           derive XP per public key from the awards of the
                           referees you trust, re-checking every award

Options:
  --relay URL   the relay; there's no default
  --key PATH    the signing key (default ~/.openagents/nostr/referee-key for
                refereeing, created on first use, and
                ~/.openagents/nostr/knowledge-key for run evidence)
  --record RUN  a Microcoder run directory or its summary.json

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
    pub record: Option<PathBuf>,
    pub benchmark: Option<String>,
    pub benchmark_version: Option<String>,
    /// `playtest`: the moderator's session record.
    pub session: Option<String>,
    /// `playtest`: the key that accepted the report.
    pub triager: Option<String>,
    pub issue: Option<String>,
    pub severity: Option<String>,
    pub commit: Option<String>,
    pub script: Option<String>,
    pub format: Option<String>,
    pub build: Option<String>,
    pub tester: Option<String>,
    /// `link`: the trainer key this key belongs to.
    pub trainer: Option<String>,
    /// `link`: withdraw this key's link.
    pub unlink: bool,
    /// `referee`: a directory of `ext-eval.*.json` quest templates
    /// instead of the built-in ones.
    pub quests: Option<PathBuf>,
    /// Where `coder-defaults` documents are kept by digest.
    pub documents: Option<PathBuf>,
    /// `referee`: where the adoption queue is written.
    pub queue: Option<PathBuf>,
    /// `referee`: where logged refusals are remembered.
    pub state: Option<PathBuf>,
    /// The `coder-defaults` root, instead of the package record's.
    pub defaults_root: Option<String>,
    /// `adopt`: the tool release to adopt.
    pub subject: Option<String>,
    /// `adopt`: days until the admission expires.
    pub expires_days: Option<u64>,
    /// `adopt`: the repository's `packages/coder-defaults` to keep the
    /// documents in, too.
    pub package_dir: Option<PathBuf>,
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
            "--record" => o.record = Some(PathBuf::from(value()?)),
            "--benchmark" => o.benchmark = Some(value()?),
            "--benchmark-version" => o.benchmark_version = Some(value()?),
            "--session" => o.session = Some(value()?),
            "--triager" => o.triager = Some(value()?),
            "--issue" => o.issue = Some(value()?),
            "--severity" => o.severity = Some(value()?),
            "--commit" => o.commit = Some(value()?),
            "--script" => o.script = Some(value()?),
            "--format" => o.format = Some(value()?),
            "--build" => o.build = Some(value()?),
            "--tester" => o.tester = Some(value()?),
            "--trainer" => o.trainer = Some(value()?),
            "--unlink" => o.unlink = true,
            "--quests" => o.quests = Some(PathBuf::from(value()?)),
            "--documents" => o.documents = Some(PathBuf::from(value()?)),
            "--queue" => o.queue = Some(PathBuf::from(value()?)),
            "--state" => o.state = Some(PathBuf::from(value()?)),
            "--defaults-root" => o.defaults_root = Some(value()?),
            "--subject" => o.subject = Some(value()?),
            "--expires-days" => {
                o.expires_days = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--expires-days needs a whole number".to_string())?,
                );
            }
            "--package-dir" => o.package_dir = Some(PathBuf::from(value()?)),
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

/// The playtest referee key: `--key`, or the default file. It is never
/// created here; `playtest-keygen` makes it.
fn playtest_key(o: &XpOptions) -> Result<PathBuf, String> {
    let key = o
        .key
        .clone()
        .or_else(ledger_xp::playtest_referee_key_file)
        .ok_or("HOME isn't set, so there's no key file: pass --key".to_string())?;
    if !key.exists() {
        return Err(format!(
            "there's no playtest referee key at {}; create it once with \
`microcoder xp playtest-keygen`",
            key.display()
        ));
    }
    Ok(key)
}

/// The key a quest file is signed with: the playtest referee key for a
/// `playtest` quest, the referee key otherwise.
fn quest_key(o: &XpOptions) -> Result<PathBuf, String> {
    let playtest = o
        .words
        .first()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|spec| spec.pointer("/acceptance/rule") == Some(&json!(xp::PLAYTEST)));
    if playtest {
        playtest_key(o)
    } else {
        referee_key(o)
    }
}

fn defaults_key(o: &XpOptions) -> Result<PathBuf, String> {
    o.key
        .clone()
        .or_else(eval::defaults_key_file)
        .ok_or("HOME isn't set, so there's no key file: pass --key".to_string())
}

fn trainer_key(o: &XpOptions) -> Result<PathBuf, String> {
    o.key
        .clone()
        .or_else(remote::key_file)
        .ok_or("HOME isn't set, so there's no key file: pass --key".to_string())
}

/// The bytes of a run record: `path` itself, or `path/summary.json` for a
/// run directory.
fn record_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = if path.is_dir() {
        path.join("summary.json")
    } else {
        path.to_path_buf()
    };
    std::fs::read(&file).map_err(|e| format!("can't read {}: {e}", file.display()))
}

/// Loads a signing key: 64 hex characters, or an `nsec`, such as the
/// trainer key the OpenAgents app reveals. A missing file is created as a
/// new hex key, as the referee key is.
fn load_key(key: &Path, role: &str) -> Result<Identity, String> {
    let identity = match std::fs::read_to_string(key) {
        Ok(text) if text.trim().starts_with("nsec1") => {
            let bytes = nostr::nip19::decode_nsec(text.trim())
                .map_err(|_| format!("{} isn't a valid nsec", key.display()))?;
            let secret = secp256k1::SecretKey::from_byte_array(bytes)
                .map_err(|_| format!("{} isn't a valid secret key", key.display()))?;
            Identity::from_secret(secret)?
        }
        _ => Identity::load_from(key)?,
    };
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
            "quest" => quest(&o, &quest_key(&o)?).await,
            "award" if o.triager.is_some() => award(&o, &playtest_key(&o)?).await,
            "award" => award(&o, &referee_key(&o)?).await,
            "playtest-keygen" => playtest_keygen(&o),
            "playtest-session" => playtest_session(&o, &trainer_key(&o)?).await,
            "revoke" => revoke(&o, &referee_key(&o)?).await,
            "claim" => claim(&o, &trainer_key(&o)?).await,
            "reproduce" => reproduce(&o, &trainer_key(&o)?).await,
            "link" => link(&o, &trainer_key(&o)?).await,
            "referee" => eval::referee(&o, &referee_key(&o)?).await,
            "adopt" => eval::adopt_command(&o, &defaults_key(&o)?).await,
            "defaults-keygen" => eval::defaults_keygen(&o),
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
    if parsed.acceptance.rule == xp::PLAYTEST {
        return playtest_award(o, &identity, &mut relay, &quest, &parsed, evidence_id).await;
    }
    let evidence = by_id(&mut relay, evidence_id, kb::EVIDENCE_KIND, "evidence").await?;
    let completion = if parsed.acceptance.rule == xp::REPRODUCE {
        let claim_id = &parsed.acceptance.claim.as_ref().ok_or("no claim")?.id;
        let claim = by_id(&mut relay, claim_id, kb::EVIDENCE_KIND, "claim").await?;
        let record = o.record.as_deref().ok_or(
            "a reproduce award needs the reproducer's run record: pass --record with the \
summary.json they sent",
        )?;
        if let Err(error) = check_record(&evidence, &record_bytes(record)?) {
            println!("{address}: not accepted: {error}");
            return Ok(1);
        }
        if let Err(error) = xp::check_reproduce(&parsed, &claim, &evidence) {
            println!("{address}: not accepted: {error}");
            return Ok(1);
        }
        Completion::Reproduce { claim }
    } else {
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
        Completion::Transfer { entry, excluded }
    };
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
    let live: Vec<(Event, xp::Award)> = existing
        .iter()
        .filter(|e| !revoked.contains(&e.id))
        .filter_map(|e| xp::parse_award(e).ok().map(|a| (e.clone(), a)))
        .collect();
    // Under `first` the key is the quest version; under `per-awardee` it
    // names the reproducer, so each distinct reproducer is paid once, up
    // to the quest's max_awards.
    let key = xp::uniqueness_key(&me, &parsed, &evidence.pubkey);
    if let Some((event, _)) = live.iter().find(|(_, award)| award.key == key) {
        let pays = if parsed.per_awardee() {
            "a per-awardee quest pays each reproducer once"
        } else {
            "a quest version pays once"
        };
        println!(
            "{address}: already awarded as {}; {pays}. Revoke that award first to replace it.",
            short(&event.id)
        );
        return Ok(1);
    }
    if let Some(limit) = parsed.max_awards
        && live.len() as u64 >= limit
    {
        println!(
            "{address}: {} live awards already, its max_awards; publish a new quest version \
to pay more",
            live.len()
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
    let parts = match &completion {
        Completion::Transfer { entry, excluded } => {
            xp::award(&quest, entry, &evidence, excluded, accepted_at)
        }
        Completion::Reproduce { claim } => {
            xp::reproduce_award(&quest, claim, &evidence, accepted_at)
        }
    }
    .map_err(|e| e.to_string())?;
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

/// `xp award` for a `playtest` quest: fetches the tester's report and any
/// session record, checks the rule and the fields the contribution needs,
/// refuses when the rule-derived key already has a live award or the quest
/// version has reached its `max_awards`, and publishes the award and any
/// labels.
async fn playtest_award(
    o: &XpOptions,
    identity: &Identity,
    relay: &mut Relay,
    quest: &Event,
    parsed: &xp::Quest,
    report_id: &str,
) -> Result<u8, String> {
    let address = &parsed.address;
    let me = identity.pubkey().to_string();
    let triager = o
        .triager
        .as_deref()
        .and_then(parse_author)
        .ok_or("name the key that accepted the report with --triager KEY")?;
    let report = by_id(
        relay,
        report_id,
        xp::playtest::REPORT_KIND,
        "playtest report",
    )
    .await?;
    let session = match o.session.as_deref() {
        Some(id) => Some(by_id(relay, id, xp::playtest::SESSION_KIND, "session record").await?),
        None => None,
    };
    let fields = xp::PlaytestAward {
        issue: o.issue.clone(),
        severity: o.severity.clone(),
        commit: o.commit.clone(),
    };
    let key = match xp::playtest::key(parsed, &report.pubkey, fields.issue.as_deref()) {
        Ok(key) => key,
        Err(error) => {
            println!("{address}: not accepted: {error}");
            return Ok(1);
        }
    };
    let existing = relay
        .query(json!({
            "kinds": [xp::AWARD_KIND, xp::REVOCATION_KIND], "authors": [&me], "limit": LIMIT,
        }))
        .await?;
    let revoked: BTreeSet<String> = existing
        .iter()
        .filter_map(|e| xp::parse_revocation(e).ok())
        .map(|r| r.award.id)
        .collect();
    let live: Vec<(Event, xp::Award)> = existing
        .iter()
        .filter(|e| !revoked.contains(&e.id))
        .filter_map(|e| xp::parse_award(e).ok().map(|a| (e.clone(), a)))
        .collect();
    if let Some((event, _)) = live.iter().find(|(_, a)| a.key == key) {
        println!(
            "{address}: {key} already has award {}; a contribution pays once. Revoke that \
award first to replace it.",
            short(&event.id)
        );
        return Ok(1);
    }
    let coordinate = xp::coordinate(&me, address);
    let max = parsed
        .acceptance
        .playtest
        .as_ref()
        .map_or(0, |p| p.max_awards);
    let used = live
        .iter()
        .filter(|(_, a)| a.coordinate == coordinate)
        .count() as u64;
    if used >= max {
        println!("{address}: already has {used} live awards, its max_awards of {max}");
        return Ok(1);
    }
    let accepted_at = existing
        .iter()
        .filter_map(|e| xp::parse_award(e).ok())
        .filter(|a| a.key == key)
        .map(|a| a.accepted_at + 1)
        .fold(now(), u64::max);
    let parts = match xp::playtest_award(
        quest,
        &report,
        session.as_ref(),
        &triager,
        &fields,
        accepted_at,
    ) {
        Ok(parts) => parts,
        Err(error) => {
            println!("{address}: not accepted: {error}");
            return Ok(1);
        }
    };
    let signed = sign(identity, parts);
    relay.publish(&signed).await?;
    println!(
        "{address}: award {} published for {}: tester {} {} XP",
        short(&signed.id),
        key,
        npub(&report.pubkey),
        parsed.award["tester"]
    );
    let mut refused = 0;
    for value in &o.labels {
        let parts = xp::achievement(&signed, value).map_err(|e| format!("label {value}: {e}"))?;
        let label = sign(identity, parts);
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

/// `xp playtest-keygen`: creates the playtest referee key, once, and
/// prints only its public key.
///
/// # Errors
///
/// When the file already exists or can't be written.
pub fn playtest_keygen(o: &XpOptions) -> Result<u8, String> {
    let key = o
        .key
        .clone()
        .or_else(ledger_xp::playtest_referee_key_file)
        .ok_or("HOME isn't set, so there's no key file: pass --key")?;
    if key.exists() {
        let pubkey = remote::own_pubkey(&key).unwrap_or_default();
        println!(
            "{} already holds a key ({}); it is never replaced here",
            key.display(),
            npub(&pubkey)
        );
        return Ok(1);
    }
    if let Some(dir) = key.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let identity = Identity::load_from(&key)?;
    println!(
        "playtest referee {}\npublic key (hex) {}\nsecret key in {} (mode 0600); keep it \
there and back it up offline. Never paste it anywhere.",
        npub(identity.pubkey()),
        identity.pubkey(),
        key.display()
    );
    Ok(0)
}

/// `xp playtest-session`: a moderator publishes the record of a completed
/// moderated or group session.
///
/// # Errors
///
/// A bad usage, key, or relay.
pub async fn playtest_session(o: &XpOptions, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let tester = o
        .tester
        .as_deref()
        .and_then(parse_author)
        .ok_or("name the tester with --tester KEY")?;
    let need = |value: &Option<String>, flag: &str| {
        value.clone().ok_or(format!("name the session's {flag}"))
    };
    let parts = xp::playtest_session(
        &need(&o.script, "--script")?,
        &need(&o.format, "--format")?,
        &need(&o.build, "--build")?,
        &tester,
        now(),
    )
    .map_err(|e| e.to_string())?;
    let identity = load_key(key, "moderating as")?;
    if identity.pubkey() == tester {
        return Err("a moderator can't record their own session".into());
    }
    let event = sign(&identity, parts);
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    relay.publish(&event).await?;
    println!("session record {} published", event.id);
    Ok(0)
}

/// What an award accepts, by the quest's rule.
enum Completion {
    Transfer { entry: Event, excluded: Vec<String> },
    Reproduce { claim: Event },
}

/// The referee's check of a reproduction's run record: `bytes` are the
/// exact file whose digest the reproduction names, and the extract it
/// carries is the one those bytes give.
///
/// # Errors
///
/// When the bytes or the extract differ from what the reproduction signed.
pub fn check_record(reproduction: &Event, bytes: &[u8]) -> Result<(), String> {
    let run = xp::parse_run_evidence(reproduction).map_err(|e| format!("the reproduction: {e}"))?;
    xp::check_run_record(&run.record, bytes)
        .map_err(|e| format!("the run record isn't the one the reproduction names: {e}"))
}

/// `xp claim`: publishes a graded run as run evidence, a claim whose
/// recipe a `reproduce` quest can pin.
///
/// # Errors
///
/// A bad usage, record, key, or relay.
pub async fn claim(o: &XpOptions, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let path = o
        .record
        .as_deref()
        .ok_or("name the run with --record RUN (its directory or summary.json)")?;
    let bytes = record_bytes(path)?;
    let recipe = xp::recipe_from_summary(
        &bytes,
        o.benchmark.as_deref().unwrap_or("terminal-bench"),
        o.benchmark_version.as_deref().unwrap_or("2.1"),
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let record = xp::record_from_summary(&bytes).map_err(|e| e.to_string())?;
    let identity = load_key(key, "signing as")?;
    let me = identity.pubkey().to_string();
    let parts = xp::run_evidence(&me, &me, &recipe, &record, &[]).map_err(|e| e.to_string())?;
    let event = sign(&identity, parts);
    let parsed = xp::parse_run_evidence(&event).map_err(|e| e.to_string())?;
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    relay.publish(&event).await?;
    println!(
        "claim {} published: {} on {} ({}), reward {}; recipe {}",
        event.id,
        parsed.recipe.task,
        parsed.recipe.model,
        parsed.verdict,
        parsed.record.reward,
        parsed.recipe_digest
    );
    Ok(u8::from(parsed.verdict != "pass"))
}

/// `xp reproduce`: publishes the trainer's rerun of a `reproduce` quest's
/// claim, after checking it against the quest's rule.
///
/// # Errors
///
/// A bad usage, record, key, or relay, or a quest the relay doesn't have.
pub async fn reproduce(o: &XpOptions, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let address = o
        .quest
        .as_deref()
        .ok_or("name the quest version with --quest ID@VERSION")?;
    let referee = o
        .referees
        .first()
        .and_then(|k| parse_author(k))
        .ok_or("name the quest's referee with --referee KEY (an npub or hex key)")?;
    let path = o
        .record
        .as_deref()
        .ok_or("name your run with --record RUN (its directory or summary.json)")?;
    let bytes = record_bytes(path)?;
    let identity = load_key(key, "signing as")?;
    let me = identity.pubkey().to_string();
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    let quests = my_quests(&mut relay, &referee, address).await?;
    let quest = match quests.as_slice() {
        [one] => one.clone(),
        [] => {
            return Err(format!(
                "{} has no quest {address} on this relay",
                npub(&referee)
            ));
        }
        _ => return Err(format!("{address} has conflicting versions on this relay")),
    };
    let parsed = xp::parse_quest(&quest).map_err(|e| e.to_string())?;
    let claim_pointer = parsed
        .acceptance
        .claim
        .as_ref()
        .ok_or(format!("{address} isn't a reproduce quest"))?;
    let claim = by_id(&mut relay, &claim_pointer.id, kb::EVIDENCE_KIND, "claim").await?;
    let claimed = xp::parse_run_evidence(&claim).map_err(|e| format!("the claim: {e}"))?;
    let record = xp::record_from_summary(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let parts = match xp::run_evidence(
        &me,
        &claim.pubkey,
        &claimed.recipe.to_value(),
        &record,
        std::slice::from_ref(&claim.id),
    ) {
        Ok(parts) => parts,
        Err(error) => {
            println!("{address}: not a reproduction: {error}");
            return Ok(1);
        }
    };
    let event = sign(&identity, parts);
    if let Err(error) = xp::check_reproduce(&parsed, &claim, &event) {
        println!("{address}: not a reproduction the quest accepts: {error}");
        return Ok(1);
    }
    relay.publish(&event).await?;
    println!(
        "{address}: reproduction {} published. Send {} to the referee, {}, with this event ID; \
it checks the file before it awards the quest.",
        event.id,
        path.display(),
        npub(&referee)
    );
    Ok(0)
}

/// `xp link`: publishes this key's link to `--trainer`, or withdraws it
/// with `--unlink`, and says whether the trainer's profile lists this key
/// yet: a link counts only when both keys signed it.
///
/// # Errors
///
/// A bad usage, key, or relay.
pub async fn link(o: &XpOptions, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let trainer = match (&o.trainer, o.unlink) {
        (Some(_), true) => return Err("pass --trainer KEY or --unlink, not both".into()),
        (None, false) => {
            return Err(
                "name your trainer key with --trainer KEY (an npub or hex key), \
or withdraw the link with --unlink"
                    .into(),
            );
        }
        (Some(text), false) => {
            Some(parse_author(text).ok_or(format!("--trainer {text} isn't an npub or a hex key"))?)
        }
        (None, true) => None,
    };
    let identity = load_key(key, "signing as")?;
    let me = identity.pubkey().to_string();
    let parts = xp::link(&me, trainer.as_deref()).map_err(|e| e.to_string())?;
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    // A replaceable event must be newer than the one it replaces.
    let existing = relay
        .query(json!({"kinds": [xp::LINK_KIND], "authors": [&me], "limit": LIMIT}))
        .await?;
    let created_at = existing
        .iter()
        .map(|e| e.created_at + 1)
        .fold(now(), u64::max);
    let event = identity
        .signer()
        .sign(created_at, parts.kind, parts.tags, parts.content);
    relay.publish(&event).await?;
    let Some(trainer) = trainer else {
        println!(
            "link withdrawn ({}); this key's XP counts only as its own",
            short(&event.id)
        );
        return Ok(0);
    };
    let profiles = relay
        .query(json!({"kinds": [xp::PROFILE_KIND], "authors": [&trainer], "limit": LIMIT}))
        .await?;
    let listed = xp::trainer::newest(profiles.iter().filter(|e| xp::parse_profile(e).is_ok()))
        .and_then(|e| xp::parse_profile(e).ok())
        .is_some_and(|p| p.keys.contains(&me));
    println!(
        "link to {} published ({}); {}",
        npub(&trainer),
        short(&event.id),
        if listed {
            "the trainer's profile lists this key, so readers now sum its XP into the trainer's"
        } else {
            "waiting for the trainer's profile to list this key: in the OpenAgents app, open \
Account > Trainer > Link a key and enter this key's npub"
        }
    );
    Ok(0)
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
        wanted.extend(award.entry.map(|e| e.id));
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
    let releases: Vec<Event> = events
        .iter()
        .filter(|e| e.kind == nostr::ext::RELEASE_KIND)
        .cloned()
        .collect();
    let documents = eval::documents_for(
        &mut relay,
        &releases,
        o.documents.clone().or_else(eval::documents_dir).as_deref(),
    )
    .await;
    let derived = ledger_xp::derive_with(&events, &documents, trust);
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
