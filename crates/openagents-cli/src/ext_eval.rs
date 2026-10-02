//! `openagents plugin test` (also `openagents ext eval`): run a plugin's
//! eval suite against Coder in both arms, write a starter case, publish a
//! result to the Gym, and check someone else's.
//!
//! The runner, the sandbox, the scoring, and the wire records live in
//! `crates/ext-eval`; this module resolves the target (a directory with a
//! package record, or an installed identity), asks for trust, reads the
//! pinned doors from the operator's environment, and talks to the relay.
//! `docs/extensions/evaluation.md` is the specification.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use coder::package::{HeldLock, Package};
use ext_eval::arms::{self, AgentPin, Program, Subject};
use ext_eval::case::{Grant, LoadOptions};
use ext_eval::proxy::Secret;
use ext_eval::run::{self, Author, DecisionPin, Door, Options, Progress, Setup};
use ext_eval::signal::Cancel;
use ext_eval::{Filter, Suite};
use nostr::domain::Event;
use serde_json::{Value, json};

use crate::relay::{Client, DEFAULT_WAIT, relay_url, signer_for, unix_now};
use crate::{Args, EXIT_FAILURE, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents plugin test COMMAND [OPTIONS]
  run TARGET [--runs N] [--case GLOB]... [--tag TAG]... [--baseline on|off]
      [--concurrency N] [--grant read|write|exec|network]... [--trust]
      [--door NAME] [--eval-dir DIR] [--output-dir DIR] [--keep-temp]
      [--coder PATH] [--questions DIR] [--gate ext-eval-v2|ext-eval-cost-v1]
        Run the tests for TARGET (a plugin directory, a test directory in
        it, or an installed PUBKEY:SLUG@VERSION) with the plugin and
        without it, and write report.json and report.html. --gate names
        the Gym gate the suite is judged by: ext-eval-v2 (correctness
        first, the default) or ext-eval-cost-v1 (cost first, correctness
        held non-inferior).
  release TARGET [--eval-dir DIR] [--gate ID] [--relay URL] [--blossom URL]
      [--blobs-dir DIR] [--as PROFILE]
        Release TARGET's test set as a NIP-EXT release signed by your key
        without running it: a second test set for someone else's plugin, which
        a hosted run can then cite. --blobs-dir writes the suite's files
        by digest for an operator to upload instead of a Blossom server.
  init [TARGET] [--bare] [--out DIR] [--eval-dir DIR]
        Write a test set with the authoring interview for the plugin at
        TARGET (default .), or with --bare a blank test named TARGET,
        evals/TARGET/, from the template.
  publish REPORT [--relay URL] [--blossom URL] [--as PROFILE] [--validates EVENT]
        Add a result to the Gym: release its suite (once) and publish the
        3189 result signed by your world key. --validates names a
        published result on the same plugin that this result, on a second
        test set, externally validates.
  check EVENT [TARGET] [--runs N] [--concurrency N] [--grant read|write|exec|network]...
      [--trust] [--output-dir DIR] [--coder PATH] [--questions DIR] [--relay URL]
      [--blossom URL] [--as PROFILE]
        Rerun someone's published result against the same plugin
        (TARGET, default .) and publish a check that confirms or disputes it.
Run progress goes to stderr and a summary table to stdout; --json writes
the result document instead, to stdout or to a path given after TARGET.
--coder names the agent binary (default: coder beside this program, then
on PATH) and --questions the question sets it asks (default: the
questions/ of the checkout it was built in, then ~/.openagents/questions);
both arms get the same ones. The pinned door is CODER_DOOR_URL, CODER_DOOR_KEY (or CODER_AI_GATEWAY_KEY),
and CODER_MODEL from this shell; the run's child never sees the key.
TYPESAFE_API_KEY, when set, is the decision door for decision graders and
the child's classifier. Nothing leaves this computer until publish.
Exit codes: 0 Better or a clean single-arm run, 1 Worse, inconclusive, or a
load failure, 2 partial, 64 invalid usage, 130 or 143 on a signal.
`openagents ext eval` is another name for this command.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("test run", Effect::LongRunning),
    Declared::computer("test init", Effect::LocalWrite),
    Declared::computer("test publish", Effect::Publishes),
    Declared::computer("test check", Effect::LongRunning),
    Declared::computer("test release", Effect::Publishes),
];

/// The package record file at an extension's root.
pub const PACKAGE_FILE: &str = "package.json";
/// Where an extension's skills live, relative to its root.
pub const SKILLS_DIR: &str = "skills";

const SWITCHES: &[&str] = &["trust", "keep-temp", "bare"];

const NAME: &str = "plugin test";

/// `openagents plugin test ...` (also `openagents ext eval ...`).
pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage(NAME, "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage(NAME, &message, USAGE),
    };
    match command.as_str() {
        "run" => run_command(output, &args),
        // The interview is `ext_eval_init` (#9937); `--bare` is the template.
        "init" if !args.switch("bare") => crate::ext_eval_init::run(output, rest),
        "init" => init(output, &args),
        "publish" => publish(output, &args),
        "check" => check(output, &args),
        "release" => release(output, &args),
        other => output.usage(NAME, &format!("unknown command `{other}`"), USAGE),
    }
}

/// The gate `--gate` names, checked against the gates the profile knows.
fn gate_option(args: &Args) -> Result<Option<String>, UsageError> {
    match args.option("gate") {
        None => Ok(None),
        Some(gate) if nostr::eval_ext::GATES.contains(&gate) => Ok(Some(gate.to_string())),
        Some(other) => Err(UsageError(format!(
            "--gate takes one of {}, not {other}",
            nostr::eval_ext::GATES.join(", ")
        ))),
    }
}

