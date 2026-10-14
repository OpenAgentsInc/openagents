use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use robot_lab::document::{
    Asset, Build, Controller as ControllerDocument, CubeTask, Experiment, Outcome, Reference,
    ResourceLimits, Sensor,
};
use robot_sim::runner::{
    ActionChunk, Backend, Controller, Mode, Observation, Progress, Runner, Snapshot,
};
use robot_sim::{Error, Result};
mod common;

#[test]
fn run_errors_use_plain_visible_copy() {
    use robot_sim::runner::Scripted;
    for error in [
        Scripted::new(vec![], 1).err().unwrap(),
        Scripted::new(vec![BTreeMap::from([("joint".into(), f64::NAN)])], 1)
            .err()
            .unwrap(),
    ] {
        assert!(
            oa_copy::violations(&error.to_string(), &[]).is_empty(),
            "{error}"
        );
    }
}

fn build(name: &str) -> Build {
    Build {
        name: name.into(),
        sha256: robot_lab::store::digest(name.as_bytes()),
    }
}
fn experiment() -> Experiment {
    Experiment {
        scene: Reference {
            document: "bench".into(),
            version: "a".repeat(64),
        },
        task: CubeTask {
            version: build("cube-task"),
            object: "cube".into(),
            target_min: [0.0; 3],
            target_max: [1.0; 3],
            stable_seconds: 0.1,
            max_linear_speed: 0.01,
            max_angular_speed: 0.01,
        },
        controller: ControllerDocument::Scripted {
            build: build("controller"),
            robot: "arm".into(),
            joints: vec!["hinge".into()],
        },
        backend: build("backend"),
        timestep_seconds: 0.01,
        control_every_steps: 2,
        sensor_every_steps: 3,
        seeds: vec![7, 8],
        limits: ResourceLimits {
            max_steps: 8,
            wall_seconds: 10,
            max_artifact_bytes: 1_000,
            inference_timeout_ms: 100,
        },
    }
}

#[derive(Default)]
struct Calls {
    controls: Vec<u32>,
    sensors: Vec<u32>,
    seen_sensors: Vec<u64>,
    seeds: Vec<u64>,
}

struct MockBackend {
    calls: Arc<Mutex<Calls>>,
    steps: u32,
    target: f64,
    fail_at: Option<u32>,
}
impl Backend for MockBackend {
    fn build(&self) -> Build {
        build("backend")
    }
    fn timestep(&self) -> f64 {
        0.01
    }
    fn reset(&mut self, seed: u64) -> Result<Snapshot> {
        self.steps = 0;
        self.target = 0.0;
        self.calls.lock().unwrap().seeds.push(seed);
        self.snapshot()
    }
    fn snapshot(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            joints: BTreeMap::from([("hinge".into(), self.target)]),
            ..Snapshot::default()
        })
    }
    fn validate_action(&self, values: &BTreeMap<String, f64>) -> Result<()> {
        if values.len() == 1
            && values
                .get("hinge")
                .is_some_and(|n| n.is_finite() && n.abs() <= 1.0)
        {
            Ok(())
        } else {
            Err(Error::Invalid("Keep the joint inside its limits".into()))
        }
    }
    fn apply(&mut self, values: &BTreeMap<String, f64>) -> Result<()> {
        self.validate_action(values)?;
        self.target = values["hinge"];
        Ok(())
    }
    fn step(&mut self) -> Result<Snapshot> {
        if self.fail_at == Some(self.steps) {
            return Err(Error::Invalid("The backend stopped".into()));
        }
        self.steps += 1;
        self.snapshot()
    }
    fn sensors(&mut self, step: u32) -> Result<BTreeMap<String, Sensor>> {
        self.calls.lock().unwrap().sensors.push(step);
        Ok(BTreeMap::from([(
            "front".into(),
            Sensor {
                asset: Asset {
                    sha256: format!("{step:064x}"),
                    bytes: 10,
                    media_type: "image/png".into(),
                },
                timestamp_seconds: f64::from(step) * 0.01,
                frame: u64::from(step),
            },
        )]))
    }
}

struct MockController {
    calls: Arc<Mutex<Calls>>,
    stale: bool,
    invalid: bool,
    chunk: usize,
    delay_ms: u64,
}
impl Controller for MockController {
    fn build(&self) -> Build {
        build("controller")
    }
    fn reset(&mut self, _: u64) -> Result<()> {
        Ok(())
    }
    fn infer(&mut self, observation: Observation<'_>) -> Result<ActionChunk> {
        if self.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        }
        let mut calls = self.calls.lock().unwrap();
        calls.controls.push(observation.step);
        calls.seen_sensors.push(observation.sensors["front"].frame);
        Ok(ActionChunk {
            observation_step: if self.stale {
                observation.step + 1
            } else {
                observation.step
            },
            expires_step: 7,
            actions: vec![
                BTreeMap::from([("hinge".into(), if self.invalid { 2.0 } else { 0.3 })]);
                self.chunk
            ],
        })
    }
}

