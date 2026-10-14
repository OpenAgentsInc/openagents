//! The owner's loopback player. Mutations accept JSON from the same origin;
//! remote access belongs to the authenticated host adapter.
use crate::host::Host;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use robot_lab::Reference;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path as FilePath, PathBuf};
use std::sync::{Arc, Mutex};

pub struct Server {
    pub host: Host,
    pub build: PathBuf,
    robots: Mutex<BTreeMap<String, Arc<robot_description::Robot>>>,
    posters: Mutex<BTreeMap<String, robot_lab::document::Asset>>,
}

impl Server {
    pub fn new(host: Host, build: &FilePath) -> Self {
        Self {
            host,
            build: build.into(),
            robots: Mutex::new(BTreeMap::new()),
            posters: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route(
                "/api/tools",
                get(|| async { Json(crate::host::definitions()) }),
            )
            .route("/api/docs", get(documents))
            .route("/api/tool/{name}", post(tool))
            .route("/api/view", get(view))
            .route("/api/asset/{hash}", get(asset))
            .route("/api/poster", get(poster))
            .route("/build/{file}", get(build))
            .route("/play", get(play))
            .layer(DefaultBodyLimit::max(1 << 20))
            .with_state(self)
    }
    pub fn robot(
        &self,
        definition: &robot_lab::document::Robot,
    ) -> Result<Arc<robot_description::Robot>, String> {
        let key = robot_lab::store::digest(
            &serde_json::to_vec(&definition.source.files).map_err(|e| e.to_string())?,
        );
        let mut cache = self
            .robots
            .lock()
            .map_err(|_| "The robot cache couldn't be opened")?;
        if let Some(robot) = cache.get(&key) {
            return Ok(robot.clone());
        }
        let description = definition
            .source
            .files
            .get(&definition.description)
            .ok_or("This robot is missing its description")?;
        let bytes = self
            .host
            .store
            .asset_bytes(description, 1 << 20)
            .map_err(|e| e.to_string())?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "Use a UTF-8 robot description")?;
        let robot = robot_description::parse_with_reader(text, &|name| {
            let asset = definition.source.files.get(name).ok_or_else(|| {
                robot_description::Error::Read {
                    path: name.into(),
                    reason: "This mesh isn't saved with the robot".into(),
                }
            })?;
            self.host
                .store
                .asset_bytes(asset, robot_description::MAX_MESH_BYTES as usize)
                .map_err(|e| robot_description::Error::Read {
                    path: name.into(),
                    reason: e.to_string(),
                })
        })
        .map_err(|e| e.to_string())?;
        if cache.len() >= 4 {
            cache.clear();
        }
        let robot = Arc::new(robot);
        cache.insert(key, robot.clone());
        Ok(robot)
    }
    pub fn poster(&self, reference: &Reference) -> Result<robot_lab::document::Asset, String> {
        let bundle = self.host.bundle(reference)?;
        let key = robot_lab::store::digest(
            &serde_json::to_vec(&(
                reference,
                "robot-poster-v1",
                env!("ROBOT_RENDER_BUILD_SHA"),
                640,
                400,
                crate::view::Orbit::default(),
            ))
            .map_err(|e| e.to_string())?,
        );
        let mut posters = self
            .posters
            .lock()
            .map_err(|_| "The preview cache couldn't be opened")?;
        if let Some(asset) = posters.get(&key) {
            return Ok(asset.clone());
        }
        let directory = self.host.store.root().join("previews");
        if directory.exists()
            && !std::fs::symlink_metadata(&directory)
                .map_err(|e| e.to_string())?
                .is_dir()
        {
            return Err("Use an ordinary folder for robot previews".into());
        }
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let saved = directory.join(format!("{key}.json"));
        if saved.exists() {
            let metadata = std::fs::symlink_metadata(&saved).map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() > 4096 {
                return Err("This saved preview couldn't be opened".into());
            }
            let asset: robot_lab::document::Asset =
                serde_json::from_slice(&std::fs::read(&saved).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            self.host
                .store
                .verify_asset(&asset)
                .map_err(|e| e.to_string())?;
            if asset.media_type != "image/png" || asset.bytes > 8 << 20 {
                return Err("Choose a saved robot preview".into());
            }
            posters.insert(key, asset.clone());
            return Ok(asset);
        }
        let robot = self.robot(&bundle.robot)?;
        let scene = verse::robot_lab::document_scene(
            &robot,
            &bundle.scene,
            bundle.episode.as_ref().map(|e| &e.steps[0]),
        )?;
        let mut lit = scene.props;
        lit.extend(scene.robot);
        let world = verse::mesh::Mesh {
            lit,
            ..Default::default()
        };
        let (view_proj, eye) = crate::view::Orbit::default().matrices(640, 400);
        let dynamic = lighting();
        let capture = tempfile::NamedTempFile::with_suffix_in(".png", self.host.store.root())
            .map_err(|e| e.to_string())?;
        verse::render::capture_with_atmosphere(
            capture.path(),
            640,
            400,
            &world,
            verse::render::View { view_proj, eye },
            &dynamic,
            &verse::ui::UiBatch::default(),
            &verse::ui::Atlas::new(16.0),
            verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
        )?;
        let asset = self
            .host
            .store
            .put_asset(
                std::fs::File::open(capture.path()).map_err(|e| e.to_string())?,
                "image/png",
                8 << 20,
            )
            .map_err(|e| e.to_string())?;
        use std::io::Write;
        let mut index = tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
        index
            .write_all(&serde_json::to_vec(&asset).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        index.as_file().sync_all().map_err(|e| e.to_string())?;
        index.persist(saved).map_err(|e| e.error.to_string())?;
        std::fs::File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|e| e.to_string())?;
        if posters.len() >= 256 {
            posters.clear();
        }
        posters.insert(key, asset.clone());
        Ok(asset)
    }
}

pub fn lighting() -> verse::mesh::Mesh {
    let mut light = verse::pbr::Neon::neutral(0.0);
    light.key = Some(verse::pbr::Key {
        dir: glam::Vec3::new(0.45, 0.8, 0.4).normalize(),
        illuminance: 4200.0,
        angular_radius: 0.035,
        rim_dir: glam::Vec3::new(-0.5, 0.4, -0.75).normalize(),
        rim_illuminance: 1800.0,
        rim_angular_radius: 0.12,
        sky: 420.0,
        ground: 60.0,
        ev100: 10.0,
        shadow_center: glam::Vec3::new(0.1, 0.9, 0.0),
        shadow_half: 1.5,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    verse::mesh::Mesh {
        neon: Some(light),
        ..Default::default()
    }
}

#[derive(Deserialize)]
pub struct Subject {
    pub document: String,
    pub version: String,
}
impl Subject {
    fn reference(&self) -> Reference {
        Reference {
            document: self.document.clone(),
            version: self.version.clone(),
        }
    }
}
fn failure(message: String) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error":message}))).into_response()
}
async fn documents(State(server): State<Arc<Server>>) -> Response {
    match server.host.documents() {
        Ok(value) => Json(value).into_response(),
        Err(e) => failure(e),
    }
}
async fn view(State(server): State<Arc<Server>>, Query(subject): Query<Subject>) -> Response {
    match server.host.bundle(&subject.reference()) {
        Ok(value) => Json(value).into_response(),
        Err(e) => failure(e),
    }
}
async fn tool(
    State(server): State<Arc<Server>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(arguments): Json<Value>,
) -> Response {
    if let Some(origin) = headers.get(header::ORIGIN) {
        if origin.to_str().ok() != Some(&server.host.base) {
            return (StatusCode::FORBIDDEN, "Open this control in Robot Lab").into_response();
        }
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    match tokio::task::spawn_blocking(move || server.host.execute(&name, arguments, now)).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(e)) => failure(e),
        Err(_) => failure("The robot tool couldn't finish".into()),
    }
}
async fn poster(State(server): State<Arc<Server>>, Query(subject): Query<Subject>) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        let asset = server.poster(&subject.reference())?;
        let bytes = server
            .host
            .store
            .asset_bytes(&asset, 8 << 20)
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(bytes)
    })
    .await;
    match result {
        Ok(Ok(bytes)) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (
                    header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable",
                ),
            ],
            bytes,
        )
            .into_response(),
        Ok(Err(e)) => failure(e),
        Err(_) => failure("The preview couldn't be made".into()),
    }
}
async fn asset(State(server): State<Arc<Server>>, Path(hash): Path<String>) -> Response {
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return failure("Choose a saved robot file".into());
    }
    let path = server.host.store.root().join("assets").join(&hash);
    let result = tokio::task::spawn_blocking(move || {
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > 64 << 20 {
            return Err("Choose a smaller saved robot file".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if robot_lab::store::digest(&bytes) != hash {
            return Err("This saved robot file has changed".into());
        }
        Ok(bytes)
    })
    .await;
    match result {
        Ok(Ok(bytes)) => {
            let content_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                "image/png"
            } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
                "image/jpeg"
            } else {
                "application/octet-stream"
            };
            (
                [
                    (header::CONTENT_TYPE, content_type),
                    (
                        header::CACHE_CONTROL,
                        "private, max-age=31536000, immutable",
                    ),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                ],
                bytes,
            )
                .into_response()
        }
        Ok(Err(e)) => failure(e),
        Err(_) => failure("The robot file couldn't be opened".into()),
    }
}
async fn build(State(server): State<Arc<Server>>, Path(file): Path<String>) -> Response {
    let content_type = match file.as_str() {
        "robot_web.js" | "start.js" => "text/javascript",
        "robot_web_bg.wasm" => "application/wasm",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let path = server.build.join(file);
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(m) => m,
        Err(_) => return failure("Build the Robot Lab player before opening it".into()),
    };
    if !metadata.is_file() || metadata.len() > 128 << 20 {
        return failure("The player file couldn't be opened".into());
    }
    match tokio::fs::read(path).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, content_type),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => failure("The player file couldn't be opened".into()),
    }
}
async fn play() -> Html<&'static str> {
    Html(include_str!("player.html"))
}