/// A usage error: a message and the usage exit code.
#[derive(Debug)]
struct UsageError(String);

fn usage(output: &Output, error: UsageError) -> u8 {
    output.usage(NAME, &error.0, USAGE)
}

/// The run options shared by `run` and `check`.
fn options(args: &Args, check: bool) -> Result<Options, UsageError> {
    let mut options = Options::default();
    let runs = args.option("runs");
    if let Some(runs) = runs {
        let runs: u32 = runs
            .parse()
            .map_err(|_| UsageError("--runs must be a number from 1 to 10".into()))?;
        if !(1..=10).contains(&runs) {
            return Err(UsageError("--runs must be a number from 1 to 10".into()));
        }
        options.runs = Some(runs);
    }
    options.concurrency = args.number::<usize>("concurrency", 1).map_err(UsageError)?;
    if !run::CONCURRENCY.contains(&options.concurrency) {
        return Err(UsageError("--concurrency must be from 1 to 8".into()));
    }
    if !check {
        options.baseline = match args.option("baseline") {
            None | Some("on") => true,
            Some("off") => false,
            Some(other) => {
                return Err(UsageError(format!(
                    "--baseline takes on or off, not {other}"
                )));
            }
        };
    }
    for grant in args.options("grant") {
        let grant = match grant {
            "read" => Grant::Read,
            "write" => Grant::Write,
            "exec" => Grant::Exec,
            "network" => Grant::Network,
            other => {
                return Err(UsageError(format!(
                    "--grant takes read, write, exec, or network, not {other}"
                )));
            }
        };
        options.grants.insert(grant);
    }
    options.keep_temp = args.switch("keep-temp");
    options.gate = gate_option(args)?;
    Ok(options)
}

/// The operator's `.openagents` directory.
pub(crate) fn openagents_home() -> PathBuf {
    std::env::var_os("OPENAGENTS_HOME").map_or_else(
        || {
            std::env::var_os("HOME")
                .map_or_else(|| PathBuf::from("."), PathBuf::from)
                .join(".openagents")
        },
        PathBuf::from,
    )
}

