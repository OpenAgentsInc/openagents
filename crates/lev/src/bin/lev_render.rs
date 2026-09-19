//! Dumps what every suite item compiles to, as JSON Lines.
//!
//! Training and serving must share one renderer. The training harness is
//! Python, because Apple's toolkit is, so there are two implementations of
//! the same rendering rules and they can drift. This binary is the reference
//! side; `training/lev-adapter/check_parity.py` reads it and fails if the two
//! disagree on a single character.

use lev::api::{Extensions, SystemOneRequest};
use lev::schema::compile;
use lev::suite::Suite;

const SUITE: &str = include_str!("../../suites/support-v1.json");

fn main() {
    let suite = Suite::load(SUITE).expect("the shipped suite loads");
    for item in &suite.items {
        let request = SystemOneRequest {
            state: item.state.clone(),
            model: None,
            questions: [(item.id.clone(), item.question.clone())].into_iter().collect(),
            extensions: Extensions::default(),
        };
        let compiled = compile(&request).expect("the item compiles");
        let one = &compiled[&item.id];
        let line = serde_json::json!({
            "id": item.id,
            "instructions": one.instructions,
            "prompt": one.prompt,
            "options": one.options,
        });
        println!("{line}");
    }
}