fn runner(
    mode: Mode,
    stale: bool,
    invalid: bool,
    chunk: usize,
    fail_at: Option<u32>,
) -> (Runner<MockBackend, MockController>, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let backend = MockBackend {
        calls: Arc::clone(&calls),
        steps: 0,
        target: 0.0,
        fail_at,
    };
    let controller = MockController {
        calls: Arc::clone(&calls),
        stale,
        invalid,
        chunk,
        delay_ms: 0,
    };
    let reference = Reference {
        document: "experiment".into(),
        version: "b".repeat(64),
    };
    (
        Runner::new(backend, controller, reference, experiment(), 7, mode).unwrap(),
        calls,
    )
}

#[test]
fn inference_timeout_and_concurrent_stop_never_advance_physics() {
    for timeout in [true, false] {
        let calls = Arc::new(Mutex::new(Calls::default()));
        let backend = MockBackend {
            calls: Arc::clone(&calls),
            steps: 0,
            target: 0.0,
            fail_at: None,
        };
        let controller = MockController {
            calls: Arc::clone(&calls),
            stale: false,
            invalid: false,
            chunk: 1,
            delay_ms: 30,
        };
        let mut experiment = experiment();
        experiment.limits.inference_timeout_ms = if timeout { 1 } else { 1000 };
        let reference = Reference {
            document: "experiment".into(),
            version: "b".repeat(64),
        };
        let mut runner =
            Runner::new(backend, controller, reference, experiment, 7, Mode::Batch).unwrap();
        let cancel = if timeout {
            None
        } else {
            let stop = runner.stop_handle();
            Some(std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(5));
                stop.store(true, std::sync::atomic::Ordering::Release);
            }))
        };
        assert_eq!(runner.tick().unwrap(), Progress::Finished);
        if let Some(cancel) = cancel {
            cancel.join().unwrap();
        }
        assert_eq!(runner.recording().episode.steps.len(), 1);
        assert_eq!(runner.backend().steps, 0);
        if timeout {
            assert!(matches!(
                runner.recording().episode.outcome,
                Outcome::Failed { .. }
            ));
            assert!(runner.recording().episode.steps[0].inference_ms.unwrap() >= 30.0);
            assert_eq!(
                runner.recording().episode.events[0].kind,
                robot_lab::document::EpisodeEventKind::InferenceTimeout
            );
        } else {
            assert!(matches!(
                runner.recording().episode.outcome,
                Outcome::Stopped { .. }
            ));
        }
    }
}

#[test]
fn camera_limits_keep_a_replayable_physical_step_without_oversized_images() {
    for quota in [9, 10] {
        let calls = Arc::new(Mutex::new(Calls::default()));
        let backend = MockBackend {
            calls: Arc::clone(&calls),
            steps: 0,
            target: 0.0,
            fail_at: None,
        };
        let controller = MockController {
            calls,
            stale: false,
            invalid: false,
            chunk: 1,
            delay_ms: 0,
        };
        let mut experiment = experiment();
        experiment.limits.max_artifact_bytes = quota;
        let reference = Reference {
            document: "experiment".into(),
            version: "b".repeat(64),
        };
        let result = Runner::new(backend, controller, reference, experiment, 7, Mode::Batch);
        if quota == 9 {
            assert!(result.is_err());
            continue;
        }
        let mut runner = result.unwrap();
        while runner.tick().unwrap() != Progress::Finished {}
        assert!(matches!(
            runner.recording().episode.outcome,
            Outcome::Failed { .. }
        ));
        assert_eq!(runner.recording().episode.steps.last().unwrap().index, 3);
        assert!(runner.recording().episode.steps[3].sensors.is_empty());
        assert_eq!(runner.recording().episode.steps[3].joints["hinge"], 0.3);
        assert_eq!(
            runner
                .recording()
                .episode
                .steps
                .iter()
                .map(|s| s.sensors.values().map(|s| s.asset.bytes).sum::<u64>())
                .sum::<u64>(),
            10
        );
        runner.recording().episode.validate().unwrap();
    }
}

