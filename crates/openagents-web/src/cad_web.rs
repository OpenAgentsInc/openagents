//! CAD Lab on openagents.com (#11653): the `cad_*` tools for signed-in
//! people in any web chat, with no CAD Lab running on a Mac, documents and
//! versions kept per account or per project, and the `/cad/{doc}` page.
//!
//! **Where it runs.** A chat turn's tools run on this server, because the
//! turn runs here (a phone may have sent the message and closed): each call
//! starts the CAD engine (`cad-lab tool NAME -`, the same tools and checks
//! as the Lab on a Mac) as a child process with a deadline and a memory
//! limit, one at a time, over a working copy of the scope's documents.
//! The browser runs the engine where it can (`crates/cad-wasm`): the
//! `/cad/{doc}` page makes STEP, STL, 3MF and drawing files on the person's
//! own device from the stored document.
//!
//! **Where it's kept.** In the private chat store (the chat bucket in
//! production), under the account's folder, or the project's home
//! account's folder for a chat in a project (`projects::api::home_of`):
//!
//! ```text
//! {owner}/cad/me/index.json                 the documents and their versions
//! {owner}/cad/p-{project}/index.json        a project's
//! {...}/{doc}/v{N}/doc.json                 the version's document
//! {...}/{doc}/v{N}/version.json             its summary, author and check report
//! {...}/{doc}/v{N}/view.json                the viewer's model, with face names
//! {...}/{doc}/v{N}/model.glb, hero.png      its glTF and its picture
//! {...}/{doc}/v{N}/out/{file}               renders, drawings and exports
//! ```
//!
//! A version's `doc.json` is written only when nothing is there, so two
//! writers can't both make version N; the index is replaced by generation.
//!
//! **Who may.** Viewers of a project read its models; editors and the owner
//! change them. Tools that change a model are refused for a viewer before
//! anything runs.
//!
//! **Limits.** Arguments up to [`MAX_ARGS_BYTES`], a document up to
//! [`MAX_DOC_BYTES`], [`MAX_DOCS`] documents a scope and [`MAX_VERSIONS`]
//! versions a document, one rebuild at a time on this server, each within
//! [`DEADLINE`] and [`MEMORY_BYTES`], and [`RUNS_PER_HOUR`] tool runs per
//! person.

mod api;
pub(crate) use api::CadApi;
mod configurator;
mod handoff;
mod imports;
mod platform;
mod sales;
mod world_physics;
#[cfg(test)]
pub(crate) use sales::tests::fake_wallet as fake_sale_wallet;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{Form, Path as UrlPath, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonLink, ButtonType, ButtonVariant, Color};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::App;
use crate::chat_store::{Error as StoreError, Store};

/// The largest tool arguments, as JSON.
pub(crate) const MAX_ARGS_BYTES: usize = 256 * 1024;
/// The largest document a version keeps, as JSON.
pub(crate) const MAX_DOC_BYTES: usize = 512 * 1024;
/// The most documents one account or project keeps.
pub(crate) const MAX_DOCS: usize = 100;
/// The most versions one document keeps.
pub(crate) const MAX_VERSIONS: u64 = 300;
/// How long one tool run (a rebuild and its checks) may take.
pub(crate) const DEADLINE: Duration = Duration::from_secs(75);
/// How long a run waits for the one before it.
const QUEUE_WAIT: Duration = Duration::from_secs(40);
/// The engine's memory.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) const MEMORY_BYTES: u64 = 640 * 1024 * 1024;
/// Tool runs per person per hour.
pub(crate) const RUNS_PER_HOUR: usize = 150;
/// The largest file a run leaves (a render, a drawing, an export).
const MAX_FILE_BYTES: usize = 15 * 1024 * 1024;
/// How much the working copy of one scope's built parts may hold.
const MAX_PARTS_BYTES: u64 = 48 * 1024 * 1024;

/// The tools that change a model (a viewer may not run them).
const CHANGES: [&str; 13] = [
    "cad_new_part",
    "cad_add_feature",
    "cad_edit_feature",
    "cad_drag_face",
    "cad_set_part",
    "cad_assemble",
    "cad_from_json",
    "cad_import_dxf",
    "cad_import_step",
    "cad_import_file",
    "cad_import_model",
    "cad_place",
    "cad_robot",
];
/// A version's files the viewer and the page read.
pub(crate) const FILES: [&str; 6] = [
    "doc.json",
    "assets.json",
    "version.json",
    "view.json",
    "model.glb",
    "hero.png",
];

const INDEX_SCHEMA: &str = "openagents.web.cad-index.v1";

// ---- where -------------------------------------------------------------------

/// Whose models a chat or a page works on: an account's own, or a
/// project's (kept in the project's home account's folder).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Scope {
    /// The chat-store owner whose folder keeps the models.
    pub home: String,
    /// `cad/me` or `cad/p-{project}`.
    pub folder: String,
    /// The project, when it's a project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Whether the person may change the models.
    #[serde(skip)]
    pub edits: bool,
    /// Whether the person owns this space and may delete its models.
    #[serde(skip)]
    pub owns: bool,
    /// Who is asking (their runs count toward their limit).
    #[serde(skip)]
    pub actor: String,
}

fn valid_project(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A document id as the Lab makes them: lowercase letters, digits, `-` and
/// `_`, at most 48.
pub(crate) fn valid_doc(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && !id.starts_with(['-', '_'])
}

/// A file a run left in a version's `out/`.
fn valid_out(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 160
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'+'))
}

impl Scope {
    /// An account's own models.
    pub(crate) fn personal(owner: &str) -> Option<Scope> {
        crate::chat_store::is_account_owner(owner).then(|| Scope {
            home: owner.to_owned(),
            folder: "cad/me".into(),
            project: None,
            edits: true,
            owns: true,
            actor: owner.to_owned(),
        })
    }

    /// A project's models, kept in `home`'s folder.
    pub(crate) fn project(home: &str, project: &str, edits: bool, actor: &str) -> Option<Scope> {
        (crate::chat_store::is_account_owner(home) && valid_project(project)).then(|| Scope {
            home: home.to_owned(),
            folder: format!("cad/p-{project}"),
            project: Some(project.to_owned()),
            edits,
            owns: edits && actor == home,
            actor: actor.to_owned(),
        })
    }

    /// The models a chat turn works on: the chat's project's when it is in
    /// one (with the sender's role there), else the sender's own. `None`
    /// for a visitor, or a project chat whose project isn't known here.
    pub(crate) fn for_turn(
        sender: &str,
        chat_project: Option<&str>,
        project: Option<&oa_auth::repos::Project>,
    ) -> Option<Scope> {
        if !crate::chat_store::is_account_owner(sender) {
            return None;
        }
        match chat_project {
            None => Scope::personal(sender),
            Some(id) => {
                let project = project.filter(|p| p.id == id)?;
                let home = project
                    .sharing
                    .as_ref()
                    .and_then(|s| s.home.as_deref())
                    .map_or_else(|| sender.to_owned(), crate::chat_store::account_owner);
                let edits = crate::projects::api::role_of(project).edits();
                let mut scope = Scope::project(&home, id, edits, sender)?;
                scope.owns = crate::projects::api::role_of(project).owns();
                Some(scope)
            }
        }
    }

