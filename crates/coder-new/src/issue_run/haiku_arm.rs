//! The constrained Haiku arm (#11387): a fourth decision door that answers
//! the identical System One question a Jev or Clef door gets.
//!
//! The request is the same `jev::SystemOneRequest` (state, Noul/Choice
//! questions, criteria); its wire body goes to `claude -p --model haiku`
//! with no tools and no MCP servers, and the reply is read back into a
//! `jev::SystemOneResponse` with the same answer shapes, probabilities, model
//! id and cost, so the arms compare apples to apples and the per-judgment
//! join row is written the same way for every door.

use std::{path::PathBuf, time::Duration};

use serde_json::{Map, Value, json};

/// The model the arm asks.
pub const MODEL: &str = "haiku";
/// How long one question may take.
const TIMEOUT: Duration = Duration::from_secs(120);

/// The `claude` program the arm runs: `OA_HAIKU_ARM_CLAUDE` when set, else
/// `claude` on the path.
#[must_use]
pub fn program() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("OA_HAIKU_ARM_CLAUDE") {
        return Some(PathBuf::from(path));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("claude"))
        .find(|candidate| candidate.is_file())
}

/// The arm's whole prompt: the typed request, verbatim, and the answer
/// shape it must return.
#[must_use]
pub fn prompt(body: &Map<String, Value>) -> String {
    let state = body.get("state").cloned().unwrap_or(Value::Null);
    let questions = body.get("questions").cloned().unwrap_or(Value::Null);
    format!(
        "You are a decision function, not an assistant. Read the STATE and answer every \
question in QUESTIONS. Use no tools. Reply with one JSON object and nothing else.\n\n\
Answer shapes, keyed by question id:\n\
- a question of type \"noul\": {{\"type\": \"noul\", \"noul\": P}} where P is the probability \
(0 to 1) that the answer is yes.\n\
- a question of type \"choice\": {{\"type\": \"choice\", \"probabilities\": {{OPTION: P, ...}}}} \
with one probability per option in its criteria, summing to 1.\n\n\
Reply as {{\"answers\": {{QUESTION_ID: ANSWER, ...}}}}.\n\n\
QUESTIONS:\n{}\n\nSTATE:\n{}\n",
        serde_json::to_string_pretty(&questions).unwrap_or_default(),
        match &state {
            Value::String(text) => text.clone(),
            other => serde_json::to_string_pretty(other).unwrap_or_default(),
        },
    )
}

/// The first JSON object in a reply, which may be fenced or wrapped in prose.
fn object_in(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start < end)
        .then(|| serde_json::from_str::<Value>(&text[start..=end]).ok())
        .flatten()
        .filter(Value::is_object)
}

/// A probability read leniently (a number or a numeric string), clamped.
fn probability(value: &Value) -> Option<f64> {
    let p = value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))?;
    p.is_finite().then(|| p.clamp(0.0, 1.0))
}

/// Reads the arm's reply against the questions asked and builds the wire
/// body a System One door would send: each asked Noul gets `noul`, each
/// asked Choice a full distribution over its own options (renormalized;
/// options the arm left out get zero), its argmax and that confidence.
///
/// # Errors
///
/// A sentence naming the first question the reply did not answer.
pub fn wire(questions: &Value, reply: &str, model: &str, usage: Value) -> Result<Value, String> {
    let parsed = object_in(reply).ok_or("The Haiku arm's reply held no JSON object.")?;
    let answers = parsed.get("answers").unwrap_or(&parsed);
    let asked = questions
        .as_object()
        .ok_or("The request carried no questions.")?;
    let mut out = Map::new();
    for (id, question) in asked {
        let answer = answers
            .get(id)
            .ok_or_else(|| format!("The Haiku arm did not answer {id}."))?;
        match question["type"].as_str() {
            Some("noul") => {
                let p = answer
                    .get("noul")
                    .and_then(probability)
                    .or_else(|| probability(answer))
                    .ok_or_else(|| format!("The Haiku arm's {id} had no probability."))?;
                out.insert(id.clone(), json!({"type": "noul", "noul": p}));
            }
            Some("choice") => {
                let options: Vec<String> = question["criteria"]
                    .as_object()
                    .map(|criteria| criteria.keys().cloned().collect())
                    .unwrap_or_default();
                if options.is_empty() {
                    return Err(format!("The question {id} named no options."));
                }
                let given = answer.get("probabilities").unwrap_or(answer);
                let raw: Vec<f64> = options
                    .iter()
                    .map(|name| given.get(name).and_then(probability).unwrap_or(0.0))
                    .collect();
                let mass: f64 = raw.iter().sum();
                if mass <= 0.0 {
                    return Err(format!("The Haiku arm gave {id} no probability mass."));
                }
                let mut probabilities = Map::new();
                let (mut best, mut best_p) = (options[0].clone(), -1.0);
                for (name, p) in options.iter().zip(raw) {
                    let p = p / mass;
                    if p > best_p {
                        (best, best_p) = (name.clone(), p);
                    }
                    probabilities.insert(name.clone(), json!(p));
                }
                out.insert(
                    id.clone(),
                    json!({
                        "type": "choice",
                        "choice": best,
                        "confidence": best_p,
                        "probabilities": probabilities,
                    }),
                );
            }
            other => {
                return Err(format!(
                    "The Haiku arm answers Noul and Choice questions, not {}.",
                    other.unwrap_or("an untyped one")
                ));
            }
        }
    }
    Ok(json!({"model": model, "usage": usage, "answers": out}))
}