/// A resolved target: the extension's root, its package, the subject,
/// and the case a case-directory target names.
pub(crate) struct Target {
    pub(crate) root: PathBuf,
    pub(crate) package: Package,
    pub(crate) subject: Subject,
    pub(crate) lock: coder::package::Lock,
    pub(crate) only_case: Option<String>,
    pub(crate) installed: bool,
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// Resolves `target`: an installed identity, an extension directory, or a
/// case directory inside one.
pub(crate) fn resolve(target: &str) -> Result<Target, String> {
    let (root, only_case, installed) = if let Some((identity, version)) = target.split_once('@')
        && let Some((key, slug)) = identity.split_once(':')
        && is_hex64(key)
    {
        let dir = openagents_home()
            .join("extensions")
            .join(key)
            .join(slug)
            .join(version);
        if !dir.is_dir() {
            return Err(format!(
                "{target} is not installed on this computer (looked in {})",
                dir.display()
            ));
        }
        (dir, None, true)
    } else {
        let path = PathBuf::from(target)
            .canonicalize()
            .map_err(|error| format!("{target}: {error}"))?;
        if path.join("prompt.md").is_file() {
            let root = path
                .ancestors()
                .skip(1)
                .find(|dir| dir.join(PACKAGE_FILE).is_file())
                .ok_or_else(|| {
                    format!(
                        "{} is a test with no plugin around it (no {PACKAGE_FILE} above it)",
                        path.display()
                    )
                })?
                .to_path_buf();
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            (root, name, false)
        } else {
            (path, None, false)
        }
    };
    let record = root.join(PACKAGE_FILE);
    let bytes = std::fs::read(&record).map_err(|error| {
        format!(
            "{} has no package record ({PACKAGE_FILE}): {error}",
            root.display()
        )
    })?;
    let package = Package::load(&record)?;
    let lock = Package::resolve(&root, &package)
        .map_err(|refusal| format!("the package record doesn't resolve: {refusal}"))?;
    // A plugin that only runs in the background carries no program: its
    // tests admit its skills alone.
    let programs = match (&lock.program, &package.program) {
        (Some(pinned), Some(reference)) => {
            let path = root.join(&pinned.found);
            let bytes =
                std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            vec![Program {
                slug: reference.name.clone(),
                bytes,
            }]
        }
        _ => Vec::new(),
    };
    let mut skills = arms::skills_in(&root.join(SKILLS_DIR))?;
    skills.extend(background_skills(&root, &lock)?);
    // An unpublished extension is named under the local key, whoever runs
    // it, so a check by another trainer names the same subject.
    let publisher = if is_hex64(&package.publisher) {
        package.publisher.clone()
    } else {
        crate::ext_eval_init::LOCAL_KEY.to_string()
    };
    let name = package
        .program
        .as_ref()
        .map_or(package.slug.as_str(), |program| program.name.as_str());
    let definition = arms::definition(&publisher, &package.slug, name, &bytes);
    let subject = Subject {
        slug: package.slug.clone(),
        definition,
        package_lock: serde_json::to_value(&lock).unwrap_or(Value::Null),
        programs,
        skills,
    };
    Ok(Target {
        root,
        package,
        subject,
        lock,
        only_case,
        installed,
    })
}

/// A plugin's background rules, as the subject arm reads them: each pinned
/// rule document, under a line saying the host runs it on its own. A Coder
/// turn never runs the rule; what the plugin knows (what is safe to clean,
/// what never is, and that it previews first) is what a test can measure.
fn background_skills(root: &Path, lock: &coder::package::Lock) -> Result<Vec<arms::Skill>, String> {
    let mut skills = Vec::new();
    for (name, pin) in &lock.background {
        let path = root.join(&pin.found);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let body = format!(
            "# Background rule `{name}`\n\nThis plugin brings a rule the OpenAgents host runs on its own while the plugin is on. The host decides from this rule what is safe to delete, checks every safety rule itself, and shows a dry run first. The rule:\n\n```json\n{}\n```\n",
            text.trim()
        );
        skills.push(arms::Skill {
            name: format!("background-{name}"),
            bytes: body.into_bytes(),
        });
    }
    Ok(skills)
}

/// The agent binary: `--coder`, `coder` beside this program, or `coder`
/// on `PATH`.
fn agent_path(args: &Args) -> Result<PathBuf, String> {
    if let Some(path) = args.option("coder") {
        return PathBuf::from(path)
            .canonicalize()
            .map_err(|error| format!("--coder {path}: {error}"));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && dir.join("coder").is_file()
    {
        return Ok(dir.join("coder"));
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let candidate = dir.join("coder");
        if candidate.is_file() {
            return candidate
                .canonicalize()
                .map_err(|error| format!("{}: {error}", candidate.display()));
        }
    }
    Err("no coder binary: pass --coder PATH".into())
}

/// The agent both arms run, with its question sets.
fn agent(args: &Args) -> Result<AgentPin, String> {
    let path = agent_path(args)?;
    let questions = match args.option("questions") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => path
            .ancestors()
            .skip(1)
            .map(|dir| dir.join("questions"))
            .find(|dir| dir.join("program.json").is_file())
            .or_else(|| {
                let home = openagents_home().join("questions");
                home.is_dir().then_some(home)
            }),
    };
    let agent = AgentPin::of(path)?;
    match questions {
        Some(dir) => agent.with_questions(&dir),
        None => {
            eprintln!(
                "warning: no question sets found; the agent can't select programs (pass --questions DIR)"
            );
            Ok(agent)
        }
    }
}

/// The door table: `default` is the operator's `CODER_DOOR_*`; a lane
/// name (`gemini`, `glm`) is the same door running that lane's model.
fn door(name: Option<&str>) -> Result<Door, String> {
    let key = std::env::var("CODER_DOOR_KEY")
        .ok()
        .filter(|key| !key.is_empty())
        .or_else(|| {
            std::env::var("CODER_AI_GATEWAY_KEY")
                .ok()
                .filter(|key| !key.is_empty())
        })
        .ok_or_else(|| {
            "no door key: set CODER_DOOR_KEY or CODER_AI_GATEWAY_KEY in this shell".to_string()
        })?;
    let url = std::env::var("CODER_DOOR_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| coder::generate::DEFAULT_DOOR_URL.to_string());
    let (name, model) = match name {
        None | Some("default") => (
            "default".to_string(),
            coder::generate::model_from_env(coder::generate::MODEL_VAR)
                .unwrap_or_else(|| coder::generate::DEFAULT_MODEL.to_string()),
        ),
        Some(lane) => match coder::generate::Lane::read(lane) {
            Some(lane) => (lane.name().to_string(), lane.model().to_string()),
            None => {
                return Err(format!(
                    "no door named {lane}; the table has default, gemini, and glm"
                ));
            }
        },
    };
    Ok(Door {
        name,
        url,
        key: Secret::new(key),
        model,
    })
}

/// The decision door: `TYPESAFE_API_KEY` (and `TYPESAFE_BASE_URL`), or
/// the key in `~/.openagents/jev.json`, with Jev's other doors whose keys
/// are set (`AI_GATEWAY_API_KEY`, `OPENROUTER_API_KEY`) asked first.
fn decision_pin() -> Option<DecisionPin> {
    let key = std::env::var(jev::env::API_KEY)
        .ok()
        .filter(|key| !key.trim().is_empty())
        .or_else(|| {
            let path = openagents_home().join("jev.json");
            let value: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
            value
                .get("api_key")
                .and_then(Value::as_str)
                .map(str::to_string)
        })?;
    let url = std::env::var(jev::env::BASE_URL)
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| jev::defaults::BASE_URL.to_string());
    Some(DecisionPin::new(url, Secret::new(key)).with_fallbacks(&|var| std::env::var(var).ok()))
}

fn progress(event: Progress) {
    match event {
        Progress::Started {
            case, arm, attempt, ..
        } => eprintln!("  {case} {} #{attempt} …", arm.word()),
        Progress::Finished {
            case,
            arm,
            attempt,
            outcome,
            seconds,
        } => eprintln!(
            "  {case} {} #{attempt} {outcome} ({seconds:.1}s)",
            arm.word()
        ),
        Progress::Kept(path) => eprintln!("  kept {}", path.display()),
    }
}

/// Everything `run` and `check` share once the target is resolved.
struct Prepared {
    suite: Suite,
    target: Target,
    agent: AgentPin,
    door: Door,
    decision: Option<DecisionPin>,
    options: Options,
}

fn prepare(
    args: &Args,
    target: Target,
    suite: Suite,
    options: Options,
) -> Result<Prepared, String> {
    let named: BTreeSet<&str> = suite
        .cases
        .iter()
        .filter_map(|case| case.run.door.as_deref())
        .collect();
    let chosen = match (args.option("door"), named.len()) {
        (Some(flag), _) => Some(flag.to_string()),
        (None, 0) => None,
        (None, 1) => named.iter().next().map(|name| (*name).to_string()),
        (None, _) => {
            return Err(
                "the cases name different doors; pass --door to run them all through one".into(),
            );
        }
    };
    let door = door(chosen.as_deref())?;
    let agent = agent(args)?;
    Ok(Prepared {
        suite,
        target,
        agent,
        door,
        decision: decision_pin(),
        options,
    })
}

fn run_command(output: &Output, args: &Args) -> u8 {
    let positional = args.positional();
    let Some(target_word) = positional.first() else {
        return output.usage(NAME, "run needs a TARGET", USAGE);
    };
    let json_path = positional.get(1).cloned();
    if json_path.is_some() && !output.json() {
        return output.usage(NAME, "run takes one TARGET", USAGE);
    }
    let options = match options(args, false) {
        Ok(options) => options,
        Err(error) => return usage(output, error),
    };
    if let Err(error) = ext_eval::sandbox::confinement_available() {
        return output.fail(NAME, &format!("unconfined_host: {error}"));
    }
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail(NAME, &message),
    };
    let operator = signer.pubkey().to_string();
    let target = match resolve(target_word) {
        Ok(target) => target,
        Err(message) => return output.fail(NAME, &message),
    };
    if !target.installed {
        if let Err(error) = ext_eval::trust::check_owner(&target.root) {
            return output.fail(NAME, &error.to_string());
        }
        let store = ext_eval::trust::TrustStore::under(&openagents_home());
        let answer = if args.switch("trust") {
            ext_eval::trust::Answer::Flag
        } else {
            ext_eval::trust::Answer::Terminal
        };
        if let Err(error) = ext_eval::trust::decide(&store, &target.root, answer) {
            return output.fail(NAME, &error.to_string());
        }
    }
    let eval_dir = match ext_eval::eval_dir(
        &target.root,
        args.option("eval-dir"),
        target.package.eval_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => return output.usage(NAME, &error.to_string(), USAGE),
    };
    let suite = match Suite::load(&eval_dir, LoadOptions::default()) {
        Ok(suite) => suite,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    let mut filter = Filter {
        cases: args
            .options("case")
            .into_iter()
            .map(str::to_string)
            .collect(),
        tags: args
            .options("tag")
            .into_iter()
            .map(str::to_string)
            .collect(),
    };
    if let Some(case) = &target.only_case {
        filter.cases = vec![case.clone()];
    }
    let suite = suite.filtered(&filter);
    if suite.cases.is_empty() {
        return output.fail(NAME, "no case matches --case and --tag");
    }
    let results_base = match args.option("output-dir") {
        Some(dir) => PathBuf::from(dir),
        None if target.installed => std::env::current_dir().unwrap_or_default().join("results"),
        None => eval_dir.join("results"),
    };
    let prepared = match prepare(args, target, suite, options) {
        Ok(prepared) => prepared,
        Err(message) => return output.fail(NAME, &message),
    };
    let author = Author {
        author: operator.clone(),
        package: suite_package(&prepared.target.package.slug),
        component: ext_eval::publish::SUITE_COMPONENT.to_string(),
        evaluator: operator,
        suite_release: None,
        requester: None,
    };
    execute(
        output,
        &prepared,
        &author,
        &results_base,
        json_path.as_deref(),
        false,
    )
    .0
}

/// The package slug a suite for extension `slug` is published under.
fn suite_package(slug: &str) -> String {
    let base: String = slug.chars().take(58).collect();
    format!("{base}-tests")
}

/// Runs a prepared suite and prints its summary, on stderr when `quiet`.
/// Returns the exit code and, when it finished, the results directory.
fn execute(
    output: &Output,
    prepared: &Prepared,
    author: &Author,
    results_base: &Path,
    json_path: Option<&str>,
    quiet: bool,
) -> (u8, Option<PathBuf>) {
    let mut held = HeldLock::open(&prepared.target.lock);
    for warning in prepared
        .suite
        .cases
        .iter()
        .flat_map(ext_eval::Case::warnings)
    {
        eprintln!("warning: {warning}");
    }
    let setup = Setup {
        suite: &prepared.suite,
        subject: &prepared.target.subject,
        agent: &prepared.agent,
        door: &prepared.door,
        decision: prepared.decision.as_ref(),
        options: &prepared.options,
    };
    let jev = prepared
        .decision
        .as_ref()
        .and_then(|pin| pin.jev_door(None).ok());
    let cancel = Cancel::on_signals();
    eprintln!(
        "running {} case(s) of {} through {} …",
        prepared.suite.cases.len(),
        prepared.target.subject.slug,
        prepared.door.label()
    );
    let outcome = run::run_suite(
        &setup,
        author,
        results_base,
        jev.as_ref().map(|door| door as &dyn ext_eval::DecisionDoor),
        &cancel,
        &progress,
    );
    // The lock held for the whole run: an extension that changed under
    // the run is a result about bytes nobody pinned.
    if let Ok(now) = Package::resolve(&prepared.target.root, &prepared.target.package)
        && let Err(refusal) = held.consider(&now)
    {
        eprintln!("warning: the plugin changed during the run: {refusal}");
    }
    held.finish();
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => return (output.fail(NAME, &error.to_string()), None),
    };
    for path in &outcome.kept {
        eprintln!("kept {}", path.display());
    }
    let report = &outcome.evaluation.report;
    if quiet {
        eprintln!("{}", summary(&outcome.evaluation, &outcome.results));
    } else if output.json() {
        match json_path {
            Some(path) => {
                if let Err(error) = std::fs::write(path, &outcome.evaluation.report_bytes) {
                    return (output.fail(NAME, &format!("{path}: {error}")), None);
                }
            }
            None => println!("{report}"),
        }
    } else {
        println!("{}", summary(&outcome.evaluation, &outcome.results));
    }
    (
        u8::try_from(outcome.exit_code).unwrap_or(EXIT_FAILURE),
        Some(outcome.results),
    )
}

