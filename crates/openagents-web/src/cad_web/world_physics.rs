//! Account-scoped CAD physics authority, persisted with conditional writes.
use super::*;
use axum::{Json, extract::DefaultBodyLimit};
use verse::zones::{
    cad_place,
    everglade::{
        self,
        cad_physics::sync::{Binding, Inputs, Object, Snapshot},
        demolition::site::Site,
    },
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    zone: String,
    client: String,
    sequence: u64,
    base: u64,
    bindings: Vec<Binding>,
    inputs: Inputs,
}
#[derive(Clone, Serialize, Deserialize)]
struct Control {
    sequence: u64,
    revision: u64,
    heard: u64,
    inputs: Inputs,
}
#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    snapshot: Snapshot,
    millis: u64,
    controls: BTreeMap<String, Control>,
}
struct Host {
    site: Site,
    models: Vec<cad_place::Placed>,
    saved: Saved,
    generation: Option<String>,
    index: Option<String>,
}
type Cache = HashMap<String, Arc<tokio::sync::Mutex<Option<Host>>>>;
fn hosts() -> &'static Mutex<Cache> {
    static HOSTS: OnceLock<Mutex<Cache>> = OnceLock::new();
    HOSTS.get_or_init(Mutex::default)
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
fn key(scope: &Scope, zone: &str) -> Result<String, String> {
    scope
        .key(&format!(
            "world/physics-{}.json",
            zone.trim_start_matches("verse-")
        ))
        .map_err(|e| e.to_string())
}
fn valid_zone(zone: &str) -> bool {
    matches!(
        zone,
        "verse-everglade" | "verse-meteor-showcase" | "verse-meteor-stress-test"
    )
}
fn bindings(models: &[cad_place::Placed]) -> Vec<Binding> {
    models
        .iter()
        .map(|p| Binding {
            id: p.placement.key().into(),
            doc: p.placement.doc.clone(),
            version: p.placement.version,
            hash: p.geometry.clone(),
            generation: p.placement.generation.clone(),
        })
        .collect()
}
async fn models(
    store: &Store,
    scope: &Scope,
    zone: &str,
    entries: &[Value],
) -> Result<Vec<cad_place::Placed>, String> {
    let mut out = Vec::new();
    let mut total = 0;
    let mut triangles = 0;
    let arrive = match zone {
        "verse-meteor-showcase" => verse::zones::meteor_showcase::SPAWN,
        "verse-meteor-stress-test" => verse::zones::meteor_stress::SPAWN,
        _ => cad_place::ahead(
            everglade::Everglade::spawn(),
            everglade::Everglade::spawn_yaw(),
        ),
    };
    let default = glam::Vec3::new(arrive.x, everglade::land(arrive.x, arrive.z), arrive.z);
    for entry in entries.iter().filter(|p| p["zone"] == zone) {
        if out.len() >= 20 {
            return Err("This world has too many models.".into());
        }
        let file = entry["file"]
            .as_str()
            .filter(|f| f.starts_with("world/") && !f.contains(".."))
            .ok_or("A model could not be opened.")?;
        let bytes = store
            .read_key(&scope.key(file).map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("A model could not be opened.")?
            .0;
        total += bytes.len();
        if total > 32 << 20 {
            return Err("This world's models are too large.".into());
        }
        let mut entry = entry.clone();
        entry["glb"] = json!(format!("/cad-world/{}", file.trim_start_matches("world/")));
        let placement = serde_json::from_value(entry.clone())
            .map_err(|_| "A placed model could not be read.")?;
        out.push(cad_place::stand_bytes(
            &placement,
            &bytes,
            everglade::land,
            default,
        )?);
        if entry["sha256"]
            .as_str()
            .is_some_and(|hash| hash != out.last().unwrap().geometry)
        {
            return Err(
                "This saved model has changed. Save a new version before placing it.".into(),
            );
        }
        triangles += out.last().unwrap().vertices.len() / 3;
        if triangles > cad_place::MAX_TRIANGLES {
            return Err("There are too many CAD triangles in this world.".into());
        }
    }
    Ok(out)
}
fn make(
    models: Vec<cad_place::Placed>,
    account: &str,
    saved: Option<Saved>,
    generation: Option<String>,
    index: Option<String>,
    zone: &str,
) -> Result<Host, String> {
    let mut site = Site::new(vec![], 1);
    if zone == "verse-everglade" {
        site.set_cad_terrain(everglade::height);
    }
    for p in &models {
        site.put_cad(
            p.placement.key(),
            &p.fingerprint,
            p.model.clone(),
            p.origin.as_dvec3(),
            f64::from(p.placement.heading_deg.to_radians()),
            f64::from(p.placement.scale),
        );
    }
    let current = bindings(&models);
    let mut old = saved.unwrap_or(Saved {
        snapshot: Snapshot {
            account: account.into(),
            revision: 0,
            objects: Vec::new(),
        },
        millis: now(),
        controls: BTreeMap::new(),
    });
    if old.snapshot.account != account {
        return Err("This world belongs to another account.".into());
    }
    let mut states = site.cad_states();
    for object in &old.snapshot.objects {
        if current.contains(&object.binding) {
            states.insert(object.binding.id.clone(), object.state.clone());
        }
    }
    site.apply_cad_states(&states, false)?;
    old.snapshot.objects = current
        .into_iter()
        .map(|binding| Object {
            state: states[&binding.id].clone(),
            binding,
        })
        .collect();
    Ok(Host {
        site,
        models,
        saved: old,
        generation,
        index,
    })
}
impl Host {
    fn advance(&mut self, request: Option<&Request>, time: u64) -> Result<(), String> {
        self.saved
            .controls
            .retain(|_, c| c.revision >= self.saved.snapshot.revision.saturating_sub(1024));
        if let Some(request) = request {
            if request.base > self.saved.snapshot.revision
                || request.base < self.saved.snapshot.revision.saturating_sub(1024)
            {
                return Err("This movement is older than the current world.".into());
            }
            let mut expected = bindings(&self.models);
            let mut received = request.bindings.clone();
            expected.sort_by(|a, b| a.id.cmp(&b.id));
            received.sort_by(|a, b| a.id.cmp(&b.id));
            if expected != received {
                return Err("These models have changed. Reload the world.".into());
            }
            if let Some(old) = self.saved.controls.get(&request.client) {
                if request.sequence < old.sequence {
                    return Err("This movement is older than the current world.".into());
                }
            }
            let duplicate = self
                .saved
                .controls
                .get(&request.client)
                .is_some_and(|old| old.sequence == request.sequence);
            if !duplicate {
                for b in &request.inputs.blasts {
                    self.site.explode(b.center, b.radius, 0, b.speed);
                }
                let mut input = request.inputs.clone();
                input.blasts.clear();
                if self.saved.controls.len() >= 2048
                    && !self.saved.controls.contains_key(&request.client)
                {
                    return Err(
                        "Too many world windows are connected. Close an older window.".into(),
                    );
                }
                self.saved.controls.insert(
                    request.client.clone(),
                    Control {
                        sequence: request.sequence,
                        revision: self.saved.snapshot.revision + 1,
                        heard: time,
                        inputs: input,
                    },
                );
            }
        }
        let dt = (time.saturating_sub(self.saved.millis) as f64 / 1000.).clamp(0., 0.25);
        let active: Vec<_> = self
            .saved
            .controls
            .values()
            .filter(|c| time.saturating_sub(c.heard) <= 60_000)
            .collect();
        let gravity = active.iter().rev().find_map(|c| c.inputs.forces.gravity);
        let winds = active
            .iter()
            .flat_map(|c| c.inputs.forces.winds.iter().cloned())
            .take(4)
            .collect::<Vec<_>>();
        let count = (dt * 60.).ceil() as usize;
        for _ in 0..count {
            let h = (dt / count as f64) as f32;
            self.site.cad_forces(
                gravity,
                winds.clone(),
                glam::Vec3::ZERO,
                glam::Vec3::ZERO,
                h,
            );
            for c in active
                .iter()
                .filter(|c| time.saturating_sub(c.heard) <= 500)
            {
                self.site.cad_forces(
                    gravity,
                    winds.clone(),
                    c.inputs.forces.player,
                    c.inputs.forces.push,
                    h,
                );
            }
            self.site.tick(h);
        }
        self.saved.millis = time;
        self.saved.snapshot.revision = self
            .saved
            .snapshot
            .revision
            .checked_add(1)
            .ok_or("This world needs to be reopened.")?;
        let states = self.site.cad_states();
        self.saved.snapshot.objects = bindings(&self.models)
            .into_iter()
            .map(|binding| Object {
                state: states[&binding.id].clone(),
                binding,
            })
            .collect();
        Ok(())
    }
}
async fn exchange(
    store: &Store,
    scope: &Scope,
    zone: &str,
    request: Option<Request>,
) -> Result<Snapshot, (StatusCode, String)> {
    let fail = |e: String| (StatusCode::SERVICE_UNAVAILABLE, e);
    if !valid_zone(zone) {
        return Err((StatusCode::BAD_REQUEST, "Choose a CAD world.".into()));
    }
    if request.as_ref().is_some_and(|r| {
        r.zone != zone
            || r.client.len() != 32
            || !r.client.bytes().all(|b| b.is_ascii_hexdigit())
            || r.sequence == 0
            || r.bindings.len() > 20
            || !r.inputs.valid()
    }) {
        return Err((
            StatusCode::BAD_REQUEST,
            "This movement could not be read.".into(),
        ));
    }
    let file = key(scope, zone).map_err(fail)?;
    let slot = {
        let mut map = hosts().lock().unwrap();
        if map.len() >= 16 && !map.contains_key(&file) {
            let old = map.keys().next().cloned().unwrap();
            map.remove(&old);
        }
        map.entry(file.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(None)))
            .clone()
    };
    let mut cache = slot.lock().await;
    for _ in 0..4 {
        let index = store
            .read_key(
                &scope
                    .key("world/index.json")
                    .map_err(|e| fail(e.to_string()))?,
            )
            .await
            .map_err(|e| fail(e.to_string()))?;
        let index_gen = index.as_ref().map(|p| p.1.clone());
        let record = store
            .read_key(&file)
            .await
            .map_err(|e| fail(e.to_string()))?;
        let generation = record.as_ref().map(|p| p.1.clone());
        if cache
            .as_ref()
            .is_none_or(|h| h.generation != generation || h.index != index_gen)
        {
            let entries = index
                .map(|p| serde_json::from_slice::<Vec<Value>>(&p.0))
                .transpose()
                .map_err(|_| fail("This world's models could not be read.".into()))?
                .unwrap_or_default();
            let models = models(store, scope, zone, &entries).await.map_err(fail)?;
            let saved = record
                .map(|p| serde_json::from_slice::<Saved>(&p.0))
                .transpose()
                .map_err(|_| fail("This world's movement could not be read.".into()))?;
            *cache = Some(
                make(
                    models,
                    &scope.actor,
                    saved,
                    generation.clone(),
                    index_gen,
                    zone,
                )
                .map_err(fail)?,
            );
        }
        let mut moved = cache.take().unwrap();
        let command = request.clone();
        let (moved, result) = tokio::task::spawn_blocking(move || {
            let result = moved.advance(command.as_ref(), now());
            (moved, result)
        })
        .await
        .map_err(|_| fail("This world's movement could not be updated.".into()))?;
        *cache = Some(moved);
        result.map_err(|e| (StatusCode::CONFLICT, e))?;
        let host = cache.as_mut().unwrap();
        let bytes = serde_json::to_vec(&host.saved).map_err(|e| fail(e.to_string()))?;
        match store.write_key(&file, bytes, generation.as_deref()).await {
            Ok(generation) => {
                host.generation = Some(generation);
                return Ok(host.saved.snapshot.clone());
            }
            Err(StoreError::Conflict) => {
                *cache = None;
                continue;
            }
            Err(_) => {
                *cache = None;
                return Err(fail("This world's movement could not be saved.".into()));
            }
        }
    }
    Err((
        StatusCode::CONFLICT,
        "This world changed in another window. Try again.".into(),
    ))
}
#[derive(Deserialize)]
struct QueryZone {
    zone: String,
}
async fn get_world(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<QueryZone>,
) -> Response {
    respond(&app, &headers, q.zone, None).await
}
async fn update(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> Response {
    respond(&app, &headers, request.zone.clone(), Some(request)).await
}
async fn respond(
    app: &App,
    headers: &HeaderMap,
    zone: String,
    request: Option<Request>,
) -> Response {
    let Some(scope) = page_scope(app, headers, None).await else {
        return (StatusCode::UNAUTHORIZED, "Log in to open your world.").into_response();
    };
    let mut response = match exchange(&app.config.chat_store, &scope, &zone, request).await {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(e) => e.into_response(),
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cad-world/physics", get(get_world).post(update))
        .layer(DefaultBodyLimit::max(64 * 1024))
}

#[cfg(test)]
mod tests;
