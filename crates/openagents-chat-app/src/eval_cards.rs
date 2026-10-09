//! The Gym's cards and sheets in chat, as the phone draws them
//! (`docs/product/2026-09-28-app-wireframe.md`, revision 3: `CARD-01` to
//! `CARD-07`, `SCR-05`, `SCR-06`, `SCR-11`, `SCR-20`, `SCR-21`).
//!
//! Rust builds every card and sheet as a closed view value with the app's
//! own labels; the iOS and Android hosts draw it. A card's numbers come only
//! from a record: a card the worker sent (checked by NIP-CJ's parser), a
//! run this phone started and the report it got back, or the phone's own
//! XP ledger. Nothing here reads a worker's label, and a value it doesn't
//! have reads "Not known yet" or is left out, never guessed.
//!
//! Every button carries an ID the phone minted, and [`Actions`] maps it to
//! what the tap does. A tap whose ID the last view didn't mint does
//! nothing, so a card's button is the only thing that sends a request.

use std::collections::BTreeMap;

use nostr::cj_conversation::{Add, Closest, Draft, NewsItem, Reach, ResultLine, Size, Source};
use nostr::eval_ext::{CaseKind, Headline, Verdict};
use serde::Serialize;
use serde_json::Value;

#[cfg(any(test, feature = "test-support"))]
/// Words that never appear on a label, button, chip, card, or sheet
/// (`Words on screen` in the wireframe). Answers may differ; the phone's
/// own words may not. The word for anything a person adds is "plugin"
/// (decided 2026-10-01, #10087): "capability", "extension", and "tool" are
/// banned as names for it, and "Wasm" stays off labels. A model's tool
/// call in a transcript is the model's text, not a label.
pub const BANNED: &[&str] = &[
    "npub",
    "nsec",
    "key",
    "relay",
    "nostr",
    "nip",
    "atif",
    "tailnet",
    "tailscale",
    "wasm",
    "capability",
    "capabilities",
    "extension",
    "tool",
    "benchmark",
    "terminal-bench",
    "tb",
    "eval",
    "evaluation",
    "suite",
    "case",
    "grader",
    "rubric",
    "judge",
    "baseline",
    "arm",
    "harness",
    "stand-in",
    "mock",
    "pilot",
    "jev",
    "luna",
    "microcoder",
    "verifier",
    "trace",
    "recipe",
    "grant",
    "sats",
    "btc",
    "₿",
    "lightning",
    "invoice",
    "host",
    "workspace",
    "pubkey",
    "hex",
];

/// Product names that hold a banned word on purpose: bring-your-own-keys
/// names a person's own API keys "Your keys". Longest first. "Key" anywhere
/// else ("Your Nostr key", "secret key") stays banned. The host keeps the
/// same lists (`crates/coder/src/router/gym.rs`); a test there holds them
/// equal.
#[cfg(any(test, feature = "test-support"))]
pub const NAMES: &[&str] = &["your own keys", "use my keys", "your keys", "my keys"];

/// Whether `text` contains a banned word as a whole word, in any case,
/// outside the product [`NAMES`].
#[cfg(any(test, feature = "test-support"))]
pub fn jargon(text: &str) -> Option<&'static str> {
    let mut lower = text.to_lowercase();
    for name in NAMES {
        lower = lower.replace(name, " ");
    }
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '₿'))
        .filter(|w| !w.is_empty())
        .collect();
    BANNED
        .iter()
        .find(|banned| words.iter().any(|word| word == *banned))
        .copied()
}

