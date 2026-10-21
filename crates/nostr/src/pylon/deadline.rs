//! The declared-deadline extension to Pylon's NIP-CJ conversation jobs
//! (`pylon.deadline.v1`, W14, #12401).
//!
//! An ordinary job keeps Pylon's 90-second profile. A model that needs
//! longer, such as a model group whose head beacon declares
//! `deadline_secs` over 90, takes a job only when the buyer names
//! [`FEATURE`] in `requires` and states its own budget in `budget_secs`.
//! The provider runs the job for the smaller of that budget and its own
//! declared ceiling, measured from admission. `processing` feedback, the
//! first and every heartbeat after it, says that effective deadline and
//! never extends it.
//!
//! Each side refuses what it doesn't serve instead of waiting it out:
//!
//! - A provider that predates the feature refuses every request whose
//!   `requires` is not empty, `unsupported_feature`, so a buyer hears no
//!   at once.
//! - A provider whose ceiling is over 90 seconds refuses an ordinary
//!   request, `unsupported_feature`, rather than running it past an old
//!   buyer's wait.
//! - Responses to an extended request name the feature in `requires`, so a
//!   reader that doesn't know it refuses them rather than misreading them.
//!
//! Pure: no clock, no network. See `nips/openagents/NIP-PYLON.md`,
//! "Declared deadlines".

use serde_json::{Value, json};

/// The feature ID a request and its responses name in `requires`.
pub const FEATURE: &str = "pylon.deadline.v1";
/// The most seconds an ordinary job runs: Pylon's default profile.
pub const ORDINARY_SECS: u64 = 90;
/// The most seconds a budget or a ceiling may be, as a head beacon's
/// `deadline_secs`.
pub const MAX_SECS: u64 = 3_600;
/// How often a provider repeats `processing` feedback while an extended job
/// runs. A heartbeat says the job is alive; it renews nothing.
pub const HEARTBEAT_SECS: u64 = 30;

/// How a request asks to be timed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// No feature: Pylon's 90-second profile.
    Ordinary,
    /// [`FEATURE`] with the buyer's budget in seconds.
    Declared { budget_secs: u64 },
}

/// Why a body is refused, with its NIP-CJ refusal code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
}