    /// The scope a card was made in (its `where`), read only.
    pub(crate) fn of_card(card: &Value) -> Option<Scope> {
        let place = &card["where"];
        let home = place["home"].as_str()?;
        let folder = place["folder"].as_str()?;
        let mut scope = match place["project"].as_str() {
            Some(project) => Scope::project(home, project, false, home)?,
            None => Scope::personal(home)?,
        };
        scope.edits = false;
        scope.owns = false;
        (scope.folder == folder).then_some(scope)
    }

    pub(crate) fn key(&self, rest: &str) -> Result<String, StoreError> {
        Store::owner_key(&self.home, &format!("{}/{rest}", self.folder))
    }

    /// The working copy's folder name on this server.
    fn local(&self) -> String {
        let hash = Sha256::digest(format!("{}\0{}", self.home, self.folder).as_bytes());
        hash.iter().take(12).map(|b| format!("{b:02x}")).collect()
    }

    /// Where a card or the page links to this scope's document.
    pub(crate) fn page(&self, doc: &str, version: Option<u64>) -> String {
        let mut url = format!("/cad/{doc}");
        let mut sep = '?';
        if let Some(v) = version {
            url.push_str(&format!("{sep}v={v}"));
            sep = '&';
        }
        if let Some(project) = &self.project {
            url.push_str(&format!("{sep}project={project}"));
        }
        url
    }

    fn where_json(&self) -> Value {
        json!({"home": self.home, "folder": self.folder, "project": self.project})
    }
}

// ---- the index -----------------------------------------------------------------

/// One version in a document's history.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct Version {
    pub n: u64,
    pub summary: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_kg: Option<f64>,
}

/// One document.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct Entry {
    pub title: String,
    #[serde(default)]
    pub kind: String,
    pub versions: u64,
    #[serde(default)]
    pub history: Vec<Version>,
}

/// A scope's documents.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct Index {
    pub schema: String,
    pub docs: BTreeMap<String, Entry>,
}

async fn read_index(store: &Store, scope: &Scope) -> Result<(Index, Option<String>), String> {
    let (mut index, generation) = read_legacy_index(store, scope).await?;
    platform::overlay_index(store, scope, &mut index).await?;
    Ok((index, generation))
}

async fn read_legacy_index(
    store: &Store,
    scope: &Scope,
) -> Result<(Index, Option<String>), String> {
    let key = scope.key("index.json").map_err(|e| e.to_string())?;
    match store.read_key(&key).await {
        Ok(Some((bytes, generation))) => {
            let index: Index = if store.labs().is_some() {
                platform::legacy_index(store, scope).await?
            } else {
                serde_json::from_slice(&bytes)
                    .map_err(|_| "We couldn't read your CAD documents.".to_owned())?
            };
            Ok((index, Some(generation)))
        }
        Ok(None) => Ok((
            Index {
                schema: INDEX_SCHEMA.into(),
                docs: BTreeMap::new(),
            },
            None,
        )),
        Err(e) => Err(format!("We couldn't read your CAD documents ({e}).")),
    }
}

/// Changes the index with `edit`, retrying when another writer got there
/// first.
async fn update_index(
    store: &Store,
    scope: &Scope,
    edit: impl Fn(&mut Index),
) -> Result<Index, String> {
    let key = scope.key("index.json").map_err(|e| e.to_string())?;
    for _ in 0..5 {
        let (mut index, generation) = read_legacy_index(store, scope).await?;
        index.schema = INDEX_SCHEMA.into();
        edit(&mut index);
        let bytes = serde_json::to_vec(&index).map_err(|e| e.to_string())?;
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(index),
            Err(StoreError::Conflict) => continue,
            Err(e) => return Err(format!("We couldn't save your CAD documents ({e}).")),
        }
    }
    Err("Your CAD documents kept changing; try again.".into())
}

/// The documents for the model's guidance: id, title, kind, versions.
pub(crate) fn documents(index: &Index) -> Vec<Value> {
    index
        .docs
        .iter()
        .map(|(id, e)| json!({"id": id, "title": e.title, "kind": e.kind, "versions": e.versions}))
        .collect()
}

// ---- the engine ----------------------------------------------------------------

/// The CAD engine on this server: the `cad-lab` program, its working
/// copies, and the queue.
pub(crate) struct Engine {
    program: PathBuf,
    cache: PathBuf,
    jobs: tokio::sync::Semaphore,
    scopes: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    runs: Mutex<HashMap<String, VecDeque<Instant>>>,
    tools: OnceLock<Vec<Value>>,
}

static ENGINE: OnceLock<Option<Engine>> = OnceLock::new();

/// The engine, when this server has one: `OPENAGENTS_CAD_ENGINE`, else
/// `/usr/local/bin/cad-lab` (the image's).
pub(crate) fn engine() -> Option<&'static Engine> {
    ENGINE
        .get_or_init(|| {
            let program = std::env::var_os("OPENAGENTS_CAD_ENGINE")
                .map(PathBuf::from)
                .or_else(|| {
                    let image = PathBuf::from("/usr/local/bin/cad-lab");
                    image.is_file().then_some(image)
                })?;
            program.is_file().then(|| Engine::new(program))
        })
        .as_ref()
}

impl Engine {
    fn new(program: PathBuf) -> Engine {
        let cache = std::env::var_os("OPENAGENTS_CAD_CACHE").map_or_else(
            || std::env::temp_dir().join("openagents-cad"),
            PathBuf::from,
        );
        Engine {
            program,
            cache,
            jobs: tokio::sync::Semaphore::new(1),
            scopes: Mutex::new(HashMap::new()),
            runs: Mutex::new(HashMap::new()),
            tools: OnceLock::new(),
        }
    }

    /// The tools the model is offered: the Lab's, without those that need
    /// a Mac, and a STEP model only as text.
    pub(crate) async fn tools(&self) -> Option<Vec<Value>> {
        if let Some(tools) = self.tools.get() {
            return Some(tools.clone());
        }
        let output = tokio::time::timeout(
            Duration::from_secs(20),
            tokio::process::Command::new(&self.program)
                .arg("tools")
                .env_clear()
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .ok()?
        .ok()?;
        let value: Value = serde_json::from_slice(&output.stdout).ok()?;
        let tools = hosted_tools(value["tools"].as_array()?);
        Some(self.tools.get_or_init(|| tools).clone())
    }

    fn scope_lock(&self, scope: &Scope) -> Arc<tokio::sync::Mutex<()>> {
        let mut scopes = self.scopes.lock().unwrap_or_else(|e| e.into_inner());
        scopes.entry(scope.local()).or_default().clone()
    }

    /// Counts a run toward `actor`'s hour; false when it's over the limit.
    fn admit(&self, actor: &str) -> bool {
        let mut runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let recent = runs.entry(actor.to_owned()).or_default();
        while recent
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(3600))
        {
            recent.pop_front();
        }
        if recent.len() >= RUNS_PER_HOUR {
            return false;
        }
        recent.push_back(now);
        true
    }

