//! The person's routing preferences (#11455, #11451 phase 1): which
//! engines Coder should lean on, lean away from, or never use, kept in one
//! Markdown file the person can read and edit.
//!
//! The file is `routing-preferences.md` in Coder's settings folder. It
//! opens with typed settings between `+++` lines (TOML), and the rest is
//! free Markdown notes:
//!
//! ```text
//! +++
//! version = 1
//!
//! [harness]
//! claude-code = "avoid"
//!
//! [[never]]
//! harness = "coder-loop"
//! model = "deepseek/deepseek-v4"
//!
//! [[change]]
//! at = "2026-10-12T18:00:00.000Z"
//! by = "model"
//! harness = "claude-code"
//! from = "neutral"
//! to = "avoid"
//! said = "stop using Claude so much"
//! +++
//! # Routing preferences
//! ...notes...
//! ```
//!
//! The model changes it through one typed tool, [`TOOL`], which it calls
//! when the person's words ask for it; nothing here reads the person's
//! words for meaning. Every change appends an entry with the time, who
//! made it, the stance before and after, and the person's words, so any
//! change can be undone by another entry that restores the earlier stance.
//!
//! Selection reads it ([`crate::harness_plugin::candidates`]): `never`
//! leaves a harness out with its own exclusion, as does a `never` pair of
//! a harness and the exact model it would run; `prefer` and `avoid`
//! reorder the harnesses that remain.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::harness_plugin::Harness;

/// The file's name in Coder's settings folder.
pub const FILENAME: &str = "routing-preferences.md";
/// The tool the model calls to change it.
pub const TOOL: &str = "routing_preference";
const VERSION: u32 = 1;
const FENCE: &str = "+++";
const NOTES: &str = "# Routing preferences\n\nWhich engines Coder leans on when it picks one for a turn. Coder changes the settings above when you ask it to use an engine more, less, or never, and lists every change with your words, so any of them can be undone. You can edit this file yourself; write your own notes below.\n";
const READ_ERROR: &str =
    "Cannot read your routing preferences. Fix or remove the file; it was not changed.";
const WRITE_ERROR: &str = "Cannot save your routing preferences. The file was not changed.";

/// The repository `cwd` is in, by its folder's name (the main checkout's
/// for a worktree), for pins that apply in one repository.
#[must_use]
pub fn repo_name(cwd: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let common = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let top = if common.file_name().is_some_and(|name| name == ".git") {
        common.parent()?.to_path_buf()
    } else {
        common
    };
    top.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// Where the routing preferences live under Coder's settings folder `root`.
#[must_use]
pub fn path(root: &Path) -> PathBuf {
    root.join(FILENAME)
}

/// How the person wants an engine used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stance {
    /// Try it before the others.
    Prefer,
    /// No preference: the standing order.
    #[default]
    Neutral,
    /// Try it after the others.
    Avoid,
    /// Never use it.
    Never,
}

impl Stance {
    /// Where a harness with this stance sorts among the admitted ones:
    /// lower first. `Never` never reaches the sort.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Prefer => 0,
            Self::Neutral => 1,
            Self::Avoid => 2,
            Self::Never => 3,
        }
    }

    /// The words a person reads for it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Prefer => "use first",
            Self::Neutral => "no preference",
            Self::Avoid => "use less",
            Self::Never => "never use",
        }
    }
}

/// One value per harness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    deny_unknown_fields,
    bound(
        serialize = "T: Serialize + Default + PartialEq",
        deserialize = "T: Deserialize<'de> + Default"
    )
)]
pub struct PerHarness<T> {
    #[serde(default, skip_serializing_if = "is_default")]
    pub claude_code: T,
    #[serde(default, skip_serializing_if = "is_default")]
    pub coder_loop: T,
    #[serde(default, skip_serializing_if = "is_default")]
    pub codex: T,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

impl<T: Copy> PerHarness<T> {
    /// The value for `harness`.
    #[must_use]
    pub fn get(&self, harness: Harness) -> T {
        match harness {
            Harness::ClaudeCode => self.claude_code,
            Harness::CoderLoop => self.coder_loop,
            Harness::Codex => self.codex,
        }
    }

    /// Sets the value for `harness`.
    pub fn set(&mut self, harness: Harness, value: T) {
        match harness {
            Harness::ClaudeCode => self.claude_code = value,
            Harness::CoderLoop => self.coder_loop = value,
            Harness::Codex => self.codex = value,
        }
    }
}

/// An exact harness and model the person never wants used together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeverPair {
    pub harness: Harness,
    pub model: String,
}

