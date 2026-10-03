//! Quests, XP, and the quest board from the command line, read from NIP-XP
//! events (`nips/openagents/NIP-XP.md`) exactly as the desktop client reads
//! them: quests (`30193`), awards (`3193`), revocations (`3194`), and
//! achievement labels (`1985`), derived under the reader's trust list by
//! [`verse::xp::snapshot`]. Everything here is read-only.

use std::collections::BTreeMap;

use nostr::domain::Event;
use nostr::xp as kinds;
use serde_json::{Value, json};
use verse::xp::{QuestRow, Snapshot, level_of, load_trust, missing, my_keys, snapshot};

use crate::relay::{Client, DEFAULT_WAIT, identity_for, relay_url, unix_now};
use crate::{Args, Output, out};

/// Most events asked for per kind group, as the desktop client asks.
const LIMIT: usize = 500;

/// The relay the XP reader uses: `--xp-relay`, else `VERSE_XP_RELAY`, else
/// the world relay.
fn xp_relay(args: &Args) -> String {
    args.option("xp-relay")
        .map(str::to_owned)
        .or_else(|| std::env::var("VERSE_XP_RELAY").ok())
        .unwrap_or_else(|| relay_url(args.option("relay")))
}

/// What the reader derived, with where it read and who it trusted.
pub struct Reading {
    pub relay: String,
    pub snapshot: Snapshot,
    pub problem: Option<String>,
    /// The keys whose XP counts as this identity's.
    pub mine: Vec<String>,
}

/// Reads the relay once and derives the snapshot: gather the four kinds,
/// fetch the quest, entry, and evidence events the trusted awards name,
/// then derive.
pub fn read(args: &Args) -> Result<Reading, String> {
    let identity = identity_for(args.option("as"))?;
    let referees: Vec<String> = args
        .options("referee")
        .into_iter()
        .map(str::to_owned)
        .collect();
    let (trust, problem) = load_trust(&referees);
    let extra: Vec<String> = args
        .options("pubkey")
        .into_iter()
        .map(str::to_owned)
        .collect();
    let mine = my_keys(Some(identity.signer.pubkey()), &extra);
    let relay = xp_relay(args);
    let snapshot = read_under(&relay, &trust, &identity.signer)?;
    Ok(Reading {
        relay,
        snapshot,
        problem,
        mine,
    })
}

/// Reads `relay` once under `trust`: quests, awards, revocations, labels,
/// trainer profiles, and key links, then the events the trusted awards
/// name.
fn read_under(
    relay: &str,
    trust: &verse::xp::XpTrust,
    signer: &nostr::domain::RelaySigner,
) -> Result<Snapshot, String> {
    let trust = trust.clone();
    let mut client = Client::connect(relay, signer.clone());
    let mut events: BTreeMap<String, Event> = BTreeMap::new();
    client.subscribe(
        vec![
            json!({"kinds": [kinds::QUEST_KIND, kinds::AWARD_KIND, kinds::REVOCATION_KIND], "limit": LIMIT}),
            json!({"kinds": [kinds::LABEL_KIND], "#L": [kinds::LABEL_NAMESPACE], "limit": LIMIT}),
            json!({"kinds": [kinds::PROFILE_KIND, kinds::LINK_KIND], "limit": LIMIT}),
        ],
        false,
        DEFAULT_WAIT,
        |event| {
            if event.validate_id().is_ok() {
                events.entry(event.id.clone()).or_insert_with(|| event.clone());
            }
        },
    )?;
    let want: Vec<String> = missing(&events, &trust).into_iter().collect();
    for chunk in want.chunks(100) {
        client.subscribe(
            vec![json!({"ids": chunk, "limit": chunk.len()})],
            false,
            DEFAULT_WAIT,
            |event| {
                if event.validate_id().is_ok() {
                    events
                        .entry(event.id.clone())
                        .or_insert_with(|| event.clone());
                }
            },
        )?;
    }
    client.close();
    let all: Vec<Event> = events.into_values().collect();
    Ok(snapshot(&all, &trust))
}

