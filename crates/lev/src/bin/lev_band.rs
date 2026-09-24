//! Tests whether the certainty band carries any signal.
//!
//! On the base model it does not: every item comes back `likely`, including
//! the wrong ones, which is why L3 has never had anything to calibrate. The
//! question here is whether an adapter trained against measured outcomes
//! changes that.
//!
//! A band is useful if items in a low band are wrong more often than items
//! in a high band. That is the whole test, and it is stricter than "the
//! field varies" — a field that varies at random also varies.
//!
//! ```text
//! cargo run -p lev --features serve --bin lev-band -- \
//!     --label lev-band --adapter ~/code/lev-adapter-work/runs/lev-band/levband.fmadapter
//! ```

use std::collections::{BTreeMap, BTreeSet};

use gym::calibrate::{Map, Observation, score};
use gym::gate;
use lev::api::{Extensions, SystemOneRequest};
use lev::bridge::{Bridge, Call, Pool, Sampling};
use lev::schema::{BANDS, compile};
use lev::suite::Suite;

const SUITE: &str = include_str!("../../suites/support-v2.json");

/// The items `support-v2-three-way` holds back, which this probe must not
/// read.
///
/// `support-v2` and `support-v2-three-way` are the same 196 items under two
/// partitionings. A lock is a property of the item, not of the file that
/// names it, so a probe reading the older suite spends the newer suite's
/// locked partition without ever passing the flag that would have refused.
/// Thirty-nine of these items are locked; this probe skips them and says how
/// many it skipped.
fn locked_items() -> BTreeSet<String> {
    let Ok(three_way) = gym::suite::support_v2_three_way() else {
        return BTreeSet::new();
    };
    three_way
        .items
        .iter()
        .filter(|item| item.partition == gym::suite::Partition::Locked)
        .map(|item| item.id.clone())
        .collect()
}