impl Refusal {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// The feature IDs in a body's `requires`, refusing a missing list, a
/// non-string, or a repeated ID.
fn requires(body: &Value) -> Result<Vec<&str>, Refusal> {
    let list = body["requires"]
        .as_array()
        .ok_or_else(|| Refusal::new("malformed", "requires is missing"))?;
    let mut ids: Vec<&str> = Vec::with_capacity(list.len());
    for id in list {
        let id = id
            .as_str()
            .ok_or_else(|| Refusal::new("malformed", "requires lists feature IDs"))?;
        if ids.contains(&id) {
            return Err(Refusal::new("malformed", "requires repeats a feature"));
        }
        ids.push(id);
    }
    Ok(ids)
}

/// Whether the extension's features are exactly [`FEATURE`], none, or
/// something a reader that `supports` it (or not) must refuse.
fn extended(ids: &[&str], supports: bool) -> Result<bool, Refusal> {
    match ids {
        [] => Ok(false),
        [FEATURE] if supports => Ok(true),
        _ if !supports => Err(Refusal::new(
            "unsupported_feature",
            "this pylon requires no features",
        )),
        _ => Err(Refusal::new(
            "unsupported_feature",
            format!("this pylon serves only {FEATURE}"),
        )),
    }
}

/// Read a conversation request's timing. `supports` is false for a
/// provider without the extension, which refuses any feature exactly as
/// providers before it did.
///
/// # Errors
///
/// `malformed` for a bad `requires`, a `budget_secs` without the feature,
/// or the feature without a whole-second budget; `limit_exceeded` for a
/// budget outside 1 to [`MAX_SECS`]; `unsupported_feature` for a feature
/// this provider doesn't serve.
pub fn request_profile(body: &Value, supports: bool) -> Result<Profile, Refusal> {
    let ids = requires(body)?;
    let budget = body.get("budget_secs");
    if !extended(&ids, supports)? {
        if budget.is_some() {
            return Err(Refusal::new(
                "malformed",
                format!("budget_secs needs {FEATURE} in requires"),
            ));
        }
        return Ok(Profile::Ordinary);
    }
    let budget_secs = budget
        .and_then(Value::as_u64)
        .ok_or_else(|| Refusal::new("malformed", "budget_secs is whole seconds"))?;
    if !(1..=MAX_SECS).contains(&budget_secs) {
        return Err(Refusal::new(
            "limit_exceeded",
            format!("budget_secs is 1 to {MAX_SECS}"),
        ));
    }
    Ok(Profile::Declared { budget_secs })
}

/// The seconds a provider whose declared ceiling is `ceiling_secs` gives a
/// request: its ceiling for an ordinary request, and the smaller of the
/// buyer's budget and its ceiling for an extended one.
///
/// # Errors
///
/// `unsupported_feature` for an ordinary request to a provider whose
/// ceiling is over [`ORDINARY_SECS`]: an old buyer stops waiting long
/// before such a job ends, so it's told so at once.
pub fn admit(profile: Profile, ceiling_secs: u64) -> Result<u64, Refusal> {
    let ceiling = ceiling_secs.clamp(1, MAX_SECS);
    match profile {
        Profile::Ordinary if ceiling > ORDINARY_SECS => Err(Refusal::new(
            "unsupported_feature",
            format!("this model takes up to {ceiling} s; ask with {FEATURE} and a budget"),
        )),
        Profile::Ordinary => Ok(ceiling),
        Profile::Declared { budget_secs } => Ok(budget_secs.min(ceiling)),
    }
}

/// The deadline a buyer waits for: the smaller of its budget and the
/// ceiling the provider's beacon declares ([`ORDINARY_SECS`] when it
/// declares none). Feedback never moves it.
#[must_use]
pub fn effective(budget_secs: u64, declared_secs: Option<u64>) -> u64 {
    budget_secs
        .clamp(1, MAX_SECS)
        .min(declared_secs.unwrap_or(ORDINARY_SECS).clamp(1, MAX_SECS))
}

/// Ask for the extension in a conversation request `body` with a budget of
/// `budget_secs`.
pub fn declare(body: &mut Value, budget_secs: u64) {
    body["requires"] = json!([FEATURE]);
    body["budget_secs"] = json!(budget_secs.clamp(1, MAX_SECS));
}

/// The `processing` feedback for an extended job, sent at admission and
/// repeated as the heartbeat, each with the same effective deadline.
#[must_use]
pub fn processing_body(version: u64, deadline_secs: u64) -> Value {
    json!({
        "v": version,
        "requires": [FEATURE],
        "type": "status",
        "status": "processing",
        "deadline_secs": deadline_secs,
    })
}

/// Mark an extended job's `result` body: the feature, the effective
/// deadline it ran under, and `finish: "stop"`, since a provider sends an
/// extended job only a complete answer.
pub fn mark_result(body: &mut Value, deadline_secs: u64) {
    body["requires"] = json!([FEATURE]);
    body["deadline_secs"] = json!(deadline_secs);
    body["finish"] = json!("stop");
}

/// What a response to a conversation request says, as a buyer reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// `processing` feedback, with the effective deadline when the job is
    /// extended.
    Processing { deadline_secs: Option<u64> },
    /// A result, with the deadline and finish of an extended job.
    Result {
        deadline_secs: Option<u64>,
        finish: Option<String>,
    },
    /// Any other body without the feature, which this module doesn't read.
    Other,
}

