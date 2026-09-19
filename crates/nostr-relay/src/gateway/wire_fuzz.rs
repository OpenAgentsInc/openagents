use std::{collections::BTreeSet, panic::AssertUnwindSafe};

use serde::Deserialize;

use super::{ClientMessage, parse_client_message};

const FIXTURE_SCHEMA: &str = "openagents.nostr-relay.deterministic-fuzz.v1";

#[derive(Deserialize)]
struct Fixture {
    schema: String,
    seed: String,
    wire_iterations: usize,
    maximum_wire_bytes: usize,
    wire_seeds: Vec<String>,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    wire_accepted: usize,
    wire_rejected: usize,
    wire_maximum_observed_bytes: usize,
    accepted_verbs: Vec<String>,
}

#[test]
fn deterministic_wire_mutations_are_bounded_and_panic_free() -> Result<(), String> {
    let fixture = fixture()?;
    if fixture.schema != FIXTURE_SCHEMA {
        return Err(format!(
            "unexpected fuzz fixture schema {:?}",
            fixture.schema
        ));
    }
    if fixture.wire_seeds.is_empty() {
        return Err("wire fuzz corpus must have at least one seed".to_owned());
    }
    if !(1..=100_000).contains(&fixture.wire_iterations) {
        return Err("wire fuzz iteration count is outside the bounded range".to_owned());
    }
    if fixture.maximum_wire_bytes != 131_072 {
        return Err("wire fuzz bound must match the default gateway frame bound".to_owned());
    }

    let seed = parse_seed(&fixture.seed)?;
    let mut random = DeterministicRandom::new(seed);
    let mut accepted = 0_usize;
    let mut rejected = 0_usize;
    let mut maximum_observed_bytes = 0_usize;
    let mut accepted_verbs = BTreeSet::new();
    let mut mutation_coverage = [0_usize; 8];

    for iteration in 0..fixture.wire_iterations {
        let seed_index = random.bounded(fixture.wire_seeds.len());
        let seed_message = fixture
            .wire_seeds
            .get(seed_index)
            .ok_or_else(|| format!("wire seed index {seed_index} is out of bounds"))?;
        let (candidate, operation) = if iteration < fixture.wire_seeds.len() {
            (seed_message.clone(), None)
        } else {
            let operation = random.bounded(mutation_coverage.len());
            (
                mutate_ascii(
                    seed_message,
                    operation,
                    &mut random,
                    fixture.maximum_wire_bytes,
                )?,
                Some(operation),
            )
        };
        if let Some(operation) = operation {
            let count = mutation_coverage
                .get_mut(operation)
                .ok_or_else(|| format!("mutation operation {operation} is out of bounds"))?;
            *count = count.saturating_add(1);
        }
        if candidate.len() > fixture.maximum_wire_bytes {
            return Err(format!(
                "iteration {iteration} exceeded the wire bound with {} bytes",
                candidate.len()
            ));
        }
        maximum_observed_bytes = maximum_observed_bytes.max(candidate.len());
        let first = std::panic::catch_unwind(AssertUnwindSafe(|| parse_client_message(&candidate)))
            .map_err(|_| format!("wire parser panicked at iteration {iteration}: {candidate:?}"))?;
        let second = parse_client_message(&candidate);
        if first != second {
            return Err(format!(
                "wire parser was nondeterministic at iteration {iteration}: {candidate:?}"
            ));
        }
        match first {
            Ok(message) => {
                accepted = accepted.saturating_add(1);
                accepted_verbs.insert(message_verb(&message).to_owned());
            }
            Err(_) => rejected = rejected.saturating_add(1),
        }
    }

    if mutation_coverage.contains(&0) {
        return Err(format!(
            "not every wire mutation operator ran: {mutation_coverage:?}"
        ));
    }
    let expected_verbs = fixture.expected.accepted_verbs.into_iter().collect();
    let actual = (accepted, rejected, maximum_observed_bytes, accepted_verbs);
    let expected = (
        fixture.expected.wire_accepted,
        fixture.expected.wire_rejected,
        fixture.expected.wire_maximum_observed_bytes,
        expected_verbs,
    );
    if actual != expected {
        return Err(format!(
            "wire fuzz summary changed: actual {actual:?}, expected {expected:?}"
        ));
    }
    Ok(())
}

