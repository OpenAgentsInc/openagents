//! Batched-environment benchmark (#11659): steps per second per
//! environment, the total, and real time per environment, for three active
//! scenes, with the hardware and settings printed alongside.
//!
//! ```sh
//! cargo run --release -p robot-sim --features mujoco --example batch_bench -- \
//!     LIBRARY FRANKA_ROOT [ENVS,ENVS,...] [THREADS]
//! ```
//!
//! `LIBRARY` is the pinned MuJoCo 3.3.7 library and `FRANKA_ROOT` the
//! Menagerie Panda folder (`tests/fixtures/franka/fetch.py`). Every timed
//! step is a real control step: the caller's thread writes a fresh action
//! row per environment from the last observations, then the workers step
//! every environment once and write its observation back ([`Batch::run`]). No scene is idle: the
//! Franka chases a new random joint target every half second, the SO-101
//! runs a grasp cycle against the table cube, and the pile is timed from
//! the drop with sleep off.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::DVec3;
use physics::batch::{BatchWorld, pile};
use robot_sim::mujoco::batch::{self, Batch, Settings};
use robot_sim::mujoco::{LibraryPin, Model, Runtime, Servo};
use robot_sim::runner::Backend;

const SECONDS: f64 = 2.0;

struct Row {
    scene: &'static str,
    mode: &'static str,
    envs: usize,
    threads: usize,
    dt: f64,
    steps: usize,
    wall: f64,
    activity: String,
}

impl Row {
    fn print(&self) {
        let total = (self.envs * self.steps) as f64 / self.wall;
        let per_env = self.steps as f64 / self.wall;
        println!(
            "{:<13} {:<8} {:>6} {:>4} {:>8.5} {:>6} {:>8.3} {:>12.0} {:>11.0} {:>9.2}x  {}",
            self.scene,
            self.mode,
            self.envs,
            self.threads,
            self.dt,
            self.steps,
            self.wall,
            total,
            per_env,
            per_env * self.dt,
            self.activity
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: batch_bench LIBRARY FRANKA_ROOT [ENVS,...] [THREADS]");
        std::process::exit(2);
    }
    let library = PathBuf::from(&args[1]);
    let franka = PathBuf::from(&args[2]);
    let counts: Vec<usize> = args
        .get(3)
        .map_or("1,16,64,256,1024", String::as_str)
        .split(',')
        .map(|n| n.parse().expect("environment counts"))
        .collect();
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let threads: usize = args.get(4).map_or(cores, |t| t.parse().expect("threads"));

    let bytes = std::fs::read(&library).expect("MuJoCo library");
    let pin = LibraryPin {
        path: library,
        sha256: robot_lab::store::digest(&bytes),
    };
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&pin, scratch.path()).expect("MuJoCo 3.3.7");

    println!("hardware: {}", hardware());
    println!("load before: {}", load());
    println!(
        "settings: {threads} worker threads of {cores} cores, {SECONDS} s simulated per environment, \
         f64, MuJoCo 3.3.7 (implicitfast) and openagents physics"
    );
    println!(
        "{:<13} {:<8} {:>6} {:>4} {:>8} {:>6} {:>8} {:>12} {:>11} {:>10}  activity",
        "scene",
        "mode",
        "envs",
        "thr",
        "dt s",
        "steps",
        "wall s",
        "env-steps/s",
        "steps/s/env",
        "RT/env"
    );

    let so101_source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let franka_model = franka_reach(&runtime, &franka);
    let so101 = batch::so101_table(&runtime, &so101_source).expect("SO-101 scene");
    for (scene, model, policy) in [
        ("franka-reach", &franka_model, reach as Policy),
        ("so101-grasp", &so101, grasp as Policy),
    ] {
        for &envs in &counts {
            for central in [true, false] {
                mujoco_row(scene, model, envs, threads, policy, central).print();
            }
        }
    }
    for &envs in &counts {
        for central in [true, false] {
            pile_row(envs, threads, central).print();
        }
    }
    println!("load after: {}", load());
    println!(
        "RT/env is simulated seconds per wall second for one environment; env-steps/s is the sum \
         over every environment in the batch. Mode sync hands every step's observations to one \
         policy on the caller's thread and waits for all environments (Batch::run); rollout lets \
         each worker carry its environments through every step with a per-environment policy \
         (Batch::rollout, BatchWorld::rollout). A batch of one runs on one thread; the pile's \
         single world may still use its own island threads."
    );
}