fn summary(evaluation: &ext_eval::Evaluation, results: &Path) -> String {
    let scores = &evaluation.scores;
    let total = scores.cases.len();
    let mut lines = vec![match &scores.baseline {
        Some(baseline) => format!(
            "{}: passes {} of {total} tests with the plugin, {} without.",
            evaluation.verdict.plain(),
            scores.subject.cases_passed,
            baseline.cases_passed
        ),
        None => format!(
            "Passes {} of {total} tests with the plugin (no run without it, so no verdict).",
            scores.subject.cases_passed
        ),
    }];
    // Time and cost never make a tool Better (ext-eval-v2); they are
    // stated beside the verdict instead.
    for note in ext_eval::notes(scores) {
        lines.push(format!("{note}."));
    }
    if let Some(partial) = &evaluation.partial {
        lines.push(format!("Partial: {partial}."));
    }
    let mut rows = vec![vec![
        "test".to_string(),
        "kind".to_string(),
        "with".to_string(),
        "without".to_string(),
        "change".to_string(),
    ]];
    for case in &scores.cases {
        let arm = |arm: &ext_eval::score::CaseArm| format!("{}/{}", arm.runs_passed, arm.planned);
        rows.push(vec![
            case.id.clone(),
            case.kind.word().to_string(),
            arm(&case.subject),
            case.baseline.as_ref().map_or("-".to_string(), arm),
            case.change
                .map_or("-".to_string(), |change| format!("{change:+.2}")),
        ]);
    }
    lines.push(crate::out::table(&rows));
    lines.push(format!("report: {}", results.join("report.json").display()));
    lines.join("\n")
}