/// Reads a trainer card: a file holding the signed `30194` event (`-` for
/// standard input), or an `naddr` fetched from its relay hint or `relay`.
fn load_card(
    source: &str,
    relay: Option<&str>,
    signer: &nostr::domain::RelaySigner,
) -> Result<Event, String> {
    if source.starts_with("naddr1") || source.starts_with("nostr:naddr1") {
        let naddr = nostr::nip19::decode_naddr(source.trim_start_matches("nostr:"))
            .map_err(|e| format!("{source} isn't an naddr: {e:?}"))?;
        if naddr.kind != u32::from(kinds::CARD_KIND) {
            return Err(format!(
                "{source} names kind {}, not a trainer card",
                naddr.kind
            ));
        }
        let author: String = naddr.pubkey.iter().map(|b| format!("{b:02x}")).collect();
        let url = relay
            .map(str::to_owned)
            .or_else(|| naddr.relays.first().cloned())
            .ok_or("the naddr has no relay hint; pass --xp-relay URL")?;
        let mut client = Client::connect(&url, signer.clone());
        let mut found: Vec<Event> = Vec::new();
        client.subscribe(
            vec![json!({"kinds": [kinds::CARD_KIND], "authors": [author], "#d": [naddr.identifier], "limit": 8})],
            false,
            DEFAULT_WAIT,
            |event| found.push(event.clone()),
        )?;
        client.close();
        let valid = found.iter().filter(|e| kinds::parse_card(e).is_ok());
        return kinds::trainer::newest(valid)
            .cloned()
            .ok_or(format!("{url} has no trainer card at {source}"));
    }
    let text = if source == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
            .map_err(|e| format!("can't read standard input: {e}"))?;
        text
    } else {
        std::fs::read_to_string(source).map_err(|e| format!("can't read {source}: {e}"))?
    };
    serde_json::from_str(&text).map_err(|e| format!("{source} isn't a signed Nostr event: {e}"))
}

/// `xp verify-card FILE|NADDR`: re-derives a trainer card's level from the
/// relays under the card's own trust list and reports every difference.
/// Exits 0 when the card matches and 1 when it doesn't.
pub fn verify_card(output: &Output, args: &Args) -> Result<u8, String> {
    let source = args
        .positional()
        .get(1)
        .ok_or("name the card: openagents xp verify-card <card.json | naddr1… | ->")?;
    let identity = identity_for(args.option("as"))?;
    let explicit = args
        .option("xp-relay")
        .map(str::to_owned)
        .or_else(|| args.option("relay").map(str::to_owned));
    let event = load_card(source, explicit.as_deref(), &identity.signer)?;
    let card = kinds::parse_card(&event).map_err(|e| format!("not a valid trainer card: {e}"))?;
    let relay = explicit.unwrap_or_else(|| card.relays[0].clone());
    let trust = verse::xp::XpTrust {
        referees: card.referees.iter().cloned().collect(),
        runners: card.runners.iter().cloned().collect(),
    };
    let snapshot = read_under(&relay, &trust, &identity.signer)?;
    let check = verse::xp::check_card(&card, &event.pubkey, &snapshot);
    let matches = check.differences.is_empty();
    output.emit(
        &json!({
            "card": event.id,
            "trainer": event.pubkey,
            "relay": relay,
            "curve": card.curve,
            "claimed": {"keys": card.keys, "xp": card.xp, "level": card.level, "awards": card.awards.len()},
            "derived": check,
            "matches": matches,
        }),
        |value| {
            let mut lines = vec![format!(
                "card {} by {}: signature valid; derived from {} under the card's {} referee{}",
                &event.id[..12],
                &event.pubkey[..12],
                value["relay"].as_str().unwrap_or_default(),
                card.referees.len(),
                if card.referees.len() == 1 { "" } else { "s" },
            )];
            lines.push(format!(
                "claimed: level {} ({}) with {} XP from {} awards over {} keys",
                card.level,
                card.curve,
                card.xp,
                card.awards.len(),
                card.keys.len()
            ));
            lines.push(format!(
                "derived: level {} with {} XP from {} awards over {} keys",
                check
                    .level
                    .map_or_else(|| "unknown".to_owned(), |l| l.to_string()),
                check.xp,
                check.awards,
                check.keys.len()
            ));
            if matches {
                lines.push("the card matches the relays".into());
            } else {
                lines.extend(check.differences.iter().map(|d| format!("difference: {d}")));
            }
            lines.join("\n")
        },
    );
    Ok(u8::from(!matches))
}