/// A pin (#11475): this harness, and with `model` that model, for the
/// person's work, in one repository (`repo`, its folder's name) or one kind
/// of work (`kind`), or both, or everywhere. A pin decides ahead of the
/// router's own steps and records as `Rule::Pin`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub harness: Harness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::router::Kind>,
    /// The person's words that asked for it.
    pub said: String,
    pub at: String,
}

impl Pin {
    /// Whether the pin applies in `repo`.
    #[must_use]
    pub fn applies_in(&self, repo: Option<&str>) -> bool {
        self.repo.is_none() || self.repo.as_deref() == repo
    }

    /// The pin in plain words, for the preferences screen.
    #[must_use]
    pub fn words(&self) -> String {
        let agent = match &self.model {
            Some(model) => format!("{} ({model})", self.harness.declaration().name),
            None => self.harness.declaration().name.to_owned(),
        };
        let kind = self.kind.map(|kind| match kind {
            crate::router::Kind::Answer => "questions and answers",
            crate::router::Kind::RepoChange => "changes to code",
            crate::router::Kind::IssueToPr => "issues to pull requests",
            crate::router::Kind::Research => "research",
        });
        let place = self
            .repo
            .as_ref()
            .map_or_else(|| "everywhere".to_owned(), |repo| format!("in {repo}"));
        match kind {
            Some(kind) => format!("Always {agent} for {kind}, {place}"),
            None => format!("Always {agent}, {place}"),
        }
    }
}

/// Who made a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Who {
    /// The person, from a screen of Coder's.
    Person,
    /// The model, through [`TOOL`], from the person's words.
    Model,
}

/// One recorded change. With `model`, it sets or clears a never pair
/// (`from` and `to` are then `neutral` or `never`); without, it sets the
/// harness's stance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub at: String,
    pub by: Who,
    pub harness: Harness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub from: Stance,
    pub to: Stance,
    /// The person's words that asked for it.
    pub said: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The change (counting from one) this one undoes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undoes: Option<usize>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Front {
    version: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    harness: PerHarness<Stance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    never: Vec<NeverPair>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pin: Vec<Pin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    change: Vec<Change>,
}

/// The whole document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingPreferences {
    pub stances: PerHarness<Stance>,
    pub never: Vec<NeverPair>,
    /// The person's pins (#11475).
    pub pins: Vec<Pin>,
    pub changes: Vec<Change>,
    /// The Markdown after the settings: the person's notes, and any the
    /// model added for them.
    pub notes: String,
}

impl Default for RoutingPreferences {
    fn default() -> Self {
        Self {
            stances: PerHarness::default(),
            never: Vec::new(),
            pins: Vec::new(),
            changes: Vec::new(),
            notes: NOTES.to_owned(),
        }
    }
}

/// What the model asks [`TOOL`] to do.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default)]
    harness: Option<Harness>,
    #[serde(default)]
    stance: Option<Stance>,
    #[serde(default)]
    model: Option<String>,
    said: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    undo: bool,
    /// Pin the harness (with `model`) here (#11475).
    #[serde(default)]
    pin: bool,
    /// Remove the pins on the harness.
    #[serde(default)]
    unpin: bool,
    /// The pin applies only in this repository.
    #[serde(default)]
    this_repo: bool,
    #[serde(default)]
    kind: Option<crate::router::Kind>,
}

impl RoutingPreferences {
    /// Parses the file's text. Text with no settings block is all notes.
    ///
    /// # Errors
    /// The settings block is unclosed, invalid, or from a newer Coder.
    pub fn parse(text: &str) -> Result<Self, String> {
        let Some(rest) = text
            .strip_prefix(FENCE)
            .and_then(|rest| rest.strip_prefix('\n').or(rest.strip_prefix("\r\n")))
        else {
            return Ok(Self {
                notes: text.to_owned(),
                ..Self::default()
            });
        };
        let (front, notes) = if let Some(notes) = rest.strip_prefix(FENCE) {
            ("", notes)
        } else {
            let end = rest
                .find(&format!("\n{FENCE}"))
                .ok_or_else(|| READ_ERROR.to_owned())?;
            (&rest[..=end], &rest[end + 1 + FENCE.len()..])
        };
        let notes = notes
            .strip_prefix("\r\n")
            .or(notes.strip_prefix('\n'))
            .unwrap_or(notes);
        let front: Front = toml::from_str(front).map_err(|_| READ_ERROR.to_owned())?;
        if front.version != VERSION
            || front.never.iter().any(|pair| !well_formed(&pair.model))
            || front.pin.iter().any(|pin| {
                pin.model
                    .as_deref()
                    .is_some_and(|model| !well_formed(model))
            })
        {
            return Err(READ_ERROR.into());
        }
        Ok(Self {
            stances: front.harness,
            never: front.never,
            pins: front.pin,
            changes: front.change,
            notes: notes.to_owned(),
        })
    }