fn init(output: &Output, args: &Args) -> u8 {
    let Some(name) = args.positional().first() else {
        return output.usage(NAME, "init --bare needs a NAME for the case", USAGE);
    };
    if let Err(error) = ext_eval::case::check_name(name, "NAME", "case") {
        return output.usage(NAME, &error.to_string(), USAGE);
    }
    let root = std::env::current_dir().unwrap_or_default();
    let record = Package::load(&root.join(PACKAGE_FILE)).ok();
    let eval_dir = match ext_eval::eval_dir(
        &root,
        args.option("eval-dir"),
        record
            .as_ref()
            .and_then(|package| package.eval_dir.as_deref()),
    ) {
        Ok(dir) => dir,
        Err(error) => return output.usage(NAME, &error.to_string(), USAGE),
    };
    let dir = eval_dir.join(name);
    if dir.exists() {
        return output.fail(NAME, &format!("{} already exists", dir.display()));
    }
    let written = std::fs::create_dir_all(dir.join("graders"))
        .and_then(|()| std::fs::write(dir.join("prompt.md"), TEMPLATE_PROMPT))
        .and_then(|()| std::fs::write(dir.join("graders/criteria.md"), TEMPLATE_GRADER));
    if let Err(error) = written {
        return output.fail(NAME, &format!("{}: {error}", dir.display()));
    }
    output.emit(
        &json!({"case": name, "dir": dir.display().to_string()}),
        |value| {
            format!(
                "wrote {}/prompt.md and graders/criteria.md; replace each TODO line before running it",
                value["dir"].as_str().unwrap_or_default()
            )
        },
    );
    0
}

/// `openagents plugin test init NAME --bare` writes this `prompt.md`.
pub const TEMPLATE_PROMPT: &str = "+++
v = \"openagents.eval-case.v1\"
kind = \"should-fire\"
+++

TODO: describe a task someone would give Coder
";

/// And this `graders/criteria.md`.
pub const TEMPLATE_GRADER: &str = "+++
type = \"decision\"
question = \"Did the run do what the task asked?\"
threshold = 0.7
+++

TODO: describe what a successful run looks like
";

/// Fetches events matching `filter` from `client`.
pub(crate) fn fetch(client: &mut Client, filter: Value) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    client.subscribe(vec![filter], false, DEFAULT_WAIT, |event| {
        events.push(event.clone());
    })?;
    Ok(events)
}

pub(crate) fn blossom(args: &Args, relay: &str) -> Result<ext_eval::blob::Blossom, String> {
    match args.option("blossom") {
        Some(base) => ext_eval::blob::Blossom::new(base),
        None => ext_eval::blob::Blossom::for_relay(relay),
    }
}

/// What a publish sent.
struct Sent {
    suite_release: Value,
    result: Event,
}

fn publish(output: &Output, args: &Args) -> u8 {
    let Some(report) = args.positional().first() else {
        return output.usage(NAME, "publish needs a REPORT (a report.json path)", USAGE);
    };
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail(NAME, &message),
    };
    let relay = relay_url(args.option("relay"));
    let validates = args.option("validates");
    if let Some(id) = validates
        && !is_hex64(id)
    {
        return output.usage(
            NAME,
            "--validates takes a result's event id: 64 lowercase hex digits",
            USAGE,
        );
    }
    let cites = validates.map(nostr::eval_ext::Cites::Validates);
    match publish_results(Path::new(report), &signer, &relay, args, cites) {
        Ok(sent) => {
            output.emit(
                &json!({
                    "relay": relay,
                    "suite_release": sent.suite_release,
                    "result": sent.result.id,
                }),
                |value| {
                    format!(
                        "added to the Gym: result {} (suite release {}) on {}",
                        value["result"].as_str().unwrap_or_default(),
                        value["suite_release"]["id"].as_str().unwrap_or_default(),
                        value["relay"].as_str().unwrap_or_default()
                    )
                },
            );
            0
        }
        Err(message) => output.fail(NAME, &message),
    }
}