fn quest_value(row: &QuestRow) -> Value {
    json!({
        "address": row.address,
        "referee": row.referee,
        "trusted": row.trusted,
        "conflict": row.conflict,
        "title": row.title,
        "rule": row.rule,
        "task": row.task,
        "recipe": row.recipe,
        "claim": row.claim,
        "min_pass_rate": row.min_pass_rate,
        "max_usd_per_run": row.max_usd_per_run,
        "reference": row.reference.as_ref().map(|r| json!({
            "label": r.label,
            "usd": r.usd,
            "seconds": r.seconds,
            "source": r.source,
        })),
        "split": row.split.iter().map(|(role, xp)| json!({"role": role, "xp": xp})).collect::<Vec<_>>(),
        "total_xp": row.total(),
        "season": {
            "id": row.season.id,
            "opens_at": row.season.opens_at,
            "closes_at": row.season.closes_at,
        },
        "awards": row.awards,
        "counted": row.counted,
        "completions": row.completions,
        "max_awards": row.max_awards,
        "titles": row.titles,
    })
}

fn season_word(opens_at: u64, closes_at: u64, now: u64) -> &'static str {
    if now < opens_at {
        "upcoming"
    } else if now <= closes_at {
        "open"
    } else {
        "closed"
    }
}

/// Awards counted out of those found, and the quest's award limit: `0/0`
/// read as "none possible", so no awards and no limit say so in words.
fn awards_cell(row: &Value) -> String {
    let counted = row["counted"].as_u64().unwrap_or(0);
    let awards = row["awards"].as_u64().unwrap_or(0);
    let limit = match row["max_awards"].as_u64() {
        Some(max) => format!("max {max}"),
        None => "no limit".to_owned(),
    };
    if awards == 0 {
        format!("none yet ({limit})")
    } else {
        format!("{counted} of {awards} counted ({limit})")
    }
}

fn render_quests(value: &Value) -> String {
    let Some(rows) = value["quests"].as_array() else {
        return String::new();
    };
    if rows.is_empty() {
        return "no quests".into();
    }
    let now = unix_now();
    let mut table = vec![vec![
        "address".to_owned(),
        "xp".to_owned(),
        "season".to_owned(),
        "awards".to_owned(),
        "trust".to_owned(),
        "title".to_owned(),
    ]];
    for row in rows {
        let season = &row["season"];
        table.push(vec![
            row["address"].as_str().unwrap_or("").to_owned(),
            row["total_xp"].to_string(),
            format!(
                "{} {}",
                season["id"].as_str().unwrap_or(""),
                season_word(
                    season["opens_at"].as_u64().unwrap_or(0),
                    season["closes_at"].as_u64().unwrap_or(0),
                    now
                )
            ),
            awards_cell(row),
            match (
                row["trusted"].as_bool().unwrap_or(false),
                row["conflict"].as_bool().unwrap_or(false),
            ) {
                (_, true) => "conflict",
                (true, false) => "trusted",
                (false, false) => "untrusted",
            }
            .to_owned(),
            row["title"].as_str().unwrap_or("").to_owned(),
        ]);
    }
    out::table(&table)
}

/// `verse quests`: every quest version on the relay, trusted referees first.
pub fn quests(output: &Output, args: &Args) -> Result<u8, String> {
    let reading = read(args)?;
    let quests: Vec<Value> = reading.snapshot.quests.iter().map(quest_value).collect();
    output.emit(
        &json!({
            "relay": reading.relay,
            "referees": reading.snapshot.referees,
            "problem": reading.problem,
            "quests": quests,
        }),
        render_quests,
    );
    Ok(0)
}

fn ledger_value(snapshot: &Snapshot, keys: &[String]) -> Value {
    let total = snapshot.xp_of(keys);
    let level = level_of(total);
    json!({
        "keys": keys,
        "xp": total,
        "curve": verse::xp::CURVE,
        "level": level,
        "next_level_at": verse::xp::xp_to_reach(level + 1),
        "titles": snapshot.titles_of(keys),
        "per_key": keys.iter().map(|key| json!({
            "pubkey": key,
            "xp": snapshot.totals.get(key).copied().unwrap_or(0),
            "titles": snapshot.titles.get(key),
        })).collect::<Vec<_>>(),
    })
}

