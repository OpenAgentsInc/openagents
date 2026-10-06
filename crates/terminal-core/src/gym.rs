//! The sheet's Gym page: retained plugin evaluations and their paired
//! comparisons, reopened and recomputed (#10663).
//!
//! F12 lists the results directories under the shell's directory through
//! `openagents --json plugin test studies DIR`, which reads each report
//! without checking it. ENTER opens the one picked through
//! `openagents --json plugin test show DIR`, which recomputes the report
//! from every retained attempt and says whether the attempts agree with
//! it, dispute it, or can't verify it. The page shows the exact plugin
//! release and run lock beside both arms, every attempt with its outcome,
//! cost, time, grades, and transcript state, and the known and unknown
//! costs. Only a report that agrees shows its verdict's words. Reading runs
//! nothing and publishes nothing; publishing and checking stay with
//! `openagents plugin test publish` and `check`, which the page names.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::mpsc::Receiver;

/// The most bytes of helper output the page reads.
pub const READ_MAX: usize = 4 * 1024 * 1024;

/// One results directory as the listing names it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Listed {
    pub dir: String,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub reported: Option<String>,
    #[serde(default)]
    pub ended_at: Option<u64>,
}

/// The results directories under one directory.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Studies {
    pub root: String,
    pub studies: Vec<Listed>,
}

/// One arm's coverage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Coverage {
    pub planned: u64,
    pub attempted: u64,
    pub completed: u64,
    pub refused: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub unknown: u64,
    pub excluded: u64,
}

/// One arm's totals: a cost or time is absent when any attempt's is
/// unknown.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Totals {
    pub cost_usd: Option<f64>,
    pub cost_unknown: usize,
    pub seconds: Option<f64>,
    pub seconds_unknown: usize,
    pub cases_scored: usize,
    pub cases_passed: usize,
    pub cases_unknown: usize,
}

/// One case in one arm.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct CaseArm {
    pub planned: u32,
    pub scored: u32,
    pub runs_passed: u32,
    pub score: Option<f64>,
    pub passed: Option<bool>,
}

/// One case across both arms.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Case {
    pub id: String,
    pub kind: String,
    pub compared: bool,
    pub subject_only: bool,
    pub subject: CaseArm,
    pub baseline: Option<CaseArm>,
    pub change: Option<f64>,
}

/// One retained attempt.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Attempt {
    pub case: String,
    pub arm: String,
    pub attempt: u32,
    pub outcome: String,
    pub reason: Option<String>,
    pub score: Option<f64>,
    pub passed: Option<bool>,
    pub cost_usd: Option<f64>,
    pub seconds: Option<f64>,
    pub grades: String,
    pub trajectory: String,
}

/// One arm's identity.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Side {
    pub definition: Value,
    pub lock: Value,
}

/// A retained evaluation as `plugin test show` recomputes it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Study {
    pub dir: String,
    pub report: Option<String>,
    pub suite: Option<String>,
    pub gate: Option<String>,
    pub gate_digest: Option<String>,
    pub subject: Option<Side>,
    pub baseline: Option<Side>,
    pub evaluator: Option<String>,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub defaults: Option<Value>,
    pub development: usize,
    pub held_out: usize,
    pub reported: Option<String>,
    pub recomputed: Option<String>,
    pub agreement: String,
    pub shown: String,
    pub coverage: BTreeMap<String, Coverage>,
    pub totals: BTreeMap<String, Totals>,
    pub cases: Vec<Case>,
    pub attempts: Vec<Attempt>,
    pub partial: Option<String>,
    pub limitations: Vec<String>,
    pub published: Option<String>,
    pub relay: Option<String>,
    pub problems: Vec<String>,
    pub missing: Vec<String>,
}

/// What reading the listing answered.
pub type ListRead = Result<Studies, String>;
/// What reading one study answered.
pub type Read = Result<Study, String>;

fn last_json(bytes: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().rev().find(|line| !line.trim().is_empty())?;
    serde_json::from_str(line).ok()
}

fn error_of(stdout: &[u8], stderr: &[u8]) -> String {
    last_json(stdout)
        .or_else(|| last_json(stderr))
        .and_then(|value| value["error"].as_str().map(crate::ascii::ascii))
        .unwrap_or_else(|| "the answer was not readable".into())
}

