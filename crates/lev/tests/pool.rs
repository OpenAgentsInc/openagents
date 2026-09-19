//! A pool changes the wall clock, not the answer.
//!
//! Sampling concurrently is only sound because sessions are independent —
//! the same property that gives question isolation. This test checks that the
//! claim holds in practice: the same seeds through one helper and through a
//! pool produce the same distribution.

#![cfg(feature = "serve")]

use indexmap::IndexMap;
use lev::api::{Extensions, Question, SystemOneRequest};
use lev::bridge::{Bridge, Pool};
use lev::estimator::{l2, l2_pool};
use lev::schema::compile;
use serde_json::json;

fn compiled() -> lev::schema::Compiled {
    let mut criteria = IndexMap::new();
    criteria.insert("billing".to_string(), Some(json!("Charges and refunds")));
    criteria.insert("technical".to_string(), Some(json!("Bugs and outages")));
    criteria.insert("sales".to_string(), Some(json!("Quotes and plans")));
    let mut questions = IndexMap::new();
    questions.insert(
        "q".to_string(),
        Question::Choice { instructions: Some(json!("Route this message.")), criteria },
    );
    let request = SystemOneRequest {
        state: json!("We added seats last month and the invoice does not match the quote."),
        model: None,
        questions,
        extensions: Extensions::default(),
    };
    compile(&request).expect("it compiles").swap_remove("q").expect("one question")
}

#[test]
fn a_pool_reproduces_a_single_helper_s_estimate_and_beats_its_clock() {
    let Ok(mut single) = Bridge::discover() else {
        eprintln!("skipping: no lev-bridge helper");
        return;
    };
    match single.availability() {
        Ok(availability) if availability.is_available() => {}
        other => {
            eprintln!("skipping: the model is not available ({other:?})");
            return;
        }
    }
    let question = compiled();
    let n = 8;

    let start = std::time::Instant::now();
    let serial = l2(&mut single, &question, n).expect("the serial estimate ran");
    let serial_ms = start.elapsed().as_millis();
    drop(single);

    let pool = Pool::discover(4).expect("four helpers start");
    let start = std::time::Instant::now();
    let parallel = l2_pool(&pool, &question, n).expect("the pooled estimate ran");
    let parallel_ms = start.elapsed().as_millis();

    eprintln!(
        "l2 over {n} seeds: serial {serial_ms} ms, pool of {} {parallel_ms} ms",
        pool.width()
    );

    assert_eq!(serial.choice, parallel.choice, "the pool changed the answer");
    assert_eq!(
        serial.frequency, parallel.frequency,
        "the pool changed the distribution: {:?} against {:?}",
        serial.frequency, parallel.frequency
    );
    assert_eq!(serial.seeds, parallel.seeds);
    assert!(
        parallel_ms < serial_ms,
        "the pool was not faster: {parallel_ms} ms against {serial_ms} ms"
    );
}
