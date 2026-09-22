//! The decision door: one System One call per judgment, on the record.
//!
//! An ensemble world that names a `decision` endpoint asks a System One
//! model — a local `kev-serve`, or a live TypeSafe door — to choose
//! between named options where the choice needs judgment rather than
//! arithmetic. The call is evidence: the exact state, the questions,
//! the model identity, and the answer are recorded into the run
//! directory so the resulting action can be traced back to the answer
//! that selected it. Each record names its transport; the NIP-CJ relay
//! family carries the same request body when a decision worker is
//! deployed, and `local-http` marks what this door used.
//!
//! Probabilities are calibrated judgments, not permission. What a guild
//! may do is bounded by the manifest and the ledger; the model only
//! orders honest work. And the answer is recorded as sent: NIP-CJ
//! normalizes `confidence` to the picked option's probability, but the
//! SDK accepts any in-range value, so [`Door::choose`] recomputes it
//! from the returned distribution rather than trusting the field.

use std::path::{Path, PathBuf};
use std::time::Duration;

use jev::{Choice, Noul, Questions, RetryPolicy, SystemOneRequest};
use serde_json::{Map, Value, json};

use crate::error::{Error, Result};

/// A System One door an episode may ask.
pub struct Door {
    client: jev::BlockingClient,
    /// The model the request names, for the record.
    model: String,
    /// Where the evidence lands — `decisions/` under the run dir.
    dir: PathBuf,
    /// How many calls this door has answered.
    calls: u64,
    /// Whether the door is a loopback endpoint — the record's
    /// `transport` reads `local-http` for one and `live-http` for the
    /// remote API, so a claim never names a transport it did not use.
    local: bool,
}

/// What a `choice` question returned, decoded for the caller.
#[derive(Clone, Debug)]
pub struct Picked {
    /// The option the model selected.
    pub choice: String,
    /// The distribution's concentration.
    pub confidence: f64,
    /// Every option's probability.
    pub probabilities: Map<String, Value>,
    /// The recorded request and answer.
    pub record: PathBuf,
}

/// What a `noul` question returned, decoded for the caller.
#[derive(Clone, Debug)]
pub struct Judged {
    /// The probability the condition holds.
    pub probability: f64,
    /// The recorded request and answer.
    pub record: PathBuf,
}

impl Door {
    /// Points at a `POST /v1/systemone` endpoint — `kev-serve` answers
    /// without a key, so the credential is a placeholder for the
    /// client, not a secret.
    ///
    /// # Errors
    ///
    /// The client must build; the decisions directory must create.
    pub fn local(url: &str, model: &str, dir: impl AsRef<Path>) -> Result<Self> {
        std::fs::create_dir_all(dir.as_ref())?;
        let client = jev::BlockingClient::new(jev::Config::local(url, model))
            .map_err(|error| Error::decision(format!("client: {error}")))?;
        Ok(Door {
            client,
            model: model.to_string(),
            dir: dir.as_ref().to_path_buf(),
            calls: 0,
            local: true,
        })
    }

    /// Points at a remote `POST /v1/systemone` endpoint — the live
    /// TypeSafe API — where the caller's `TYPESAFE_API_KEY` does the
    /// talking.
    ///
    /// # Errors
    ///
    /// The client must build: a credential must resolve and the
    /// decisions directory must create.
    pub fn live(url: &str, model: &str, dir: impl AsRef<Path>) -> Result<Self> {
        std::fs::create_dir_all(dir.as_ref())?;
        let client =
            jev::BlockingClient::new(jev::Config::new().base_url(url).default_model(model))
                .map_err(|error| Error::decision(format!("client: {error}")))?;
        Ok(Door {
            client,
            model: model.to_string(),
            dir: dir.as_ref().to_path_buf(),
            calls: 0,
            local: false,
        })
    }