/// Decodes `openagents --json plugin test studies DIR`.
#[must_use]
pub fn decode_list(stdout: &[u8], stderr: &[u8]) -> ListRead {
    if stdout.len() > READ_MAX {
        return Err("the listing is too large to show".into());
    }
    match last_json(stdout) {
        Some(value) if value.get("studies").is_some() => {
            serde_json::from_value(value).map_err(|_| "the listing was not readable".into())
        }
        _ => Err(error_of(stdout, stderr)),
    }
}

/// Decodes `openagents --json plugin test show DIR`: the study for `dir`
/// and no other.
#[must_use]
pub fn decode(stdout: &[u8], stderr: &[u8], dir: &str) -> Read {
    if stdout.len() > READ_MAX {
        return Err("the study is too large to show".into());
    }
    match last_json(stdout) {
        Some(value) if value.get("agreement").is_some() => {
            let study: Study =
                serde_json::from_value(value).map_err(|_| "the study was not readable")?;
            if study.dir == dir {
                Ok(study)
            } else {
                Err("the answer named another results directory".into())
            }
        }
        _ => Err(error_of(stdout, stderr)),
    }
}

/// A retained result about one installed plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Evidence {
    pub dir: String,
    pub reported: Option<String>,
    pub ended_at: Option<u64>,
    /// Whether its subject is exactly the installed release.
    pub exact: bool,
}

/// What the page names for a component; it runs none of them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Commands {
    pub test: Option<String>,
    pub turn: Option<String>,
    #[serde(rename = "use")]
    pub use_: Option<String>,
}

/// One installed plugin by its exact release (#10664).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub enabled: bool,
    pub workflow: bool,
    pub background: Vec<String>,
    pub revocation: String,
    pub evidence: Vec<Evidence>,
    pub commands: Commands,
}

impl Component {
    /// The newest result for exactly this release.
    #[must_use]
    pub fn exact(&self) -> Option<&Evidence> {
        self.evidence
            .iter()
            .filter(|evidence| evidence.exact)
            .max_by_key(|evidence| evidence.ended_at)
    }
}

/// The installed plugins, as `plugin inspect` reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Components {
    pub plugins: Vec<Component>,
}

/// What reading the components answered.
pub type ComponentsRead = Result<Components, String>;

/// Decodes `openagents --json plugin inspect --results DIR`.
#[must_use]
pub fn decode_components(stdout: &[u8], stderr: &[u8]) -> ComponentsRead {
    if stdout.len() > READ_MAX {
        return Err("the plugins are too many to show".into());
    }
    match last_json(stdout) {
        Some(value) if value.get("plugins").is_some() => {
            serde_json::from_value(value).map_err(|_| "the plugins were not readable".into())
        }
        _ => Err(error_of(stdout, stderr)),
    }
}

/// The page's state.
#[derive(Default)]
pub struct Page {
    pub open: bool,
    /// The directory listed.
    pub root: Option<String>,
    pub listed: Option<ListRead>,
    pub listing: Option<Receiver<ListRead>>,
    pub selected: usize,
    /// The results directory open, when one is.
    pub viewing: Option<String>,
    pub shown: Option<Read>,
    pub reading: Option<Receiver<Read>>,
    pub scroll: usize,
    pub reads: u64,
    /// F2: the installed plugins instead of the results.
    pub components: bool,
    pub held: Option<ComponentsRead>,
    pub holding: Option<Receiver<ComponentsRead>>,
    pub component: usize,
}

impl Page {
    /// The picked results directory, when the listing is read.
    #[must_use]
    pub fn picked(&self) -> Option<&Listed> {
        match &self.listed {
            Some(Ok(studies)) => studies.studies.get(self.selected),
            _ => None,
        }
    }

    /// The picked component, when the plugins are read.
    #[must_use]
    pub fn picked_component(&self) -> Option<&Component> {
        match &self.held {
            Some(Ok(held)) => held.plugins.get(self.component),
            _ => None,
        }
    }
}