fn hardware() -> String {
    let sysctl = |key: &str| {
        std::process::Command::new("sysctl")
            .args(["-n", key])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let cpu = sysctl("machdep.cpu.brand_string").unwrap_or_else(|| std::env::consts::ARCH.into());
    let p = sysctl("hw.perflevel0.physicalcpu");
    let e = sysctl("hw.perflevel1.physicalcpu");
    let memory = sysctl("hw.memsize")
        .and_then(|m| m.parse::<u64>().ok())
        .map(|b| format!(", {} GB", b >> 30))
        .unwrap_or_default();
    match (p, e) {
        (Some(p), Some(e)) => format!(
            "{cpu} ({p} performance + {e} efficiency cores{memory}), {}",
            std::env::consts::OS
        ),
        _ => format!("{cpu}{memory}, {}", std::env::consts::OS),
    }
}

/// The 1, 5 and 15 minute load averages: other work on the machine slows
/// every row, and the synchronous mode most.
fn load() -> String {
    std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_matches(['{', '}', ' '])
                .to_string()
        })
        .unwrap_or_else(|| "unknown".into())
}

fn franka_reach(runtime: &Runtime, root: &Path) -> Model {
    let mut model = runtime
        .load(root, "panda_nohand.xml", 1.0 / 500.0)
        .expect("Franka Panda (Menagerie)");
    let home = [0.0, 0.0, 0.0, -1.57079, 0.0, 1.57079, -0.7853];
    let mut targets = BTreeMap::new();
    for (index, value) in home.iter().enumerate() {
        let name = format!("joint{}", index + 1);
        model
            .control_joint(
                &name,
                Servo {
                    kp: 400.0,
                    kd: 40.0,
                    effort: if index < 4 { 87.0 } else { 12.0 },
                    speed: 2.0,
                },
            )
            .unwrap();
        targets.insert(name, *value);
    }
    model.initial_pose(&targets).unwrap();
    model
}

/// A fresh random joint target every half second, a third of the way to
/// each limit from home.
fn reach(home: &[f64], limits: &[Option<(f64, f64)>], seed: u64, t: f64, out: &mut [f64]) {
    let epoch = (t / 0.5) as u64;
    let mut draw = physics::batch::Draw(seed ^ epoch.wrapping_mul(0x9e37_79b9));
    for ((h, l), o) in home.iter().zip(limits).zip(out) {
        *o = match l {
            Some((lo, hi)) => {
                let u = draw.unit();
                (h + (if u < 0.5 { lo - h } else { hi - h }) * (u - 0.5).abs() * 0.66)
                    .clamp(*lo, *hi)
            }
            None => *h,
        };
    }
}

/// A 2 s grasp cycle at the table cube: reach down, close, lift, open.
/// Joint order is name order: elbow_flex, gripper, shoulder_lift,
/// shoulder_pan, wrist_flex, wrist_roll.
fn grasp(_: &[f64], limits: &[Option<(f64, f64)>], seed: u64, t: f64, out: &mut [f64]) {
    let phase = (t + (seed % 97) as f64 * 0.01) % 2.0;
    let down = (phase / 0.6).min(1.0);
    let lift = ((phase - 1.2) / 0.5).clamp(0.0, 1.0);
    let close = if (0.7..1.8).contains(&phase) {
        1.0
    } else {
        0.0
    };
    let target = [
        0.1 * down,
        1.0 - 1.2 * close,
        0.25 * down - 0.4 * lift,
        -0.4,
        1.35 * down,
        1.15,
    ];
    for ((l, v), o) in limits.iter().zip(target).zip(out) {
        *o = l.map_or(v, |(lo, hi)| v.clamp(lo, hi));
    }
}

