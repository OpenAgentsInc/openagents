//! Auto's policy when no coding harness can run (#11548).
//!
//! A person with no Claude Code, Codex, OpenCode, or ACP agent (Grok
//! Build, Devin, Cursor...) installed and turned on still gets a coding
//! agent: Auto runs Coder's own loop with Coder's own tools (read, edit,
//! write, bash, grep and the rest), and every model call of the turn
//! (planning, each tool step, edits, the answer, a cheap answer, and a
//! compaction summary) goes to one model, [`NoHarnessModel`], by default
//! Gemini 4 Argon. System One's judgments (route, brief, squeeze, gate,
//! watch, verify) are not model calls of the turn: they keep asking the
//! Decisions API, Clef first.
//!
//! The policy is data, read once from the environment, so the host makes
//! the decision in one place:
//!
//! - [`AutoPolicy::engine`] says whether a turn runs on the harnesses or on
//!   the Coder loop with the no-harness model;
//! - [`AutoPolicy::model_for`] names the model for each kind of call;
//! - [`AutoPolicy::doors`] orders the ways to reach that model: the
//!   OpenAgents gateway while signed in, then the person's own Gemini API
//!   key.
//!
//! `CODER_NO_HARNESS=1` forces the mode on a computer that has harnesses
//! (to test it, or to keep every turn on one model): the harness plugins
//! count as off for the run, nothing is saved. `CODER_NO_HARNESS_MODEL`
//! names another gateway model.

use serde::{Deserialize, Serialize};

/// Gemini 4 Argon's gateway id.
pub const ARGON: &str = "google/gemini-4-argon";

/// The variable that forces no-harness mode for a run.
pub const FORCE_VAR: &str = "CODER_NO_HARNESS";

/// The variable that names the no-harness model, a gateway id
/// (`publisher/model`).
pub const MODEL_VAR: &str = "CODER_NO_HARNESS_MODEL";

/// The variables that hold the person's own Gemini API key, in the order
/// they are read.
pub const GEMINI_KEY_VARS: [&str; 2] = ["GEMINI_API_KEY", "GOOGLE_API_KEY"];

/// The Gemini API's OpenAI-compatible endpoint, for the own-key door.
pub const GEMINI_OPENAI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/openai";

/// The model every call of a no-harness turn runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoHarnessModel(String);

impl Default for NoHarnessModel {
    fn default() -> Self {
        Self(ARGON.to_owned())
    }
}

impl NoHarnessModel {
    /// A model by gateway id. A blank id is Argon.
    #[must_use]
    pub fn new(id: &str) -> Self {
        let id = id.trim();
        if id.is_empty() {
            Self::default()
        } else {
            Self(id.to_owned())
        }
    }

    /// The gateway id, `google/gemini-4-argon`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.0
    }

    /// The id a provider's own API takes: the gateway id without its
    /// publisher, `gemini-4-argon`.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// Whether a model a reply named is this one: replies through the
    /// gateway name the gateway id, replies through the Gemini API the
    /// provider's.
    #[must_use]
    pub fn is(&self, served: &str) -> bool {
        let served = served.trim();
        served == self.0 || served == self.provider_id()
    }

    /// Whether the person's own Gemini API key can reach it.
    #[must_use]
    pub fn on_gemini_api(&self) -> bool {
        self.0.starts_with("google/gemini-")
    }

    /// The model's context window when it is known here, input and output
    /// together: Argon's is one million tokens (1,048,576).
    #[must_use]
    pub fn window(&self) -> Option<u64> {
        window(self.id())
    }
}

/// The context window of a model this module knows, by gateway or
/// provider id.
#[must_use]
pub fn window(model: &str) -> Option<u64> {
    let model = model.trim();
    let model = model.rsplit('/').next().unwrap_or(model);
    match model {
        "gemini-4-argon" => Some(1_048_576),
        _ => None,
    }
}

/// The kind of model call in a no-harness turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Call {
    /// The turn's work: planning, every tool step, edits, the answer.
    Work,
    /// A small answer the router judged needs no repository.
    CheapAnswer,
    /// Summarizing a long chat to fit its window.
    Compaction,
}

/// What runs a turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Engine {
    /// The harnesses Auto chooses among, as before.
    Harnesses,
    /// Coder's own loop with Coder's own tools, every call on this model.
    CoderLoop(NoHarnessModel),
}

/// A way to reach the no-harness model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Door {
    /// The OpenAgents gateway as the signed-in account (Vertex AI).
    Gateway,
    /// The person's own Gemini API key on this computer.
    GeminiKey,
}

/// Auto's settings for a computer with no usable harness.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoPolicy {
    /// The model every call runs on.
    pub no_harness: NoHarnessModel,
    /// Treat every harness as off for this run.
    pub force_no_harness: bool,
}

/// Whether a variable's value turns a switch on: `1`, `true`, `on`, `yes`.
#[must_use]
pub fn switched_on(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}

