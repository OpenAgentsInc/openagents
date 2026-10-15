// The CAD engine off the page's thread (/cad, #11649). The page posts
// {id, call, args, format}; the answer is {id, ok: true, value} or
// {id, ok: false, error, broken}. `broken` means the engine stopped
// (a WebAssembly trap) and the page should start a new worker.
//
//   run      args {tool, args}: a cad_* tool's JSON answer
//   export   args {document}, format glb|step|3mf|stl: the file's bytes
//   drawing  args {document, views, ...}, format svg|pdf|dxf: the bytes
import init, { run, export_file, drawing_file } from "./cad_wasm.js";

const started = performance.now();
const ready = init({ module_or_path: new URL("./cad_wasm_bg.wasm", import.meta.url) });
ready.then(
  () => self.postMessage({ ready: true, ms: Math.round(performance.now() - started) }),
  (error) => self.postMessage({ ready: false, error: String((error && error.message) || error) }),
);

self.onmessage = async (event) => {
  const { id, call, args, format } = event.data;
  try {
    await ready;
    let value;
    if (call === "run") {
      value = JSON.parse(run(args.tool, JSON.stringify(args.args)));
    } else if (call === "export") {
      value = export_file(JSON.stringify(args.document), format);
    } else if (call === "drawing") {
      value = drawing_file(JSON.stringify(args), format);
    } else {
      throw new Error(`unknown call ${call}`);
    }
    const transfer = value instanceof Uint8Array ? [value.buffer] : [];
    self.postMessage({ id, ok: true, value }, transfer);
  } catch (error) {
    self.postMessage({
      id,
      ok: false,
      error: String((error && error.message) || error),
      broken: error instanceof WebAssembly.RuntimeError,
    });
  }
};
