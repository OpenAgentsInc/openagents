//! Codex as a harness (#11510): chat turns on the ChatGPT login this
//! computer holds, through `codex app-server` and the Coder SDK's Codex
//! backend (`coder_sdk::codex`).
//!
//! A chat on Codex keeps one long-lived app-server and one Codex thread
//! ([`crate::codex_chat`]); follow-ups continue that thread. Codex runs
//! under approvals, never its bypass flags: each command and patch is
//! answered by the session's hooks ([`crate::sdk_hooks::codex`]), and a
//! held step asks the person on the approval card while Codex waits.
//! Coder's own tools (background agents, Solver, cloud jobs) ride along as
//! Codex dynamic tools, in process. The model is the one picked in /models
//! for Codex ([`choice`]).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

/// The plugin that offers Codex, in /plugins and `bundled-plugins.json`.
pub const PLUGIN: &str = "codex";

/// What /plugins says when the plugin cannot turn on here.
pub const SIGN_IN_FIRST: &str = "Sign in to Codex first";

/// What `coder chat --mode codex` says on a computer with no Codex login.
pub const NEEDS_LOGIN: &str =
    "Codex needs a ChatGPT sign-in on this computer: run `codex login`, then try again.";

/// The picker rows' plugin name, so /models knows a Codex row.
pub const PICKER: &str = "codex";

/// The model a Codex turn runs before the person picks one.
pub const MODEL: &str = "gpt-6.1-sol";

/// The models /models offers for Codex, [`MODEL`] first: id, name, and
/// what the row says. Codex's own list (`model/list`, codex-cli 0.162.1).
pub const MODELS: [(&str, &str, &str); 4] = [
    (
        MODEL,
        "GPT-6.1-Sol",
        "Codex's workhorse for coding and everyday work",
    ),
    (
        "gpt-6-astra",
        "GPT-6-Astra",
        "Frontier intelligence for the most demanding work",
    ),
    ("gpt-6-sol", "GPT-6-Sol", "The previous workhorse"),
    (
        "gpt-6-luna",
        "GPT-6-Luna",
        "Fast and affordable for easier tasks",
    ),
];

/// The efforts a picked model may run at, as Codex names them.
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// Where the person's pick is kept, beside Coder's other settings.
const CHOICE_FILE: &str = "codex-model.json";

/// The model and effort Codex turns run, as picked in /models.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub model: String,
    /// Codex's reasoning effort, or `None` for the model's own default.
    pub effort: Option<String>,
}

impl Default for Choice {
    fn default() -> Self {
        Self {
            model: MODEL.into(),
            effort: None,
        }
    }
}

static CHOICE: Mutex<(Option<Choice>, Option<PathBuf>)> = Mutex::new((None, None));

#[derive(serde::Deserialize, serde::Serialize)]
struct ChoiceFile {
    version: u32,
    model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    effort: Option<String>,
}

/// The model and effort the next Codex turn runs.
#[must_use]
pub fn choice() -> Choice {
    CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .0
        .clone()
        .unwrap_or_default()
}

/// Reads the saved pick under `root`, Coder's settings folder.
pub fn load_choice(root: &Path) {
    let path = root.join(CHOICE_FILE);
    let saved = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ChoiceFile>(&bytes).ok())
        .filter(|file| file.version == 1 && offered(&file.model))
        .map(|file| Choice {
            model: file.model,
            effort: file
                .effort
                .filter(|effort| EFFORTS.contains(&effort.as_str())),
        });
    *CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = (saved, Some(path));
}

/// Makes `model` at `effort` the Codex pick and saves it.
///
/// # Errors
/// Codex does not offer `model`, or the pick could not be saved.
pub fn choose(model: &str, effort: Option<String>) -> Result<(), String> {
    if !offered(model) {
        return Err(format!("Codex does not offer {model}."));
    }
    let effort = effort.filter(|effort| EFFORTS.contains(&effort.as_str()));
    let mut held = CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(path) = &held.1 {
        let file = ChoiceFile {
            version: 1,
            model: model.into(),
            effort: effort.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&file).map_err(|error| error.to_string())?;
        std::fs::write(path, bytes)
            .map_err(|_| "Cannot save the Codex model. The earlier one stays.".to_string())?;
    }
    held.0 = Some(Choice {
        model: model.into(),
        effort,
    });
    Ok(())
}