/// The components' text before wrapping, at `now` in Unix seconds.
fn component_lines(page: &Page, now: u64) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    let mut out = Vec::new();
    let held = match &page.held {
        None => {
            out.push(("PLUGINS on this computer  [reading]".into(), Tone::Loud));
            return out;
        }
        Some(Err(why)) => {
            out.push(("PLUGINS on this computer  [unavailable]".into(), Tone::Loud));
            out.push((
                format!("The plugins can't be read now: {why}. F2 twice reads them again."),
                Tone::Present,
            ));
            return out;
        }
        Some(Ok(held)) => held,
    };
    out.push((
        format!(
            "PLUGINS on this computer  {} installed  [{}]",
            held.plugins.len(),
            if page.holding.is_some() {
                "reading again"
            } else {
                "current"
            }
        ),
        Tone::Loud,
    ));
    out.push((
        "Each is its exact release. Results count only for the release they tested; ENTER opens \
         the newest one in the Gym."
            .into(),
        Tone::Quiet,
    ));
    out.push((String::new(), Tone::Quiet));
    if held.plugins.is_empty() {
        out.push((
            "No plugins are installed here: openagents plugin install DIR".into(),
            Tone::Present,
        ));
    }
    for (index, component) in held.plugins.iter().enumerate() {
        let picked = index == page.component;
        let name = if component.name.is_empty() {
            String::new()
        } else {
            format!(" ({})", ascii(&component.name))
        };
        out.push((
            format!(
                "{} {}{name}  {}  {}  package {}",
                if picked { ">" } else { " " },
                ascii(&component.id),
                ascii(&component.version),
                if component.enabled { "on" } else { "off" },
                short(&component.digest),
            ),
            if picked { Tone::Loud } else { Tone::Present },
        ));
        let others = component.evidence.iter().filter(|e| !e.exact).count();
        let exact = match component.exact() {
            Some(evidence) => format!(
                "tested: {} {} ago",
                evidence.reported.as_deref().unwrap_or("?"),
                evidence.ended_at.map_or_else(
                    || "some time".into(),
                    |at| crate::rules::span(now.saturating_sub(at))
                )
            ),
            None => "not tested in this release".into(),
        };
        out.push((
            format!(
                "    {exact}; {others} results for other releases; revocation {}; {}",
                if component.revocation == "not_checked" {
                    "not checked here"
                } else {
                    component.revocation.as_str()
                },
                match (component.workflow, component.background.is_empty()) {
                    (true, _) => "runs a workflow",
                    (false, false) => "runs in the background",
                    (false, true) => "skills only",
                }
            ),
            Tone::Present,
        ));
        if picked {
            for command in [
                &component.commands.test,
                &component.commands.turn,
                &component.commands.use_,
            ]
            .into_iter()
            .flatten()
            {
                out.push((format!("    {}", ascii(command)), Tone::Quiet));
            }
        }
    }
    out
}

fn short(digest: &str) -> String {
    let hex = digest.trim_start_matches("sha256:");
    hex[..hex.len().min(12)].to_string()
}

fn money(value: Option<f64>, unknown: usize, of: u64) -> String {
    match value {
        Some(value) => format!("${value:.4}"),
        None => format!("unknown ({unknown} of {of} attempts unknown)"),
    }
}

fn time(value: Option<f64>, unknown: usize, of: u64) -> String {
    match value {
        Some(value) => format!("{value:.1} s"),
        None => format!("unknown ({unknown} of {of} attempts unknown)"),
    }
}

fn number(value: Option<f64>) -> String {
    value.map_or_else(|| "-".into(), |value| format!("{value:.2}"))
}

fn arm_line(word: &str, arm: Option<&CaseArm>) -> String {
    match arm {
        None => format!("{word} not run"),
        Some(arm) => format!(
            "{word} {}/{} passed, score {}, {}",
            arm.runs_passed,
            arm.planned,
            number(arm.score),
            match arm.passed {
                Some(true) => "passes",
                Some(false) => "fails",
                None => "undecided",
            }
        ),
    }
}

fn side_lines(word: &str, side: Option<&Side>, out: &mut Vec<(String, crate::paper::Tone)>) {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    match side {
        None => out.push((format!("{word} none ran"), Tone::Present)),
        Some(side) => {
            let id = side.definition["id"].as_str().unwrap_or("?");
            let artifact = side.definition["artifact"]["digest"]
                .as_str()
                .map(short)
                .unwrap_or_default();
            let event = side.definition["event"]["id"]
                .as_str()
                .map(|event| format!(", release event {}", short(event)))
                .unwrap_or_default();
            out.push((
                format!("{word} {}  package {artifact}{event}", ascii(id)),
                Tone::Present,
            ));
            out.push((
                format!(
                    "    run lock {}",
                    side.lock["digest"].as_str().map(short).unwrap_or_default()
                ),
                Tone::Quiet,
            ));
        }
    }
}