    /// The file's text.
    #[must_use]
    pub fn render(&self) -> String {
        let front = Front {
            version: VERSION,
            harness: self.stances,
            never: self.never.clone(),
            pin: self.pins.clone(),
            change: self.changes.clone(),
        };
        let front = toml::to_string(&front).unwrap_or_else(|_| format!("version = {VERSION}\n"));
        format!("{FENCE}\n{front}{FENCE}\n{}", self.notes)
    }

    /// Reads the file at `path`; a missing file is the default.
    ///
    /// # Errors
    /// The file exists and cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err(READ_ERROR.into()),
        }
    }

    /// Writes the file at `path` in one step, readable only by its owner.
    ///
    /// # Errors
    /// The folder or the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let write = || -> std::io::Result<()> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let staged = path.with_extension(format!("md.{}.tmp", std::process::id()));
            std::fs::write(&staged, self.render())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o600))?;
            }
            std::fs::rename(&staged, path)
        };
        write().map_err(|_| WRITE_ERROR.to_owned())
    }

    /// Whether the person said never to run `model` on `harness`.
    #[must_use]
    pub fn never_with(&self, harness: Harness, model: &str) -> bool {
        self.never
            .iter()
            .any(|pair| pair.harness == harness && pair.model == model)
    }

    /// The stance `harness` (with `model`, for a pair) has now.
    fn current(&self, harness: Harness, model: Option<&str>) -> Stance {
        match model {
            Some(model) if self.never_with(harness, model) => Stance::Never,
            Some(_) => Stance::Neutral,
            None => self.stances.get(harness),
        }
    }

    fn put(&mut self, harness: Harness, model: Option<&str>, stance: Stance) {
        match model {
            Some(model) => {
                self.never
                    .retain(|pair| !(pair.harness == harness && pair.model == model));
                if stance == Stance::Never {
                    self.never.push(NeverPair {
                        harness,
                        model: model.to_owned(),
                    });
                }
            }
            None => self.stances.set(harness, stance),
        }
    }

    /// Sets `harness`'s stance, or with `model` sets or clears that exact
    /// pair, and records the change.
    ///
    /// # Errors
    /// A pair takes only `never` or `neutral`, and a model name must be one
    /// word.
    pub fn change(
        &mut self,
        by: Who,
        harness: Harness,
        model: Option<&str>,
        to: Stance,
        said: &str,
        reason: Option<&str>,
    ) -> Result<&Change, String> {
        if let Some(model) = model {
            if !valid_model(model) {
                return Err(unknown_model(model));
            }
            if !matches!(to, Stance::Never | Stance::Neutral) {
                return Err(
                    "A model on an engine can only be never used, or allowed again.".into(),
                );
            }
        }
        let from = self.current(harness, model);
        self.put(harness, model, to);
        self.changes.push(Change {
            at: atif::iso(atif::now_ms()),
            by,
            harness,
            model: model.map(str::to_owned),
            from,
            to,
            said: said.trim().to_owned(),
            reason: reason
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_owned),
            undoes: None,
        });
        Ok(self.changes.last().expect("just pushed"))
    }

    /// Undoes the most recent change not already undone, restoring the
    /// stance it replaced, and records that as a change of its own.
    ///
    /// # Errors
    /// There is nothing left to undo.
    pub fn undo(&mut self, by: Who, said: &str) -> Result<&Change, String> {
        let undone: Vec<usize> = self.changes.iter().filter_map(|c| c.undoes).collect();
        let (index, last) = self
            .changes
            .iter()
            .enumerate()
            .rev()
            .find(|(index, change)| change.undoes.is_none() && !undone.contains(&(index + 1)))
            .ok_or_else(|| "There is no change left to undo.".to_owned())?;
        let (harness, model, restore) = (last.harness, last.model.clone(), last.from);
        let from = self.current(harness, model.as_deref());
        self.put(harness, model.as_deref(), restore);
        self.changes.push(Change {
            at: atif::iso(atif::now_ms()),
            by,
            harness,
            model,
            from,
            to: restore,
            said: said.trim().to_owned(),
            reason: None,
            undoes: Some(index + 1),
        });
        Ok(self.changes.last().expect("just pushed"))
    }

    /// Adds a line to the notes.
    pub fn add_note(&mut self, note: &str) {
        let note = note.trim();
        if note.is_empty() {
            return;
        }
        if !self.notes.is_empty() && !self.notes.ends_with('\n') {
            self.notes.push('\n');
        }
        self.notes.push_str(&format!("\n- {note}\n"));
    }

    /// What selection reads, with each harness's current model checked
    /// against the never pairs.
    #[must_use]
    pub fn leaning(&self, model: impl Fn(Harness) -> Option<String>) -> Leaning {
        let mut never_with_model = PerHarness::<bool>::default();
        for harness in [Harness::ClaudeCode, Harness::CoderLoop, Harness::Codex] {
            if model(harness).is_some_and(|model| self.never_with(harness, &model)) {
                never_with_model.set(harness, true);
            }
        }
        Leaning {
            stances: self.stances,
            never_with_model,
        }
    }

    /// One line per harness with a stance, and one per never pair, for the
    /// preferences screen.
    #[must_use]
    pub fn summary(&self) -> Vec<String> {
        let mut lines: Vec<String> = [Harness::ClaudeCode, Harness::CoderLoop, Harness::Codex]
            .into_iter()
            .filter(|harness| self.stances.get(*harness) != Stance::Neutral)
            .map(|harness| {
                format!(
                    "{}: {}",
                    harness.declaration().name,
                    self.stances.get(harness).label()
                )
            })
            .collect();
        lines.extend(self.never.iter().map(|pair| {
            format!(
                "{}: never use with {}",
                pair.harness.declaration().name,
                pair.model
            )
        }));
        lines.extend(self.pins.iter().map(Pin::words));
        lines
    }

    /// The pins that apply in `repo`.
    #[must_use]
    pub fn pins_in(&self, repo: Option<&str>) -> Vec<Pin> {
        self.pins
            .iter()
            .filter(|pin| pin.applies_in(repo))
            .cloned()
            .collect()
    }

    /// Adds a pin, replacing one for the same place and kind.
    ///
    /// # Errors
    /// A model name must be one word, and a never pair cannot be pinned.
    pub fn pin(&mut self, pin: Pin) -> Result<&Pin, String> {
        if let Some(model) = &pin.model {
            if !valid_model(model) {
                return Err(unknown_model(model));
            }
            if self.never_with(pin.harness, model) {
                return Err("You said never to use that model there; allow it again first.".into());
            }
        }
        if self.stances.get(pin.harness) == Stance::Never {
            return Err("You said never to use that engine; allow it again first.".into());
        }
        self.pins
            .retain(|old| !(old.repo == pin.repo && old.kind == pin.kind));
        self.pins.push(pin);
        Ok(self.pins.last().expect("just pushed"))
    }

    /// Removes the pins on `harness` (in `repo` when given). Returns how
    /// many went.
    pub fn unpin(&mut self, harness: Harness, repo: Option<&str>) -> usize {
        let before = self.pins.len();
        self.pins.retain(|pin| {
            !(pin.harness == harness && (repo.is_none() || pin.repo.as_deref() == repo))
        });
        before - self.pins.len()
    }
}