fn message_verb(message: &ClientMessage) -> &'static str {
    match message {
        ClientMessage::Event(_) => "EVENT",
        ClientMessage::Req { .. } => "REQ",
        ClientMessage::Close { .. } => "CLOSE",
        ClientMessage::Count { .. } => "COUNT",
        ClientMessage::Auth(_) => "AUTH",
    }
}

fn mutate_ascii(
    seed: &str,
    operation: usize,
    random: &mut DeterministicRandom,
    maximum_bytes: usize,
) -> Result<String, String> {
    let mut bytes = seed.as_bytes().to_vec();
    match operation {
        0 => {
            let position = random.bounded(bytes.len().max(1));
            if let Some(byte) = bytes.get_mut(position) {
                *byte = mutation_byte(random.bounded(16));
            } else {
                bytes.push(mutation_byte(random.bounded(16)));
            }
        }
        1 => {
            let position = random.bounded(bytes.len().saturating_add(1));
            bytes.insert(position, mutation_byte(random.bounded(16)));
        }
        2 => {
            if !bytes.is_empty() {
                let start = random.bounded(bytes.len());
                let maximum = bytes.len().saturating_sub(start).min(32);
                let length = random.bounded(maximum).saturating_add(1);
                bytes.drain(start..start.saturating_add(length));
            }
        }
        3 => {
            if !bytes.is_empty() {
                let start = random.bounded(bytes.len());
                let maximum = bytes.len().saturating_sub(start).min(32);
                let length = random.bounded(maximum).saturating_add(1);
                let end = start.saturating_add(length);
                let duplicate = bytes
                    .get(start..end)
                    .ok_or_else(|| "wire duplicate range is invalid".to_owned())?
                    .to_vec();
                let position = random.bounded(bytes.len().saturating_add(1));
                bytes.splice(position..position, duplicate);
            }
        }
        4 => {
            let length = random.bounded(bytes.len().saturating_add(1));
            bytes.truncate(length);
        }
        5 => {
            let token = mutation_token(random.bounded(8));
            let position = random.bounded(bytes.len().saturating_add(1));
            bytes.splice(position..position, token.bytes());
        }
        6 => {
            let repetitions = random.bounded(8).saturating_add(1);
            let original = bytes.clone();
            for _ in 0..repetitions {
                if bytes.len() >= maximum_bytes {
                    break;
                }
                bytes.extend_from_slice(&original);
            }
        }
        7 => {
            let length = random.bounded(512);
            bytes.clear();
            bytes.reserve(length);
            for _ in 0..length {
                bytes.push(mutation_byte(random.bounded(16)));
            }
        }
        _ => return Err(format!("unknown wire mutation operation {operation}")),
    }
    bytes.truncate(maximum_bytes);
    String::from_utf8(bytes).map_err(|error| format!("mutation produced non-UTF-8: {error}"))
}

fn mutation_byte(index: usize) -> u8 {
    match index {
        0 => b'{',
        1 => b'}',
        2 => b'[',
        3 => b']',
        4 => b'"',
        5 => b'\\',
        6 => b',',
        7 => b':',
        8 => b'0',
        9 => b'9',
        10 => b'n',
        11 => b't',
        12 => b'f',
        13 => b' ',
        14 => b'\n',
        _ => b'x',
    }
}

fn mutation_token(index: usize) -> &'static str {
    match index {
        0 => "null",
        1 => "true",
        2 => "false",
        3 => "{}",
        4 => "[]",
        5 => "\"x\"",
        6 => "18446744073709551616",
        _ => "\\u0000",
    }
}

struct DeterministicRandom(u64);

impl DeterministicRandom {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn bounded(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }
        usize::try_from(self.next() % u64::try_from(upper).unwrap_or(u64::MAX)).unwrap_or(0)
    }
}

fn parse_seed(seed: &str) -> Result<u64, String> {
    u64::from_str_radix(seed, 16).map_err(|error| format!("invalid fuzz seed {seed:?}: {error}"))
}

fn fixture() -> Result<Fixture, String> {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/nip01/fuzz-corpus.json"
    ))
    .map_err(|error| format!("invalid wire fuzz fixture: {error}"))
}
