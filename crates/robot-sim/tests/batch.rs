#![cfg(feature = "mujoco")]
//! Batched environments (#11659): environment `i` of a threaded batch of 64
//! is the same trajectory as a batch of one built from its seed.

use std::path::{Path, PathBuf};

use robot_sim::mujoco::batch::{self, Batch, Randomize, Settings};
use robot_sim::mujoco::{LibraryPin, Runtime};
use sha2::{Digest, Sha256};

fn library() -> LibraryPin {
    let path = PathBuf::from(
        std::env::var_os("ROBOT_LAB_MUJOCO_LIBRARY")
            .expect("Set ROBOT_LAB_MUJOCO_LIBRARY to the qualified MuJoCo 3.3.7 library"),
    );
    let bytes = std::fs::read(&path).unwrap();
    LibraryPin {
        path,
        sha256: robot_lab::store::digest(&bytes),
    }
}

/// A smooth per-environment target sweep inside the joint limits.
fn actions(batch: &Batch, step: usize, seeds: &[u64]) -> Vec<f64> {
    let home = batch.home();
    let limits = batch.limits();
    let t = step as f64 / 240.0;
    let mut out = Vec::new();
    for seed in seeds {
        let phase = (seed % 1000) as f64 * 0.006_283;
        for (j, (h, l)) in home.iter().zip(&limits).enumerate() {
            let v = h + 0.5 * (2.0 * t + phase + j as f64).sin();
            out.push(l.map_or(v, |(lo, hi)| v.clamp(lo, hi)));
        }
    }
    out
}

#[test]
#[ignore = "requires the configured MuJoCo 3.3.7 library"]
fn each_env_of_a_threaded_batch_of_64_matches_its_single_env_run() {
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&library(), scratch.path()).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let model = batch::so101_table(&runtime, &source).unwrap();
    let settings = Settings {
        threads: 8,
        randomize: Some(Randomize {
            mass: 0.2,
            friction: 0.3,
            gain: 0.2,
        }),
    };
    let steps = 240;
    let mut big = Batch::new(&model, 64, 11_659, settings).unwrap();
    let seeds: Vec<u64> = (0..64).map(|i| big.seed(i)).collect();
    let width = big.observation_width();
    let mut traces = vec![Sha256::new(); 64];
    let mut obs = vec![0.0; 64 * width];
    let mut contacts = 0;
    for step in 0..steps {
        big.step(&actions(&big, step, &seeds)).unwrap();
        big.observe(&mut obs).unwrap();
        for (trace, row) in traces.iter_mut().zip(obs.chunks(width)) {
            for v in row {
                trace.update(v.to_bits().to_le_bytes());
            }
        }
        contacts += (0..64).map(|e| big.contacts(e)).sum::<usize>();
    }
    assert!(
        contacts > 64 * steps,
        "the scene is contact-rich: {contacts}"
    );
    let masses: Vec<f64> = (0..64).map(|e| big.total_mass(e)).collect();
    assert!(
        masses.windows(2).any(|w| w[0] != w[1]),
        "masses are randomised"
    );
    let big: Vec<_> = traces.into_iter().map(|t| t.finalize()).collect();
    for (i, seed) in seeds.iter().enumerate() {
        let mut one = Batch::with_seeds(
            &model,
            &[*seed],
            Settings {
                threads: 1,
                ..settings
            },
        )
        .unwrap();
        let mut trace = Sha256::new();
        let mut row = vec![0.0; width];
        for step in 0..steps {
            one.step(&actions(&one, step, &[*seed])).unwrap();
            one.observe(&mut row).unwrap();
            for v in &row {
                trace.update(v.to_bits().to_le_bytes());
            }
        }
        assert_eq!(trace.finalize(), big[i], "environment {i} diverged");
        assert_eq!(one.total_mass(0), masses[i]);
    }
}

#[test]
#[ignore = "requires the configured MuJoCo 3.3.7 library"]
fn reset_restores_one_env_without_touching_the_others() {
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&library(), scratch.path()).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let model = batch::so101_table(&runtime, &source).unwrap();
    let mut b = Batch::new(&model, 4, 3, Settings::default()).unwrap();
    let seeds: Vec<u64> = (0..4).map(|i| b.seed(i)).collect();
    let width = b.observation_width();
    let mut start = vec![0.0; 4 * width];
    b.observe(&mut start).unwrap();
    for step in 0..60 {
        b.step(&actions(&b, step, &seeds)).unwrap();
    }
    let mut moved = vec![0.0; 4 * width];
    b.observe(&mut moved).unwrap();
    b.reset(2).unwrap();
    let mut after = vec![0.0; 4 * width];
    b.observe(&mut after).unwrap();
    assert_eq!(&after[2 * width..3 * width], &start[2 * width..3 * width]);
    assert_eq!(&after[..2 * width], &moved[..2 * width]);
    assert_ne!(&moved[..width], &start[..width]);
    assert_eq!(b.time(2), 0.0);
}
