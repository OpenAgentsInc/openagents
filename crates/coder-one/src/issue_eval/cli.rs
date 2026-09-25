//! `coder-one issue-eval …`: list the set, grade a checkout or commit,
//! verify the graders, and run the issue flow on an entry.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::grade::{Grade, grade};
use super::run::{Options, default_runs_dir, default_target_dir, run};
use super::{Part, Set, checkout, default_set_dir, load, seal, source_of};

/// The commands' usage.
pub const USAGE: &str = "usage: coder-one issue-eval list [--json]
       coder-one issue-eval show ID [--json]
       coder-one issue-eval grade ID (--dir PATH | --commit REV | --base | --fix) [--json]
       coder-one issue-eval verify [ID...] [--part development|held-out] [--json]
       coder-one issue-eval run ID [--held-out] [--jev live|off] [--model MODEL]
                               [--policy MANIFEST] [--script NAME=FILE] [--out DIR] [--json]
       coder-one issue-eval seal
   every command takes [--set DIR] [--source DIR] [--target-dir DIR]

The set is a pinned list of past issues from this repository, each with the
commit before its fix and a grader: the tests the real fix added or changed,
and checks for the issue's stated deliverables. It is split into a
development part, for working on an issue-flow change, and a held-out part,
for confirming a change once it is chosen.

list shows every entry with its part and category. show prints one entry.
grade grades a checkout (--dir), a commit (--commit), the base (--base), or
the real fix (--fix) in a scratch clone. verify grades the base and the fix of
each entry and reports whether the grader discriminates: the base must fail
and the fix must pass. run works the entry through the issue flow in a
scratch clone at its base commit, publishing nothing, grades the result, and
records the run with its time and its Luna and Jev cost under
~/.openagents/coder-one/issue-evals (or --out), where
`gym coder minitasks --runs-dir ~/.openagents/coder-one/issue-evals` lists
it. A held-out entry runs only with --held-out. --policy names the
Microluna manifest the flow runs under, a reference file name or a path;
without it, the flow takes its own, as CODER_ISSUE_POLICY picks it. --script
plays scripted Luna replies instead of the Codex login: a JSON array whose
items are {\"call\": TOOL, \"arguments\": {...}} or {\"say\": TEXT}.
seal rewrites the manifest's digests after an entry is changed on purpose.

--set defaults to crates/coder-one/issues-eval, --source to the git checkout
holding the current directory, and --target-dir to CARGO_TARGET_DIR or
~/.openagents/coder-one/issue-evals/target.";

struct Args {
    positional: Vec<String>,
    set: PathBuf,
    source: Option<PathBuf>,
    target_dir: Option<PathBuf>,
    out: Option<PathBuf>,
    json: bool,
    held_out: bool,
    jev: String,
    model: Option<String>,
    script: Option<String>,
    policy: Option<String>,
    part: Option<Part>,
    what: Option<What>,
}

/// What `grade` grades.
enum What {
    Dir(PathBuf),
    Commit(String),
    Base,
    Fix,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        positional: Vec::new(),
        set: default_set_dir(),
        source: None,
        target_dir: None,
        out: None,
        json: false,
        held_out: false,
        jev: "live".to_string(),
        model: None,
        script: None,
        policy: None,
        part: None,
        what: None,
    };
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        let mut value = || {
            index += 1;
            args.get(index)
                .cloned()
                .ok_or_else(|| format!("{argument} needs a value"))
        };
        match argument {
            "--json" => parsed.json = true,
            "--held-out" => parsed.held_out = true,
            "--base" => parsed.what = Some(What::Base),
            "--fix" => parsed.what = Some(What::Fix),
            "--dir" => parsed.what = Some(What::Dir(value()?.into())),
            "--commit" => parsed.what = Some(What::Commit(value()?)),
            "--set" => parsed.set = value()?.into(),
            "--source" => parsed.source = Some(value()?.into()),
            "--target-dir" => parsed.target_dir = Some(value()?.into()),
            "--out" => parsed.out = Some(value()?.into()),
            "--model" => parsed.model = Some(value()?),
            "--script" => parsed.script = Some(value()?),
            "--policy" => parsed.policy = Some(value()?),
            "--jev" => {
                parsed.jev = value()?;
                if !["live", "off"].contains(&parsed.jev.as_str()) {
                    return Err("--jev takes live or off".to_string());
                }
            }
            "--part" => {
                parsed.part = Some(match value()?.as_str() {
                    "development" => Part::Development,
                    "held-out" => Part::HeldOut,
                    other => return Err(format!("no part {other}: development or held-out")),
                });
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown option {other}\n\n{USAGE}"));
            }
            other => parsed.positional.push(other.to_string()),
        }
        index += 1;
    }
    Ok(parsed)
}

impl Args {
    fn source(&self) -> Result<PathBuf, String> {
        match &self.source {
            Some(source) => Ok(source.clone()),
            None => source_of(
                &std::env::current_dir()
                    .map_err(|error| format!("no current directory: {error}"))?,
            ),
        }
    }

