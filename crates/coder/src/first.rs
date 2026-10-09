//! The first response's shared pieces, and the suggestion ranking.
//!
//! The chat worker's first response is now the chat router
//! ([`crate::router`], the `chat-router-v2` question set and the
//! `chat-answers-v1` bank as a data file). What stays here is what the
//! router and the ranking share: the judgment's [`BUDGET`] and [`LATE`]
//! bounds and [`retry`] policy, the [`state`] it reads, the [`Lane`]
//! reading, the door [`Facts`] a prepared answer's model slots come from,
//! and the
//! [`MODEL_NOTE`] and [`UNROUTED_NOTE`] the model gets beside a first
//! response.
//!
//! [`rank_questions`] and [`ranking`] turn the same kind of judgment to the
//! phone's suggestions: the caller names its candidate repositories or
//! actions, and the answer orders them. Read
//! `docs/coder/measurements/2026-09-28-first-reply.md` for what the first
//! response saves and `nips/openagents/NIP-CJ.md` for the wire shapes.
//!
//! Everything here is pure: a state in, a request out; an answer in, a
//! reading out. The caller owns the HTTP.

use std::time::Duration;

use indexmap::IndexMap;
use jev::{Answer, Choice, ChoiceAnswer, Entry, Questions, RetryPolicy, SystemOneResponse};
use serde_json::Value;

use crate::generate::{
    DEFAULT_DOOR_URL, Lane as ModelLane, Message, OPENROUTER_DOOR_URL, ProviderPrivacy,
};

/// The ranking's question set identity, as a `rank` result names it.
pub const SET: &str = "coder-first-response-v2";

/// How long the worker waits for the judgment before it shows something:
/// past this, a turn that asked to be shown a first response gets the
/// [`PROGRESS_OPENER`] line while the judgment finishes, up to [`LATE`].
/// A healthy judge answers well inside it (p50 about 0.26 s), so the first
/// response is as fast as before.
pub const BUDGET: Duration = Duration::from_millis(2_500);

/// The second, longer bound on the judgment (#10110). Until it, a turn
/// that asked to be shown a first response holds the model's words, so a
/// late judgment still routes the turn; past it the model answers
/// unrouted, under [`UNROUTED_NOTE`].
pub const LATE: Duration = Duration::from_millis(6_000);

/// The bank opener shown when the judgment runs past [`BUDGET`]: a fixed
/// line, chosen by no reading of the message.
pub const PROGRESS_OPENER: &str = "explain";

/// The fixed note the model gets on a turn that asked for a first
/// response, beside the judgment (#10110). When no judgment routes the
/// turn (the judge is absent, failed, or ran past [`LATE`]) the model
/// answers alone, and this keeps it answering as OpenAgents about our own
/// products. It is an instruction, never a router: it reads nothing.
pub const UNROUTED_NOTE: &str = "OpenAgents' own products have plain names: the Gym (where we \
test plugins for Coder and trainers add and check results), the Verse, Coder (our coding agent), \
plugins, and Jev (the small, fast model from TypeSafe that reads each message first). When the \
user names one of these, they mean ours, not an outside project with a similar name (the Gym is \
not OpenAI Gym or Gymnasium). Answer a question about them here, about ours, and say plainly when \
you do not know a specific detail; do not offer to run Coder or check a repository to answer a \
question.";

/// The instruction the worker adds to the caller's, so the model speaks as
/// OpenAgents and does not open with an acknowledgement of its own after
/// the one that may be shown.
pub const MODEL_NOTE: &str = "We are OpenAgents: always speak as \"we\" and \"us\", never \
\"I\" or \"me\". The user may already see a short opening line above your reply, such as \
\"We'll look that up for you.\" or \"Sorry about that.\", so do not open with an acknowledgement, \
apology, or greeting: begin directly with the substance.";

/// The facts a prepared answer's slots are filled from: the worker's own
/// configuration, never text someone typed once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The chat model, for a person: "Google's Gemini 3.8 Flash".
    pub chat_model: Option<String>,
    /// Where the model is reached: "the Vercel AI Gateway".
    pub chat_model_host: Option<String>,
    /// What the primary's provider may keep of what we send it, or what we
    /// ask of it, as a sentence without its full stop: see
    /// [`keeps_sentence`] (#11040).
    pub chat_model_keeps: Option<String>,
}

/// What the privacy answer says about the providers keeping what we send,
/// at the level the worker's doors ask for ([`ProviderPrivacy`], #11040).
/// Under `strict` every chat-model request asks for no training and no
/// retention, and a primary that cannot agree is not used; below it,
/// Space Bunny Alpha's anonymous provider may keep prompts and replies
/// (OpenRouter's notice for the model, 2026-10-01).
#[must_use]
pub fn keeps_sentence(privacy: ProviderPrivacy) -> &'static str {
    match privacy {
        ProviderPrivacy::Strict => {
            "We ask the model providers not to keep or train on what we send them"
        }
        ProviderPrivacy::NoTraining => {
            "We ask the model providers not to train on what we send them; Space Bunny Alpha's \
             anonymous provider may still keep the messages it is sent and its replies"
        }
        ProviderPrivacy::Off => {
            "Space Bunny Alpha's anonymous provider may keep the messages it is sent and its \
             replies, though not to train on them"
        }
    }
}

