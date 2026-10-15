//! Records Lagrange 2's arm reference move in MuJoCo 3.3.7
//! (`verse_lagrange2::oracle`): the factory arm, 100 times Robot Lab's
//! SO-101, carrying a loaded tank from its parking pose to the transit
//! height, through this crate's native runner and servo law.
//!
//! ```sh
//! cargo run -p robot-sim --features mujoco --example lagrange2_arm_oracle -- \
//!   EXECUTION.json crates/verse-lagrange2/oracle/arm-reference.json
//! ```
//!
//! `EXECUTION.json` names the pinned library as
//! `{"library": {"path": ..., "sha256": ...}}`. The Cargo test
//! `arms_match_mujoco_for_a_reference_move` replays the same targets in
//! `physics` without MuJoCo.

use std::collections::BTreeMap;
use std::path::PathBuf;

use robot_sim::mujoco::{LibraryPin, Runtime, Servo};
use robot_sim::runner::Backend;
use verse_lagrange2::arm::JOINTS;
use verse_lagrange2::factory::DT;
use verse_lagrange2::oracle::{Reference, ReferenceMove, SCHEMA};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [execution, output] = args.as_slice() else {
        return Err("usage: lagrange2_arm_oracle EXECUTION.json OUTPUT.json".into());
    };
    let execution: serde_json::Value = serde_json::from_slice(&std::fs::read(execution)?)?;
    let path = PathBuf::from(
        execution["library"]["path"]
            .as_str()
            .ok_or("execution.json has no library.path")?,
    );
    let sha256 = robot_lab::store::digest(&std::fs::read(&path)?);
    if Some(sha256.as_str()) != execution["library"]["sha256"].as_str() {
        return Err("the library's SHA-256 doesn't match execution.json".into());
    }
    let scratch = tempfile::tempdir()?;
    let runtime = Runtime::open(
        &LibraryPin {
            path,
            sha256: sha256.clone(),
        },
        scratch.path(),
    )?;
    let reference = ReferenceMove::sample();
    let xml = reference.mjcf();
    let models = tempfile::tempdir()?;
    std::fs::write(models.path().join("arm.xml"), &xml)?;
    let mut model = runtime.load(models.path(), "arm.xml", DT)?;
    for (i, name) in JOINTS.iter().enumerate() {
        let (kp, kd, effort, speed) = ReferenceMove::mujoco_servo(i);
        model.control_joint(
            name,
            Servo {
                kp,
                kd,
                effort,
                speed,
            },
        )?;
    }
    let pose = |q: [f64; 5]| -> BTreeMap<String, f64> {
        JOINTS
            .iter()
            .zip(q)
            .map(|(n, v)| ((*n).to_owned(), v))
            .collect()
    };
    model.initial_pose(&pose(reference.motion.from))?;
    let mut angles = Vec::new();
    for k in 0..reference.steps {
        model.apply(&pose(reference.target(k)))?;
        model.step()?;
        if (k + 1) % reference.record_every == 0 {
            let q = model.qpos();
            angles.push([q[0], q[1], q[2], q[3], q[4]]);
        }
    }
    let record = Reference {
        schema: SCHEMA.into(),
        mujoco: "3.3.7".into(),
        library_sha256: sha256,
        timestep: DT,
        model_xml: xml,
        record_every: reference.record_every,
        angles,
    };
    std::fs::write(output, serde_json::to_string(&record)? + "\n")?;
    println!(
        "{output}: {} samples over {:.0} s",
        record.angles.len(),
        reference.steps as f64 * DT
    );
    Ok(())
}