    /// Runs one tool on `scope`'s models. The answer is the Lab's JSON
    /// (a refusal is `{"error": why}`), with its card saying where it was
    /// made; a new version is kept before this returns.
    pub(crate) async fn run(
        &self,
        store: &Store,
        scope: &Scope,
        name: &str,
        args: &Value,
    ) -> Value {
        match self.try_run(store, scope, name, args).await {
            Ok(value) => value,
            Err(why) => json!({"error": why}),
        }
    }

    async fn try_run(
        &self,
        store: &Store,
        scope: &Scope,
        name: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let mut args = admitted(scope, name, args)?;
        let name = if name == "cad_import_file" {
            args = imports::from_chat(store, scope, &args).await?;
            "cad_import_model"
        } else {
            name
        };
        if !self.admit(&scope.actor) {
            return Err(
                "You've run a lot of CAD steps this hour. Try again in a few minutes.".into(),
            );
        }
        let lock = self.scope_lock(scope);
        let _scope = lock.lock().await;
        let (index, _) = read_index(store, scope).await?;
        if name == "cad_drag_face" {
            let base = args
                .as_object_mut()
                .and_then(|a| a.remove("base_version"))
                .and_then(|v| v.as_u64())
                .ok_or("Reload the design before pulling a face.")?;
            let doc = args["doc"].as_str().unwrap_or_default();
            if !index
                .docs
                .get(doc)
                .is_some_and(|entry| entry.versions == base)
            {
                return Err("This design changed. Reload it before pulling a face.".into());
            }
        }
        if CHANGES.contains(&name) && !matches!(name, "cad_place" | "cad_robot") {
            if let Some(doc) = args["doc"].as_str() {
                if !index.docs.contains_key(doc) && index.docs.len() >= MAX_DOCS {
                    return Err(format!(
                        "You have {MAX_DOCS} CAD documents here, the most we keep. Delete one you don't need first."
                    ));
                }
                if index
                    .docs
                    .get(doc)
                    .is_some_and(|e| e.versions >= MAX_VERSIONS)
                {
                    return Err(format!(
                        "{doc} has {MAX_VERSIONS} versions, the most we keep. Start a new document from it with cad_from_json."
                    ));
                }
            }
        }
        let placement = if name == "cad_place" {
            handoff::place_settings(&args)?;
            let doc = args["doc"].as_str().ok_or("Choose a design.")?;
            let entry = index.docs.get(doc).ok_or("This design isn't here.")?;
            let n = args["version"].as_u64().unwrap_or(entry.versions);
            if !entry.history.iter().any(|v| v.n == n) {
                return Err("Choose a saved design version.".into());
            }
            if args["remove"] == true {
                return handoff::place(store, scope, &args, doc, n, None).await;
            }
            Some((doc.to_owned(), n))
        } else {
            None
        };
        let dir = self.cache.join(scope.local());
        let wanted = named_docs(&args);
        sync(store, scope, &dir, &index, &wanted).await?;
        imports::restore(store, scope, &dir, &index, &args).await?;
        let result = {
            let _job = tokio::time::timeout(QUEUE_WAIT, self.jobs.acquire())
                .await
                .map_err(|_| {
                    "We're building another model right now. Try again in a minute.".to_owned()
                })?
                .map_err(|_| "The CAD engine stopped.".to_owned())?;
            if let Some((doc, n)) = &placement {
                self.child(
                    &dir,
                    "cad_export",
                    &json!({"doc":doc,"version":n,"formats":["glb"]}),
                )
                .await
            } else {
                self.child(&dir, name, &args).await
            }
        };
        let mut result = match result {
            Ok(result) => result,
            Err(why) => {
                // A run that stopped part way may have left a version here.
                let _ = sync(store, scope, &dir, &index, &HashSet::new()).await;
                return Err(why);
            }
        };
        if let Some((doc, n)) = placement.as_ref().filter(|_| result["error"].is_null()) {
            let bytes = tokio::fs::read(dir.join(doc).join(format!("v{n}/out/{doc}-v{n}.glb")))
                .await
                .map_err(|_| "The model couldn't be sent to Verse.")?;
            let delivery = handoff::place(store, scope, &args, doc, *n, Some(bytes)).await?;
            result["summary"] = delivery["summary"].clone();
            result["verse"] = delivery["verse"].clone();
            result["card"]["verse"] = delivery["verse"].clone();
        }
        if name == "cad_export" && result["error"].is_null() {
            for file in result["files"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|f| f["format"] == "urdf")
            {
                let path = PathBuf::from(
                    file["path"]
                        .as_str()
                        .ok_or("The URDF archive wasn't written.")?,
                );
                let file = path
                    .file_name()
                    .and_then(|p| p.to_str())
                    .filter(|n| valid_out(n))
                    .ok_or("The URDF archive wasn't written.")?;
                let doc = result["doc"].as_str().ok_or("Choose a design.")?;
                let n = result["version"]
                    .as_u64()
                    .ok_or("Choose a saved version.")?;
                let bytes = tokio::fs::read(&path)
                    .await
                    .map_err(|_| "The URDF archive couldn't be read.")?;
                if bytes.len() > MAX_FILE_BYTES {
                    return Err("This robot archive is too large. Send a smaller assembly.".into());
                }
                if !platform::output(store, scope, doc, n, file, &bytes).await? {
                    put(
                        store,
                        &scope
                            .key(&format!("{doc}/v{n}/out/{file}"))
                            .map_err(|e| e.to_string())?,
                        bytes,
                    )
                    .await?;
                }
            }
        }
        if name == "cad_robot" && result["error"].is_null() {
            handoff::robot_saved(store, scope, &dir, &mut result).await?;
        }
        if result["saved"] == true {
            if let Err(why) = persist(store, scope, &dir, &result).await {
                let _ = sync(store, scope, &dir, &index, &HashSet::new()).await;
                return Err(why);
            }
        }
        keep_outputs(store, scope, &dir).await;
        trim_parts(&dir.join(".parts"));
        if name != "cad_configure" && name != "cad_config_options" {
            hosted_result(&mut result, scope);
        }
        Ok(result)
    }

