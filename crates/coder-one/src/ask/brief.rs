//! The briefing an ask's executor reads: the question, how to answer it,
//! what the Gym holds overall, and the evidence the host and Jev chose.
//! Code assembles it from what the probes returned, in a fixed order, and
//! caps it; the executor's `read` tool reaches anything it leaves out.

use serde_json::{Value, json};

use super::clip;

/// The most characters a briefing takes.
pub const CAP: usize = 48_000;

/// How the executor is told to work and to answer.
pub const HOW: &str = "\
You answer an operator's question about Terminal-Bench runs by reading the Gym, the \
store of every run's records and Jev's judgments of what each run shows. You only \
read: nothing you do changes a file.

- The host already read the Gym and chose the evidence below with Jev. Start from it. \
Use the `read` tool only for what the briefing leaves out, such as another run's \
`gym runs show JOB/TRIAL --json`, or `gym runs --reason ID --json` for every run with \
a judgment. Each read costs time; most answers need a few reads or none.
- Finish by calling the `answer` tool once. Put the answer in a few short paragraphs, \
patterns first, then break it into claims. Every claim cites the runs it rests on, \
exactly as `job/trial`, and where it helps, transcript step numbers from the run's \
`step` field and judgment IDs such as `unearned_success`. Cite a judgment only for a \
run whose probability for it is at or above 0.50. Code checks every citation, and a \
claim whose citation doesn't check is shown as unverified.
- Say what the evidence doesn't settle. Don't count or rank runs you haven't read; \
the group counts below are exact, so use them for how many.
- When the findings suggest a change to Coder One, a check, or a briefing, name it in \
`proposed_change` for a person to decide on. Don't make it.
- When the findings support a concrete change, also put it in `proposals`, typed, at most \
4. A person approves each before anything runs, and then it runs on the tasks you name \
and no others. Name its `source_runs`, runs your claims cite, and its `expected_tasks`, \
those runs' tasks. Prefer a `policy` or `check` proposal: a JSON merge patch on a manifest \
in `crates/coder-one/policies/` (read it with `cat` first), which code applies and \
validates. A `check` changes only `policy.verify`. A change that needs Rust, including a \
new check scenario or a question-set change, is `code` or `questions`, with the issue \
body in `issue`. Propose nothing when the evidence doesn't support a change.";

/// How the executor is told to draft highlights.
pub const HOW_DRAFTS: &str = "\
You write short drafts a person may post about Terminal-Bench findings. Code computed \
each claim below from the retained runs with fixed rules; you only word it. Nothing you \
write posts anywhere: a person picks, edits, and posts.

- Write one draft per claim: one to three plain, specific sentences for a reader who \
hasn't seen the runs. Say what was compared and on which task.
- Use only numbers the claim writes, as it writes them: its sentence, its numbers, and its \
caveats. Code refuses a draft with any other number, a rounded or computed one included. \
A model name such as Opus 5.5 is a number the claim writes.
- Keep the caveat that matters most. When a claim rests on one run (n=1), say so, such as \
\"in one run\", and never phrase it as a benchmark result.
- Call the `answer` tool once. Put one line in `answer`. Put each draft in `claims`: \
`claim` is the draft, `highlight` is the claim's key, `runs` are the runs it rests on, \
chosen from the claim's runs exactly as `job/trial`, and `steps`, `judgments`, `files`, \
and `marks` are empty. Code checks every draft's numbers and citations.
- Everything you need is here; you don't need the `read` tool.";

/// One run the briefing opens in full.
#[derive(Clone, Debug)]
pub struct Opened {
    pub id: String,
    /// Jev's relevance probability, when Jev answered.
    pub relevance: Option<f64>,
    /// `gym runs show RUN --json`.
    pub shown: Value,
    /// `gym runs show RUN --evidence`: the state Jev ranked the run from.
    pub evidence: Option<Value>,
    /// The transcript steps Jev chose: step, headline, body, probability.
    pub steps: Vec<(u64, String, String, Option<f64>)>,
}