/// Reads `claude -p --output-format json`'s result object into the arm's
/// answer: the reply text, the model id, and the usage with its cost.
///
/// # Errors
///
/// A sentence when the result is an error or not the expected object.
pub fn read_result(stdout: &[u8], questions: &Value) -> Result<jev::SystemOneResponse, String> {
    let result: Value = serde_json::from_slice(stdout)
        .map_err(|_| "The Haiku arm's output was not a JSON result.".to_owned())?;
    if result["is_error"].as_bool() == Some(true) {
        return Err(format!(
            "The Haiku arm failed: {}",
            result["result"].as_str().unwrap_or("no reason given")
        ));
    }
    let reply = result["result"]
        .as_str()
        .ok_or("The Haiku arm's result had no text.")?;
    let model = result["modelUsage"]
        .as_object()
        .and_then(|models| models.keys().next().cloned())
        .unwrap_or_else(|| MODEL.to_owned());
    let input = [
        "input_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
    ]
    .iter()
    .filter_map(|key| result["usage"][key].as_u64())
    .sum::<u64>();
    let usage = json!({
        "input_tokens": input,
        "output_tokens": result["usage"]["output_tokens"].as_u64(),
        "cost": result["total_cost_usd"].as_f64(),
    });
    let body = wire(questions, reply, &model, usage)?;
    jev::SystemOneResponse::decode(jev::RawResponse {
        status: 200,
        headers: reqwest::header::HeaderMap::new(),
        bytes: serde_json::to_vec(&body).unwrap_or_default(),
    })
    .map_err(|error| format!("The Haiku arm's answer broke the contract: {error}"))
}

/// Asks the arm one request.
///
/// # Errors
///
/// A sentence when `claude` cannot run, times out, or answers off contract.
pub async fn ask(
    program: &std::path::Path,
    request: &jev::SystemOneRequest,
) -> Result<jev::SystemOneResponse, String> {
    use tokio::io::AsyncWriteExt;
    let body = request
        .body(MODEL)
        .map_err(|error| format!("The request did not encode: {error}"))?;
    let questions = body.get("questions").cloned().unwrap_or(Value::Null);
    let mut child = tokio::process::Command::new(program)
        .args([
            "-p",
            "--model",
            MODEL,
            "--output-format",
            "json",
            "--strict-mcp-config",
            "--tools",
            "",
            "--permission-mode",
            "dontAsk",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("The Haiku arm could not start claude: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt(&body).as_bytes())
            .await
            .map_err(|error| format!("The Haiku arm could not read the question: {error}"))?;
    }
    let out = tokio::time::timeout(TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "The Haiku arm took longer than two minutes.".to_owned())?
        .map_err(|error| format!("The Haiku arm stopped: {error}"))?;
    read_result(&out.stdout, &questions)
}

#[cfg(test)]
mod tests {
    use super::{prompt, read_result, wire};
    use serde_json::json;

    fn kind_request() -> jev::SystemOneRequest {
        let choice = jev::Choice::default()
            .option("bug", "fix it")
            .option("feature", "new behavior")
            .option("docs", "text only");
        jev::SystemOneRequest::new(
            "Issue #1: crash",
            jev::Questions::new().with("kind", choice),
        )
    }

    #[test]
    fn the_arm_gets_the_identical_typed_question() {
        let body = kind_request().body("haiku").unwrap();
        let text = prompt(&body);
        assert!(text.contains("\"kind\""), "{text}");
        assert!(text.contains("\"feature\""), "{text}");
        assert!(text.contains("Issue #1: crash"), "{text}");
    }

    #[test]
    fn a_choice_reply_reads_back_as_a_door_answer() {
        let body = kind_request().body("haiku").unwrap();
        let questions = body["questions"].clone();
        let stdout = json!({
            "type": "result",
            "is_error": false,
            "result": "```json\n{\"answers\": {\"kind\": {\"type\": \"choice\", \"probabilities\": {\"bug\": 0.6, \"feature\": 0.2}}}}\n```",
            "total_cost_usd": 0.0012,
            "usage": {"input_tokens": 300, "cache_read_input_tokens": 20, "output_tokens": 40},
            "modelUsage": {"claude-haiku-5-5": {}},
        });
        let response = read_result(stdout.to_string().as_bytes(), &questions).unwrap();
        assert_eq!(response.model, "claude-haiku-5-5");
        assert_eq!(response.usage.cost_usd(), Some(0.0012));
        assert_eq!(response.usage.input_tokens, Some(320));
        let answer = response.choice("kind").unwrap();
        assert_eq!(answer.choice, "bug");
        assert!((answer.probabilities["bug"] - 0.75).abs() < 1e-9);
        assert_eq!(answer.probabilities["docs"], 0.0);
    }

    #[test]
    fn a_noul_reply_reads_back_and_a_missing_answer_is_named() {
        let request = jev::SystemOneRequest::new(
            "state",
            jev::Questions::new().with("relevant", jev::Noul::new("q")),
        );
        let questions = request.body("haiku").unwrap()["questions"].clone();
        let stdout = json!({"is_error": false, "result": "{\"relevant\": {\"noul\": 0.8}}"});
        let response = read_result(stdout.to_string().as_bytes(), &questions).unwrap();
        assert!((response.noul("relevant").unwrap().noul - 0.8).abs() < 1e-9);
        let missing = wire(&questions, "{\"answers\": {}}", "haiku", json!({})).unwrap_err();
        assert!(missing.contains("relevant"), "{missing}");
    }
}
