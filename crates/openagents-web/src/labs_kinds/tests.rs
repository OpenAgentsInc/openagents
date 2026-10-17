use super::*;
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use lab_api::{Access, Action, Api, Authorizer, Gate, Registry};
use lab_kit::SpaceRef;
use lab_store::{ArtifactInput, backend::FsBackend};
use std::sync::Arc;
use tower::ServiceExt;

struct Alice;
#[async_trait::async_trait]
impl Authorizer for Alice {
    async fn authorize(&self, _: &HeaderMap, space: &str, _: Action) -> Result<Access, ApiError> {
        if space != "me" {
            return Err(ApiError::missing());
        }
        Ok(Access {
            account: "alice".into(),
            space: space_ref(),
            experimental: true,
            cad: false,
        })
    }
}
fn space_ref() -> SpaceRef {
    SpaceRef::Personal {
        account: "alice".into(),
    }
}
struct Fixture {
    app: Router,
    store: Store,
    _tmp: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let backend = Arc::new(FsBackend::open(tmp.path().join("labs")).unwrap());
        let mut registry = Registry::default();
        registry
            .register(Arc::new(GameApi), Gate::Experimental)
            .unwrap();
        registry
            .register(Arc::new(ship()), Gate::Experimental)
            .unwrap();
        registry
            .register(Arc::new(video()), Gate::Experimental)
            .unwrap();
        registry
            .register(Arc::new(RobotApi), Gate::Experimental)
            .unwrap();
        let store = Store::new(backend.clone(), &space_ref()).unwrap();
        let app = Arc::new(Api {
            backend,
            authorizer: Arc::new(Alice),
            registry,
        })
        .router();
        Self {
            app,
            store,
            _tmp: tmp,
        }
    }
    async fn call(&self, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
        let r = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", "Bearer alice")
            .header("content-type", "application/json")
            .header("Idempotency-Key", lab_kit::digest(&json!([path, body])))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let r = self.app.clone().oneshot(r).await.unwrap();
        let status = r.status();
        let bytes = axum::body::to_bytes(r.into_body(), 4 << 20).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
    async fn create(&self, id: &str, kind: &str, schema: &str, content: Value) {
        let (s, b) = self
            .call(
                "POST",
                "/v1/labs/documents",
                json!({"id": id, "kind": kind, "schema": schema, "title": id,
                    "content": content, "summary": "Created"}),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
    }
    async fn tool(&self, kind: &str, name: &str, base: u64, args: Value) -> (StatusCode, Value) {
        self.call(
            "POST",
            &format!("/v1/labs/kinds/{kind}/tools/{name}"),
            json!({"base": base, "arguments": args}),
        )
        .await
    }
}

#[tokio::test]
async fn only_the_mapped_tools_are_offered() {
    let f = Fixture::new();
    let (s, kinds) = f.call("GET", "/v1/labs/kinds", Value::Null).await;
    assert_eq!(s, StatusCode::OK, "{kinds}");
    for (kind, tools) in [
        ("game", GAME_TOOLS),
        ("ship", SHIP_TOOLS),
        ("video", VIDEO_TOOLS),
    ] {
        let manifest = kinds["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["kind"] == kind)
            .unwrap();
        let names: Vec<_> = manifest["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, tools);
    }
}

#[tokio::test]
async fn a_game_edit_waits_for_its_playtest_then_saves() {
    let f = Fixture::new();
    f.create(
        "house",
        "game",
        game_lab::doc::KIND,
        game_lab::doc::original(),
    )
    .await;
    let patch = "[{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":6.2}]";
    let args = json!({"game": "house", "patch": patch, "summary": "A faster man"});
    let (s, b) = f.tool("game", "game_edit", 1, args.clone()).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    assert_eq!(b["error"]["code"], "playtest_waiting");
    assert_eq!(b["error"]["message"], PLAYTEST_WAITING);
    let jobs = f.store.jobs("house", 0, 10).await.unwrap().items;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].spec.job_type, "playtest");
    assert_eq!(jobs[0].status, JobStatus::Queued);
    assert!(jobs[0].message.as_deref().unwrap().contains("Waiting"));

    // Asking again doesn't queue a second playtest.
    assert_eq!(
        f.tool("game", "game_edit", 1, args.clone()).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(f.store.jobs("house", 0, 10).await.unwrap().items.len(), 1);

    // A worker plays it, as `game-lab playtest-job` does.
    let base = f.store.version("house", 1).await.unwrap();
    let lease = f
        .store
        .claim(&jobs[0].id, "worker", now(), 30_000)
        .await
        .unwrap();
    let lease = f.store.start(&lease, now()).await.unwrap();
    let input = json!({"work": {"version": base, "assignment": {"lease": lease}}});
    let output = game_lab::kind::playtest_handler(input.to_string().as_bytes()).unwrap();
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(output["outputs"][0]["content_base64"].as_str().unwrap())
        .unwrap();
    let artifact = f
        .store
        .put_job_artifact(
            &lease,
            ArtifactInput {
                version: base.reference(),
                role: ArtifactRole::Export,
                media_type: "application/json".into(),
                class: lab_kit::AssetClass::Public,
                no_training: false,
            },
            bytes,
            1 << 20,
            now(),
        )
        .await
        .unwrap();
    f.store
        .finish(&lease, JobStatus::Done, None, vec![artifact.id], now())
        .await
        .unwrap();

    let (s, b) = f.tool("game", "game_edit", 1, args).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["result"]["version"]["number"], 2);
    // A schema problem is refused at once, without a playtest.
    let (s, b) = f
        .tool(
            "game",
            "game_edit",
            2,
            json!({"game": "house", "patch": "[{\"op\":\"remove\",\"path\":\"/man/sight\"}]"}),
        )
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(b["error"]["code"], "game_schema");
}

