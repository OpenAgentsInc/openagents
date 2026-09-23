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
`proposed_change` for a person to decide on. Don't make it.";

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