/// The model ids Coder offers (#11482): Claude Code's in /models, then
/// the ones /models lists for Coder's own loop. A preference or pin names
/// one of these; a made-up or retired id is refused before anything is
/// written.
#[must_use]
pub fn catalog() -> Vec<&'static str> {
    crate::claude::MODELS
        .iter()
        .map(|(id, _, _)| *id)
        .chain(
            crate::models::SHORTLIST
                .iter()
                .copied()
                .filter(|id| *id != crate::models::DEFAULT_MODEL),
        )
        .collect()
}

/// A model the person wrote into the file themselves: one word. Only a
/// new change is held to [`catalog`], so a file that names an older model
/// still reads.
fn well_formed(model: &str) -> bool {
    !model.is_empty() && model.len() <= 200 && !model.chars().any(char::is_whitespace)
}

fn valid_model(model: &str) -> bool {
    catalog().contains(&model)
}

/// What a refused model id is told: the ids that exist.
fn unknown_model(model: &str) -> String {
    format!(
        "Coder has no model {model}. The models are: {}.",
        catalog().join(", ")
    )
}

/// Whether `model` is one of Claude Code's models in /models.
fn claude_model(model: &str) -> bool {
    crate::claude::MODELS.iter().any(|(id, _, _)| *id == model)
}

