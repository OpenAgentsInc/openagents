//! Spend records on scratch homes and file keys: no relay, no model, and
//! no real money.

use super::*;
use crate::task::agent_steer::{
    self, Hands, Judgment, Mind, Move, Plan, Policy, ScriptedJudge, ScriptedPlanner, Step, TurnEnd,
    Turned,
};
use serde_json::Value;

const OWNER: [u8; 32] = [7; 32];
const STRANGER: [u8; 32] = [9; 32];
/// Noon UTC, so an hour either way stays on the same day.
const NOW: u64 = 1_800_000_000 - 1_800_000_000 % DAY + DAY / 2;

fn owner() -> SecretKey {
    SecretKey::from_byte_array(OWNER).unwrap()
}

fn owner_public() -> XOnlyPublicKey {
    agent::public_hex(&owner()).parse().unwrap()
}

/// Alice under a scratch root, with her own file key, attested by
/// [`OWNER`].
fn alice(dir: &tempfile::TempDir) -> (Store, Record) {
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let record = store.open(dir.path(), 1).unwrap();
    let record = store.ensure_key(record, 1).unwrap();
    let record = store
        .attest(record, &owner(), NOW + 300 * DAY, NOW - DAY)
        .unwrap();
    (store, record)
}

fn budget(store: &Store, edit: impl FnOnce(&mut Budget)) {
    let mut budget = Budget::default();
    edit(&mut budget);
    budget.save(store).unwrap();
}

fn plan_call(input: u64, output: u64, usd: f64) -> Call {
    Call {
        harness: HARNESS,
        turn_id: "plan".into(),
        model: Some("gpt-test".into()),
        usage: Counters {
            input_tokens: Some(input),
            output_tokens: Some(output),
            total_tokens: None,
            cost_usd: Some(usd),
        },
        stop: "end_turn",
    }
}

fn coder_call(n: u32, tokens: u64) -> Call {
    Call {
        harness: CODER_HARNESS,
        turn_id: format!("coder-{n}"),
        model: Some("coder-model".into()),
        usage: Counters {
            total_tokens: Some(tokens),
            ..Counters::default()
        },
        stop: "end_turn",
    }
}

fn metric() -> Metric {
    Metric {
        harness: HARNESS.into(),
        model: Some("gpt-test".into()),
        channel_id: None,
        session_id: Some("alice-1".into()),
        turn_id: Some("plan".into()),
        turn_seq: Some(1),
        timestamp: crate::relay::usage::rfc3339(NOW * 1_000),
        turn: Counters {
            input_tokens: Some(1_234),
            output_tokens: Some(567),
            total_tokens: None,
            cost_usd: Some(0.0123),
        },
        cumulative: Some(Counters {
            input_tokens: Some(1_234),
            output_tokens: Some(567),
            total_tokens: None,
            cost_usd: Some(0.0123),
        }),
        delta_reliable: true,
        stop_reason: Some("end_turn".into()),
    }
}

#[test]
fn the_envelope_passes_the_relay_check_and_only_the_owner_reads_it() {
    let agent_secret = SecretKey::from_byte_array([3; 32]).unwrap();
    let event = seal(&metric(), &agent_secret, &owner_public(), NOW).unwrap();

    // The relay's NIP-AM gate: kind, one p (the owner), one agent (the
    // author), no h, and NIP-44 v2 content.
    assert_eq!(event.kind, 44_200);
    assert_eq!(
        agent_turn_metric_owner(&event).unwrap(),
        agent::public_hex(&owner())
    );
    let names: Vec<&str> = event.tags.iter().filter_map(Tag::name).collect();
    assert_eq!(names, ["p", "agent"]);
    assert_eq!(event.tags[1].value(), Some(event.pubkey.as_str()));
    event.validate_crypto().unwrap();

    // The owner decrypts with the owner key alone.
    let read = open_as_owner(&event, &owner()).unwrap();
    assert_eq!(read, metric());
    // The payload is NIP-AM's: camelCase, unknown counters as null, no
    // cache fields, and nothing but numbers and identifiers.
    let key = nip44::conversation_key(&owner(), &event.pubkey.parse().unwrap());
    let raw: Value = serde_json::from_str(&nip44::decrypt(&event.content, &key).unwrap()).unwrap();
    assert_eq!(raw["turn"]["totalTokens"], Value::Null);
    assert_eq!(raw["turn"]["inputTokens"], 1_234);
    assert_eq!(raw["channelId"], Value::Null);
    assert_eq!(raw["sessionId"], "alice-1");
    assert_eq!(raw["turnSeq"], 1);
    assert!(raw["turn"].get("cacheReadTokens").is_none());
    assert!(raw.get("prompt").is_none() && raw.get("reply").is_none());

    // She reads it with her key; a stranger reads nothing.
    open_as_agent(&event, &agent_secret, &owner_public()).unwrap();
    let stranger = SecretKey::from_byte_array(STRANGER).unwrap();
    assert!(open_as_owner(&event, &stranger).is_err());

    // A tampered envelope fails the relay's check before any decryption.
    let mut tampered = event.clone();
    tampered
        .tags
        .push(Tag::new(vec!["h".into(), "room".into()]));
    assert!(agent_turn_metric_owner(&tampered).is_err());
    assert!(open_as_owner(&tampered, &owner()).is_err());
}