    fn target_dir(&self) -> Result<PathBuf, String> {
        self.target_dir
            .clone()
            .or_else(default_target_dir)
            .ok_or_else(|| "no --target-dir, CARGO_TARGET_DIR, or HOME".to_string())
    }
}

fn print(value: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
    );
    Ok(())
}

/// Runs an issue-eval command and returns the exit code.
///
/// # Errors
///
/// Returns a message for bad arguments, a set that doesn't load, or a
/// scratch clone that can't be made.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let args = parse(rest)?;
    if verb == "seal" {
        let digest = seal(&args.set)?;
        println!("sealed {} · set digest {digest}", args.set.display());
        return Ok(0);
    }
    let set = load(&args.set)?;
    match verb.as_str() {
        "list" => list(&set, &args),
        "show" => {
            let id = args.positional.first().ok_or("show needs an entry ID")?;
            let (entry, part, digest) = set.find(id)?;
            if args.json {
                print(&json!({ "part": part.word(), "digest": digest, "entry": entry }))?;
            } else {
                println!(
                    "{} · {} · {} · #{}\nbase {} · fix {}\n\n{}\n",
                    entry.id,
                    part.word(),
                    entry.category,
                    entry.issue.number,
                    entry.base,
                    entry.fix.head,
                    entry.request()
                );
                for check in &entry.checks {
                    println!("  [{}] {}: {}", check.group, check.id, check.note);
                }
            }
            Ok(0)
        }
        "grade" => {
            let id = args.positional.first().ok_or("grade needs an entry ID")?;
            let what = args
                .what
                .as_ref()
                .ok_or("grade needs --dir, --commit, --base, or --fix")?;
            let (entry, _, _) = set.find(id)?.clone();
            let target = args.target_dir()?;
            let (graded, scratch) = match what {
                What::Dir(dir) => (grade(&set, &entry, dir, &target), None),
                What::Commit(commit) => graded_at(&set, &args, id, Some(commit))?,
                What::Base => graded_at(&set, &args, id, None)?,
                What::Fix => graded_at(&set, &args, id, Some(&entry.fix.head))?,
            };
            if let Some(scratch) = scratch {
                let _ = std::fs::remove_dir_all(scratch);
            }
            if args.json {
                print(&json!({ "entry": entry.id, "grade": graded }))?;
            } else {
                print_grade(&entry.id, &graded);
            }
            Ok(i32::from(graded.verdict != "passed"))
        }
        "verify" => verify(&set, &args),
        "run" => {
            let id = args.positional.first().ok_or("run needs an entry ID")?;
            let (_, part, _) = set.find(id)?;
            if *part == Part::HeldOut && !args.held_out {
                return Err(format!(
                    "{id} is held out: run it only to confirm a change chosen on the development \
                     part, and pass --held-out to say so"
                ));
            }
            let jev = if args.jev == "live" {
                let dir = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
                let key = crate::credentials::jev_key(|name| std::env::var(name).ok(), &dir)?;
                Some(crate::credentials::jev_client(&key.secret)?)
            } else {
                None
            };
            let script = match &args.script {
                None => None,
                Some(spec) => {
                    let (name, file) = spec.split_once('=').unwrap_or(("script", spec));
                    let text = std::fs::read_to_string(file)
                        .map_err(|error| format!("cannot read {file}: {error}"))?;
                    Some((name.to_string(), replies(&text, file)?))
                }
            };
            let out = args
                .out
                .clone()
                .or_else(default_runs_dir)
                .ok_or("no --out and no HOME")?;
            let ran = run(Options {
                source: args.source()?,
                target_dir: args.target_dir()?,
                set: set.clone(),
                id: id.clone(),
                out,
                jev,
                model: args.model.clone(),
                script,
                policy: match &args.policy {
                    Some(named) => Some(crate::terminal::load(named)?),
                    None => None,
                },
                quiet: args.json,
            })
            .await?;
            if args.json {
                print(&ran.manifest)?;
            } else {
                print_grade(id, &ran.grade);
                let cost = &ran.manifest["cost"];
                println!(
                    "outcome {} · {:.1} s · Luna {} · Jev {}\nrecorded in {}",
                    ran.manifest["outcome"].as_str().unwrap_or("?"),
                    ran.manifest["milliseconds"].as_f64().unwrap_or_default() / 1000.0,
                    usd(&cost["luna_usd"]),
                    usd(&cost["jev_usd"]),
                    ran.dir.display()
                );
            }
            Ok(i32::from(ran.grade.verdict != "passed"))
        }
        other => Err(format!("unknown command {other}\n\n{USAGE}")),
    }
}

/// Scripted Luna replies from a JSON array: each item is
/// `{"call": TOOL, "arguments": {…}}` or `{"say": TEXT}`.
fn replies(text: &str, file: &str) -> Result<Vec<microluna::Reply>, String> {
    let items: Vec<Value> = serde_json::from_str(text)
        .map_err(|error| format!("{file} is not a JSON array: {error}"))?;
    let usage = microluna::TokenUsage {
        input: 0,
        cached: 0,
        output: 0,
        reasoning: 0,
    };
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if let Some(tool) = item["call"].as_str() {
                Ok(microluna::fake::call(
                    &format!("c{}", i + 1),
                    tool,
                    &item["arguments"],
                    usage,
                ))
            } else if let Some(said) = item["say"].as_str() {
                Ok(microluna::fake::say(said, usage))
            } else {
                Err(format!(
                    "{file}: item {} is neither a call nor a say",
                    i + 1
                ))
            }
        })
        .collect()
}