/// What a tap does. The phone decides it when it draws the button, from
/// its own state; the host sends back only the button's ID.
// A view mints a few dozen at most; boxing the start's offer buys nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Start the run a `start_eval` offer under reply `turn` of `talk`
    /// names, for `purpose`.
    Start {
        talk: String,
        turn: usize,
        offer: Value,
        tool: String,
        purpose: Purpose,
    },
    /// Ask before stopping a run.
    Stop { run: String },
    /// Stop it: the confirm sheet's button.
    ConfirmStop { run: String },
    /// Start a failed run again.
    Retry { run: String },
    /// The full test set after a one-run try.
    FullRun { run: String },
    /// The result's detail (`SCR-05`).
    Details { run: String },
    /// A test set (`SCR-21`).
    TestSet { source: TestSetSource },
    /// Add to the Gym (`SCR-20`).
    Publish { run: String },
    /// `SCR-20`'s button: publish.
    ConfirmPublish { run: String },
    /// Close the sheet on screen.
    CloseSheet,
    /// Approve the interview's step: sends "Looks good" with the draft.
    LooksGood { talk: String },
    /// Put "Change: " in the composer.
    ChangeIt { talk: String },
    /// Send `text` as the person's message: in the open chat, or in a new
    /// chat when `fresh`.
    Say { text: String, fresh: bool },
    /// Open the Coder chat a run started on a computer.
    OpenCoder { host: String, task: String },
    /// Account > Computers, to connect one.
    ConnectComputer,
    /// The system share sheet with `text`.
    Share { text: String },
    /// The main menu's primary: the chat, with a pending card on top.
    Chat,
    /// The main menu's Profile row (`SCR-11`).
    Profile,
    /// The main menu's Gym in the Verse row.
    VerseGym,
    /// The Gym intro, step 1: **CHOOSE CODER**.
    ChooseCoder,
    /// The Gym intro, the end card's **LET'S GO**.
    LetsGo,
    /// The Gym intro's **Not now**: back to the chat, opted out.
    NotNow,
    /// Leave the intro when a run can't start.
    SkipFirstRun,
    /// Close the level-up overlay.
    Nice,
}

/// Why a run happens.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "purpose", rename_all = "snake_case")]
pub enum Purpose {
    /// Test a tool with a test set, three runs a side.
    Test,
    /// Try a draft once (`Try it once`).
    Try,
    /// Check another trainer's published result.
    Check {
        /// The result's `3189` pointer, as the check card cited it.
        publication: Value,
        /// Its trainer, as a name.
        trainer: String,
        claim: Claim,
    },
}

/// A result's counts and verdict, as a record said them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Claim {
    pub with: u64,
    pub without: Option<u64>,
    pub total: u64,
    /// `pass`, `fail`, or `inconclusive`.
    pub verdict: Verdict3,
}

/// A verdict, stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict3 {
    Pass,
    Fail,
    Inconclusive,
}

impl From<Verdict> for Verdict3 {
    fn from(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Pass => Self::Pass,
            Verdict::Fail => Self::Fail,
            Verdict::Inconclusive => Self::Inconclusive,
        }
    }
}

impl Verdict3 {
    pub fn word(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Inconclusive => "inconclusive",
        }
    }

    /// The verdict in the phone's words.
    pub fn plain(self) -> &'static str {
        match self {
            Self::Pass => "Better",
            Self::Fail => "Worse",
            Self::Inconclusive => "No clear change",
        }
    }
}

impl Claim {
    pub fn of(headline: &Headline, verdict: Verdict) -> Self {
        Self {
            with: headline.subject_passed,
            without: headline.baseline_passed,
            total: headline.total,
            verdict: verdict.into(),
        }
    }

    /// "5 of 8 → 7 of 8".
    pub fn arrow(&self) -> String {
        match self.without {
            Some(without) => format!(
                "{without} of {} → {} of {}",
                self.total, self.with, self.total
            ),
            None => format!("{} of {}", self.with, self.total),
        }
    }
}

/// Which test set a sheet lists.
#[derive(Clone, Debug, PartialEq)]
pub enum TestSetSource {
    /// The open draft of a chat.
    Draft { talk: String },
    /// The test set a run ran.
    Run { run: String },
}

/// A button the host draws. Its ID is the only thing a tap sends back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Button {
    pub id: String,
    pub label: String,
    /// `map`, `search`, `test`, `check`, `list`, `info`, `computer`, `add`,
    /// `share`, `ask`, `news`, `stop`, `retry`, or none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glyph: Option<&'static str>,
    pub enabled: bool,
}

/// How a line of text reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Body,
    Strong,
    Quiet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Line {
    pub text: String,
    pub tone: Tone,
}