/// Read a response body. `supports` is false for a buyer without the
/// extension.
///
/// # Errors
///
/// `unsupported_feature` for a response naming a feature this buyer doesn't
/// know, and `malformed` for an extended response without its deadline, or
/// a result whose finish isn't `stop`.
pub fn read_response(body: &Value, supports: bool) -> Result<Response, Refusal> {
    let ids = requires(body)?;
    let extended = extended(&ids, supports).map_err(|mut refusal| {
        if refusal.code == "unsupported_feature" {
            refusal.message =
                format!("the pylon answered with features this buyer doesn't know: {ids:?}");
        }
        refusal
    })?;
    let deadline = || -> Result<Option<u64>, Refusal> {
        if !extended {
            return Ok(None);
        }
        body["deadline_secs"]
            .as_u64()
            .filter(|secs| (1..=MAX_SECS).contains(secs))
            .map(Some)
            .ok_or_else(|| Refusal::new("malformed", "deadline_secs is 1 to 3600"))
    };
    match (body["type"].as_str(), body["status"].as_str()) {
        (Some("status"), Some("processing")) => Ok(Response::Processing {
            deadline_secs: deadline()?,
        }),
        (Some("result"), _) => {
            let deadline_secs = deadline()?;
            let finish = body["finish"].as_str().map(str::to_string);
            if extended && finish.as_deref() != Some("stop") {
                return Err(Refusal::new(
                    "malformed",
                    "an extended result is a finished answer",
                ));
            }
            Ok(Response::Result {
                deadline_secs,
                finish,
            })
        }
        _ if extended => Err(Refusal::new(
            "malformed",
            "an extended response is processing or a result",
        )),
        _ => Ok(Response::Other),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn fixture(name: &str) -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/pylon/fixtures")
            .join(name);
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn code(result: Result<impl std::fmt::Debug, Refusal>) -> &'static str {
        result.unwrap_err().code
    }

    #[test]
    fn an_ordinary_request_keeps_the_ninety_second_profile() {
        let ordinary = fixture("job-ordinary-request.json");
        assert_eq!(request_profile(&ordinary, true), Ok(Profile::Ordinary));
        assert_eq!(request_profile(&ordinary, false), Ok(Profile::Ordinary));
        assert_eq!(admit(Profile::Ordinary, 90), Ok(90));
        assert_eq!(admit(Profile::Ordinary, 30), Ok(30));
    }

    #[test]
    fn an_extended_request_runs_for_the_smaller_of_budget_and_ceiling() {
        let extended = fixture("job-deadline-request.json");
        let profile = request_profile(&extended, true).unwrap();
        assert_eq!(profile, Profile::Declared { budget_secs: 300 });
        assert_eq!(admit(profile, 600), Ok(300));
        assert_eq!(admit(profile, 120), Ok(120));
        // A complete pylon's ceiling is the ordinary 90 seconds.
        assert_eq!(admit(profile, ORDINARY_SECS), Ok(90));
        // The buyer computes the same deadline from the beacon.
        assert_eq!(effective(300, Some(600)), 300);
        assert_eq!(effective(300, Some(120)), 120);
        assert_eq!(effective(300, None), 90);
        assert_eq!(effective(0, Some(600)), 1);
    }

    /// New buyer, old provider: a provider without the feature refuses at
    /// once, with the recorded body an old pylon sends.
    #[test]
    fn an_old_provider_refuses_the_extended_form() {
        let extended = fixture("job-deadline-request.json");
        let refusal = request_profile(&extended, false).unwrap_err();
        let recorded = fixture("job-old-provider-refusal.json");
        assert_eq!(recorded["code"], refusal.code);
        assert_eq!(recorded["message"], refusal.message);
        assert_eq!(recorded["status"], "error");
        assert_eq!(recorded["requires"], json!([]));
    }

    /// Old buyer, new provider: an ordinary request to a model whose
    /// ceiling is over 90 seconds is refused at once, not run past the
    /// buyer's wait.
    #[test]
    fn a_long_model_refuses_an_ordinary_request() {
        let ordinary = fixture("job-ordinary-request.json");
        let profile = request_profile(&ordinary, true).unwrap();
        let refusal = admit(profile, 600).unwrap_err();
        let recorded = fixture("job-long-model-refusal.json");
        assert_eq!(recorded["code"], refusal.code);
        assert_eq!(recorded["message"], refusal.message);
        assert_eq!(admit(profile, 91).unwrap_err().code, "unsupported_feature");
    }

    /// A reader that doesn't know the feature refuses extended responses
    /// instead of reading them as ordinary ones; one that knows it reads the
    /// deadline and the finish.
    #[test]
    fn extended_responses_name_the_feature() {
        let processing = fixture("job-deadline-processing.json");
        let result = fixture("job-deadline-result.json");
        assert_eq!(processing, processing_body(1, 300));
        assert_eq!(
            read_response(&processing, true),
            Ok(Response::Processing {
                deadline_secs: Some(300)
            })
        );
        assert_eq!(
            read_response(&result, true),
            Ok(Response::Result {
                deadline_secs: Some(300),
                finish: Some("stop".into())
            })
        );
        assert_eq!(
            code(read_response(&processing, false)),
            "unsupported_feature"
        );
        assert_eq!(code(read_response(&result, false)), "unsupported_feature");
        let mut built =
            json!({"v": 1, "requires": [], "type": "result", "text": "4", "model": "m@sha256:00"});
        mark_result(&mut built, 300);
        assert_eq!(built["requires"], json!([FEATURE]));
        assert_eq!(built["finish"], "stop");
        // Ordinary responses read as before.
        let ordinary = json!({"v": 1, "requires": [], "type": "status", "status": "processing"});
        assert_eq!(
            read_response(&ordinary, false),
            Ok(Response::Processing {
                deadline_secs: None
            })
        );
    }

    #[test]
    fn malformed_extended_bodies_are_refused() {
        let base = fixture("job-deadline-request.json");
        let with = |f: &dyn Fn(&mut Value)| {
            let mut body = base.clone();
            f(&mut body);
            request_profile(&body, true)
        };
        assert_eq!(code(with(&|b| b["requires"] = json!([]))), "malformed");
        assert_eq!(
            code(with(&|b| {
                b.as_object_mut().unwrap().remove("budget_secs");
            })),
            "malformed"
        );
        assert_eq!(code(with(&|b| b["budget_secs"] = json!(1.5))), "malformed");
        assert_eq!(
            code(with(&|b| b["budget_secs"] = json!("300"))),
            "malformed"
        );
        assert_eq!(
            code(with(&|b| b["budget_secs"] = json!(0))),
            "limit_exceeded"
        );
        assert_eq!(
            code(with(&|b| b["budget_secs"] = json!(MAX_SECS + 1))),
            "limit_exceeded"
        );
        assert_eq!(
            code(with(&|b| b["requires"] = json!([FEATURE, FEATURE]))),
            "malformed"
        );
        assert_eq!(
            code(with(&|b| b["requires"] = json!([FEATURE, "payer.keys"]))),
            "unsupported_feature"
        );
        assert_eq!(
            code(with(&|b| b["requires"] = json!(["pylon.deadline.v2"]))),
            "unsupported_feature"
        );
        assert_eq!(code(with(&|b| b["requires"] = json!([1]))), "malformed");
        assert_eq!(
            code(with(&|b| {
                b.as_object_mut().unwrap().remove("requires");
            })),
            "malformed"
        );
        let mut built = json!({"v": 1, "requires": [], "task": "x"});
        declare(&mut built, 9_999);
        assert_eq!(built["budget_secs"], MAX_SECS);
        assert_eq!(
            request_profile(&built, true),
            Ok(Profile::Declared {
                budget_secs: MAX_SECS
            })
        );
        let mut late = processing_body(1, 300);
        late["deadline_secs"] = json!(0);
        assert_eq!(code(read_response(&late, true)), "malformed");
        let mut cut = fixture("job-deadline-result.json");
        cut["finish"] = json!("length");
        assert_eq!(code(read_response(&cut, true)), "malformed");
    }
}