fn usd(value: &Value) -> String {
    value
        .as_f64()
        .map_or_else(|| "unknown".to_string(), |usd| format!("${usd:.4}"))
}

fn list(set: &Set, args: &Args) -> Result<i32, String> {
    if args.json {
        print(&json!({
            "set": set.dir.display().to_string(),
            "digest": set.digest,
            "entries": set.entries.iter().map(|(entry, part, digest)| json!({
                "id": entry.id,
                "part": part.word(),
                "category": entry.category,
                "issue": entry.issue.number,
                "title": entry.issue.title,
                "base": entry.base,
                "fix": entry.fix.head,
                "checks": entry.checks.len(),
                "digest": digest,
            })).collect::<Vec<_>>(),
        }))?;
        return Ok(0);
    }
    println!(
        "Issue-flow evaluation set · {} entries · digest {}",
        set.entries.len(),
        &set.digest[..16]
    );
    for part in [Part::Development, Part::HeldOut] {
        println!("\n{}:", part.word());
        for (entry, _, _) in set.entries.iter().filter(|(_, p, _)| *p == part) {
            println!(
                "  {:<38} {:<20} {:>2} checks  {}",
                entry.id,
                entry.category,
                entry.checks.len(),
                entry.issue.title
            );
        }
    }
    Ok(0)
}

/// Grades entry `id` in a scratch clone at its base, or at `commit` when
/// given. Returns the grade and the scratch directory to remove.
fn graded_at(
    set: &Set,
    args: &Args,
    id: &str,
    commit: Option<&str>,
) -> Result<(Grade, Option<PathBuf>), String> {
    let (entry, _, _) = set.find(id)?;
    let scratch = std::env::temp_dir().join(format!(
        "coder-one-issue-eval-grade-{}-{}-{}",
        entry.id,
        std::process::id(),
        atif::now_ms()
    ));
    checkout(&args.source()?, &entry.base, commit, &scratch)?;
    Ok((
        grade(set, entry, &scratch, &args.target_dir()?),
        Some(scratch),
    ))
}

fn print_grade(id: &str, graded: &Grade) {
    println!("{id}: {} · {}", graded.verdict, graded.detail);
    for check in &graded.checks {
        println!(
            "  {} [{}] {} · {}",
            if check.passed { "pass" } else { "FAIL" },
            check.group,
            check.id,
            crate::judge::clip(&check.detail.replace('\n', " "), 240)
        );
    }
}

/// Grades each entry's base and fix; a grader discriminates when the base
/// fails and the fix passes.
fn verify(set: &Set, args: &Args) -> Result<i32, String> {
    let mut rows = Vec::new();
    let mut all = true;
    for (entry, part, _) in &set.entries {
        if !args.positional.is_empty()
            && !args
                .positional
                .iter()
                .any(|id| set.find(id).is_ok_and(|(e, _, _)| e.id == entry.id))
        {
            continue;
        }
        if args.part.is_some_and(|want| want != *part) {
            continue;
        }
        let (base, scratch) = graded_at(set, args, &entry.id, None)?;
        if let Some(scratch) = scratch {
            let _ = std::fs::remove_dir_all(scratch);
        }
        let (fix, scratch) = graded_at(set, args, &entry.id, Some(&entry.fix.head))?;
        if let Some(scratch) = scratch {
            let _ = std::fs::remove_dir_all(scratch);
        }
        let discriminates = base.verdict == "failed" && fix.verdict == "passed";
        all &= discriminates;
        if !args.json {
            println!(
                "{:<38} base {} ({}/{}) · fix {} ({}/{}) · {}",
                entry.id,
                base.verdict,
                base.passed,
                base.total,
                fix.verdict,
                fix.passed,
                fix.total,
                if discriminates {
                    "discriminates"
                } else {
                    "DOES NOT DISCRIMINATE"
                }
            );
            for check in &fix.checks {
                let at_base = base.checks.iter().find(|c| c.id == check.id);
                println!(
                    "    {:<30} base {} · fix {}",
                    check.id,
                    at_base.map_or("?", |c| if c.passed { "pass" } else { "fail" }),
                    if check.passed { "pass" } else { "FAIL" }
                );
                if !check.passed {
                    println!(
                        "      {}",
                        crate::judge::clip(&check.detail.replace('\n', " "), 400)
                    );
                }
            }
        }
        rows.push(json!({
            "entry": entry.id,
            "part": part.word(),
            "base": base,
            "fix": fix,
            "discriminates": discriminates,
        }));
    }
    if args.json {
        print(&json!({ "set_digest": set.digest, "entries": rows }))?;
    }
    Ok(i32::from(!all))
}