/// What the briefing is built from.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub question: String,
    pub context: Vec<String>,
    /// `gym runs group --by reason --json`.
    pub reasons: Option<Value>,
    /// The runs list's totals: runs, judged, running.
    pub totals: Option<Value>,
    /// `gym coder matrix --json`.
    pub matrix: Option<Value>,
    /// The reasons Jev said the question asks about, with probabilities.
    pub asked_reasons: Vec<(String, Option<f64>)>,
    /// Tasks the question names.
    pub tasks: Vec<String>,
    pub opened: Vec<Opened>,
    /// The candidates not opened: summary and relevance.
    pub others: Vec<(Value, Option<f64>)>,
    /// For a repository question: path, relevance, and excerpt.
    pub files: Vec<(String, Option<f64>, String)>,
    /// `gym runs marks --json`: a person's marks on runs.
    pub marks: Option<Value>,
    /// Whether the question asks about marked runs.
    pub about_marks: bool,
    /// For a highlights ask, the highlights to draft, as `gym runs
    /// highlights --json` gives them.
    pub highlights: Vec<Value>,
    /// Strategy evidence: `gym runs fingerprints --task T --json` per named
    /// task, and `gym runs moves --cached --json` when the question asks
    /// about strategies.
    pub strategy: Vec<Value>,
}

/// The strategy fingerprints and candidate moves, compactly: one line per
/// trajectory, and the strongest moves with their citations.
#[must_use]
pub fn strategy_section(strategy: &[Value]) -> String {
    let mut out = String::from(
        "# Strategy fingerprints and candidate moves\n\n\
         A fingerprint summarizes one trajectory's steps by phase (orient, read, plan, edit, \
         build, test, verify, finish): the step of its first edit, its test count, its \
         verification after the last edit, its retries, whether it ran the task's example \
         before editing, and its phase sequence (`O3 R5 E1`, `?` unplaced). Step numbers \
         here are fingerprint steps, which `gym runs fingerprint RUN --no-jev` lists; they are not \
         the transcript's `step` field, so don't cite them as steps. Fable trajectories are \
         public trial IDs, not `job/trial`, so the citation check can't open them: cite local \
         runs as usual and name Fable trials in the claim's text. A candidate move is a \
         difference that repeats across tasks, a hypothesis rather than a finding.\n\n",
    );
    for value in strategy {
        if let Some(prints) = value["fingerprints"].as_array() {
            let tasks: Vec<&str> = value["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            out.push_str(&format!(
                "Fingerprints for {} ({} trajectories; `gym runs fingerprints --task T`):\n",
                tasks.join(", "),
                prints.len()
            ));
            for print in prints.iter().take(45) {
                let first_edit = print["first_edit_step"]
                    .as_u64()
                    .map_or("never".to_string(), |n| format!("step {n}"));
                let example = if print["task_has_example"] == true {
                    if print["ran_example_before_edit"] == true {
                        "ran the example first"
                    } else {
                        "didn't run the example first"
                    }
                } else {
                    "no example named"
                };
                out.push_str(&format!(
                    "- {} · {} · {} · {} steps · first edit {} · {} tests · {} checks after the last edit · {} retries · {} · {}\n",
                    print["run"].as_str().unwrap_or("?"),
                    print["arm"].as_str().unwrap_or("?"),
                    print["outcome"].as_str().unwrap_or("?"),
                    print["steps"],
                    first_edit,
                    print["tests"],
                    print["verification"]["after_last_edit"],
                    print["retries"]["repeated_failed"].as_u64().unwrap_or(0)
                        + print["retries"]["by_jev"].as_u64().unwrap_or(0),
                    example,
                    clip(print["sequence"].as_str().unwrap_or(""), 90),
                ));
            }
            out.push('\n');
        }
        if let Some(moves) = value
            .pointer("/report/candidates")
            .and_then(Value::as_array)
        {
            out.push_str(
                "Candidate moves across the Luna baseline subset, strongest first (`gym runs moves`):\n",
            );
            for one in moves.iter().take(10) {
                let cites: Vec<String> = one["citations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(3)
                    .map(|c| {
                        format!(
                            "{}: {} vs {}",
                            c["task"].as_str().unwrap_or("?"),
                            c["a_run"].as_str().unwrap_or("?"),
                            c["b_run"].as_str().unwrap_or("?")
                        )
                    })
                    .collect();
                out.push_str(&format!(
                    "- {} {} {}: {} of {} tasks, mean Cliff's delta {:+.2} ({}). {}\n",
                    one["a"].as_str().unwrap_or("?"),
                    if one["direction"] == "more" {
                        "does more:"
                    } else {
                        "does less:"
                    },
                    one["what"].as_str().unwrap_or("?"),
                    one["tasks_agreeing"],
                    one["tasks_with_data"],
                    one["mean_delta"].as_f64().unwrap_or(0.0),
                    one["comparison"].as_str().unwrap_or("?"),
                    cites.join("; ")
                ));
            }
            out.push('\n');
        }
    }
    out
}

