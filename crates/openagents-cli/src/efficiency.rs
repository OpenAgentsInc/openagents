//! `openagents efficiency` (#10210): routed against raw delegation, from
//! recorded runs: the committed Gym studies (#10162), this computer's
//! shadow baselines, and its routed Coder runs. Cost is not shown in
//! normal app use; this report is where it is.

use serde_json::json;

use crate::Output;
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::efficiency;
use coder::task::{local, shadow};

pub(crate) const USAGE: &str = "usage: openagents efficiency [--all] [--study FILE]...
       openagents efficiency decisions [--store DIR]
       openagents efficiency refit [--store DIR] [--write] [--export FILE]
Cost per checked result, time to a checked result, and pass rate, routed
against raw Claude Code and raw Codex, with run counts and 95% intervals:
  - the standing Gym study's latest run (--all: every committed study),
    the same pinned tasks through each arm with an independent check;
  - this computer's shadow baselines (openagents shadow report), paired;
  - this computer's routed Coder runs, by engine and class, from the route
    records in ~/.openagents/routes.
  --study FILE   also report a study's rows (bench/efficiency/study.py collect).
  decisions      per-question counts, threshold accuracy (run-pass proxy), and
                 raw-probability reliability, joined to independent checks,
                 cost, and time; --store DIR selects a task store.
  refit          refit each delegation threshold from the joined outcomes and
                 report what a held-out check would adopt; --write adopts
                 those that pass (a new versioned file serving reads, under
                 ~/.openagents/calibration; hosts run it nightly), --export
                 FILE writes the public summary openagents.com/efficiency shows.
Methodology: bench/efficiency/README.md; public summary: openagents.com/efficiency.";

/// What the command does and where the phone runs it, for the chat
/// router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[Declared::computer("", Effect::ReadOnly)];

/// Refit the delegation thresholds; with `write`, adopt those that pass.
/// The report, and the versioned file written, if any.
pub(crate) fn recalibrate(
    store: &std::path::Path,
    write: bool,
) -> Result<(serde_json::Value, Option<std::path::PathBuf>), String> {
    let rows = efficiency::decisions::rows(store);
    let previous = coder_delegate::calibration::path()
        .map(|p| coder_delegate::calibration::read(&p))
        .unwrap_or_default();
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let now = crate::wallet::date(secs);
    let (report, next) = efficiency::refit::refit(&rows, &previous, &now);
    let written = match (write, next) {
        (true, Some(file)) => Some(efficiency::refit::write(&file)?),
        _ => None,
    };
    Ok((report, written))
}

fn refit(output: &Output, words: &[String]) -> u8 {
    let mut store = local::default_store();
    let mut write = false;
    let mut export = None;
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match word.as_str() {
            "--help" | "-h" | "help" => {
                println!("{USAGE}");
                return 0;
            }
            "--write" => write = true,
            "--store" => match rest.next() {
                Some(p) => store = std::path::PathBuf::from(p),
                None => return output.usage("efficiency refit", "--store needs a DIR", USAGE),
            },
            "--export" => match rest.next() {
                Some(p) => export = Some(std::path::PathBuf::from(p)),
                None => return output.usage("efficiency refit", "--export needs a FILE", USAGE),
            },
            other => {
                return output.usage(
                    "efficiency refit",
                    &format!("unexpected argument {other}"),
                    USAGE,
                );
            }
        }
    }
    let (report, written) = match recalibrate(&store, write) {
        Ok(v) => v,
        Err(why) => {
            eprintln!("openagents efficiency refit: {why}");
            return 1;
        }
    };
    if let Some(path) = &export {
        let decisions = efficiency::decisions::report(&efficiency::decisions::rows(&store));
        let public = efficiency::refit::public(&decisions, &report);
        let text = serde_json::to_string_pretty(&public).unwrap_or_default() + "\n";
        if let Err(e) = std::fs::write(path, text) {
            eprintln!(
                "openagents efficiency refit: cannot write {}: {e}",
                path.display()
            );
            return 1;
        }
    }
    let written_text = written.as_ref().map(|p| p.display().to_string());
    output.emit(
        &json!({"refit": report, "written": written_text, "exported": export.map(|p| p.display().to_string())}),
        |v| {
            let mut t = efficiency::refit::text(&v["refit"]);
            match v["written"].as_str() {
                Some(p) => t.push_str(&format!("\nAdopted: wrote {p}.")),
                None if v["refit"]["changed"] == true => {
                    t.push_str("\nNothing written; pass --write to adopt.");
                }
                None => {}
            }
            if let Some(p) = v["exported"].as_str() {
                t.push_str(&format!("\nPublic summary: {p}."));
            }
            t
        },
    );
    0
}

