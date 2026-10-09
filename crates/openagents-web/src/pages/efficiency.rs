//! `/efficiency` (#10210): routed against raw delegation, from the
//! committed Gym study rows (#10162), with the method.
//!
//! Every figure is computed by [`coder::efficiency`] from the rows the
//! repository holds (`bench/efficiency/results/`, `docs/cost/`), compiled
//! into this build; a new study run is published by committing its rows
//! and deploying the site. Nothing here is typed in by hand, and the
//! findings say where routing loses as plainly as where it wins.

use std::sync::OnceLock;

use axum::Router;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use coder::efficiency;
use maud::PreEscaped;
use serde_json::Value;

use crate::App;
use crate::layout::escape;
use crate::ui_page::{UiPage, wide_prose};

const REPO: &str = "https://github.com/OpenAgentsInc/openagents/blob/main/";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/efficiency", get(efficiency_page))
}

fn ratio(v: &Value) -> String {
    match (v["point"].as_f64(), v["low"].as_f64(), v["high"].as_f64()) {
        (Some(p), Some(l), Some(h)) => {
            format!("{p:.2}× <span class=\"oa-page-meta\">({l:.2}–{h:.2})</span>")
        }
        _ => "\u{2014}".into(),
    }
}

fn estimate(v: &Value, f: fn(f64) -> String) -> String {
    match (v["point"].as_f64(), v["low"].as_f64(), v["high"].as_f64()) {
        (Some(p), Some(l), Some(h)) => {
            format!(
                "{} <span class=\"oa-page-meta\">({}–{})</span>",
                f(p),
                f(l),
                f(h)
            )
        }
        _ => "\u{2014}".into(),
    }
}

/// `ratios`: the ratio columns, which compare whole arms, so only the
/// whole study's table has them.
fn arms_table(study: &Value, arms: &Value, ratios: bool) -> String {
    let comparison = |arm: &str| {
        study["comparisons"]
            .as_array()
            .and_then(|c| {
                c.iter()
                    .find(|c| c["arm"] == arm && c["baseline"] == efficiency::BASELINE)
            })
            .cloned()
            .unwrap_or(Value::Null)
    };
    let mut rows = String::new();
    for a in arms.as_array().into_iter().flatten() {
        let arm = a["arm"].as_str().unwrap_or("");
        let c = comparison(arm);
        let base = arm == efficiency::BASELINE;
        rows.push_str(&format!(
            "<tr><th scope=\"row\">{}</th><td>{}</td><td>{}/{} {}</td><td>{}</td><td>{}</td>{}</tr>",
            escape(efficiency::arm_label(arm)),
            a["n"],
            a["passed"],
            a["checked"],
            match (a["pass_rate"]["low"].as_f64(), a["pass_rate"]["high"].as_f64()) {
                (Some(l), Some(h)) => format!("<span class=\"oa-page-meta\">({:.0}–{:.0}%)</span>", l * 100.0, h * 100.0),
                _ => String::new(),
            },
            estimate(&a["cost_per_checked_usd"], |x| format!("${x:.3}")),
            estimate(&a["time_to_checked_s"], |x| format!("{x:.0} s")),
            if !ratios {
                String::new()
            } else if base {
                "<td>1 (baseline)</td><td>1 (baseline)</td>".into()
            } else {
                format!("<td>{}</td><td>{}</td>", ratio(&c["cost_ratio"]), ratio(&c["time_ratio"]))
            },
        ));
    }
    format!(
        "<table><thead><tr><th scope=\"col\">Arm</th><th scope=\"col\">Runs</th>\
<th scope=\"col\">Passed (95%)</th><th scope=\"col\">Cost per checked result (95%)</th>\
<th scope=\"col\">Time to a checked result, median (95%)</th>{}\
</tr></thead><tbody>{rows}</tbody></table>",
        if ratios {
            "<th scope=\"col\">Cost against raw Claude Code (95%)</th>\
<th scope=\"col\">Time against raw Claude Code (95%)</th>"
        } else {
            ""
        }
    )
}

