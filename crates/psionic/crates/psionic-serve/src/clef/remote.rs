//! A remote decision trunk: the Clef backbone and head run in a sealed
//! worker process (the Cloud TPU JAX worker, `crates/psionic/clef-tpu`)
//! and this lane keeps the request edge in Rust: parsing, the byte-exact
//! encoder, admission, answers, calibration and the identity block
//! (#11459). The worker takes the encoded record and returns per-option
//! logits; it never sees request text.

use serde_json::{Value, json};

use super::encode::EncodedRecord;

/// The worker at `url` (`POST /v1/logits`, `GET /v1/identity`).
#[derive(Debug)]
pub struct RemoteTrunk {
    url: String,
    client: reqwest::blocking::Client,
    /// The worker's identity (`backend`, `device`, `precision`, `artifact_digest`, ...).
    pub identity: Value,
}

impl RemoteTrunk {
    /// Connects and reads the worker identity.
    pub fn connect(url: &str) -> Result<Self, String> {
        let url = url.trim_end_matches('/').to_string();
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|error| format!("decision worker client: {error}"))?;
        let identity: Value = client
            .get(format!("{url}/v1/identity"))
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .and_then(reqwest::blocking::Response::json)
            .map_err(|error| format!("decision worker {url}: {error}"))?;
        Ok(Self {
            url,
            client,
            identity,
        })
    }

    /// The backend label the worker reports (`tpu`).
    pub fn backend(&self) -> &str {
        self.identity["backend"].as_str().unwrap_or("remote")
    }

    /// Per-question logits in prompt option order, with the worker's
    /// prefill and head seconds and the prompt tokens it restored from a
    /// prefix snapshot. `chunk` 0 takes the worker's default.
    pub fn logits(
        &self,
        record: &EncodedRecord,
        chunk: usize,
    ) -> Result<(Vec<Vec<f32>>, f64, f64, usize), String> {
        let questions: Vec<Value> = record
            .questions
            .iter()
            .map(|question| {
                json!({
                    "type": question.question_type,
                    "question_span": [question.question_span.0, question.question_span.1],
                    "option_spans": question.option_spans.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(),
                })
            })
            .collect();
        // the exact prompt prefix (system text and state): the worker keeps
        // its backbone state there and restarts from it on a repeat
        let body = json!({"input_ids": record.input_ids, "questions": questions, "chunk": chunk,
                          "prefix_tokens": if record.media.is_none() { record.prefix_tokens } else { 0 }});
        let reply: Value = self
            .client
            .post(format!("{}/v1/logits", self.url))
            .json(&body)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .and_then(reqwest::blocking::Response::json)
            .map_err(|error| format!("decision worker: {error}"))?;
        let logits = parse_logits(&reply, record)?;
        let seconds = |key: &str| reply[key].as_f64().unwrap_or(0.0);
        let restored = reply["cached_tokens"].as_u64().unwrap_or(0) as usize;
        Ok((
            logits,
            seconds("prefill_seconds"),
            seconds("head_seconds"),
            restored,
        ))
    }
}

/// Checks the reply's shape against the record: one row per question, one
/// finite logit per option.
fn parse_logits(reply: &Value, record: &EncodedRecord) -> Result<Vec<Vec<f32>>, String> {
    let rows = reply["logits"]
        .as_array()
        .ok_or("decision worker: reply has no logits")?;
    if rows.len() != record.questions.len() {
        return Err(format!(
            "decision worker: {} logit rows for {} questions",
            rows.len(),
            record.questions.len()
        ));
    }
    rows.iter()
        .zip(&record.questions)
        .map(|(row, question)| {
            let row = row
                .as_array()
                .ok_or("decision worker: a logit row is not a list")?;
            if row.len() != question.option_spans.len() {
                return Err(format!(
                    "decision worker: {} logits for {} options",
                    row.len(),
                    question.option_spans.len()
                ));
            }
            row.iter()
                .map(|value| {
                    value
                        .as_f64()
                        .filter(|value| value.is_finite())
                        .map(|value| value as f32)
                        .ok_or_else(|| {
                            String::from("decision worker: a logit is not a finite number")
                        })
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clef::encode::EncodedQuestion;

    fn record() -> EncodedRecord {
        EncodedRecord {
            input_ids: vec![1, 2, 3, 4],
            questions: vec![EncodedQuestion {
                question_id: String::from("q"),
                question_type: 0,
                question_span: (0, 1),
                option_spans: vec![(1, 2), (2, 3)],
                option_ids: vec![String::from("true"), String::from("false")],
            }],
            truncated_state_tokens: 0,
            prefix_tokens: 0,
            media: None,
        }
    }

    #[test]
    fn reply_shape_is_checked() {
        let record = record();
        assert_eq!(
            parse_logits(&json!({"logits": [[0.5, -1.0]]}), &record).unwrap(),
            vec![vec![0.5, -1.0]]
        );
        assert!(parse_logits(&json!({"logits": [[0.5]]}), &record).is_err());
        assert!(parse_logits(&json!({"logits": [[0.5, null]]}), &record).is_err());
        assert!(parse_logits(&json!({"logits": []}), &record).is_err());
        assert!(parse_logits(&json!({}), &record).is_err());
    }
}
