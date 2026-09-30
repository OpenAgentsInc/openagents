//! The NIP-HOST fixtures validate and round-trip exactly under the
//! production relay policy; the invalid ones refuse with the stated code,
//! either when parsed or when validated. Runs without the host feature.

use coder_access::protocol::{Reply, Request};
use coder_access::{Code, Enrollment, Grant, RelayPolicy};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const POLICY: RelayPolicy = RelayPolicy::Production;

fn fixtures() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/nip-host.json"
    ))
    .expect("fixture file");
    serde_json::from_str(&text).expect("fixture JSON")
}

/// Parse `json` as `T` and confirm it serializes back to the same value.
fn round_trip<T: DeserializeOwned + Serialize>(name: &str, json: &Value) -> T {
    let body: T = serde_json::from_value(json.clone())
        .unwrap_or_else(|error| panic!("{name} does not parse: {error}"));
    assert_eq!(
        &serde_json::to_value(&body).expect("serialize"),
        json,
        "{name} does not round-trip"
    );
    body
}

#[test]
fn every_valid_fixture_validates_and_round_trips() {
    let all = fixtures();
    let valid = all["valid"].as_object().expect("valid map");
    let mut requests = std::collections::BTreeMap::new();
    for (name, json) in valid {
        if name.starts_with("grant") {
            let grant: Grant = round_trip(name, json);
            grant
                .validate(POLICY)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        } else if name == "enrollment" {
            let enrollment: Enrollment = round_trip(name, json);
            enrollment
                .validate(POLICY)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        } else if name.starts_with("request_") {
            let request: Request = round_trip(name, json);
            request
                .validate(POLICY)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            requests.insert(request.request.clone(), request);
        } else if !name.starts_with("reply_") {
            panic!("no check for fixture {name}");
        }
    }
    // A reply's outcome must be one its request's operation can produce.
    for (name, json) in valid.iter().filter(|(name, _)| name.starts_with("reply_")) {
        let reply: Reply = round_trip(name, json);
        let request = requests
            .get(&reply.request)
            .unwrap_or_else(|| panic!("{name} answers no fixture request"));
        match &reply.result {
            coder_access::protocol::ReplyResult::Ok { outcome } => {
                assert!(
                    outcome.answers(&request.op),
                    "{name} answers the wrong operation"
                );
                outcome
                    .validate()
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
            }
            coder_access::protocol::ReplyResult::Refused { code, missing } => {
                if *code == Code::MissingRight {
                    assert_eq!(
                        *missing,
                        request.op.required(),
                        "{name} names the wrong right"
                    );
                } else {
                    assert!(missing.is_none(), "{name} names a right it does not need");
                }
            }
        }
    }
}

/// The code a body produces, from parsing or from validation.
fn refusal<T: DeserializeOwned>(
    body: &Value,
    validate: impl Fn(&T) -> coder_access::Result<()>,
) -> Code {
    match serde_json::from_value::<T>(body.clone()) {
        Err(_) => Code::Malformed,
        Ok(parsed) => validate(&parsed).expect_err("the body validated").code,
    }
}

#[test]
fn every_invalid_fixture_refuses_with_its_code() {
    let all = fixtures();
    for case in all["invalid"].as_array().expect("invalid list") {
        let name = case["name"].as_str().expect("name");
        let body = &case["body"];
        let code = match case["schema"].as_str().expect("schema") {
            "grant" => refusal::<Grant>(body, |g| g.validate(POLICY)),
            "enrollment" => refusal::<Enrollment>(body, |e| e.validate(POLICY)),
            "request" => refusal::<Request>(body, |r| r.validate(POLICY)),
            other => panic!("unknown schema {other}"),
        };
        let expected: Code = serde_json::from_value(case["code"].clone()).expect("fixture code");
        assert_eq!(code, expected, "{name}");
    }
}

#[test]
fn every_operation_has_a_fixture() {
    let all = fixtures();
    let kinds: std::collections::BTreeSet<String> = all["valid"]
        .as_object()
        .expect("valid map")
        .values()
        .filter_map(|body| body["op"]["kind"].as_str().map(str::to_owned))
        .collect();
    for kind in [
        "enroll.redeem",
        "enroll.approve",
        "enroll.deny",
        "invite.create",
        "invite.cancel",
        "device.list",
        "device.revoke",
        "task.create",
        "terminal.open",
        "task.steer",
        "task.cancel",
        "workspace.list",
        "task.command",
        "task.queue",
        "spend.list",
        "spend.settle",
        "chats.invite",
        "thread.list",
        "thread.read",
        "thread.send",
    ] {
        assert!(kinds.contains(kind), "no request fixture for {kind}");
    }
}