impl Facts {
    /// The facts of a door serving `model` at `url` (`None` for a door
    /// that is not a gateway door). A model or a host this does not know
    /// how to name is left out, and the answers that need it with it.
    #[must_use]
    pub fn of(model: &str, url: Option<&str>) -> Self {
        let lane = ModelLane::ALL
            .into_iter()
            .find(|lane| lane.model() == model);
        let chat_model = lane.map(|lane| match lane {
            ModelLane::Gemini => "Google's Gemini 3.8 Flash".to_string(),
            ModelLane::Glm => "Z.ai's GLM 5.3 Flash".to_string(),
            ModelLane::SpaceBunny => "Space Bunny Alpha (an anonymous preview model)".to_string(),
        });
        let chat_model_keeps = lane
            .filter(|lane| *lane == ModelLane::SpaceBunny)
            .map(|_| keeps_sentence(ProviderPrivacy::from_env()).to_string());
        let chat_model_host = url
            .map(|url| url.trim_end_matches('/'))
            .and_then(|url| match url {
                DEFAULT_DOOR_URL => Some("the Vercel AI Gateway".to_string()),
                OPENROUTER_DOOR_URL => Some("OpenRouter".to_string()),
                _ => None,
            });
        Self {
            chat_model,
            chat_model_host,
            chat_model_keeps,
        }
    }
}

/// Where the judgment says the request belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Answerable in the chat: no files, repository, or commands.
    Chat,
    /// Needs a computer: a repository, files, commands, tests, or changes.
    Computer,
    /// The judgment has no read.
    Unknown,
}

impl Lane {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Lane::Chat => "chat",
            Lane::Computer => "computer",
            Lane::Unknown => "unknown",
        }
    }
}

/// The state the judgment reads: the same bounded shape Classify reads.
#[must_use]
pub fn state(task: &str, transcript: &[Message]) -> Value {
    crate::classify::state_of(task, transcript, &[])
}

/// A retry policy for a call that is only worth anything fast: one
/// attempt, bounded by [`LATE`].
#[must_use]
pub fn retry() -> RetryPolicy {
    RetryPolicy {
        max_retries: 0,
        budget: Some(LATE),
        ..RetryPolicy::default()
    }
}

fn choice<'a>(response: &'a SystemOneResponse, id: &str) -> Option<&'a ChoiceAnswer> {
    match response.answers.get(id) {
        Some(Answer::Choice(choice)) => Some(choice),
        _ => None,
    }
}

/// The most candidates one ranking takes.
pub const MAX_CANDIDATES: usize = 16;
/// The longest candidate id, in bytes.
pub const MAX_ID_BYTES: usize = 64;
/// The longest candidate label, in bytes.
pub const MAX_LABEL_BYTES: usize = 200;

/// One suggestion a caller wants ranked: a repository or an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    pub label: String,
}

/// Reads `candidates` from a request payload: an array of
/// `{ "id", "label" }`, bounded.
///
/// # Errors
///
/// Names what is wrong: not an array, too many, an empty, long, repeated,
/// or reserved id, or a long label.
pub fn candidates_of(value: &Value) -> Result<Vec<Candidate>, String> {
    let list = value
        .as_array()
        .ok_or("candidates is not an array of { id, label }")?;
    if list.is_empty() || list.len() > MAX_CANDIDATES {
        return Err(format!("candidates must name 1 to {MAX_CANDIDATES}"));
    }
    let mut out: Vec<Candidate> = Vec::with_capacity(list.len());
    for item in list {
        let id = item["id"].as_str().unwrap_or_default();
        let label = item["label"].as_str().unwrap_or(id);
        if id.is_empty() || id.len() > MAX_ID_BYTES || id == "none" {
            return Err(format!(
                "a candidate id must be 1 to {MAX_ID_BYTES} bytes and not `none`"
            ));
        }
        if label.len() > MAX_LABEL_BYTES {
            return Err(format!(
                "a candidate label must be at most {MAX_LABEL_BYTES} bytes"
            ));
        }
        if out.iter().any(|seen| seen.id == id) {
            return Err(format!("the candidate id {id} is repeated"));
        }
        out.push(Candidate {
            id: id.to_string(),
            label: label.to_string(),
        });
    }
    Ok(out)
}