/// The listing's text before wrapping, at `now` in Unix seconds.
fn list_lines(page: &Page, now: u64) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    let root = page.root.as_deref().map(ascii).unwrap_or_default();
    let mut out = Vec::new();
    let studies = match &page.listed {
        None => {
            out.push((format!("GYM studies under {root}  [reading]"), Tone::Loud));
            return out;
        }
        Some(Err(why)) => {
            out.push((
                format!("GYM studies under {root}  [unavailable]"),
                Tone::Loud,
            ));
            out.push((
                format!("The studies can't be listed now: {why}. F12 twice lists them again."),
                Tone::Present,
            ));
            return out;
        }
        Some(Ok(studies)) => studies,
    };
    out.push((
        format!(
            "GYM studies under {root}  {} results  [{}]",
            studies.studies.len(),
            if page.listing.is_some() {
                "reading again"
            } else {
                "current"
            }
        ),
        Tone::Loud,
    ));
    out.push((
        "The verdicts listed are the reports' own, unchecked. ENTER recomputes the one picked \
         from its retained attempts."
            .into(),
        Tone::Quiet,
    ));
    out.push((String::new(), Tone::Quiet));
    if studies.studies.is_empty() {
        out.push((
            "No plugin test results here: results/, evals/results/, or */evals/results/.".into(),
            Tone::Present,
        ));
        out.push((
            "A run writes one: openagents plugin test run TARGET".into(),
            Tone::Quiet,
        ));
    }
    for (index, study) in studies.studies.iter().enumerate() {
        let picked = index == page.selected;
        let when = study.ended_at.map_or_else(String::new, |at| {
            format!("  ended {} ago", crate::rules::span(now.saturating_sub(at)))
        });
        out.push((
            format!(
                "{} {}  reported {}{when}",
                if picked { ">" } else { " " },
                ascii(study.subject.as_deref().unwrap_or("unknown plugin")),
                study.reported.as_deref().unwrap_or("nothing"),
            ),
            if picked { Tone::Loud } else { Tone::Present },
        ));
        out.push((format!("    {}", ascii(&study.dir)), Tone::Quiet));
    }
    out
}