    /// The engine as a child process over `dir`: the arguments on stdin,
    /// the answer on stdout, stopped at the deadline.
    async fn child(&self, dir: &Path, name: &str, args: &Value) -> Result<Value, String> {
        let mut command = tokio::process::Command::new(&self.program);
        command
            .arg("tool")
            .arg(name)
            .arg("-")
            .env_clear()
            .env("CAD_LAB_DIR", dir)
            .env("HOME", dir)
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        limit(&mut command);
        let mut child = command
            .spawn()
            .map_err(|_| "The CAD engine couldn't start.".to_owned())?;
        if let Some(mut stdin) = child.stdin.take() {
            let bytes = args.to_string().into_bytes();
            stdin
                .write_all(&bytes)
                .await
                .map_err(|_| "The CAD engine couldn't read the step.".to_owned())?;
        }
        let output = match tokio::time::timeout(DEADLINE, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(_)) => return Err("The CAD engine stopped.".into()),
            Err(_) => {
                return Err(format!(
                    "That took longer than {} seconds to rebuild, so we stopped it. Try a simpler change, or fewer patterned features.",
                    DEADLINE.as_secs()
                ));
            }
        };
        serde_json::from_slice(&output.stdout).map_err(|_| {
            if output.status.success() {
                "The CAD engine's answer couldn't be read.".to_owned()
            } else {
                "The CAD engine ran out of room for this model. Try a simpler one.".to_owned()
            }
        })
    }
}

/// The engine's limits: memory, and CPU time as a backstop to the deadline.
#[cfg(target_os = "linux")]
fn limit(command: &mut tokio::process::Command) {
    // SAFETY: setrlimit is async-signal-safe; nothing else runs between
    // fork and exec here.
    unsafe {
        command.pre_exec(|| {
            let memory = libc::rlimit {
                rlim_cur: MEMORY_BYTES,
                rlim_max: MEMORY_BYTES,
            };
            libc::setrlimit(libc::RLIMIT_DATA, &memory);
            let cpu = libc::rlimit {
                rlim_cur: DEADLINE.as_secs() * 2,
                rlim_max: DEADLINE.as_secs() * 2 + 5,
            };
            libc::setrlimit(libc::RLIMIT_CPU, &cpu);
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn limit(_command: &mut tokio::process::Command) {}

/// The arguments as the engine gets them, or why the run is refused before
/// it starts.
fn admitted(scope: &Scope, name: &str, args: &Value) -> Result<Value, String> {
    if !name.starts_with("cad_")
        || !name[4..]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'_')
    {
        return Err(format!("{name} isn't a CAD tool."));
    }
    let mut args = match args {
        Value::Object(_) => args.clone(),
        _ => json!({}),
    };
    let argument_limit = if name == "cad_import_model" {
        (2 * 1024 * 1024usize).div_ceil(3) * 4 + 8192
    } else {
        MAX_ARGS_BYTES
    };
    if args.to_string().len() > argument_limit {
        return Err(format!(
            "That step is more than {} KB; make it in smaller steps.",
            MAX_ARGS_BYTES / 1024
        ));
    }
    if CHANGES.contains(&name) && !scope.edits {
        return Err(
            "You can look at this project's models, but only its editors can change them.".into(),
        );
    }
    if name == "cad_import_step" && args.get("path").is_some() {
        return Err(
            "Give the STEP model's text (step); files on this server can't be read.".into(),
        );
    }
    if name == "cad_render" {
        for (key, most) in [("width", 1600), ("height", 1000)] {
            if args[key].as_u64().is_some_and(|v| v > most) {
                args[key] = json!(most);
            }
        }
    }
    if let Some(doc) = args.get("doc").and_then(Value::as_str) {
        if !valid_doc(doc.trim()) {
            return Err(
                "A document id is lowercase letters, digits and -, like \"fan-bracket\".".into(),
            );
        }
    }
    Ok(args)
}

/// The tools as offered here.
fn hosted_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter(|t| t["name"].as_str().is_some_and(|n| n.starts_with("cad_")))
        .cloned()
        .map(|mut tool| {
            if let Some(text) = tool["description"].as_str() {
                tool["description"] = json!(
                    text.replace(" in CAD Lab on this computer", "")
                        .replace(" on this computer", "")
                );
            }
            if tool["name"] == "cad_import_step" {
                if let Some(props) = tool["parameters"]["properties"].as_object_mut() {
                    props.remove("path");
                }
                if let Some(required) = tool["parameters"]["required"].as_array_mut() {
                    required.retain(|r| r != "path");
                }
            }
            tool
        })
        .chain(std::iter::once(imports::tool()))
        .collect()
}

/// Every document id the arguments name (`doc`, and `from.doc` in
/// templates): their whole history is read, others only their latest.
fn named_docs(args: &Value) -> HashSet<String> {
    let mut out = HashSet::new();
    fn walk(value: &Value, out: &mut HashSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, v) in map {
                    if key == "doc" {
                        if let Some(id) = v.as_str().map(str::trim).filter(|id| valid_doc(id)) {
                            out.insert(id.to_owned());
                        }
                    }
                    walk(v, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, out)),
            Value::String(text) if text.trim_start().starts_with('{') => {
                if let Ok(inner) = serde_json::from_str::<Value>(text) {
                    walk(&inner, out);
                }
            }
            _ => {}
        }
    }
    walk(args, &mut out);
    out
}

/// Brings the working copy in `dir` to the index: documents and versions
/// the index doesn't have go, and each document's latest version (every
/// version of those in `full`) is read in.
async fn sync(
    store: &Store,
    scope: &Scope,
    dir: &Path,
    index: &Index,
    full: &HashSet<String>,
) -> Result<(), String> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|_| "The CAD engine has no room to work.".to_owned())?;
    // What the index doesn't have.
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|_| "The CAD engine has no room to work.".to_owned())?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        match index.docs.get(&name) {
            None => {
                let _ = tokio::fs::remove_dir_all(entry.path()).await;
            }
            Some(doc) => {
                let mut versions = tokio::fs::read_dir(entry.path()).await.ok();
                while let Some(Ok(Some(v))) = match versions.as_mut() {
                    Some(list) => Some(list.next_entry().await),
                    None => None,
                } {
                    let n = v
                        .file_name()
                        .to_str()
                        .and_then(|s| s.strip_prefix('v'))
                        .and_then(|s| s.parse::<u64>().ok());
                    if n.is_none_or(|n| n > doc.versions || n == 0) {
                        let _ = tokio::fs::remove_dir_all(v.path()).await;
                    }
                }
            }
        }
    }
    // What the run needs.
    let mut needed = Vec::new();
    for (id, doc) in &index.docs {
        let range: Vec<u64> = if full.contains(id) {
            (1..=doc.versions).collect()
        } else {
            vec![doc.versions]
        };
        for n in range.into_iter().filter(|n| *n > 0) {
            let at = dir.join(id).join(format!("v{n}"));
            if !at.join("doc.json").is_file() || !at.join("version.json").is_file() {
                needed.push((id.clone(), n, at));
            }
        }
    }
    use futures_util::stream::{self, StreamExt};
    let results: Vec<Result<(), String>> = stream::iter(needed)
        .map(|(id, n, at)| async move {
            tokio::fs::create_dir_all(&at)
                .await
                .map_err(|_| "The CAD engine has no room to work.".to_owned())?;
            for file in ["version.json", "doc.json"] {
                let bytes = read_file(store, scope, &id, n, file)
                    .await
                    .ok_or_else(|| format!("{id} v{n} is missing."))?;
                tokio::fs::write(at.join(file), bytes)
                    .await
                    .map_err(|_| "The CAD engine has no room to work.".to_owned())?;
            }
            Ok(())
        })
        .buffer_unordered(8)
        .collect()
        .await;
    results.into_iter().collect()
}

