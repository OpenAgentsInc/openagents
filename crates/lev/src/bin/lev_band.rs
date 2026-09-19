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

use std::collections::BTreeMap;

use lev::api::{Extensions, SystemOneRequest};
use lev::bridge::{Bridge, Call, Sampling};
use lev::schema::{BANDS, compile};
use lev::suite::Suite;

const SUITE: &str = include_str!("../../suites/support-v2.json");

fn main() {
    let mut adapter: Option<String> = None;
    let mut label = "lev-base".to_string();
    let mut split = "evaluation".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--adapter" => adapter = args.next().filter(|value| !value.is_empty()),
            "--label" => label = args.next().unwrap_or(label),
            "--split" => split = args.next().unwrap_or(split),
            other => eprintln!("unknown flag {other}"),
        }
    }

    let suite = Suite::load(SUITE).expect("the shipped suite loads");
    let mut bridge = Bridge::discover().expect("a helper starts");
    let bands: Vec<String> = BANDS.iter().map(|band| (*band).to_string()).collect();

    // correct, total, per band
    let mut tally: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut answered = 0_usize;
    let mut correct = 0_usize;

    for item in &suite.items {
        if split != "all" && item.split != split {
            continue;
        }
        let request = SystemOneRequest {
            state: item.state.clone(),
            model: None,
            questions: [("q".to_string(), item.question.clone())].into_iter().collect(),
            extensions: Extensions::default(),
        };
        let Ok(compiled) = compile(&request) else { continue };
        let call = Call::decide(&compiled["q"], Sampling::Greedy).with_band(bands.clone());
        let call = match adapter.as_deref() {
            Some(path) => call.with_adapter(path),
            None => call,
        };
        let Ok(outcome) = bridge.decide(&call) else { continue };
        let Some(choice) = outcome.choice else { continue };
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
    println!("Answered {answered}, correct {correct}, accuracy {:.2}.\n", correct as f64 / answered.max(1) as f64);
    println!("| Band | Items | Correct | Accuracy |");
    println!("| --- | --- | --- | --- |");
    // Report in the band's own order, not alphabetically.
    for band in BANDS {
        if let Some((right, total)) = tally.get(band) {
            println!("| `{band}` | {total} | {right} | {:.2} |", *right as f64 / (*total).max(1) as f64);
        }
    }
    for (band, (right, total)) in &tally {
        if !BANDS.contains(&band.as_str()) {
            println!("| `{band}` | {total} | {right} | {:.2} |", *right as f64 / (*total).max(1) as f64);
        }
    }

    let distinct = tally.len();
    println!(
        "\n{distinct} distinct band{} used.",
        if distinct == 1 { "" } else { "s" }
    );
}