/// One row of a list: marks before it, its text, a quiet line under it,
/// and a trailing note such as "+25 XP".
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Item {
    /// `check`, `cross`, `wait`, `dot`, or `none`; two on a test row
    /// (without, with).
    pub marks: Vec<&'static str>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailing: Option<String>,
}

/// Tests passed without the tool and with it, the biggest thing on a
/// result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Compare {
    /// "without the tool", "without it".
    pub without_label: String,
    /// "5 of 8", or `None` when the run had no side without the tool.
    pub without: Option<String>,
    /// "with Project map".
    pub with_label: String,
    pub with: String,
}

/// One side of a run, as blocks: done of total.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub label: String,
    pub done: u64,
    pub total: u64,
}

/// A card in a chat reply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CardView {
    pub id: String,
    /// `tool`, `draft`, `run`, `result`, `news`, `check`, `credit`, or
    /// `capability`.
    pub kind: &'static str,
    /// "STEP 2 OF 3" on the first run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<&'static str>,
    pub title: String,
    /// "+50 XP", top right.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<Compare>,
    pub lines: Vec<Line>,
    pub items: Vec<Item>,
    pub progress: Vec<Progress>,
    /// The one filled button.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<Button>,
    /// Outlined buttons inside the card.
    pub secondary: Vec<Button>,
    /// Chips under the card.
    pub chips: Vec<Button>,
    /// Where its facts come from, small and gray.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Something is on its way: a spinner beside the title.
    pub busy: bool,
}

/// A section of a sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Section {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    pub lines: Vec<Line>,
    pub items: Vec<Item>,
}

/// A level bar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Bar {
    pub label: String,
    /// XP into the level, and the level's span.
    pub value: u64,
    pub max: u64,
}

/// A sheet over the chat or the menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SheetView {
    pub id: String,
    /// `result`, `publish`, `test_set`, `level_up`, `profile`, or `stop`.
    pub kind: &'static str,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headline: Option<String>,
    /// A big number, as the new level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub big: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<Compare>,
    pub sections: Vec<Section>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar: Option<Bar>,
    /// The "Next:" line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<Button>,
    pub secondary: Vec<Button>,
    /// The X at the top.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close: Option<Button>,
    /// Something is on its way: the primary shows a spinner.
    pub busy: bool,
}

/// The actions the last view minted, by button ID.
#[derive(Default, Debug)]
pub struct Actions(BTreeMap<String, Action>);

impl Actions {
    /// A button with `label` that does `action`, known by `id`.
    pub fn button(
        &mut self,
        id: impl Into<String>,
        label: &str,
        glyph: Option<&'static str>,
        action: Action,
    ) -> Button {
        let id = id.into();
        self.0.insert(id.clone(), action);
        Button {
            id,
            label: label.to_owned(),
            glyph,
            enabled: true,
        }
    }

    /// A button drawn gray, which does nothing.
    pub fn inert(id: impl Into<String>, label: &str) -> Button {
        Button {
            id: id.into(),
            label: label.to_owned(),
            glyph: None,
            enabled: false,
        }
    }