/// What the candidate check reads from the routing preferences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leaning {
    pub stances: PerHarness<Stance>,
    /// The harnesses whose current model the person said never to use on
    /// them.
    pub never_with_model: PerHarness<bool>,
}

impl Leaning {
    /// The leaning from the file at `path`, or no preference when there is
    /// no file or it cannot be read (the preferences screen then shows the
    /// error).
    #[must_use]
    pub fn read(path: Option<&Path>, model: impl Fn(Harness) -> Option<String>) -> Self {
        path.and_then(|path| RoutingPreferences::load(path).ok())
            .map(|preferences| preferences.leaning(model))
            .unwrap_or_default()
    }
}

/// The tool's definition, in the shape the live loop offers tools.
#[must_use]
pub fn definition() -> Value {
    json!({"type":"function","function":{"name":TOOL,
        "description":"Change which engines or models Coder uses for the person's turns. Call it only when the person asks to change that: to use an engine more, less, never, or as usual again; never to run a certain model on one; to always use one (a pin: set pin, with this_repo for this repository only, kind for one kind of work, and model for one model; unpin removes it); or to prefer a Claude model (harness claude-code, stance prefer, and the model: this sets the model Claude Code runs, the same as picking it in /models). Never call it for a question about how something works; answer the question instead. Asked how to use a model such as Opus, say in one line: type /models and pick it (Claude Code offers Haiku 5.5, Sonnet 5.5, Opus 5.5 and Fable 5.1). The engines: claude-code is Claude Code on this computer's Claude plan; coder-loop is Coder's own loop on OpenAgents; codex is Codex on this computer's ChatGPT login. Stances: prefer (try it first), avoid (try it after the others), never (never use it), neutral (no preference). With a model other than a Claude preference, only never or neutral apply, to that exact model on that engine. Set undo to reverse the most recent change instead. Always pass the person's own words in said. The change is saved and applies from the next turn; afterwards tell the person in one line what changed.",
        "parameters":{"type":"object","additionalProperties":false,"required":["said"],"properties":{
            "harness":{"type":"string","enum":["claude-code","coder-loop","codex"]},
            "stance":{"type":"string","enum":["prefer","neutral","avoid","never"]},
            "model":{"type":"string","enum":catalog(),"description":"One of Coder's models: a Claude model to prefer on claude-code, or the exact model for a never, neutral, or pin."},
            "said":{"type":"string","minLength":1,"maxLength":500,"description":"The person's words that asked for this."},
            "reason":{"type":"string","maxLength":500},
            "note":{"type":"string","maxLength":500,"description":"A preference that is not one of the stances, kept in the notes, such as cheaper engines for small tasks."},
            "undo":{"type":"boolean"},
            "pin":{"type":"boolean","description":"Always use this harness (and model) for the person's work."},
            "unpin":{"type":"boolean","description":"Remove the pins on this harness."},
            "this_repo":{"type":"boolean","description":"The pin applies only in the current repository."},
            "kind":{"type":"string","enum":["answer","repo_change","issue_to_pr","research"],"description":"The pin applies only to this kind of work."}}}}})
}

/// What the model is told while the tool is offered.
pub const INSTRUCTIONS: &str = "When the person tells you to use an engine more, less, never, or as usual again, never to run a model on an engine, always to use one (here, or for a kind of work), or to use a Claude model, call routing_preference with their words; it changes which engine or model takes later turns. A question about how something works is not such a request: answer it, and change nothing.\n";

/// Runs one call of [`TOOL`] against the file at `path`.
///
/// # Errors
/// The arguments are invalid, or the file cannot be read or saved.
pub fn execute(path: &Path, arguments: Value) -> Result<Value, String> {
    execute_in(path, arguments, None)
}

