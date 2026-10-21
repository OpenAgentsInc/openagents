//! The tool worker: instantiate one `wasm32-wasip1` module with every
//! `wasi_snapshot_preview1` import bound to [`crate::wasi::Process`], run
//! `_start`, and hand back the output and the changed workspace.
//!
//! The worker gets a snapshot of the workspace and returns the new one, so
//! no call ever blocks on the page: the program sees an ordinary
//! synchronous filesystem, and nothing needs `SharedArrayBuffer`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js_sys::{Array, Function, Object, Uint8Array, WebAssembly};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::util::{get, now_ms, set, text};
use crate::vfs::Vfs;
use crate::wasi::{Memory, Outcome, Platform, Process};

struct Browser;

impl Platform for Browser {
    fn realtime_ns(&self) -> u64 {
        (js_sys::Date::now() * 1e6) as u64
    }
    fn monotonic_ns(&self) -> u64 {
        (now_ms() * 1e6) as u64
    }
    fn random(&self, buf: &mut [u8]) {
        let _ = getrandom::fill(buf);
    }
}

struct JsMemory(Uint8Array);

impl Memory for JsMemory {
    fn read(&self, ptr: u32, len: u32) -> Vec<u8> {
        self.0.subarray(ptr, ptr + len).to_vec()
    }
    fn write(&mut self, ptr: u32, data: &[u8]) {
        self.0
            .subarray(ptr, ptr + data.len() as u32)
            .copy_from(data);
    }
}

/// The sentinel a host call returns for `proc_exit`; the shim throws it.
const EXIT: &str = "kiln-exit";

pub async fn run(message: JsValue) -> Result<JsValue, JsValue> {
    let started = now_ms();
    let module: WebAssembly::Module = get(&message, "module").dyn_into()?;
    let argv: Vec<String> = Array::from(&get(&message, "argv"))
        .iter()
        .filter_map(|v| v.as_string())
        .collect();
    let stdin = Uint8Array::new(&get(&message, "stdin")).to_vec();
    let snapshot = Uint8Array::new(&get(&message, "fs")).to_vec();
    let vfs = Vfs::decode(&snapshot).map_err(|e| JsValue::from_str(&e))?;
    let env = vec!["HOME=/".to_string(), "PWD=/".to_string()];
    let process = Rc::new(RefCell::new(Process::new(vfs, argv, env, stdin, Browser)));
    let memory: Rc<RefCell<Option<WebAssembly::Memory>>> = Rc::new(RefCell::new(None));
    let exit: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));

    let handler = {
        let (process, memory, exit) = (process.clone(), memory.clone(), exit.clone());
        Closure::<dyn Fn(String, Array) -> JsValue>::new(move |name: String, args: Array| {
            let args: Vec<i64> = args
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as i64)
                .collect();
            let Some(mem) = memory.borrow().clone() else {
                return JsValue::from(crate::wasi::errno::NOSYS);
            };
            let mut mem = JsMemory(Uint8Array::new(&mem.buffer()));
            match process.borrow_mut().call(&name, &args, &mut mem) {
                Outcome::Return(code) => JsValue::from(code),
                Outcome::Exit(code) => {
                    exit.set(Some(code));
                    JsValue::from_str(EXIT)
                }
            }
        })
    };
    // Each import becomes a JavaScript function that forwards its
    // arguments (64-bit ones as numbers) and throws to end the program.
    let shim = Function::new_with_args(
        "h, name",
        "return (...a) => { const r = h(name, a.map(x => typeof x === 'bigint' ? Number(BigInt.asIntN(64, x)) : x)); if (typeof r === 'string') throw new Error(r); return r; }",
    );
    let wasi = Object::new();
    for import in WebAssembly::Module::imports(&module).iter() {
        let module_name = get(&import, "module").as_string().unwrap_or_default();
        let name = get(&import, "name").as_string().unwrap_or_default();
        if module_name == "wasi_snapshot_preview1" {
            let f = shim.call2(&JsValue::NULL, handler.as_ref(), &JsValue::from_str(&name))?;
            set(&wasi, &name, f);
        }
    }
    let imports = Object::new();
    set(&imports, "wasi_snapshot_preview1", wasi);
    let instance: WebAssembly::Instance =
        JsFuture::from(WebAssembly::instantiate_module(&module, &imports))
            .await?
            .dyn_into()?;
    let exports = instance.exports();
    *memory.borrow_mut() = Some(get(&exports, "memory").dyn_into()?);
    let instantiated = now_ms();
    let start: Function = get(&exports, "_start").dyn_into()?;
    let result = start.call0(&JsValue::NULL);
    let finished = now_ms();
    drop(handler);

    let mut process = process.borrow_mut();
    let code = match (exit.get(), result) {
        (Some(code), _) => code,
        (None, Ok(_)) => 0,
        (None, Err(error)) => {
            process
                .stderr
                .extend_from_slice(format!("\nthe program trapped: {}\n", text(&error)).as_bytes());
            134
        }
    };
    let out = Object::new();
    set(&out, "code", code);
    set(&out, "stdout", Uint8Array::from(process.stdout.as_slice()));
    set(&out, "stderr", Uint8Array::from(process.stderr.as_slice()));
    set(
        &out,
        "fs",
        Uint8Array::from(process.vfs.encode().as_slice()),
    );
    set(&out, "instantiate_ms", instantiated - started);
    set(&out, "run_ms", finished - instantiated);
    let calls: u64 = process.counts.values().sum();
    set(&out, "host_calls", calls as f64);
    Ok(out.into())
}
