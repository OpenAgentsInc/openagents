//! Run the exact pinned ACT checkpoint on one saved native scene.
use robot_lab::{Document, Reference, document::*};
use robot_policy::{artifacts::Artifact, controller::NativeController};
use robot_sim::{
    jobs::Book,
    mujoco::{LibraryPin, Runtime, Servo},
    runner::{Backend, Mode, Runner},
};
use std::{fs::File, path::Path};
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 8 {
        return Err(
            "Use: STORE PROJECT SCENE@VERSION JOB LIBRARY LIBRARY_SHA POLICY_FOLDER POLICY_SHA"
                .into(),
        );
    }
    let store = robot_lab::Store::open(Path::new(&args[0]), &args[1])?;
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = robot_workbench::host::Host::open(store.clone(), &catalog, "http://127.0.0.1:8794")?;
    let (document, version) = args[2]
        .split_once('@')
        .ok_or("Choose the exact saved scene")?;
    let reference = Reference {
        document: document.into(),
        version: version.into(),
    };
    let bundle = host.bundle(&reference)?;
    let checked = robot_sim::import::pinned_urdf(&store, &catalog)?;
    if checked != bundle.robot {
        return Err("Use the checked robot description".into());
    }
    let artifact = Artifact::open(Path::new(&args[6]), &args[7])?;
    let weights = store.put_asset(
        File::open(artifact.root.join(&artifact.pin.weights))?,
        "application/x-safetensors",
        256 << 20,
    )?;
    let normalization = store.put_asset(
        File::open(artifact.root.join(&artifact.pin.normalization))?,
        "application/json",
        1 << 20,
    )?;
    let runtime = Runtime::open(
        &LibraryPin {
            path: args[4].clone().into(),
            sha256: args[5].clone(),
        },
        store.root(),
    )?;
    let backend = runtime.arm_scene(
        &catalog,
        &bundle.robot.description,
        &bundle.scene,
        artifact.pin.binding.timestep_seconds,
        Servo {
            kp: 100.,
            kd: 4.,
            effort: 2.4,
            speed: 2.,
        },
    )?;
    let (_, visual) = robot_description::pin::load_pinned(&catalog)?;
    let backend = robot_workbench::record::Cameras::new(
        backend,
        store.clone(),
        bundle.scene.clone(),
        std::sync::Arc::new(visual),
    )?;
    let experiment = Experiment {
        scene: reference,
        task: CubeTask {
            version: artifact.pin.binding.task.clone(),
            object: "cube".into(),
            target_min: [0.3, 0.75, -0.15],
            target_max: [0.4, 0.85, -0.05],
            stable_seconds: 0.2,
            max_linear_speed: 0.01,
            max_angular_speed: 0.01,
        },
        controller: Controller::Policy {
            build: artifact.build.clone(),
            weights,
            normalization,
            robot: bundle.scene.robots[0].id.clone(),
            joints: artifact
                .pin
                .binding
                .actions
                .joints
                .iter()
                .map(|j| j.joint.clone())
                .collect(),
            cameras: artifact
                .pin
                .binding
                .cameras
                .iter()
                .map(|c| c.id.clone())
                .collect(),
            instruction: artifact.pin.binding.instruction.clone(),
        },
        backend: backend.build(),
        timestep_seconds: artifact.pin.binding.timestep_seconds,
        control_every_steps: artifact.pin.binding.control_every_steps,
        sensor_every_steps: artifact.pin.binding.control_every_steps,
        seeds: vec![7],
        limits: ResourceLimits {
            max_steps: 241,
            wall_seconds: 60,
            max_artifact_bytes: 4 << 20,
            inference_timeout_ms: 1000,
        },
    };
    let controller = NativeController::new(
        artifact,
        store.clone(),
        &bundle.robot,
        &bundle.scene,
        &experiment,
    )?;
    let exp_ref = store.save(
        &format!("act-{}", args[3]),
        None,
        Document::Experiment(experiment.clone()),
        "Local user",
        "Started ACT attempt",
        now(),
    )?;
    let book = Book::open(store.clone())?;
    let mut runner = Runner::new(
        backend,
        controller,
        exp_ref.clone(),
        experiment,
        7,
        Mode::Interactive,
    )?;
    book.create(&args[3], exp_ref, 7, Mode::Interactive)?;
    let mut worker = book.start(&args[3], &runner)?;
    worker.tick(&mut runner, true, "Local user", now())?;
    assert!(runner.paused());
    let previous = runner.reset(7)?;
    assert_eq!(previous.episode.steps.len(), 2);
    drop(worker);
    let reset_run = format!("{}-reset", args[3]);
    book.recover(&args[3], "Local user", now())?;
    book.create(
        &reset_run,
        runner.recording().episode.experiment.clone(),
        7,
        Mode::Interactive,
    )?;
    let mut worker = book.start(&reset_run, &runner)?;
    runner.resume();
    while !runner.finished() {
        worker.tick(&mut runner, false, "Local user", now())?;
    }
    let episode = worker.job().episode.clone().unwrap();
    let recording = &runner.recording().episode;
    assert!(matches!(recording.outcome, Outcome::Complete));
    assert!(
        recording
            .steps
            .iter()
            .filter(|s| !s.actions.is_empty())
            .all(|s| s.observation_step.is_some_and(|origin| origin < s.index))
    );
    let latencies = recording
        .steps
        .iter()
        .filter_map(|s| s.inference_ms)
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"episode":episode,"steps":recording.steps.len(),"inference_calls":latencies.len(),"max_inference_ms":latencies.iter().copied().fold(0.,f64::max),"limit_violations":recording.steps.iter().map(|s|s.limit_violations.len()).sum::<usize>(),"card":host.card(&episode,None)?})
        )?
    );
    Ok(())
}
