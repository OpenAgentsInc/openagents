//! Behavior scenarios derived from what a task states about its outputs.
//!
//! The generic scenarios see only that an output exists and parses. These
//! scenarios run the candidate the way the task says it will be run, with
//! inputs and relations taken from the instruction's own words, and
//! compare what it does with what the instruction says it must do. They
//! cover the families of the Terminal-Bench 4.0 tasks that another agent
//! solves reliably and Coder One hasn't:
//!
//! | Scenario | Applies when the instruction | Expected relation |
//! | --- | --- | --- |
//! | `behavior.filter-removes` | asks for a program that removes JavaScript from an HTML file named on its command line, in place | After it runs on documents carrying common script vectors, no script element, event handler, or script URL is left, and the surrounding content is. |
//! | `behavior.filter-preserves` | also asks it to preserve the rest of the HTML | Clean documents come out unchanged, up to the normalization an HTML parser may do. |
//! | `behavior.named-command` | names a command that writes named outputs | The command exits 0 and writes each output; when the instruction says so, a second run writes the same bytes, another seed writes different ones, the inputs it names stay unchanged, and it runs without the executable the verifier removes. |
//! | `behavior.reference-diff` | asks for a program that behaves exactly like a reference binary | On the reference's help, on its subcommands' help, and on a bad command, the program's exit code, output, and errors match the reference's. |
//! | `behavior.json-overlap` | asks for a JSON report and says a selected position overlaps a range | Each `…position…` integer in the report lies within the `…_start`/`…_end` range that shares its unit, such as `protein_position` within `protein_residue_start` to `protein_residue_end`. |
//!
//! Every document, vector, and probe is the host's own, written from the
//! public wording of the task family; none comes from a verifier. Protected
//! tests are never read.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use super::html;
use super::place::{Place, missing_environment};
use super::{Bounds, Context, Ineligible, Relation, Scenario, Verdict, generic};
use crate::minitask::process;
use crate::requirements::Requirement;

/// The scenario kinds this family builds.
pub const KINDS: &[&str] = &[
    "behavior.filter-removes",
    "behavior.filter-preserves",
    "behavior.named-command",
    "behavior.reference-diff",
    "behavior.json-overlap",
];

/// The most characters of output an observation keeps.
const OUTPUT_CHARS: usize = 1_200;

/// Where probes are written, in the task's filesystem.
const PROBES: &str = "/tmp/coder-one-behavior";

/// Script vectors every in-place HTML sanitizer must defuse: the common
/// cases of the public XSS filter-evasion literature.
const VECTORS: &[&str] = &[
    "<script>alert(1)</script>",
    "<SCRIPT SRC=http://x.invalid/xss.js></SCRIPT>",
    "<img src=x onerror=alert(1)>",
    "<IMG SRC=\"javascript:alert('XSS');\">",
    "<IMG SRC=JaVaScRiPt:alert(1)>",
    "<a href=\"jav&#x09;ascript:alert(1)\">x</a>",
    "<body onload=alert(1)>",
    "<svg/onload=alert(1)>",
    "<iframe src=\"javascript:alert(1)\"></iframe>",
    "<div style=\"background:url(javascript:alert(1))\">x</div>",
    "<input onfocus=alert(1) autofocus>",
    "<a href=\"data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==\">x</a>",
    "<details open ontoggle=alert(1)>",
    "<object data=\"javascript:alert(1)\"></object>",
    "<form action=\"javascript:alert(1)\"><button>x</button></form>",
    "<meta http-equiv=\"refresh\" content=\"0;url=javascript:alert(1)\">",
    "<a href=\"&#106;&#97;&#118;&#97;&#115;&#99;&#114;&#105;&#112;&#116;&#58;alert(1)\">x</a>",
    "<a href=\" javascript:alert(1)\">x</a>",
];

/// Clean documents a sanitizer must leave alone: ordinary markup with
/// comments, entities, forms, tables, and irregular but valid syntax.
const CLEAN: &[&str] = &[
    r#"<!DOCTYPE html>
<html>
<head>
<title>Quarterly report</title>
<meta charset="utf-8">
<style>
  table { border-collapse: collapse; }
</style>
</head>
<body>
<h1 id="top" class="title">Quarterly report</h1>
<p>Revenue grew <b>12%</b> &amp; costs fell. See <a href="https://example.com/q3" title="Q3">the details</a>.</p>
<table border="1">
  <tr><th>Region</th><th>Sales</th></tr>
  <tr><td>North</td><td>1,200</td></tr>
</table>
<ul>
  <li>First</li>
  <li>Second</li>
</ul>
<img src="chart.png" alt="Chart" width="200">
<pre>  keep   spacing  </pre>
</body>
</html>
"#,
    r#"<!DOCTYPE html>
<HTML lang='en'>
<head>
<!-- Site header: last edited 2024-03-01 -->
<!-- <div class="banner">Old banner, kept for reference</div> -->
<link rel=stylesheet href=style.css>
</head>
<BODY>
<nav><a href='/'>Home</a> | <a href="/docs/">Docs</a></nav>
<h2>How to embed a script tag</h2>
<p>Write <code>&lt;script src="app.js"&gt;&lt;/script&gt;</code> in the head. Use 3 &lt; 5 and 5 &gt; 3.</p>
<form method="post" action="/subscribe">
  <label for=email>Email</label> <input type=email id=email name=email required>
  <select name="plan"><option value="a" selected>Basic</option><option value="b">Pro</option></select>
  <textarea name="note" rows="3">Line one
Line two</textarea>
  <button type="submit">Subscribe</button>
</form>
<p>Line with a break<br>and another<br/>and entities: &copy; &eacute; &#169; &nbsp;</p>
<blockquote cite="https://example.com">Quoted text</blockquote>
</BODY>
</HTML>
"#,
];

