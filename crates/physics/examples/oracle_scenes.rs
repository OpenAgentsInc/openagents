//! Writes the Genesis oracle scenes (GP-8, #9785) and this crate's trace of
//! each, for `oracle/genesis_oracle.py` to replay and compare.
//!
//! ```sh
//! cargo run -p physics --example oracle_scenes -- target/oracle
//! cargo run -p physics --example oracle_scenes -- target/oracle compare
//! ```
//!
//! With `compare`, it reads each `<name>.genesis.json` and prints the largest
//! difference from this crate's run per quantity.

use std::path::PathBuf;

use physics::oracle::{Scene, tank_into_panel};

fn main() -> std::io::Result<()> {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/oracle".into()),
    );
    std::fs::create_dir_all(&dir)?;
    let compare = std::env::args().nth(2).as_deref() == Some("compare");
    for scene in [tank_into_panel()] {
        if compare {
            let path = dir.join(format!("{}.genesis.json", scene.name));
            let genesis: physics::Trace =
                serde_json::from_str(&std::fs::read_to_string(path)?).expect("a trace");
            let ours = scene.run();
            let d = ours.max_difference(&genesis);
            // After the impact settles: the second half of the run.
            let half = |t: &physics::Trace| physics::Trace {
                samples: t.samples[t.samples.len() / 2..].to_vec(),
            };
            let late = half(&ours).max_difference(&half(&genesis));
            println!(
                "{}: largest difference from Genesis: position {:.4} m, velocity {:.4} m/s, attitude {:.4} rad, body rate {:.4} rad/s",
                scene.name, d.pos, d.vel, d.angle, d.omega
            );
            println!(
                "  second half: velocity {:.4} m/s, body rate {:.4} rad/s",
                late.vel, late.omega
            );
        } else {
            write(&dir, &scene)?;
        }
    }
    Ok(())
}

fn write(dir: &std::path::Path, scene: &Scene) -> std::io::Result<()> {
    let scene_json = serde_json::to_string_pretty(scene).expect("scene serializes");
    let trace_json = serde_json::to_string_pretty(&scene.run()).expect("trace serializes");
    std::fs::write(dir.join(format!("{}.scene.json", scene.name)), scene_json)?;
    std::fs::write(dir.join(format!("{}.rust.json", scene.name)), trace_json)?;
    println!("wrote {} to {}", scene.name, dir.display());
    Ok(())
}