#[test]
fn a_metric_that_breaks_a_rule_is_never_sealed() {
    let agent_secret = SecretKey::from_byte_array([3; 32]).unwrap();
    let mut bad = metric();
    bad.turn.cost_usd = Some(-1.0);
    assert!(seal(&bad, &agent_secret, &owner_public(), NOW).is_err());
    let mut bad = metric();
    bad.session_id = None;
    assert!(seal(&bad, &agent_secret, &owner_public(), NOW).is_err());
    // A reader ignores fields it doesn't know.
    let text = r#"{"harness":"h","timestamp":"t","turn":{"inputTokens":1},"future":1}"#;
    let read: Metric = serde_json::from_str(text).unwrap();
    assert_eq!(read.turn.input_tokens, Some(1));
    assert!(read.delta_reliable);
}

#[test]
fn each_call_is_one_ordered_record_the_owner_decrypts() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = alice(&dir);
    let mut meter = Meter::open(&store, &record, NOW).unwrap();
    assert_eq!(meter.unsealed, None);
    assert_eq!(meter.record(&plan_call(100, 20, 0.01), NOW).unwrap(), None);
    // No observed usage: no record.
    let mut silent = coder_call(1, 0);
    silent.usage.total_tokens = None;
    assert_eq!(meter.record(&silent, NOW + 1).unwrap(), None);
    assert_eq!(meter.record(&coder_call(2, 500), NOW + 2).unwrap(), None);

    let view = owner_read(&store, &owner()).unwrap();
    assert!(view.problems.is_empty(), "{:?}", view.problems);
    assert_eq!(view.records.len(), 2);
    let (first, second) = (&view.records[0].metric, &view.records[1].metric);
    assert_eq!(first.session_id.as_deref(), Some(meter.session()));
    assert_eq!(second.session_id, first.session_id);
    assert_eq!((first.turn_seq, second.turn_seq), (Some(1), Some(2)));
    assert_eq!(first.harness, HARNESS);
    assert_eq!(second.harness, CODER_HARNESS);
    assert_eq!(second.turn_id.as_deref(), Some("coder-2"));
    // Cumulative within the request: a counter one turn didn't report is
    // unknown from then on, never a zero.
    let cumulative = second.cumulative.unwrap();
    assert_eq!(cumulative.input_tokens, None);
    assert_eq!(cumulative.cost_usd, None);
    assert_eq!(first.cumulative.unwrap().cost_usd, Some(0.01));
    // Her own key reads the same records; every line is ciphertext.
    assert_eq!(agent_read(&store, &record).unwrap().records.len(), 2);
    let text = std::fs::read_to_string(store.dir().join(LEDGER_FILE)).unwrap();
    assert!(!text.contains("gpt-test") && !text.contains("coder-2"));
    let tally = view.tally(day_of(NOW));
    assert_eq!((tally.records, tally.tokens, tally.unpriced), (2, 620, 1));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(store.dir().join(LEDGER_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn the_request_budget_stops_her_and_the_days_records_refuse_the_next() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = alice(&dir);
    budget(&store, |b| {
        b.request_tokens = 1_000;
        b.daily_usd = 0.05;
    });
    let mut meter = Meter::open(&store, &record, NOW).unwrap();
    meter.admit().unwrap();
    assert_eq!(meter.record(&coder_call(1, 600), NOW).unwrap(), None);
    assert_eq!(
        meter
            .record(&coder_call(2, 600), NOW + 1)
            .unwrap()
            .as_deref(),
        Some("request budget of 1000 tokens")
    );

    // A new request reads today's records from the ledger.
    let mut meter = Meter::open(&store, &record, NOW + 10).unwrap();
    assert_eq!(meter.today().tokens, 1_200);
    meter.admit().unwrap();
    let used = meter.record(&plan_call(10, 10, 0.06), NOW + 11).unwrap();
    assert_eq!(used.as_deref(), Some("daily budget of $0.05"));
    let refused = Meter::open(&store, &record, NOW + 20)
        .unwrap()
        .admit()
        .unwrap_err();
    assert!(refused.contains("daily budget of $0.05"), "{refused}");
    // Tomorrow starts fresh.
    Meter::open(&store, &record, NOW + DAY)
        .unwrap()
        .admit()
        .unwrap();
}

#[test]
fn she_never_spends_against_a_budget_or_ledger_she_cannot_read() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = alice(&dir);
    std::fs::write(store.dir().join(BUDGET_FILE), "{\"schema\":\"x\",\"v\":1}").unwrap();
    assert!(Meter::open(&store, &record, NOW).is_err());
    std::fs::write(
        store.dir().join(BUDGET_FILE),
        format!("{{\"schema\":\"{BUDGET_SCHEMA}\",\"v\":1,\"daily_usd\":-1}}"),
    )
    .unwrap();
    assert!(Meter::open(&store, &record, NOW).is_err());
    std::fs::remove_file(store.dir().join(BUDGET_FILE)).unwrap();

    // A record she can't open is a ledger she can't check.
    let stranger = SecretKey::from_byte_array(STRANGER).unwrap();
    let theirs = seal(&metric(), &stranger, &owner_public(), NOW).unwrap();
    append(&store, &theirs).unwrap();
    let why = Meter::open(&store, &record, NOW).unwrap_err();
    assert!(why.contains("another agent"), "{why}");
    std::fs::write(store.dir().join(LEDGER_FILE), "not an event\n").unwrap();
    assert!(Meter::open(&store, &record, NOW).is_err());
}