fn main() {
    let mut adapter: Option<String> = None;
    let mut label = "lev-base".to_string();
    let mut split = "evaluation".to_string();
    let mut calibrate = false;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--adapter" => adapter = args.next().filter(|value| !value.is_empty()),
            "--label" => label = args.next().unwrap_or(label),
            "--split" => split = args.next().unwrap_or(split),
            "--calibrate" => calibrate = true,
            other => eprintln!("lev-band: unknown flag `{other}`"),
        }
    }

    let suite = Suite::load(SUITE).expect("the shipped suite loads");
    let locked = locked_items();
    let mut bridge = Bridge::discover().expect("a helper starts");
    let bands: Vec<String> = BANDS.iter().map(|band| (*band).to_string()).collect();

    // correct, total, per band
    let mut tally: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut answered = 0_usize;
    let mut correct = 0_usize;
    let mut held_back = 0_usize;

    for item in &suite.items {
        if split != "all" && item.split != split {
            continue;
        }
        if locked.contains(&item.id) {
            held_back += 1;
            continue;
        }
        let request = SystemOneRequest {
            state: item.state.clone(),
            model: None,
            questions: [("q".to_string(), item.question.clone())]
                .into_iter()
                .collect(),
            extensions: Extensions::default(),
        };
        let Ok(compiled) = compile(&request) else {
            continue;
        };
        let call = Call::decide(&compiled["q"], Sampling::Greedy).with_band(bands.clone());
        let call = match adapter.as_deref() {
            Some(path) => call.with_adapter(path),
            None => call,
        };
        let Ok(outcome) = bridge.decide(&call) else {
            continue;
        };
        let Some(choice) = outcome.choice else {
            continue;
        };
        let band = outcome.band.unwrap_or_else(|| "<none>".to_string());
        let right = choice == item.truth;
        answered += 1;
        if right {
            correct += 1;
        }
        let slot = tally.entry(band).or_insert((0, 0));
        slot.1 += 1;
        if right {
            slot.0 += 1;
        }
    }

    println!("## {label} ({split} split)\n");
    println!(
        "Answered {answered}, correct {correct}, accuracy {:.2}.\n",
        correct as f64 / answered.max(1) as f64
    );
    println!(
        "{held_back} item(s) of this split are locked by `support-v2-three-way` and were not \
         read.\n"
    );
    println!("| Band | Items | Correct | Accuracy |");
    println!("| --- | --- | --- | --- |");
    // Report in the band's own order, not alphabetically.
    for band in BANDS {
        if let Some((right, total)) = tally.get(band) {
            println!(
                "| `{band}` | {total} | {right} | {:.2} |",
                *right as f64 / (*total).max(1) as f64
            );
        }
    }
    for (band, (right, total)) in &tally {
        if !BANDS.contains(&band.as_str()) {
            println!(
                "| `{band}` | {total} | {right} | {:.2} |",
                *right as f64 / (*total).max(1) as f64
            );
        }
    }

    let distinct = tally.len();
    println!(
        "\n{distinct} distinct band{} used.",
        if distinct == 1 { "" } else { "s" }
    );

    if !calibrate {
        return;
    }

    // Does conditioning the calibration map on the band beat pooling?
    //
    // The frequency alone says how consistently the model answered. The band
    // says how reliable an answer like this is. If the band carries signal,
    // a table fitted per band should beat one fitted over everything — and
    // if it does not, the band is decoration.
    println!("\n## Band-conditioned calibration\n");
    println!(
        "Fitted on the `support-v2` calibration split and scored on its evaluation split, both \
         with the {} items `support-v2-three-way` locks left out.\n",
        locked.len()
    );
    let pool = Pool::discover(4).expect("a pool starts");
    let mut rows: Vec<(String, Observation)> = Vec::new();

    for item in &suite.items {
        if locked.contains(&item.id) {
            continue;
        }
        let request = SystemOneRequest {
            state: item.state.clone(),
            model: None,
            questions: [("q".to_string(), item.question.clone())]
                .into_iter()
                .collect(),
            extensions: Extensions::default(),
        };
        let Ok(compiled) = compile(&request) else {
            continue;
        };
        let Ok(raw) = l2_pool_adapted(&pool, &compiled["q"], 8, 0, adapter.as_deref()) else {
            continue;
        };
        // One extra greedy call for the band.
        let call = Call::decide(&compiled["q"], Sampling::Greedy).with_band(bands.clone());
        let call = match adapter.as_deref() {
            Some(path) => call.with_adapter(path),
            None => call,
        };
        let band = bridge.decide(&call).ok().and_then(|outcome| outcome.band);
        let correct = raw.choice == item.truth;
        let observation = match band {
            Some(band) => Observation::banded(raw.top(), correct, band),
            None => Observation::new(raw.top(), correct),
        };
        rows.push((item.split.clone(), observation));
    }

    let fit_on: Vec<Observation> = rows
        .iter()
        .filter(|(s, _)| s == "calibration")
        .map(|(_, o)| o.clone())
        .collect();
    let held: Vec<(String, Observation)> = rows
        .iter()
        .filter(|(s, _)| s == "evaluation")
        .cloned()
        .collect();

    let pooled_map = Map::fit_auto(&fit_on);
    let banded_map = Map::fit_banded(&fit_on);

    let raw_scores: Vec<Observation> = held.iter().map(|(_, o)| o.clone()).collect();
    let pooled: Vec<Observation> = held
        .iter()
        .map(|(_, o)| Observation::new(pooled_map.apply(o.raw), o.correct))
        .collect();
    let conditioned: Vec<Observation> = held
        .iter()
        .map(|(_, o)| {
            Observation::new(banded_map.apply_banded(o.raw, o.band.as_deref()), o.correct)
        })
        .collect();

    println!("| Map | ECE | Brier | NLL | Confident errors | Items |");
    println!("| --- | --- | --- | --- | --- | --- |");
    for (name, set) in [
        ("raw", &raw_scores),
        ("pooled", &pooled),
        ("band-conditioned", &conditioned),
    ] {
        let m = score(set);
        println!(
            "| {name} | {:.3} | {:.3} | {:.3} | {} | {} |",
            m.ece, m.brier, m.nll, m.confident_errors, m.items
        );
    }

    // Judged by the committed rule rather than by constants in this file, so
    // the verdict names the rule that produced it and moves when the rule
    // does.
    let rule = gate::load("probability-v1").expect("the committed gate loads");
    let outcome = rule.judge(
        &gate::Comparison::new(
            "band-conditioned against raw",
            score(&raw_scores).scores(),
            score(&conditioned).scores(),
        )
        .fitted_on(fit_on.len()),
    );
    println!("\n### Band-conditioned map against the raw signal\n");
    println!("Judged by `{}`, digest `{}`.\n", rule.id, rule.digest());
    println!("| Criterion | Rank | Verdict | Detail |");
    println!("| --- | --- | --- | --- |");
    for criterion in &outcome.criteria {
        println!(
            "| `{}` | {} | {} | {} |",
            criterion.name, criterion.rank, criterion.verdict, criterion.detail
        );
    }
    println!("\nVerdict: {}.", outcome.verdict);
    println!(
        "\nBands with their own table: {:?}",
        banded_map.by_band.keys().collect::<Vec<_>>()
    );
}

/// `l2_pool` with an optional adapter, which the pool API takes per call.
fn l2_pool_adapted(
    pool: &Pool,
    compiled: &lev::schema::Compiled,
    n: u64,
    seed_base: u64,
    adapter: Option<&str>,
) -> lev::error::Result<lev::estimator::Raw> {
    lev::estimator::l2_pool_with(pool, compiled, n, seed_base, adapter)
}