#[test]
fn durable_jobs_stop_at_a_boundary_and_recover_the_last_checkpoint() {
    use robot_sim::jobs::{Book, Status};
    let (_directory, store, _, _, reference) = common::fixture();
    let robot_lab::Document::Experiment(mut experiment) = store.get(&reference).unwrap().content
    else {
        panic!()
    };
    experiment.backend = build("backend");
    experiment.controller = ControllerDocument::Scripted {
        build: build("controller"),
        robot: "arm".into(),
        joints: vec!["hinge".into()],
    };
    let reference = store
        .save(
            "experiment",
            Some(&reference),
            robot_lab::Document::Experiment(experiment.clone()),
            "Test",
            "Changed controller",
            4,
        )
        .unwrap();
    let book = Book::open(store.clone()).unwrap();
    let backend = || MockBackend {
        calls: Arc::new(Mutex::new(Calls::default())),
        steps: 0,
        target: 0.0,
        fail_at: None,
    };
    // Job tests need no fake camera assets in their saved episodes.
    struct NoCamera(MockBackend);
    impl Backend for NoCamera {
        fn build(&self) -> Build {
            self.0.build()
        }
        fn timestep(&self) -> f64 {
            self.0.timestep()
        }
        fn reset(&mut self, seed: u64) -> Result<Snapshot> {
            self.0.reset(seed)
        }
        fn snapshot(&self) -> Result<Snapshot> {
            self.0.snapshot()
        }
        fn validate_action(&self, v: &BTreeMap<String, f64>) -> Result<()> {
            self.0.validate_action(v)
        }
        fn apply(&mut self, v: &BTreeMap<String, f64>) -> Result<()> {
            self.0.apply(v)
        }
        fn step(&mut self) -> Result<Snapshot> {
            self.0.step()
        }
        fn sensors(&mut self, _: u32) -> Result<BTreeMap<String, Sensor>> {
            Ok(BTreeMap::new())
        }
    }
    struct Hold;
    impl Controller for Hold {
        fn build(&self) -> Build {
            build("controller")
        }
        fn reset(&mut self, _: u64) -> Result<()> {
            Ok(())
        }
        fn infer(&mut self, observation: Observation<'_>) -> Result<ActionChunk> {
            Ok(ActionChunk {
                observation_step: observation.step,
                expires_step: observation.step,
                actions: vec![BTreeMap::from([("hinge".into(), 0.3)])],
            })
        }
    }
    let mut run = Runner::new(
        NoCamera(backend()),
        Hold,
        reference.clone(),
        experiment.clone(),
        7,
        Mode::Batch,
    )
    .unwrap();
    book.create("stop", reference.clone(), 7, Mode::Batch)
        .unwrap();
    let mut worker = book.start("stop", &run).unwrap();
    worker.tick(&mut run, false, "Test", 5).unwrap();
    assert!(
        book.recover("stop", "Test", 6).is_err(),
        "a live worker cannot be recovered"
    );
    book.stop("stop").unwrap();
    assert_eq!(
        worker.tick(&mut run, false, "Test", 7).unwrap(),
        Progress::Finished
    );
    assert_eq!(
        worker.job().recording.as_ref().unwrap().episode.steps.len(),
        2
    );
    let saved = store.get(worker.job().episode.as_ref().unwrap()).unwrap();
    assert!(
        matches!(saved.content, robot_lab::Document::Episode(ref e) if matches!(e.outcome, Outcome::Stopped { .. }) && e.steps.len()==2)
    );
    drop(worker);
    assert_eq!(
        book.recover("stop", "Test", 8).unwrap().status,
        Status::Finished
    );
    let mut unfinished_save = book.get("stop").unwrap();
    let episode_reference = unfinished_save.episode.take();
    std::fs::write(
        store.root().join("jobs/stop.json"),
        serde_json::to_vec(&unfinished_save).unwrap(),
    )
    .unwrap();
    assert_eq!(
        book.recover("stop", "Test", 8).unwrap().episode,
        episode_reference,
        "a crash between the terminal checkpoint and saving its reference keeps one episode"
    );

    let mut run = Runner::new(
        NoCamera(backend()),
        Hold,
        reference.clone(),
        experiment,
        8,
        Mode::Batch,
    )
    .unwrap();
    book.create("crash", reference, 8, Mode::Batch).unwrap();
    let mut worker = book.start("crash", &run).unwrap();
    worker.tick(&mut run, false, "Test", 9).unwrap();
    worker.tick(&mut run, false, "Test", 10).unwrap();
    drop(worker);
    drop(run);
    let reopened = Book::open(store.clone()).unwrap();
    let recovered = reopened.recover("crash", "Test", 11).unwrap();
    assert_eq!(recovered.status, Status::Interrupted);
    assert_eq!(recovered.recording.as_ref().unwrap().episode.steps.len(), 3);
    let saved = store.get(recovered.episode.as_ref().unwrap()).unwrap();
    assert!(
        matches!(saved.content, robot_lab::Document::Episode(ref e) if e.events.last().unwrap().reason.contains("computer stopped"))
    );
    assert_eq!(
        reopened.recover("crash", "Test", 12).unwrap().episode,
        recovered.episode
    );
    assert!(
        book.create("../escape", recovered.experiment.clone(), 7, Mode::Batch)
            .is_err()
    );
}

