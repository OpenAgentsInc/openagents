//! Run a bounded scripted SO-101 attempt, or stop/recover its private job.
use robot_lab::document::*;
use robot_lab::{Document, Store};
use robot_sim::jobs::Book;
use robot_sim::mujoco::{LibraryPin, Runtime, Servo};
use robot_sim::runner::{Backend, Controller as _, Mode, Runner, Scripted};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn scene(reference: Reference, joints: BTreeMap<String, f64>) -> Scene {
    let object = |id: &str, position, size, mass| Object {
        id: id.into(),
        geometry: Geometry {
            pose: Pose {
                position,
                ..Pose::default()
            },
            shape: Shape::Box { size },
            rgba: [0.8, 0.1, 0.1, 1.0],
        },
        mesh: None,
        collider: Some(Shape::Box { size }),
        mass,
        friction: 0.7,
    };
    Scene {
        name: "SO-101 table".into(),
        robots: vec![RobotInstance {
            id: "arm".into(),
            robot: reference,
            pose: Pose {
                position: [0.0, 0.75, 0.0],
                ..Pose::default()
            },
            joints,
        }],
        objects: vec![
            object("table", [0.0, 0.73, 0.0], [1.2, 0.04, 0.75], 0.0),
            object("cube", [0.22, 0.80, -0.1], [0.05; 3], 0.05),
        ],
        lights: vec![],
        cameras: vec![Camera {
            id: "front".into(),
            pose: Pose::default(),
            width: 64,
            height: 64,
            intrinsics: [50.0, 50.0, 32.0, 32.0],
            robot_link: None,
        }],
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        return Err("Use: run|stop|recover STORE PROJECT JOB [LIBRARY SHA256]".into());
    }
    let store = Store::open(Path::new(&args[1]), &args[2])?;
    let book = Book::open(store.clone())?;
    match args[0].as_str() {
        "stop" => {
            book.stop(&args[3])?;
            println!("Stop requested");
            return Ok(());
        }
        "recover" => {
            println!(
                "{}",
                serde_json::to_string(&book.recover(&args[3], "Local user", now())?)?
            );
            return Ok(());
        }
        "run" if args.len() == 6 => (),
        _ => return Err("Use: run STORE PROJECT JOB LIBRARY SHA256".into()),
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let robot = robot_sim::import::pinned_urdf(&store, &source)?;
    let robot_ref = store.save(
        &format!("arm-{}", args[3]),
        None,
        Document::Robot(robot.clone()),
        "Local user",
        "Added SO-101",
        now(),
    )?;
    let joints: BTreeMap<_, _> = robot
        .joints
        .iter()
        .filter(|j| j.kind != JointKind::Fixed)
        .map(|j| (j.name.clone(), 0.0))
        .collect();
    let scene = scene(robot_ref, joints.clone());
    let scene_ref = store.save(
        &format!("scene-{}", args[3]),
        None,
        Document::Scene(scene.clone()),
        "Local user",
        "Added table and cube",
        now(),
    )?;
    let runtime = Runtime::open(
        &LibraryPin {
            path: PathBuf::from(&args[4]),
            sha256: args[5].clone(),
        },
        store.root(),
    )?;
    let backend = runtime.arm_scene(
        &source,
        &robot.description,
        &scene,
        1.0 / 240.0,
        Servo {
            kp: 100.0,
            kd: 4.0,
            effort: 2.4,
            speed: 2.0,
        },
    )?;
    let mut moved = joints.clone();
    moved.insert("shoulder_pan".into(), 0.2);
    let controller = Scripted::new(vec![joints, moved], 15)?;
    controller.save(&store)?;
    let experiment = Experiment {
        scene: scene_ref,
        task: CubeTask {
            version: Build {
                name: "cube-placement-v1".into(),
                sha256: robot_lab::store::digest(b"cube-placement-v1"),
            },
            object: "cube".into(),
            target_min: [0.3, 0.75, -0.15],
            target_max: [0.4, 0.85, -0.05],
            stable_seconds: 0.2,
            max_linear_speed: 0.01,
            max_angular_speed: 0.01,
        },
        controller: robot_lab::document::Controller::Scripted {
            build: controller.build(),
            robot: "arm".into(),
            joints: robot
                .joints
                .iter()
                .filter(|j| j.kind != JointKind::Fixed)
                .map(|j| j.name.clone())
                .collect(),
        },
        backend: backend.build(),
        timestep_seconds: 1.0 / 240.0,
        control_every_steps: 8,
        sensor_every_steps: 8,
        seeds: vec![7],
        limits: ResourceLimits {
            max_steps: 241,
            wall_seconds: 60,
            max_artifact_bytes: 1 << 20,
            inference_timeout_ms: 1000,
        },
    };
    let reference = store.save(
        &format!("experiment-{}", args[3]),
        None,
        Document::Experiment(experiment.clone()),
        "Local user",
        "Added scripted attempt",
        now(),
    )?;
    book.create(&args[3], reference.clone(), 7, Mode::Batch)?;
    let mut runner = Runner::new(backend, controller, reference, experiment, 7, Mode::Batch)?;
    let mut worker = book.start(&args[3], &runner)?;
    while !runner.finished() {
        worker.tick(&mut runner, false, "Local user", now())?;
    }
    println!(
        "{}",
        serde_json::to_string(worker.job().episode.as_ref().unwrap())?
    );
    Ok(())
}
