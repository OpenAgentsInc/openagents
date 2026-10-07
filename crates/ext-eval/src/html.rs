//! `report.html`: a self-contained view of a result.
//!
//! One file with its styles inline: no scripts, no fonts, no images, and no
//! request to anywhere. The only links are relative ones to each run's
//! trajectory under the same results directory
//! ([`crate::record::run_path`]).

use std::fmt::Write as _;

use crate::evaluate::Evaluation;
use crate::record::{Arm, run_path};
use crate::score::{CaseArm, GradedRun};

const STYLE: &str = "\
:root{--bg:#fbfaf7;--fg:#1d1c1a;--muted:#6b675f;--line:#e2ded5;--good:#23703a;--bad:#a4282a;--mid:#8a6a12;--card:#ffffff}\
@media (prefers-color-scheme:dark){:root{--bg:#141311;--fg:#ece9e2;--muted:#a09a8f;--line:#34312b;--good:#6fcf8a;--bad:#f08a86;--mid:#e5c15a;--card:#1c1b18}}\
*{box-sizing:border-box}\
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.5 \"Paper Mono\",monospace}\
main{max-width:960px;margin:0 auto;padding:24px 16px 48px}\
h1{font-size:22px;margin:0 0 4px}h2{font-size:17px;margin:32px 0 8px}h3{font-size:15px;margin:0}\
.muted{color:var(--muted)}.good{color:var(--good)}.bad{color:var(--bad)}.mid{color:var(--mid)}\
.headline{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:16px;margin:16px 0}\
.verdict{font-size:20px;font-weight:600}\
table{border-collapse:collapse;width:100%;font-size:14px}\
th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line);vertical-align:top}\
.case{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:12px 16px;margin:12px 0}\
details{margin:6px 0}summary{cursor:pointer}\
code{font-family:inherit;font-size:13px;overflow-wrap:anywhere}\
.wrap{overflow-x:auto}";

/// Escapes text for HTML.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn score(value: Option<f64>) -> String {
    value.map_or_else(|| "unknown".into(), |value| format!("{value:.2}"))
}

fn signed(value: Option<f64>) -> String {
    value.map_or_else(|| "unknown".into(), |value| format!("{value:+.2}"))
}

fn money(value: Option<f64>) -> String {
    value.map_or_else(|| "unknown".into(), |value| format!("${value:.4}"))
}

fn seconds(value: Option<f64>) -> String {
    value.map_or_else(|| "unknown".into(), |value| format!("{value:.1} s"))
}

fn pass_word(passed: Option<bool>) -> &'static str {
    match passed {
        Some(true) => "<span class=\"good\">passed</span>",
        Some(false) => "<span class=\"bad\">failed</span>",
        None => "<span class=\"mid\">unknown</span>",
    }
}

fn arm_cell(arm: Option<&CaseArm>) -> String {
    arm.map_or_else(
        || "<span class=\"muted\">not run</span>".into(),
        |arm| {
            format!(
                "{} · {} of {} runs · score {}",
                pass_word(arm.passed),
                arm.runs_passed,
                arm.planned,
                score(arm.score)
            )
        },
    )
}

