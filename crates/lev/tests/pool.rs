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
    let serial = l2(&mut single, &question, n, 0).expect("the serial estimate ran");
    let serial_ms = start.elapsed().as_millis();
    drop(single);

    let pool = Pool::discover(4).expect("four helpers start");
    let start = std::time::Instant::now();
    let parallel = l2_pool(&pool, &question, n, 0).expect("the pooled estimate ran");
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

/// A seed block is the only fresh trial this runtime can offer.
///
/// Seeds reproduce exactly, so rerunning block 0 is not a second trial; it
/// is the first one again. This checks both halves of that: a block replays
/// across a fresh pool of helper processes, and a different block draws a
/// disjoint seed set, which is what makes it independent evidence.
#[test]
fn a_seed_block_replays_across_processes_and_a_second_block_is_disjoint() {
    let Ok(mut probe) = Bridge::discover() else {
        eprintln!("skipping: no lev-bridge helper");
        return;
    };
    match probe.availability() {
        Ok(availability) if availability.is_available() => {}
        other => {
            eprintln!("skipping: the model is not available ({other:?})");
            return;
        }
    }
    drop(probe);

    let question = compiled();
    let n = 8;

    let first = Pool::discover(4).expect("four helpers start");
    let block_zero = l2_pool(&first, &question, n, 0).expect("block 0 ran");
    let block_one = l2_pool(&first, &question, n, 1).expect("block 1 ran");
    drop(first);

    // A fresh pool is a fresh set of helper processes, which is the property
    // an estimate's reproducibility claim rests on.
    let second = Pool::discover(4).expect("four more helpers start");
    let replayed = l2_pool(&second, &question, n, 1).expect("block 1 ran again");

    assert_eq!(block_zero.seeds, (0..n).collect::<Vec<u64>>());
    assert_eq!(block_one.seeds, (n..2 * n).collect::<Vec<u64>>());
    assert_eq!(block_one.seed_base, 1);
    assert!(
        block_zero.seeds.iter().all(|seed| !block_one.seeds.contains(seed)),
        "the blocks share a seed: {:?} against {:?}",
        block_zero.seeds,
        block_one.seeds
    );
    assert_eq!(
        block_one.frequency, replayed.frequency,
        "block 1 did not replay across processes: {:?} against {:?}",
        block_one.frequency, replayed.frequency
    );
    assert_eq!(block_one.choice, replayed.choice);

    eprintln!(
        "block 0 {:?} at {:.3}, block 1 {:?} at {:.3}",
        block_zero.choice,
        block_zero.top(),
        block_one.choice,
        block_one.top()
    );
}
