//! Explicit policy endpoints exchange bounded observations and action chunks.
//! The manifest pins the checkpoint and mappings; it is a declaration by the
//! configured server, not remote attestation or physical execution authority.
use crate::{
    Error, Result,
    artifacts::{Artifact, Binding, digest, read},
    controller::{Engine, Input, NativeController},
    require,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use robot_data::dataset::FilePin;
use robot_lab::{
    Store,
    document::{Build, Experiment, ObjectState, Robot, Scene, Sensor},
};
use robot_sim::runner::{ActionChunk, Controller, Observation, Snapshot};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
const LIMIT: usize = 8 << 20;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub build: Build,
    pub checkpoint: Build,
    pub family: String,
    pub binding: Binding,
    pub weights: FilePin,
    pub normalization: FilePin,
    pub state_dim: usize,
    pub action_dim: usize,
    pub environment_dim: usize,
    pub chunk_size: usize,
}
impl Manifest {
    pub fn compute_build(&self) -> Result<Build> {
        Ok(Build {
            name: "robot-policy-service-v1".into(),
            sha256: digest(&serde_json::to_vec(&(
                &self.schema,
                &self.checkpoint,
                &self.family,
                &self.binding,
                &self.weights,
                &self.normalization,
                self.state_dim,
                self.action_dim,
                self.environment_dim,
                self.chunk_size,
                digest(include_bytes!("service.rs")),
                "robot-policy-http-v1",
            ))?),
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.checkpoint
            .validate()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        self.binding.validate(
            self.state_dim,
            self.action_dim,
            self.environment_dim,
            self.binding.cameras.len(),
            self.chunk_size,
        )?;
        require(
            self.schema == "robot-policy-service.v1"
                && matches!(self.family.as_str(), "act" | "smolvla")
                && self.build == self.compute_build()?
                && (1..=64).contains(&self.state_dim)
                && (1..=64).contains(&self.action_dim)
                && self.environment_dim <= 64
                && (1..=128).contains(&self.chunk_size),
            "Choose a supported policy service and checked manifest",
        )?;
        for pin in [&self.weights, &self.normalization] {
            require(
                pin.bytes > 0
                    && pin.bytes <= 2 << 30
                    && pin.sha256.len() == 64
                    && pin.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "Pin the service's checkpoint and normalization files",
            )?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub schema: String,
    pub endpoint: String,
    pub manifest: String,
    pub manifest_sha256: String,
    pub bearer_env: String,
}
pub struct Configured {
    pub pin: Pin,
    pub manifest: Manifest,
    pub manifest_bytes: Vec<u8>,
    token: String,
    client: reqwest::blocking::Client,
}
impl Configured {
    pub fn open(root: &Path, expected: &str) -> Result<Self> {
        require(
            root.is_absolute(),
            "Choose an absolute policy-service configuration folder",
        )?;
        let root = root.canonicalize()?;
        require(
            !root.ancestors().any(|p| p.join(".git").exists()),
            "Keep private policy settings outside the Git checkout",
        )?;
        let bytes = read(&root, "policy.json", 65536)?;
        require(
            digest(&bytes) == expected,
            "The policy-service settings changed. Review them before running",
        )?;
        let pin: Pin = serde_json::from_slice(&bytes)?;
        let endpoint = url::Url::parse(&pin.endpoint)
            .map_err(|_| Error::Invalid("Choose the configured policy service's address".into()))?;
        require(
            pin.schema == "robot-policy-service-client.v1"
                && endpoint.username().is_empty()
                && endpoint.password().is_none()
                && endpoint.query().is_none()
                && endpoint.fragment().is_none()
                && endpoint.path() == "/"
                && (endpoint.scheme() == "https"
                    || (endpoint.scheme() == "http"
                        && matches!(
                            endpoint.host_str(),
                            Some("127.0.0.1" | "localhost" | "[::1]")
                        )))
                && pin.bearer_env.starts_with("ROBOT_POLICY_")
                && pin.bearer_env.len() <= 96
                && pin
                    .bearer_env
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
            "Use HTTPS or a local policy service and its dedicated credential",
        )?;
        let manifest_bytes = read(&root, &pin.manifest, 2 << 20)?;
        require(
            digest(&manifest_bytes) == pin.manifest_sha256,
            "The saved service manifest changed",
        )?;
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
        manifest.validate()?;
        let token = std::env::var(&pin.bearer_env).map_err(|_| {
            Error::Invalid("Set the configured policy-service credential before starting".into())
        })?;
        validate_token(&token)?;
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(3))
            .no_proxy()
            .build()
            .map_err(|_| Error::Invalid("The policy connection couldn't be prepared".into()))?;
        Ok(Self {
            pin,
            manifest,
            manifest_bytes,
            token,
            client,
        })
    }
    pub fn controller(
        self,
        store: Store,
        robot: &Robot,
        scene: &Scene,
        experiment: &Experiment,
    ) -> Result<NativeController> {
        self.manifest
            .binding
            .validate_scene(robot, scene, experiment)?;
        let robot_lab::document::Controller::Service {
            build,
            manifest,
            robot,
            joints,
            cameras,
            instruction,
        } = &experiment.controller
        else {
            return Err(Error::Invalid(
                "Choose a saved policy-service experiment".into(),
            ));
        };
        require(
            build == &self.manifest.build
                && manifest.sha256 == self.pin.manifest_sha256
                && manifest.bytes == self.manifest_bytes.len() as u64
                && robot == &scene.robots[0].id
                && joints.iter().collect::<std::collections::BTreeSet<_>>()
                    == self
                        .manifest
                        .binding
                        .actions
                        .joints
                        .iter()
                        .map(|j| &j.joint)
                        .collect()
                && cameras.iter().collect::<std::collections::BTreeSet<_>>()
                    == self
                        .manifest
                        .binding
                        .cameras
                        .iter()
                        .map(|c| &c.id)
                        .collect()
                && instruction == &self.manifest.binding.instruction,
            "Use the exact service and input mappings saved with this experiment",
        )?;
        store
            .verify_asset(manifest)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let build = self.manifest.build.clone();
        NativeController::worker(
            Arc::new(Remote {
                configured: self,
                store,
                max_steps: experiment.limits.max_steps,
            }),
            build,
            experiment.seeds.clone(),
        )
    }
}
struct Remote {
    configured: Configured,
    store: Store,
    max_steps: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub sensor: Sensor,
    pub png_base64: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub manifest_sha256: String,
    pub observation_step: u32,
    pub seed: u64,
    pub timeout_ms: u32,
    pub joints: BTreeMap<String, f64>,
    pub objects: BTreeMap<String, ObjectState>,
    pub images: BTreeMap<String, Image>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Actions {
        build: Build,
        chunk: ActionChunk,
        inference_ms: f64,
    },
    Refused {
        reason: String,
    },
}
impl Engine for Remote {
    fn infer(&self, input: &Input) -> Result<ActionChunk> {
        let binding = &self.configured.manifest.binding;
        require(
            input.step < self.max_steps && input.step.is_multiple_of(binding.control_every_steps),
            "Use a scheduled policy observation",
        )?;
        let mut images = BTreeMap::new();
        for camera in &binding.cameras {
            let sensor = input
                .sensors
                .get(&camera.id)
                .ok_or_else(|| Error::Invalid("A required policy camera is missing".into()))?;
            let bytes = self
                .store
                .asset_bytes(&sensor.asset, 2 << 20)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            images.insert(
                camera.id.clone(),
                Image {
                    sensor: sensor.clone(),
                    png_base64: STANDARD.encode(bytes),
                },
            );
        }
        let request = Request {
            schema: "robot-policy-observation.v1".into(),
            manifest_sha256: self.configured.pin.manifest_sha256.clone(),
            observation_step: input.step,
            seed: input.seed,
            timeout_ms: input.timeout_ms,
            joints: input.state.joints.clone(),
            objects: input.state.objects.clone(),
            images,
        };
        let bytes = serde_json::to_vec(&request)?;
        require(bytes.len() <= LIMIT, "Use a smaller policy observation")?;
        let base = self.configured.pin.endpoint.trim_end_matches('/');
        let get = self
            .configured
            .client
            .get(format!("{base}/v1/policy"))
            .bearer_auth(&self.configured.token)
            .timeout(Duration::from_millis(u64::from(input.timeout_ms)))
            .send()
            .map_err(|_| Error::Invalid("The policy service didn't answer in time".into()))?;
        let manifest = response(get, 2 << 20)?;
        require(
            digest(&manifest) == self.configured.pin.manifest_sha256,
            "The policy service changed its checkpoint. Stop and review it",
        )?;
        let reply = self
            .configured
            .client
            .post(format!("{base}/v1/policy/infer"))
            .bearer_auth(&self.configured.token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(bytes)
            .timeout(Duration::from_millis(u64::from(input.timeout_ms)))
            .send()
            .map_err(|_| Error::Invalid("The policy service didn't answer in time".into()))?;
        let reply: Reply = serde_json::from_slice(&response(reply, 1 << 20)?)?;
        match reply {
            Reply::Refused { reason } => {
                require(
                    reason.len() <= 512,
                    "The policy service returned an invalid refusal",
                )?;
                Err(Error::Invalid(format!(
                    "The policy service refused this observation: {reason}"
                )))
            }
            Reply::Actions {
                build,
                chunk,
                inference_ms,
            } => {
                require(
                    build == self.configured.manifest.build
                        && chunk.observation_step == input.step
                        && !chunk.actions.is_empty()
                        && chunk.actions.len() <= binding.action_steps
                        && chunk.expires_step
                            == input.step
                                + (chunk.actions.len() as u32 - 1) * binding.control_every_steps
                        && chunk.actions.iter().all(|a| {
                            a.len() == binding.actions.joints.len()
                                && binding.actions.joints.iter().all(|joint| {
                                    a.get(&joint.joint).is_some_and(|v| {
                                        v.is_finite() && *v >= joint.lower && *v <= joint.upper
                                    })
                                })
                        })
                        && inference_ms.is_finite()
                        && (0. ..=300000.).contains(&inference_ms),
                    "The policy service returned an invalid or outdated action chunk",
                )?;
                Ok(chunk)
            }
        }
    }
}
fn response(mut response: reqwest::blocking::Response, limit: usize) -> Result<Vec<u8>> {
    require(
        response.status().is_success(),
        "The policy service refused this request",
    )?;
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= limit,
        "The policy service returned too much data",
    )?;
    Ok(bytes)
}
fn validate_token(token: &str) -> Result<()> {
    require(
        (24..=256).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_graphic()),
        "Use a dedicated policy-service credential of 24 to 256 characters",
    )
}

pub struct Server {
    manifest: Manifest,
    bytes: Vec<u8>,
    token_sha256: String,
    controller: Mutex<NativeController>,
    store: Store,
    max_steps: u32,
}
impl Server {
    pub fn act(
        artifact: Artifact,
        store: Store,
        robot: &Robot,
        scene: &Scene,
        experiment: &Experiment,
        token: String,
    ) -> Result<Self> {
        let p = &artifact.pin;
        let manifest = Manifest {
            schema: "robot-policy-service.v1".into(),
            build: Build {
                name: "pending".into(),
                sha256: "0".repeat(64),
            },
            checkpoint: artifact.build.clone(),
            family: "act".into(),
            binding: p.binding.clone(),
            weights: p.files[&p.weights].clone(),
            normalization: p.files[&p.normalization].clone(),
            state_dim: p.architecture.state_dim,
            action_dim: p.architecture.action_dim,
            environment_dim: p.architecture.environment_dim,
            chunk_size: p.architecture.chunk_size,
        };
        let controller = NativeController::new(artifact, store.clone(), robot, scene, experiment)?;
        Self::new(
            manifest,
            controller,
            store,
            experiment.limits.max_steps,
            token,
        )
    }
    pub fn smol(
        artifact: crate::smol_artifacts::Artifact,
        store: Store,
        robot: &Robot,
        scene: &Scene,
        experiment: &Experiment,
        token: String,
    ) -> Result<Self> {
        let p = &artifact.pin;
        let manifest = Manifest {
            schema: "robot-policy-service.v1".into(),
            build: Build {
                name: "pending".into(),
                sha256: "0".repeat(64),
            },
            checkpoint: artifact.build.clone(),
            family: "smolvla".into(),
            binding: p.binding.clone(),
            weights: p.files[&p.weights].clone(),
            normalization: p.files[&p.normalization].clone(),
            state_dim: p.binding.state.joints.len(),
            action_dim: p.binding.actions.joints.len(),
            environment_dim: 0,
            chunk_size: p.architecture.chunk_size,
        };
        let controller =
            NativeController::new_smol(artifact, store.clone(), robot, scene, experiment)?;
        Self::new(
            manifest,
            controller,
            store,
            experiment.limits.max_steps,
            token,
        )
    }
    fn new(
        mut manifest: Manifest,
        controller: NativeController,
        store: Store,
        max_steps: u32,
        token: String,
    ) -> Result<Self> {
        validate_token(&token)?;
        manifest.build = manifest.compute_build()?;
        manifest.validate()?;
        let bytes = serde_json::to_vec(&manifest)?;
        Ok(Self {
            manifest,
            bytes,
            token_sha256: digest(token.as_bytes()),
            controller: Mutex::new(controller),
            store,
            max_steps,
        })
    }
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.bytes
    }
    fn authorized(&self, headers: &axum::http::HeaderMap) -> bool {
        let Some(token) = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
        else {
            return false;
        };
        let actual = digest(token.as_bytes());
        actual
            .bytes()
            .zip(self.token_sha256.bytes())
            .fold(0_u8, |v, (a, b)| v | (a ^ b))
            == 0
    }
    pub fn infer(&self, request: Request) -> Result<Reply> {
        require(
            request.schema == "robot-policy-observation.v1"
                && request.manifest_sha256 == digest(&self.bytes)
                && request.observation_step < self.max_steps
                && request
                    .observation_step
                    .is_multiple_of(self.manifest.binding.control_every_steps)
                && (1..=30000).contains(&request.timeout_ms)
                && request.joints.len() == self.manifest.state_dim
                && request.objects.len() <= 64
                && request.images.len() == self.manifest.binding.cameras.len(),
            "Use the policy service's checked observation layout",
        )?;
        require(
            request
                .joints
                .iter()
                .all(|(n, v)| n.len() <= 96 && v.is_finite() && v.abs() <= 1e6)
                && request.objects.values().all(|o| {
                    o.pose.validate().is_ok()
                        && o.linear_velocity
                            .iter()
                            .chain(&o.angular_velocity)
                            .all(|v| v.is_finite() && v.abs() <= 1e6)
                }),
            "Use finite robot observations",
        )?;
        let mut sensors = BTreeMap::new();
        for camera in &self.manifest.binding.cameras {
            let image = request
                .images
                .get(&camera.id)
                .ok_or_else(|| Error::Invalid("A required policy camera is missing".into()))?;
            require(
                image.png_base64.len() <= 3 << 20
                    && image.sensor.asset.media_type == "image/png"
                    && image.sensor.timestamp_seconds.is_finite()
                    && image.sensor.timestamp_seconds >= 0.
                    && image.sensor.timestamp_seconds
                        <= f64::from(request.observation_step)
                            * self.manifest.binding.timestep_seconds
                            + 1e-9
                    && f64::from(request.observation_step) * self.manifest.binding.timestep_seconds
                        - image.sensor.timestamp_seconds
                        <= f64::from(camera.max_age_steps) * self.manifest.binding.timestep_seconds
                            + 1e-9,
                "Use bounded PNG policy camera images",
            )?;
            let bytes = STANDARD
                .decode(&image.png_base64)
                .map_err(|_| Error::Invalid("A policy camera image couldn't be read".into()))?;
            require(
                bytes.len() <= 2 << 20
                    && bytes.len() as u64 == image.sensor.asset.bytes
                    && digest(&bytes) == image.sensor.asset.sha256,
                "A policy camera image changed",
            )?;
            robot_data::export::rgb(&bytes, camera.source)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            self.store
                .put_asset(std::io::Cursor::new(bytes), "image/png", 2 << 20)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            sensors.insert(camera.id.clone(), image.sensor.clone());
        }
        let mut controller = self.controller.try_lock().map_err(|_| {
            Error::Invalid("The policy service is busy. Try this observation again".into())
        })?;
        controller
            .reset(request.seed)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let state = Snapshot {
            joints: request.joints,
            objects: request.objects,
            ..Default::default()
        };
        let started = std::time::Instant::now();
        let chunk = controller
            .infer(Observation {
                step: request.observation_step,
                seed: request.seed,
                state: &state,
                sensors: &sensors,
                timeout_ms: request.timeout_ms,
            })
            .map_err(|e| Error::Invalid(e.to_string()))?;
        Ok(Reply::Actions {
            build: self.manifest.build.clone(),
            chunk,
            inference_ms: started.elapsed().as_secs_f64() * 1000.,
        })
    }
    pub fn router(self: Arc<Self>) -> axum::Router {
        axum::Router::new()
            .route("/v1/policy", axum::routing::get(manifest_route))
            .route("/v1/policy/infer", axum::routing::post(infer_route))
            .layer(axum::extract::DefaultBodyLimit::max(LIMIT))
            .with_state(self)
    }
}
async fn manifest_route(
    axum::extract::State(server): axum::extract::State<Arc<Server>>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !server.authorized(&headers) {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            "Use this service's configured credential",
        )
            .into_response();
    }
    (
        [
            (axum::http::header::CONTENT_TYPE, "application/json"),
            (axum::http::header::CACHE_CONTROL, "no-store"),
        ],
        server.bytes.clone(),
    )
        .into_response()
}
async fn infer_route(
    axum::extract::State(server): axum::extract::State<Arc<Server>>,
    headers: axum::http::HeaderMap,
    axum::Json(request): axum::Json<Request>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !server.authorized(&headers) {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            "Use this service's configured credential",
        )
            .into_response();
    }
    let result = tokio::task::spawn_blocking(move || server.infer(request)).await;
    let reply = match result {
        Ok(Ok(reply)) => reply,
        Ok(Err(error)) => Reply::Refused {
            reason: error.to_string().chars().take(512).collect(),
        },
        Err(_) => Reply::Refused {
            reason: "The policy couldn't answer this observation".into(),
        },
    };
    axum::Json(reply).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::{Camera, Source};
    use robot_data::mapping::{JointMapping, Mapping, Unit};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Echo {
        stale: bool,
        delay_ms: u64,
        calls: Arc<AtomicUsize>,
    }
    impl Engine for Echo {
        fn infer(&self, input: &Input) -> Result<ActionChunk> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(self.delay_ms));
            let step = if self.stale {
                input.step + 1
            } else {
                input.step
            };
            Ok(ActionChunk {
                observation_step: step,
                expires_step: step,
                actions: vec![BTreeMap::from([("shoulder".into(), 0.2)])],
            })
        }
    }
    fn fixture(
        stale: bool,
    ) -> (
        tempfile::TempDir,
        Arc<Server>,
        Configured,
        Store,
        Arc<AtomicUsize>,
    ) {
        timed_fixture(stale, 0)
    }
    fn timed_fixture(
        stale: bool,
        delay_ms: u64,
    ) -> (
        tempfile::TempDir,
        Arc<Server>,
        Configured,
        Store,
        Arc<AtomicUsize>,
    ) {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(root.path(), "service-tests").unwrap();
        let token = digest(root.path().to_string_lossy().as_bytes());
        let mapping = Mapping {
            schema: "robot-joint-mapping.v1".into(),
            robot_type: "fixture".into(),
            robot_version: "a".repeat(64),
            joints: vec![JointMapping {
                feature: "shoulder.pos".into(),
                joint: "shoulder".into(),
                unit: Unit::Radians,
                offset: 0.,
                direction: 1.,
                lower: -1.,
                upper: 1.,
            }],
        };
        let binding = Binding {
            robot_type: "fixture".into(),
            robot_description_sha256: "a".repeat(64),
            task: Build {
                name: "cube-task".into(),
                sha256: "b".repeat(64),
            },
            task_definition_sha256: "c".repeat(64),
            instruction: "Move the cube".into(),
            state: mapping.clone(),
            actions: mapping,
            environment: vec![],
            cameras: vec![Camera {
                id: "front".into(),
                source: [144, 96],
                target: [32, 32],
                definition_sha256: "d".repeat(64),
                max_age_steps: 0,
            }],
            timestep_seconds: 1. / 240.,
            control_every_steps: 8,
            action_steps: 1,
        };
        let build = Build {
            name: "protocol-fixture".into(),
            sha256: "e".repeat(64),
        };
        let manifest = Manifest {
            schema: "robot-policy-service.v1".into(),
            build: build.clone(),
            checkpoint: build.clone(),
            family: "act".into(),
            binding,
            weights: FilePin {
                sha256: "f".repeat(64),
                bytes: 1,
            },
            normalization: FilePin {
                sha256: "0".repeat(64),
                bytes: 1,
            },
            state_dim: 1,
            action_dim: 1,
            environment_dim: 0,
            chunk_size: 1,
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let controller = NativeController::worker(
            Arc::new(Echo {
                stale,
                delay_ms,
                calls: calls.clone(),
            }),
            build,
            vec![7],
        )
        .unwrap();
        let server =
            Arc::new(Server::new(manifest, controller, store.clone(), 241, token.clone()).unwrap());
        let configured = Configured {
            pin: Pin {
                schema: "robot-policy-service-client.v1".into(),
                endpoint: "http://127.0.0.1/".into(),
                manifest: "manifest.json".into(),
                manifest_sha256: digest(&server.bytes),
                bearer_env: "ROBOT_POLICY_TEST_TOKEN".into(),
            },
            manifest: server.manifest.clone(),
            manifest_bytes: server.bytes.clone(),
            token,
            client: reqwest::blocking::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        };
        (root, server, configured, store, calls)
    }
    fn serve(
        server: Arc<Server>,
    ) -> (
        String,
        tokio::sync::oneshot::Sender<()>,
        std::thread::JoinHandle<()>,
    ) {
        let (ready, receive) = std::sync::mpsc::channel();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                        .await
                        .unwrap();
                    ready
                        .send(format!(
                            "http://127.0.0.1:{}/",
                            listener.local_addr().unwrap().port()
                        ))
                        .unwrap();
                    axum::serve(listener, server.router())
                        .with_graceful_shutdown(async {
                            let _ = stopped.await;
                        })
                        .await
                        .unwrap();
                });
        });
        (receive.recv().unwrap(), stop, thread)
    }
    fn input(store: &Store) -> Input {
        let bytes = include_bytes!("../fixtures/camera-pattern.png");
        let asset = store
            .put_asset(std::io::Cursor::new(bytes), "image/png", 2 << 20)
            .unwrap();
        Input {
            step: 0,
            seed: 7,
            timeout_ms: 1000,
            state: Snapshot {
                joints: BTreeMap::from([("shoulder".into(), 0.)]),
                ..Default::default()
            },
            sensors: BTreeMap::from([(
                "front".into(),
                Sensor {
                    asset,
                    timestamp_seconds: 0.,
                    frame: 0,
                },
            )]),
        }
    }
    #[test]
    fn authenticated_service_uses_the_exact_manifest_and_rejects_late_actions() {
        for stale in [false, true] {
            let (_root, server, mut configured, store, calls) = fixture(stale);
            let (url, stop, thread) = serve(server);
            configured.pin.endpoint = url;
            let remote = Remote {
                configured,
                store: store.clone(),
                max_steps: 241,
            };
            let result = remote.infer(&input(&store));
            if stale {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().actions[0]["shoulder"], 0.2);
            }
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            let _ = stop.send(());
            thread.join().unwrap();
        }
    }
    #[test]
    fn wrong_authentication_and_changed_manifest_cannot_reach_inference() {
        let (_root, server, mut configured, store, calls) = fixture(false);
        let (url, stop, thread) = serve(server);
        configured.pin.endpoint = url;
        let correct_token = configured.token.clone();
        configured.token = "wrong dedicated credential".into();
        let mut remote = Remote {
            configured,
            store: store.clone(),
            max_steps: 241,
        };
        assert!(remote.infer(&input(&store)).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        remote.configured.token = correct_token;
        remote.configured.pin.manifest_sha256 = "a".repeat(64);
        assert!(remote.infer(&input(&store)).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let _ = stop.send(());
        thread.join().unwrap();
    }
    #[test]
    fn a_service_timeout_stops_the_controller_and_cannot_return_late_actions() {
        let (_root, server, mut configured, store, _) = timed_fixture(false, 50);
        let (url, stop, thread) = serve(server);
        configured.pin.endpoint = url;
        let remote = Remote {
            configured,
            store: store.clone(),
            max_steps: 241,
        };
        let mut input = input(&store);
        input.timeout_ms = 10;
        let started = std::time::Instant::now();
        assert!(remote.infer(&input).is_err());
        assert!(started.elapsed() < Duration::from_millis(150));
        std::thread::sleep(Duration::from_millis(70));
        input.timeout_ms = 1000;
        assert!(remote.infer(&input).is_err());
        let _ = stop.send(());
        thread.join().unwrap();
    }
    #[test]
    fn source_checks_and_missing_camera_data_are_plain_refusals() {
        let (_root, server, configured, store, _) = fixture(false);
        let mut manifest = server.manifest.clone();
        manifest.state_dim = 0;
        assert!(manifest.validate().is_err());
        let request = Request {
            schema: "robot-policy-observation.v1".into(),
            manifest_sha256: configured.pin.manifest_sha256,
            observation_step: 0,
            seed: 7,
            timeout_ms: 1000,
            joints: BTreeMap::from([("shoulder".into(), 0.)]),
            objects: BTreeMap::new(),
            images: BTreeMap::new(),
        };
        assert!(server.infer(request).is_err());
        assert!(!input(&store).sensors.is_empty());
        assert!(
            Source {
                repository: "https://example.com".into(),
                revision: "x".into(),
                license: "Apache-2.0".into(),
                purpose: "Fixture".into(),
                training_evidence: serde_json::json!({})
            }
            .validate()
            .is_err()
        );
    }
}
