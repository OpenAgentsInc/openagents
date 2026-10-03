//! `openagents shadow`: the shadow baseline's records (#10209), what a
//! sample of this computer's Coder runs would have cost through the raw
//! engine (`coder::task::shadow`). Turned on and budgeted only by the
//! person, with `openagents settings set coder.shadow PERCENT` and
//! `coder.shadow_budget_usd`.

use serde_json::{Value, json};

use crate::Output;
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::task::{local, shadow};

pub(crate) const USAGE: &str = "usage: openagents shadow COMMAND
  report      Finish the baselines that have ended (run the recipe's kept
              checks on both sides, write the record, delete the scratch
              copy), then print the routed runs against their raw baselines:
              cost and wall time, totals and medians, and checks passed.
  records     Finish the ended baselines and print every record.
Off unless set: `openagents settings set coder.shadow PERCENT` runs that share
of this computer's finished Coder runs (a first turn on Codex or Claude Code)
once more through the raw engine on its own defaults, in a scratch copy of the
same commit with no remote; its changes are never applied. One runs at a time,
and `coder.shadow_budget_usd` stops new ones once the recorded baselines cost
that much. Records live in ~/.openagents/shadow/ beside the task store.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("report", Effect::LocalWrite),
    Declared::computer("records", Effect::LocalWrite),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some(command) = words.first() else {
        return output.usage("shadow", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    if words.len() != 1 {
        return output.usage("shadow", &format!("`{command}` takes no words"), USAGE);
    }
    let store = local::default_store();
    let finished = shadow::finalize(&store);
    let records = shadow::records(&shadow::dir(&store));
    match command.as_str() {
        "report" => {
            let report = shadow::report(&records);
            output.emit(
                &json!({"finished_now": finished.len(), "report": report}),
                |value| {
                    text(
                        &value["report"],
                        value["finished_now"].as_u64().unwrap_or(0),
                    )
                },
            );
            0
        }
        "records" => {
            output.emit(&json!({"records": records}), |value| {
                value["records"]
                    .as_array()
                    .map(|records| {
                        records
                            .iter()
                            .map(|r| {
                                format!(
                                    "{} routed {} {} · baseline {} {}",
                                    r["task"].as_str().unwrap_or("").get(..12).unwrap_or(""),
                                    r["routed"]["engine"].as_str().unwrap_or("?"),
                                    money(&r["routed"]["cost_usd"]),
                                    r["baseline"]["engine"].as_str().unwrap_or("?"),
                                    money(&r["baseline"]["cost_usd"]),
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default()
            });
            0
        }
        other => output.usage("shadow", &format!("unknown command `{other}`"), USAGE),
    }
}

fn money(value: &Value) -> String {
    value
        .as_f64()
        .map_or_else(|| "unknown".into(), |usd| format!("${usd:.4}"))
}

fn text(report: &Value, finished_now: u64) -> String {
    if report["records"].as_u64() == Some(0) {
        let on = coder::task::settings::load()
            .ok()
            .and_then(|settings| settings.coder.shadow_percent)
            .is_some();
        return if on {
            "No shadow baselines have finished yet.".into()
        } else {
            "No shadow baselines yet: shadow runs are off. \
             `openagents settings set coder.shadow 10` runs 10% of finished Coder runs again \
             through the raw engine."
                .into()
        };
    }
    let cost = &report["cost_usd"];
    let wall = &report["wall_s"];
    let saving = |side: &Value| {
        side["saving"]
            .as_f64()
            .map_or_else(|| "n/a".into(), |s| format!("{:.0}%", s * 100.0))
    };
    let number = |value: &Value| {
        value
            .as_f64()
            .map_or_else(|| "n/a".into(), |v| format!("{v:.1} s"))
    };
    format!(
        "{} records ({} finished now)\ncost, {} pairs: routed {} vs raw {} in all; medians {} vs {}; saving {}\nwall time, {} pairs: routed {} vs raw {} in all; medians {} vs {}; saving {}\nkept checks: routed {} passed, {} failed; raw {} passed, {} failed",
        report["records"],
        finished_now,
        cost["n"],
        money(&cost["routed_total"]),
        money(&cost["baseline_total"]),
        money(&cost["routed_median"]),
        money(&cost["baseline_median"]),
        saving(cost),
        wall["n"],
        number(&wall["routed_total"]),
        number(&wall["baseline_total"]),
        number(&wall["routed_median"]),
        number(&wall["baseline_median"]),
        saving(wall),
        report["checks"]["routed_passed"],
        report["checks"]["routed_failed"],
        report["checks"]["baseline_passed"],
        report["checks"]["baseline_failed"],
    )
}