#[test]
fn workspace_lists_are_sorted_distinct_and_bounded() {
    use coder_access::protocol::{MAX_WORKSPACES, Operation, Outcome};
    let list = |labels: &[&str]| Outcome::Workspaces {
        workspaces: labels.iter().map(|label| (*label).to_owned()).collect(),
    };
    assert!(list(&[]).validate().is_ok());
    assert!(list(&["openagents", "scratch"]).validate().is_ok());
    assert!(list(&["openagents", "scratch"]).answers(&Operation::ListWorkspaces {}));
    assert!(!list(&["openagents"]).answers(&Operation::ListDevices {}));
    for bad in [
        list(&["scratch", "openagents"]),
        list(&["same", "same"]),
        list(&[""]),
        list(&["line\nbreak"]),
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    let many: Vec<String> = (0..=MAX_WORKSPACES).map(|i| format!("w{i:03}")).collect();
    assert_eq!(
        Outcome::Workspaces { workspaces: many }
            .validate()
            .expect_err("over the bound")
            .code,
        Code::Bounds
    );
    assert_eq!(
        Operation::ListWorkspaces {}.required(),
        Some(coder_access::Right::Operate)
    );
}

#[test]
fn chats_invite_requires_observe_and_answers_only_a_chat_invitation() {
    use coder_access::protocol::{MAX_CHAT_INVITATION, Operation, Outcome};
    let op = Operation::InviteChats {};
    assert_eq!(op.name(), "chats.invite");
    assert_eq!(op.required(), Some(coder_access::Right::Observe));
    let chats = |invitation: String| Outcome::Chats {
        invitation,
        expires_at: 1_790_000_000,
    };
    let good = chats("coder-pair:AAAA".into());
    assert!(good.validate().is_ok());
    assert!(good.answers(&op));
    assert!(!good.answers(&Operation::ListWorkspaces {}));
    for bad in [
        chats("coder-host:AAAA".into()),
        chats(format!("coder-pair:{}", "A".repeat(MAX_CHAT_INVITATION))),
        chats("coder-pair:\u{e9}".into()),
    ] {
        assert_eq!(bad.validate().expect_err("refused").code, Code::Malformed);
    }
}

#[test]
fn thread_reads_need_observe_sends_need_operate_and_reads_are_not_retained() {
    use coder_access::protocol::{Operation, Outcome, Receipt};
    use coder_access::thread::{MAX_THREADS, ThreadPage, ThreadRow};
    let thread = "0f".repeat(16);
    let list = Operation::ListThreads {};
    let read = Operation::ReadThread {
        thread: thread.clone(),
        before: None,
    };
    let send = Operation::SendThread {
        thread: thread.clone(),
        request: "1e".repeat(16),
        text: "And in the snow?".into(),
    };
    assert_eq!(list.required(), Some(coder_access::Right::Observe));
    assert_eq!(read.required(), Some(coder_access::Right::Observe));
    assert_eq!(send.required(), Some(coder_access::Right::Operate));
    assert!(list.reads_only() && read.reads_only() && !send.reads_only());
    let row = |n: usize| ThreadRow {
        thread: format!("{n:032x}"),
        title: format!("Thread {n}"),
        started: 1,
        updated: 2,
        pinned: false,
        coder: None,
    };
    let rows = Outcome::Threads {
        threads: (0..3).map(row).collect(),
    };
    assert!(rows.validate().is_ok() && rows.answers(&list) && !rows.answers(&read));
    let too_many = Outcome::Threads {
        threads: (0..=MAX_THREADS).map(row).collect(),
    };
    assert_eq!(too_many.validate().unwrap_err().code, Code::Bounds);
    let page = ThreadPage {
        thread: thread.clone(),
        title: "Rain".into(),
        start: 0,
        total: 0,
        turns: vec![],
        busy: false,
        partial: String::new(),
        failure: None,
        coder: None,
    };
    let answer = Outcome::Thread {
        thread: Box::new(page.clone()),
    };
    assert!(answer.validate().is_ok() && answer.answers(&read));
    let other = Outcome::Thread {
        thread: Box::new(ThreadPage {
            thread: "1f".repeat(16),
            ..page.clone()
        }),
    };
    assert!(!other.answers(&read));
    let beyond = Outcome::Thread {
        thread: Box::new(ThreadPage { start: 1, ..page }),
    };
    assert!(beyond.validate().is_err());
    let receipt = Outcome::Dispatched {
        receipt: Receipt {
            operation: "thread.send".into(),
            reference: thread,
        },
    };
    assert!(receipt.answers(&send));
}