#[test]
fn without_an_attestation_she_tallies_the_request_but_records_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let record = store.open(dir.path(), 1).unwrap();
    budget(&store, |b| b.request_tokens = 100);
    let mut meter = Meter::open(&store, &record, NOW).unwrap();
    assert!(meter.unsealed.as_deref().unwrap().contains("not recorded"));
    assert!(meter.record(&coder_call(1, 150), NOW).unwrap().is_some());
    assert!(!store.dir().join(LEDGER_FILE).exists());
}

/// Hands that play Coder turns reporting `tokens` each and record through
/// a [`Meter`].
struct Metered {
    meter: Meter,
    tokens: u64,
    prompts: Vec<String>,
    journal: Vec<String>,
}

impl Hands for Metered {
    fn say(&mut self, _line: &str) {}

    fn journal(&mut self, _kind: agent::Kind, text: &str, _status: Option<i32>) {
        self.journal.push(text.to_string());
    }

    fn coder(&mut self, prompt: &str) -> Turned {
        self.prompts.push(prompt.to_string());
        let mut turned = Turned::ended(TurnEnd::Finished("Done.".into()));
        turned.ran.push(("cargo test".into(), Some(0)));
        turned.tokens = Some(self.tokens);
        turned
    }

    fn spent(&mut self, call: &Call) -> Option<String> {
        self.meter.record(call, NOW).unwrap()
    }
}

#[test]
fn the_loop_stops_at_the_request_budget_before_the_next_coder_turn() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = alice(&dir);
    budget(&store, |b| b.request_tokens = 1_000);
    let step = |n: u32| Step {
        prompt: format!("Do part {n} of the work."),
        done_when: format!("part {n} is done"),
    };
    let plan = Plan {
        understanding: "Three parts.".into(),
        answer_directly: false,
        reply_if_direct: None,
        steps: vec![step(1), step(2), step(3)],
        verify: None,
    };
    let mut mind = Mind {
        planner: Box::new(ScriptedPlanner {
            plan: Some(plan),
            report: Some("All three parts are done.".into()),
            asked: Default::default(),
        }),
        judge: Some(Box::new(ScriptedJudge {
            judgments: vec![
                Judgment {
                    done: 0.9,
                    unsupported: 0.0,
                    next: Move::Continue,
                    by: String::new(),
                };
                3
            ]
            .into(),
            states: Default::default(),
        })),
        unjudged: String::new(),
    };
    let mut hands = Metered {
        meter: Meter::open(&store, &record, NOW).unwrap(),
        tokens: 600,
        prompts: Vec::new(),
        journal: Vec::new(),
    };
    let policy = Policy::defaults();
    let input = agent_steer::Input {
        record: &record,
        request: "do the three parts",
        briefing: "",
        core: None,
        cwd: "/work",
        policy: &policy,
        note: None,
    };
    let steered = agent_steer::run(&mut hands, &mut mind, &input);
    assert_eq!(hands.prompts.len(), 2, "the third turn never starts");
    assert_eq!(steered.report.headline, "over budget");
    assert!(
        hands
            .journal
            .iter()
            .any(|l| l.contains("my spend records reach the request budget of 1000 tokens")),
        "{:?}",
        hands.journal
    );
    // A used budget buys no report call: Coder's words stand.
    assert_eq!(steered.report.reply, "Done.");
    let view = owner_read(&store, &owner()).unwrap();
    assert_eq!(view.records.len(), 2);
    assert!(
        view.records
            .iter()
            .all(|r| r.metric.harness == CODER_HARNESS)
    );
}