/// [`execute`] in the repository named `repo` (its folder's name), which a
/// pin for this repository applies to.
///
/// # Errors
/// As [`execute`].
pub fn execute_in(path: &Path, arguments: Value, repo: Option<&str>) -> Result<Value, String> {
    let request: Request = serde_json::from_value(arguments).map_err(|_| {
        "Pass said, and either undo, or a harness with a stance (and a model for a pair)."
            .to_owned()
    })?;
    if request.said.trim().is_empty() {
        return Err("Pass the person's words in said.".into());
    }
    let mut preferences = RoutingPreferences::load(path)?;
    if request.pin || request.unpin {
        let harness = request.harness.ok_or("Pass the harness to pin.")?;
        let repo = if request.this_repo {
            Some(
                repo.ok_or("This folder is not in a repository.")?
                    .to_owned(),
            )
        } else {
            None
        };
        let pinned = if request.pin {
            Some(
                preferences
                    .pin(Pin {
                        harness,
                        model: request.model.clone(),
                        repo,
                        kind: request.kind,
                        said: request.said.trim().to_owned(),
                        at: atif::iso(atif::now_ms()),
                    })?
                    .words(),
            )
        } else {
            preferences.unpin(harness, repo.as_deref());
            None
        };
        preferences.save(path)?;
        return Ok(json!({
            "saved": path.display().to_string(),
            "pinned": pinned,
            "now": preferences.summary(),
        }));
    }
    // "Prefer Opus" (#11482): the model Claude Code runs, the same pick
    // /models makes, rather than a stance on the pair.
    if let (Some(Harness::ClaudeCode), Some(Stance::Prefer), Some(model), false) = (
        request.harness,
        request.stance,
        request.model.as_deref(),
        request.undo,
    ) {
        if !claude_model(model) {
            return Err(unknown_model(model));
        }
        crate::claude::choose(model, crate::claude::choice().effort)?;
        return Ok(json!({
            "model": model,
            "now": format!(
                "Claude Code runs {} from the next message.",
                crate::claude::name(model)
            ),
        }));
    }
    if let (Some(Stance::Prefer), Some(model)) = (request.stance, request.model.as_deref()) {
        return Err(format!(
            "To run {} on Coder's own loop, the person picks it in /models.",
            crate::models::label(model)
        ));
    }
    let change = if request.undo {
        Some(preferences.undo(Who::Model, &request.said)?.clone())
    } else {
        match (request.harness, request.stance) {
            (Some(harness), Some(stance)) => Some(
                preferences
                    .change(
                        Who::Model,
                        harness,
                        request.model.as_deref(),
                        stance,
                        &request.said,
                        request.reason.as_deref(),
                    )?
                    .clone(),
            ),
            (None, None) if request.note.is_some() => None,
            _ => return Err("Pass both a harness and a stance.".into()),
        }
    };
    if let Some(note) = &request.note {
        preferences.add_note(note);
    }
    preferences.save(path)?;
    Ok(json!({
        "saved": path.display().to_string(),
        "change": change,
        "now": preferences.summary(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_plugin::{Exclusion, Facts, Health, candidates};

    fn temporary() -> tempfile::TempDir {
        match std::env::var_os("OPENAGENTS_SCRATCH") {
            Some(root) => tempfile::tempdir_in(root).unwrap(),
            None => tempfile::tempdir().unwrap(),
        }
    }

    fn facts(leaning: Leaning) -> Facts {
        Facts {
            local: true,
            claude_plugin_on: true,
            claude: Health {
                signed_in: true,
                at_limit: false,
            },
            codex_on: true,
            codex: Health {
                signed_in: true,
                at_limit: false,
            },
            openrouter_key: false,
            leaning,
        }
    }

    #[test]
    fn a_missing_file_has_no_preference_and_plain_notes_parse() {
        let temporary = temporary();
        let path = path(temporary.path());
        let preferences = RoutingPreferences::load(&path).unwrap();
        assert_eq!(preferences, RoutingPreferences::default());
        assert!(!path.exists());
        let notes = RoutingPreferences::parse("Just my notes.\n").unwrap();
        assert_eq!(notes.notes, "Just my notes.\n");
        assert_eq!(notes.stances, PerHarness::default());
    }

    #[test]
    fn a_hand_written_file_parses_and_survives_a_rewrite() {
        let text = "+++\nversion = 1\n\n[harness]\nclaude-code = \"avoid\"\ncodex = \"prefer\"\n\n[[never]]\nharness = \"coder-loop\"\nmodel = \"deepseek/deepseek-v4\"\n+++\n# Mine\n\nKeep it cheap.\n";
        let preferences = RoutingPreferences::parse(text).unwrap();
        assert_eq!(preferences.stances.get(Harness::ClaudeCode), Stance::Avoid);
        assert_eq!(preferences.stances.get(Harness::Codex), Stance::Prefer);
        assert_eq!(preferences.stances.get(Harness::CoderLoop), Stance::Neutral);
        assert!(preferences.never_with(Harness::CoderLoop, "deepseek/deepseek-v4"));
        assert_eq!(preferences.notes, "# Mine\n\nKeep it cheap.\n");
        let again = RoutingPreferences::parse(&preferences.render()).unwrap();
        assert_eq!(again, preferences);
    }

    #[test]
    fn a_damaged_or_newer_file_is_refused_and_kept() {
        let temporary = temporary();
        let path = path(temporary.path());
        for text in [
            "+++\nversion = 1\n",
            "+++\nversion = 2\n+++\n",
            "+++\nversion = 1\nunexpected = 1\n+++\n",
            "+++\nversion = 1\n[harness]\nclaude-code = \"sometimes\"\n+++\n",
            "+++\nversion = 1\n[[never]]\nharness = \"codex\"\nmodel = \"two words\"\n+++\n",
        ] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(RoutingPreferences::load(&path).unwrap_err(), READ_ERROR);
            assert!(
                execute(
                    &path,
                    json!({"harness":"codex","stance":"avoid","said":"x"})
                )
                .is_err()
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
    }

    #[test]
    fn every_change_is_recorded_and_can_be_undone() {
        let temporary = temporary();
        let path = path(temporary.path());
        std::fs::write(&path, "+++\nversion = 1\n+++\nMy notes.\n").unwrap();
        execute(
            &path,
            json!({"harness":"claude-code","stance":"avoid","said":"stop using Claude so much","note":"cheaper engines for small tasks"}),
        )
        .unwrap();
        execute(
            &path,
            json!({"harness":"coder-loop","stance":"never","model":"deepseek/deepseek-v4.1-flash","said":"never send this to deepseek"}),
        )
        .unwrap();
        let saved = RoutingPreferences::load(&path).unwrap();
        assert_eq!(saved.stances.get(Harness::ClaudeCode), Stance::Avoid);
        assert!(saved.never_with(Harness::CoderLoop, "deepseek/deepseek-v4.1-flash"));
        assert_eq!(saved.changes.len(), 2);
        assert_eq!(saved.changes[0].by, Who::Model);
        assert_eq!(saved.changes[0].said, "stop using Claude so much");
        assert_eq!(
            (saved.changes[0].from, saved.changes[0].to),
            (Stance::Neutral, Stance::Avoid)
        );
        assert!(saved.notes.starts_with("My notes.\n"));
        assert!(saved.notes.contains("- cheaper engines for small tasks"));

        execute(
            &path,
            json!({"undo":true,"said":"actually, allow deepseek"}),
        )
        .unwrap();
        execute(&path, json!({"undo":true,"said":"and Claude as usual"})).unwrap();
        let undone = RoutingPreferences::load(&path).unwrap();
        assert!(!undone.never_with(Harness::CoderLoop, "deepseek/deepseek-v4.1-flash"));
        assert_eq!(undone.stances.get(Harness::ClaudeCode), Stance::Neutral);
        assert_eq!(undone.changes.len(), 4);
        assert_eq!(undone.changes[2].undoes, Some(2));
        assert_eq!(undone.changes[3].undoes, Some(1));
        assert!(execute(&path, json!({"undo":true,"said":"once more"})).is_err());
    }

    #[test]
    fn a_pair_takes_only_never_or_neutral() {
        let temporary = temporary();
        let path = path(temporary.path());
        assert!(
            execute(
                &path,
                json!({"harness":"codex","stance":"prefer","model":"gpt-6","said":"x"})
            )
            .is_err()
        );
        assert!(execute(&path, json!({"harness":"codex","said":"x"})).is_err());
        assert!(
            execute(
                &path,
                json!({"harness":"nobody","stance":"avoid","said":"x"})
            )
            .is_err()
        );
        assert!(!path.exists());
    }

    /// "stop using Claude so much": the model calls the tool, and the next
    /// turn's first candidate is no longer Claude Code.
    #[test]
    fn avoiding_claude_code_changes_the_first_candidate() {
        let temporary = temporary();
        let path = path(temporary.path());
        let before = candidates(&facts(Leaning::read(Some(&path), |_| None)));
        assert_eq!(before.admitted[0], Harness::ClaudeCode);

        execute(
            &path,
            json!({"harness":"claude-code","stance":"avoid","said":"stop using Claude so much"}),
        )
        .unwrap();
        let after = candidates(&facts(Leaning::read(Some(&path), |_| None)));
        assert_eq!(after.admitted, [Harness::CoderLoop, Harness::ClaudeCode]);

        execute(
            &path,
            json!({"harness":"claude-code","stance":"prefer","said":"go back to Claude first"}),
        )
        .unwrap();
        let preferred = candidates(&facts(Leaning::read(Some(&path), |_| None)));
        assert_eq!(preferred.admitted[0], Harness::ClaudeCode);
    }

    #[test]
    fn never_and_never_pairs_leave_a_harness_out() {
        let mut preferences = RoutingPreferences::default();
        preferences
            .change(
                Who::Person,
                Harness::ClaudeCode,
                None,
                Stance::Never,
                "no Claude",
                None,
            )
            .unwrap();
        preferences
            .change(
                Who::Person,
                Harness::CoderLoop,
                Some("deepseek/deepseek-v4.1-flash"),
                Stance::Never,
                "not deepseek",
                None,
            )
            .unwrap();
        let on = |model: &'static str| {
            move |harness: Harness| (harness == Harness::CoderLoop).then(|| model.to_owned())
        };
        let found = candidates(&facts(preferences.leaning(on("openagents/auto"))));
        assert_eq!(found.admitted, [Harness::CoderLoop]);
        assert!(
            found
                .excluded
                .contains(&(Harness::ClaudeCode, Exclusion::PersonSaidNever))
        );

        let found = candidates(&facts(
            preferences.leaning(on("deepseek/deepseek-v4.1-flash")),
        ));
        assert!(found.admitted.is_empty());
        assert!(
            found
                .excluded
                .contains(&(Harness::CoderLoop, Exclusion::PersonSaidNeverWithModel))
        );
    }

    /// #11482: a made-up or retired model is refused before anything is
    /// written, with the ids that exist; the schema lists only those; a
    /// preferred model on Coder's own loop points to /models.
    #[test]
    fn only_coders_own_models_are_taken() {
        let dir = temporary();
        let file = path(dir.path());
        let error = execute(
            &file,
            json!({"harness":"claude-code","stance":"prefer","model":"claude-3-opus-20240229","said":"i want opus"}),
        )
        .unwrap_err();
        assert!(error.contains("no model claude-3-opus-20240229"), "{error}");
        assert!(error.contains("claude-opus-5-5"), "{error}");
        let error = execute(
            &file,
            json!({"harness":"coder-loop","stance":"never","model":"claude-3-opus-20240229","said":"never"}),
        )
        .unwrap_err();
        assert!(error.contains("The models are:"), "{error}");
        let error = execute(
            &file,
            json!({"harness":"coder-loop","stance":"prefer","model":"x-ai/grok-4.7","said":"prefer grok"}),
        )
        .unwrap_err();
        assert!(error.contains("/models"), "{error}");
        assert!(!file.exists(), "nothing was written");

        let schema = definition();
        let models = schema["function"]["parameters"]["properties"]["model"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(models, catalog());
        assert!(models.contains(&"claude-opus-5-5"));
        assert!(!models.contains(&crate::models::DEFAULT_MODEL));
        let description = schema["function"]["description"].as_str().unwrap();
        assert!(description.contains("Never call it for a question"));
        assert!(description.contains("type /models and pick it"));
    }

    #[test]
    fn the_tool_is_offered_only_with_a_place_to_save() {
        let mut execution = crate::App::default().agent_execution();
        execution.routing = None;
        let named = |execution: &crate::plugin_tools::ExecutionSettings| {
            execution
                .defs()
                .iter()
                .any(|tool| tool["function"]["name"] == TOOL)
        };
        assert!(!named(&execution));
        execution.routing = Some(PathBuf::from("/unavailable/routing-preferences.md"));
        assert!(named(&execution));
        assert!(
            crate::claude_tools::names(&execution)
                .iter()
                .any(|name| name == TOOL)
        );
    }
}
