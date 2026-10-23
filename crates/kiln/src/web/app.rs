//! The page: the workspace (OPFS-backed), the tools (fetched by content
//! hash and compiled once), the worker that runs them, the agent loop, and
//! the log a person watches.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use futures_util::StreamExt;
use futures_util::future::{Either, select};
use js_sys::{Array, Object, Uint8Array, WebAssembly};
use nostr::pylon::BeaconBook;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, Element, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions,
    FileSystemWritableFileStream, HtmlInputElement, HtmlTextAreaElement, MessageEvent, Worker,
    WorkerOptions, WorkerType,
};

use super::relay::Relay;
use super::util::{get, now_ms, random32, set, sleep, text};
use crate::agent::{Agent, Command};
use crate::cj::{self, Key};
use crate::vfs::Vfs;

thread_local! {static PREVIEW_LAB: Cell<crate::shader::Lab> = const {Cell::new(crate::shader::Lab::Haunt)};}
fn shader_path() -> &'static str {
    PREVIEW_LAB.with(|lab| lab.get().shader())
}
fn scene_path() -> &'static str {
    PREVIEW_LAB.with(|lab| lab.get().scene())
}
const REPO: &str = "OpenAgentsInc/openagents";
/// Where the base snapshot keeps what was loaded.
const REPO_META: &str = ".kiln/repo.json";
const REMOTE_WAIT_SECS: u64 = 900;
const MAX_STEPS: usize = 16;
const TOOL_TIMEOUT_MS: i32 = 30_000;

struct Tool {
    hash: String,
    module: WebAssembly::Module,
}

#[derive(Default)]
struct Metrics {
    page: Value,
    tools: Value,
    runs: Vec<Value>,
    steps: Vec<Value>,
    pylon: Vec<Value>,
}

struct App {
    doc: Document,
    vfs: RefCell<Vfs>,
    tools: RefCell<BTreeMap<String, Tool>>,
    opfs: RefCell<Option<FileSystemFileHandle>>,
    /// The workspace as loaded from the repository, for the remote diff.
    base: RefCell<Vfs>,
    opfs_base: RefCell<Option<FileSystemFileHandle>>,
    /// The connection remote Cargo runs use.
    runner_relay: RefCell<Option<Relay>>,
    /// The digest of the crate's files at the last passing `cargo test`.
    green: RefCell<Option<String>>,
    /// The NIP-34 repository address patches go to, from `config.json`.
    repo_address: RefCell<String>,
    /// The Kiln build this page runs (`tools/manifest.json`'s `kiln.build`),
    /// or `site` for the host's own build.
    build: RefCell<String>,
    /// The last `cargo test`: whether it passed, and its summary.
    last_test: RefCell<Option<(bool, String)>>,
    /// The proposed patch: its ID, review page, and the runner's check.
    patch: RefCell<Option<kiln_view::Patch>>,
    /// The last problem, in a sentence, until the next run.
    problem: RefCell<Option<String>>,
    /// Runners found by their beacons (#12457): when, and best first.
    runners: RefCell<Option<(f64, Vec<cj::Runner>)>>,
    /// Runners that just refused or didn't answer, and when.
    failing_runners: RefCell<BTreeMap<String, f64>>,
    /// The one pylon a run may use, from `?pylon=` (hex or npub).
    pin_pylon: RefCell<Option<String>>,
    /// The run as the transcript draws it.
    run_view: RefCell<crate::transcript::Run>,
    /// The tool card of the command running now, for its progress lines.
    current_tool: RefCell<Option<String>>,
    /// A newer build waits for the agent to go idle.
    pending_build: RefCell<Option<String>>,
    /// The key self-edit jobs are signed with: kept in this browser so the
    /// runner's admin list can name it.
    admin_key: RefCell<Option<Key>>,
    key: Key,
    worker: RefCell<Option<Worker>>,
    busy: Cell<bool>,
    shader_preview: Cell<bool>,
    verse: RefCell<Option<crate::verse::Workspace>>,
    stop: Cell<bool>,
    metrics: RefCell<Metrics>,
}

struct ToolRun {
    code: i32,
    output: String,
}

thread_local! {
    /// Where Kiln's files are, such as `/admin/kiln/`, when another page
    /// runs Kiln ([`run_headless`]); empty for Kiln's own page.
    static VERSE: RefCell<Option<crate::verse::Context>> = const { RefCell::new(None) };
    static BASE: RefCell<String> = const { RefCell::new(String::new()) };
    /// The controls Kiln reads and writes when another page runs it: a
    /// detached copy of its own page's, never shown.
    static HEADLESS: RefCell<Option<Element>> = const { RefCell::new(None) };
}

/// The controls a page without Kiln's own markup gets, by ID.
const HEADLESS_CONTROLS: &str = r#"<textarea id="task"></textarea>
<input id="slice" value="crates/memory-stream"><input id="ref" value="main">
<input id="model" value="flint"><input type="checkbox" id="fallback" checked>
<input id="relay" value="wss://relay.openagents.com"><input id="runner">
<span id="admin-key"></span><textarea id="credential"></textarea><div id="identity"></div>
<span id="key"></span><span id="env"></span><span id="build"></span>
<div id="usage"></div><div id="files"></div><div id="viewer-name"></div><pre id="viewer"></pre>
<div id="metrics"></div><div id="log"></div>"#;

/// `path` under [`BASE`] when it's relative.
fn asset(path: &str) -> String {
    if path.starts_with('/') || path.contains("://") {
        return path.to_owned();
    }
    let path = path.strip_prefix("./").unwrap_or(path);
    BASE.with(|base| format!("{}{path}", base.borrow()))
}

fn headless() -> bool {
    HEADLESS.with(|h| h.borrow().is_some())
}

fn el(doc: &Document, id: &str) -> Element {
    if let Some(found) = HEADLESS.with(|h| {
        h.borrow()
            .as_ref()
            .and_then(|root| root.query_selector(&format!("#{id}")).ok().flatten())
    }) {
        return found;
    }
    doc.get_element_by_id(id)
        .unwrap_or_else(|| wasm_bindgen::throw_str(&format!("#{id} is missing")))
}

/// Run Kiln inside another page, such as the Grid's terminal (#12456),
/// with its files under `base` and no markup of its own: the page draws
/// Kiln from `kilnState()` and drives it through the `kiln*` functions.
pub async fn run_lab(base: String, lab: crate::shader::Lab) {
    PREVIEW_LAB.with(|current| current.set(lab));
    run_headless(base).await;
}

pub async fn run_verse(base: String, raw: String) -> Result<(), JsValue> {
    if raw.len() > 24_000 {
        return Err(JsValue::from_str("Choose a smaller project context."));
    }
    let context: crate::verse::Context = serde_json::from_str(&raw)
        .map_err(|_| JsValue::from_str("Choose a valid project context."))?;
    crate::verse::Workspace::new(context.clone()).map_err(|e| JsValue::from_str(&e))?;
    VERSE.with(|v| *v.borrow_mut() = Some(context));
    run_headless(base).await;
    Ok(())
}

pub async fn run_headless(base: String) {
    let base = if base.ends_with('/') {
        base
    } else {
        format!("{base}/")
    };
    BASE.with(|b| *b.borrow_mut() = base);
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.create_element("div").ok())
        .unwrap_or_else(|| wasm_bindgen::throw_str("no document"));
    root.set_inner_html(HEADLESS_CONTROLS);
    HEADLESS.with(|h| *h.borrow_mut() = Some(root));
    run().await;
}

pub async fn run() {
    let window = web_sys::window().unwrap_or_else(|| wasm_bindgen::throw_str("no window"));
    let doc = window
        .document()
        .unwrap_or_else(|| wasm_bindgen::throw_str("no document"));
    let key = loop {
        if let Ok(key) = Key::from_bytes(random32()) {
            break key;
        }
    };
    let verse = VERSE
        .with(|v| v.borrow_mut().take())
        .and_then(|v| crate::verse::Workspace::new(v).ok());
    let scoped_vfs = verse.as_ref().map(|v| v.vfs.clone()).unwrap_or_default();
    let scoped = verse.is_some();
    let app = Rc::new(App {
        doc,
        vfs: RefCell::new(scoped_vfs),
        tools: RefCell::new(BTreeMap::new()),
        opfs: RefCell::new(None),
        base: RefCell::new(Vfs::default()),
        opfs_base: RefCell::new(None),
        runner_relay: RefCell::new(None),
        green: RefCell::new(None),
        repo_address: RefCell::new(String::new()),
        build: RefCell::new(String::new()),
        pending_build: RefCell::new(None),
        run_view: RefCell::new(crate::transcript::Run::default()),
        pin_pylon: RefCell::new(None),
        last_test: RefCell::new(None),
        runners: RefCell::new(None),
        failing_runners: RefCell::new(BTreeMap::new()),
        patch: RefCell::new(None),
        problem: RefCell::new(None),
        current_tool: RefCell::new(None),
        admin_key: RefCell::new(None),
        key,
        worker: RefCell::new(None),
        busy: Cell::new(false),
        shader_preview: Cell::new(false),
        verse: RefCell::new(verse),
        stop: Cell::new(false),
        metrics: RefCell::new(Metrics::default()),
    });
    app.wire();
    let (storage, tools) = if scoped {
        ("Project workspace".into(), "Project tools".into())
    } else {
        (app.open_storage().await, app.load_tools().await)
    };
    let isolated = get(&window, "crossOriginIsolated").as_bool() == Some(true);
    el(&app.doc, "key").set_text_content(Some(&app.key.npub()));
    el(&app.doc, "env").set_text_content(Some(&format!(
        "{storage} · {tools} · cross-origin isolated: {}",
        if isolated { "yes" } else { "no" }
    )));
    app.page_metrics();
    app.render_files();
    if !scoped {
        app.restore_session();
    }
    app.render_metrics();
    if !scoped {
        app.load_admin_key();
    }
    *app.build.borrow_mut() = current_build().await.unwrap_or_else(|| "site".into());
    if let Some(node) = app.doc.get_element_by_id("build") {
        node.set_text_content(Some(&app.build.borrow()));
    }
    if !headless() {
        wasm_bindgen_futures::spawn_local(app.clone().watch_builds());
    }
    // Another page's address is its own business.
    let search = if headless() {
        String::new()
    } else {
        window.location().search().unwrap_or_default()
    };
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok();
    let param = |name: &str| params.as_ref().and_then(|p| p.get(name));
    // The host names the runner, the repository, and the relay in
    // `config.json` beside the page; every URL here is relative, so the
    // page works under a path prefix such as `/admin/kiln/`.
    let mut site_identity = true;
    if let Ok(config) = fetch_json("config.json").await {
        site_identity = config["site_identity"].as_bool().unwrap_or(true);
        if let Some(runner) = config["runner"].as_str().filter(|r| !r.is_empty()) {
            let input: HtmlInputElement = el(&app.doc, "runner").unchecked_into();
            input.set_value(runner);
        }
        if let Some(repo) = config["repo"].as_str() {
            *app.repo_address.borrow_mut() = repo.to_string();
        }
        if let Some(relay) = config["relay"].as_str().filter(|r| !r.is_empty()) {
            let input: HtmlInputElement = el(&app.doc, "relay").unchecked_into();
            input.set_value(relay);
        }
    }
    if headless() && site_identity {
        // The Grid can vouch for a visitor's key. Private local previews
        // disable this request in their configuration.
        wasm_bindgen_futures::spawn_local(app.clone().site_identity());
    }
    if let Some(model) = param("model") {
        let input: HtmlInputElement = el(&app.doc, "model").unchecked_into();
        input.set_value(&model);
    }
    if let Some(pin) = param("pylon").and_then(|p| nostr_hex(&p)) {
        *app.pin_pylon.borrow_mut() = Some(pin);
    }
    if let Some(relay) = param("relay") {
        let input: HtmlInputElement = el(&app.doc, "relay").unchecked_into();
        input.set_value(&relay);
    }
    if let Some(runner) = param("runner") {
        let input: HtmlInputElement = el(&app.doc, "runner").unchecked_into();
        input.set_value(&runner);
    }
    if let Some(slice) = param("load") {
        app.clone()
            .load_repo(slice, param("ref").unwrap_or_else(|| "main".into()))
            .await;
    }
    if let Some(line) = param("cargo") {
        // Run one remote Cargo command without the model, for checks.
        let run = app.run_command(&Command::Run(line)).await;
        set(&window, "kilnCargo", run.map_or(-1, |r| r.code));
    }
    if param("selftest").is_some() {
        app.clone().selftest().await;
    }
    if let Some(task) = param("task") {
        let area: HtmlTextAreaElement = el(&app.doc, "task").unchecked_into();
        area.set_value(&task);
        if param("auto").is_some() {
            app.clone().agent(task).await;
        }
    }
    app.expose();
    // A reload (Kiln's own hot reload among them) mustn't repeat a load or
    // a task the address asked for once.
    if !headless()
        && let Ok(history) = window.history()
    {
        let path = window.location().pathname().unwrap_or_default();
        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&path));
    }
    set(&window, "kilnReady", true);
}