/// Keeps a version the run saved: its document first (only when that
/// version isn't there yet), then its files, then the index.
async fn persist(store: &Store, scope: &Scope, dir: &Path, result: &Value) -> Result<(), String> {
    if platform::persist(store, scope, dir, result).await? {
        return Ok(());
    }
    let doc = result["doc"]
        .as_str()
        .filter(|d| valid_doc(d))
        .ok_or("The engine saved no document.")?;
    let n = result["version"]
        .as_u64()
        .ok_or("The engine saved no version.")?;
    let at = dir.join(doc).join(format!("v{n}"));
    let document = tokio::fs::read(at.join("doc.json"))
        .await
        .map_err(|_| "The new version couldn't be read.".to_owned())?;
    if document.len() > MAX_DOC_BYTES {
        return Err(format!(
            "That document is more than {} KB, the most we keep; nothing was saved.",
            MAX_DOC_BYTES / 1024
        ));
    }
    let record_bytes = tokio::fs::read(at.join("version.json"))
        .await
        .map_err(|_| "The new version couldn't be read.".to_owned())?;
    let record: Value = serde_json::from_slice(&record_bytes).unwrap_or(Value::Null);
    let parsed: Value = serde_json::from_slice(&document).unwrap_or(Value::Null);
    let key = |file: &str| {
        scope
            .key(&format!("{doc}/v{n}/{file}"))
            .map_err(|e| e.to_string())
    };
    match store.write_key(&key("doc.json")?, document, None).await {
        Ok(_) => {}
        Err(StoreError::Conflict) => {
            return Err(format!(
                "{doc} just changed in another chat, so this change wasn't saved. Try it again."
            ));
        }
        Err(e) => return Err(format!("We couldn't save {doc} ({e}).")),
    }
    for file in [
        "version.json",
        "assets.json",
        "view.json",
        "model.glb",
        "hero.png",
    ] {
        let Ok(bytes) = tokio::fs::read(at.join(file)).await else {
            continue;
        };
        put(store, &key(file)?, bytes).await?;
        if file != "version.json" && file != "assets.json" {
            let _ = tokio::fs::remove_file(at.join(file)).await;
        }
    }
    let entry_version = Version {
        n,
        summary: record["summary"].as_str().unwrap_or_default().to_owned(),
        author: record["author"].as_str().unwrap_or_default().to_owned(),
        created: record["created"].as_u64().unwrap_or_default(),
        mass_kg: record["report"]["mass"]["mass_kg"].as_f64(),
    };
    let title = parsed["title"].as_str().unwrap_or(doc).to_owned();
    let kind = parsed["kind"].as_str().unwrap_or_default().to_owned();
    imports::persist(store, scope, doc, n, &at).await?;
    update_index(store, scope, |index| {
        let entry = index.docs.entry(doc.to_owned()).or_default();
        entry.title.clone_from(&title);
        entry.kind.clone_from(&kind);
        entry.versions = entry.versions.max(n);
        entry.history.retain(|v| v.n != n);
        entry.history.push(entry_version.clone());
        entry.history.sort_by_key(|v| v.n);
    })
    .await?;
    Ok(())
}

/// Writes `bytes` at `key`, replacing what's there.
async fn put(store: &Store, key: &str, bytes: Vec<u8>) -> Result<(), String> {
    for _ in 0..3 {
        let current = store
            .read_key(key)
            .await
            .map_err(|e| format!("We couldn't save a file ({e})."))?
            .map(|(_, generation)| generation);
        match store
            .write_key(key, bytes.clone(), current.as_deref())
            .await
        {
            Ok(_) => return Ok(()),
            Err(StoreError::Conflict) => continue,
            Err(e) => return Err(format!("We couldn't save a file ({e}).")),
        }
    }
    Err("We couldn't save a file; it kept changing.".into())
}

/// Keeps the renders, drawings and exports the run left in any version's
/// `out/`, then takes them off this server.
async fn keep_outputs(store: &Store, scope: &Scope, dir: &Path) {
    let Ok(mut docs) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(doc)) = docs.next_entry().await {
        let doc_name = doc.file_name().to_string_lossy().into_owned();
        if !valid_doc(&doc_name) {
            continue;
        }
        let Ok(mut versions) = tokio::fs::read_dir(doc.path()).await else {
            continue;
        };
        while let Ok(Some(version)) = versions.next_entry().await {
            let out = version.path().join("out");
            let Ok(mut files) = tokio::fs::read_dir(&out).await else {
                continue;
            };
            let v = version.file_name().to_string_lossy().into_owned();
            while let Ok(Some(file)) = files.next_entry().await {
                let name = file.file_name().to_string_lossy().into_owned();
                let path = file.path();
                if path.is_dir() {
                    let _ = tokio::fs::remove_dir_all(&path).await;
                    continue;
                }
                if valid_out(&name) && !name.ends_with(".robot.json") {
                    if let Ok(bytes) = tokio::fs::read(&path).await {
                        if bytes.len() <= MAX_FILE_BYTES {
                            if let Some(n) = v.strip_prefix('v').and_then(|v| v.parse::<u64>().ok())
                            {
                                match platform::output(store, scope, &doc_name, n, &name, &bytes)
                                    .await
                                {
                                    Ok(true) => {
                                        let _ = tokio::fs::remove_file(&path).await;
                                        continue;
                                    }
                                    Ok(false) => {}
                                    Err(why) => {
                                        eprintln!("openagents-web: cad: {why}");
                                        continue;
                                    }
                                }
                            }
                            if let Ok(key) = scope.key(&format!("{doc_name}/{v}/out/{name}")) {
                                if let Err(why) = put(store, &key, bytes).await {
                                    eprintln!("openagents-web: cad: {why}");
                                }
                            }
                        }
                    }
                }
                let _ = tokio::fs::remove_file(&path).await;
            }
        }
    }
}

