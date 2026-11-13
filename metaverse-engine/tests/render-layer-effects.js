// Firm reason: 1 (WebAssembly, WebGPU, ImageBitmap, digest and HTTP browser SDK effects; Coax schedules and decides fixture admission).
(() => {
  const state = { done: false, error: null, value: null };
  const digest = async bytes => [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(b => b.toString(16).padStart(2, '0')).join('');
  const start = fn => {
    state.done = false; state.error = null; state.value = null;
    Promise.resolve().then(fn).then(value => { state.value = value; state.done = true; }, error => { state.error = String(error.stack || error); state.done = true; });
    return 'started';
  };
  globalThis.renderLayerPoll = () => ({ done: state.done, error: state.error, value: state.value });
  globalThis.renderLayerInitialize = request => start(async () => {
    const { RenderHost } = await import('/coaxial/render.js');
    const adapter = await navigator.gpu.requestAdapter(), device = await adapter.requestDevice();
    const memory = new WebAssembly.Memory({ initial: 256, maximum: 1024 });
    const wasm = await (await fetch('/test.wasm')).arrayBuffer();
    const module = await WebAssembly.compile(wasm), imports = { env: { memory } };
    for (const row of WebAssembly.Module.imports(module)) if (row.kind === 'function') (imports[row.module] ||= {})[row.name] = () => { throw Error('unexpected import ' + row.module + '.' + row.name); };
    const instance = await WebAssembly.instantiate(module, imports), x = instance.exports;
    x._initialize?.(); x.coaxial_render_reset(); x.coaxial_render_device(1, 1, 0);
    const data = new Uint8Array(await (await fetch('/package/render.json')).arrayBuffer());
    const admitted = x.coaxial_render_json_admit(data.length);
    const observed = { wasm_bytes: wasm.byteLength, wasm_sha256: await digest(wasm), package_bytes: data.length, package_sha256: await digest(data), admitted, adapter: adapter.info };
    if (admitted !== 0) return observed;
    new Uint8Array(memory.buffer, x.coaxial_render_json_ptr(), data.length).set(data);
    observed.prepared = x.coaxial_render_prepare(data.length, request.profile);
    if (observed.prepared !== 0) return observed;
    const renderer = new RenderHost({ device, memory, exports: x, arena: {}, profile: request.profile, capture: true });
    renderer.ids = { asset: Object.fromEntries(['invalid', 'data', 'storage', 'index', 'layers', 'sound', 'loop', 'particles', 'mesh'].map((name, i) => [name, x.coaxial_render_enum(4, i)])) };
    renderer.readAssets(); state.renderer = renderer;
    observed.assets = renderer.assetRows; observed.layer_kind = renderer.ids.asset.layers;
    observed.texture_kind = x.coaxial_render_enum(2, 7); observed.bindings = [];
    for (let resource = 0; resource < x.coaxial_render_binding_count(0); resource++) observed.bindings.push({ resource, kind: Number(x.coaxial_render_binding(0, resource, 6)) });
    return observed;
  });
  globalThis.renderLayerDecode = request => start(async () => {
    const images = [], observed = [];
    for (const source of request.images) {
      const response = await fetch(source.url), data = await response.arrayBuffer();
      const image = await createImageBitmap(new Blob([data])); images.push(image);
      observed.push({ url: source.url, status: response.status, bytes: data.byteLength, sha256: await digest(data), width: image.width, height: image.height });
    }
    state.images = images; return { images: observed };
  });
  globalThis.renderLayerCapture = request => start(async () => {
    state.renderer.layerRequest = { set: request.set, resource: request.resource };
    state.renderer.textureCaptureSink = async (receipt, bytes) => { state.capture = receipt; state.bytes = bytes; };
    await state.renderer.makeLayers(state.images);
    return { capture: state.capture, bytes: state.bytes.length };
  });
  globalThis.renderLayerUpload = request => start(async () => {
    const response = await fetch(request.url, { method: 'POST', body: state.bytes.slice(request.from, request.to) });
    return { status: response.status, ack: await response.json() };
  });
  return 'ready';
})()