/// Publishes a results directory: the suite release (once) and the
/// `3189` (once). `cites` names the publication a check checks or a
/// validation validates.
fn publish_results(
    report: &Path,
    signer: &nostr::domain::RelaySigner,
    relay: &str,
    args: &Args,
    cites: Option<nostr::eval_ext::Cites<'_>>,
) -> Result<Sent, String> {
    let results = ext_eval::publish::Results::open(report).map_err(|error| error.to_string())?;
    if results.evaluator() != signer.pubkey() {
        return Err(format!(
            "the report's evaluator is {}; publish it with that key (--as PROFILE)",
            results.evaluator()
        ));
    }
    let mut record = ext_eval::publish::Record::read(&results.dir);
    let mut client = Client::connect(relay, signer.clone());
    let wait = Duration::from_secs(10);
    let suite_release = match results.suite_release() {
        Some(release) => release,
        None => {
            if results.author() != signer.pubkey() {
                return Err(format!(
                    "the suite's author is {}; publish its release with that key (--as PROFILE)",
                    results.author()
                ));
            }
            let release =
                ext_eval::publish::suite_release(&results).map_err(|error| error.to_string())?;
            let store = blossom(args, relay)?;
            let blobs = release.blobs();
            eprintln!(
                "uploading the suite's {} files to {} (a relay's upload limit can make this take a few minutes) …",
                blobs.len(),
                store.base()
            );
            for (bytes, media) in blobs {
                store.upload(signer, &bytes, &media, unix_now())?;
                record.blobs.insert(
                    nostr::contracts::digest_bytes(&bytes),
                    store.base().to_string(),
                );
            }
            let unsigned = release.event();
            let existing = fetch(
                &mut client,
                json!({"kinds": [unsigned.kind], "authors": [signer.pubkey()], "#t": ["oa:ext:release:v1"]}),
            )?
            .into_iter()
            .find(|event| event.content == unsigned.content);
            let event = match existing {
                Some(event) => event,
                None => {
                    let event =
                        signer.sign(unix_now(), unsigned.kind, unsigned.tags, unsigned.content);
                    let published = client.publish(event.clone(), wait)?;
                    if !published.accepted {
                        return Err(format!(
                            "the relay refused the suite release: {}",
                            published.message
                        ));
                    }
                    event
                }
            };
            record.suite_release = Some(event.id.clone());
            json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind})
        }
    };
    let report_bytes = if results.suite_release().is_some() {
        results.report.clone()
    } else {
        let bytes = ext_eval::publish::with_suite_release(&results.value, &suite_release)
            .map_err(|error| error.to_string())?;
        std::fs::write(results.dir.join("report.json"), &bytes)
            .map_err(|error| format!("report.json: {error}"))?;
        bytes
    };
    let unsigned = ext_eval::publish::result_event_citing(&report_bytes, cites)
        .map_err(|error| error.to_string())?;
    let digest = nostr::contracts::digest_bytes(&report_bytes);
    let existing = fetch(
        &mut client,
        json!({"kinds": [unsigned.kind], "authors": [signer.pubkey()], "#x": [digest.trim_start_matches("sha256:")]}),
    )?
    .into_iter()
    .find(|event| event.content == unsigned.content);
    let result = match existing {
        Some(event) => event,
        None => {
            let event = signer.sign(unix_now(), unsigned.kind, unsigned.tags, unsigned.content);
            let published = client.publish(event.clone(), wait)?;
            if !published.accepted {
                return Err(format!(
                    "the relay refused the result: {}",
                    published.message
                ));
            }
            event
        }
    };
    nostr::eval_ext::parse_publication(&result).map_err(|error| error.to_string())?;
    client.close();
    record.result = Some(result.id.clone());
    record.relay = Some(relay.to_string());
    record
        .write(&results.dir)
        .map_err(|error| format!("published.json: {error}"))?;
    Ok(Sent {
        suite_release,
        result,
    })
}

