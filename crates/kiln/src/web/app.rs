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
use crate::agent::{Agent, Command, INSTRUCTIONS};
use crate::cj::{self, Key};
use crate::vfs::Vfs;

const SNAPSHOT: &str = "kiln-workspace-v1.bin";
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
    key: Key,
    worker: RefCell<Option<Worker>>,
    busy: Cell<bool>,
    stop: Cell<bool>,
    metrics: RefCell<Metrics>,
}

struct ToolRun {
    code: i32,
    output: String,
}

fn el(doc: &Document, id: &str) -> Element {
    doc.get_element_by_id(id)
        .unwrap_or_else(|| wasm_bindgen::throw_str(&format!("#{id} is missing")))
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
    let app = Rc::new(App {
        doc,
        vfs: RefCell::new(Vfs::default()),
        tools: RefCell::new(BTreeMap::new()),
        opfs: RefCell::new(None),
        key,
        worker: RefCell::new(None),
        busy: Cell::new(false),
        stop: Cell::new(false),
        metrics: RefCell::new(Metrics::default()),
    });
    app.wire();
    let storage = app.open_storage().await;
    let tools = app.load_tools().await;
    let isolated = get(&window, "crossOriginIsolated").as_bool() == Some(true);
    el(&app.doc, "key").set_text_content(Some(&app.key.npub()));
    el(&app.doc, "env").set_text_content(Some(&format!(
        "{storage} · {tools} · cross-origin isolated: {}",
        if isolated { "yes" } else { "no" }
    )));
    app.page_metrics();
    app.render_files();
    let params =
        web_sys::UrlSearchParams::new_with_str(&window.location().search().unwrap_or_default())
            .ok();
    let param = |name: &str| params.as_ref().and_then(|p| p.get(name));
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
    set(&window, "kilnReady", true);
}

impl App {
    fn log(&self, class: &str, text: &str) {
        let log = el(&self.doc, "log");
        if let Ok(line) = self.doc.create_element("div") {
            line.set_class_name(class);
            line.set_text_content(Some(text));
            let _ = log.append_child(&line);
            log.set_scroll_top(log.scroll_height());
        }
    }

    fn wire(self: &Rc<Self>) {
        let on = |id: &str, f: Box<dyn Fn(web_sys::Event)>| {
            let closure = Closure::<dyn Fn(web_sys::Event)>::new(f);
            let _ = el(&self.doc, id)
                .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
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
                    app.persist().await;
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
            let handle: FileSystemFileHandle =
                JsFuture::from(dir.get_file_handle_with_options(SNAPSHOT, &options))
                    .await?
                    .dyn_into()?;
            let file: web_sys::File = JsFuture::from(handle.get_file()).await?.dyn_into()?;
            let bytes = Uint8Array::new(&JsFuture::from(file.array_buffer()).await?).to_vec();
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
        el(&self.doc, "usage").set_text_content(Some(&format!("{count} files, {bytes} bytes")));
    }

    fn take_worker(&self) -> Result<Worker, String> {
        if let Some(worker) = self.worker.borrow_mut().take() {
            return Ok(worker);
        }
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        Worker::new_with_options("./worker.js", &options).map_err(|e| text(&e))
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
        let reply = match select(rx.next(), Box::pin(sleep(TOOL_TIMEOUT_MS))).await {
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
        match command.call() {
            Ok(Some(call)) => {
                self.log("cmd", &format!("$ {}", command.label()));
                let run = self.run_tool(&call.argv, &call.stdin).await;
                let shown = crate::agent::clip(run.output.trim_end(), 4000);
                if !shown.is_empty() {
                    self.log(if run.code == 0 { "out" } else { "err" }, &shown);
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
        let mut agent = Agent::new(&task, cj::LEGACY_MAX_INPUT_BYTES);
        let mut relay: Option<Relay> = None;
        let mut book = BeaconBook::default();
        let mut book_at = f64::NEG_INFINITY;
        let mut failing = BTreeSet::new();
        let run_started = now_ms();
        for step in 1..=MAX_STEPS {
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
            self.log("step", &format!("Step {step}"));
            let files = self.listing();
            let snapshot = agent.clone();
            let prompt = move |budget: usize| {
                let mut a = snapshot.clone();
                a.budget = budget;
                a.prompt(&files)
            };
            let app = self.clone();
            let feedback = move |note: &str| app.log("info", note);
            let asked = conn
                .ask(
                    &self.key,
                    &book,
                    want,
                    fallback,
                    &failing,
                    INSTRUCTIONS,
                    &prompt,
                    &feedback,
                )
                .await;
            let asked = match asked {
                Ok(asked) => asked,
                Err(error) => {
                    self.log("error", &error.message);
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
            let commands = agent.plan(&asked.text);
            if commands.is_empty() {
                self.log(
                    "error",
                    &format!(
                        "The reply was not usable: {}",
                        crate::agent::clip(&asked.text, 600)
                    ),
                );
            } else if let Some(last) = agent.steps.last() {
                self.log("thought", &last.thought.clone());
            }
            let tools_started = now_ms();
            for command in &commands {
                if let Command::Done(summary) = command {
                    agent.observe(command, 0, String::new());
                    self.log("done", &format!("Done: {summary}"));
                    break;
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
        if let Some(window) = web_sys::window() {
            set(&window, "kilnAgentDone", agent.done.is_some());
        }
        self.busy.set(false);
    }
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
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
