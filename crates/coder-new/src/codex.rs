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
pub const SIGN_IN_FIRST: &str = "Log in to Codex first";

/// What `coder chat --mode codex` says on a computer with no Codex login.
pub const NEEDS_LOGIN: &str =
    "Codex needs a ChatGPT login on this computer: run `codex login`, then try again.";

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

/// A new chat starts on the default model, not an earlier chat's pick
/// (#11556); the saved pick stays on disk.
pub fn reset_choice() {
    CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .0 = None;
}

/// A reopened chat goes on with the model it ran on (#11563): `choice`
/// holds for this run, and the person's saved pick on disk stays.
pub fn hold(choice: Choice) {
    CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .0 = Some(choice);
}

/// Reads the saved pick under `root`, Coder's settings folder.
pub fn load_choice(root: &Path) {
    let path = root.join(CHOICE_FILE);
    let saved = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ChoiceFile>(&bytes).ok())
        .filter(|file| file.version == 1 && offered(&file.model))
        .map(|file| Choice {
            effort: file
                .effort
                .filter(|effort| effort_offered(&file.model, effort)),
            model: file.model,
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
    let effort = effort.filter(|effort| effort_offered(model, effort));
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

/// Whether Codex runs `model` at `effort`: one of its usual efforts, or
/// one its own catalog lists for the model ("ultra").
fn effort_offered(model: &str, effort: &str) -> bool {
    EFFORTS.contains(&effort)
        || listed()
            .iter()
            .any(|listed| listed.id == model && listed.efforts.iter().any(|known| known == effort))
}

/// Whether /models offers `model` for Codex.
#[must_use]
pub fn offered(model: &str) -> bool {
    MODELS.iter().any(|(id, _, _)| *id == model) || listed().iter().any(|listed| listed.id == model)
}

/// What a person reads for `model`: its name in /models.
#[must_use]
pub fn name(model: &str) -> String {
    let bare = model.rsplit('/').next().unwrap_or(model);
    MODELS
        .iter()
        .find(|(id, _, _)| *id == bare)
        .map(|(_, name, _)| (*name).to_owned())
        .or_else(|| {
            listed()
                .into_iter()
                .find(|listed| listed.id == bare)
                .map(|listed| listed.name)
        })
        .unwrap_or_else(|| crate::models::label(model))
}

/// One model Codex's own picker lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub name: String,
    pub description: String,
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
}

/// The models Codex's own `/model` picker shows, from the catalog Codex
/// keeps (`$CODEX_HOME/models_cache.json`): the ones whose `visibility` is
/// `list`, in `priority` order, each with its reasoning levels. Ported from
/// Codex's `model_popups.rs` and `openai_models.rs` (Apache-2.0,
/// github.com/openai/codex): `show_in_picker` is `visibility == List`.
#[must_use]
pub fn parse_models_cache(text: &str) -> Vec<Listed> {
    let value: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    let mut models: Vec<(i64, Listed)> = value["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|model| model["visibility"].as_str() == Some("list"))
        .filter_map(|model| {
            let id = model["slug"].as_str()?.to_owned();
            Some((
                model["priority"].as_i64().unwrap_or(i64::MAX),
                Listed {
                    name: model["display_name"].as_str().unwrap_or(&id).to_owned(),
                    description: model["description"].as_str().unwrap_or_default().to_owned(),
                    efforts: model["supported_reasoning_levels"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|level| level["effort"].as_str().map(str::to_owned))
                        .collect(),
                    default_effort: model["default_reasoning_level"].as_str().map(str::to_owned),
                    id,
                },
            ))
        })
        .collect();
    models.sort_by_key(|(priority, _)| *priority);
    models.into_iter().map(|(_, model)| model).collect()
}

/// What Codex's own picker lists on this computer, else [`MODELS`] when
/// its catalog isn't there. A unit test reads no catalog.
#[must_use]
pub fn listed() -> Vec<Listed> {
    let cached = if cfg!(test) {
        Vec::new()
    } else {
        std::env::var_os("CODEX_HOME")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| user_home::home_os().map(|home| PathBuf::from(home).join(".codex")))
            .and_then(|dir| std::fs::read_to_string(dir.join("models_cache.json")).ok())
            .map(|text| parse_models_cache(&text))
            .unwrap_or_default()
    };
    if !cached.is_empty() {
        return cached;
    }
    MODELS
        .iter()
        .map(|(id, name, description)| Listed {
            id: (*id).into(),
            name: (*name).into(),
            description: (*description).into(),
            efforts: EFFORTS.iter().map(|effort| (*effort).to_owned()).collect(),
            default_effort: None,
        })
        .collect()
}

/// What /models lists for Codex: Codex's own picker list.
#[must_use]
pub fn catalog() -> Vec<crate::models::Model> {
    listed()
        .into_iter()
        .map(|model| crate::models::Model {
            plugin: PICKER.into(),
            provider: "OpenAI".into(),
            id: model.id,
            name: model.name,
            description: model.description,
            context_length: None,
            max_output_tokens: None,
            efforts: model.efforts,
            default_effort: model.default_effort,
            supports_output_limit: false,
            group: None,
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
        .or_else(|| user_home::home_os().map(|home| PathBuf::from(home).join(".codex")))
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

/// Codex's reasoning summary, asked for when the person's own config
/// names none (#11552): without it Codex reasons in silence and the chat
/// shows `Thinking` with no words.
pub const REASONING_SUMMARY: &str = "model_reasoning_summary=\"auto\"";

/// Whether the person's Codex config (`config.toml` under [`home`]) picks
/// a reasoning summary itself, `none` included.
fn summary_configured(home: Option<&Path>) -> bool {
    home.and_then(|home| std::fs::read_to_string(home.join("config.toml")).ok())
        .and_then(|text| text.parse::<toml::Table>().ok())
        .is_some_and(|table| table.contains_key("model_reasoning_summary"))
}

/// How a chat's app-server starts: the person's own config and login, with
/// `AGENTS.md` reading off when the chat carries a briefing (#11473), the
/// reasoning summary on unless the person's config says otherwise, the
/// engine mark, and no inherited API credentials.
#[must_use]
pub fn launch(program: PathBuf, cwd: &Path, briefed: bool) -> coder_sdk::codex::Launch {
    let mut config = Vec::new();
    if briefed {
        config.push("project_doc_max_bytes=0".to_owned());
    }
    if !summary_configured(home().as_deref()) {
        config.push(REASONING_SUMMARY.to_owned());
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
/// percent used). A window Codex leaves out (`null`, as `secondary` is on
/// a plan with one window) or reports without its length is not one.
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
            let window = limits.get(*key).filter(|window| window.is_object())?;
            let minutes = window["windowDurationMins"].as_u64().filter(|m| *m > 0)?;
            let resets = window["resetsAt"].as_u64().unwrap_or(u64::MAX);
            (resets > now).then(|| (minutes, window["usedPercent"].as_f64().unwrap_or(0.0)))
        })
        .collect()
}

/// Whether a Codex window is at its limit now.
#[must_use]
pub fn at_limit(now: u64) -> bool {
    windows(now).iter().any(|(_, used)| *used >= 100.0)
}

/// The windows as the rail shows them beside Codex's name: "5h 12% · week
/// 15%". Empty before Codex reported any. Codex rounds each window to a
/// whole percent, so a little use reads "<1%", not a flat 0%.
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
            if *used < 0.5 {
                format!("{span} <1%")
            } else {
                format!("{span} {}%", used.round() as u64)
            }
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
        // #11552: what codex-cli 0.162.1 sent on a Pro plan on 2026-10-13
        // (`account/rateLimits/updated`): one weekly window, `secondary`
        // null. The null window is no "0d 0%", and a rounded 0 is "<1%".
        observe_limits(
            &serde_json::json!({"limitId": "codex", "limitName": null,
                "primary": {"usedPercent": 0, "windowDurationMins": 10080, "resetsAt": 1_792_480_941},
                "secondary": null, "planType": "pro"}),
            1_791_916_188,
        );
        assert_eq!(windows_text(1_791_916_188), "week <1%");
        assert!(!at_limit(1_791_916_188));
        observe_limits(
            &serde_json::json!({"primary": {"usedPercent": 12.4, "windowDurationMins": 300, "resetsAt": 2_000},
                "secondary": {"usedPercent": 100, "windowDurationMins": 10_080, "resetsAt": 9_000}}),
            1_000,
        );
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

    /// #11552: Codex's reasoning summary is asked for unless the person's
    /// own config picks one.
    #[test]
    fn the_reasoning_summary_is_asked_for_unless_configured() {
        let home = tempfile::tempdir().unwrap();
        assert!(!summary_configured(Some(home.path())));
        assert!(!summary_configured(None));
        std::fs::write(home.path().join("config.toml"), "model = \"gpt-6.1-sol\"\n").unwrap();
        assert!(!summary_configured(Some(home.path())));
        std::fs::write(
            home.path().join("config.toml"),
            "model_reasoning_summary = \"concise\"\n",
        )
        .unwrap();
        assert!(summary_configured(Some(home.path())));
    }
}