/// The model a Codex turn runs on: a pin's `model`, else the pick.
#[must_use]
pub fn turn_model(pinned: Option<String>) -> String {
    pinned
        .filter(|model| offered(model))
        .unwrap_or_else(|| choice().model)
}

/// Whether /models offers `model` for Codex.
#[must_use]
pub fn offered(model: &str) -> bool {
    MODELS.iter().any(|(id, _, _)| *id == model)
}

/// What a person reads for `model`: its name in /models.
#[must_use]
pub fn name(model: &str) -> String {
    let bare = model.rsplit('/').next().unwrap_or(model);
    MODELS.iter().find(|(id, _, _)| *id == bare).map_or_else(
        || crate::models::label(model),
        |(_, name, _)| (*name).to_owned(),
    )
}

/// What /models lists for Codex.
#[must_use]
pub fn catalog() -> Vec<crate::models::Model> {
    MODELS
        .iter()
        .map(|(id, name, description)| crate::models::Model {
            plugin: PICKER.into(),
            provider: "OpenAI".into(),
            id: (*id).into(),
            name: (*name).into(),
            description: (*description).into(),
            context_length: None,
            max_output_tokens: None,
            efforts: EFFORTS.iter().map(|effort| (*effort).to_owned()).collect(),
            default_effort: None,
            supports_output_limit: false,
        })
        .collect()
}

/// The `codex` program this computer runs, if any.
#[must_use]
pub fn program() -> Option<PathBuf> {
    #[cfg(test)]
    {
        test_program()
    }
    #[cfg(not(test))]
    crate::acp_discovery::resolve(Path::new("codex"), &|name| std::env::var_os(name))
}

/// A test's `codex`: set only by a live test run by hand, and shared with
/// the chat's own thread.
#[cfg(test)]
static TEST_PROGRAM: Mutex<Option<PathBuf>> = Mutex::new(None);

#[cfg(test)]
fn test_program() -> Option<PathBuf> {
    TEST_PROGRAM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Makes the test process hold a `codex` at `program`, or none.
#[cfg(test)]
pub(crate) fn set_test_program(program: Option<PathBuf>) {
    *TEST_PROGRAM
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = program;
}

/// Codex's home: `CODEX_HOME`, else `~/.codex`.
#[must_use]
pub fn home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
}

static AVAILABLE: Mutex<Option<bool>> = Mutex::new(None);

/// Whether Codex can take chat turns here: the CLI is installed and holds a
/// sign-in (`auth.json` under its home). Read once and kept; a turn that
/// will not start calls [`recheck`]. A unit test never asks the computer.
#[must_use]
pub fn available() -> bool {
    #[cfg(test)]
    {
        test_program().is_some()
    }
    #[cfg(not(test))]
    {
        let mut cache = AVAILABLE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(known) = *cache {
            return known;
        }
        let known =
            program().is_some() && home().is_some_and(|home| home.join("auth.json").is_file());
        *cache = Some(known);
        known
    }
}

/// Forget the cached answer.
pub fn recheck() {
    *AVAILABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// How a chat's app-server starts: the person's own config and login, with
/// `AGENTS.md` reading off when the chat carries a briefing (#11473), the
/// engine mark, and no inherited API credentials.
#[must_use]
pub fn launch(program: PathBuf, cwd: &Path, briefed: bool) -> coder_sdk::codex::Launch {
    let mut config = Vec::new();
    if briefed {
        config.push("project_doc_max_bytes=0".to_owned());
    }
    let (mark, value) = coder_delegate::delegate::Agent::Codex.engine_mark();
    let mut env = vec![(mark.to_owned(), value.to_owned())];
    env.extend(
        crate::bundled_runtime::child_env()
            .into_iter()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            }),
    );
    coder_sdk::codex::Launch {
        program,
        config,
        cwd: Some(cwd.to_owned()),
        // A test's stand-in program is no app-server: it falls back fast.
        start_wait: cfg!(test).then(|| std::time::Duration::from_millis(500)),
        env,
        env_remove: std::env::vars_os()
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .filter(|name| acp_client::process::is_credential_name(name))
            .collect(),
    }
}