/// Builds the briefing.
#[must_use]
pub fn build(inputs: &Inputs) -> String {
    let mut out = String::new();
    out.push_str("# Question\n\n");
    out.push_str(inputs.question.trim());
    out.push_str("\n\n");
    if !inputs.context.is_empty() {
        out.push_str("The operator asked from a terminal showing:\n");
        for line in &inputs.context {
            out.push_str(&format!("- {line}\n"));
        }
        out.push('\n');
    }
    if !inputs.highlights.is_empty() {
        out.push_str("# How to draft\n\n");
        out.push_str(HOW_DRAFTS);
        out.push_str("\n\n# Claims to draft\n\n");
        for highlight in &inputs.highlights {
            out.push_str(&highlight_section(highlight));
            out.push('\n');
        }
        return clip(&out, CAP);
    }
    out.push_str("# How to answer\n\n");
    out.push_str(HOW);
    out.push_str("\n\n");

    if inputs.reasons.is_some() || inputs.totals.is_some() {
        out.push_str("# What the Gym holds\n\n");
    }
    if let Some(totals) = &inputs.totals {
        out.push_str(&format!(
            "{} runs, {} of them judged by Jev with the `runs-learning-v1` question set; {} running.\n\n",
            totals["total"], totals["ranked"], totals["running"]
        ));
    }
    if let Some(reasons) = &inputs.reasons {
        out.push_str("Runs per reason, where a run is in every reason Jev gave it at 0.50 or above (exact counts from `gym runs group --by reason`):\n");
        for group in reasons["groups"].as_array().into_iter().flatten() {
            let strongest: Vec<String> = group["mean_probability"]
                .as_array()
                .into_iter()
                .flatten()
                .take(3)
                .map(|m| format!("{} {}", m["id"].as_str().unwrap_or("?"), m["mean"]))
                .collect();
            out.push_str(&format!(
                "- `{}` ({}): {} runs; mean over them: {}\n",
                group["key"].as_str().unwrap_or("?"),
                group["tag"].as_str().unwrap_or(""),
                group["count"],
                strongest.join(", ")
            ));
        }
        out.push('\n');
    }
    if !inputs.asked_reasons.is_empty() || !inputs.tasks.is_empty() {
        out.push_str("Jev read the question as asking about ");
        let mut parts: Vec<String> = inputs
            .asked_reasons
            .iter()
            .map(|(id, p)| match p {
                Some(p) => format!("the reason `{id}` ({p:.2})"),
                None => format!("the reason `{id}`"),
            })
            .collect();
        parts.extend(inputs.tasks.iter().map(|t| format!("the task `{t}`")));
        out.push_str(&parts.join(", "));
        out.push_str(", so the candidates include those runs.\n\n");
    }

    let marks: Vec<&Value> = inputs
        .marks
        .as_ref()
        .and_then(|marks| marks["marks"].as_array())
        .into_iter()
        .flatten()
        .collect();
    if !marks.is_empty() {
        out.push_str("# A person's marks\n\n");
        out.push_str(
            "A mark is a person's word on a run, or on one step of its transcript: bad, \
             with the judgment IDs that name what went wrong and a note, or cleared, \
             meaning nothing is wrong. A mark outranks Jev's judgment. Cite a mark in \
             a claim's `marks` as `job/trial`, or `job/trial/STEP` for a step's mark; \
             code checks it.",
        );
        if inputs.about_marks {
            out.push_str(
                " Jev read the question as asking about marked runs, so the marked \
                 runs come first among the candidates.",
            );
        }
        out.push_str("\n\n");
        for mark in marks.iter().take(40) {
            out.push_str(&format!("- {}\n", mark_line(mark)));
        }
        if marks.len() > 40 {
            out.push_str(&format!(
                "- and {} more; `gym runs marks --json` lists them.\n",
                marks.len() - 40
            ));
        }
        out.push('\n');
    }

    if !inputs.strategy.is_empty() {
        out.push_str(&strategy_section(&inputs.strategy));
    }

    if !inputs.opened.is_empty() {
        out.push_str("# The runs Jev judged most relevant\n\n");
        let per_run = (CAP / 2) / inputs.opened.len().max(1);
        for opened in &inputs.opened {
            out.push_str(&clip(&run_section(opened), per_run));
            out.push_str("\n\n");
        }
    }

    if !inputs.files.is_empty() {
        out.push_str("# Repository files Jev judged relevant\n\n");
        for (path, p, excerpt) in &inputs.files {
            out.push_str(&format!(
                "## `{path}`{}\n\n```\n{}\n```\n\n",
                p.map_or(String::new(), |p| format!(" (relevance {p:.2})")),
                clip(excerpt, 3_000)
            ));
        }
    }

    if !inputs.others.is_empty() {
        out.push_str("# Other candidate runs\n\nNot opened here; `gym runs show JOB/TRIAL --json` reads one.\n\n");
        for (summary, p) in &inputs.others {
            out.push_str(&format!(
                "- `{}` {} · {} · {}{}{}\n",
                summary["run"].as_str().unwrap_or("?"),
                summary["task"].as_str().unwrap_or("?"),
                summary["agent"].as_str().unwrap_or("?"),
                summary["outcome"].as_str().unwrap_or("?"),
                match summary["reasons"].as_array() {
                    Some(r) if !r.is_empty() => format!(
                        " · reasons: {}",
                        r.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    _ => String::new(),
                },
                p.map_or(String::new(), |p| format!(" · relevance {p:.2}"))
            ));
        }
        out.push('\n');
    }

    if let Some(matrix) = &inputs.matrix {
        out.push_str("# The outcome matrix\n\n");
        out.push_str(&matrix_lines(matrix));
        out.push('\n');
    }
    clip(&out, CAP)
}

/// One mark as a line: its target, verdict, tags, note, and author.
fn mark_line(mark: &Value) -> String {
    let run = mark["run"].as_str().unwrap_or("?");
    let target = match mark["step"].as_u64() {
        Some(step) => format!("`{run}/{step}` (step {step})"),
        None => format!("`{run}`"),
    };
    let mut line = format!(
        "{target}{}: {}",
        mark["task"]
            .as_str()
            .map_or(String::new(), |task| format!(" {task}")),
        if mark["verdict"] == "clear" {
            "cleared, nothing wrong"
        } else {
            "marked bad"
        }
    );
    let tags: Vec<&str> = mark["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !tags.is_empty() {
        line.push_str(&format!(", tagged {}", tags.join(", ")));
    }
    if let Some(note) = mark["note"].as_str() {
        line.push_str(&format!(" — \"{}\"", clip(note, 300)));
    }
    if let Some(author) = mark["author"].as_str() {
        line.push_str(&format!(" ({author})"));
    }
    line
}

/// One highlight to draft: its key, claim, sample, numbers, runs, and
/// caveats.
fn highlight_section(highlight: &Value) -> String {
    let mut out = format!(
        "## `{}` ({})\n\n{}\n\n",
        highlight["key"].as_str().unwrap_or("?"),
        highlight["rule"].as_str().unwrap_or("?"),
        highlight["claim"].as_str().unwrap_or("")
    );
    out.push_str(&if highlight["n1"] == true {
        "Sample: n=1. It rests on one run: say so, and don't phrase it as a benchmark result.\n"
            .to_string()
    } else {
        format!("Sample: n={}.\n", highlight["sample"])
    });
    let numbers: Vec<String> = highlight["numbers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|n| {
            format!(
                "{} {}",
                n["label"].as_str().unwrap_or("?"),
                n["text"].as_str().unwrap_or("?")
            )
        })
        .collect();
    if !numbers.is_empty() {
        out.push_str(&format!("Numbers: {}.\n", numbers.join("; ")));
    }
    let runs: Vec<String> = highlight["runs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|run| format!("`{run}`"))
        .collect();
    out.push_str(&format!("Runs: {}.\n", runs.join(", ")));
    for caveat in highlight["caveats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        out.push_str(&format!("- Caveat: {caveat}\n"));
    }
    out
}

/// One opened run: its facts, every judgment, the evidence Jev ranked it
/// from, and the transcript steps Jev chose.
fn run_section(opened: &Opened) -> String {
    let run = &opened.shown["run"];
    let mut out = format!(
        "## `{}`{}\n\n",
        opened.id,
        opened
            .relevance
            .map_or(String::new(), |p| format!(" (relevance {p:.2})"))
    );
    out.push_str(&format!(
        "{} · {} · {}{} · {} · cost {} · agent time {}\n",
        run["task"].as_str().unwrap_or("?"),
        opened.shown["byline"].as_str().unwrap_or(""),
        run["outcome"].as_str().unwrap_or("?"),
        run["why_not_graded"]
            .as_str()
            .map_or(String::new(), |why| format!(" ({why})")),
        run["tests"]
            .as_object()
            .map_or("no test counts".to_string(), |t| format!(
                "{} of {} tests passed",
                t["passed"], t["total"]
            )),
        run["cost_usd"]
            .as_f64()
            .map_or("unknown".to_string(), |c| format!("${c:.4}")),
        run["agent_ms"]
            .as_u64()
            .map_or("unknown".to_string(), |ms| format!(
                "{:.0}s",
                ms as f64 / 1000.0
            )),
    ));
    let learning = &opened.shown["learning"];
    if learning.is_object() {
        let every: Vec<String> = learning["every_judgment"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|j| j["probability"].as_f64().unwrap_or(0.0) >= 0.3)
            .map(|j| {
                format!(
                    "{}{} {:.2}",
                    j["id"].as_str().unwrap_or("?"),
                    if j["reason"] == true { "*" } else { "" },
                    j["probability"].as_f64().unwrap_or(0.0)
                )
            })
            .collect();
        out.push_str(&format!(
            "Jev: learning value {}, overall score {}; judgments at 0.30 or above (* marks a reason, 0.50 or above): {}\n",
            learning["learning"],
            learning["value"],
            if every.is_empty() { "none".to_string() } else { every.join(", ") }
        ));
    } else {
        out.push_str("Jev hasn't judged this run.\n");
    }
    for mark in opened.shown["marks"].as_array().into_iter().flatten() {
        out.push_str(&format!("A person's mark: {}\n", mark_line(mark)));
    }
    for paragraph in opened.shown["summary"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{}: {}\n",
            paragraph["heading"].as_str().unwrap_or(""),
            clip(paragraph["text"].as_str().unwrap_or(""), 700)
        ));
    }
    if let Some(evidence) = &opened.evidence {
        let state = &evidence["state"];
        let keep = json!({
            "run": state["run"],
            "coder_one": state["coder_one"],
            "activity": state["activity"],
            "leaderboard": state["leaderboard"],
            "other_runs": state["other_runs"],
        });
        out.push_str(&format!(
            "Evidence Jev ranked the run from (`gym runs show {} --evidence`, trimmed): {}\n",
            opened.id,
            clip(&keep.to_string(), 2_500)
        ));
    }
    if !opened.steps.is_empty() {
        out.push_str(&format!(
            "Transcript steps Jev judged relevant ({} steps in all):\n",
            opened.shown["transcript"].as_array().map_or(0, Vec::len)
        ));
        for (step, headline, body, p) in &opened.steps {
            out.push_str(&format!(
                "- step {step}{}: {} — {}\n",
                p.map_or(String::new(), |p| format!(" ({p:.2})")),
                clip(headline, 160),
                clip(&body.replace('\n', " ⏎ "), 900)
            ));
        }
    }
    out
}

