//! `icecream-live measure`: times each kind of change from the moment it is
//! saved to the moment a page already playing in the zone applies it.
//!
//! Run it under `openagents browser run` with the host serving
//! (`icecream-live serve`). It opens `--pages` pages where the game's bot
//! plays, waits for their first play, then for each of `--runs` rounds:
//!
//! 1. **tuning**: the Ice Cream Man 20% slower and two more hiding spots
//!    (`zone/tuning.json`, Game Lab's names);
//! 2. **logic**: seven sprinkles a volley instead of five, a change to the
//!    rules' code (`VOLLEY_PELLETS`), so the logic module rebuilds;
//! 3. **asset**: the Ice Cream Man's forehead in mint (`zone/palette.json`),
//!    which changes one chunk;
//! 4. **broken**: a logic module that fails on its first step, then
//!    **rollback** to the one before it;
//! 5. **chat** (with `--lab URL`): Game Lab's `game_edit`, the tool call a
//!    chat makes for "make him slower", with Game Lab's on-save hook set to
//!    `icecream-live lab`; timed from the call to the page's ack, so it
//!    includes Game Lab's own bot check of the edit.
//!
//! Each page acks what it applied with the bytes it fetched and whether the
//! run carried on. The sources are put back, and the zone deployed as it
//! was, at the end. It writes every line to `--out` and prints a summary.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::{Args, DeployArgs, cdp, deploy_once, epoch_ms, host, rollback_once, root, runtime};

/// A source file the cases edit, and its original text.
struct Source {
    path: PathBuf,
    original: String,
}

impl Source {
    fn new(path: PathBuf) -> Result<Self> {
        let original = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        Ok(Self { path, original })
    }

    fn write(&self, text: &str) -> Result<()> {
        Ok(std::fs::write(&self.path, text)?)
    }

    fn restore(&self) {
        let _ = std::fs::write(&self.path, &self.original);
    }
}

struct Sources {
    tuning: Source,
    palette: Source,
    rules: Source,
    logic: Source,
}

impl Sources {
    fn open() -> Result<Self> {
        let zone = Path::new(env!("CARGO_MANIFEST_DIR")).join("zone");
        Ok(Self {
            tuning: Source::new(zone.join("tuning.json"))?,
            palette: Source::new(zone.join("palette.json"))?,
            rules: Source::new(root().join("crates/icecream-rules/src/tuning.rs"))?,
            logic: Source::new(root().join("crates/icecream-logic/src/lib.rs"))?,
        })
    }

    fn restore(&self) {
        for source in [&self.tuning, &self.palette, &self.rules, &self.logic] {
            source.restore();
        }
    }
}