/// One study's text before wrapping.
fn study_lines(page: &Page) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    let dir = page.viewing.as_deref().map(ascii).unwrap_or_default();
    let mut out = Vec::new();
    let study = match &page.shown {
        None => {
            out.push((format!("STUDY {dir}  [recomputing]"), Tone::Loud));
            return out;
        }
        Some(Err(why)) => {
            out.push((format!("STUDY {dir}  [unavailable]"), Tone::Loud));
            out.push((
                format!("The study can't be read now: {why}. ENTER reads it again."),
                Tone::Present,
            ));
            return out;
        }
        Some(Ok(study)) => study,
    };
    out.push((
        format!(
            "STUDY {}  reported {}, recomputed {}  [{}]",
            ascii(&study.shown),
            study.reported.as_deref().unwrap_or("nothing"),
            study.recomputed.as_deref().unwrap_or("unknown"),
            match study.agreement.as_str() {
                "agrees" => "the retained attempts agree",
                "disputes" => "the retained attempts dispute the report",
                _ => "the retained attempts can't verify the report",
            }
        ),
        Tone::Loud,
    ));
    out.push((format!("DIR {dir}"), Tone::Quiet));
    side_lines("SUBJECT", study.subject.as_ref(), &mut out);
    side_lines("BASELINE", study.baseline.as_ref(), &mut out);
    out.push((
        format!(
            "SUITE {}  GATE {} {}  REPORT {}",
            ascii(study.suite.as_deref().unwrap_or("unknown")),
            ascii(study.gate.as_deref().unwrap_or("unknown")),
            study.gate_digest.as_deref().map(short).unwrap_or_default(),
            study.report.as_deref().map(short).unwrap_or_default(),
        ),
        Tone::Present,
    ));
    let mut facts = vec![format!(
        "PARTITION {} development, {} held out",
        study.development, study.held_out
    )];
    if let Some(defaults) = &study.defaults {
        facts.push(format!(
            "marginal over coder-defaults {}",
            defaults["id"].as_str().map(short).unwrap_or_default()
        ));
    }
    facts.push(match (&study.published, &study.relay) {
        (Some(event), Some(relay)) => {
            format!("published as {} on {}", short(event), ascii(relay))
        }
        (Some(event), None) => format!("published as {}", short(event)),
        _ => "not published".into(),
    });
    out.push((facts.join("  "), Tone::Present));
    if let Some(partial) = &study.partial {
        out.push((format!("PARTIAL {}", ascii(partial)), Tone::Loud));
    }
    for line in &study.problems {
        out.push((format!("DISPUTES {}", ascii(line)), Tone::Loud));
    }
    for line in &study.missing {
        out.push((format!("MISSING {}", ascii(line)), Tone::Present));
    }
    out.push((String::new(), Tone::Quiet));
    for (word, coverage) in &study.coverage {
        let totals = study.totals.get(word).cloned().unwrap_or_default();
        out.push((
            format!(
                "{} {} attempted of {} cases: {} completed, {} failed, {} refused, {} cancelled, \
                 {} unknown; {} cases passed, {} undecided",
                word.to_uppercase(),
                coverage.attempted,
                coverage.planned,
                coverage.completed,
                coverage.failed,
                coverage.refused,
                coverage.cancelled,
                coverage.unknown,
                totals.cases_passed,
                totals.cases_unknown,
            ),
            Tone::Present,
        ));
        out.push((
            format!(
                "    cost {}  time {}",
                money(totals.cost_usd, totals.cost_unknown, coverage.attempted),
                time(totals.seconds, totals.seconds_unknown, coverage.attempted),
            ),
            Tone::Quiet,
        ));
    }
    out.push((String::new(), Tone::Quiet));
    out.push(("CASES".into(), Tone::Loud));
    for case in &study.cases {
        let change = if case.compared {
            format!("change {}", number(case.change))
        } else if case.subject_only {
            "plugin only; not compared".into()
        } else {
            "not compared".into()
        };
        out.push((
            format!(
                "  {} ({})  {}  {}  {change}",
                ascii(&case.id),
                ascii(&case.kind),
                arm_line("with", Some(&case.subject)),
                arm_line("without", case.baseline.as_ref()),
            ),
            Tone::Present,
        ));
    }
    out.push((String::new(), Tone::Quiet));
    out.push(("ATTEMPTS".into(), Tone::Loud));
    for attempt in &study.attempts {
        let reason = attempt
            .reason
            .as_deref()
            .map(|reason| format!(" ({})", ascii(reason)))
            .unwrap_or_default();
        let passed = match attempt.passed {
            Some(true) => "pass",
            Some(false) => "fail",
            None => "-",
        };
        out.push((
            format!(
                "  {} {} #{}  {}{reason}  {passed}  score {}  cost {}  time {}  grades {}  \
                 transcript {}",
                ascii(&attempt.case),
                ascii(&attempt.arm),
                attempt.attempt,
                ascii(&attempt.outcome),
                number(attempt.score),
                attempt
                    .cost_usd
                    .map_or_else(|| "unknown".into(), |cost| format!("${cost:.4}")),
                attempt
                    .seconds
                    .map_or_else(|| "unknown".into(), |seconds| format!("{seconds:.1} s")),
                ascii(&attempt.grades),
                ascii(&attempt.trajectory),
            ),
            Tone::Present,
        ));
    }
    if !study.limitations.is_empty() {
        out.push((String::new(), Tone::Quiet));
        out.push(("LIMITATIONS".into(), Tone::Loud));
        for line in &study.limitations {
            out.push((format!("  {}", ascii(line)), Tone::Quiet));
        }
    }
    out.push((String::new(), Tone::Quiet));
    out.push((
        format!(
            "Reading ran and published nothing. Publish: openagents plugin test publish {dir}  \
             Check a published one: openagents plugin test check EVENT"
        ),
        Tone::Quiet,
    ));
    out
}

/// The page's text before wrapping, at `now` in Unix seconds.
#[must_use]
pub fn lines(page: &Page, now: u64) -> Vec<(String, crate::paper::Tone)> {
    if page.viewing.is_some() {
        study_lines(page)
    } else if page.components {
        component_lines(page, now)
    } else {
        list_lines(page, now)
    }
}
