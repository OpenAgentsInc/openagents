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