/// Makes the edit for `case`.
fn edit(sources: &Sources, case: &str) -> Result<()> {
    match case {
        "tuning" => {
            let base = icecream_rules::Tuning::default();
            let slower = json!({
                "man_wander": f64::from(base.man_wander) * 0.8,
                "man_investigate": f64::from(base.man_investigate) * 0.8,
                "man_search": f64::from(base.man_search) * 0.8,
                "man_chase": f64::from(base.man_chase) * 0.8,
                "extra_closets": base.extra_closets + 2,
            });
            sources
                .tuning
                .write(&serde_json::to_string_pretty(&slower)?)
        }
        "logic" => {
            let from = "pub const VOLLEY_PELLETS: usize = 5;";
            if !sources.rules.original.contains(from) {
                bail!("the rules no longer say {from}");
            }
            sources.rules.write(
                &sources
                    .rules
                    .original
                    .replace(from, "pub const VOLLEY_PELLETS: usize = 7;"),
            )
        }
        "asset" => sources.palette.write(r#"{"man": {"FFE4D0": "8FD3B4"}}"#),
        "broken" => {
            let from = "\"step\" => {";
            if !sources.logic.original.contains(from) {
                bail!("the logic module no longer has its step operation");
            }
            sources.logic.write(&sources.logic.original.replace(
                from,
                "\"step\" => {\n            if !args.is_empty() {\n                panic!(\"a broken build\");\n            }",
            ))
        }
        other => bail!("no case {other}"),
    }
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

pub fn run(mut args: Args) -> Result<()> {
    let work = args.work();
    let host = args.host();
    let runs: usize = args.value("--runs").map_or(Ok(1), |v| v.parse())?;
    let pages: usize = args.value("--pages").map_or(Ok(1), |v| v.parse())?;
    let out = args
        .value("--out")
        .map_or_else(|| work.dir.join("measure.json"), PathBuf::from);
    let lab = args.value("--lab");
    let port = cdp::port().context("run this under `openagents browser run`")?;
    let sources = Sources::open()?;
    let rt = runtime()?;
    let result = rt.block_on(measure(
        &work,
        &host,
        &sources,
        port,
        runs,
        pages,
        lab.as_deref(),
    ));
    sources.restore();
    // Put the zone back as it was.
    let base = DeployArgs {
        channel: "live".into(),
        force: false,
        wait: None,
    };
    rt.block_on(deploy_once(&work, &host, &base, epoch_ms()))?;
    let lines = result?;
    std::fs::write(&out, serde_json::to_vec_pretty(&lines)?)?;
    println!("{}", summary(&lines));
    eprintln!("icecream-live: every line is in {}", out.display());
    Ok(())
}

async fn measure(
    work: &host::Work,
    host: &str,
    sources: &Sources,
    port: u16,
    runs: usize,
    pages: usize,
    lab: Option<&str>,
) -> Result<Vec<Value>> {
    sources.restore();
    let wait = Duration::from_secs(240);
    let base = DeployArgs {
        channel: "live".into(),
        force: false,
        wait: None,
    };
    deploy_once(work, host, &base, epoch_ms()).await?;
    let mut lines = Vec::new();

    let mut acks = host::Acks::open(host).await?;
    let mut tabs = Vec::new();
    for _ in 0..pages {
        tabs.push(
            cdp::Page::open(port, &format!("{host}/t/{}/#bot", icecream_zone::TENANT)).await?,
        );
    }
    for _ in 0..pages {
        let first = acks
            .wait(
                &json!({ "tenant": icecream_zone::TENANT, "kind": "first_play" }),
                wait,
            )
            .await?;
        lines.push(json!({ "case": "first_play", "ack": first }));
    }
    // Let the bots play a little first.
    tokio::time::sleep(Duration::from_secs(3)).await;

    let with_wait = DeployArgs {
        channel: "live".into(),
        force: false,
        wait: Some((wait, pages)),
    };
    for run in 0..runs {
        // Each run starts from the zone as it is checked in; the edits then
        // pile up as a person's would, so each case builds only what it
        // changed.
        sources.restore();
        deploy_once(work, host, &base, epoch_ms()).await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        for case in ["tuning", "logic", "asset", "broken"] {
            let started = epoch_ms();
            edit(sources, case)?;
            let mut line = deploy_once(work, host, &with_wait, started).await?;
            line["case"] = case.into();
            line["run"] = run.into();
            eprintln!(
                "icecream-live: {case} live in {} ms",
                line["edit_to_live_ms"]
            );
            lines.push(line);
            if case == "broken" {
                // The page stops on the failed step; let it sit stopped for
                // a moment, then roll back.
                tokio::time::sleep(Duration::from_millis(1500)).await;
                let mut line = rollback_once(
                    work,
                    host,
                    icecream_zone::TENANT,
                    "live",
                    Some((wait, pages)),
                )
                .await?;
                line["case"] = "rollback".into();
                line["run"] = run.into();
                eprintln!(
                    "icecream-live: rollback live in {} ms",
                    line["edit_to_live_ms"]
                );
                lines.push(line);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        if let Some(lab) = lab {
            let line = chat(lab, host, run, (wait, pages)).await?;
            eprintln!("icecream-live: chat live in {} ms", line["edit_to_live_ms"]);
            lines.push(line);
        }
    }
    sources.restore();
    drop(tabs);
    Ok(lines)
}

/// One chat-style edit through Game Lab, timed to the page's ack.
async fn chat(lab: &str, host: &str, run: usize, wait: (Duration, usize)) -> Result<Value> {
    let client = reqwest::Client::new();
    // A speed the latest version doesn't have, so the edit is a change.
    let open: Value = client
        .post(format!("{lab}/api/tool/game_open"))
        .json(&json!({ "game": "the-ice-cream-man" }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let now = open["document"]["man"]["man_chase"].as_f64().unwrap_or(5.6);
    let chase = if now > 4.5 { now - 0.4 } else { now + 0.4 };
    let chase = (chase * 10.0).round() / 10.0;
    let mut acks = host::Acks::open(host).await?;
    let started = epoch_ms();
    let call = json!({
        "game": "the-ice-cream-man",
        "patch": json!([
            {"op": "replace", "path": "/man/man_chase", "value": chase},
            {"op": "replace", "path": "/layout/extra_closets", "value": 4},
        ])
        .to_string(),
        "summary": format!("He chases at {chase} m/s; two more hiding spots"),
        "author": "icecream-live measure",
    });
    let edited: Value = client
        .post(format!("{lab}/api/tool/game_edit"))
        .json(&call)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let answered = epoch_ms();
    if edited["saved"] != true {
        bail!("Game Lab refused the edit: {edited}");
    }
    let published = &edited["published"];
    let manifest = published["manifest"]
        .as_str()
        .with_context(|| format!("Game Lab's hook didn't publish: {published}"))?
        .to_owned();
    let mut line = json!({
        "case": "chat",
        "run": run,
        "game_lab_version": edited["version"],
        "game_lab_ms": ((answered - started) * 10.0).round() / 10.0,
        "build_ms": published["build_ms"],
        "bake_ms": published["bake_ms"],
        "publish_ms": published["publish_ms"],
        "manifest": manifest,
    });
    crate::add_acks(
        &mut acks,
        &mut line,
        (icecream_zone::TENANT, &manifest),
        started,
        wait,
    )
    .await?;
    Ok(line)
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// The table the demo doc records.
#[must_use]
pub fn summary(lines: &[Value]) -> String {
    let mut out = String::new();
    let first: Vec<&Value> = lines.iter().filter(|l| l["case"] == "first_play").collect();
    let full_bytes = first.first().map_or(0.0, |l| number(&l["ack"]["bytes"]));
    for line in &first {
        let ack = &line["ack"];
        out.push_str(&format!(
            "first play: {} ms (budget {} ms), {:.0} KB in {} files, {} of {} rooms\n",
            ack["first_play_ms"],
            ack["budget_ms"],
            number(&ack["bytes"]) / 1024.0,
            ack["files"],
            ack["rooms_loaded"],
            ack["rooms"],
        ));
    }
    out.push_str(
        "case      edit-to-live (min-max)      build    bake  publish  fetched new  again  new vs reload  same run\n",
    );
    for case in ["tuning", "logic", "asset", "broken", "rollback", "chat"] {
        let rows: Vec<&Value> = lines.iter().filter(|l| l["case"] == case).collect();
        if rows.is_empty() {
            continue;
        }
        let all = |f: &dyn Fn(&Value) -> f64| rows.iter().map(|r| f(r)).collect::<Vec<f64>>();
        let pick = |f: &dyn Fn(&Value) -> f64| median(all(f));
        let live = all(&|r| number(&r["edit_to_live_ms"]));
        let low = live.iter().copied().fold(f64::INFINITY, f64::min);
        let high = live.iter().copied().fold(0.0, f64::max);
        // The first run sees each version new; later runs publish the same
        // hashes again, which the page already holds.
        let new = rows.first().map_or(0.0, |r| number(&r["acks"][0]["bytes"]));
        let again = median(
            rows.iter()
                .skip(1)
                .map(|r| number(&r["acks"][0]["bytes"]))
                .collect(),
        );
        let same = rows.iter().all(|r| {
            r["acks"]
                .as_array()
                .is_some_and(|a| a.iter().all(|k| k["same_run"] == true))
        });
        out.push_str(&format!(
            "{case:<9} {:>7.0} ms ({:>5.0}-{:<5.0}) {:>7.0} ms {:>4.0} ms {:>5.0} ms {:>8.1} KB {:>5.1} KB {:>9.1}%  {}\n",
            median(live),
            low,
            high,
            pick(&|r| number(&r["build_ms"])),
            pick(&|r| number(&r["bake_ms"])),
            pick(&|r| number(&r["publish_ms"])),
            new / 1024.0,
            again / 1024.0,
            if full_bytes > 0.0 { new / full_bytes * 100.0 } else { 0.0 },
            if same { "yes" } else { "no" },
        ));
    }
    let rollbacks: Vec<&Value> = lines.iter().filter(|l| l["case"] == "rollback").collect();
    if !rollbacks.is_empty() {
        let resumed = rollbacks.iter().all(|r| {
            r["acks"].as_array().is_some_and(|a| {
                a.iter()
                    .all(|k| k["was_paused"] == true && k["paused"] == false)
            })
        });
        out.push_str(&format!(
            "the broken build paused the run and the rollback resumed it, same run: {}\n",
            if resumed { "yes" } else { "no" }
        ));
    }
    out.push_str(&format!(
        "(medians of {} runs; \"vs reload\" is the bytes against a fresh page's first play, {:.0} KB)\n",
        lines.iter().filter(|l| l["case"] == "tuning").count(),
        full_bytes / 1024.0
    ));
    out
}