type Policy = fn(&[f64], &[Option<(f64, f64)>], u64, f64, &mut [f64]);

fn mujoco_row(
    scene: &'static str,
    model: &Model,
    envs: usize,
    threads: usize,
    policy: Policy,
    central: bool,
) -> Row {
    let mut b = Batch::new(
        model,
        envs,
        11_659,
        Settings {
            threads,
            randomize: None,
        },
    )
    .unwrap();
    let home = b.home();
    let limits = b.limits();
    let seeds: Vec<u64> = (0..envs).map(|e| b.seed(e)).collect();
    let dt = model.timestep();
    let steps = (SECONDS / dt).round() as usize;
    let width = b.observation_width();
    let joints = home.len();
    let vcols: Vec<usize> = b.joint_columns().into_iter().map(|(_, dof)| dof).collect();
    let nq = model.dimensions().0;
    let (mut contacts, mut speed) = (0usize, 0.0);
    let start = Instant::now();
    if central {
        b.run(steps, |step, obs, ncon, actions| {
            let t = step as f64 * dt;
            for (seed, row) in seeds.iter().zip(actions.chunks_mut(joints)) {
                policy(&home, &limits, *seed, t, row);
            }
            if step % 8 == 0 {
                contacts += ncon.iter().sum::<usize>();
                for row in obs.chunks(width) {
                    let qvel = &row[nq..];
                    speed += vcols.iter().map(|c| qvel[*c].abs()).sum::<f64>() / vcols.len() as f64;
                }
            }
        })
        .unwrap();
    } else {
        b.rollout(steps, |env, step, _, action| {
            policy(&home, &limits, seeds[env], step as f64 * dt, action);
        })
        .unwrap();
    }
    let wall = start.elapsed().as_secs_f64();
    let samples = steps.div_ceil(8) * envs;
    Row {
        scene,
        mode: if central { "sync" } else { "rollout" },
        envs,
        threads: b.threads().min(envs),
        dt,
        steps,
        wall,
        activity: if central {
            format!(
                "mean joint speed {:.2} rad/s, {:.1} contacts/step",
                speed / samples as f64,
                contacts as f64 / samples as f64
            )
        } else {
            String::new()
        },
    }
}

fn pile_row(envs: usize, threads: usize, central: bool) -> Row {
    let gravity = physics::Uniform(DVec3::new(0.0, -9.81, 0.0));
    let mut b = BatchWorld::new(envs, 11_659, |s| pile(s, 50));
    b.threads = threads;
    let dt = b.worlds()[0].dt;
    let steps = (SECONDS / dt).round() as usize;
    let (mut contacts, mut speed, mut samples) = (0usize, 0.0, 0usize);
    let start = Instant::now();
    if central {
        for step in 0..steps {
            b.step(&gravity);
            if step % 8 == 0 {
                for w in b.worlds() {
                    contacts += w.contacts.len();
                    let moving: Vec<_> = w.bodies().iter().filter(|b| b.moves()).collect();
                    speed +=
                        moving.iter().map(|b| b.vel.length()).sum::<f64>() / moving.len() as f64;
                    samples += 1;
                }
            }
        }
    } else {
        b.rollout(steps, |_, _, w| w.step(&gravity));
    }
    let wall = start.elapsed().as_secs_f64();
    Row {
        scene: "pile-50",
        mode: if central { "sync" } else { "rollout" },
        envs,
        threads: b.thread_count(),
        dt,
        steps,
        wall,
        activity: if central {
            format!(
                "mean body speed {:.2} m/s, {:.1} contact points/step",
                speed / samples as f64,
                contacts as f64 / samples as f64
            )
        } else {
            String::new()
        },
    }
}
