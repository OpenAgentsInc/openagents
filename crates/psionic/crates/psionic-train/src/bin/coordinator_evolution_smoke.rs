//! CPU-only smoke for the coordinator evolution lane (Khala M6, P3–P5).
//!
//! Runs separable CMA-ES against a real `CoordinatorHead` (P2) using the P4
//! fixture-verdict atomic evaluation over a P5 capability-filtered worker pool.
//! Prints the before/after fitness and whether ES improved. This is a SMOKE —
//! the fitness is a deterministic FIXTURE verdict (route the probe to the
//! capability-eligible "correct" worker), NOT a frontier ML result.
//!
//! Run:
//!   cargo run -q -p psionic-train --bin coordinator_evolution_smoke
//!
//! It exits non-zero if ES fails to reach a verified head, so it can gate CI.

use psionic_core::Shape;
use psionic_models::{CoordinatorHead, CoordinatorHeadConfig};
use psionic_nn::NnTensor;
use psionic_train::{
    FixtureCoordinatorEval, SepCmaEs, SepCmaEsConfig, TerminalRewardAdapter, TrajectoryOutcome,
    VerificationVerdict, WorkerKind, WorkerPoolBinding, WorkerPoolMember,
};
use std::collections::BTreeSet;
use std::process::ExitCode;

fn worker(id: &str, kind: WorkerKind, caps: &[&str]) -> WorkerPoolMember {
    WorkerPoolMember {
        worker_id: id.to_string(),
        kind,
        receipted_capabilities: caps.iter().map(|c| (*c).to_string()).collect::<BTreeSet<_>>(),
    }
}

fn main() -> ExitCode {
    println!("== Coordinator evolution CPU smoke (Khala M6, P3-P5) ==");
    println!("NOTE: fixture-verdict fitness, not a frontier ML result.\n");

    // --- P5: capability-filtered worker pool -------------------------------
    let candidates = vec![
        worker("frontier-a", WorkerKind::Frontier, &["rust_build", "python"]),
        worker("open-z", WorkerKind::Open, &["rust_build"]),
        worker("open-mid", WorkerKind::Open, &["python"]), // filtered out
    ];
    let pool = match WorkerPoolBinding::from_candidates(candidates, "rust_build") {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("worker pool binding failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "P5 worker pool: {} eligible for `{}` -> {:?}",
        pool.len(),
        pool.required_capability(),
        pool.workers().iter().map(|w| w.worker_id.as_str()).collect::<Vec<_>>()
    );
    // The "correct" worker for this fixture is index 1 of the filtered pool
    // (`open-z`). A zero head ties and argmax-tie-breaks to index 0, so the
    // start FAILS verification and ES must actually move the head to verify --
    // a more honest improvement signal than starting already-correct.
    let correct_worker = 1_usize;

    // --- P2: a real coordinator head sized to the eligible pool -------------
    let config = CoordinatorHeadConfig {
        hidden_dim: 8,
        num_workers: pool.len(),
        num_roles: 3,
    };
    let seed_head = match CoordinatorHead::zeros(config) {
        Ok(head) => head,
        Err(error) => {
            eprintln!("seed head failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let dimension = config.parameter_count();
    println!(
        "P2 head: hidden_dim={} num_workers={} num_roles={} -> {} params",
        config.hidden_dim, config.num_workers, config.num_roles, dimension
    );

    // --- P4: fixture atomic evaluation (offline reward) ---------------------
    let probe_values = vec![1.0_f32, 0.6, -0.4, 0.2, 0.9, -0.1, 0.3, -0.7];
    let reward = TerminalRewardAdapter::offline();
    let probe_for_eval = probe_values.clone();
    let eval = FixtureCoordinatorEval::new(seed_head.clone(), reward, move |head| {
        let probe = NnTensor::f32(Shape::new(vec![1, 8]), probe_for_eval.clone())
            .expect("probe tensor");
        let decisions = head.decide(&probe).expect("decisions");
        let verdict = if decisions[0].worker_index == correct_worker {
            VerificationVerdict::Verified
        } else {
            VerificationVerdict::Rejected
        };
        vec![TrajectoryOutcome::offline(verdict)]
    });

    // --- P3: separable CMA-ES ----------------------------------------------
    let optimizer = match SepCmaEs::new(SepCmaEsConfig {
        dimension,
        population_size: 24,
        generations: 60,
        initial_sigma: 0.5,
        seed: 0x5A_6A_4A_4A,
    }) {
        Ok(optimizer) => optimizer,
        Err(error) => {
            eprintln!("optimizer config failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    let initial = seed_head.flatten_parameters().expect("flat params");
    let outcome = match optimizer.optimize(&eval, &initial) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("optimize failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    println!("\nP3 sep-CMA-ES result:");
    println!("  initial fitness : {:.4}", outcome.initial_fitness);
    println!("  best fitness    : {:.4}", outcome.best_fitness);
    println!("  improved        : {}", outcome.improved());
    println!("  evaluations     : {}", outcome.evaluations);

    if (outcome.best_fitness - 1.0).abs() < 1e-6 {
        println!("\nSMOKE PASS: optimizer reached a verified head (fixture reward 1.0).");
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "\nSMOKE FAIL: best fitness {} did not reach verified (1.0).",
            outcome.best_fitness
        );
        ExitCode::FAILURE
    }
}
