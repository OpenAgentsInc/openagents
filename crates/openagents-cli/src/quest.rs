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
    let mut client = Client::connect(&relay, identity.signer.clone());
    let mut events: BTreeMap<String, Event> = BTreeMap::new();
    client.subscribe(
        vec![
            json!({"kinds": [kinds::QUEST_KIND, kinds::AWARD_KIND, kinds::REVOCATION_KIND], "limit": LIMIT}),
            json!({"kinds": [kinds::LABEL_KIND], "#L": [kinds::LABEL_NAMESPACE], "limit": LIMIT}),
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
    Ok(Reading {
        relay,
        snapshot: snapshot(&all, &trust),
        problem,
        mine,
    })
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
            format!("{}/{}", row["counted"], row["awards"]),
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