/// Where Codex's plan windows stand, as it last reported them, and when.
static PLAN: Mutex<Option<(Value, u64)>> = Mutex::new(None);

/// Keeps Codex's rate-limit report (`account/rateLimits/updated`).
pub fn observe_limits(limits: &Value, now: u64) {
    *PLAN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((limits.clone(), now));
}

/// Codex's windows that have not reset yet: (minutes the window spans,
/// percent used).
#[must_use]
pub fn windows(now: u64) -> Vec<(u64, f64)> {
    let held = PLAN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let Some((limits, _)) = held else {
        return Vec::new();
    };
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let window = limits.get(*key)?;
            let resets = window["resetsAt"].as_u64().unwrap_or(u64::MAX);
            (resets > now).then(|| {
                (
                    window["windowDurationMins"].as_u64().unwrap_or(0),
                    window["usedPercent"].as_f64().unwrap_or(0.0),
                )
            })
        })
        .collect()
}

/// Whether a Codex window is at its limit now.
#[must_use]
pub fn at_limit(now: u64) -> bool {
    windows(now).iter().any(|(_, used)| *used >= 100.0)
}

/// The windows as the rail shows them beside Codex's name: "5h 12% · week
/// 15%". Empty before Codex reported any.
#[must_use]
pub fn windows_text(now: u64) -> String {
    windows(now)
        .iter()
        .map(|(minutes, used)| {
            let span = match minutes {
                300 => "5h".to_owned(),
                10_080 => "week".to_owned(),
                m if m % 1_440 == 0 => format!("{}d", m / 1_440),
                m if m % 60 == 0 => format!("{}h", m / 60),
                m => format!("{m}m"),
            };
            format!("{span} {}%", used.round() as u64)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// "Codex" and, in parentheses, the model a Codex turn runs, as /models
/// names it.
#[must_use]
pub fn rail_label() -> String {
    format!("Codex ({})", name(&choice().model))
}

/// The most of a conversation a new thread's first message carries.
const SEED_LIMIT: usize = 48 * 1024;

/// A new thread's first message: the person's message alone in a new chat,
/// else the conversation so far, so a chat that moves to Codex (or whose
/// thread Codex lost) goes on where it was. Coder's identity and tools are
/// in the thread's developer instructions, not here.
#[must_use]
pub fn seed(messages: &[openrouter::Message]) -> String {
    match messages {
        [] => String::new(),
        [only] => only.content.clone(),
        _ => format!(
            "The conversation so far, ending with the person's newest message:\n{}",
            crate::live::local_conversation(messages, SEED_LIMIT)
        ),
    }
}

/// `message` led by a note that the person switched the chat to `model`.
#[must_use]
pub fn with_switch(model: &str, message: &str) -> String {
    let name = name(model);
    format!(
        "<model-switch>\nThe person switched this chat to {name}. From this message on you run on {name}: when asked which model runs you, name {name}.\n</model-switch>\n\n{message}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_read_as_the_rail_shows_them() {
        observe_limits(
            &serde_json::json!({"primary": {"usedPercent": 12.4, "windowDurationMins": 300, "resetsAt": 2_000},
                "secondary": {"usedPercent": 100, "windowDurationMins": 10_080, "resetsAt": 9_000}}),
            1_000,
        );
        assert_eq!(windows_text(1_000), "5h 12% · week 100%");
        assert!(at_limit(1_000));
        // A window past its reset is gone.
        assert_eq!(windows_text(5_000), "week 100%");
        assert!(!at_limit(10_000));
    }

    #[test]
    fn models_and_launch() {
        assert_eq!(name("gpt-6-astra"), "GPT-6-Astra");
        assert_eq!(turn_model(Some("made-up".into())), choice().model);
        assert!(catalog().iter().all(|model| model.plugin == PICKER));
        let launch = launch(PathBuf::from("codex"), Path::new("/tmp"), true);
        assert!(
            launch
                .config
                .contains(&"project_doc_max_bytes=0".to_owned())
        );
        assert!(!launch.args().iter().any(|arg| arg.contains("bypass")));
    }
}