fn study_section(study: &Value, latest: bool) -> String {
    let mut out = format!(
        "<h{h}>{}</h{h}><p class=\"oa-page-meta\">{} \u{b7} {} runs on {} tasks \u{b7} <a href=\"{REPO}{}\">rows and write-up</a></p>",
        escape(study["name"].as_str().unwrap_or("")),
        escape(study["label"].as_str().unwrap_or("")),
        study["runs"],
        study["tasks"],
        escape(study["source"].as_str().unwrap_or("")),
        h = if latest { 2 } else { 3 },
    );
    out.push_str(&arms_table(study, &study["arms"], true));
    if latest {
        for class in study["classes"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "<h3>Class: {}</h3>",
                escape(class["class"].as_str().unwrap_or(""))
            ));
            out.push_str(&arms_table(study, &class["arms"], false));
        }
    }
    out
}

/// The Decisions section (#10387): how often Jev's thresholded decisions
/// were right, from the committed refit summary. Small samples say so.
fn decisions_section() -> String {
    let p = efficiency::refit::published();
    let mut out = String::from(
        "<h2>Decisions</h2><p>Coder turns Jev's probabilities into actions at fixed thresholds: \
whether a task is hard, which checks to keep, which guidance to include. Each decision is now \
recorded with its probability and joined to the run's independent check. A nightly refit \
proposes a new threshold and adopts it only when it beats the default on held-out runs.</p>",
    );
    let questions = p["questions"].as_array().cloned().unwrap_or_default();
    let measured: Vec<&Value> = questions
        .iter()
        .filter(|q| q["checked_n"].as_u64().unwrap_or(0) >= 20)
        .collect();
    if measured.is_empty() {
        out.push_str(
            "<p class=\"oa-page-meta\">Not enough data yet: no decision has 20 checked runs. \
Accuracy and reliability appear here once one does.</p>",
        );
    } else {
        out.push_str(
            "<table><thead><tr><th>Decision</th><th>Threshold</th><th>Runs</th>\
<th>Checked</th><th>Right at threshold</th></tr></thead><tbody>",
        );
        for q in measured {
            let acc = q["accuracy_at_threshold"]
                .as_f64()
                .map_or_else(|| "\u{2014}".to_owned(), |a| format!("{:.0}%", 100.0 * a));
            out.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(q["site"].as_str().unwrap_or("")),
                q["threshold"],
                q["n"],
                q["checked_n"],
                acc
            ));
        }
        out.push_str("</tbody></table>");
    }
    out.push_str(
        "<table><thead><tr><th>Setting</th><th>Default</th><th>In effect</th>\
<th>Labelled runs</th><th>Status</th></tr></thead><tbody>",
    );
    for s in p["settings"].as_array().into_iter().flatten() {
        let name = s["setting"].as_str().unwrap_or("");
        let flag = if s["unmeasured_default"] == serde_json::json!(true) {
            " <span class=\"oa-page-meta\">(default never measured)</span>"
        } else {
            ""
        };
        out.push_str(&format!(
            "<tr><td>{}{flag}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(name),
            s["default"],
            s["in_effect"],
            s["labelled_n"],
            escape(s["reason"].as_str().unwrap_or(""))
        ));
    }
    out.push_str(&format!(
        "</tbody></table><p class=\"oa-page-meta\">Accuracy is measured against the run's independent \
check, a proxy for whether each decision was right; the hard decision is measured against \
whether the run took more than 10 minutes. See <a href=\"{REPO}docs/research/typesafe/2026-10-03-calibration.md\">\
the calibration plan</a>.</p>"
    ));
    out
}