/// Renders the report page.
#[must_use]
pub fn render(evaluation: &Evaluation) -> String {
    let scores = &evaluation.scores;
    let total = scores.cases.len();
    let (class, word) = match evaluation.verdict {
        crate::evaluate::Verdict::Pass => ("good", "Better"),
        crate::evaluate::Verdict::Fail => ("bad", "Worse"),
        crate::evaluate::Verdict::Inconclusive => ("mid", "No clear change"),
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
         <title>Plugin test</title>\n<style>{}{STYLE}</style>\n</head>\n<body>\n<main>\n",
        // A report is a file with no server beside it, so it carries its
        // typeface, Paper Mono, inline.
        paper_mono::font_face_inline()
    );
    let _ = write!(
        out,
        "<h1>Plugin test</h1>\n<p class=\"muted\">Evaluator <code>{}</code> · suite \
         <code>{}</code> · report <code>{}</code></p>\n",
        escape(&evaluation.evaluator),
        escape(&evaluation.suite_ref.digest),
        escape(&evaluation.report_ref.digest),
    );
    let _ = write!(
        out,
        "<section class=\"headline\">\n<div class=\"verdict {class}\">{word}</div>\n<p>Passes \
         {} of {total} tests with the plugin",
        scores.subject.cases_passed
    );
    if let Some(baseline) = &scores.baseline {
        let _ = write!(
            out,
            ", {} of {} without",
            baseline.cases_passed, baseline.coverage.planned
        );
    }
    let _ = writeln!(
        out,
        ". Change in mean score: {}.</p>",
        signed(scores.change)
    );
    if let Some(partial) = &evaluation.partial {
        let _ = writeln!(out, "<p class=\"mid\">Partial: {}</p>", escape(partial));
    }
    let _ = write!(
        out,
        "<p class=\"muted\">Decided by gate <code>{}</code> <code>{}</code>.</p>\n</section>\n",
        escape(&evaluation.gate.gate_id),
        escape(&evaluation.gate.gate_digest)
    );

    out.push_str("<h2>Per arm</h2>\n<div class=\"wrap\"><table>\n<tr><th></th><th>Cases passed</th><th>Mean score</th><th>Cost</th><th>Time</th></tr>\n");
    for (label, summary) in [
        ("With the plugin", Some(&scores.subject)),
        ("Without it", scores.baseline.as_ref()),
    ] {
        let Some(summary) = summary else { continue };
        let _ = writeln!(
            out,
            "<tr><td>{label}</td><td>{} of {}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            summary.cases_passed,
            summary.coverage.planned,
            score(summary.mean_score),
            money(summary.cost_usd),
            seconds(summary.seconds)
        );
    }
    out.push_str("</table></div>\n");

    out.push_str("<h2>Gate</h2>\n<div class=\"wrap\"><table>\n<tr><th>Criterion</th><th>Verdict</th><th>Detail</th></tr>\n");
    for criterion in &evaluation.gate.criteria {
        let class = match criterion.verdict {
            gym::gate::Verdict::Passed => "good",
            gym::gate::Verdict::Failed => "bad",
            gym::gate::Verdict::Unverifiable => "mid",
        };
        let _ = writeln!(
            out,
            "<tr><td><code>{}</code></td><td class=\"{class}\">{}</td><td>{}</td></tr>",
            escape(&criterion.name),
            criterion.verdict,
            escape(&criterion.detail)
        );
    }
    out.push_str("</table></div>\n");

    out.push_str("<h2>Tests</h2>\n");
    for case in &scores.cases {
        let _ = write!(
            out,
            "<section class=\"case\">\n<h3>{} <span class=\"muted\">· {}{}</span></h3>\n\
             <p>With the plugin: {}<br>Without it: {}<br>Change: {}</p>\n",
            escape(&case.id),
            case.kind,
            if case.subject_only {
                " · scored with the plugin only"
            } else {
                ""
            },
            arm_cell(Some(&case.subject)),
            arm_cell(case.baseline.as_ref()),
            signed(case.change)
        );
        for run in evaluation.runs.iter().filter(|run| run.case == case.id) {
            render_run(&mut out, run);
        }
        out.push_str("</section>\n");
    }
    if !evaluation.limitations.is_empty() {
        out.push_str("<h2>Limitations</h2>\n<ul>\n");
        for limitation in &evaluation.limitations {
            let _ = writeln!(out, "<li>{}</li>", escape(limitation));
        }
        out.push_str("</ul>\n");
    }
    out.push_str("</main>\n</body>\n</html>\n");
    out
}

fn render_run(out: &mut String, run: &GradedRun) {
    let arm = match run.arm {
        Arm::Subject => "with the plugin",
        Arm::Baseline => "without it",
    };
    let reason = crate::report::reason(run)
        .map(|reason| format!(" ({reason})"))
        .unwrap_or_default();
    let _ = write!(
        out,
        "<details>\n<summary>Run {} {arm}: {}{} · score {} · {} · {}</summary>\n",
        run.attempt,
        pass_word(run.passed),
        escape(&reason),
        score(run.score),
        money(run.cost_usd),
        seconds(run.seconds)
    );
    let _ = write!(
        out,
        "<p class=\"muted\">Outcome {}",
        run.outcome.coverage_word()
    );
    if run.trajectory.is_some() {
        let _ = write!(
            out,
            " · <a href=\"{}/trajectory.json\">trajectory</a>",
            escape(&run_path(&run.case, run.arm, run.attempt))
        );
    }
    out.push_str("</p>\n");
    if !run.graders.is_empty() {
        out.push_str("<div class=\"wrap\"><table>\n<tr><th>Grader</th><th>Type</th><th>Result</th><th>Why</th></tr>\n");
        for grader in &run.graders {
            let result = if grader.passed {
                "<span class=\"good\">pass</span>"
            } else {
                "<span class=\"bad\">fail</span>"
            };
            let scored = if grader.scored {
                ""
            } else {
                " <span class=\"muted\">(not scored)</span>"
            };
            let votes = if grader.votes.is_empty() {
                String::new()
            } else {
                format!(
                    " <span class=\"muted\">[{}]</span>",
                    escape(
                        &grader
                            .votes
                            .iter()
                            .map(|vote| vote.answer.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                )
            };
            let _ = writeln!(
                out,
                "<tr><td><code>{}</code></td><td>{}</td><td>{result}{scored}</td><td>{}{votes}</td></tr>",
                escape(&grader.name),
                grader.kind,
                escape(&grader.explanation)
            );
        }
        out.push_str("</table></div>\n");
    }
    if !run.created_files.is_empty() {
        let _ = writeln!(
            out,
            "<p class=\"muted\">Created: <code>{}</code></p>",
            escape(&run.created_files.join(", "))
        );
    }
    if !run.changed_files.is_empty() {
        let _ = writeln!(
            out,
            "<p class=\"muted\">Changed: <code>{}</code></p>",
            escape(&run.changed_files.join(", "))
        );
    }
    out.push_str("</details>\n");
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn text_is_escaped() {
        assert_eq!(
            escape("<a href=\"x\">'&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
        );
    }
}