#[allow(clippy::too_many_arguments)]
fn scenario(
    context: &Context<'_>,
    id: String,
    kind: &str,
    requirement: &Requirement,
    applies: Vec<String>,
    interface: String,
    seconds: u64,
    effects: Vec<String>,
    expected: Relation,
    params: Value,
) -> Scenario {
    Scenario {
        id,
        kind: kind.to_string(),
        requirements: vec![requirement.id.clone()],
        spans: context.spans_of(&[requirement]),
        applies,
        interface,
        bounds: Bounds {
            seconds,
            processes: 1,
        },
        effects,
        candidate: context.candidate.digest(),
        input: atif::digest(&params),
        seed: None,
        expected,
        params,
    }
}

/// The instruction's sentences: a stop ends one only before whitespace, so
/// a path's dot doesn't.
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        current.push(c);
        let ends = c == '\n'
            || (matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|n| n.is_whitespace()));
        if ends {
            let next = current.trim().to_string();
            if next.len() > 1 {
                out.push(next);
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

/// The backticked pieces of `text`.
fn code_spans(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn interpreter(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("py") => "python3",
        Some("sh") => "bash",
        Some("js" | "mjs") => "node",
        Some("rb") => "ruby",
        Some("pl") => "perl",
        _ => "",
    }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Build: every behavior scenario the instruction justifies, or why none
/// applies.
///
/// # Errors
///
/// Returns why no behavior scenario applies.
pub fn build(context: &Context<'_>) -> Result<Vec<Scenario>, Vec<Ineligible>> {
    // Off, the family adds nothing to a report, so a policy without it
    // checks exactly as it did before the family existed.
    let Some(workspace) = context.workspace.filter(|w| w.options.behavior) else {
        return Ok(Vec::new());
    };
    let mut scenarios = Vec::new();
    let mut ineligible = Vec::new();
    for built in [
        filter(context, workspace),
        named_commands(context, workspace),
        reference_diff(context, workspace),
        json_overlap(context),
    ] {
        match built {
            Ok(mut found) => scenarios.append(&mut found),
            Err(why) => ineligible.push(why),
        }
    }
    if scenarios.is_empty() {
        Err(ineligible)
    } else {
        Ok(scenarios)
    }
}

/// The program path a requirement asks the executor to create: the first
/// absolute path with a script extension in a requirement that asks for
/// it to be created or implemented.
fn program_asked_for<'a>(context: &'a Context<'_>) -> Option<(String, &'a Requirement)> {
    for requirement in &context.map.requirements {
        let lower = requirement.text.to_lowercase();
        if !["create", "implement", "write", "build"]
            .iter()
            .any(|w| lower.contains(w))
        {
            continue;
        }
        for path in &requirement.extracted.paths {
            if path.starts_with('/') && !path.starts_with("/tests") && !interpreter(path).is_empty()
            {
                return Some((path.clone(), requirement));
            }
        }
    }
    None
}

fn filter(
    context: &Context<'_>,
    workspace: &generic::Workspace,
) -> Result<Vec<Scenario>, Ineligible> {
    let refuse = |why: &str| Ineligible {
        kind: "behavior.filter".to_string(),
        why: why.to_string(),
    };
    let lower = context.task.instruction.to_lowercase();
    let in_place = lower.contains("in-place") || lower.contains("in place");
    let argument = lower.contains("argv[1]")
        || lower.contains("command-line argument")
        || lower.contains("command line argument");
    let html = lower.contains("html");
    let script = lower.contains("javascript") || lower.contains("xss");
    if !(in_place && argument && html && script) {
        return Err(refuse(
            "the instruction asks for no in-place HTML filter that removes JavaScript",
        ));
    }
    let Some((program, asked)) = program_asked_for(context) else {
        return Err(refuse("no requirement names the filter's path"));
    };
    let removes = context
        .requirements_saying(&[&["remove", "javascript"], &["xss"], &["remove"]])
        .first()
        .copied()
        .unwrap_or(asked);
    let preserves = context
        .requirements_saying(&[&["preserve"], &["formatting"], &["identical"]])
        .first()
        .copied();
    let run = format!("{} {program} FILE", interpreter(&program));
    let effects = vec![format!(
        "writes probe documents under {PROBES} and runs the filter on each, as the task says it is run"
    )];
    let mut out = vec![scenario(
        context,
        "behavior.filter-removes".to_string(),
        "behavior.filter-removes",
        removes,
        vec![format!(
            "{} asks {program} to remove JavaScript from the HTML file it is given, in place",
            removes.id
        )],
        format!("`{run}`"),
        60,
        effects.clone(),
        Relation {
            statement: "After the filter runs on a document carrying a common script vector between two paragraphs, no script element, event handler, or script URL is left, and both paragraphs are.".to_string(),
            derivation: format!(
                "{} asks for all JavaScript to be removed and the rest kept; the vectors are the common cases of the public XSS filter-evasion literature.",
                removes.id
            ),
        },
        json!({ "program": program, "interpreter": interpreter(&program), "vectors": VECTORS.len(), "dir": workspace.dir }),
    )];
    if let Some(preserves) = preserves {
        out.push(scenario(
            context,
            "behavior.filter-preserves".to_string(),
            "behavior.filter-preserves",
            preserves,
            vec![format!(
                "{} asks the filter to preserve the HTML it doesn't remove",
                preserves.id
            )],
            format!("`{run}`"),
            30,
            effects,
            Relation {
                statement: "Clean documents, with comments, entities, forms, and tables, come out unchanged, up to the normalization an HTML parser may do.".to_string(),
                derivation: format!(
                    "{} asks for the formatting to be preserved, allowing only parser normalization.",
                    preserves.id
                ),
            },
            json!({ "program": program, "interpreter": interpreter(&program), "documents": CLEAN.len(), "dir": workspace.dir }),
        ));
    }
    Ok(out)
}

/// A command named in the instruction that runs the deliverable rather
/// than a test: its text, and the requirement that names it.
fn deliverable_commands<'a>(context: &'a Context<'_>) -> Vec<(String, &'a Requirement)> {
    let mut found: Vec<(String, &Requirement)> = Vec::new();
    for requirement in &context.map.requirements {
        let lower = requirement.text.to_lowercase();
        let mut named: Vec<String> = requirement.extracted.commands.clone();
        if lower.contains("command") {
            named.extend(
                requirement
                    .extracted
                    .paths
                    .iter()
                    .filter(|p| p.starts_with('/') && p.ends_with(".sh"))
                    .cloned(),
            );
        }
        for command in named {
            let text = command.trim().to_string();
            let placeholder = text.contains('<')
                || text.contains("...")
                || text.contains("YYYY")
                || text.contains('|')
                || text.contains('>')
                || text.contains(';')
                || text.contains("$(")
                || text.contains('`');
            let lowered = text.to_lowercase();
            let changes = [
                "install", "rm ", "curl", "wget", "sudo", "apt", "git ", "kill", "docker",
            ]
            .iter()
            .any(|w| lowered.contains(w));
            let test = generic::refusal(&text).is_none();
            let runs_a_program = text.starts_with('/')
                || [
                    "python", "python3", "bash", "sh", "node", "bun", "make", "./",
                ]
                .iter()
                .any(|p| text.split_whitespace().next() == Some(*p) || text.starts_with(p));
            if !placeholder
                && !changes
                && !test
                && runs_a_program
                && !text.contains("/tests")
                && !found.iter().any(|(c, _)| *c == text)
            {
                found.push((text, requirement));
            }
        }
    }
    found
}

/// Absolute file paths `text` names that aren't in `command`.
fn named_outputs(text: &str, command: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for span in code_spans(text) {
        let span = span.trim_end_matches(['.', ',']);
        let is_file = span.starts_with('/')
            && !span.contains(' ')
            && span
                .rsplit('/')
                .next()
                .is_some_and(|last| last.contains('.'))
            && !span.starts_with("/tests");
        if is_file && !command.contains(span) && !out.iter().any(|o| o == span) {
            out.push(span.to_string());
        }
    }
    out
}

/// The values of output options in a command: `--out DIR`, `--output DIR`,
/// `-o DIR`.
fn option_outputs(command: &str) -> Vec<String> {
    let words: Vec<&str> = command.split_whitespace().collect();
    let mut out = Vec::new();
    for pair in words.windows(2) {
        if matches!(
            pair[0],
            "--out" | "--output" | "-o" | "--out-dir" | "--output-dir"
        ) && pair[1].starts_with('/')
        {
            out.push(pair[1].to_string());
        }
        for flag in ["--out=", "--output="] {
            if let Some(value) = pair[0].strip_prefix(flag)
                && value.starts_with('/')
            {
                out.push(value.to_string());
            }
        }
    }
    out
}

/// Executables the instruction says the verifier removes before it runs
/// the candidate: backticked names in a sentence that says not to call
/// them, or that the verifier removes them.
fn removed_executables(instruction: &str) -> Vec<String> {
    let mut out = Vec::new();
    for sentence in sentences(instruction) {
        let lower = sentence.to_lowercase();
        let removal = (lower.contains("remove") && lower.contains("verifier"))
            || lower.contains("do not call")
            || lower.contains("don't call");
        let executable =
            lower.contains("binary") || lower.contains("command") || lower.contains("executable");
        if removal && executable {
            for span in code_spans(&sentence) {
                if !span.contains(' ') && !span.contains('/') && !out.contains(&span) {
                    out.push(span);
                }
            }
        }
    }
    out
}

fn named_commands(
    context: &Context<'_>,
    workspace: &generic::Workspace,
) -> Result<Vec<Scenario>, Ineligible> {
    let lower = context.task.instruction.to_lowercase();
    let deterministic = lower.contains("deterministic");
    let unchanged_inputs = [
        "do not modify",
        "don't modify",
        "without modifying",
        "must not modify",
        "must not mutate",
        "not modify raw input",
    ]
    .iter()
    .any(|w| lower.contains(w));
    let seed_varies = lower.contains("change when the seed changes")
        || (lower.contains("seed") && lower.contains("must change"));
    let hidden = removed_executables(&context.task.instruction);
    let mut out = Vec::new();
    for (i, (command, requirement)) in deliverable_commands(context).into_iter().enumerate() {
        let mut outputs = named_outputs(&requirement.text, &command);
        outputs.extend(option_outputs(&command));
        outputs.dedup();
        if outputs.is_empty() {
            continue;
        }
        let seed = command
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .find(|w| w[0] == "--seed")
            .and_then(|w| w[1].parse::<i64>().ok());
        let inputs: Vec<String> = command
            .split_whitespace()
            .skip(1)
            .filter(|w| w.starts_with('/') && !outputs.iter().any(|o| o == *w))
            .filter(|w| !w.ends_with(".py") && !w.ends_with(".sh"))
            .map(str::to_string)
            .collect();
        let mut relations = vec![
            "exits 0".to_string(),
            format!("writes {}", outputs.join(", ")),
        ];
        if deterministic {
            relations.push("writes the same bytes when run again".to_string());
        }
        if seed_varies && seed.is_some() {
            relations.push("writes different bytes under another seed".to_string());
        }
        if unchanged_inputs && !inputs.is_empty() {
            relations.push(format!("leaves {} unchanged", inputs.join(", ")));
        }
        if !hidden.is_empty() {
            relations.push(format!("runs without {} on its PATH", hidden.join(", ")));
        }
        let runs = 1 + usize::from(deterministic) + 2 * usize::from(seed_varies && seed.is_some());
        let seconds = workspace.command_sec.max(1) * runs as u64;
        out.push(scenario(
            context,
            format!("behavior.named-command:{}", i + 1),
            "behavior.named-command",
            requirement,
            vec![format!(
                "{} names the command `{command}` and the outputs it writes",
                requirement.id
            )],
            format!("the command `{command}`"),
            seconds,
            vec![format!(
                "runs `{command}` in the task's working directory {runs} time(s), which rewrites its outputs, without the episode's credentials"
            )],
            Relation {
                statement: format!("`{command}` {}.", relations.join("; ")),
                derivation: format!(
                    "{} names the command and its outputs; the instruction's own words add each further relation.",
                    requirement.id
                ),
            },
            json!({
                "command": command,
                "outputs": outputs,
                "inputs": if unchanged_inputs { inputs } else { Vec::new() },
                "deterministic": deterministic,
                "seed": if seed_varies { seed } else { None },
                "hidden": hidden,
                "dir": workspace.dir,
            }),
        ));
    }
    if out.is_empty() {
        Err(Ineligible {
            kind: "behavior.named-command".to_string(),
            why: "the instruction names no command that writes named outputs".to_string(),
        })
    } else {
        Ok(out)
    }
}

fn reference_diff(
    context: &Context<'_>,
    workspace: &generic::Workspace,
) -> Result<Vec<Scenario>, Ineligible> {
    let refuse = |why: &str| Ineligible {
        kind: "behavior.reference-diff".to_string(),
        why: why.to_string(),
    };
    let mut reference = None;
    let mut clone = None;
    for sentence in sentences(&context.task.instruction) {
        let lower = sentence.to_lowercase();
        let spans: Vec<String> = code_spans(&sentence)
            .into_iter()
            .filter(|s| s.starts_with('/') && !s.contains(' '))
            .collect();
        if lower.contains("reference binary") || lower.contains("reference implementation") {
            if reference.is_none() {
                reference = spans
                    .iter()
                    .find(|s| s.contains("artifact") || lower.contains("provided reference"))
                    .cloned()
                    .or_else(|| spans.last().cloned());
            }
            if clone.is_none() && spans.len() >= 2 {
                clone = spans.first().cloned();
            }
        } else if clone.is_none()
            && (lower.starts_with("implement") || lower.starts_with("create"))
            && let Some(first) = spans.first()
        {
            clone = Some(first.clone());
        }
    }
    let lower = context.task.instruction.to_lowercase();
    let exact = lower.contains("exactly the same")
        || lower.contains("identical to the reference")
        || lower.contains("output identical");
    let (Some(reference), Some(clone)) = (reference, clone) else {
        return Err(refuse("the instruction names no reference binary to match"));
    };
    if !exact || reference == clone {
        return Err(refuse(
            "the instruction doesn't ask to match the reference exactly",
        ));
    }
    let requirement = context
        .requirements_saying(&[&["exactly", "same"], &["identical"], &["reference"]])
        .first()
        .copied()
        .or_else(|| context.map.requirements.first());
    let Some(requirement) = requirement else {
        return Err(refuse("no requirement to observe"));
    };
    Ok(vec![scenario(
        context,
        "behavior.reference-diff".to_string(),
        "behavior.reference-diff",
        requirement,
        vec![format!(
            "{} asks {clone} to behave exactly like {reference}",
            requirement.id
        )],
        format!("`{clone}` and `{reference}` on the same arguments"),
        60,
        vec![format!(
            "runs both programs with help and bad-command arguments in a scratch directory under {PROBES}"
        )],
        Relation {
            statement: format!(
                "On `--help`, on each subcommand's `--help`, and on a command that doesn't exist, `{clone}` exits with the same code and writes the same output and errors as `{reference}`, with each program's own path read as the same name."
            ),
            derivation: format!(
                "{} asks for output identical to the reference for valid and invalid inputs, exit status included.",
                requirement.id
            ),
        },
        json!({ "reference": reference, "clone": clone, "dir": workspace.dir }),
    )])
}

/// Runs one behavior scenario.
pub async fn run(context: &Context<'_>, scenario: &Scenario, scratch: &Path) -> Verdict {
    let Some(workspace) = context.workspace else {
        return Verdict::unavailable(&scenario.id, "the check has no live workspace");
    };
    let place = workspace.place();
    match scenario.kind.as_str() {
        "behavior.filter-removes" | "behavior.filter-preserves" => {
            run_filter(workspace, &place, scenario, scratch).await
        }
        "behavior.named-command" => run_named(workspace, &place, scenario, scratch).await,
        "behavior.reference-diff" => run_reference(workspace, &place, scenario, scratch).await,
        "behavior.json-overlap" => run_overlap(workspace, &place, scenario),
        other => Verdict::unavailable(&scenario.id, &format!("no runner for {other}")),
    }
}

fn probe_dir(place: &Place, scenario: &Scenario) -> (String, PathBuf) {
    let inside = format!(
        "{PROBES}-{}/{}",
        std::process::id(),
        scenario.id.replace([':', '/'], "-")
    );
    let host = place.host(Path::new(&inside));
    let _ = std::fs::remove_dir_all(&host);
    (inside, host)
}

async fn run_filter(
    workspace: &generic::Workspace,
    place: &Place,
    scenario: &Scenario,
    scratch: &Path,
) -> Verdict {
    let mut verdict = Verdict::new(&scenario.id, "passed");
    let program = scenario.params["program"].as_str().unwrap_or_default();
    if !place.host(Path::new(program)).is_file() {
        verdict.verdict = "failed".to_string();
        verdict
            .observations
            .push(json!({ "program": program, "exists": false }));
        verdict.hypotheses =
            vec!["the filter was never written at the path the task names".to_string()];
        return verdict;
    }
    let removes = scenario.kind == "behavior.filter-removes";
    let documents: Vec<String> = if removes {
        VECTORS
            .iter()
            .map(|v| format!("<html><body><p>Hello</p>{v}<p>World</p></body></html>\n"))
            .collect()
    } else {
        CLEAN.iter().map(|d| (*d).to_string()).collect()
    };
    let (inside, host) = probe_dir(place, scenario);
    if let Err(error) = std::fs::create_dir_all(&host) {
        return Verdict::unavailable(&scenario.id, &format!("cannot write probes: {error}"));
    }
    for (i, document) in documents.iter().enumerate() {
        if std::fs::write(host.join(format!("{i:02}.html")), document).is_err() {
            return Verdict::unavailable(&scenario.id, "cannot write probes");
        }
    }
    let interpreter = scenario.params["interpreter"].as_str().unwrap_or_default();
    let run = if interpreter.is_empty() {
        quote(program)
    } else {
        format!("{interpreter} {}", quote(program))
    };
    let script = format!(
        "cd {dir} && for f in {inside}/*.html; do {run} \"$f\" >/dev/null 2>\"$f.err\" || echo \"$(basename \"$f\") $?\" >> {inside}/exits; done",
        dir = quote(&workspace.dir),
        inside = quote(&inside),
    );
    let command = match place.shell(&script, "/", &[], scratch) {
        Ok(command) => command,
        Err(why) => return Verdict::unavailable(&scenario.id, &why),
    };
    let ran = process::run(command, Duration::from_secs(scenario.bounds.seconds)).await;
    let exits = std::fs::read_to_string(host.join("exits")).unwrap_or_default();
    if ran.killed {
        verdict.verdict = "inconclusive".to_string();
        verdict.coverage.push(format!(
            "the probes ran past the {}-second bound",
            scenario.bounds.seconds
        ));
        let _ = std::fs::remove_dir_all(&host);
        return verdict;
    }
    let mut problems: Vec<Value> = Vec::new();
    for (i, document) in documents.iter().enumerate() {
        let name = format!("{i:02}.html");
        let after = std::fs::read_to_string(host.join(&name)).unwrap_or_default();
        let errors = std::fs::read_to_string(host.join(format!("{name}.err"))).unwrap_or_default();
        if let Some(line) = exits.lines().find(|l| l.starts_with(&name)) {
            if place.is_replay()
                && let Some(missing) = missing_environment(&errors)
            {
                let _ = std::fs::remove_dir_all(&host);
                return Verdict::unavailable(
                    &scenario.id,
                    &format!("the replay lacks what the task's image provides: {missing}"),
                );
            }
            problems.push(json!({
                "document": name,
                "exit": line.split_whitespace().nth(1),
                "errors": crate::judge::clip_tail(errors.trim(), 400),
            }));
            continue;
        }
        if removes {
            let left = html::scripts(&after);
            let kept = after.contains("Hello") && after.contains("World");
            if !left.is_empty() || !kept {
                problems.push(json!({
                    "vector": VECTORS[i],
                    "left": left,
                    "content_kept": kept,
                    "output": crate::judge::clip(after.trim(), 300),
                }));
            }
        } else if after != *document {
            let same = html::normalized(&after) == html::normalized(document);
            if same {
                verdict
                    .coverage
                    .push(format!("{name} changed only as a parser may normalize it"));
            } else {
                let changed = document
                    .lines()
                    .zip(after.lines().chain(std::iter::repeat("")))
                    .find(|(a, b)| a != b)
                    .map(|(a, b)| json!({ "expected": a, "got": b }));
                problems.push(json!({ "document": name, "first_change": changed }));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&host);
    verdict.observations.push(json!({
        "documents": documents.len(),
        "problems": problems.len(),
    }));
    if problems.is_empty() {
        verdict.coverage.push(if removes {
            "defuses the listed vectors, not every vector a browser accepts".to_string()
        } else {
            "preserves these documents, not every document".to_string()
        });
    } else {
        verdict.verdict = "failed".to_string();
        verdict.observations.extend(problems.into_iter().take(6));
        verdict.hypotheses = if removes {
            vec![
                "the filter misses a common script form: a case, an encoding, or an attribute it doesn't look at".to_string(),
                "the filter removes the surrounding content with the script".to_string(),
            ]
        } else {
            vec!["the filter rewrites or drops markup that carries no script, such as a comment or an attribute".to_string()]
        };
    }
    verdict
}

/// Digests of every regular file under `path` (a file or a directory), by
/// path relative to it.
fn digests(path: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(next) = stack.pop() {
        if next.is_dir() {
            for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
                stack.push(entry.path());
            }
        } else if let Ok(bytes) = std::fs::read(&next) {
            let relative = next
                .strip_prefix(path)
                .unwrap_or(&next)
                .to_string_lossy()
                .into_owned();
            out.insert(relative, hex(&bytes));
        }
        if out.len() > 2_000 {
            break;
        }
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn outputs_digest(place: &Place, outputs: &[String]) -> BTreeMap<String, BTreeMap<String, String>> {
    outputs
        .iter()
        .map(|o| (o.clone(), digests(&place.host(Path::new(o)))))
        .collect()
}

async fn run_named(
    workspace: &generic::Workspace,
    place: &Place,
    scenario: &Scenario,
    scratch: &Path,
) -> Verdict {
    let mut verdict = Verdict::new(&scenario.id, "passed");
    let command = scenario.params["command"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let strings = |key: &str| -> Vec<String> {
        scenario.params[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    };
    let outputs = strings("outputs");
    let inputs = strings("inputs");
    let hidden = strings("hidden");
    let per_run = workspace.command_sec.max(1);
    if place.is_replay() {
        // A replay has only the files the task image copies in; an input
        // its build steps made isn't there, and that says nothing about
        // the candidate.
        let absent: Vec<&str> = command
            .split_whitespace()
            .skip(1)
            .filter(|w| w.starts_with('/') && !outputs.iter().any(|o| o == *w))
            .filter(|w| !place.host(Path::new(w)).exists())
            .filter(|w| !w.ends_with(".py") && !w.ends_with(".sh"))
            .collect();
        if !absent.is_empty() {
            return Verdict::unavailable(
                &scenario.id,
                &format!(
                    "the replay lacks {}, which the task image's build makes",
                    absent.join(", ")
                ),
            );
        }
    }
    let before_inputs = outputs_digest(place, &inputs);
    let mut problems: Vec<String> = Vec::new();
    let once = |text: String| {
        let hidden = hidden.clone();
        async move {
            match place.shell(&text, &workspace.dir, &hidden, scratch) {
                Ok(c) => Ok(process::run(c, Duration::from_secs(per_run)).await),
                Err(why) => Err(why),
            }
        }
    };
    let first = match once(command.clone()).await {
        Ok(ran) => ran,
        Err(why) => return Verdict::unavailable(&scenario.id, &why),
    };
    let output = crate::support::scrub(&format!("{}{}", first.stdout, first.stderr));
    verdict.observations.push(json!({
        "command": command,
        "exit_code": first.code,
        "killed": first.killed,
        "milliseconds": first.milliseconds,
        "output_tail": crate::judge::clip_tail(output.trim(), OUTPUT_CHARS),
    }));
    if first.killed {
        verdict.verdict = "inconclusive".to_string();
        verdict
            .coverage
            .push(format!("the command ran past its {per_run}-second bound"));
        return verdict;
    }
    if first.code != Some(0) {
        if place.is_replay()
            && let Some(missing) = missing_environment(&output)
        {
            return Verdict::unavailable(
                &scenario.id,
                &format!("the replay lacks what the task's image provides: {missing}"),
            );
        }
        problems.push(format!("exited {:?}", first.code));
    }
    let written = outputs_digest(place, &outputs);
    for (path, files) in &written {
        let empty = files.is_empty()
            || (files.len() == 1
                && std::fs::metadata(place.host(Path::new(path))).is_ok_and(|m| m.len() == 0));
        if empty {
            problems.push(format!("wrote nothing at {path}"));
        }
    }
    if problems.is_empty() && scenario.params["deterministic"] == true {
        match once(command.clone()).await {
            Ok(again) if again.code == Some(0) => {
                let second = outputs_digest(place, &outputs);
                let differing: Vec<String> = second
                    .iter()
                    .flat_map(|(root, files)| {
                        let firsts = written.get(root).cloned().unwrap_or_default();
                        files
                            .iter()
                            .filter(move |(f, d)| firsts.get(*f) != Some(*d))
                            .map(move |(f, _)| format!("{root}/{f}"))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                if !differing.is_empty() {
                    problems.push(format!(
                        "a second run wrote different bytes to {}",
                        differing.into_iter().take(5).collect::<Vec<_>>().join(", ")
                    ));
                }
            }
            Ok(again) => problems.push(format!("a second run exited {:?}", again.code)),
            Err(why) => verdict.coverage.push(format!("no second run: {why}")),
        }
    }
    if problems.is_empty()
        && let Some(seed) = scenario.params["seed"].as_i64()
    {
        let other = command.replace(&format!("--seed {seed}"), &format!("--seed {}", seed + 1));
        if let Ok(ran) = once(other).await
            && ran.code == Some(0)
        {
            let varied = outputs_digest(place, &outputs);
            if varied == written {
                problems.push(format!(
                    "seed {} wrote the same bytes as seed {seed}",
                    seed + 1
                ));
            }
        }
        // Put the named seed's outputs back.
        let _ = once(command.clone()).await;
    }
    if !inputs.is_empty() {
        let after = outputs_digest(place, &inputs);
        for (path, files) in &after {
            if before_inputs.get(path) != Some(files) {
                problems.push(format!("changed the input {path}"));
            }
        }
    }
    if !hidden.is_empty() {
        verdict.coverage.push(format!(
            "ran without {} on PATH, not without it on disk",
            hidden.join(", ")
        ));
    }
    verdict
        .coverage
        .push("runs the task's own inputs, not the verifier's".to_string());
    if !problems.is_empty() {
        verdict.verdict = "failed".to_string();
        verdict.observations.push(json!({ "problems": problems }));
        verdict.hypotheses = vec![
            "the command depends on something the task says won't be there".to_string(),
            "the outputs depend on time, order, or randomness the task says must not matter"
                .to_string(),
            "the command writes its outputs somewhere other than where the task names them"
                .to_string(),
        ];
    }
    verdict
}

/// Subcommands a help text lists: the words in a `{a,b,c}` group, or the
/// first word of each indented line under a `commands:` heading.
fn subcommands(help: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(start) = help.find('{')
        && let Some(end) = help[start..].find('}')
    {
        for word in help[start + 1..start + end].split(',') {
            let word = word.trim();
            if !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                out.push(word.to_string());
            }
        }
    }
    let mut listing = false;
    for line in help.lines() {
        let lower = line.trim().to_lowercase();
        if lower.ends_with("commands:") || lower == "commands" || lower.ends_with("subcommands:") {
            listing = true;
            continue;
        }
        if listing {
            if line.trim().is_empty() {
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('\t') {
                listing = false;
                continue;
            }
            if let Some(word) = line.split_whitespace().next()
                && word
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                && !word.starts_with('-')
                && !out.iter().any(|o| o == word)
            {
                out.push(word.to_string());
            }
        }
    }
    out.truncate(12);
    out
}

async fn run_reference(
    workspace: &generic::Workspace,
    place: &Place,
    scenario: &Scenario,
    scratch: &Path,
) -> Verdict {
    let mut verdict = Verdict::new(&scenario.id, "passed");
    let reference = scenario.params["reference"].as_str().unwrap_or_default();
    let clone = scenario.params["clone"].as_str().unwrap_or_default();
    if !place.host(Path::new(reference)).is_file() {
        return Verdict::unavailable(&scenario.id, "the reference binary isn't there");
    }
    if !place.host(Path::new(clone)).is_file() {
        verdict.verdict = "failed".to_string();
        verdict
            .observations
            .push(json!({ "program": clone, "exists": false }));
        verdict.hypotheses =
            vec!["the program was never written at the path the task names".to_string()];
        return verdict;
    }
    let (inside, host) = probe_dir(place, scenario);
    let _ = std::fs::create_dir_all(&host);
    let call = |program: &str, args: &[String]| {
        let line = std::iter::once(quote(program))
            .chain(args.iter().map(|a| quote(a)))
            .collect::<Vec<_>>()
            .join(" ");
        let script = format!("cd {} && {line}", quote(&inside));
        let command = place.shell(&script, &workspace.dir, &[], scratch);
        async move {
            match command {
                Ok(c) => Some(process::run(c, Duration::from_secs(10)).await),
                Err(_) => None,
            }
        }
    };
    let Some(help) = call(reference, &["--help".to_string()]).await else {
        return Verdict::unavailable(&scenario.id, "no sandbox to run the programs in");
    };
    let mut probes: Vec<Vec<String>> = vec![
        vec!["--help".to_string()],
        vec![],
        vec!["no-such-command-coder-one".to_string()],
    ];
    for sub in subcommands(&format!("{}\n{}", help.stdout, help.stderr)) {
        probes.push(vec![sub, "--help".to_string()]);
    }
    let names = |text: &str| {
        let base = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
        text.replace(reference, "PROG")
            .replace(clone, "PROG")
            .replace(&base(reference), "PROG")
            .replace(&base(clone), "PROG")
    };
    let mut differences = Vec::new();
    for args in &probes {
        let (Some(want), Some(got)) = (call(reference, args).await, call(clone, args).await) else {
            continue;
        };
        let same = want.code == got.code
            && names(&want.stdout) == names(&got.stdout)
            && names(&want.stderr) == names(&got.stderr);
        if !same {
            if place.is_replay()
                && let Some(missing) = missing_environment(&got.stderr)
            {
                let _ = std::fs::remove_dir_all(&host);
                return Verdict::unavailable(
                    &scenario.id,
                    &format!("the replay lacks what the task's image provides: {missing}"),
                );
            }
            differences.push(json!({
                "args": args,
                "reference": { "exit": want.code, "stdout": crate::judge::clip(&names(&want.stdout), 300), "stderr": crate::judge::clip(&names(&want.stderr), 300) },
                "program": { "exit": got.code, "stdout": crate::judge::clip(&names(&got.stdout), 300), "stderr": crate::judge::clip(&names(&got.stderr), 300) },
            }));
        }
    }
    let _ = std::fs::remove_dir_all(&host);
    verdict
        .observations
        .push(json!({ "probes": probes.len(), "differences": differences.len() }));
    verdict
        .coverage
        .push("compares help and error paths, not data commands".to_string());
    if !differences.is_empty() {
        verdict.verdict = "failed".to_string();
        verdict.observations.extend(differences.into_iter().take(5));
        verdict.hypotheses = vec![
            "the program's usage text or error messages differ from the reference's".to_string(),
            "the program exits with another status than the reference on these arguments"
                .to_string(),
        ];
    }
    verdict
}

fn json_overlap(context: &Context<'_>) -> Result<Vec<Scenario>, Ineligible> {
    let refuse = |why: &str| Ineligible {
        kind: "behavior.json-overlap".to_string(),
        why: why.to_string(),
    };
    let said: Vec<String> = sentences(&context.task.instruction)
        .into_iter()
        .filter(|s| {
            let lower = s.to_lowercase();
            lower.contains("overlap") && lower.contains("position")
        })
        .collect();
    if said.is_empty() {
        return Err(refuse("the instruction relates no position to a range"));
    }
    let mut out = Vec::new();
    for requirement in &context.map.requirements {
        if !generic::asks_executor(&requirement.text) {
            continue;
        }
        for path in &requirement.extracted.paths {
            if !path.starts_with('/') || !path.ends_with(".json") || path.starts_with("/tests") {
                continue;
            }
            if out
                .iter()
                .any(|s: &Scenario| s.params["path"] == path.as_str())
            {
                continue;
            }
            out.push(scenario(
                context,
                format!("behavior.json-overlap:{path}"),
                "behavior.json-overlap",
                requirement,
                vec![
                    format!("{} asks for the JSON report {path}", requirement.id),
                    format!("the instruction says: {}", crate::judge::clip(&said[0], 200)),
                ],
                format!("the file {path}"),
                1,
                vec!["reads the file".to_string()],
                Relation {
                    statement: format!(
                        "In {path}, each integer field named for a position lies within the start-to-end range whose field names share its unit."
                    ),
                    derivation: "The instruction selects a position because it overlaps a range; the report states both.".to_string(),
                },
                json!({ "path": path }),
            ));
        }
    }
    if out.is_empty() {
        Err(refuse("no requirement asks for a JSON report"))
    } else {
        Ok(out)
    }
}

/// Integer fields of a JSON value by their key path, objects only, three
/// levels down; integers written as strings count.
fn integers(value: &Value, trail: &str, depth: usize, out: &mut Vec<(String, i64)>) {
    let Value::Object(map) = value else {
        return;
    };
    for (key, value) in map {
        let here = if trail.is_empty() {
            key.clone()
        } else {
            format!("{trail}.{key}")
        };
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    out.push((here, i));
                }
            }
            Value::String(s) => {
                if let Ok(i) = s.trim().parse::<i64>() {
                    out.push((here, i));
                }
            }
            Value::Object(_) if depth < 3 => integers(value, &here, depth + 1, out),
            _ => {}
        }
    }
}

/// The words of a field name that name its unit, without the words that
/// name its role.
fn unit_words(key: &str) -> Vec<String> {
    let last = key.rsplit('.').next().unwrap_or(key).to_lowercase();
    last.split('_')
        .filter(|w| {
            !w.is_empty()
                && !matches!(
                    *w,
                    "start"
                        | "end"
                        | "position"
                        | "pos"
                        | "begin"
                        | "stop"
                        | "residue"
                        | "first"
                        | "last"
                )
        })
        .map(str::to_string)
        .collect()
}

fn run_overlap(workspace: &generic::Workspace, place: &Place, scenario: &Scenario) -> Verdict {
    let path = scenario.params["path"].as_str().unwrap_or_default();
    let Ok(text) = std::fs::read_to_string(place.host(Path::new(path))) else {
        if workspace.known(path) {
            return Verdict::unavailable(&scenario.id, "the report is missing");
        }
        return Verdict::unavailable(&scenario.id, "the trial didn't retain the report");
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Verdict::unavailable(&scenario.id, "the report isn't JSON");
    };
    let mut fields = Vec::new();
    integers(&value, "", 0, &mut fields);
    let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| *v);
    let mut ranges: Vec<(String, i64, i64, Vec<String>)> = Vec::new();
    for (key, start) in &fields {
        let (trail, last) = key.rsplit_once('.').unwrap_or(("", key));
        if !last.split('_').any(|w| w == "start") {
            continue;
        }
        let partner: Vec<&str> = last
            .split('_')
            .map(|w| if w == "start" { "end" } else { w })
            .collect();
        let partner = partner.join("_");
        let partner = if trail.is_empty() {
            partner
        } else {
            format!("{trail}.{partner}")
        };
        if let Some(end) = get(&partner)
            && *start <= end
        {
            ranges.push((key.clone(), *start, end, unit_words(key)));
        }
    }
    let mut verdict = Verdict::new(&scenario.id, "passed");
    let mut compared = 0;
    for (key, position) in &fields {
        let last = key.rsplit('.').next().unwrap_or(key).to_lowercase();
        if !last.contains("position") {
            continue;
        }
        let units = unit_words(key);
        let matching: Vec<&(String, i64, i64, Vec<String>)> = ranges
            .iter()
            .filter(|(_, _, _, words)| !units.is_empty() && words.iter().any(|w| units.contains(w)))
            .collect();
        let [(stem, start, end, _)] = matching.as_slice() else {
            continue;
        };
        compared += 1;
        let inside = (*start..=*end).contains(position);
        verdict.observations.push(json!({
            "position": key, "value": position, "range": stem, "start": start, "end": end, "inside": inside,
        }));
        if !inside {
            verdict.verdict = "failed".to_string();
        }
    }
    if compared == 0 {
        return Verdict::unavailable(
            &scenario.id,
            "the report has no position field whose unit matches one range",
        );
    }
    verdict.coverage.push(
        "checks that the report agrees with itself, not that its values are right".to_string(),
    );
    if verdict.verdict == "failed" {
        verdict.hypotheses = vec![
            "the selection used a different position than the one reported".to_string(),
            "the position and the range come from different coordinate systems or transcripts"
                .to_string(),
        ];
    }
    verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_is_compared_with_the_range_of_its_unit() {
        let report = json!({
            "domain": { "protein_residue_start": 2316, "protein_residue_end": 2416 },
            "selected": { "protein_position": 2479, "genomic_coordinate": 77508394 },
            "fragment": { "fragment_start_chrx": 77508374, "fragment_end_chrx": 77508413 },
            "target": { "cut_position_chrx": "77508394" },
        });
        let mut fields = Vec::new();
        integers(&report, "", 0, &mut fields);
        assert!(fields.contains(&("target.cut_position_chrx".to_string(), 77_508_394)));
        assert_eq!(unit_words("selected.protein_position"), ["protein"]);
        assert_eq!(
            unit_words("fragment.fragment_start_chrx"),
            ["fragment", "chrx"]
        );
    }

    #[test]
    fn a_named_removal_hides_the_executable() {
        let text = "Probe it with `legacy-score`. In your final implementation, do not call or copy `legacy-score` at runtime, because the verifier will remove that diagnostic binary before running your repaired code.";
        assert_eq!(removed_executables(text), ["legacy-score"]);
        assert!(removed_executables("Run `make test` to check.").is_empty());
    }

    #[test]
    fn outputs_come_from_the_requirement_and_the_command() {
        assert_eq!(
            named_outputs(
                "The command `/app/rebuild.sh` should write `/app/output/a.csv` and `/app/output/b.json`.",
                "/app/rebuild.sh"
            ),
            ["/app/output/a.csv", "/app/output/b.json"]
        );
        assert_eq!(
            option_outputs("python3 /app/anon.py /app/input --output /app/output --seed 42"),
            ["/app/output"]
        );
    }

    #[test]
    fn subcommands_come_from_the_help_text() {
        assert_eq!(
            subcommands("usage: tool {pack,unpack,list} ...\n"),
            ["pack", "unpack", "list"]
        );
        assert_eq!(
            subcommands(
                "Usage: tool CMD\n\nCommands:\n  pack    Pack files\n  repair  Repair\n\nOptions:\n  -h\n"
            ),
            ["pack", "repair"]
        );
    }
}