impl App {
    fn log(&self, class: &str, text: &str) {
        if matches!(class, "error" | "err") {
            *self.problem.borrow_mut() = Some(text.to_owned());
        }
        let log = el(&self.doc, "log");
        if let Ok(line) = self.doc.create_element("div") {
            line.set_class_name(class);
            line.set_text_content(Some(text));
            let _ = log.append_child(&line);
            log.set_scroll_top(log.scroll_height());
            // Nobody reads the log of a page run elsewhere; keep it short.
            while headless()
                && log.child_element_count() > 400
                && let Some(first) = log.first_element_child()
            {
                first.remove();
            }
        }
    }

    fn wire(self: &Rc<Self>) {
        let on = |id: &str, f: Box<dyn Fn(web_sys::Event)>| {
            let closure = Closure::<dyn Fn(web_sys::Event)>::new(f);
            // A host page older than this module may lack a control.
            if let Some(node) = self.doc.get_element_by_id(id) {
                let _ = node
                    .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
            }
            closure.forget();
        };
        let app = self.clone();
        on(
            "run",
            Box::new(move |_| {
                let area: HtmlTextAreaElement = el(&app.doc, "task").unchecked_into();
                let task = area.value();
                if task.trim().is_empty() || app.busy.get() {
                    return;
                }
                wasm_bindgen_futures::spawn_local(app.clone().agent(task));
            }),
        );
        let app = self.clone();
        on(
            "selftest",
            Box::new(move |_| {
                if !app.busy.get() {
                    wasm_bindgen_futures::spawn_local(app.clone().selftest());
                }
            }),
        );
        let app = self.clone();
        on("stop", Box::new(move |_| app.stop.set(true)));
        for (id, raw) in [("view-transcript", false), ("view-raw", true)] {
            let app = self.clone();
            on(
                id,
                Box::new(move |_| {
                    let toggle = |id: &str, hidden: bool| {
                        if let Some(node) = app.doc.get_element_by_id(id) {
                            let _ = node.class_list().toggle_with_force("hidden", hidden);
                        }
                    };
                    toggle("log", !raw);
                    toggle("gpui-mount", raw);
                    for (button, on) in [("view-transcript", !raw), ("view-raw", raw)] {
                        if let Some(node) = app.doc.get_element_by_id(button) {
                            let _ = node.class_list().toggle_with_force("on", on);
                        }
                    }
                }),
            );
        }
        let app = self.clone();
        on(
            "drawer",
            Box::new(move |_| {
                if let Some(body) = app.doc.body() {
                    let _ = body.class_list().toggle("drawer-open");
                }
            }),
        );
        let app = self.clone();
        on(
            "self-load",
            Box::new(move |_| {
                if app.busy.get() {
                    return;
                }
                let reference: HtmlInputElement = el(&app.doc, "ref").unchecked_into();
                let reference = reference.value();
                wasm_bindgen_futures::spawn_local(
                    app.clone()
                        .load_repo("crates/kiln,crates/kiln-tools".into(), reference),
                );
            }),
        );
        for (id, action) in [
            ("self-publish", "publish"),
            ("self-revert", "revert"),
            ("self-reset", "reset"),
        ] {
            let app = self.clone();
            on(
                id,
                Box::new(move |_| {
                    if app.busy.get() {
                        return;
                    }
                    wasm_bindgen_futures::spawn_local(app.clone().self_edit(action));
                }),
            );
        }
        let app = self.clone();
        on(
            "reload-now",
            Box::new(move |_| {
                let app = app.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(build) = app.pending_build.borrow().clone() {
                        app.swap(&build).await;
                    }
                });
            }),
        );
        let app = self.clone();
        on(
            "load",
            Box::new(move |_| {
                if app.busy.get() {
                    return;
                }
                let slice: HtmlInputElement = el(&app.doc, "slice").unchecked_into();
                let reference: HtmlInputElement = el(&app.doc, "ref").unchecked_into();
                let (slice, reference) = (slice.value(), reference.value());
                wasm_bindgen_futures::spawn_local(app.clone().load_repo(slice, reference));
            }),
        );
        let app = self.clone();
        on(
            "reset",
            Box::new(move |_| {
                if app.busy.get() {
                    return;
                }
                let app = app.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    *app.vfs.borrow_mut() = Vfs::default();
                    *app.base.borrow_mut() = Vfs::default();
                    app.persist().await;
                    app.persist_base().await;
                    app.render_files();
                    el(&app.doc, "viewer").set_text_content(Some(""));
                    app.log("info", "Cleared the workspace.");
                });
            }),
        );
        let app = self.clone();
        on(
            "files",
            Box::new(move |event| {
                let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
                    return;
                };
                let Some(path) = target.get_attribute("data-path") else {
                    return;
                };
                let vfs = app.vfs.borrow();
                let shown = vfs.read(&path).map_or_else(
                    |_| String::new(),
                    |b| String::from_utf8_lossy(b).into_owned(),
                );
                el(&app.doc, "viewer-name").set_text_content(Some(&path));
                el(&app.doc, "viewer").set_text_content(Some(&shown));
            }),
        );
    }

    /// Open the OPFS snapshot and load it; fall back to memory.
    async fn open_storage(&self) -> String {
        let started = now_ms();
        let result: Result<(FileSystemFileHandle, Vec<u8>), JsValue> = async {
            let storage = web_sys::window().ok_or("no window")?.navigator().storage();
            if let Ok(p) = storage.persist() {
                let _ = JsFuture::from(p).await;
            }
            let dir: FileSystemDirectoryHandle =
                JsFuture::from(storage.get_directory()).await?.dyn_into()?;
            let options = FileSystemGetFileOptions::new();
            options.set_create(true);
            let handle: FileSystemFileHandle = JsFuture::from(dir.get_file_handle_with_options(
                PREVIEW_LAB.with(|lab| lab.get().snapshot()),
                &options,
            ))
            .await?
            .dyn_into()?;
            let file: web_sys::File = JsFuture::from(handle.get_file()).await?.dyn_into()?;
            let bytes = Uint8Array::new(&JsFuture::from(file.array_buffer()).await?).to_vec();
            let base: FileSystemFileHandle = JsFuture::from(dir.get_file_handle_with_options(
                PREVIEW_LAB.with(|lab| lab.get().base_snapshot()),
                &options,
            ))
            .await?
            .dyn_into()?;
            let file: web_sys::File = JsFuture::from(base.get_file()).await?.dyn_into()?;
            let base_bytes = Uint8Array::new(&JsFuture::from(file.array_buffer()).await?).to_vec();
            if let Ok(vfs) = Vfs::decode(&base_bytes) {
                *self.base.borrow_mut() = vfs;
            }
            *self.opfs_base.borrow_mut() = Some(base);
            Ok((handle, bytes))
        }
        .await;
        match result {
            Ok((handle, bytes)) => {
                *self.opfs.borrow_mut() = Some(handle);
                let ms = now_ms() - started;
                if bytes.is_empty() {
                    self.log(
                        "info",
                        &format!("OPFS is ready; the workspace is new ({ms:.0} ms)."),
                    );
                } else {
                    match Vfs::decode(&bytes) {
                        Ok(vfs) => {
                            let (files, size) = vfs.usage();
                            *self.vfs.borrow_mut() = vfs;
                            self.log(
                                "info",
                                &format!("Loaded the workspace from OPFS: {files} files, {size} bytes in {ms:.0} ms."),
                            );
                        }
                        Err(e) => self.log(
                            "error",
                            &format!("The saved workspace is unreadable ({e}); starting empty."),
                        ),
                    }
                }
                self.metrics.borrow_mut().page["opfs_load_ms"] = json!(ms);
                "workspace: OPFS".into()
            }
            Err(error) => {
                self.log(
                    "error",
                    &format!(
                        "OPFS is unavailable ({}); the workspace lives in memory.",
                        text(&error)
                    ),
                );
                "workspace: memory only".into()
            }
        }
    }

    /// Write the base snapshot to OPFS.
    async fn persist_base(&self) {
        let Some(handle) = self.opfs_base.borrow().clone() else {
            return;
        };
        let bytes = self.base.borrow().encode();
        let result: Result<(), JsValue> = async {
            let stream: FileSystemWritableFileStream =
                JsFuture::from(handle.create_writable()).await?.dyn_into()?;
            JsFuture::from(stream.write_with_u8_array(&bytes)?).await?;
            JsFuture::from(stream.close()).await?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            self.log(
                "error",
                &format!("Saving the base to OPFS failed: {}", text(&error)),
            );
        }
    }

    /// Write the snapshot to OPFS.
    async fn persist(&self) {
        let Some(handle) = self.opfs.borrow().clone() else {
            return;
        };
        let bytes = self.vfs.borrow().encode();
        let started = now_ms();
        let result: Result<(), JsValue> = async {
            let stream: FileSystemWritableFileStream =
                JsFuture::from(handle.create_writable()).await?.dyn_into()?;
            JsFuture::from(stream.write_with_u8_array(&bytes)?).await?;
            JsFuture::from(stream.close()).await?;
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {
                let ms = now_ms() - started;
                let mut m = self.metrics.borrow_mut();
                m.page["opfs_save_ms_last"] = json!(ms);
                m.page["opfs_snapshot_bytes"] = json!(bytes.len());
            }
            Err(error) => self.log("error", &format!("Saving to OPFS failed: {}", text(&error))),
        }
    }

    /// Fetch the tool manifest and each tool by its hash, check the hash,
    /// and compile it once.
    async fn load_tools(&self) -> String {
        let started = now_ms();
        let result: Result<(usize, usize), String> = async {
            let manifest = fetch_bytes("tools/manifest.json").await?;
            let manifest: Value = serde_json::from_slice(&manifest).map_err(|e| e.to_string())?;
            let entries = manifest["tools"]
                .as_object()
                .ok_or("the manifest has no tools")?;
            let mut bytes = 0;
            for (name, entry) in entries {
                let hash = entry["sha256"]
                    .as_str()
                    .ok_or("a tool has no hash")?
                    .to_string();
                let wasm = fetch_bytes(&format!("tools/{hash}.wasm")).await?;
                if crate::sha256_hex(&wasm) != hash {
                    return Err(format!("{name} does not match its hash"));
                }
                bytes += wasm.len();
                let module: WebAssembly::Module = JsFuture::from(WebAssembly::compile(
                    &Uint8Array::from(wasm.as_slice()).into(),
                ))
                .await
                .map_err(|e| text(&e))?
                .unchecked_into();
                self.tools
                    .borrow_mut()
                    .insert(name.clone(), Tool { hash, module });
            }
            Ok((entries.len(), bytes))
        }
        .await;
        let ms = now_ms() - started;
        match result {
            Ok((count, bytes)) => {
                let names: Vec<String> = self
                    .tools
                    .borrow()
                    .iter()
                    .map(|(n, t)| format!("{n}@{}", &t.hash[..8]))
                    .collect();
                self.log(
                    "info",
                    &format!(
                        "Loaded {count} tools ({} KiB) by content hash and compiled them in {ms:.0} ms: {}.",
                        bytes / 1024,
                        names.join(" ")
                    ),
                );
                self.metrics.borrow_mut().tools =
                    json!({"count": count, "bytes": bytes, "fetch_and_compile_ms": ms});
                format!("{count} tools")
            }
            Err(e) => {
                self.log("error", &format!("Could not load the tools: {e}"));
                "no tools".into()
            }
        }
    }

    fn page_metrics(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let performance = get(&window, "performance");
        let Ok(entries) = js_sys::Reflect::get(&performance, &"getEntriesByType".into())
            .and_then(|f| f.dyn_into::<js_sys::Function>())
            .and_then(|f| f.call1(&performance, &"resource".into()))
        else {
            return;
        };
        let (mut transfer, mut decoded, mut wasm) = (0.0, 0.0, 0.0);
        for entry in Array::from(&entries).iter() {
            let t = get(&entry, "transferSize").as_f64().unwrap_or(0.0);
            let d = get(&entry, "decodedBodySize").as_f64().unwrap_or(0.0);
            transfer += t;
            decoded += d;
            if get(&entry, "name")
                .as_string()
                .is_some_and(|n| n.ends_with("kiln_bg.wasm"))
            {
                wasm = d;
            }
        }
        let mut m = self.metrics.borrow_mut();
        m.page["resources_transfer_bytes"] = json!(transfer);
        m.page["resources_decoded_bytes"] = json!(decoded);
        m.page["kiln_wasm_bytes"] = json!(wasm);
        m.page["ready_ms"] = json!(now_ms());
        drop(m);
        self.render_metrics();
    }

    fn render_metrics(&self) {
        let m = self.metrics.borrow();
        let all = json!({
            "page": m.page,
            "tools": m.tools,
            "tool_runs": m.runs,
            "steps": m.steps,
            "pylon": m.pylon,
        });
        let mut lines = Vec::new();
        let kib = |v: &Value| {
            v.as_f64()
                .map_or("-".into(), |b| format!("{:.0} KiB", b / 1024.0))
        };
        let ms = |v: &Value| v.as_f64().map_or("-".into(), |b| format!("{b:.0} ms"));
        lines.push(format!(
            "page: {} decoded ({} kiln wasm), ready at {}",
            kib(&m.page["resources_decoded_bytes"]),
            kib(&m.page["kiln_wasm_bytes"]),
            ms(&m.page["ready_ms"])
        ));
        lines.push(format!(
            "tools: {} in {}",
            kib(&m.tools["bytes"]),
            ms(&m.tools["fetch_and_compile_ms"])
        ));
        if !m.runs.is_empty() {
            let mut walls: Vec<f64> = m
                .runs
                .iter()
                .filter_map(|r| r["wall_ms"].as_f64())
                .collect();
            walls.sort_by(f64::total_cmp);
            let first = m.runs[0]["wall_ms"].as_f64().unwrap_or(0.0);
            lines.push(format!(
                "tool runs: {} · first {first:.0} ms · median {:.0} ms · max {:.0} ms",
                walls.len(),
                walls[walls.len() / 2],
                walls[walls.len() - 1]
            ));
        }
        if !m.pylon.is_empty() {
            let mut answers: Vec<f64> = m
                .pylon
                .iter()
                .filter_map(|r| r["answer_ms"].as_f64())
                .collect();
            answers.sort_by(f64::total_cmp);
            lines.push(format!(
                "pylon answers: {} · median {:.0} ms · max {:.0} ms",
                answers.len(),
                answers[answers.len() / 2],
                answers[answers.len() - 1]
            ));
        }
        if !m.steps.is_empty() {
            let mut totals: Vec<f64> = m
                .steps
                .iter()
                .filter_map(|r| r["total_ms"].as_f64())
                .collect();
            totals.sort_by(f64::total_cmp);
            lines.push(format!(
                "agent steps: {} · median {:.0} ms",
                totals.len(),
                totals[totals.len() / 2]
            ));
        }
        el(&self.doc, "metrics").set_text_content(Some(&lines.join("\n")));
        if let Some(window) = web_sys::window() {
            if let Ok(value) = js_sys::JSON::parse(&all.to_string()) {
                set(&window, "kilnMetrics", value);
            }
        }
    }

    fn listing(&self) -> String {
        let vfs = self.vfs.borrow();
        let mut out = String::new();
        for (path, node) in vfs.iter() {
            if path.is_empty() {
                continue;
            }
            if node.is_dir() {
                out.push_str(&format!("{path}/\n"));
            } else {
                out.push_str(&format!("{path}  {} bytes\n", node.size()));
            }
        }
        out
    }

    fn render_files(&self) {
        let list = el(&self.doc, "files");
        list.set_inner_html("");
        let vfs = self.vfs.borrow();
        let (count, bytes) = vfs.usage();
        for (path, node) in vfs.iter() {
            if path.is_empty() {
                continue;
            }
            if let Ok(item) = self.doc.create_element("div") {
                let depth = path.matches('/').count();
                let name = path.rsplit('/').next().unwrap_or(path);
                if node.is_dir() {
                    item.set_class_name("dir");
                    item.set_text_content(Some(&format!("{}{name}/", "  ".repeat(depth))));
                } else {
                    item.set_class_name("file");
                    let _ = item.set_attribute("data-path", path);
                    item.set_text_content(Some(&format!(
                        "{}{name}  {}",
                        "  ".repeat(depth),
                        node.size()
                    )));
                }
                let _ = list.append_child(&item);
            }
        }
        // Name the crates the workspace holds, so it's clear what the agent
        // is looking at whatever the Repository field says.
        let crates: Vec<&str> = vfs
            .iter()
            .filter(|(path, node)| node.is_dir() && path.matches('/').count() == 1)
            .filter_map(|(path, _)| path.strip_prefix("crates/"))
            .collect();
        let held = if crates.is_empty() {
            String::new()
        } else {
            format!("{} · ", crates.join(", "))
        };
        el(&self.doc, "usage")
            .set_text_content(Some(&format!("{held}{count} files, {bytes} bytes")));
    }

    fn take_worker(&self) -> Result<Worker, String> {
        if let Some(worker) = self.worker.borrow_mut().take() {
            return Ok(worker);
        }
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        Worker::new_with_options(&asset("worker.js"), &options).map_err(|e| text(&e))
    }

    /// Run one tool in the worker against the workspace, apply what it
    /// changed, and persist.
    async fn run_tool(&self, argv: &[String], stdin: &[u8]) -> ToolRun {
        let started = now_ms();
        let name = argv[0].clone();
        let Some(module) = self.tools.borrow().get(&name).map(|t| t.module.clone()) else {
            return ToolRun {
                code: 127,
                output: format!("no tool named {name}"),
            };
        };
        let fresh = self.worker.borrow().is_none();
        let worker = match self.take_worker() {
            Ok(worker) => worker,
            Err(e) => {
                return ToolRun {
                    code: 126,
                    output: format!("could not start a worker: {e}"),
                };
            }
        };
        let (tx, mut rx) = futures_channel::mpsc::unbounded::<Result<JsValue, String>>();
        let tx2 = tx.clone();
        let on_message = Closure::<dyn Fn(MessageEvent)>::new(move |m: MessageEvent| {
            let _ = tx.unbounded_send(Ok(m.data()));
        });
        let on_error =
            Closure::<dyn Fn(web_sys::ErrorEvent)>::new(move |e: web_sys::ErrorEvent| {
                let _ = tx2.unbounded_send(Err(e.message()));
            });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        let message = Object::new();
        set(&message, "module", module);
        set(
            &message,
            "argv",
            argv.iter().map(|a| JsValue::from_str(a)).collect::<Array>(),
        );
        set(&message, "stdin", Uint8Array::from(stdin));
        let before = self.vfs.borrow().encode();
        set(&message, "fs", Uint8Array::from(before.as_slice()));
        if let Err(e) = worker.post_message(&message) {
            worker.terminate();
            return ToolRun {
                code: 126,
                output: format!("could not post to the worker: {}", text(&e)),
            };
        }
        let reply = match select(rx.next(), sleep(TOOL_TIMEOUT_MS)).await {
            Either::Left((Some(reply), _)) => reply,
            Either::Left((None, _)) => Err("the worker went away".into()),
            Either::Right(_) => {
                worker.terminate();
                return ToolRun {
                    code: 124,
                    output: format!("stopped: still running after {} s", TOOL_TIMEOUT_MS / 1000),
                };
            }
        };
        worker.set_onmessage(None);
        worker.set_onerror(None);
        let reply = match reply {
            Ok(reply) => {
                *self.worker.borrow_mut() = Some(worker);
                reply
            }
            Err(e) => {
                worker.terminate();
                return ToolRun {
                    code: 126,
                    output: format!("the worker failed: {e}"),
                };
            }
        };
        if let Some(error) = reply.as_string() {
            return ToolRun {
                code: 126,
                output: format!("the worker failed: {error}"),
            };
        }
        let code = get(&reply, "code").as_f64().unwrap_or(1.0) as i32;
        let stdout = Uint8Array::new(&get(&reply, "stdout")).to_vec();
        let stderr = Uint8Array::new(&get(&reply, "stderr")).to_vec();
        let fs = Uint8Array::new(&get(&reply, "fs")).to_vec();
        if let Ok(next) = Vfs::decode(&fs) {
            let (changed, removed) = self.vfs.borrow().diff(&next);
            if !changed.is_empty() || !removed.is_empty() {
                *self.vfs.borrow_mut() = next;
                self.persist().await;
                self.render_files();
                if changed.iter().chain(&removed).any(|p| p == shader_path()) {
                    self.notify_shader();
                }
                if changed.iter().chain(&removed).any(|p| p == scene_path()) {
                    self.notify_scene();
                }
            }
        }
        let wall = now_ms() - started;
        self.metrics.borrow_mut().runs.push(json!({
            "tool": name,
            "wall_ms": wall,
            "instantiate_ms": get(&reply, "instantiate_ms").as_f64(),
            "run_ms": get(&reply, "run_ms").as_f64(),
            "host_calls": get(&reply, "host_calls").as_f64(),
            "fresh_worker": fresh,
        }));
        self.render_metrics();
        let mut output = String::from_utf8_lossy(&stdout).into_owned();
        if !stderr.is_empty() {
            output.push_str(&String::from_utf8_lossy(&stderr));
        }
        ToolRun { code, output }
    }

    async fn run_command(&self, command: &Command) -> Option<ToolRun> {
        if let Some(workspace) = self.verse.borrow_mut().as_mut() {
            let result = workspace.execute(command);
            *self.vfs.borrow_mut() = workspace.vfs.clone();
            self.render_files();
            return Some(match result {
                Ok(output) => ToolRun { code: 0, output },
                Err(output) => ToolRun { code: 1, output },
            });
        }

        match command.call() {
            Ok(Some(call)) => {
                self.log("cmd", &format!("$ {}", command.label()));
                let card = crate::transcript::call_for(&call.argv);
                let file_path = matches!(card.tool, "write" | "edit")
                    .then(|| card.path.clone())
                    .flatten()
                    .and_then(|p| crate::vfs::resolve("", &p).ok());
                let before = file_path.as_ref().and_then(|p| {
                    self.vfs
                        .borrow()
                        .read(p)
                        .ok()
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                });
                let id = self.run_view.borrow_mut().tool(&card, now_ms());
                *self.current_tool.borrow_mut() = Some(id.clone());
                self.push_transcript();
                let run = if call.argv[0] == "cargo" {
                    self.remote_cargo(&call.argv).await
                } else {
                    self.read_sparse(&call.argv).await;
                    self.run_tool(&call.argv, &call.stdin).await
                };
                *self.current_tool.borrow_mut() = None;
                let after = file_path.as_ref().and_then(|p| {
                    self.vfs
                        .borrow()
                        .read(p)
                        .ok()
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                });
                let diff = match (run.code, after) {
                    (0, Some(after)) if before.as_deref() != Some(after.as_str()) => {
                        Some((before, after))
                    }
                    _ => None,
                };
                let (loud, quiet) = crate::transcript::split_noise(&run.output);
                self.run_view
                    .borrow_mut()
                    .finish(&id, loud.trim_end(), run.code != 0, diff);
                self.push_transcript();
                let shown = crate::agent::clip(loud.trim_end(), 4000);
                if !shown.is_empty() {
                    self.log(if run.code == 0 { "out" } else { "err" }, &shown);
                }
                if !quiet.is_empty() {
                    self.log("muted", &crate::agent::clip(quiet.trim_end(), 2000));
                }
                if run.code != 0 {
                    self.log("err", &format!("exit {}", run.code));
                }
                Some(run)
            }
            Ok(None) => None,
            Err(e) => {
                self.log("cmd", &format!("$ {}", command.label()));
                self.log("err", &e);
                Some(ToolRun {
                    code: 127,
                    output: e,
                })
            }
        }
    }

    /// Run the tools without a model: write a module and a test, run the
    /// tests, search, patch, and run a script.
    async fn selftest(self: Rc<Self>) {
        self.busy.set(true);
        self.log(
            "step",
            "Sandbox self-test: every command below runs as a WASI program in a worker.",
        );
        let fib = "fn fib(n) {\n    if n < 2 { return n; }\n    fib(n - 1) + fib(n - 2)\n}\n";
        let test = "import \"fib\" as f;\n\nfn test_small() {\n    assert_eq(f::fib(0), 0);\n    assert_eq(f::fib(1), 1);\n}\n\nfn test_ten() {\n    assert_eq(f::fib(10), 55);\n}\n";
        let main = "import \"fib\" as f;\nfor i in 0..12 { print(`fib(${i}) = ${f::fib(i)}`); }\n";
        let commands = vec![
            Command::Write {
                path: "selftest/fib.rhai".into(),
                content: fib.replace("fib(n - 1) + fib(n - 2)", "fib(n - 1) + fib(n - 1)"),
            },
            Command::Write {
                path: "selftest/fib_test.rhai".into(),
                content: test.into(),
            },
            Command::Write {
                path: "selftest/main.rhai".into(),
                content: main.into(),
            },
            Command::Run("test selftest".into()),
            Command::Run("grep -i \"fib(n - 1)\" selftest".into()),
            Command::Patch {
                path: "selftest/fib.rhai".into(),
                search: "fib(n - 1) + fib(n - 1)".into(),
                replace: "fib(n - 1) + fib(n - 2)".into(),
            },
            Command::Run("test selftest".into()),
            Command::Run("rhai selftest/main.rhai".into()),
            Command::Run("cat selftest/fib.rhai".into()),
            Command::Run("ls -R".into()),
        ];
        let mut codes = Vec::new();
        for command in &commands {
            if let Some(run) = self.run_command(command).await {
                codes.push(run.code);
            }
        }
        let expected = [0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        let ok = codes == expected;
        self.log(
            if ok { "done" } else { "error" },
            &if ok {
                "Self-test passed: the first test run failed on the planted bug, the patch fixed it, and the second run passed.".to_string()
            } else {
                format!("Self-test failed: exit codes {codes:?}, expected {expected:?}.")
            },
        );
        if let Some(window) = web_sys::window() {
            set(&window, "kilnSelftest", ok);
        }
        self.busy.set(false);
    }

    /// The agent loop against Pylons.
    async fn agent(self: Rc<Self>, task: String) {
        self.busy.set(true);
        self.stop.set(false);
        let model: HtmlInputElement = el(&self.doc, "model").unchecked_into();
        let model = model.value().trim().to_string();
        let want = (!model.is_empty()).then_some(model.as_str());
        let fallback: HtmlInputElement = el(&self.doc, "fallback").unchecked_into();
        let fallback = fallback.checked();
        let relay_url: HtmlInputElement = el(&self.doc, "relay").unchecked_into();
        let relay_url = relay_url.value();
        self.log("task", &format!("Task: {task}"));
        self.run_view.borrow_mut().start(&task, now_ms());
        self.run_view
            .borrow_mut()
            .working(Some("Asking a Pylon"), now_ms());
        self.push_transcript();
        let mut agent = if let Some(workspace) = self.verse.borrow().as_ref() {
            match workspace.agent(&task) {
                Ok(agent) => agent,
                Err(error) => {
                    *self.problem.borrow_mut() = Some(error);
                    self.busy.set(false);
                    return;
                }
            }
        } else {
            Agent::new(&task, cj::LEGACY_MAX_INPUT_BYTES)
        };
        if self.verse.borrow().is_some() {
            agent.instructions = crate::verse::INSTRUCTIONS.into();
        } else if let Some(repo) = self.repo() {
            let crates: Vec<String> = repo["crates"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| c.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            agent.instructions =
                crate::agent::RUST_INSTRUCTIONS.replace("{crates}", &crates.join(", "));
        } else if self.shader_preview.get() {
            agent.instructions = crate::shader::INSTRUCTIONS.to_owned();
        }
        let instructions = agent.instructions.clone();
        let mut relay: Option<Relay> = None;
        let mut book = BeaconBook::default();
        let mut book_at = f64::NEG_INFINITY;
        let mut failing = BTreeSet::new();
        // A tighter prompt budget after a pylon refused one as too long.
        let mut cap: Option<usize> = None;
        let mut published = false;
        let run_started = now_ms();
        // Steps count model answers; waiting on a busy pylon or a dropped
        // connection doesn't use one, within an overall hour.
        let mut step = 0;
        while step < MAX_STEPS && now_ms() - run_started < 3_600_000.0 {
            if self.stop.get() {
                self.log("info", "Stopped.");
                break;
            }
            let step_started = now_ms();
            if relay.as_ref().is_none_or(|r| r.closed) {
                let started = now_ms();
                match Relay::connect(&relay_url, &self.key).await {
                    Ok(r) => {
                        self.log(
                            "info",
                            &format!(
                                "Connected to {relay_url} as the agent's own key in {:.0} ms.",
                                now_ms() - started
                            ),
                        );
                        relay = Some(r);
                    }
                    Err(e) => {
                        self.log("error", &e);
                        sleep(2_000).await;
                        continue;
                    }
                }
            }
            let Some(conn) = relay.as_mut() else { continue };
            if now_ms() - book_at > 60_000.0 {
                let started = now_ms();
                match conn.beacons().await {
                    Ok(b) => {
                        book = b;
                        book_at = now_ms();
                        // `?pylon=KEY` keeps the run on one pylon.
                        if let Some(pin) = self.pin_pylon.borrow().as_deref() {
                            for (_, beacon) in book.iter() {
                                if beacon.provider != pin {
                                    failing.insert(beacon.provider.clone());
                                }
                            }
                        }
                        let pylons = cj::describe(&book, super::util::unix());
                        self.log(
                            "info",
                            &format!(
                                "Read {} beacons in {:.0} ms; {} pylons can take a job: {}",
                                book.iter().count(),
                                now_ms() - started,
                                pylons.len(),
                                pylons.join("; ")
                            ),
                        );
                    }
                    Err(e) => {
                        self.log("error", &e);
                        relay = None;
                        continue;
                    }
                }
            }
            // The preferred model's pylon may be busy with the previous
            // step; wait for its slot before settling for another model.
            if let Some(want) = want
                && cj::choose(&book, super::util::unix(), Some(want), false).is_none()
                && cj::serves_somewhere(&book, super::util::unix(), want)
            {
                self.log("info", &format!("Waiting for a free {want} slot."));
                let until = now_ms() + 90_000.0;
                while now_ms() < until && !self.stop.get() {
                    sleep(3_000).await;
                    if let Ok(b) = conn.beacons().await {
                        book = b;
                        book_at = now_ms();
                    }
                    if cj::choose(&book, super::util::unix(), Some(want), false).is_some() {
                        break;
                    }
                }
            }
            self.log("step", &format!("Step {}", step + 1));
            let files = self.listing();
            let snapshot = agent.clone();
            let prompt = move |budget: usize| {
                let mut a = snapshot.clone();
                a.budget = budget;
                a.prompt(&files)
            };
            {
                let mut view = self.run_view.borrow_mut();
                view.turn(now_ms());
                view.working(Some("Asking a Pylon"), now_ms());
                view.status(
                    &format!("Asking Pylons for {}…", want.unwrap_or("any model")),
                    now_ms(),
                );
            }
            self.push_transcript();
            let app = self.clone();
            let feedback = move |note: &str| {
                app.log("info", note);
                app.run_view
                    .borrow_mut()
                    .update_status(&crate::transcript::pylon_words(note), now_ms());
                app.push_transcript();
            };
            let asked = conn
                .ask(
                    &self.key,
                    &book,
                    want,
                    fallback,
                    &failing,
                    &instructions,
                    &prompt,
                    &feedback,
                    cap,
                )
                .await;
            let asked = match asked {
                Ok(asked) => asked,
                Err(error) => {
                    self.log("error", &error.message);
                    self.run_view
                        .borrow_mut()
                        .update_status(&format!("No answer: {}", error.message), now_ms());
                    self.push_transcript();
                    if error.message.contains("limit_exceeded") && error.message.contains("tokens")
                    {
                        // Too long a prompt, not a bad pylon: ask again with
                        // less, at the limit it named.
                        let tokens = cj::refused_token_limit(&error.message).unwrap_or(2_000);
                        let bytes = (tokens * 5 / 2) as usize;
                        cap = Some(cap.map_or(bytes, |c| c.min(bytes) * 4 / 5));
                        self.log(
                            "info",
                            &format!(
                                "Asking again with a prompt under {} KiB.",
                                cap.unwrap_or(0) / 1024
                            ),
                        );
                        continue;
                    }
                    if error.message.contains("rate_limited") || error.message.contains("busy") {
                        // The pylon is full, not broken: wait and ask again.
                        sleep(10_000).await;
                        book_at = f64::NEG_INFINITY;
                        continue;
                    }
                    if let Some(pylon) = error.pylon {
                        failing.insert(pylon);
                    }
                    if conn.closed {
                        relay = None;
                    }
                    book_at = f64::NEG_INFINITY;
                    continue;
                }
            };
            step += 1;
            let model_ms = now_ms() - step_started;
            self.log(
                "info",
                &format!(
                    "{} ({}) answered in {:.1} s (first contact {}).",
                    asked.label,
                    asked.model,
                    asked.answer_ms / 1000.0,
                    asked
                        .contact_ms
                        .map_or("-".into(), |c| format!("{c:.0} ms"))
                ),
            );
            self.metrics.borrow_mut().pylon.push(json!({
                "pylon": asked.pylon,
                "label": asked.label,
                "model": asked.model,
                "discover_ms": asked.discover_ms,
                "contact_ms": asked.contact_ms,
                "answer_ms": asked.answer_ms,
                "usage": asked.usage,
            }));
            {
                let mut view = self.run_view.borrow_mut();
                view.update_status(
                    &format!(
                        "{} · answered in {:.1} s",
                        crate::transcript::who(&asked.label, &asked.model),
                        asked.answer_ms / 1000.0
                    ),
                    now_ms(),
                );
                if let Some(reasoning) = &asked.reasoning {
                    view.reasoning(reasoning, now_ms());
                }
                view.working(Some("Running tools"), now_ms());
            }
            let mut commands = agent.plan(&asked.text);
            // Run `done` last, so a reply that says done and publish does both.
            commands.sort_by_key(|c| matches!(c, Command::Done(_)));
            if commands.is_empty() {
                self.log(
                    "error",
                    &format!(
                        "The reply was not usable: {}",
                        crate::agent::clip(&asked.text, 600)
                    ),
                );
                self.run_view.borrow_mut().status(
                    "I couldn't read that reply as commands, so I'll ask again. The raw log has it.",
                    now_ms(),
                );
            } else if let Some(last) = agent.steps.last() {
                self.log("thought", &last.thought.clone());
                // The step's stated thinking draws as a thinking block when
                // the pylon sent no reasoning of its own.
                if asked.reasoning.is_some() {
                    self.run_view.borrow_mut().text(&last.thought, now_ms());
                } else {
                    self.run_view
                        .borrow_mut()
                        .reasoning(&last.thought, now_ms());
                }
            }
            self.push_transcript();
            let tools_started = now_ms();
            for command in &commands {
                if let Command::Done(summary) = command {
                    // A Rust change that passed its tests and was never
                    // published is published now, under the summary.
                    if !published
                        && let Some(repo) = self.repo()
                        && self.green.borrow().as_deref() == Some(self.slice_digest(&repo).as_str())
                        && !self.repo_address.borrow().is_empty()
                    {
                        self.log("cmd", &format!("$ publish patch: {summary}"));
                        let run = self.publish(summary, "").await;
                        self.log(if run.code == 0 { "done" } else { "err" }, &run.output);
                    }
                    agent.observe(command, 0, String::new());
                    self.log("done", &format!("Done: {summary}"));
                    self.run_view.borrow_mut().text(summary, now_ms());
                    self.push_transcript();
                    break;
                }
                if self.verse.borrow().is_some() && matches!(command, Command::Publish { .. }) {
                    agent.observe(
                        command,
                        1,
                        "Use the project controls to publish after review.".into(),
                    );
                    continue;
                }
                if let Command::Publish { subject, body } = command {
                    self.log("cmd", &format!("$ {}", command.label()));
                    let run = self.publish(subject, body).await;
                    published |= run.code == 0;
                    self.log(if run.code == 0 { "done" } else { "err" }, &run.output);
                    agent.observe(command, run.code, run.output);
                    continue;
                }
                if let Some(run) = self.run_command(command).await {
                    agent.observe(command, run.code, run.output);
                }
            }
            self.metrics.borrow_mut().steps.push(json!({
                "step": step,
                "model_ms": model_ms,
                "tools_ms": now_ms() - tools_started,
                "total_ms": now_ms() - step_started,
                "commands": commands.len(),
            }));
            self.render_metrics();
            if agent.done.is_some() {
                break;
            }
        }
        let secs = (now_ms() - run_started) / 1000.0;
        if agent.done.is_none() {
            self.log(
                "info",
                &format!(
                    "The agent stopped after {} steps ({secs:.0} s) without saying done.",
                    agent.steps.len()
                ),
            );
        } else {
            self.log(
                "info",
                &format!("Finished in {} steps and {secs:.0} s.", agent.steps.len()),
            );
        }
        self.run_view.borrow_mut().working(None, now_ms());
        self.push_transcript();
        if let Some(window) = web_sys::window() {
            set(&window, "kilnAgentDone", agent.done.is_some());
        }
        self.busy.set(false);
    }
}

impl App {
    /// What the base snapshot says was loaded from the repository.
    fn repo(&self) -> Option<Value> {
        let base = self.base.borrow();
        let bytes = base.read(REPO_META).ok()?;
        serde_json::from_slice(bytes).ok()
    }

    /// Replace the workspace with one directory of the repository at a
    /// commit: resolve `reference` to a commit, list the directory's tree,
    /// and fetch each file from raw.githubusercontent.com. Public reads
    /// only; no token.
    async fn load_repo(self: Rc<Self>, slice: String, reference: String) {
        self.busy.set(true);
        let started = now_ms();
        let slice = slice.trim().trim_matches('/').to_string();
        self.log(
            "step",
            &format!("Loading {slice} from {REPO} at {reference}"),
        );
        let slices: Vec<String> = slice
            .split(',')
            .map(|s| s.trim().trim_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let result: Result<(String, Vfs, usize, f64), String> = async {
            let commit: Value = serde_json::from_slice(
                &fetch_bytes(&format!(
                    "https://api.github.com/repos/{REPO}/commits/{reference}"
                ))
                .await?,
            )
            .map_err(|e| e.to_string())?;
            let sha = commit["sha"]
                .as_str()
                .ok_or("the commit has no sha")?
                .to_string();
            let mut wanted = Vec::new();
            for slice in &slices {
                let tree: Value = serde_json::from_slice(
                    &fetch_bytes(&format!(
                        "https://api.github.com/repos/{REPO}/git/trees/{sha}:{slice}?recursive=1"
                    ))
                    .await?,
                )
                .map_err(|e| e.to_string())?;
                let entries = tree["tree"].as_array().ok_or("the tree has no entries")?;
                for entry in entries {
                    if entry["type"] != "blob" || entry["size"].as_u64().unwrap_or(0) > 512 * 1024 {
                        continue;
                    }
                    let path = entry["path"].as_str().unwrap_or_default();
                    let blob = entry["sha"].as_str().unwrap_or_default().to_string();
                    wanted.push((format!("{slice}/{path}"), blob));
                }
            }
            let listed_ms = now_ms() - started;
            // Every file at once, from this browser's blob cache when it
            // has the blob, else from raw.githubusercontent.com.
            let fetched = futures_util::future::join_all(wanted.iter().map(|(full, blob)| {
                let url = format!("https://raw.githubusercontent.com/{REPO}/{sha}/{full}");
                async move { fetch_blob(&url, blob).await }
            }))
            .await;
            let mut vfs = Vfs::default();
            let mut bytes = 0;
            let mut cached = 0;
            for ((full, _), data) in wanted.iter().zip(fetched) {
                let (data, hit) = data?;
                bytes += data.len();
                cached += usize::from(hit);
                vfs.put(full, &data, 0).map_err(String::from)?;
            }
            if cached > 0 {
                self.log(
                    "info",
                    &format!(
                        "{cached} of {} files came from this browser's blob cache",
                        wanted.len()
                    ),
                );
            }
            Ok((sha, vfs, bytes, listed_ms))
        }
        .await;
        match result {
            Ok((sha, vfs, bytes, listed_ms)) => {
                let crates: Vec<String> = slices
                    .iter()
                    .filter_map(|slice| {
                        vfs.read(&format!("{slice}/Cargo.toml"))
                            .ok()
                            .and_then(|t| crate_name(&String::from_utf8_lossy(t)))
                    })
                    .collect();
                let (files, _) = vfs.usage();
                let mut base = vfs.clone();
                let meta = json!({"repo": REPO, "commit": sha, "slice": slices.first().cloned().unwrap_or_default(), "slices": slices, "crates": crates});
                let _ = base.put(REPO_META, meta.to_string().as_bytes(), 0);
                *self.vfs.borrow_mut() = vfs;
                *self.base.borrow_mut() = base;
                self.persist().await;
                self.persist_base().await;
                self.render_files();
                let ms = now_ms() - started;
                self.log(
                    "info",
                    &format!(
                        "Loaded {files} files ({} KiB) of {slice} at {} in {ms:.0} ms (commit and tree {listed_ms:.0} ms). Crate: {}.",
                        bytes / 1024,
                        &sha[..10],
                        crates.join(", ")
                    ),
                );
                self.metrics.borrow_mut().page["repo_load"] = json!({"slice": slice, "commit": sha, "files": files, "bytes": bytes, "ms": ms, "listing_ms": listed_ms});
                self.render_metrics();
                if crates.iter().any(|c| c == "kiln") {
                    let area: HtmlTextAreaElement = el(&self.doc, "task").unchecked_into();
                    area.set_value(KILN_TASK);
                } else if let Some(krate) = crates.first() {
                    let area: HtmlTextAreaElement = el(&self.doc, "task").unchecked_into();
                    if krate == "memory-stream" {
                        area.set_value(MEMORY_STREAM_TASK);
                    } else {
                        area.set_value(&format!(
                            "In {slice}, add a small public helper function with a doc comment and a unit test for it, then run `cargo test -p {krate}` until it passes."
                        ));
                    }
                }
            }
            Err(e) => self.log("error", &format!("Could not load {slice}: {e}")),
        }
        if let Some(window) = web_sys::window() {
            set(&window, "kilnLoaded", true);
        }
        self.busy.set(false);
    }

    /// Fetch the repository paths a reading tool names that the workspace
    /// lacks (#12488): a file, or a directory's files, at the loaded
    /// commit, into the base and the workspace, so the agent can read other
    /// crates. They count as unchanged.
    async fn read_sparse(&self, argv: &[String]) {
        let Some(repo) = self.repo() else {
            return;
        };
        let commit = repo["commit"].as_str().unwrap_or_default().to_string();
        for path in crate::remote::read_paths(argv) {
            let present = {
                let vfs = self.vfs.borrow();
                let prefix = format!("{path}/");
                vfs.get(&path).is_some() || vfs.iter().any(|(p, _)| p.starts_with(&prefix))
            };
            if present {
                continue;
            }
            let last = path.rsplit('/').next().unwrap_or_default();
            let raw = |p: &str| format!("https://raw.githubusercontent.com/{REPO}/{commit}/{p}");
            let mut files: Vec<(String, Vec<u8>)> = Vec::new();
            if last.contains('.') {
                if let Ok(data) = fetch_bytes(&raw(&path)).await {
                    files.push((path.clone(), data));
                }
            } else if let Ok(tree) = fetch_json(&format!(
                "https://api.github.com/repos/{REPO}/git/trees/{commit}:{path}?recursive=1"
            ))
            .await
            {
                let entries: Vec<(String, String)> = tree["tree"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|e| {
                                e["type"] == "blob" && e["size"].as_u64().unwrap_or(0) <= 512 * 1024
                            })
                            .take(300)
                            .map(|e| {
                                (
                                    format!("{path}/{}", e["path"].as_str().unwrap_or_default()),
                                    e["sha"].as_str().unwrap_or_default().to_string(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let fetched = futures_util::future::join_all(
                    entries
                        .iter()
                        .map(|(full, blob)| async move { fetch_blob(&raw(full), blob).await }),
                )
                .await;
                for ((full, _), data) in entries.into_iter().zip(fetched) {
                    if let Ok((data, _)) = data {
                        files.push((full, data));
                    }
                }
            }
            if files.is_empty() {
                continue;
            }
            {
                let mut vfs = self.vfs.borrow_mut();
                let mut base = self.base.borrow_mut();
                for (full, data) in &files {
                    let _ = vfs.put(full, data, 0);
                    let _ = base.put(full, data, 0);
                }
            }
            self.persist().await;
            self.persist_base().await;
            self.render_files();
            self.log(
                "info",
                &format!("Read {} files of {path} from the repository", files.len()),
            );
        }
    }

    /// Run `cargo ...` on the runner: send the files that differ from the
    /// base, wait for the result, and show progress as it comes.
    async fn remote_cargo(&self, argv: &[String]) -> ToolRun {
        let started = now_ms();
        let fail = |output: String| ToolRun { code: 127, output };
        let Some(repo) = self.repo() else {
            return fail("cargo runs only on a crate loaded from the repository".into());
        };
        let crates: Vec<String> = repo["crates"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let args = match crate::remote::parse_command(argv, &crates) {
            Ok(args) => args,
            Err(e) => return fail(e),
        };
        let (files, deleted) = {
            let vfs = self.vfs.borrow();
            let base = self.base.borrow();
            let slices = repo_slices(&repo);
            let inside = |p: &str| slices.iter().any(|s| p.starts_with(s.as_str()));
            let mut files = std::collections::BTreeMap::new();
            for (path, node) in vfs.iter() {
                if node.is_dir() || !inside(path) {
                    continue;
                }
                let current = vfs.read(path).unwrap_or_default();
                if base.read(path).ok() != Some(current) {
                    files.insert(path.clone(), String::from_utf8_lossy(current).into_owned());
                }
            }
            let deleted: Vec<String> = base
                .iter()
                .filter(|(p, n)| !n.is_dir() && inside(p) && vfs.get(p).is_none())
                .map(|(p, _)| p.clone())
                .collect();
            (files, deleted)
        };
        let job = crate::remote::Job {
            v: crate::remote::JOB_V.into(),
            base: repo["commit"].as_str().unwrap_or_default().to_string(),
            args,
            files,
            deleted,
            patch: None,
            action: None,
            diff: None,
        };
        let changed = job.files.len() + job.deleted.len();
        // Large files go as a diff: a relay frame holds about 128 KiB.
        let mut job = job;
        if job.files.values().map(String::len).sum::<usize>() > 16 * 1024 {
            let base = self.slice_files(&repo, true);
            let current = self.slice_files(&repo, false);
            job.diff = Some(crate::patch::format_patch(
                &crate::patch::Header {
                    author: "Kiln",
                    email: "kiln@localhost",
                    subject: "Kiln test run",
                    body: "",
                    date: super::util::unix(),
                },
                &crate::patch::changes(&base, &current),
            ));
            job.files.clear();
            job.deleted.clear();
        }
        if let Err(e) = job.check(&crates) {
            return fail(e);
        }
        let relay_url: HtmlInputElement = el(&self.doc, "relay").unchecked_into();
        let relay_url = relay_url.value();
        let mut relay = match self.runner_relay.borrow_mut().take() {
            Some(relay) if !relay.closed => relay,
            _ => match Relay::connect(&relay_url, &self.runner_key()).await {
                Ok(relay) => relay,
                Err(e) => return fail(e),
            },
        };
        let body = json!({
            "v": 1,
            "requires": [],
            "task": serde_json::to_string(&job).unwrap_or_default(),
            "transcript": [],
            "client": concat!("kiln ", env!("CARGO_PKG_VERSION")),
        });
        let line = self.doc.create_element("div").ok();
        if let Some(line) = &line {
            line.set_class_name("info");
            line.set_text_content(Some(&format!("sent {changed} changed files to the runner")));
            let _ = el(&self.doc, "log").append_child(line);
        }
        let progress = |note: Option<&str>| {
            if let (Some(line), Some(note)) = (&line, note) {
                line.set_text_content(Some(&format!(
                    "runner, {:.0} s: {note}",
                    (now_ms() - started) / 1000.0
                )));
                if let Some(id) = self.current_tool.borrow().clone() {
                    self.run_view.borrow_mut().progress(
                        &id,
                        &format!("{:.0} s: {note}", (now_ms() - started) / 1000.0),
                    );
                    self.push_transcript();
                }
            }
        };
        let named = crate::remote::named_crates(&job.args);
        let candidates = self.runner_candidates(&mut relay, &named).await;
        if candidates.is_empty() {
            *self.runner_relay.borrow_mut() = Some(relay);
            return fail(format!(
                "no test runner is online for {}; start kiln-runner, or enter a runner's key under Settings",
                named.join(", ")
            ));
        }
        // Like a Pylon job: the best runner first, the next when one
        // refuses or doesn't answer, and a receipt for each.
        let mut result = Err(String::new());
        for (runner, beacon) in candidates.iter().take(3) {
            if let Some(beacon) = beacon {
                self.log("info", &format!("Runner: {}", beacon.label));
            }
            let sent = super::util::unix();
            result = relay
                .exchange_tagged(
                    &self.runner_key(),
                    runner,
                    &body,
                    REMOTE_WAIT_SECS,
                    &progress,
                    &super::identity::tags(),
                )
                .await;
            if let Some(beacon) = beacon {
                self.receipt(&mut relay, beacon, sent, &result).await;
            }
            match &result {
                Ok(_) => break,
                Err(e) => {
                    self.failing_runners
                        .borrow_mut()
                        .insert(runner.clone(), now_ms());
                    self.log("err", &format!("runner {}: {e}", &runner[..8]));
                }
            }
        }
        *self.runner_relay.borrow_mut() = Some(relay);
        let got = match result {
            Ok(got) => got,
            Err(e) => return fail(format!("the runner did not answer: {e}")),
        };
        let Ok(outcome) = serde_json::from_str::<crate::remote::Outcome>(&got.text) else {
            return fail("the runner's answer is not a result".into());
        };
        let wall = now_ms() - started;
        if outcome.code == Some(0) && job.args[0] == "test" {
            *self.green.borrow_mut() = Some(self.slice_digest(&repo));
        }
        if job.args[0] == "test" {
            *self.last_test.borrow_mut() = Some((outcome.code == Some(0), outcome.summary.clone()));
        }
        self.metrics.borrow_mut().runs.push(json!({
            "tool": format!("cargo {}", argv.get(1).map_or("", String::as_str)),
            "wall_ms": wall,
            "runner_ms": outcome.ms,
            "contact_ms": got.contact_ms,
            "changed_files": changed,
            "summary": outcome.summary,
            "remote": true,
        }));
        self.render_metrics();
        ToolRun {
            code: outcome.code.unwrap_or(124),
            output: format!(
                "{} ({:.1} s on the runner)\n{}",
                outcome.summary,
                outcome.ms as f64 / 1000.0,
                outcome.log
            ),
        }
    }
}

impl App {
    /// The loaded crates' files, by path: the workspace's, or with `base`
    /// the untouched copy's.
    fn slice_files(&self, repo: &Value, base: bool) -> std::collections::BTreeMap<String, String> {
        let slices = repo_slices(repo);
        let vfs = if base {
            self.base.borrow()
        } else {
            self.vfs.borrow()
        };
        vfs.iter()
            .filter(|(p, n)| !n.is_dir() && slices.iter().any(|s| p.starts_with(s)))
            .map(|(p, _)| {
                (
                    p.clone(),
                    String::from_utf8_lossy(vfs.read(p).unwrap_or_default()).into_owned(),
                )
            })
            .collect()
    }

    /// Where a job for `crates` may go: the runner named under Settings,
    /// or the runners whose beacons offer those crates (#12457), best
    /// first, read again after a minute.
    async fn runner_candidates(
        &self,
        relay: &mut Relay,
        crates: &[String],
    ) -> Vec<(String, Option<cj::Runner>)> {
        let runner: HtmlInputElement = el(&self.doc, "runner").unchecked_into();
        if let Some(runner) = nostr_hex(runner.value().trim()) {
            return vec![(runner, None)];
        }
        let stale = self
            .runners
            .borrow()
            .as_ref()
            .is_none_or(|(at, _)| now_ms() - at > 60_000.0);
        if stale {
            match relay.query("runners", &cj::runner_filter()).await {
                Ok(events) => {
                    let repository = self.repo_address.borrow().clone();
                    let found = cj::runners(
                        &events,
                        super::util::unix(),
                        &[],
                        (!repository.is_empty()).then_some(repository.as_str()),
                        &BTreeSet::new(),
                    );
                    self.log(
                        "info",
                        &format!("Found {} test runners by their beacons", found.len()),
                    );
                    *self.runners.borrow_mut() = Some((now_ms(), found));
                }
                Err(e) => self.log("err", &format!("Reading runner beacons failed: {e}")),
            }
        }
        // A runner that failed is skipped for five minutes.
        let now = now_ms();
        self.failing_runners
            .borrow_mut()
            .retain(|_, at| now - *at < 300_000.0);
        let failing = self.failing_runners.borrow();
        self.runners
            .borrow()
            .as_ref()
            .map(|(_, list)| {
                list.iter()
                    .filter(|r| !failing.contains_key(&r.provider))
                    .filter(|r| crates.iter().all(|c| r.crates.contains(c)))
                    .map(|r| (r.provider.clone(), Some(r.clone())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Publish the buyer's receipt for one runner job, as for any Pylon.
    async fn receipt(
        &self,
        relay: &mut Relay,
        runner: &cj::Runner,
        started: u64,
        result: &Result<super::relay::Exchanged, String>,
    ) {
        let Some((request, digest)) = relay.last_request.clone() else {
            return;
        };
        let (outcome, result_digest) = match result {
            Ok(got) => (
                nostr::pylon::Outcome::Accepted,
                Some(crate::sha256_hex(got.text.as_bytes())),
            ),
            Err(e) if e.starts_with("no answer") => (nostr::pylon::Outcome::Timeout, None),
            Err(_) => (nostr::pylon::Outcome::Rejected, None),
        };
        let key = self.runner_key();
        let receipt = cj::runner_receipt(
            key.pubkey(),
            runner,
            &request,
            &digest,
            result_digest,
            started,
            super::util::unix(),
            outcome,
        );
        if let Ok(event) = nostr::pylon::receipt_event(key.signer(), &receipt, super::util::unix())
        {
            let _ = relay.publish(&event).await;
        }
    }

    fn slice_digest(&self, repo: &Value) -> String {
        let files = self.slice_files(repo, false);
        crate::sha256_hex(serde_json::to_string(&files).unwrap_or_default().as_bytes())
    }

    /// Publish the crate's change as a NIP-34 patch (`1617`) signed by the
    /// page's key, then have the runner apply that exact patch, test it, and
    /// publish its check comment.
    async fn publish(&self, subject: &str, body: &str) -> ToolRun {
        if self.verse.borrow().is_some() {
            return ToolRun {
                code: 1,
                output: "Use the project controls to publish after review.".into(),
            };
        }
        let fail = |output: String| ToolRun { code: 1, output };
        let Some(repo) = self.repo() else {
            return fail("publish needs a crate loaded from the repository".into());
        };
        let address = self.repo_address.borrow().clone();
        let Some(owner) = address.split(':').nth(1).map(str::to_string) else {
            return fail("no repository to publish to; kiln-dev names it with --repo".into());
        };
        if self.green.borrow().as_deref() != Some(self.slice_digest(&repo).as_str()) {
            return fail(
                "run `cargo test` first: publish only a change whose tests just passed".into(),
            );
        }
        let commit = repo["commit"].as_str().unwrap_or_default().to_string();
        let base = self.slice_files(&repo, true);
        let current = self.slice_files(&repo, false);
        let changes = crate::patch::changes(&base, &current);
        if changes.is_empty() {
            return fail("nothing changed, so there's nothing to publish".into());
        }
        // Signed with this browser's lasting key, with the identity its
        // owner gave it when there is one (#12451).
        let signer = self.runner_key();
        let email = format!("{}@nostr", signer.npub());
        let text = crate::patch::format_patch(
            &crate::patch::Header {
                author: "Kiln agent",
                email: &email,
                subject,
                body,
                date: super::util::unix(),
            },
            &changes,
        );
        let found = crate::patch::secret_findings(&text);
        if !found.is_empty() {
            return fail(format!(
                "not published: the patch seems to hold {}",
                found.join(", ")
            ));
        }
        if text.len() > crate::patch::MAX_PATCH_BYTES {
            return fail("the patch is over 60 KB; NIP-34 wants a pull request for that".into());
        }
        // Every loaded crate the change touches is named and tested (#12450).
        let paths: Vec<String> = changes.iter().map(|c| c.path.to_string()).collect();
        let touched = crate::remote::touched_crates(&paths);
        let mut tags = crate::patch::patch_tags(&address, &owner, &commit, &touched);
        tags.extend(super::identity::tags());
        let event = signer.sign(crate::patch::PATCH_KIND, tags, text, super::util::unix());
        let relay_url: HtmlInputElement = el(&self.doc, "relay").unchecked_into();
        let relay_url = relay_url.value();
        let mut relay = match self.runner_relay.borrow_mut().take() {
            Some(relay) if !relay.closed => relay,
            _ => match Relay::connect(&relay_url, &self.runner_key()).await {
                Ok(relay) => relay,
                Err(e) => return fail(e),
            },
        };
        if let Err(e) = relay.publish(&event).await {
            *self.runner_relay.borrow_mut() = Some(relay);
            return fail(format!("not published: {e}"));
        }
        self.log(
            "info",
            &format!(
                "Published patch {} to {relay_url}; the runner is checking it.",
                &event.id[..12]
            ),
        );
        // The runner that checks the patch: the one under Settings, or the
        // best one whose beacon offers these crates (#12457).
        let runner = self
            .runner_candidates(&mut relay, &touched)
            .await
            .into_iter()
            .next()
            .map(|(runner, _)| runner)
            .unwrap_or_default();
        let job = crate::remote::Job {
            v: crate::remote::JOB_V.into(),
            base: commit,
            args: std::iter::once("test".to_string())
                .chain(touched.iter().flat_map(|c| ["-p".to_string(), c.clone()]))
                .collect(),
            files: std::collections::BTreeMap::new(),
            deleted: Vec::new(),
            patch: Some(event.clone()),
            action: None,
            diff: None,
        };
        let body = json!({
            "v": 1,
            "requires": [],
            "task": serde_json::to_string(&job).unwrap_or_default(),
            "transcript": [],
            "client": concat!("kiln ", env!("CARGO_PKG_VERSION")),
        });
        let line = self.doc.create_element("div").ok();
        if let Some(line) = &line {
            line.set_class_name("info");
            let _ = el(&self.doc, "log").append_child(line);
        }
        let started = now_ms();
        let progress = |note: Option<&str>| {
            if let (Some(line), Some(note)) = (&line, note) {
                line.set_text_content(Some(&format!(
                    "runner check, {:.0} s: {note}",
                    (now_ms() - started) / 1000.0
                )));
            }
        };
        let checked = relay
            .exchange_tagged(
                &self.runner_key(),
                &runner,
                &body,
                REMOTE_WAIT_SECS,
                &progress,
                &super::identity::tags(),
            )
            .await;
        *self.runner_relay.borrow_mut() = Some(relay);
        let check = checked
            .ok()
            .and_then(|got| serde_json::from_str::<crate::remote::Outcome>(&got.text).ok());
        let review = asset(&format!("review.html#{}", event.id));
        if let Some(window) = web_sys::window() {
            set(&window, "kilnPatch", event.id.clone());
        }
        *self.patch.borrow_mut() = Some(kiln_view::Patch {
            id: event.id.clone(),
            review: review.clone(),
            check: check.as_ref().map(|c| c.summary.clone()),
        });
        if let Some(log) = self.doc.get_element_by_id("log")
            && let Ok(link) = self.doc.create_element("a")
        {
            let _ = link.set_attribute("href", &review);
            let _ = link.set_attribute("target", "_blank");
            link.set_text_content(Some(&format!("Review patch {}", &event.id[..12])));
            let _ = log.append_child(&link);
        }
        ToolRun {
            code: 0,
            output: format!(
                "published patch {} ({} changed files); the runner's check: {}",
                &event.id[..12],
                changes.len(),
                check.map_or_else(|| "no answer yet".to_string(), |c| c.summary)
            ),
        }
    }
}

const MEMORY_STREAM_TASK: &str = "In crates/memory-stream/src/lib.rs, add a public function `half_life_hours() -> f64` with a doc comment: the number of hours after which `recency` falls to 0.5 under RECENCY_DECAY. Add a unit test in the existing tests module that checks `recency(half_life_hours())` is within 1e-9 of 0.5 and that the half-life is between 68 and 70 hours. Run `cargo test -p memory-stream` until it passes, then publish the change.";

/// The loaded directories, each with a trailing `/`.
fn repo_slices(repo: &Value) -> Vec<String> {
    let listed: Vec<String> = repo["slices"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(|s| format!("{s}/")))
                .collect()
        })
        .unwrap_or_default();
    if listed.is_empty() {
        vec![format!("{}/", repo["slice"].as_str().unwrap_or_default())]
    } else {
        listed
    }
}

impl App {
    /// Console and automation hooks: `kilnRun(line)` runs a tool command,
    /// `kilnWrite(path, text)` writes a file through the `write` tool, and
    /// `kilnSelfEdit(action)` asks the runner to `publish`, `revert`, or
    /// `reset`. Each returns a promise.
    fn expose(self: &Rc<Self>) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let app = self.clone();
        let run = Closure::<dyn Fn(String) -> js_sys::Promise>::new(move |line: String| {
            let app = app.clone();
            wasm_bindgen_futures::future_to_promise(async move {
                let run = app.run_command(&Command::Run(line)).await;
                Ok(JsValue::from(run.map_or(-1, |r| r.code)))
            })
        });
        set(&window, "kilnRun", run.as_ref().clone());
        run.forget();
        let app = self.clone();
        let write = Closure::<dyn Fn(String, String) -> js_sys::Promise>::new(
            move |path: String, content: String| {
                let app = app.clone();
                wasm_bindgen_futures::future_to_promise(async move {
                    let run = app.run_command(&Command::Write { path, content }).await;
                    Ok(JsValue::from(run.map_or(-1, |r| r.code)))
                })
            },
        );
        set(&window, "kilnWrite", write.as_ref().clone());
        write.forget();
        // This mode replaces the VFS with three creator-owned records. It grants no mutation authority.
        let app = self.clone();
        let context = Closure::<dyn Fn(String) -> Result<(), JsValue>>::new(move |raw: String| {
            if app.busy.get() {
                return Err(JsValue::from_str("Stop Kiln before changing its context."));
            }
            if raw.len() > 24_000 {
                return Err(JsValue::from_str("Choose a smaller editing context."));
            }
            let context = serde_json::from_str(&raw)
                .map_err(|_| JsValue::from_str("Choose a valid project context."))?;
            let workspace =
                crate::verse::Workspace::new(context).map_err(|e| JsValue::from_str(&e))?;
            *app.vfs.borrow_mut() = workspace.vfs.clone();
            *app.base.borrow_mut() = Vfs::default();
            *app.verse.borrow_mut() = Some(workspace);
            *app.opfs.borrow_mut() = None;
            *app.opfs_base.borrow_mut() = None;
            app.shader_preview.set(false);
            app.render_files();
            Ok(())
        });
        set(&window, "kilnVerseContext", context.as_ref().clone());
        context.forget();
        let app = self.clone();
        let candidate = Closure::<dyn Fn() -> Result<String, JsValue>>::new(move || {
            let held = app.verse.borrow();
            let workspace = held
                .as_ref()
                .ok_or_else(|| JsValue::from_str("Open a project context first."))?;
            let action = workspace.candidate().map_err(|e| JsValue::from_str(&e))?;
            Ok(json!({"project":workspace.context.project,"selection":workspace.context.selection,"action":action}).to_string())
        });
        set(&window, "kilnVerseCandidate", candidate.as_ref().clone());
        candidate.forget();
        // Only this opt-in API exposes the fixed shader file to a preview.
        let app = self.clone();
        let save = Closure::<dyn Fn(String) -> js_sys::Promise>::new(move |source: String| {
            let app = app.clone();
            wasm_bindgen_futures::future_to_promise(async move {
                crate::shader::validate(&source).map_err(JsValue::from_str)?;
                if app.busy.get() || app.repo().is_some() {
                    return Err(JsValue::from_str("Kiln is busy or has a repository open."));
                }
                app.shader_preview.set(true);
                app.busy.set(true);
                let result = app
                    .run_command(&Command::Write {
                        path: shader_path().to_owned(),
                        content: source,
                    })
                    .await;
                app.busy.set(false);
                match result {
                    Some(run) if run.code == 0 => Ok(JsValue::TRUE),
                    Some(run) => Err(JsValue::from_str(&run.output)),
                    None => Err(JsValue::from_str("Kiln could not save the shader.")),
                }
            })
        });
        set(&window, "kilnSaveShader", save.as_ref().clone());
        save.forget();
        let app = self.clone();
        let read = Closure::<dyn Fn() -> JsValue>::new(move || {
            if app.repo().is_some() {
                return JsValue::NULL;
            }
            app.shader_preview.set(true);
            app.shader_source()
                .map_or(JsValue::NULL, |s| JsValue::from_str(&s))
        });
        set(&window, "kilnReadShader", read.as_ref().clone());
        read.forget();
        let app = self.clone();
        let scene = Closure::<dyn Fn() -> JsValue>::new(move || {
            if app.repo().is_some() {
                return JsValue::NULL;
            }
            app.workspace_source(scene_path())
                .map_or(JsValue::NULL, |s| JsValue::from_str(&s))
        });
        set(&window, "kilnReadScene", scene.as_ref().clone());
        scene.forget();
        let app = self.clone();
        let changes = Closure::<dyn Fn() -> JsValue>::new(move || {
            if app.repo().is_some() {
                return JsValue::NULL;
            }
            let path = scene_path().replace("scene.json", "changes.json");
            app.workspace_source(&path)
                .map_or(JsValue::NULL, |s| JsValue::from_str(&s))
        });
        set(&window, "kilnReadChanges", changes.as_ref().clone());
        changes.forget();
        let app = self.clone();
        let edit = Closure::<dyn Fn(String) -> js_sys::Promise>::new(move |action: String| {
            let app = app.clone();
            wasm_bindgen_futures::future_to_promise(async move {
                let action: &'static str = match action.as_str() {
                    "publish" => "publish",
                    "revert" => "revert",
                    _ => "reset",
                };
                app.self_edit(action).await;
                Ok(JsValue::TRUE)
            })
        });
        set(&window, "kilnSelfEdit", edit.as_ref().clone());
        edit.forget();
        self.expose_cards(&window);
    }

    fn shader_source(&self) -> Option<String> {
        if !self.shader_preview.get() {
            return None;
        }
        self.workspace_source(shader_path())
    }

    fn workspace_source(&self, path: &str) -> Option<String> {
        PREVIEW_LAB.with(|lab| crate::shader::workspace_source(&self.vfs.borrow(), path, lab.get()))
    }

    fn notify_shader(&self) {
        let Some(source) = self.shader_source() else {
            return;
        };
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(callback) = js_sys::Reflect::get(&window, &"kilnShaderChanged".into())
            && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
        {
            let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(&source));
        }
    }

    fn notify_scene(&self) {
        if self.repo().is_some() {
            return;
        }
        let Some(source) = self.workspace_source(scene_path()) else {
            return;
        };
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(callback) = js_sys::Reflect::get(&window, &"kilnSceneChanged".into())
            && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
        {
            let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(&source));
        }
    }

    /// What the Grid terminal's cards draw (`kiln_view::State`).
    fn state(&self) -> kiln_view::State {
        let repo = self.repo();
        let base = self.base.borrow();
        let vfs = self.vfs.borrow();
        let mut files = Vec::new();
        let mut file_count = 0;
        for (path, node) in vfs.iter() {
            if path.is_empty() || node.is_dir() || path.starts_with(".kiln/") {
                continue;
            }
            file_count += 1;
            if files.len() < kiln_view::FILES_SHOWN {
                let changed = repo.is_some() && base.read(path).ok() != vfs.read(path).ok();
                files.push(kiln_view::File {
                    path: path.clone(),
                    bytes: usize::try_from(node.size()).unwrap_or(usize::MAX),
                    changed,
                });
            }
        }
        drop((base, vfs));
        let (changed, green) = match &repo {
            Some(repo) => {
                let changed = self.slice_files(repo, true) != self.slice_files(repo, false);
                let green =
                    self.green.borrow().as_deref() == Some(self.slice_digest(repo).as_str());
                (changed, green)
            }
            None => (false, false),
        };
        let runner: HtmlInputElement = el(&self.doc, "runner").unchecked_into();
        kiln_view::State {
            ready: true,
            busy: self.busy.get(),
            working: self.run_view.borrow().activity().map(str::to_owned),
            repo: repo.map(|r| kiln_view::Repo {
                slice: repo_slices(&r)
                    .iter()
                    .map(|s| s.trim_end_matches('/'))
                    .collect::<Vec<_>>()
                    .join(","),
                reference: String::new(),
                commit: r["commit"].as_str().unwrap_or_default().to_owned(),
            }),
            files,
            file_count,
            // A runner set under Settings, or found by its beacon (or not
            // looked for yet).
            runner: !runner.value().trim().is_empty()
                || self
                    .runners
                    .borrow()
                    .as_ref()
                    .is_none_or(|(_, list)| !list.is_empty()),
            tests: self
                .last_test
                .borrow()
                .clone()
                .map(|(passed, summary)| kiln_view::Tests { passed, summary }),
            changed,
            green,
            patch: self.patch.borrow().clone(),
            problem: self.problem.borrow().clone(),
        }
    }

    /// The Grid terminal's hooks (#12456): `kilnState()` returns the
    /// cards' state as JSON; `kilnAgent(task)`, `kilnStop()`,
    /// `kilnLoad(directory)`, and `kilnPublish()` do what the cards'
    /// buttons say.
    fn expose_cards(self: &Rc<Self>, window: &web_sys::Window) {
        let app = self.clone();
        let state = Closure::<dyn Fn() -> String>::new(move || {
            serde_json::to_string(&app.state()).unwrap_or_default()
        });
        set(window, "kilnState", state.as_ref().clone());
        state.forget();
        let app = self.clone();
        let agent = Closure::<dyn Fn(String)>::new(move |task: String| {
            if task.trim().is_empty() || app.busy.get() {
                return;
            }
            let area: HtmlTextAreaElement = el(&app.doc, "task").unchecked_into();
            area.set_value(&task);
            *app.patch.borrow_mut() = None;
            *app.problem.borrow_mut() = None;
            wasm_bindgen_futures::spawn_local(app.clone().agent(task));
        });
        set(window, "kilnAgent", agent.as_ref().clone());
        agent.forget();
        let app = self.clone();
        let stop = Closure::<dyn Fn()>::new(move || app.stop.set(true));
        set(window, "kilnStop", stop.as_ref().clone());
        stop.forget();
        let app = self.clone();
        let load = Closure::<dyn Fn(String)>::new(move |slice: String| {
            if slice.trim().is_empty() || app.busy.get() || app.verse.borrow().is_some() {
                return;
            }
            *app.patch.borrow_mut() = None;
            *app.problem.borrow_mut() = None;
            *app.last_test.borrow_mut() = None;
            let reference: HtmlInputElement = el(&app.doc, "ref").unchecked_into();
            wasm_bindgen_futures::spawn_local(app.clone().load_repo(slice, reference.value()));
        });
        set(window, "kilnLoad", load.as_ref().clone());
        load.forget();
        let app = self.clone();
        let publish = Closure::<dyn Fn()>::new(move || {
            if app.busy.get() {
                return;
            }
            let area: HtmlTextAreaElement = el(&app.doc, "task").unchecked_into();
            let task = area.value();
            let subject: String = task
                .lines()
                .next()
                .unwrap_or("Kiln change")
                .chars()
                .take(72)
                .collect();
            let app = app.clone();
            wasm_bindgen_futures::spawn_local(async move {
                app.busy.set(true);
                let run = app.publish(&subject, "").await;
                app.log(if run.code == 0 { "done" } else { "err" }, &run.output);
                app.busy.set(false);
            });
        });
        set(window, "kilnPublish", publish.as_ref().clone());
        publish.forget();
    }

    /// The key runner jobs are signed with: this browser's lasting admin
    /// key, which an operator lists on the runner (#12449), or the page's
    /// key before it loads.
    fn runner_key(&self) -> Key {
        self.admin_key
            .borrow()
            .clone()
            .unwrap_or_else(|| self.key.clone())
    }

    /// The admin key, from this browser's storage, made on first use.
    fn load_admin_key(&self) {
        let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
        let stored = storage
            .as_ref()
            .and_then(|s| s.get_item("kiln-admin-secret").ok().flatten())
            .filter(|h| h.len() == 64);
        let bytes = match stored {
            Some(hex) => {
                let mut b = [0u8; 32];
                for (i, x) in b.iter_mut().enumerate() {
                    *x = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap_or(0);
                }
                b
            }
            None => {
                let b = random32();
                let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
                if let Some(s) = &storage {
                    let _ = s.set_item("kiln-admin-secret", &hex);
                }
                b
            }
        };
        if let Ok(key) = Key::from_bytes(bytes) {
            if let Some(node) = self.doc.get_element_by_id("admin-key") {
                node.set_text_content(Some(&key.npub()));
            }
            super::identity::wire(&self.doc, key.pubkey());
            *self.admin_key.borrow_mut() = Some(key);
        }
    }

    /// Ask the site for a credential for this browser's key, when it holds
    /// no valid one: `POST identity` beside Kiln's files, which answers a
    /// signed-in visitor (#12488).
    async fn site_identity(self: Rc<Self>) {
        let Some(key) = self.admin_key.borrow().clone() else {
            return;
        };
        if super::identity::holds(key.pubkey()) {
            return;
        }
        let body = json!({"key": key.pubkey()}).to_string();
        match post_json(&asset("identity"), &body).await {
            Ok(text) => {
                super::identity::adopt(&self.doc, key.pubkey(), &text);
                if super::identity::holds(key.pubkey()) {
                    self.log("info", "The site vouched for this browser's key.");
                }
            }
            Err(e) => self.log("info", &format!("No identity from the site: {e}")),
        }
    }

    /// Ask the runner to publish Kiln from this workspace, or to revert or
    /// reset it. Only the runner's admin keys get an answer other than a
    /// refusal.
    async fn self_edit(self: Rc<Self>, action: &str) {
        if self.verse.borrow().is_some() {
            return;
        }
        self.busy.set(true);
        let started = now_ms();
        let result = self.clone().self_edit_job(action).await;
        match result {
            Ok(outcome) => {
                let ok = outcome.code == Some(0);
                self.log(if ok { "done" } else { "err" }, &outcome.summary);
                if !ok && !outcome.log.is_empty() {
                    self.log("err", &crate::agent::clip(&outcome.log, 3000));
                }
                if let Some(published) = &outcome.published {
                    self.log(
                        "info",
                        &format!(
                            "Runner phases (ms): {}; {:.1} s from the click.",
                            published["phases_ms"],
                            (now_ms() - started) / 1000.0
                        ),
                    );
                    self.metrics.borrow_mut().page["self_edit"] = published.clone();
                    self.render_metrics();
                }
            }
            Err(e) => self.log("error", &e),
        }
        self.busy.set(false);
        // A build that arrived while this ran swaps in now.
        if let Some(build) = self.pending_build.borrow().clone() {
            self.swap(&build).await;
        }
    }

    async fn self_edit_job(self: Rc<Self>, action: &str) -> Result<crate::remote::Outcome, String> {
        let key = self
            .admin_key
            .borrow()
            .clone()
            .ok_or("this browser has no admin key")?;
        let runner: HtmlInputElement = el(&self.doc, "runner").unchecked_into();
        let runner = nostr_hex(runner.value().trim()).ok_or("no test runner is set")?;
        let (base, files) = if action == "publish" {
            let repo = self
                .repo()
                .filter(|r| {
                    r["crates"]
                        .as_array()
                        .is_some_and(|c| c.iter().any(|c| c == "kiln"))
                })
                .ok_or("load Kiln's source first")?;
            let base = self.slice_files(&repo, true);
            let current = self.slice_files(&repo, false);
            let files: std::collections::BTreeMap<String, String> = current
                .into_iter()
                .filter(|(p, c)| base.get(p) != Some(c))
                .collect();
            (
                repo["commit"].as_str().unwrap_or_default().to_string(),
                files,
            )
        } else {
            ("0".repeat(40), std::collections::BTreeMap::new())
        };
        let job = crate::remote::Job {
            v: crate::remote::JOB_V.into(),
            base,
            args: Vec::new(),
            files,
            deleted: Vec::new(),
            patch: None,
            action: Some(action.into()),
            diff: None,
        };
        // Send a diff, not whole files: a relay frame holds about 128 KiB.
        let mut job = job;
        if !job.files.is_empty() {
            let repo = self.repo().unwrap_or_default();
            let base = self.slice_files(&repo, true);
            let current = self.slice_files(&repo, false);
            let changes = crate::patch::changes(&base, &current);
            job.diff = Some(crate::patch::format_patch(
                &crate::patch::Header {
                    author: "Kiln admin",
                    email: "kiln@localhost",
                    subject: "Kiln self-edit",
                    body: "",
                    date: super::util::unix(),
                },
                &changes,
            ));
            job.files.clear();
        }
        job.check(&[])?;
        self.log("cmd", &format!("$ kiln {action}"));
        if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
            let _ = storage.set_item("kiln-edit-started", &js_sys::Date::now().to_string());
        }
        let relay_url: HtmlInputElement = el(&self.doc, "relay").unchecked_into();
        let mut relay = Relay::connect(&relay_url.value(), &key).await?;
        let body = json!({
            "v": 1,
            "requires": [],
            "task": serde_json::to_string(&job).unwrap_or_default(),
            "transcript": [],
            "client": concat!("kiln ", env!("CARGO_PKG_VERSION")),
        });
        let line = self.doc.create_element("div").ok();
        if let Some(line) = &line {
            line.set_class_name("info");
            let _ = el(&self.doc, "log").append_child(line);
        }
        let started = now_ms();
        let progress = |note: Option<&str>| {
            if let (Some(line), Some(note)) = (&line, note) {
                line.set_text_content(Some(&format!(
                    "runner, {:.0} s: {note}",
                    (now_ms() - started) / 1000.0
                )));
            }
        };
        let got = relay
            .exchange(&key, &runner, &body, REMOTE_WAIT_SECS, &progress)
            .await?;
        serde_json::from_str::<crate::remote::Outcome>(&got.text)
            .map_err(|_| "the runner's answer is not a result".to_string())
    }

    /// Watch for a new Kiln build: poll `tools/manifest.json` every three
    /// seconds while the tab is visible.
    async fn watch_builds(self: Rc<Self>) {
        loop {
            sleep(3_000).await;
            let visible = web_sys::window()
                .and_then(|w| w.document())
                .is_none_or(|d| d.visibility_state() == web_sys::VisibilityState::Visible);
            if !visible {
                continue;
            }
            let Some(build) = current_build().await else {
                continue;
            };
            if build == *self.build.borrow()
                || self.pending_build.borrow().as_deref() == Some(build.as_str())
            {
                continue;
            }
            *self.pending_build.borrow_mut() = Some(build.clone());
            if self.busy.get() {
                self.log(
                    "info",
                    &format!("Kiln build {build} is ready; it loads when the agent is idle."),
                );
                if let Some(banner) = self.doc.get_element_by_id("update") {
                    banner.set_text_content(Some(&format!(
                        "Kiln build {build} is ready; it loads when the agent is idle."
                    )));
                    let _ = banner.class_list().remove_1("hidden");
                }
                if let Some(button) = self.doc.get_element_by_id("reload-now") {
                    let _ = button.class_list().remove_1("hidden");
                }
                continue;
            }
            self.swap(&build).await;
        }
    }

    /// Smoke-test the new build in a fresh worker, save the log and task,
    /// and reload into it. The workspace stays in OPFS.
    async fn swap(&self, build: &str) {
        let started = now_ms();
        self.log(
            "info",
            &format!("Kiln build {build} is out; testing it in a worker."),
        );
        // A fresh worker loads the new page module; run a tool through it.
        let fresh = self.worker.borrow_mut().take();
        if let Some(old) = fresh {
            old.terminate();
        }
        let run = self.run_tool(&["ls".to_string()], &[]).await;
        if run.code != 0 {
            self.log(
                "error",
                &format!(
                    "Build {build} failed its smoke test, so this page stays on {}: {}",
                    self.build.borrow(),
                    run.output
                ),
            );
            return;
        }
        let smoke_ms = now_ms() - started;
        self.save_session(build, smoke_ms);
        if let Some(window) = web_sys::window() {
            let _ = window.location().reload();
        }
    }

    fn save_session(&self, build: &str, smoke_ms: f64) {
        let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten())
        else {
            return;
        };
        let lines: Vec<Value> = children(&el(&self.doc, "log"))
            .into_iter()
            .rev()
            .take(400)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|e| json!([e.class_name(), e.text_content().unwrap_or_default()]))
            .collect();
        let task: HtmlTextAreaElement = el(&self.doc, "task").unchecked_into();
        let saved = json!({
            "log": lines,
            "task": task.value(),
            "from": *self.build.borrow(),
            "to": build,
            "smoke_ms": smoke_ms,
            "reload_at": js_sys::Date::now(),
        });
        let _ = storage.set_item("kiln-restore", &saved.to_string());
    }

    /// Put back what [`App::save_session`] kept, after a reload.
    fn restore_session(&self) {
        let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten())
        else {
            return;
        };
        let Some(saved) = storage
            .get_item("kiln-restore")
            .ok()
            .flatten()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        else {
            return;
        };
        let _ = storage.remove_item("kiln-restore");
        let log = el(&self.doc, "log");
        let fresh: Vec<Element> = children(&log);
        log.set_inner_html("");
        for line in saved["log"].as_array().into_iter().flatten() {
            self.log(
                line[0].as_str().unwrap_or("info"),
                line[1].as_str().unwrap_or_default(),
            );
        }
        for node in fresh {
            let _ = log.append_child(&node);
        }
        if let Some(task) = saved["task"].as_str() {
            let area: HtmlTextAreaElement = el(&self.doc, "task").unchecked_into();
            area.set_value(task);
        }
        let now = js_sys::Date::now();
        let reload_ms = now - saved["reload_at"].as_f64().unwrap_or(now);
        let edit_ms = storage
            .get_item("kiln-edit-started")
            .ok()
            .flatten()
            .and_then(|t| t.parse::<f64>().ok())
            .map(|t| now - t);
        let _ = storage.remove_item("kiln-edit-started");
        self.log(
            "done",
            &format!(
                "Reloaded from Kiln build {} into {} in {:.1} s (smoke {:.0} ms){}; the workspace and this log came with it.",
                saved["from"].as_str().unwrap_or("?"),
                saved["to"].as_str().unwrap_or("?"),
                reload_ms / 1000.0,
                saved["smoke_ms"].as_f64().unwrap_or(0.0),
                edit_ms.map_or(String::new(), |ms| format!(", {:.1} s from the click", ms / 1000.0))
            ),
        );
        let mut m = self.metrics.borrow_mut();
        m.page["hot_reload"] = json!({
            "from": saved["from"], "to": saved["to"], "reload_ms": reload_ms,
            "smoke_ms": saved["smoke_ms"], "click_to_reloaded_ms": edit_ms,
        });
        drop(m);
        if let Some(window) = web_sys::window() {
            set(&window, "kilnReloaded", edit_ms.unwrap_or(reload_ms));
        }
    }
}

/// An element's child elements.
fn children(parent: &Element) -> Vec<Element> {
    let list = parent.children();
    (0..list.length()).filter_map(|i| list.item(i)).collect()
}

/// The running build's name from `tools/manifest.json`, `site` when the
/// host serves its own build.
async fn current_build() -> Option<String> {
    let manifest = fetch_json_fresh("tools/manifest.json").await.ok()?;
    Some(
        manifest["kiln"]["build"]
            .as_str()
            .map_or_else(|| "site".to_string(), str::to_string),
    )
}

/// [`fetch_json`] past every cache.
async fn fetch_json_fresh(url: &str) -> Result<Value, String> {
    let window = web_sys::window().ok_or("no window")?;
    let init = web_sys::RequestInit::new();
    init.set_cache(web_sys::RequestCache::NoStore);
    let url = &asset(url);
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .map_err(|e| text(&e))?
        .dyn_into()
        .map_err(|e| text(&e))?;
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    let body = JsFuture::from(response.text().map_err(|e| text(&e))?)
        .await
        .map_err(|e| text(&e))?;
    serde_json::from_str(&body.as_string().unwrap_or_default()).map_err(|e| e.to_string())
}

impl App {
    /// Hand the run to the transcript module, when it's loaded; it also
    /// reads `kilnTranscriptLatest` when it starts.
    fn push_transcript(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let json = self.run_view.borrow().to_json();
        set(&window, "kilnTranscriptLatest", json.clone());
        if let Ok(show) = js_sys::Reflect::get(&window, &"kilnTranscriptShow".into())
            && let Ok(show) = show.dyn_into::<js_sys::Function>()
        {
            let _ = show.call1(&JsValue::NULL, &JsValue::from_str(&json));
        }
    }
}

const KILN_TASK: &str = "This workspace is Kiln's own source. In crates/kiln/src/agent.rs, add `pub fn tool_count() -> usize` with a doc comment that returns how many tools TOOLS lists, and a unit test in the existing tests module that checks it equals TOOLS.len() and is at least 7. Run `cargo test -p kiln -p kiln-tools` until it passes.";

/// The package name in a `Cargo.toml`.
fn crate_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package
            && let Some(rest) = line.strip_prefix("name")
            && let Some(value) = rest.trim().strip_prefix('=')
        {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// A runner key as 64 hex characters, from hex or `npub`.
fn nostr_hex(text: &str) -> Option<String> {
    if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Some(text.to_lowercase());
    }
    let bytes = nostr::nip19::decode_npub(text).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Fetch and parse a JSON document.
/// A file at `url` whose git blob ID is `blob`: from this browser's blob
/// cache when it holds it, else fetched, checked against `blob`, and kept
/// (#12488). Returns whether it came from the cache.
async fn fetch_blob(url: &str, blob: &str) -> Result<(Vec<u8>, bool), String> {
    let key = format!("https://kiln.invalid/blob/{blob}");
    let cache = match web_sys::window().map(|w| w.caches()) {
        Some(Ok(caches)) => JsFuture::from(caches.open("kiln-blobs-v1"))
            .await
            .ok()
            .and_then(|c| c.dyn_into::<web_sys::Cache>().ok()),
        _ => None,
    };
    if let Some(cache) = &cache
        && let Ok(hit) = JsFuture::from(cache.match_with_str(&key)).await
        && let Ok(response) = hit.dyn_into::<web_sys::Response>()
        && let Ok(buffer) = response.array_buffer()
        && let Ok(buffer) = JsFuture::from(buffer).await
    {
        let data = Uint8Array::new(&buffer).to_vec();
        if crate::patch::blob_id(&data) == blob {
            return Ok((data, true));
        }
    }
    let data = fetch_bytes(url).await?;
    if !blob.is_empty() && crate::patch::blob_id(&data) != blob {
        return Err(format!("{url} doesn't match its blob ID"));
    }
    if let Some(cache) = &cache {
        let mut copy = data.clone();
        if let Ok(response) = web_sys::Response::new_with_opt_u8_array(Some(&mut copy)) {
            let _ = JsFuture::from(cache.put_with_str(&key, &response)).await;
        }
    }
    Ok((data, false))
}

/// POST `body` as JSON to `url` and return the answer's text.
async fn post_json(url: &str, body: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or("no window")?;
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_body(&JsValue::from_str(body));
    let headers = Object::new();
    let _ = js_sys::Reflect::set(&headers, &"content-type".into(), &"application/json".into());
    init.set_headers(&headers);
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .map_err(|e| text(&e))?
        .dyn_into()
        .map_err(|e| text(&e))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = JsFuture::from(response.text().map_err(|e| text(&e))?)
        .await
        .map_err(|e| text(&e))?;
    Ok(body.as_string().unwrap_or_default())
}

pub(super) async fn fetch_json(url: &str) -> Result<Value, String> {
    let bytes = fetch_bytes(url).await?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let url = &asset(url);
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|e| text(&e))?
        .dyn_into()
        .map_err(|e| text(&e))?;
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(|e| text(&e))?)
        .await
        .map_err(|e| text(&e))?;
    Ok(Uint8Array::new(&buffer).to_vec())
}