#[test]
fn fixed_control_and_sensor_rates_keep_latest_images_and_bound_steps() {
    let (mut runner, calls) = runner(Mode::Batch, false, false, 1, None);
    while runner.tick().unwrap() != Progress::Finished {}
    let calls = calls.lock().unwrap();
    assert_eq!(calls.controls, vec![0, 2, 4, 6]);
    assert_eq!(calls.sensors, vec![0, 3, 6]);
    assert_eq!(calls.seen_sensors, vec![0, 0, 3, 6]);
    assert_eq!(runner.recording().episode.steps.len(), 8);
    assert_eq!(runner.recording().episode.outcome, Outcome::Complete);
    for (i, step) in runner.recording().episode.steps.iter().enumerate() {
        assert_eq!(step.time_seconds, i as f64 * 0.01);
    }
    assert!(runner.recording().episode.steps[1].sensors.is_empty());
    assert_eq!(
        runner.recording().episode.steps[3].sensors["front"].frame,
        3
    );
    assert_eq!(runner.recording().episode.events.len(), 1);
}

#[test]
fn chunks_are_reused_only_at_control_intervals() {
    let (mut runner, calls) = runner(Mode::Batch, false, false, 4, None);
    while runner.tick().unwrap() != Progress::Finished {}
    assert_eq!(calls.lock().unwrap().controls, vec![0]);
    assert_eq!(
        runner.recording().episode.steps[7].observation_step,
        Some(0)
    );
}

#[test]
fn invalid_or_stale_actions_stop_before_a_physics_step_and_keep_the_reason() {
    for (stale, invalid) in [(true, false), (false, true)] {
        let (mut runner, _) = runner(Mode::Batch, stale, invalid, 1, None);
        assert_eq!(runner.tick().unwrap(), Progress::Finished);
        assert_eq!(runner.recording().episode.steps.len(), 1);
        assert_eq!(runner.backend().steps, 0);
        assert!(matches!(
            runner.recording().episode.outcome,
            Outcome::Failed { .. }
        ));
        assert_eq!(runner.recording().episode.events.len(), 1);
    }
}

#[test]
fn interactive_pause_step_close_and_reset_keep_partial_evidence() {
    let (mut runner, _) = runner(Mode::Interactive, false, false, 1, None);
    assert_eq!(runner.tick().unwrap(), Progress::Paused);
    assert_eq!(runner.single_step().unwrap(), Progress::Advanced);
    runner.resume();
    assert_eq!(runner.tick().unwrap(), Progress::Advanced);
    runner.close_view();
    assert_eq!(runner.tick().unwrap(), Progress::Paused);
    let old = runner.reset(8).unwrap();
    assert_eq!(old.episode.steps.len(), 3);
    assert!(matches!(old.episode.outcome, Outcome::Stopped { .. }));
    assert_eq!(runner.recording().episode.seed, 8);
    assert_eq!(runner.recording().episode.steps.len(), 1);
    assert!(runner.reset(9).is_err());
}

#[test]
fn closing_a_batch_view_keeps_it_running_and_stop_is_a_step_boundary() {
    let (mut runner, _) = runner(Mode::Batch, false, false, 1, None);
    runner.close_view();
    assert_eq!(runner.tick().unwrap(), Progress::Advanced);
    runner
        .stop_handle()
        .store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(runner.tick().unwrap(), Progress::Finished);
    assert_eq!(runner.recording().episode.steps.len(), 2);
    assert!(matches!(
        runner.recording().episode.outcome,
        Outcome::Stopped { .. }
    ));
}

#[test]
fn backend_failure_keeps_previous_steps_and_a_saved_failure_event() {
    let (mut runner, _) = runner(Mode::Batch, false, false, 1, Some(2));
    while runner.tick().unwrap() != Progress::Finished {}
    assert_eq!(runner.recording().episode.steps.len(), 3);
    assert_eq!(runner.recording().episode.events[0].step, 2);
    assert!(matches!(
        runner.recording().episode.outcome,
        Outcome::Failed { .. }
    ));
    runner.recording().episode.validate().unwrap();
}
