// Probe in a short-lived worker so its adapter is released before GPUI
// creates devices. Share the result with every lazy module on this page.
function probeAdapter() {
  return Promise.resolve().then(function () {
    if (!navigator.gpu) return "unsupported";
    return navigator.gpu.requestAdapter({ powerPreference: "high-performance" })
      .then(function (adapter) { return adapter ? "webgpu" : "webgl"; });
  }).catch(function () { return "webgl"; });
}

var inWorker = typeof document === "undefined";
if (inWorker) {
  probeAdapter().then(function (backend) {
    globalThis.postMessage(backend);
    globalThis.close();
  });
}

function chooseRenderer() {
  if (!navigator.gpu) return Promise.resolve("webgl");
  return new Promise(function (resolve) {
    function fallback() {
      probeAdapter().then(function (backend) {
        resolve(backend === "webgpu" ? "webgpu" : "webgl");
      });
    }
    var worker;
    try {
      var url = new URL(import.meta.url);
      url.searchParams.set("gpui-probe", "1");
      worker = new Worker(url.href, { type: "module" });
    } catch (_) {
      fallback();
      return;
    }
    function stop() {
      worker.onmessage = worker.onerror = null;
      worker.terminate();
    }
    worker.onmessage = function (event) {
      stop();
      if (event.data === "unsupported") fallback();
      else resolve(event.data === "webgpu" ? "webgpu" : "webgl");
    };
    worker.onerror = function (event) {
      event.preventDefault();
      stop();
      fallback();
    };
  });
}

export const renderer = inWorker ? null :
  (globalThis.__openagentsGpuiRenderer ||
    (globalThis.__openagentsGpuiRenderer = chooseRenderer().then(function (backend) {
      document.documentElement.dataset.gpuiRenderer = backend;
      return backend;
    })));

export function loadRenderer(gpuGlue, gpuWasm, glGlue, glWasm) {
  return renderer.then(function (backend) {
    var glue = backend === "webgpu" ? gpuGlue : glGlue;
    var wasm = backend === "webgpu" ? gpuWasm : glWasm;
    return Promise.all([
      import(glue.href), WebAssembly.compileStreaming(fetch(wasm)),
    ]).then(function (loaded) {
      return loaded[0].default({ module_or_path: loaded[1] }).then(function () {
        return loaded[0];
      });
    });
  });
}