/// Keeps the working copy's built parts under [`MAX_PARTS_BYTES`].
fn trim_parts(parts: &Path) {
    let size: u64 = std::fs::read_dir(parts)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    if size > MAX_PARTS_BYTES {
        let _ = std::fs::remove_dir_all(parts);
    }
}

/// The answer as the chat gets it: files by name rather than by where this
/// server keeps them, and the card saying where it was made.
fn hosted_result(result: &mut Value, scope: &Scope) {
    fn walk(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.remove("url");
                if let Some(Value::String(path)) = map.get_mut("path") {
                    let name = path.rsplit('/').next().unwrap_or_default().to_owned();
                    *path = name;
                }
                map.values_mut().for_each(walk);
            }
            Value::Array(items) => items.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(result);
    if result["card"].is_object() {
        result["card"]["where"] = scope.where_json();
    }
}

// ---- reading and deleting --------------------------------------------------------

/// A stored file of a version: one of [`FILES`], or `out/{name}`.
pub(crate) async fn read_file(
    store: &Store,
    scope: &Scope,
    doc: &str,
    version: u64,
    name: &str,
) -> Option<Vec<u8>> {
    if !valid_doc(doc) {
        return None;
    }
    match platform::read_file(store, scope, doc, version, name).await {
        Ok(Some(bytes)) => return Some(bytes),
        Ok(None) => {}
        Err(_) => return None,
    }
    let rest = if FILES.contains(&name) {
        format!("{doc}/v{version}/{name}")
    } else if let Some(file) = name.strip_prefix("out/").filter(|file| valid_out(file)) {
        format!("{doc}/v{version}/out/{file}")
    } else if valid_out(name) {
        format!("{doc}/v{version}/out/{name}")
    } else {
        return None;
    };
    store
        .read_key(&scope.key(&rest).ok()?)
        .await
        .ok()
        .flatten()
        .map(|(bytes, _)| bytes)
}

/// The scope's documents.
pub(crate) async fn load_index(store: &Store, scope: &Scope) -> Result<Index, String> {
    read_index(store, scope).await.map(|(index, _)| index)
}

pub(crate) async fn copy_history(
    store: &Store,
    owner: &str,
    project: Option<&str>,
    approved_checksum: &str,
) -> Result<Value, String> {
    let scope = match project {
        Some(project) => Scope::project(owner, project, true, owner),
        None => Scope::personal(owner),
    }
    .ok_or("Choose a valid CAD space.")?;
    platform::migrate(store, &scope, approved_checksum).await
}

/// Deletes a document and all its versions.
pub(crate) async fn delete(store: &Store, scope: &Scope, doc: &str) -> Result<(), String> {
    if !scope.owns {
        return Err("Only the project's owner can delete its models.".into());
    }
    if !valid_doc(doc) {
        return Err("There's no such document.".into());
    }
    let lock = engine().map(|engine| engine.scope_lock(scope));
    let _scope = match &lock {
        Some(lock) => Some(lock.lock().await),
        None => None,
    };
    delete_now(store, scope, doc).await
}

async fn delete_now(store: &Store, scope: &Scope, doc: &str) -> Result<(), String> {
    configurator::purge(store, scope, doc).await?;
    sales::purge(store, scope, doc).await?;
    platform::delete(store, scope, doc).await?;
    update_index(store, scope, |index| {
        index.docs.remove(doc);
    })
    .await?;
    let folder = scope.key(doc).map_err(|e| e.to_string())?;
    store
        .remove_folder(&folder)
        .await
        .map_err(|e| format!("We couldn't delete {doc} ({e})."))
}

pub(crate) fn content_type(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or_default() {
        "png" => "image/png",
        "json" => "application/json",
        "glb" => "model/gltf-binary",
        "step" => "model/step",
        "3mf" => "model/3mf",
        "stl" => "model/stl",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "dxf" => "image/vnd.dxf",
        _ => "application/octet-stream",
    }
}

/// A stored file as a response: private, and a download unless it's for
/// the viewer.
pub(crate) fn file_response(name: &str, bytes: Vec<u8>, download: Option<&str>) -> Response {
    let mut response = (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type(name)),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            ),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        bytes,
    )
        .into_response();
    // A drawing's SVG is shown as a picture, never run as a page.
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; sandbox"),
    );
    if let Some(file) = download {
        if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{file}\"")) {
            response
                .headers_mut()
                .insert(header::CONTENT_DISPOSITION, value);
        }
    }
    response
}

// ---- the page -------------------------------------------------------------------

#[derive(Deserialize, Debug, Default)]
pub(crate) struct PageQuery {
    #[serde(default)]
    v: Option<u64>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    download: Option<String>,
    #[serde(default)]
    check: Option<String>,
}

/// The signed-in person's scope for a page: their own models, or a
/// project's they're in.
async fn page_scope(app: &App, headers: &HeaderMap, project: Option<&str>) -> Option<Scope> {
    let reader = crate::pages::chat::reader(app, headers).await?;
    if !crate::chat_store::is_account_owner(&reader) {
        return None;
    }
    match project.filter(|p| !p.is_empty()) {
        None => {
            let viewer = crate::cad_gate::viewer(app, headers).await?;
            if crate::chat_store::account_owner(&viewer.account_id) != reader {
                return None;
            }
            crate::labs_api::bind_account(app, &viewer.account_id)
                .await
                .ok()?;
            Scope::personal(&reader)
        }
        Some(id) => {
            let access = crate::projects::api::access(app, headers, id).await.ok()?;
            let mut scope = Scope::project(
                &access.home_owner(),
                &access.project.id,
                access.role.edits(),
                &reader,
            )?;
            scope.owns = access.role.owns();
            Some(scope)
        }
    }
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(configurator::routes())
        .merge(imports::routes())
        .merge(handoff::routes())
        .merge(sales::routes())
        // `{file}`, as /cad/{file} names the same segment (pages::cad).
        .route("/cad/{file}/file", get(file))
        .route("/cad/{file}/delete", post(delete_form))
        .route("/cad/{file}/pull", post(pull_form))
        .route(
            DOC_JS,
            get(|| async {
                (
                    [
                        (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
                        (header::CACHE_CONTROL, "public, max-age=300"),
                    ],
                    include_str!("../static/cad-doc.js"),
                )
            }),
        )
}

const DOC_JS: &str = "/chat/cad/doc.js";

/// `GET /cad/{doc}/file?v=N&name=NAME[&project=P][&download=1]`.
async fn file(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let (Some(v), Some(name)) = (query.v, query.name.as_deref()) else {
        return crate::not_found().await;
    };
    let Some(scope) = page_scope(&app, &headers, query.project.as_deref()).await else {
        return crate::not_found().await;
    };
    match read_file(&app.config.chat_store, &scope, &doc, v, name).await {
        Some(bytes) => {
            let download = query.download.is_some().then(|| match name {
                "model.glb" => format!("{doc}-v{v}.glb"),
                other => other.to_owned(),
            });
            file_response(name, bytes, download.as_deref())
        }
        None => crate::not_found().await,
    }
}

#[derive(Deserialize)]
struct PullForm {
    csrf: String,
    #[serde(default)]
    project: String,
    version: u64,
    part: String,
    face: String,
    delta: f64,
}

async fn pull_form(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Form(form): Form<PullForm>,
) -> Response {
    if crate::pages::chat::validate_form(&app, &headers, &form.csrf)
        .await
        .is_err()
    {
        return (StatusCode::FORBIDDEN, "Reload the page and try again.").into_response();
    }
    if !crate::cad_gate::plan(&app, &headers).await.pro() {
        return crate::cad_gate::refused();
    }
    let Some(scope) = page_scope(&app, &headers, Some(&form.project)).await else {
        return crate::not_found().await;
    };
    if !scope.edits {
        return (
            StatusCode::FORBIDDEN,
            "Only this project's editors can change this design.",
        )
            .into_response();
    }
    let Some(engine) = engine() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "The CAD engine isn't available. Try again later.",
        )
            .into_response();
    };
    let result = engine.run(&app.config.chat_store, &scope, "cad_drag_face", &json!({
        "doc":doc,"base_version":form.version,"part":form.part,"face":form.face,"delta":form.delta,
    })).await;
    if let Some(error) = result["error"].as_str() {
        return (StatusCode::CONFLICT, error.to_owned()).into_response();
    }
    if result["saved"] != true {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "This change didn't pass the design checks.",
        )
            .into_response();
    }
    axum::Json(
        json!({"url":scope.page(&doc, result["version"].as_u64()),"version":result["version"]}),
    )
    .into_response()
}

