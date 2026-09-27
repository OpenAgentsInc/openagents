//! The NIP-REACH fixtures: bodies validate and round-trip exactly, invalid
//! bodies refuse with the stated code, placement vectors pick the stated
//! host for the stated reasons, and the transcript vector pins the digest
//! both channel proofs sign.

use coder_reach::channel::{ClientHello, HostProof, transcript};
use coder_reach::directory::Directory;
use coder_reach::hints::Hints;
use coder_reach::placement::{Assessment, Candidate, Limits, Skip, assess, place};
use coder_reach::presence::{ClientProfile, Freshness, Presence, Received, VersionRange};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixtures() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/nip-reach.json"
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
    for (name, json) in all["valid"].as_object().expect("valid map") {
        let result = match name.as_str() {
            "directory" => round_trip::<Directory>(name, json).validate(),
            "presence" | "presence_without_telemetry" => {
                round_trip::<Presence>(name, json).validate()
            }
            "hints" => round_trip::<Hints>(name, json).validate(),
            other => panic!("no check for fixture {other}"),
        };
        result.unwrap_or_else(|error| panic!("{name}: {}", error.code.as_str()));
    }
}

/// The wire code a body produces, from parsing or from validation.
fn refusal<T: DeserializeOwned>(
    body: &Value,
    validate: impl Fn(&T) -> coder_reach::Result<()>,
) -> &'static str {
    match serde_json::from_value::<T>(body.clone()) {
        Err(_) => "malformed",
        Ok(parsed) => validate(&parsed)
            .expect_err("the body validated")
            .code
            .as_str(),
    }
}

#[test]
fn every_invalid_fixture_refuses_with_its_code() {
    let all = fixtures();
    for case in all["invalid"].as_array().expect("invalid list") {
        let name = case["name"].as_str().expect("name");
        let body = &case["body"];
        let code = match case["schema"].as_str().expect("schema") {
            "directory" => refusal::<Directory>(body, Directory::validate),
            "presence" => refusal::<Presence>(body, Presence::validate),
            "hints" => refusal::<Hints>(body, Hints::validate),
            other => panic!("unknown schema {other}"),
        };
        assert_eq!(code, case["code"].as_str().expect("code"), "{name}");
    }
}

fn skip_name(skip: Skip) -> &'static str {
    match skip {
        Skip::NotAdmitted => "not_admitted",
        Skip::ZeroWeight => "zero_weight",
        Skip::NoPresence => "no_presence",
        Skip::Stale => "stale",
        Skip::Incompatible => "incompatible",
        Skip::NoTelemetry => "no_telemetry",
        Skip::Overloaded => "overloaded",
    }
}

#[test]
fn every_placement_vector_picks_the_stated_host() {
    let all = fixtures();
    for vector in all["placement"].as_array().expect("placement list") {
        let name = vector["name"].as_str().expect("name");
        let client: Value = vector["client"].clone();
        let accepts: VersionRange =
            serde_json::from_value(client["accepts"].clone()).expect("accepts");
        let client = ClientProfile {
            protocol: u32::try_from(client["protocol"].as_u64().expect("protocol"))
                .expect("protocol fits"),
            accepts,
        };
        let now = vector["now"].as_u64().expect("now");
        let received: Vec<(String, u32, bool, Received)> = vector["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .map(|c| {
                let presence: Presence =
                    serde_json::from_value(c["presence"].clone()).expect("presence");
                presence.validate().expect("candidate presence is valid");
                (
                    c["host"].as_str().expect("host").to_owned(),
                    u32::try_from(c["weight"].as_u64().expect("weight")).expect("weight fits"),
                    c["admitted"].as_bool().expect("admitted"),
                    Received {
                        presence,
                        received_at: c["received_at"].as_u64().expect("received_at"),
                    },
                )
            })
            .collect();
        let candidates: Vec<Candidate<'_>> = received
            .iter()
            .map(|(host, weight, admitted, sample)| Candidate {
                host,
                weight: *weight,
                presence: Some(sample),
                admitted: *admitted,
            })
            .collect();
        let freshness = Freshness::default();
        let limits = Limits::default();
        let chosen = place(&candidates, &client, now, freshness, limits);
        assert_eq!(chosen, vector["expected"].as_str(), "{name}: chosen host");
        for assessment in assess(&candidates, &client, now, freshness, limits) {
            let (host, verdict) = match assessment {
                Assessment::Eligible { host, .. } => (host, "eligible"),
                Assessment::Skipped { host, reason } => (host, skip_name(reason)),
            };
            assert_eq!(
                vector["assessments"][host].as_str(),
                Some(verdict),
                "{name}: {host}"
            );
        }
    }
}

#[test]
fn the_transcript_vector_matches() {
    let all = fixtures();
    let vector = &all["transcript"];
    let hello: ClientHello = round_trip("hello", &vector["hello"]);
    let proof: HostProof = round_trip("host_proof", &vector["host_proof"]);
    let digest = transcript(&hello, &proof).expect("transcript");
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, vector["sha256"].as_str().expect("sha256"));
}