/// The ranking question: which candidate the user most likely wants next.
#[must_use]
pub fn rank_questions(candidates: &[Candidate]) -> Questions {
    let mut options: IndexMap<String, Option<Entry>> = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.id.clone(),
                Some(Entry::from(candidate.label.clone())),
            )
        })
        .collect();
    options.insert(
        "none".to_string(),
        Some(Entry::from("None of these fits what the user is doing")),
    );
    Questions::new().with(
        "next",
        Choice::new(
            "Given the conversation so far, which of these repositories or actions is the user \
             most likely to want next?",
            options,
        ),
    )
}

/// The ranking request over a draft (possibly empty) and the transcript.
#[must_use]
pub fn rank_request(
    draft: &str,
    transcript: &[Message],
    candidates: &[Candidate],
) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(state(draft, transcript), rank_questions(candidates))
        .retry(retry())
        .timeout(BUDGET)
}

/// The candidates in the answer's order, most likely first, each with its
/// probability. A candidate the answer left out ranks last at zero, in the
/// caller's order; `none` is not a candidate and is dropped.
#[must_use]
pub fn ranking(response: &SystemOneResponse, candidates: &[Candidate]) -> Vec<(String, f64)> {
    let probabilities = choice(response, "next").map(|next| &next.probabilities);
    let mut ranked: Vec<(usize, String, f64)> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let p = probabilities
                .and_then(|p| p.get(&candidate.id))
                .copied()
                .filter(|p| p.is_finite())
                .unwrap_or(0.0);
            (index, candidate.id.clone(), p)
        })
        .collect();
    ranked.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    ranked.into_iter().map(|(_, id, p)| (id, p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response(answers: Value) -> SystemOneResponse {
        SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: json!({ "model": "jev", "answers": answers })
                .to_string()
                .into_bytes(),
        })
        .expect("a readable response")
    }

    /// The door's facts name the configured model and gateway, or nothing.
    #[test]
    fn door_facts_name_the_configured_model_or_nothing() {
        let gateway = Facts::of(ModelLane::Gemini.model(), Some(DEFAULT_DOOR_URL));
        assert_eq!(
            gateway.chat_model.as_deref(),
            Some("Google's Gemini 3.8 Flash")
        );
        assert_eq!(
            gateway.chat_model_host.as_deref(),
            Some("the Vercel AI Gateway")
        );
        let elsewhere = Facts::of(ModelLane::Gemini.model(), Some("http://127.0.0.1:9"));
        assert_eq!(elsewhere.chat_model_host, None);
        assert_eq!(Facts::of("some/other-model", None), Facts::default());
        assert_eq!(gateway.chat_model_keeps, None);

        // The primary names itself, its door, and what its provider keeps.
        let primary = Facts::of(ModelLane::SpaceBunny.model(), Some(OPENROUTER_DOOR_URL));
        assert_eq!(
            primary.chat_model.as_deref(),
            Some("Space Bunny Alpha (an anonymous preview model)")
        );
        assert_eq!(primary.chat_model_host.as_deref(), Some("OpenRouter"));
        assert!(
            primary
                .chat_model_keeps
                .as_deref()
                .is_some_and(|keeps| keeps.contains("train")),
            "{primary:?}"
        );
        // #11040: the sentence follows what the doors ask for.
        assert_eq!(
            keeps_sentence(ProviderPrivacy::Strict),
            "We ask the model providers not to keep or train on what we send them"
        );
        assert!(keeps_sentence(ProviderPrivacy::NoTraining).contains("may still keep"));
        assert!(keeps_sentence(ProviderPrivacy::Off).contains("not to train on them"));
    }

    #[test]
    fn candidates_are_bounded_and_ranked_by_probability() {
        assert!(candidates_of(&json!("repo")).is_err());
        assert!(candidates_of(&json!([])).is_err());
        assert!(candidates_of(&json!([{ "id": "none" }])).is_err());
        assert!(candidates_of(&json!([{ "id": "a" }, { "id": "a" }])).is_err());
        assert!(candidates_of(&json!([{ "id": "x".repeat(65) }])).is_err());
        let many: Vec<Value> = (0..17).map(|i| json!({ "id": format!("r{i}") })).collect();
        assert!(candidates_of(&Value::Array(many)).is_err());

        let candidates = candidates_of(&json!([
            { "id": "openagents", "label": "OpenAgentsInc/openagents" },
            { "id": "psionic", "label": "OpenAgentsInc/psionic" },
            { "id": "run_tests", "label": "Run the tests" },
        ]))
        .unwrap();
        rank_questions(&candidates).validate().expect("valid");
        let ranked = ranking(
            &response(json!({ "next": {
                "type": "choice", "choice": "psionic", "confidence": 0.6,
                "probabilities": { "openagents": 0.3, "psionic": 0.6, "run_tests": 0.0, "none": 0.1 }
            }})),
            &candidates,
        );
        let order: Vec<&str> = ranked.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(order, ["psionic", "openagents", "run_tests"]);
    }
}