#[derive(Deserialize)]
struct DeleteForm {
    csrf: String,
    #[serde(default)]
    project: String,
}

/// `POST /cad/{doc}/delete`: the document and every version, from the page.
async fn delete_form(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Form(form): Form<DeleteForm>,
) -> Response {
    if crate::pages::chat::validate_form(&app, &headers, &form.csrf)
        .await
        .is_err()
    {
        return (StatusCode::FORBIDDEN, "Reload the page and try again.").into_response();
    }
    let Some(scope) = page_scope(&app, &headers, Some(form.project.as_str())).await else {
        return crate::not_found().await;
    };
    match delete(&app.config.chat_store, &scope, &doc).await {
        Ok(()) => Redirect::to(&match &scope.project {
            Some(project) => format!("/projects/{project}"),
            None => "/cad".to_owned(),
        })
        .into_response(),
        Err(why) => (StatusCode::FORBIDDEN, why).into_response(),
    }
}

fn file_url(scope: &Scope, doc: &str, version: u64, name: &str) -> String {
    let mut url = format!("/cad/{doc}/file?v={version}&name={name}");
    if let Some(project) = &scope.project {
        url.push_str(&format!("&project={project}"));
    }
    url
}

fn download_url(scope: &Scope, doc: &str, version: u64, name: &str) -> String {
    format!("{}&download=1", file_url(scope, doc, version, name))
}