/// The report for this computer, with any extra studies' rows.
pub(crate) fn report(extra: &[(String, String)]) -> serde_json::Value {
    let store = local::default_store();
    let records = shadow::records(&shadow::dir(&store));
    efficiency::report(
        &efficiency::studies(extra),
        &records,
        &efficiency::route_rows(&store),
    )
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.first().is_some_and(|w| w == "refit") {
        return refit(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "decisions") {
        if words[1..]
            .iter()
            .any(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
        {
            println!("{USAGE}");
            return 0;
        }
        let store = match &words[1..] {
            [] => local::default_store(),
            [flag, path] if flag == "--store" => std::path::PathBuf::from(path),
            _ => {
                return output.usage(
                    "efficiency decisions",
                    "expected --store DIR or no arguments",
                    USAGE,
                );
            }
        };
        let report = efficiency::decisions::report(&efficiency::decisions::rows(&store));
        output.emit(&json!({"report": report}), |v| {
            efficiency::decisions::text(&v["report"])
        });
        return 0;
    }
    let mut all = false;
    let mut extra = Vec::new();
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match word.as_str() {
            "--help" | "-h" | "help" => {
                println!("{USAGE}");
                return 0;
            }
            "--all" => all = true,
            "--study" => {
                let Some(path) = rest.next() else {
                    return output.usage("efficiency", "--study needs a FILE", USAGE);
                };
                match std::fs::read_to_string(path) {
                    Ok(text) => extra.push((path.clone(), text)),
                    Err(e) => {
                        return output.usage(
                            "efficiency",
                            &format!("cannot read {path}: {e}"),
                            USAGE,
                        );
                    }
                }
            }
            other => {
                return output.usage("efficiency", &format!("unknown word `{other}`"), USAGE);
            }
        }
    }
    let report = report(&extra);
    output.emit(&json!({"report": report}), |value| {
        efficiency::text(&value["report"], all)
    });
    0
}

/// The report as the terminal's `/efficiency` card: the latest study's
/// arms, its findings, and this computer's own runs in one line each.
pub(crate) fn card(report: &serde_json::Value) -> openagents_terminal::Efficiency {
    let latest = report["studies"].as_array().and_then(|s| s.last());
    let rows = latest
        .and_then(|s| s["arms"].as_array())
        .into_iter()
        .flatten()
        .map(|a| {
            (
                efficiency::arm_label(a["arm"].as_str().unwrap_or("")).to_owned(),
                efficiency::arm_line(a),
            )
        })
        .collect();
    let mut body: Vec<String> = report["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f.as_str().map(str::to_owned))
        .collect();
    let shadow = &report["shadow"];
    body.push(if shadow["pairs"].as_u64().unwrap_or(0) > 0 {
        format!(
            "This computer's shadow baselines: {} pairs, routed cost {:.2}× raw, time {:.2}×.",
            shadow["pairs"],
            shadow["cost_ratio"]["point"].as_f64().unwrap_or(f64::NAN),
            shadow["time_ratio"]["point"].as_f64().unwrap_or(f64::NAN),
        )
    } else {
        "No shadow baselines on this computer yet (openagents settings set coder.shadow 10).".into()
    });
    body.push(format!(
        "This computer's routed runs: {} settled, {} with no independent check.",
        report["runs"]["runs"], report["runs"]["unchecked"]
    ));
    body.extend(
        efficiency::LOCAL_EVIDENCE_NOTES
            .iter()
            .map(|line| (*line).to_owned()),
    );
    body.push("Every figure and interval: openagents efficiency --all. Method: openagents.com/efficiency.".into());
    openagents_terminal::Efficiency {
        title: latest.map_or_else(
            || "Efficiency".into(),
            |s| {
                format!(
                    "Efficiency · study {} · {} runs, 95% intervals",
                    s["name"].as_str().unwrap_or(""),
                    s["runs"]
                )
            },
        ),
        rows,
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_shows_the_latest_studys_arms_and_findings() {
        let report = efficiency::report(&efficiency::studies(&[]), &[], &[]);
        let card = card(&report);
        for note in efficiency::LOCAL_EVIDENCE_NOTES {
            assert!(card.body.iter().any(|line| line == note));
        }
        assert!(
            card.title.starts_with("Efficiency · study "),
            "{}",
            card.title
        );
        assert!(
            card.rows.iter().any(|(arm, _)| arm == "raw Claude Code"),
            "{:?}",
            card.rows
        );
        assert!(
            card.body
                .iter()
                .any(|line| line.contains("against raw Claude Code")),
            "{:?}",
            card.body
        );
    }
}