fn body() -> &'static str {
    static BODY: OnceLock<String> = OnceLock::new();
    BODY.get_or_init(|| {
        let studies = efficiency::studies(&[]);
        let findings = efficiency::findings(&studies);
        let mut out = String::from(
            "<section aria-labelledby=\"efficiency-title\"><h1 id=\"efficiency-title\">Efficiency</h1>\
<p class=\"oa-page-lead\">Does routing work through OpenAgents beat handing it straight to Claude Code or \
Codex? The same pinned tasks run through each, an independent check decides every pass, and \
these are the numbers, wins and losses alike.</p><h2>Findings</h2><ul>",
        );
        for f in &findings {
            out.push_str(&format!("<li>{}</li>", escape(f)));
        }
        out.push_str(
            "</ul><p class=\"oa-page-meta\">A ratio below 1 favors the arm. \u{201c}No measurable difference\u{201d} \
means the 95% interval includes 1.</p>",
        );
        if let Some((latest, earlier)) = studies.split_last() {
            out.push_str(&study_section(latest, true));
            if !earlier.is_empty() {
                out.push_str("<h2>Earlier studies</h2><p class=\"oa-page-meta\">Each compared only within itself: \
the code, arms, and tasks changed between them.</p>");
                for s in earlier.iter().rev() {
                    out.push_str(&study_section(s, false));
                }
            }
        }
        out.push_str(&decisions_section());
        out.push_str(&format!(
            "<h2>Method</h2><ul>\
<li><b>Tasks.</b> A pinned set: Terminal-Bench 2.1 tasks moved onto a host, and real fixes from \
public repositories merged after the models' training cutoff, each asked as an issue. The set \
is named in every row, and rows from different sets are never pooled.</li>\
<li><b>Arms.</b> Raw Claude Code (<code>claude -p</code> on its own defaults), raw Codex \
(<code>codex exec</code> on the routed default's model and effort), and OpenAgents' routed paths \
(<code>openagents chat send</code>): the shipped default, and Claude Code run as one lean, \
briefed session.</li>\
<li><b>Checks.</b> A pass is the task's own tests or the fix commit's own test file, run \
afterwards on the work the run left, never the agent's own claim. Every check fails on the \
untouched task and passes on the reference fix.</li>\
<li><b>Cost.</b> List price for the engine plus Jev, as each run recorded it, not a bill. Cost \
per checked result is everything an arm spent, failures included, over its passes. The chat \
router's own model call is not metered and not counted.</li>\
<li><b>Time.</b> Wall time from the command's start to the result settling. Time to a checked \
result is the median over passing runs.</li>\
<li><b>Intervals.</b> Pass rates use Wilson's 95% interval. Costs, times, and ratios use a \
95% bootstrap that resamples trials within each task. Ratios against raw Claude Code are sums \
over tasks of per-task means.</li></ul>\
<p>Read <a href=\"{REPO}docs/cost/2026-10-02-system-one-cost-efficiency-audit.md\">the cost audit</a>, \
<a href=\"{REPO}docs/cost/2026-10-02-shadow-baseline-measurement.md\">the first shadow-baseline \
measurement</a>, and <a href=\"{REPO}bench/efficiency/README.md\">the standing study's runbook</a>; \
the rows behind every number on this page are in the repository. On your own computer, \
<code>openagents efficiency</code> adds your routed runs and shadow baselines.</p></section>"
        ));
        out
    })
}

async fn efficiency_page(headers: HeaderMap) -> Response {
    UiPage::new("Efficiency")
        .path("/efficiency")
        .scriptless()
        .content(wide_prose(PreEscaped(body())))
        .respond(&headers)
}

#[cfg(test)]
mod decisions_tests {
    #[test]
    fn the_page_has_a_decisions_section_that_says_when_data_is_short() {
        let html = super::decisions_section();
        assert!(html.contains("<h2>Decisions</h2>"));
        assert!(html.contains("recipe.hard"));
        assert!(html.contains("default never measured"));
        assert!(html.contains("Not enough data yet") || html.contains("Right at threshold"));
        assert!(super::body().contains("<h2>Decisions</h2>"));
    }
}