/// `verse xp [--pubkey KEY]...`: the ledger for this identity's keys, or for
/// the keys given.
pub fn xp(output: &Output, args: &Args) -> Result<u8, String> {
    if args.positional().first().map(String::as_str) == Some("verify-card") {
        return verify_card(output, args);
    }
    let reading = read(args)?;
    let keys: Vec<String> = if args.options("pubkey").is_empty() {
        reading.mine.clone()
    } else {
        args.options("pubkey")
            .into_iter()
            .filter_map(knowledge::remote::parse_author)
            .collect()
    };
    if keys.is_empty() {
        return Err("--pubkey takes an npub or a 64-hex key".into());
    }
    let mut value = ledger_value(&reading.snapshot, &keys);
    value["relay"] = json!(reading.relay);
    value["referees"] = json!(reading.snapshot.referees);
    value["problem"] = json!(reading.problem);
    output.emit(&value, |value| {
        let titles = value["titles"]
            .as_array()
            .map(|t| {
                t.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        format!(
            "level {} ({}) with {} XP (next at {}){}",
            value["level"],
            value["curve"].as_str().unwrap_or_default(),
            value["xp"],
            value["next_level_at"],
            if titles.is_empty() {
                String::new()
            } else {
                format!("; titles: {titles}")
            }
        )
    });
    Ok(0)
}

/// `verse board`: what the quest board in the plaza shows — the ledger's
/// counts, this identity's level, and the quests.
pub fn board(output: &Output, args: &Args) -> Result<u8, String> {
    let reading = read(args)?;
    let snapshot = &reading.snapshot;
    let mut standings: Vec<(String, u64)> = snapshot
        .totals
        .iter()
        .map(|(key, xp)| (key.clone(), *xp))
        .collect();
    standings.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let quests: Vec<Value> = snapshot.quests.iter().map(quest_value).collect();
    output.emit(
        &json!({
            "relay": reading.relay,
            "problem": reading.problem,
            "referees": snapshot.referees,
            "counted": snapshot.counted,
            "revoked": snapshot.revoked,
            "refused": snapshot.refused,
            "conflicts": snapshot.conflicts,
            "me": ledger_value(snapshot, &reading.mine),
            "curve": verse::xp::CURVE,
            "standings": standings.iter().map(|(key, xp)| json!({
                "pubkey": key,
                "xp": xp,
                "level": level_of(*xp),
                "titles": snapshot.titles.get(key),
            })).collect::<Vec<_>>(),
            "quests": quests,
        }),
        |value| {
            let mut lines = vec![format!(
                "board at {}: {} referees, {} awards counted, {} revoked, {} refused, {} conflicts",
                value["relay"].as_str().unwrap_or(""),
                value["referees"],
                value["counted"],
                value["revoked"],
                value["refused"],
                value["conflicts"],
            )];
            if let Some(problem) = value["problem"].as_str() {
                lines.push(format!("trust: {problem}"));
            }
            lines.push(format!(
                "me: level {} ({}) with {} XP",
                value["me"]["level"],
                value["me"]["curve"].as_str().unwrap_or_default(),
                value["me"]["xp"]
            ));
            if let Some(rows) = value["standings"].as_array() {
                for row in rows.iter().take(10) {
                    lines.push(format!(
                        "  {} level {} {} XP",
                        row["pubkey"]
                            .as_str()
                            .map(|key| &key[..key.len().min(8)])
                            .unwrap_or(""),
                        row["level"],
                        row["xp"]
                    ));
                }
            }
            lines.push(render_quests(value));
            lines.join("\n")
        },
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn season_word_follows_the_window() {
        assert_eq!(season_word(10, 20, 5), "upcoming");
        assert_eq!(season_word(10, 20, 10), "open");
        assert_eq!(season_word(10, 20, 20), "open");
        assert_eq!(season_word(10, 20, 21), "closed");
    }

    #[test]
    fn xp_relay_prefers_the_explicit_flag() {
        let args = Args::parse(
            &[
                "--relay".into(),
                "wss://a".into(),
                "--xp-relay".into(),
                "wss://b".into(),
            ],
            &[],
        )
        .unwrap();
        assert_eq!(xp_relay(&args), "wss://b");
    }

    #[test]
    fn ledger_value_reports_level_and_next_threshold() {
        let value = ledger_value(&Snapshot::default(), &["ab".repeat(32)]);
        assert_eq!(value["xp"], 0);
        assert_eq!(value["level"], 1);
        assert_eq!(value["curve"], "trainer-curve-v1");
        assert_eq!(value["next_level_at"], verse::xp::xp_to_reach(2));
    }
}

#[cfg(test)]
mod awards_cell_tests {
    use super::*;

    #[test]
    fn no_awards_and_no_limit_read_in_words() {
        let row = json!({"counted": 0, "awards": 0, "max_awards": null});
        assert_eq!(awards_cell(&row), "none yet (no limit)");
        let row = json!({"counted": 2, "awards": 3, "max_awards": 10});
        assert_eq!(awards_cell(&row), "2 of 3 counted (max 10)");
    }
}