    /// The model this door asks — `jev-latest`, `kev-latest`, and so on.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// One `POST /v1/systemone` exchange and its record file. The
    /// question set is the caller's — `choose` and `verify` both land
    /// here so every decision the episode makes keeps the same
    /// evidence shape.
    fn call(
        &mut self,
        state: &Value,
        questions: Questions,
        purpose: &str,
    ) -> Result<(jev::SystemOneResponse, PathBuf)> {
        // A `busy` answer means a forward is computing, not that the
        // door is down — wait it out rather than fail the episode.
        let retry = RetryPolicy {
            max_retries: 8,
            backoff_initial: Duration::from_secs(1),
            backoff_max: Duration::from_secs(15),
            budget: Some(Duration::from_secs(360)),
            ..RetryPolicy::default()
        };
        let request = SystemOneRequest::new(state.clone(), questions)
            .model(self.model.clone())
            // CPU inference runs tens of seconds, and concurrent asks
            // queue behind each other — two guild forwards can hold a
            // small door for minutes. The SDK's ten-second default
            // abandons a healthy answer mid-flight.
            .timeout(Duration::from_secs(240))
            .retry(retry);
        let body = request
            .body(&self.model)
            .map_err(|error| Error::decision(format!("request: {error}")))?;
        let started = std::time::Instant::now();
        let response = self.client.system_one(request);
        let milliseconds = started.elapsed().as_millis() as u64;
        self.calls += 1;
        let record = self.dir.join(format!("decision-{}.json", self.calls));
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                // A call that got no answer is still a call: the record
                // holds the request, the wait, and the error, so a lost
                // reply leaves evidence instead of silence.
                std::fs::write(
                    &record,
                    serde_json::to_vec_pretty(&json!({
                        "purpose": purpose,
                        "transport": if self.local { "local-http" } else { "live-http" },
                        "milliseconds": milliseconds,
                        "request": body,
                        "error": error.to_string(),
                    }))?,
                )?;
                return Err(Error::decision(format!("{error}")));
            }
        };
        // The raw body is the evidence: the answers and probabilities
        // exactly as the door sent them, with the call's own latency —
        // the evidence renderer reads it for the demo's decision-time
        // metric.
        let raw = response.raw();
        let raw_body: Value =
            serde_json::from_slice(&raw.bytes).unwrap_or_else(|_| json!({"text": raw.text()}));
        std::fs::write(
            &record,
            serde_json::to_vec_pretty(&json!({
                "purpose": purpose,
                // Direct `POST /v1/systemone` HTTP, not a CJ relay
                // transport — the coverage record names it so.
                "transport": if self.local { "local-http" } else { "live-http" },
                "milliseconds": milliseconds,
                "request": body,
                "response": {
                    "status": raw.status,
                    "request_id": response.request_id(),
                    "model": response.model,
                    "body": raw_body,
                },
            }))?,
        )?;
        Ok((response, record))
    }

    /// Asks one `choice` question over `state` and returns the picked
    /// option plus the path of the recorded exchange.
    ///
    /// # Errors
    ///
    /// The door must answer a typed `choice` answer; a refusal is an
    /// error carrying the door's code.
    pub fn choose(
        &mut self,
        state: Value,
        instructions: &str,
        options: &[(String, String)],
        purpose: &str,
    ) -> Result<Picked> {
        let mut choice = Choice {
            instructions: Some(instructions.to_string().into()),
            ..Choice::default()
        };
        for (name, description) in options {
            choice = choice.option(name.clone(), description.clone());
        }
        let (response, record) =
            self.call(&state, Questions::new().with("pick", choice), purpose)?;
        let picked = response
            .choice("pick")
            .map_err(|error| Error::decision(format!("answer: {error}")))?;
        // NIP-CJ's decision family normalizes `confidence` to the
        // selected option's probability; the SDK accepts any value in
        // [0,1], so the door's own field is not trusted — the recorded
        // distribution is the evidence and the answer's own mass says
        // how sure it was.
        let confidence = picked
            .probabilities
            .get(&picked.choice)
            .copied()
            .unwrap_or(picked.confidence);
        Ok(Picked {
            choice: picked.choice.clone(),
            confidence,
            probabilities: picked
                .probabilities
                .iter()
                .map(|(name, probability)| (name.clone(), json!(probability)))
                .collect(),
            record,
        })
    }

    /// Asks one `noul` question over `state` — the critic's call:
    /// the probability that a stated condition holds, with the same
    /// recorded evidence a `choice` leaves.
    ///
    /// # Errors
    ///
    /// The door must answer a typed `noul` answer; a refusal is an
    /// error carrying the door's code.
    pub fn verify(&mut self, state: Value, instructions: &str, purpose: &str) -> Result<Judged> {
        let (response, record) = self.call(
            &state,
            Questions::new().with("verdict", Noul::new(instructions)),
            purpose,
        )?;
        let answer = response
            .noul("verdict")
            .map_err(|error| Error::decision(format!("answer: {error}")))?;
        Ok(Judged {
            probability: answer.noul,
            record,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    /// A canned `POST /v1/systemone` answer over one TCP accept — the
    /// smallest door that proves the request shape, the record file,
    /// and the picked choice.
    #[test]
    fn choose_records_the_exchange() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 8192];
            let head_end = loop {
                let read = stream.read(&mut chunk).expect("read");
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                    break at + 4;
                }
            };
            let headers = String::from_utf8_lossy(&buffer[..head_end]).to_string();
            let length: usize = headers
                .split("\r\n")
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(str::trim)
                        .map(str::to_string)
                })
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            while buffer.len() < head_end + length {
                let read = stream.read(&mut chunk).expect("read");
                buffer.extend_from_slice(&chunk[..read]);
            }
            let sent = String::from_utf8_lossy(&buffer[head_end..head_end + length]).to_string();
            let body = "{\"model\":\"kev-latest\",\"answers\":{\"pick\":{\"type\":\"choice\",\"choice\":\"beta\",\"confidence\":0.34,\"probabilities\":{\"alpha\":0.33,\"beta\":0.67}}},\"usage\":{\"input_tokens\":10,\"output_tokens\":4}}";
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).expect("write");
            sent
        });
        let dir = tempfile::tempdir().expect("dir");
        let mut door = Door::local(
            &format!("http://127.0.0.1:{port}"),
            "kev-latest",
            dir.path(),
        )
        .expect("door");
        let picked = door
            .choose(
                json!({"guild": "ferro"}),
                "which first?",
                &[
                    ("alpha".to_string(), "the first".to_string()),
                    ("beta".to_string(), "the second".to_string()),
                ],
                "test decision",
            )
            .expect("choose");
        let sent: Value =
            serde_json::from_str(&server.join().expect("server")).expect("request json");
        assert_eq!(picked.choice, "beta");
        // The door reported confidence 0.34; the normalized value is
        // the selected option's own probability.
        assert_eq!(picked.confidence, 0.67);
        assert_eq!(sent["state"]["guild"], "ferro");
        assert_eq!(sent["questions"]["pick"]["type"], "choice");
        assert_eq!(sent["model"], "kev-latest");
        let record: Value =
            serde_json::from_str(&std::fs::read_to_string(&picked.record).expect("record file"))
                .expect("record json");
        assert_eq!(record["transport"], "local-http");
        assert_eq!(record["purpose"], "test decision");
        assert_eq!(record["request"]["questions"]["pick"]["type"], "choice");
        assert_eq!(
            record["response"]["body"]["answers"]["pick"]["choice"],
            "beta"
        );
        assert_eq!(
            record["response"]["body"]["answers"]["pick"]["probabilities"]["beta"],
            0.67
        );
    }

    /// Against a live local door (`VOYAGER_TEST_DOOR_URL`): the same
    /// `choose` the ensemble posts, so a silent fallback in a run can be
    /// reproduced and read directly.
    #[test]
    #[ignore]
    fn live_door_choose() {
        let url = std::env::var("VOYAGER_TEST_DOOR_URL").expect("VOYAGER_TEST_DOOR_URL");
        let dir = tempfile::tempdir().expect("dir");
        let mut door = Door::local(&url, "kev-latest", dir.path()).expect("door");
        let picked = door
            .choose(
                json!({"guild": "ferro"}),
                "Which contested deposit should this guild work first?",
                &[
                    ("alpha".to_string(), "the first".to_string()),
                    ("beta".to_string(), "the second".to_string()),
                ],
                "live decision",
            )
            .expect("choose");
        assert!(picked.record.is_file());
    }
}