/// The matrix's frontier cells per task and its policies' totals.
fn matrix_lines(matrix: &Value) -> String {
    let mut out = String::new();
    if let Some(objective) = matrix["params"]["objective"].as_str() {
        out.push_str(&format!("Objective: {objective}\n"));
    }
    for task in matrix["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let frontier: Vec<String> = matrix["cells"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["task"] == task && c["frontier"] == true)
            .map(|c| {
                format!(
                    "{} {}/{} ${:.4}",
                    c["arms"]
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(Value::as_str)
                        .unwrap_or("?"),
                    c["passes"],
                    c["trials"],
                    c["mean_cost_usd"].as_f64().unwrap_or(f64::NAN)
                )
            })
            .collect();
        out.push_str(&format!("- {task}: frontier {}\n", frontier.join("; ")));
    }
    for policy in matrix["complete_policies"]
        .as_array()
        .into_iter()
        .flatten()
        .take(8)
    {
        out.push_str(&format!(
            "- policy {}: {}/{} passed, summed mean cost ${:.4}\n",
            policy["name"].as_str().unwrap_or("?"),
            policy["passes"],
            policy["trials"],
            policy["summed_mean_cost_usd"].as_f64().unwrap_or(f64::NAN)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_briefing_puts_the_question_the_rules_and_the_evidence_in_order() {
        let inputs = Inputs {
            question: "Why did these runs claim success?".to_string(),
            context: vec!["selected run: tb4--a/a__1".to_string()],
            reasons: Some(
                json!({"groups": [{"key": "unearned_success", "tag": "claimed unearned success", "count": 95, "mean_probability": [{"id": "unearned_success", "mean": 0.95}]}]}),
            ),
            totals: Some(json!({"total": 597, "ranked": 561, "running": 0})),
            asked_reasons: vec![("unearned_success".to_string(), Some(0.93))],
            opened: vec![Opened {
                id: "tb4--a/a__1".to_string(),
                relevance: Some(0.9),
                shown: json!({
                    "run": {"task": "a", "outcome": "failed", "tests": {"passed": 1, "total": 5}, "cost_usd": 0.5, "agent_ms": 60000},
                    "learning": {"learning": 0.7, "value": 2.5, "every_judgment": [{"id": "unearned_success", "probability": 0.97, "reason": true}]},
                    "summary": [{"heading": "What happened", "text": "It reported success."}],
                    "transcript": [{"step": 1}, {"step": 2}],
                }),
                evidence: None,
                steps: vec![(
                    2,
                    "Report".to_string(),
                    "All tests pass.".to_string(),
                    Some(0.8),
                )],
            }],
            ..Inputs::default()
        };
        let text = build(&inputs);
        let order = [
            "# Question",
            "selected run",
            "# How to answer",
            "597 runs",
            "`unearned_success` (claimed unearned success): 95 runs",
            "the reason `unearned_success` (0.93)",
            "## `tb4--a/a__1` (relevance 0.90)",
            "unearned_success* 0.97",
            "What happened: It reported success.",
            "step 2 (0.80): Report — All tests pass.",
        ];
        let mut at = 0;
        for needle in order {
            let found = text[at..]
                .find(needle)
                .unwrap_or_else(|| panic!("{needle} after {at}:\n{text}"));
            at += found;
        }
        assert!(text.chars().count() <= CAP + 1);
    }
}