fn when(unix: u64) -> String {
    // Days since 1970 to a calendar date (UTC).
    let days = (unix / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{} {d}, {y}", MONTHS[(m - 1) as usize])
}

fn mass(kg: Option<f64>) -> String {
    match kg {
        Some(kg) if kg < 1.0 => format!("{:.1} g", kg * 1000.0),
        Some(kg) => format!("{kg:.2} kg"),
        None => String::new(),
    }
}

/// The page's content for a document's version.
pub(crate) fn doc_content(
    scope: &Scope,
    doc: &str,
    entry: &Entry,
    version: u64,
    record: &Value,
    csrf: Option<&str>,
    engine_in_browser: bool,
    plan: crate::cad_gate::Plan,
) -> Markup {
    // CAD Lab's edits are part of Pro (#11693); a design stays viewable
    // and downloadable without it.
    let pro = plan.pro();
    let card = json!({
        "title": entry.title, "version": version,
        "mass": record["report"]["mass"], "size_mm": record["report"]["size_mm"],
        "parts": record["report"]["parts"].as_array().map_or(0, Vec::len),
        "instances": record["report"]["instances"].as_array().map_or(0, Vec::len),
        "features": record["report"]["features"], "joints": record["report"]["joints"],
    });
    html! {
        section.oa-cad-doc aria-labelledby="cad-doc-title"
            data-cad-doc-page="" data-cad-doc=(doc) data-cad-version=(version) data-cad-findings=[record.get("manufacturing").map(|r|r["findings"].to_string())]
            data-cad-document=(file_url(scope, doc, version, "doc.json"))
            data-cad-pull=[(pro && engine_in_browser && csrf.is_some()).then(|| format!("/cad/{doc}/pull"))]
            data-cad-csrf=[csrf] data-cad-project=(scope.project.as_deref().unwrap_or("")) {
            (MarkdownRoot::new(html! {
                h1 id="cad-doc-title" { (entry.title) }
                a href="/cad-shop" {"Browse parts"} " · " a href="/cad-purchases" {"Your purchases"}
                p.oa-page-lead {
                    "Version " (version) " of " (entry.versions)
                    @if let Some(summary) = record["summary"].as_str() { " · " (summary) }
                }
            }))
            (crate::cad_lab::viewer(
                &file_url(scope, doc, version, "view.json"),
                Some(&file_url(scope, doc, version, "hero.png")),
                &entry.title,
                doc,
                version,
            ))
            @if !pro {
                (crate::cad_gate::upsell(plan, &scope.page(doc, Some(version))))
            }
            @if pro && engine_in_browser && csrf.is_some() {
                div.oa-cad-pull data-cad-pull-panel="" hidden {
                    p data-cad-pull-label="" {}
                    label { "Move face (mm) " input type="number" step="0.1" value="0" data-cad-pull-delta=""; }
                    (Button::new("Preview").variant(ButtonVariant::Outline).attr("data-cad-pull-preview", ""))
                    (Button::new("Save change").attr("data-cad-pull-save", "").disabled(true))
                    (Button::new("Cancel").variant(ButtonVariant::Outline).attr("data-cad-pull-cancel", ""))
                    p data-cad-pull-status="" aria-live="polite" {}
                }
            }
            p.oa-cad-meta { (crate::cad_lab::mass_line(&card["mass"])) }
            p.oa-cad-meta { (crate::cad_lab::counts_line(&card)) }
            @if record["report"]["mesh_bodies"].as_u64().unwrap_or(0)>0 {
                p.oa-cad-meta {"STL mesh · dimensions in millimeters"}
            }
            @if let Some(csrf) = csrf.filter(|_|engine().is_some()) {
                @if pro { (imports::form(scope,doc,csrf,record)) }
                (handoff::form(scope,doc,version,csrf,pro))
            }
            @if let Some(report)=record.get("manufacturing") {(crate::cad_lab::manufacturing(report))}
            @if pro && engine().is_some() && record["report"]["mesh_bodies"].as_u64().unwrap_or(0)<record["report"]["solids"].as_u64().unwrap_or(1) {
                div.oa-cad-files {
                    @for (process,label) in [("print","Print checks"),("cnc","CNC checks")] {
                        (ButtonLink::new(label,format!("{}&check={process}",scope.page(doc,Some(version)))).variant(ButtonVariant::Outline))
                    }
                }
            }
            h2 { "Files" }
            div.oa-cad-files {
                (ButtonLink::new("GLB", download_url(scope, doc, version, "model.glb")).variant(ButtonVariant::Outline))
                @if engine_in_browser {
                    @for (format, label) in [("step", "STEP"), ("stl", "STL"), ("3mf", "3MF"), ("pdf", "Drawing (PDF)"), ("dxf", "Drawing (DXF)")].into_iter().filter(|(format,_)|record["report"]["mesh_bodies"].as_u64().unwrap_or(0)==0 || matches!(*format,"stl"|"3mf")) {
                        (Button::new(label).variant(ButtonVariant::Outline).attr("data-cad-download", format).disabled(true))
                    }
                }
            }
            @if engine_in_browser {
                p.oa-cad-meta data-cad-download-status="" aria-live="polite" { "Loading the CAD engine for the files\u{2026}" }
            }
            @if pro && engine_in_browser && csrf.is_some() && scope.home == scope.actor && record["report"]["mesh_bodies"].as_u64().unwrap_or(0)<record["report"]["solids"].as_u64().unwrap_or(1) {
                (ButtonLink::new("Share or sell with sliders", format!("/cad/{doc}/configurator?v={version}&project={}",scope.project.as_deref().unwrap_or(""))).variant(ButtonVariant::Outline))
            }
            h2 { "Versions" }
            ol.oa-cad-versions reversed {
                @for v in entry.history.iter().rev() {
                    li aria-current=[(v.n == version).then_some("page")] {
                        a href=(scope.page(doc, Some(v.n))) { "v" (v.n) }
                        span { (v.summary) }
                        @if v.created > 0 { span.oa-cad-dim { (when(v.created)) } }
                        @if let Some(kg) = v.mass_kg { span.oa-cad-dim { (mass(Some(kg))) } }
                    }
                }
            }
            @if let Some(csrf) = csrf.filter(|_| scope.owns) {
                details {
                    summary { "Delete this design" }
                    form method="post" action=(format!("/cad/{doc}/delete")) {
                        input type="hidden" name="csrf" value=(csrf);
                        input type="hidden" name="project" value=(scope.project.clone().unwrap_or_default());
                        p { "This deletes every version of " (entry.title) ". It can't be undone." }
                        (Button::new("Delete it").color(Color::Danger).kind(ButtonType::Submit))
                    }
                }
            }
        }
    }
}

/// `GET /cad/{doc}`: a document's version in 3D, its files and its
/// versions.
pub(crate) async fn doc_page(
    app: &App,
    headers: &HeaderMap,
    doc: &str,
    query: &PageQuery,
) -> Response {
    let not_found = || crate::not_found_for(headers);
    if !valid_doc(doc) {
        return not_found();
    }
    let Some(scope) = page_scope(app, headers, query.project.as_deref()).await else {
        return not_found();
    };
    let store = &app.config.chat_store;
    let Ok(index) = load_index(store, &scope).await else {
        return not_found();
    };
    let Some(entry) = index.docs.get(doc) else {
        return not_found();
    };
    let version = query
        .v
        .filter(|v| (1..=entry.versions).contains(v))
        .unwrap_or(entry.versions);
    let plan = crate::cad_gate::plan(app, headers).await;
    let mut record = read_file(store, &scope, doc, version, "version.json")
        .await
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or(Value::Null);
    if let (Some(process), Some(engine)) = (
        query
            .check
            .as_deref()
            .filter(|p| plan.pro() && matches!(*p, "print" | "cnc")),
        engine(),
    ) {
        let checked = engine
            .run(
                store,
                &scope,
                "cad_manufacture",
                &json!({"doc":doc,"version":version,"profile":{"process":process}}),
            )
            .await;
        if checked["manufacturing"].is_object() {
            record["manufacturing"] = checked["manufacturing"].clone();
        } else if let Some(error) = checked["error"].as_str() {
            record["manufacturing"] =
                json!({"profile":{"process":process},"findings":[],"complete":false,"error":error});
        }
    }
    let reader = crate::pages::chat::reader(app, headers)
        .await
        .unwrap_or_default();
    let csrf = scope.edits.then(|| crate::pages::chat::csrf(app, &reader));
    let engine_in_browser = app.config.cad.is_some();
    let head = html! {
        link rel="stylesheet" href=(crate::cad_lab::ASSET_CSS);
        script src=(crate::cad_lab::ASSET_JS) defer {}
        @if engine_in_browser { script type="module" src=(DOC_JS) {} }
        @if plan.pro() && scope.edits && engine().is_some() {script type="module" src=(imports::SCRIPT) {}}
    };
    let mut response = crate::ui_page::UiPage::new(entry.title.clone())
        .path(format!("/cad/{doc}"))
        .head(head)
        .content(
            PageColumn::new(doc_content(
                &scope,
                doc,
                entry,
                version,
                &record,
                csrf.as_deref(),
                engine_in_browser,
                plan,
            ))
            .wide(),
        )
        .respond(headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(crate::pages::cad::CAD_POLICY),
    );
    response
}

/// The signed-in person's designs, for `/cad`: their own.
pub(crate) async fn designs(app: &App, headers: &HeaderMap) -> Option<(Scope, Index)> {
    let scope = page_scope(app, headers, None).await?;
    let index = load_index(&app.config.chat_store, &scope).await.ok()?;
    (!index.docs.is_empty()).then_some((scope, index))
}

/// A list of designs linking to their pages.
pub(crate) fn designs_list(scope: &Scope, index: &Index) -> Markup {
    html! {
        ul.oa-cad-designs {
            @for (id, entry) in &index.docs {
                li {
                    a href=(scope.page(id, None)) { (entry.title) }
                    " "
                    span.oa-cad-dim {
                        (entry.versions) @if entry.versions == 1 { " version" } @else { " versions" }
                    }
                }
            }
        }
    }
}

/// A project's designs, for its page (`None` when it has none).
pub(crate) async fn project_designs(
    app: &App,
    headers: &HeaderMap,
    project: &str,
) -> Option<Markup> {
    let scope = page_scope(app, headers, Some(project)).await?;
    let index = load_index(&app.config.chat_store, &scope).await.ok()?;
    (!index.docs.is_empty()).then(|| designs_list(&scope, &index))
}

#[cfg(test)]
mod tests;