impl AutoPolicy {
    /// The policy `get` (an environment) sets: [`FORCE_VAR`] and
    /// [`MODEL_VAR`].
    #[must_use]
    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            no_harness: get(MODEL_VAR)
                .map(|id| NoHarnessModel::new(&id))
                .unwrap_or_default(),
            force_no_harness: get(FORCE_VAR).is_some_and(|value| switched_on(&value)),
        }
    }

    /// The policy this process's environment sets.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// What runs a turn when `harness_usable` says whether any harness
    /// (Claude Code, Codex, OpenCode) is on, installed, signed in, and
    /// under its limits. A forced policy never uses one.
    #[must_use]
    pub fn engine(&self, harness_usable: bool) -> Engine {
        if harness_usable && !self.force_no_harness {
            Engine::Harnesses
        } else {
            Engine::CoderLoop(self.no_harness.clone())
        }
    }

    /// The model for `call` in a no-harness turn: the one model, for
    /// everything (owner, 2026-10-13).
    #[must_use]
    pub fn model_for(&self, call: Call) -> &str {
        match call {
            Call::Work | Call::CheapAnswer | Call::Compaction => self.no_harness.id(),
        }
    }

    /// The doors to the model, in the order tried: the gateway first while
    /// signed in (Google-first, on the account's allowance or credit), then
    /// the person's own Gemini API key.
    #[must_use]
    pub fn doors(&self, signed_in: bool, gemini_key: bool) -> Vec<Door> {
        let mut doors = Vec::new();
        if signed_in {
            doors.push(Door::Gateway);
        }
        if gemini_key && self.no_harness.on_gemini_api() {
            doors.push(Door::GeminiKey);
        }
        doors
    }
}

/// The person's own Gemini API key from `get` (an environment), when one
/// is set.
#[must_use]
pub fn gemini_key(get: impl Fn(&str) -> Option<String>) -> Option<String> {
    GEMINI_KEY_VARS
        .iter()
        .filter_map(|name| get(name))
        .map(|key| key.trim().to_owned())
        .find(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn argon_is_the_default_and_runs_every_call() {
        let policy = AutoPolicy::from_vars(vars(&[]));
        assert!(!policy.force_no_harness);
        assert_eq!(policy.no_harness.id(), ARGON);
        assert_eq!(policy.no_harness.provider_id(), "gemini-4-argon");
        for call in [Call::Work, Call::CheapAnswer, Call::Compaction] {
            assert_eq!(policy.model_for(call), ARGON);
        }
        assert_eq!(policy.no_harness.window(), Some(1_048_576));
        assert!(policy.no_harness.is("gemini-4-argon"));
        assert!(policy.no_harness.is("google/gemini-4-argon"));
        assert!(!policy.no_harness.is("google/gemini-3.8-flash"));
    }

    #[test]
    fn harnesses_run_when_one_is_usable_unless_forced() {
        let policy = AutoPolicy::default();
        assert_eq!(policy.engine(true), Engine::Harnesses);
        assert_eq!(
            policy.engine(false),
            Engine::CoderLoop(NoHarnessModel::default())
        );
        let forced = AutoPolicy::from_vars(vars(&[(FORCE_VAR, "1")]));
        assert!(forced.force_no_harness);
        assert_eq!(
            forced.engine(true),
            Engine::CoderLoop(NoHarnessModel::default())
        );
        assert!(!AutoPolicy::from_vars(vars(&[(FORCE_VAR, "0")])).force_no_harness);
    }

    #[test]
    fn the_model_is_configurable() {
        let policy = AutoPolicy::from_vars(vars(&[(MODEL_VAR, "google/gemini-3.8-flash")]));
        assert_eq!(policy.model_for(Call::Work), "google/gemini-3.8-flash");
        assert_eq!(policy.no_harness.window(), None);
        let blank = AutoPolicy::from_vars(vars(&[(MODEL_VAR, "  ")]));
        assert_eq!(blank.no_harness.id(), ARGON);
    }

    #[test]
    fn the_gateway_comes_before_the_own_key() {
        let policy = AutoPolicy::default();
        assert_eq!(policy.doors(true, true), [Door::Gateway, Door::GeminiKey]);
        assert_eq!(policy.doors(false, true), [Door::GeminiKey]);
        assert_eq!(policy.doors(true, false), [Door::Gateway]);
        assert!(policy.doors(false, false).is_empty());
        // A model Google doesn't serve is reached only through the gateway.
        let other = AutoPolicy {
            no_harness: NoHarnessModel::new("zai/glm-5.3-flash"),
            force_no_harness: false,
        };
        assert_eq!(other.doors(false, true), Vec::<Door>::new());
    }

    #[test]
    fn the_own_key_is_read_from_either_variable() {
        assert_eq!(gemini_key(vars(&[])), None);
        assert_eq!(gemini_key(vars(&[("GEMINI_API_KEY", " ")])), None);
        assert_eq!(
            gemini_key(vars(&[("GOOGLE_API_KEY", "k2"), ("GEMINI_API_KEY", "k1")])),
            Some("k1".into())
        );
        assert_eq!(
            gemini_key(vars(&[("GOOGLE_API_KEY", "k2")])),
            Some("k2".into())
        );
    }
}
