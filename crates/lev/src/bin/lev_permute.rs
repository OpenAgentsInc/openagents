//! Measures how much option order moves the answer.
//!
//! Order sensitivity is the quality gap that neither accuracy nor calibration
//! shows. A model that answers differently when the options are listed in a
//! different order is not reading the state; it is reading the list. Kev
//! reports 7.4% argmax flips across four permutations, hosted Jev showed none
//! in published probes, and Lev's base flipped one greedy answer in eight on
//! a small hand-written set.
//!
//! This runs it properly: every Choice item in the suite, forward and
//! reversed, greedy so the result is deterministic and one call per pass.
//!
//! ```text
//! cargo run -p lev --features serve --bin lev-permute -- \
//!     --door lev-base --adapter ""      # base
//! ```

use indexmap::IndexMap;
use lev::api::{Extensions, Question, SystemOneRequest};
use lev::bridge::{Bridge, Call, Sampling};
use lev::schema::compile;
use lev::suite::Suite;

const SUITE: &str = include_str!("../../suites/support-v2.json");

fn answer(bridge: &mut Bridge, question: &Question, state: &serde_json::Value, adapter: Option<&str>) -> Option<String> {
    let request = SystemOneRequest {
        state: state.clone(),
        model: None,
        questions: [("q".to_string(), question.clone())].into_iter().collect(),
        extensions: Extensions::default(),
    };
    let compiled = compile(&request).ok()?;
    let call = Call::decide(&compiled["q"], Sampling::Greedy);
    let call = match adapter {
        Some(path) => call.with_adapter(path),
        None => call,
    };
    bridge.decide(&call).ok()?.choice
}

fn main() {
    let mut adapter: Option<String> = None;
    let mut label = "lev-base".to_string();
    // Default to the evaluation split. Running over every item mixes in the
    // records the adapter trained on, which inflates accuracy and says
    // nothing about generalisation.
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

    let mut flips = 0_usize;
    let mut trials = 0_usize;
    let mut correct_forward = 0_usize;
    let mut correct_reversed = 0_usize;

    for item in &suite.items {
        if split != "all" && item.split != split {
            continue;
        }
        // Only a Choice has an order to permute. A Noul's two options and a
        // Score's ordered levels both carry meaning in their order.
        let Question::Choice { instructions, criteria } = &item.question else {
            continue;
        };
        let reversed: IndexMap<String, Option<serde_json::Value>> =
            criteria.iter().rev().map(|(k, v)| (k.clone(), v.clone())).collect();

        let forward = Question::Choice { instructions: instructions.clone(), criteria: criteria.clone() };
        let backward = Question::Choice { instructions: instructions.clone(), criteria: reversed };

        let Some(a) = answer(&mut bridge, &forward, &item.state, adapter.as_deref()) else { continue };
        let Some(b) = answer(&mut bridge, &backward, &item.state, adapter.as_deref()) else { continue };

        trials += 1;
        if a != b {
            flips += 1;
        }
        if a == item.truth {
            correct_forward += 1;
        }
        if b == item.truth {
            correct_reversed += 1;
        }
    }

    let rate = flips as f64 / trials.max(1) as f64;
    println!("| Door | Split | Items | Flips | Flip rate | Accuracy forward | Accuracy reversed |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    println!(
        "| `{label}` | {split} | {trials} | {flips} | {rate:.3} | {:.2} | {:.2} |",
        correct_forward as f64 / trials.max(1) as f64,
        correct_reversed as f64 / trials.max(1) as f64
    );
}