    pub fn get(&self, id: &str) -> Option<&Action> {
        self.0.get(id)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// The body of a case or grader file: what follows its `+++` frontmatter,
/// trimmed.
pub fn body_of(file: &str) -> &str {
    let text = file.trim_start();
    if let Some(rest) = text.strip_prefix("+++")
        && let Some(end) = rest.find("\n+++")
    {
        return rest[end + 4..].trim();
    }
    text.trim()
}

/// The first line of `text`, at most `limit` characters.
pub fn first_line(text: &str, limit: usize) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.chars().count() <= limit {
        return line.to_owned();
    }
    let mut cut: String = line.chars().take(limit.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// A test's ID in words: `find-login` reads "Find login".
pub fn humane(id: &str) -> String {
    let words = id.replace(['-', '_', '.'], " ");
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A tool's glyph, by its name: display only.
pub fn tool_glyph(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "project map" => "map",
        "code finder" => "search",
        "test reader" => "test",
        _ => "tool",
    }
}

/// The hosted runner's catalog, compiled in: the plugin directories the
/// Gym tests, one per line, in [`CATALOG`]'s order (`crates/coder/tests/plugin_catalog.rs`
/// holds the two together, #10090).
const CATALOG_SOURCE: &str = include_str!("../../../deploy/eval-runner/catalog");

/// Each catalog plugin's directory in this repository, in [`CATALOG`]'s
/// order: [`CATALOG_SOURCE`]'s lines that are neither blank nor comments.
#[must_use]
pub fn catalog_dirs() -> Vec<&'static str> {
    CATALOG_SOURCE
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// The catalog's tools, as chips name them: the three evidence guests,
/// then the example plugins, in the hosted runner's order.
pub const CATALOG: &[&str] = &[
    "Project map",
    "Code finder",
    "Test reader",
    "Explain this error",
    "Release notes",
    "Dependency check",
];

/// A result's headline, by verdict: for a check, whether it held up.
pub fn verdict_headline(verdict: Verdict3, check: Option<Verdict3>, pilot: bool) -> String {
    if pilot {
        return "FIRST TRY".into();
    }
    if let Some(original) = check {
        return if original == verdict {
            "YOU CONFIRMED IT".into()
        } else {
            "IT DIDN'T HOLD UP".into()
        };
    }
    match verdict {
        Verdict3::Pass => "CODER GOT BETTER".into(),
        Verdict3::Inconclusive => "NO CLEAR CHANGE".into(),
        Verdict3::Fail => "CODER DID WORSE WITH THIS PLUGIN".into(),
    }
}

/// The tests passed without and with the tool.
pub fn compare(claim: &Claim, tool: &str) -> Compare {
    Compare {
        without_label: "without the plugin".into(),
        without: claim.without.map(|w| format!("{w} of {}", claim.total)),
        with_label: format!("with {tool}"),
        with: format!("{} of {}", claim.with, claim.total),
    }
}

/// What a start_eval offer will run, in one line: "8 tests, with and
/// without the plugin."
pub fn size_line(size: &Size) -> String {
    let tests = if size.cases == 1 {
        "1 test".to_owned()
    } else {
        format!("{} tests", size.cases)
    };
    if size.arms >= 2 {
        format!("{tests}, with and without the plugin.")
    } else {
        format!("{tests}, with the plugin.")
    }
}

/// A trainer's name from their public key: "Trainer 7KQ", the first three
/// letters of the key's `npub` data part. A name, not a key.
pub fn trainer_name(pubkey_hex: &str) -> String {
    let npub = pubkey_hex
        .parse::<secp256k1::XOnlyPublicKey>()
        .map(|k| nostr::nip19::encode_npub(&k.serialize()))
        .unwrap_or_default();
    match npub.get(5..8) {
        Some(letters) => format!("Trainer {}", letters.to_uppercase()),
        None => "A trainer".into(),
    }
}

/// `CARD-01` Tool card, from the worker's `tool` card and the `start_eval`
/// offer beside it, if any.
#[allow(clippy::too_many_arguments)]
pub fn tool_card(
    actions: &mut Actions,
    id: &str,
    name: &str,
    summary: &str,
    latest: Option<&ResultLine>,
    start: Option<(Action, &Size)>,
    hosted: bool,
    step: Option<String>,
) -> CardView {
    let mut lines = vec![Line {
        text: summary.to_owned(),
        tone: Tone::Quiet,
    }];
    lines.push(match latest {
        Some(latest) => Line {
            text: format!(
                "Latest: {} tests · {}",
                Claim::of(&latest.headline, latest.verdict).arrow(),
                Verdict3::from(latest.verdict).plain()
            ),
            tone: Tone::Strong,
        },
        None => Line {
            text: "Not tested yet. Be the first.".into(),
            tone: Tone::Strong,
        },
    });
    let mut chips = vec![];
    let primary = match start {
        Some((action, size)) => {
            lines.push(Line {
                text: size_line(size),
                tone: Tone::Quiet,
            });
            if hosted {
                lines.push(Line {
                    text: "Free. We run it on our computers.".into(),
                    tone: Tone::Quiet,
                });
            }
            Some(actions.button(format!("{id}.start"), "START THE TEST", None, action))
        }
        None => {
            lines.push(Line {
                text: "There's no test set for this plugin yet.".into(),
                tone: Tone::Quiet,
            });
            chips.push(actions.button(
                format!("{id}.write"),
                &format!("Write tests for {name}"),
                Some("add"),
                Action::Say {
                    text: format!("Help me write tests for {name}"),
                    fresh: false,
                },
            ));
            None
        }
    };
    for (n, other) in CATALOG
        .iter()
        .filter(|other| !other.eq_ignore_ascii_case(name))
        .enumerate()
    {
        chips.push(actions.button(
            format!("{id}.other{n}"),
            other,
            Some(tool_glyph(other)),
            Action::Say {
                text: format!("Test {other} on Coder"),
                fresh: false,
            },
        ));
    }
    CardView {
        id: id.to_owned(),
        kind: "tool",
        step,
        icon: Some(tool_glyph(name)),
        title: name.to_uppercase(),
        badge: None,
        compare: None,
        lines,
        items: vec![],
        progress: vec![],
        primary,
        secondary: vec![],
        chips,
        source: latest.map(|_| "From a published result in the Gym.".to_owned()),
        busy: false,
    }
}

/// The tests of a draft, as `CARD-02` and `SCR-21` list them.
pub fn draft_items(draft: &Draft, detail: bool) -> Vec<Item> {
    draft
        .cases
        .iter()
        .enumerate()
        .map(|(n, case)| {
            let task = first_line(body_of(&case.prompt), if detail { 200 } else { 60 });
            let stays_out = case.kind == CaseKind::ShouldNotFire;
            let checked: Vec<String> = case
                .graders
                .iter()
                .map(|(_, text)| first_line(body_of(text), 120))
                .filter(|line| !line.is_empty())
                .collect();
            let detail_text = if detail {
                let mut parts = vec![];
                if stays_out {
                    parts.push("The plugin should stay out of the way.".to_owned());
                }
                if !checked.is_empty() {
                    parts.push(format!("Checked: {}", checked.join(" ")));
                }
                (!parts.is_empty()).then(|| parts.join(" "))
            } else {
                stays_out.then(|| "(plugin should stay out of the way)".to_owned())
            };
            Item {
                marks: vec![],
                text: format!("{} {task}", n + 1),
                detail: detail_text,
                trailing: None,
            }
        })
        .collect()
}

/// `CARD-02` Test set draft card. `gate` is the interview waiting for a
/// tap; `start` is the offer's run, when the step offers one.
pub fn draft_card(
    actions: &mut Actions,
    id: &str,
    talk: &str,
    draft: &Draft,
    gate: bool,
    start: Option<(Action, &Size)>,
) -> CardView {
    let tool = if draft.tool.catalog.is_some() {
        draft.tool.name.clone()
    } else {
        format!("{} (yours)", draft.tool.name)
    };
    let mut lines = vec![Line {
        text: format!("Plugin: {tool}"),
        tone: Tone::Strong,
    }];
    let items = draft_items(draft, false);
    if draft.cases.is_empty() {
        lines.push(Line {
            text: "No tests yet. We'll draft them with you next.".into(),
            tone: Tone::Quiet,
        });
    } else {
        lines.push(Line {
            text: "Each test is checked on Coder's last message and the files it made.".into(),
            tone: Tone::Quiet,
        });
    }
    let primary = match start {
        Some((action, size)) => {
            let label = if size.runs <= 1 {
                "TRY IT ONCE"
            } else {
                "RUN THE FULL TEST SET"
            };
            Some(actions.button(format!("{id}.start"), label, None, action))
        }
        None if gate => Some(actions.button(
            format!("{id}.good"),
            "LOOKS GOOD",
            None,
            Action::LooksGood {
                talk: talk.to_owned(),
            },
        )),
        None => None,
    };
    let mut secondary = vec![actions.button(
        format!("{id}.change"),
        "Change it",
        None,
        Action::ChangeIt {
            talk: talk.to_owned(),
        },
    )];
    if !draft.cases.is_empty() {
        secondary.push(actions.button(
            format!("{id}.tests"),
            "See every test",
            Some("list"),
            Action::TestSet {
                source: TestSetSource::Draft {
                    talk: talk.to_owned(),
                },
            },
        ));
    }
    CardView {
        id: id.to_owned(),
        kind: "draft",
        step: None,
        icon: Some("list"),
        title: "YOUR TEST SET · DRAFT".into(),
        badge: None,
        compare: None,
        lines,
        items,
        progress: vec![],
        primary,
        secondary,
        chips: vec![],
        source: Some("Only on this device until you add it to the Gym.".into()),
        busy: false,
    }
}

/// `CARD-05` Gym news card. `offer` is the one next step, when the reply
/// carried one.
pub fn news_card(
    actions: &mut Actions,
    id: &str,
    items: &[NewsItem],
    offer: Option<(String, Action)>,
) -> CardView {
    let chips = vec![];
    let mut lines = vec![];
    if items.is_empty() {
        lines.push(Line {
            text: "Nothing new since you last asked.".into(),
            tone: Tone::Body,
        });
    }
    let rows = items
        .iter()
        .map(|item| Item {
            marks: vec!["dot"],
            text: item.title.clone(),
            detail: Some(item.line.clone()),
            trailing: None,
        })
        .collect();
    let sourced_path = items
        .iter()
        .any(|item| matches!(item.source, Source::Path(_)));
    let primary =
        offer.map(|(label, action)| actions.button(format!("{id}.offer"), &label, None, action));
    CardView {
        id: id.to_owned(),
        kind: "news",
        step: None,
        icon: Some("news"),
        title: "WHAT'S NEW IN THE GYM".into(),
        badge: None,
        compare: None,
        lines,
        items: rows,
        progress: vec![],
        primary,
        secondary: vec![],
        chips,
        source: (!items.is_empty()).then(|| {
            if sourced_path {
                "From the Gym's records and our changelog.".to_owned()
            } else {
                "From the Gym's records.".to_owned()
            }
        }),
        busy: false,
    }
}

/// The message a tap on the missing-capability card's button sends: it
/// starts the chat's authoring interview.
pub const ADD_CAPABILITY_MESSAGE: &str = "Help me make a plugin for that";

/// The missing-capability card (#9960): the worker's `capability` card,
/// which names the closest admitted capability, if any, and how to add
/// one. `gym` is the tap for the worker's Gym offer, when it sent one.
/// Nothing on the card is the person's message.
pub fn capability_card(
    actions: &mut Actions,
    id: &str,
    closest: Option<&Closest>,
    add: Add,
    gym: Option<Action>,
) -> CardView {
    let mut lines = vec![Line {
        text: "There's no plugin for that yet.".into(),
        tone: Tone::Strong,
    }];
    match closest {
        Some(closest) => {
            lines.push(Line {
                text: format!(
                    "The closest one we have is {}: {}",
                    closest.name, closest.summary
                ),
                tone: Tone::Body,
            });
            lines.push(Line {
                text: match closest.reach {
                    Reach::Chat => "It works right here in chat.".into(),
                    Reach::Coder => "Coder uses it on a computer you connect.".into(),
                },
                tone: Tone::Quiet,
            });
        }
        None => lines.push(Line {
            text: "Nothing we have comes close.".into(),
            tone: Tone::Body,
        }),
    }
    lines.push(Line {
        text: "Anyone can add one and test it in the Gym, so everyone can see whether it helps."
            .into(),
        tone: Tone::Quiet,
    });
    let mut secondary = vec![];
    let primary = match add {
        Add::Author => {
            if let Some(gym) = gym {
                secondary.push(actions.button(format!("{id}.gym"), "See the Gym", None, gym));
            }
            Some(actions.button(
                format!("{id}.add"),
                "ADD A PLUGIN",
                Some("add"),
                Action::Say {
                    text: ADD_CAPABILITY_MESSAGE.into(),
                    fresh: false,
                },
            ))
        }
        Add::Gym => gym.map(|gym| actions.button(format!("{id}.gym"), "SEE THE GYM", None, gym)),
    };
    CardView {
        id: id.to_owned(),
        kind: "capability",
        step: None,
        icon: Some("add"),
        title: "NO PLUGIN FOR THAT YET".into(),
        badge: None,
        compare: None,
        lines,
        items: vec![],
        progress: vec![],
        primary,
        secondary,
        chips: vec![],
        source: closest.map(|_| "From what Coder and this chat can do now.".to_owned()),
        busy: false,
    }
}

/// `CARD-06` Check card. `xp` is the checker's share when the quest
/// record says it; `credited` when the ledger already paid this trainer
/// for checking the test set, so the check promises no XP (#9948).
#[allow(clippy::too_many_arguments)]
pub fn check_card(
    actions: &mut Actions,
    id: &str,
    tool: &str,
    trainer: &str,
    claim: &Claim,
    confirms: u64,
    start: Option<Action>,
    xp: Option<u64>,
    mine: bool,
    credited: bool,
) -> CardView {
    let tool_name = humane(tool);
    let mut lines = vec![Line {
        text: match claim.without {
            Some(without) if without == claim.with => format!(
                "{trainer} says Coder passed {} of {} tests with {tool_name} and without it.",
                claim.with, claim.total
            ),
            Some(without) => format!(
                "{trainer} says {tool_name} made Coder pass {} of {} tests instead of {without}.",
                claim.with, claim.total
            ),
            None => format!(
                "{trainer} says Coder passed {} of {} tests with {tool_name}.",
                claim.with, claim.total
            ),
        },
        tone: Tone::Strong,
    }];
    if confirms > 0 {
        lines.push(Line {
            text: if confirms == 1 {
                "1 trainer confirmed it so far.".into()
            } else {
                format!("{confirms} trainers confirmed it so far.")
            },
            tone: Tone::Quiet,
        });
    }
    let primary = if mine {
        lines.push(Line {
            text: "This is your own result. Another trainer checks it.".into(),
            tone: Tone::Quiet,
        });
        None
    } else {
        lines.push(Line {
            text: if credited {
                "Run the same tests to check it. A check doesn't use a daily run.".into()
            } else {
                "Run the same tests to check it. A check doesn't use a daily run, and it earns XP whether it confirms the result or not.".into()
            },
            tone: Tone::Quiet,
        });
        if credited {
            lines.push(Line {
                text: "You already earned XP for checking this test set. This check earns no more."
                    .into(),
                tone: Tone::Quiet,
            });
        }
        start.map(|action| actions.button(format!("{id}.start"), "RUN THE CHECK", None, action))
    };
    CardView {
        id: id.to_owned(),
        kind: "check",
        step: None,
        icon: Some("check"),
        title: "CHECK A RESULT".into(),
        badge: xp
            .filter(|_| !credited && !mine)
            .map(|xp| format!("+{xp} XP")),
        compare: None,
        lines,
        items: vec![],
        progress: vec![],
        primary,
        secondary: vec![],
        chips: vec![],
        source: Some("From a published result in the Gym.".into()),
        busy: false,
    }
}

/// `CARD-04` for a published result the worker sent (a named tool's).
pub fn published_result_card(id: &str, claim: &Claim, tool: Option<&str>) -> CardView {
    let tool = tool.unwrap_or("the plugin");
    CardView {
        id: id.to_owned(),
        kind: "result",
        step: None,
        icon: None,
        title: verdict_headline(claim.verdict, None, false),
        badge: None,
        compare: Some(compare(claim, tool)),
        lines: vec![],
        items: vec![],
        progress: vec![],
        primary: None,
        secondary: vec![],
        chips: vec![],
        source: Some("From a published result in the Gym.".into()),
        busy: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banned_words_are_found_as_whole_words() {
        assert_eq!(jargon("Start the test"), None);
        assert_eq!(jargon("Open the eval"), Some("eval"));
        assert_eq!(jargon("Your Nostr key"), Some("key"));
        assert_eq!(jargon("Trainer 7KQ"), None);
        assert_eq!(jargon("TB score"), Some("tb"));
        // A word that contains a banned word is fine.
        assert_eq!(jargon("Keyboard shortcuts and hosting"), None);
        // "Your keys" names a person's own API keys; any other key is banned.
        assert_eq!(jargon("Your own keys"), None);
        assert_eq!(jargon("Turn on Use my keys"), None);
        assert_eq!(jargon("Account, then Your keys: add a key"), Some("key"));
    }

    #[test]
    fn case_and_grader_bodies_drop_their_frontmatter() {
        let prompt = "+++\nv = \"openagents.eval-case.v1\"\n+++\n\nClean up main.rs.\n";
        assert_eq!(body_of(prompt), "Clean up main.rs.");
        assert_eq!(body_of("Just a body"), "Just a body");
        assert_eq!(humane("sort-imports"), "Sort imports");
        assert_eq!(first_line(&"x".repeat(20), 5), "xxxx…");
    }

    /// A result that scored the same both ways says so, not "pass 3 of 6
    /// tests instead of 3".
    #[test]
    fn a_check_of_an_even_result_says_it_plainly() {
        let line = |with, without| {
            let claim = Claim {
                with,
                without: Some(without),
                total: 6,
                verdict: Verdict3::Inconclusive,
            };
            let mut actions = Actions(BTreeMap::new());
            check_card(
                &mut actions,
                "c",
                "project-map",
                "A trainer",
                &claim,
                0,
                None,
                None,
                false,
                false,
            )
            .lines[0]
                .text
                .clone()
        };
        assert_eq!(
            line(3, 3),
            "A trainer says Coder passed 3 of 6 tests with Project map and without it."
        );
        assert_eq!(
            line(5, 2),
            "A trainer says Project map made Coder pass 5 of 6 tests instead of 2."
        );
    }

    /// The missing-capability card names the closest admitted capability
    /// from the worker's typed card, never the message, says how to add
    /// one, and its words are the phone's plain ones.
    #[test]
    fn a_missing_capability_card_offers_to_add_one() {
        let closest = Closest {
            name: "Project map".into(),
            summary: "Shows Coder how a project is laid out.".into(),
            reach: Reach::Coder,
        };
        let mut actions = Actions(BTreeMap::new());
        let card = capability_card(
            &mut actions,
            "c",
            Some(&closest),
            Add::Author,
            Some(Action::VerseGym),
        );
        assert_eq!(card.kind, "capability");
        assert_eq!(card.title, "NO PLUGIN FOR THAT YET");
        assert_eq!(card.lines[0].text, "There's no plugin for that yet.");
        assert_eq!(
            card.lines[1].text,
            "The closest one we have is Project map: Shows Coder how a project is laid out."
        );
        assert_eq!(
            card.lines[2].text,
            "Coder uses it on a computer you connect."
        );
        let primary = card.primary.as_ref().unwrap();
        assert_eq!(primary.label, "ADD A PLUGIN");
        assert_eq!(
            actions.get(&primary.id),
            Some(&Action::Say {
                text: ADD_CAPABILITY_MESSAGE.into(),
                fresh: false
            })
        );
        assert_eq!(card.secondary[0].label, "See the Gym");
        assert_eq!(actions.get(&card.secondary[0].id), Some(&Action::VerseGym));
        for line in &card.lines {
            assert_eq!(jargon(&line.text), None, "{}", line.text);
        }
        assert_eq!(jargon(&card.title), None);

        // Without the interview, the Gym is the one way; without a Gym
        // offer, the card has no button and still says what to do.
        let gym_only = capability_card(&mut actions, "g", None, Add::Gym, Some(Action::VerseGym));
        assert_eq!(gym_only.primary.as_ref().unwrap().label, "SEE THE GYM");
        assert!(gym_only.secondary.is_empty());
        assert_eq!(gym_only.lines[1].text, "Nothing we have comes close.");
        assert_eq!(gym_only.source, None);
        let bare = capability_card(&mut actions, "b", None, Add::Gym, None);
        assert!(bare.primary.is_none());
        assert!(
            bare.lines
                .iter()
                .any(|line| line.text.starts_with("Anyone can add one"))
        );
    }

    #[test]
    fn a_trainer_is_named_not_keyed() {
        let name = trainer_name(&"6e".repeat(32));
        assert!(name.starts_with("Trainer ") && name.len() == 11, "{name}");
        assert_eq!(jargon(&name), None);
        assert_eq!(trainer_name("zz"), "A trainer");
    }
}
