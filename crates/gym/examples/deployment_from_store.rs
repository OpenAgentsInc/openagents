//! Re-read complete open-partition passes through a deployment gate.
//!
//! Usage: deployment_from_store CONFIG.json
//! The config names the suite, gate, workload budget, and an ordered list of
//! doors. The first door is the regression baseline. Each door names its store
//! and explicitly supplies its cost, or null when no current price is known.
use std::error::Error;
use std::path::Path;

use gym::coverage::{Coverage, Expected};
use gym::gate::{Budget, Cost, Deployment, Gate, Profile};
use gym::row::Row;
use gym::store::Store;
use gym::suite::{Partition, Suite};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Config {
    suite: String,
    gate: String,
    budget: Budget,
    doors: Vec<Door>,
}

#[derive(Deserialize)]
struct Door {
    name: String,
    store: String,
    cost: Option<Cost>,
}

fn profile(rows: &[Row], suite: &Suite, door: &Door) -> Result<Profile, Box<dyn Error>> {
    // The same expected-selection and coverage accounting `gym report`
    // renders: here a gap is an error rather than a table row, because a
    // deployment verdict exists only over a complete comparable pass.
    let expected = Expected::of(
        suite,
        &[Partition::Calibration, Partition::Development],
        None,
        None,
        vec![],
    )?;
    let first = rows.first().ok_or("door has no rows")?;
    let mut latencies = Vec::new();
    for row in rows {
        if row.suite_digest != suite.digest
            || row.question_digest.is_none()
            || row.question_digest != first.question_digest
            || row.door_identity != first.door_identity
            || row.estimator != first.estimator
            || row.samples != first.samples
            || row.seed_base != first.seed_base
        {
            return Err("mixed or missing workload or door identity".into());
        }
        if row.answered == row.refusal.is_some() {
            return Err("row is neither an answer nor a refusal, or claims both".into());
        }
        let latency = row
            .latency_ms
            .ok_or("untimed row: refusing to shrink the denominator")?;
        if !latency.is_finite() || latency < 0.0 {
            return Err("invalid latency".into());
        }
        latencies.push(latency);
    }
    let coverage = Coverage::of(rows, expected.items());
    if !coverage.duplicates.is_empty() {
        return Err("repeated item: select a store with exactly one pass per door".into());
    }
    if !coverage.complete() {
        return Err("pass does not contain exactly the suite's open items".into());
    }
    let mut profile = Profile::timed(&latencies)
        .refusing(rows.iter().filter(|row| row.refusal.is_some()).count());
    profile.cost = door.cost;
    Ok(profile)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        return Err("usage: deployment_from_store CONFIG.json".into());
    }
    let config: Config = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let suite = Suite::load_file(&config.suite)?;
    let gate = Gate::load(Path::new(&config.gate))?;
    let mut held = Vec::new();
    for door in &config.doors {
        let rows = Store::at(&door.store)
            .verified_rows()?
            .into_iter()
            .map(serde_json::from_value::<Row>)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|row| row.door == door.name && row.permutation.is_none())
            .collect::<Vec<_>>();
        held.push((door, profile(&rows, &suite, door)?, rows));
    }
    let (baseline, baseline_profile, baseline_rows) = held.first().ok_or("no doors configured")?;
    let baseline_profile = *baseline_profile;
    let baseline_name = baseline.name.clone();
    let question_digest = baseline_rows[0].question_digest.clone();
    if held
        .iter()
        .any(|(_, _, rows)| rows[0].question_digest != question_digest)
    {
        return Err("doors were measured with different questions".into());
    }
    for (door, profile, rows) in &held {
        let comparison =
            Deployment::new(&door.name, baseline_profile, *profile).under(config.budget.clone());
        println!(
            "{}",
            json!({
                "door": door.name, "store": door.store, "rows": rows.len(),
                "suite_digest": suite.digest, "question_digest": question_digest,
                "door_identity": rows[0].door_identity,
                "gate": gate.id, "gate_digest": gate.digest(),
                "baseline_door": baseline_name, "profile": profile,
                "comparison": comparison, "outcome": gate.judge_deployment(&comparison),
            })
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<Row>, Suite, Door) {
        let suite = gym::suite::support_v2_three_way().unwrap();
        let door = Door {
            name: "kev-0.5b".into(),
            store: "unused".into(),
            cost: Some(Cost::UnmeteredLocalLane),
        };
        let rows = include_str!("../results/support-v2-three-way-quiet.jsonl")
            .lines()
            .map(|line| serde_json::from_str::<Row>(line).unwrap())
            .filter(|row| row.door == door.name)
            .collect();
        (rows, suite, door)
    }

    #[test]
    fn missing_duplicate_and_untimed_rows_cannot_shrink_the_denominator() {
        let (mut rows, suite, door) = fixture();
        assert_eq!(profile(&rows, &suite, &door).unwrap().calls, 157);
        let last = rows.pop().unwrap();
        assert!(profile(&rows, &suite, &door).is_err());
        rows.push(last);
        rows.push(rows[0].clone());
        assert!(profile(&rows, &suite, &door).is_err());
        rows.pop();
        rows[0].latency_ms = None;
        assert!(profile(&rows, &suite, &door).is_err());
    }

    #[test]
    fn refusals_count_and_mixed_identity_is_rejected() {
        let (mut rows, suite, door) = fixture();
        rows[0].answered = false;
        rows[0].refusal = Some(gym::row::RefusalCode::Busy);
        let measured = profile(&rows, &suite, &door).unwrap();
        assert_eq!(measured.calls, 157);
        assert_eq!(measured.refusals, Some(1));
        assert_eq!(measured.cost, Some(Cost::UnmeteredLocalLane));
        rows[0].door_identity.model = "different".into();
        assert!(profile(&rows, &suite, &door).is_err());
    }
}