/// `release`: the suite of TARGET as a NIP-EXT release under the caller's
/// key, without a run. The suite's files go to the Blossom server or, with
/// `--blobs-dir`, to a directory by digest for an operator to upload.
fn release(output: &Output, args: &Args) -> u8 {
    let positional = args.positional();
    let Some(target_word) = positional.first() else {
        return output.usage(NAME, "release needs a TARGET", USAGE);
    };
    let gate = match gate_option(args) {
        Ok(gate) => gate,
        Err(error) => return usage(output, error),
    };
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail(NAME, &message),
    };
    let target = match resolve(target_word) {
        Ok(target) => target,
        Err(message) => return output.fail(NAME, &message),
    };
    let eval_dir = match ext_eval::eval_dir(
        &target.root,
        args.option("eval-dir"),
        target.package.eval_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => return output.usage(NAME, &error.to_string(), USAGE),
    };
    let suite = match Suite::load(&eval_dir, LoadOptions::default()) {
        Ok(suite) => suite,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    if suite.cases.is_empty() {
        return output.fail(NAME, "the test set has no tests");
    }
    let package = suite_package(&target.package.slug);
    let documents = ext_eval::evaluate::suite_documents_under(
        gate.as_deref().unwrap_or(ext_eval::GATE_ID),
        &suite,
        signer.pubkey(),
        &package,
        ext_eval::publish::SUITE_COMPONENT,
    );
    let (suite_bytes, cases_bytes) = match documents {
        Ok(documents) => documents,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    let results = ext_eval::publish::Results::of_suite(suite, suite_bytes, cases_bytes);
    let release = match ext_eval::publish::suite_release(&results) {
        Ok(release) => release,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    let relay = relay_url(args.option("relay"));
    let blobs = release.blobs();
    let mut written = Vec::new();
    match args.option("blobs-dir") {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if let Err(error) = std::fs::create_dir_all(&dir) {
                return output.fail(NAME, &format!("{}: {error}", dir.display()));
            }
            for (bytes, _) in &blobs {
                let digest = nostr::contracts::digest_bytes(bytes);
                let path = dir.join(digest.trim_start_matches("sha256:"));
                if let Err(error) = std::fs::write(&path, bytes) {
                    return output.fail(NAME, &format!("{}: {error}", path.display()));
                }
                written.push(path.display().to_string());
            }
        }
        None => {
            let store = match blossom(args, &relay) {
                Ok(store) => store,
                Err(message) => return output.fail(NAME, &message),
            };
            eprintln!(
                "uploading the suite's {} files to {} …",
                blobs.len(),
                store.base()
            );
            for (bytes, media) in &blobs {
                if let Err(message) = store.upload(&signer, bytes, media, unix_now()) {
                    return output.fail(NAME, &message);
                }
            }
        }
    }
    let unsigned = release.event();
    let mut client = Client::connect(&relay, signer.clone());
    let existing = match fetch(
        &mut client,
        json!({"kinds": [unsigned.kind], "authors": [signer.pubkey()], "#t": ["oa:ext:release:v1"]}),
    ) {
        Ok(found) => found
            .into_iter()
            .find(|event| event.content == unsigned.content),
        Err(message) => return output.fail(NAME, &message),
    };
    let (event, reused) = match existing {
        Some(event) => (event, true),
        None => {
            let event = signer.sign(unix_now(), unsigned.kind, unsigned.tags, unsigned.content);
            match client.publish(event.clone(), Duration::from_secs(10)) {
                Ok(published) if published.accepted => (event, false),
                Ok(published) => {
                    return output.fail(
                        NAME,
                        &format!("the relay refused the suite release: {}", published.message),
                    );
                }
                Err(message) => return output.fail(NAME, &message),
            }
        }
    };
    client.close();
    output.emit(
        &json!({
            "relay": relay,
            "release": {"id": event.id, "pubkey": event.pubkey, "kind": event.kind},
            "package": release.package,
            "version": release.version,
            "suite": results.suite_id(),
            "suite_digest": nostr::contracts::digest_bytes(&results.suite),
            "gate": gate.unwrap_or_else(|| ext_eval::GATE_ID.to_string()),
            "reused": reused,
            "blobs": blobs.iter().map(|(bytes, _)| nostr::contracts::digest_bytes(bytes)).collect::<Vec<_>>(),
            "written": written,
        }),
        |value| {
            format!(
                "{} {} as release {} on {} ({} files{})",
                if value["reused"].as_bool().unwrap_or(false) {
                    "already released"
                } else {
                    "released"
                },
                value["suite"].as_str().unwrap_or_default(),
                value["release"]["id"].as_str().unwrap_or_default(),
                value["relay"].as_str().unwrap_or_default(),
                value["blobs"].as_array().map_or(0, Vec::len),
                if value["written"].as_array().is_some_and(|w| !w.is_empty()) {
                    ", written to --blobs-dir for upload"
                } else {
                    ""
                }
            )
        },
    );
    0
}

fn check(output: &Output, args: &Args) -> u8 {
    let positional = args.positional();
    let Some(event_id) = positional.first() else {
        return output.usage(NAME, "check needs the result's EVENT id", USAGE);
    };
    if !is_hex64(event_id) {
        return output.usage(
            NAME,
            "EVENT is a result's event id: 64 lowercase hex digits",
            USAGE,
        );
    }
    let target_word = positional.get(1).map_or(".", String::as_str);
    let options = match options(args, true) {
        Ok(options) => options,
        Err(error) => return usage(output, error),
    };
    if let Err(error) = ext_eval::sandbox::confinement_available() {
        return output.fail(NAME, &format!("unconfined_host: {error}"));
    }
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail(NAME, &message),
    };
    let relay = relay_url(args.option("relay"));
    let mut client = Client::connect(&relay, signer.clone());
    let found = match fetch(&mut client, json!({"ids": [event_id]})) {
        Ok(found) => found,
        Err(message) => return output.fail(NAME, &message),
    };
    let Some(original_event) = found.into_iter().next() else {
        return output.fail(NAME, &format!("{relay} has no event {event_id}"));
    };
    let original = match ext_eval::check::read_result(&original_event) {
        Ok(original) => original,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    let release = match fetch(&mut client, json!({"ids": [original.suite_release.id]})) {
        Ok(found) => found.into_iter().next(),
        Err(message) => return output.fail(NAME, &message),
    };
    client.close();
    let Some(release) = release else {
        return output.fail(NAME, "the relay has no release of the result's suite");
    };
    let store = match blossom(args, &relay) {
        Ok(store) => store,
        Err(message) => return output.fail(NAME, &message),
    };
    let staging = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    let materialized =
        match ext_eval::check::materialize(&release, &|digest| store.fetch(digest), staging.path())
        {
            Ok(materialized) => materialized,
            Err(error) => return output.fail(NAME, &error.to_string()),
        };
    let mut target = match resolve(target_word) {
        Ok(target) => target,
        Err(message) => return output.fail(NAME, &message),
    };
    if !target.installed {
        let store = ext_eval::trust::TrustStore::under(&openagents_home());
        let answer = if args.switch("trust") {
            ext_eval::trust::Answer::Flag
        } else {
            ext_eval::trust::Answer::Terminal
        };
        if let Err(error) = ext_eval::trust::check_owner(&target.root)
            .and_then(|()| ext_eval::trust::decide(&store, &target.root, answer))
        {
            return output.fail(NAME, &error.to_string());
        }
    }
    // The subject is named as the original named it, down to its release.
    if let Some(event) = original_event_ref(&original) {
        target.subject.definition["event"] = event;
    }
    let prepared = match agent(args) {
        Ok(agent) => agent,
        Err(message) => return output.fail(NAME, &message),
    };
    let lock = target.subject.lock_document(&prepared);
    if let Err(error) = ext_eval::check::same_subject(&original, &target.subject.definition, &lock)
    {
        return output.fail(NAME, &error.to_string());
    }
    let suite = match Suite::load(&materialized.eval_dir, LoadOptions::default()) {
        Ok(suite) => suite,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    let mut options = options;
    options.baseline = original.report.baseline.is_some();
    let prepared = match prepare(args, target, suite, options) {
        Ok(prepared) => prepared,
        Err(message) => return output.fail(NAME, &message),
    };
    let author = Author {
        author: materialized.author.clone(),
        package: materialized.package.clone(),
        component: materialized.component.clone(),
        evaluator: signer.pubkey().to_string(),
        suite_release: Some(materialized.release.clone()),
        requester: None,
    };
    let results_base = args.option("output-dir").map_or_else(
        || std::env::current_dir().unwrap_or_default().join("results"),
        PathBuf::from,
    );
    let (code, results) = execute(output, &prepared, &author, &results_base, None, true);
    let Some(results) = results else {
        return code;
    };
    if code == 2 || code >= 128 {
        eprintln!("the check did not finish; nothing was published");
        return code;
    }
    let report = match std::fs::read(results.join("report.json")) {
        Ok(report) => report,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    let rerun_suite = serde_json::from_slice::<Value>(&report)
        .ok()
        .and_then(|value| value["suite"]["digest"].as_str().map(str::to_string));
    if rerun_suite.as_deref() != Some(original.report.suite.digest.as_str()) {
        return output.fail(
            NAME,
            "the rerun's suite is not byte-for-byte the published suite; nothing was published",
        );
    }
    let sent = match publish_results(
        &results,
        &signer,
        &relay,
        args,
        Some(nostr::eval_ext::Cites::Check(&original.id)),
    ) {
        Ok(sent) => sent,
        Err(message) => return output.fail(NAME, &message),
    };
    let check_publication = match ext_eval::check::read_result(&sent.result) {
        Ok(publication) => publication,
        Err(error) => return output.fail(NAME, &error.to_string()),
    };
    let linkage = ext_eval::check::linkage(&original, &check_publication);
    let word = ext_eval::check::linkage_word(linkage);
    output.emit(
        &json!({
            "relay": relay,
            "original": original.id,
            "check": sent.result.id,
            "linkage": word,
            "original_verdict": original.report.verdict.word(),
            "check_verdict": check_publication.report.verdict.word(),
            "results": results.display().to_string(),
        }),
        |value| {
            format!(
                "check {} {} the result {} ({} vs {})",
                value["check"].as_str().unwrap_or_default(),
                match value["linkage"].as_str().unwrap_or_default() {
                    "confirm" => "confirms",
                    "dispute" => "disputes",
                    _ => "doesn't check",
                },
                value["original"].as_str().unwrap_or_default(),
                value["check_verdict"].as_str().unwrap_or_default(),
                value["original_verdict"].as_str().unwrap_or_default(),
            )
        },
    );
    match linkage {
        nostr::eval_ext::Linkage::Confirm => 0,
        _ => EXIT_FAILURE,
    }
}

/// The original subject's release EventRef, when it has one.
fn original_event_ref(original: &nostr::eval_ext::Publication) -> Option<Value> {
    original
        .report
        .subject
        .definition
        .event
        .as_ref()
        .map(|event| json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_is_the_spec_s() {
        assert!(TEMPLATE_PROMPT.starts_with("+++\nv = \"openagents.eval-case.v1\"\n"));
        assert!(TEMPLATE_GRADER.contains("type = \"decision\""));
        assert!(TEMPLATE_GRADER.contains("threshold = 0.7"));
    }

    #[test]
    fn a_suite_package_slug_fits() {
        assert_eq!(suite_package("repo-map"), "repo-map-tests");
        assert!(suite_package(&"a".repeat(64)).len() <= 64);
    }

    #[test]
    fn options_bound_runs_concurrency_and_grants() {
        let args = |words: &[&str]| {
            Args::parse(
                &words.iter().map(|w| (*w).to_string()).collect::<Vec<_>>(),
                SWITCHES,
            )
            .unwrap()
        };
        assert!(options(&args(&["--runs", "11"]), false).is_err());
        assert!(options(&args(&["--concurrency", "9"]), false).is_err());
        assert!(options(&args(&["--grant", "root"]), false).is_err());
        assert!(options(&args(&["--baseline", "maybe"]), false).is_err());
        let parsed = options(
            &args(&["--runs", "1", "--grant", "write", "--baseline", "off"]),
            false,
        )
        .unwrap();
        assert_eq!(parsed.runs, Some(1));
        assert!(!parsed.baseline);
        assert!(parsed.grants.contains(&Grant::Write));
    }
}