#[tokio::test]
async fn ship_edits_are_checked_and_refusals_keep_the_ship() {
    let f = Fixture::new();
    let tug = serde_json::to_value(verse_lagrange2::ship::Spacecraft::sample()).unwrap();
    f.create("tug", "ship", verse_lagrange2::ship::SCHEMA, tug)
        .await;
    let (s, b) = f
        .tool(
            "ship",
            "ship_edit",
            1,
            json!({"doc": "tug", "edit": "scale", "parts": ["tank-port", "tank-starboard"],
                "factor": 1.05, "summary": "Both tanks 5% bigger"}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["result"]["version"]["number"], 2);
    let (s, b) = f
        .tool(
            "ship",
            "ship_edit",
            2,
            json!({"doc": "tug", "edit": "set_dry_mass", "parts": ["habitat"], "kg": 60000.0,
                "summary": "Heavy"}),
        )
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    assert_eq!(b["error"]["code"], "ship_check");
    assert_eq!(f.store.document("tug").await.unwrap().latest, 2);
}

#[tokio::test]
async fn video_edits_and_renders_through_the_platform() {
    let f = Fixture::new();
    let cut = json!({
        "fps": 30, "size": [320, 180], "out": "~/out.mp4", "timeline": {},
        "shots": [
            {"name": "open", "src": "~/clips/a.mp4", "in": 0, "dur": 1.0},
            {"name": "end", "src": "~/clips/b.mp4", "in": 0, "dur": 1.0}
        ]
    });
    f.create("film", "video", "cut.v1", cut).await;
    let (s, b) = f
        .tool(
            "video",
            "lab_edit",
            1,
            json!({"document": "film", "summary": "A longer end",
                "patch": "[{\"op\":\"replace\",\"path\":\"/shots/@end/dur\",\"value\":1.5}]"}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["result"]["version"]["number"], 2);
    let (s, b) = f
        .tool("video", "lab_render", 2, json!({"document": "film"}))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, b) = f
        .tool(
            "video",
            "lab_render",
            2,
            json!({"document": "film", "version": "2"}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["result"]["spec"]["type"], "render", "{b}");
    assert_eq!(b["result"]["status"], "queued");
}

mod robot {
    use super::*;
    use robot_lab::Document;
    use robot_lab::document::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn local(dir: &std::path::Path) -> (robot_lab::Store, robot_lab::Reference) {
        let store = robot_lab::Store::open(dir, "openagents").unwrap();
        let source = store
            .put_asset(&b"<robot name=\"test-arm\"/>"[..], "application/xml", 1_024)
            .unwrap();
        let license = store
            .put_asset(&b"Original test fixture; CC0-1.0"[..], "text/plain", 1_024)
            .unwrap();
        let robot = Robot {
            name: "Test arm".into(),
            source: Source {
                repository: "https://github.com/OpenAgentsInc/openagents".into(),
                revision: "a".repeat(40),
                files: BTreeMap::from([("arm.urdf".into(), source)]),
                license: "CC0-1.0".into(),
                notices: vec![license],
                converter: "robot-lab-test-v1".into(),
            },
            description: "arm.urdf".into(),
            meters_per_unit: 1.0,
            source_axes: Axes::ZUp,
            links: vec![
                Link {
                    name: "base".into(),
                    inertial: None,
                    visuals: vec![],
                    collisions: vec![],
                },
                Link {
                    name: "forearm".into(),
                    inertial: Some(Inertial {
                        mass: 1.0,
                        center: [0.0; 3],
                        tensor: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
                    }),
                    visuals: vec![],
                    collisions: vec![],
                },
            ],
            joints: vec![Joint {
                name: "shoulder".into(),
                parent: "base".into(),
                child: "forearm".into(),
                kind: JointKind::Revolute,
                origin: Pose::default(),
                axis: [0.0, 0.0, 1.0],
                limits: Some(Limits {
                    lower: -1.0,
                    upper: 1.0,
                    effort: 3.0,
                    velocity: 2.0,
                }),
            }],
            capabilities: BTreeSet::from([Capability::Inspect, Capability::Simulate]),
        };
        let robot = store
            .save(
                "test-arm",
                None,
                Document::Robot(robot),
                "Test",
                "Added arm",
                1,
            )
            .unwrap();
        let scene = Scene {
            name: "Arm and cube".into(),
            robots: vec![RobotInstance {
                id: "arm".into(),
                robot,
                pose: Pose::default(),
                joints: BTreeMap::from([("shoulder".into(), 0.0)]),
            }],
            objects: vec![Object {
                id: "cube".into(),
                geometry: Geometry {
                    pose: Pose {
                        position: [0.2, 0.5, 0.0],
                        ..Pose::default()
                    },
                    shape: Shape::Box { size: [0.05; 3] },
                    rgba: [1.0, 0.0, 0.0, 1.0],
                },
                mesh: None,
                collider: Some(Shape::Box { size: [0.05; 3] }),
                mass: 0.05,
                friction: 0.6,
            }],
            lights: vec![],
            cameras: vec![Camera {
                id: "front".into(),
                pose: Pose::default(),
                width: 640,
                height: 480,
                intrinsics: [500.0, 500.0, 320.0, 240.0],
                robot_link: None,
            }],
        };
        let scene = store
            .save(
                "bench",
                None,
                Document::Scene(scene),
                "Test",
                "Added bench",
                2,
            )
            .unwrap();
        let build = Build {
            name: "test-v1".into(),
            sha256: "a".repeat(64),
        };
        let experiment = Experiment {
            scene: scene.clone(),
            task: CubeTask {
                version: build.clone(),
                object: "cube".into(),
                target_min: [0.0; 3],
                target_max: [1.0; 3],
                stable_seconds: 0.1,
                max_linear_speed: 0.01,
                max_angular_speed: 0.01,
            },
            controller: Controller::Scripted {
                build: build.clone(),
                robot: "arm".into(),
                joints: vec!["shoulder".into()],
            },
            backend: build,
            timestep_seconds: 0.01,
            control_every_steps: 2,
            sensor_every_steps: 5,
            seeds: vec![7],
            limits: ResourceLimits {
                max_steps: 100,
                wall_seconds: 10,
                max_artifact_bytes: 1_024,
                inference_timeout_ms: 100,
            },
        };
        let experiment = store
            .save(
                "experiment",
                None,
                Document::Experiment(experiment),
                "Test",
                "Added experiment",
                3,
            )
            .unwrap();
        (store, experiment)
    }

    #[tokio::test]
    async fn robot_items_check_their_references_and_a_finished_run_shows_its_replay() {
        let f = Fixture::new();
        let dir = tempfile::tempdir().unwrap();
        let (local, _) = local(dir.path());
        for item in robot_lab::migrate::plan(&local).unwrap() {
            let content = item.versions.last().unwrap().content.clone();
            f.create(&item.id, "robot", &item.schema, content).await;
        }
        // A pose is checked against the referenced robot's joints.
        let (s, b) = f
            .tool(
                "robot",
                "robot_pose",
                1,
                json!({"document": "bench", "robot": "arm", "joints": {"shoulder": 0.5}}),
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let (s, b) = f
            .tool(
                "robot",
                "robot_pose",
                2,
                json!({"document": "bench", "robot": "arm", "joints": {"elbow": 0.5}}),
            )
            .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
        assert_eq!(b["error"]["code"], "robot_check");

        // An item naming a version that isn't saved is refused.
        let experiment = f.store.version("experiment", 1).await.unwrap();
        let mut broken = experiment.content.clone();
        broken["content"]["scene"]["version"] = json!("b".repeat(64));
        let (s, b) = f
            .call(
                "POST",
                "/v1/labs/documents",
                json!({"id": "broken", "kind": "robot", "schema": "robot-experiment.v1",
                    "title": "Broken", "content": broken, "summary": "Created"}),
            )
            .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{b}");

        // Pins reach the scene and, through it, the robot.
        let pins = RobotApi.pins(&experiment.content).unwrap();
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].document, "bench");

        let (s, job) = f
            .tool(
                "robot",
                "robot_run",
                1,
                json!({"document": "experiment", "version": 1}),
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{job}");
        let job = job["result"]["id"].as_str().unwrap().to_owned();
        let (_, card) = f
            .call(
                "GET",
                "/v1/labs/documents/experiment/versions/1/card",
                Value::Null,
            )
            .await;
        assert_eq!(card["state"], "queued", "{card}");

        // A worker records the replay, as `robot-workbench simulate-job` returns it.
        let lease = f.store.claim(&job, "worker", now(), 30_000).await.unwrap();
        let lease = f.store.start(&lease, now()).await.unwrap();
        let artifact = f
            .store
            .put_job_artifact(
                &lease,
                ArtifactInput {
                    version: experiment.reference(),
                    role: ArtifactRole::Replay,
                    media_type: robot_lab::kind::REPLAY_MEDIA.into(),
                    class: lab_kit::AssetClass::Public,
                    no_training: false,
                },
                br#"{"schema":"robot-episode.v1"}"#.to_vec(),
                1 << 20,
                now(),
            )
            .await
            .unwrap();
        f.store
            .finish(
                &lease,
                JobStatus::Done,
                None,
                vec![artifact.id.clone()],
                now(),
            )
            .await
            .unwrap();
        let (s, card) = f
            .call(
                "GET",
                "/v1/labs/documents/experiment/versions/1/card",
                Value::Null,
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{card}");
        assert_eq!(card["view"], "report");
        assert_eq!(card["primary"], artifact.id);
        assert_eq!(card["detail"]["replay"], artifact.id);
        assert_eq!(
            card["state_text"],
            "The simulation finished. Its replay is saved."
        );
        let card: Card = serde_json::from_value(card).unwrap();
        let html = crate::lab_card_view::markup(&card, "me").into_string();
        assert!(html.contains("Download the replay"), "{html}");
    }
}
