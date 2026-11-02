// Compare the retained WebGPU renderer and the shared renderer on one producer.
// Instrumentation adds readback/observability only; it never changes a shader,
// resource, draw, frame clock, or pass. A result is qualified only when producer
// uniform, HUD and draw bytes match exactly. Old producer fixes need separate
// before/after evidence using their original Wasm.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve, dirname, extname } from "node:path";
import { createHash } from "node:crypto";
const [wasmArg, packageArg, htmlArg, outputArg, ...options] = process.argv.slice(2);
if (!wasmArg || !packageArg || !htmlArg || !outputArg) throw Error("usage: check_render_goldens.ts PRODUCER.wasm PACKAGE PAGE.html OUTDIR [--start] [--frames N]");
const start = options.includes("--start"), frameFlag = options.indexOf("--frames");
const shaderFlag = options.indexOf("--legacy-wgsl"), reuseFlag = options.indexOf("--reuse-generic"), reuseLegacyFlag = options.indexOf("--reuse-legacy");
const legacyShader = shaderFlag < 0 ? null : resolve(options[shaderFlag + 1]);
const particleFlag = options.indexOf("--legacy-particles");
const legacyParticles = particleFlag < 0 ? null : resolve(options[particleFlag + 1]);
const reuseGeneric = reuseFlag < 0 ? null : resolve(options[reuseFlag + 1]);
const reuseLegacy = reuseLegacyFlag < 0 ? null : resolve(options[reuseLegacyFlag + 1]);
if (reuseGeneric && reuseLegacy) throw Error("reuse at most one route");
const frames = frameFlag < 0 ? 90 : Number(options[frameFlag + 1]);
if (!Number.isInteger(frames) || frames < 1 || frames > 600) throw Error("frames must be 1..600");
const size = options.find(o => o.startsWith("--size="))?.slice(7) ?? "320x180";
const [width, height] = size.split("x").map(Number);
if (!/^\d+x\d+$/.test(size) || ![width,height].every(n => Number.isSafeInteger(n) && n > 0 && n <= 8192)) throw Error("invalid golden viewport");
const query = options.find(o => o.startsWith("--query="))?.slice(8) ?? "";
const parameters = new URLSearchParams(query);
for (const flag of ["probe","golden","probe-manual"]) parameters.set(flag, "");
const root = resolve(dirname(import.meta.path), "../.."), wasm = resolve(wasmArg), pack = resolve(packageArg), htmlPath = resolve(htmlArg), out = resolve(outputArg);
const packageApp = JSON.parse(readFileSync(`${pack}/render.json`, "utf8")).app;
const watchdog = setTimeout(() => { console.error("golden comparison exceeded its 180 second command budget");process.exit(1); }, 180000);
mkdirSync(out, { recursive: true });
writeFileSync(`${out}/driver.ts`, readFileSync(import.meta.path));
const sha = (bytes: Uint8Array | string) => createHash("sha256").update(bytes).digest("hex");
const source = readFileSync(`${root}/coax/web/host.js`, "utf8");
const replaceOnce = (text: string, old: string, value: string) => {
  if (text.split(old).length !== 2) throw Error("legacy observation anchor changed: " + old);
  return text.replace(old, value);
};
let instrumented = replaceOnce(source,
  "const readback = probe && ++frames >= 1 && !gpu.probed ? readBack(encoder, texture) : null;",
  "const readback = probe && probeArmed && ++frames >= 1 && !gpu.probed ? readBack(encoder, texture) : null;");
instrumented = replaceOnce(instrumented, "if (readback) report(readback, texture);", `if (readback) report({ ...readback, rendererSnapshot: {
  admitted_hud: n, admitted_draws: draws, vertex_stride: arena.floats,
  uniform_bytes: Array.from(new Uint8Array(memory.buffer, arena.uniforms, arena.uniformFloats * 4)),
  vertex_bytes: Array.from(new Uint8Array(memory.buffer, arena.vertices, n * arena.floats * 4)),
  draw_bytes: draws ? Array.from(new Uint8Array(memory.buffer, x.coaxial_draws(arena.state), draws * 16)) : [],
} }, texture);`);
instrumented = replaceOnce(instrumented, "  const x0 = Math.floor(w * 0.3)", `  const tight = new Uint8Array(w * h * pixelBytes);
  for (let y = 0; y < h; y++) tight.set(px.subarray(y * row, y * row + w * pixelBytes), y * w * pixelBytes);
  globalThis.coaxial.renderCapture = { schema: "aethyr.render.legacy-readback.v1", width: w, height: h,
    format: texture.format, row_bytes: w * pixelBytes, bytes: Array.from(tight), ...rendererSnapshot };
  buffer.unmap();buffer.destroy();return;
  const x0 = Math.floor(w * 0.3)`);
instrumented += `\nglobalThis.coaxial.goldenStatus = () => ({engine: !!engine, gpu: !!gpu?.bg,
  shaders: shaderLoads, scene: !!gpu?.scene, group: !!gpu?.sceneGroup,
  ready: !!gpu?.bg && shaderLoads === 0 && (!gpu.scene || !!gpu.sceneGroup)});\n`;
writeFileSync(`${out}/observed-host.js`, instrumented);
const sourceBefore = Object.fromEntries([wasm, htmlPath, import.meta.path, `${pack}/render.json`, `${root}/coax/web/host.js`, `${root}/coax/web/render.js`, `${root}/coax/web/chrome.ts`, ...(legacyShader ? [legacyShader] : []), ...(legacyParticles ? [legacyParticles] : [])].map(p => [p, sha(readFileSync(p))]));
const results: any = {};
if (reuseGeneric || reuseLegacy) {
  const reused = reuseGeneric ?? reuseLegacy!, route = reuseGeneric ? "generic" : "legacy";
  const receipt = JSON.parse(readFileSync(`${reused}/receipt.json`, "utf8"));
  if (receipt.route !== route || receipt.source_changes.length || receipt.exceptions.length || receipt.errors.length || receipt.frames !== frames || receipt.start !== start || (receipt.query ?? "") !== query || (receipt.width ?? 320) !== width || (receipt.height ?? 180) !== height) throw Error("reused capture is not qualified for these controls");
  if (route === "legacy" && receipt.legacy_shader_override !== legacyShader) throw Error("reused legacy shader control differs");
  if (route === "legacy" && (receipt.legacy_particle_override ?? null) !== legacyParticles) throw Error("reused legacy particle control differs");
  const producer = Object.entries(receipt.source_inputs).find(([path]) => path.endsWith(".wasm"))?.[1];
  if (producer !== sha(readFileSync(wasm))) throw Error("reused capture has a different producer Wasm");
  const directory = `${out}/${route}`;mkdirSync(directory, { recursive: true });
  for (const key of ["bytes","uniform_bytes","vertex_bytes","draw_bytes"]) {
    const entry = receipt.capture[key], bytes = readFileSync(`${reused}/${entry.file}`);
    if (bytes.length !== entry.bytes || sha(bytes) !== entry.sha256) throw Error("reused artifact changed");
    writeFileSync(`${directory}/${entry.file}`, bytes);
  }
  writeFileSync(`${directory}/receipt.json`, JSON.stringify(receipt,null,2) + "\n");results[route] = receipt;
}
for (const route of reuseGeneric ? ["legacy"] : reuseLegacy ? ["generic"] : ["legacy", "generic"]) {
  const directory = `${out}/${route}`;mkdirSync(directory, { recursive: true });
  const served = new Map<string,string>(), events: any[] = [], requests: string[] = [];let stage = "startup";
  let html = readFileSync(htmlPath, "utf8");
  html = route === "legacy" ? html.replace(/\sdata-render(?:-profile)?="[^"]*"/g, "") : html.replace(/data-render="[^"]*"/, 'data-render="/package/render.json"');
  html = /data-wasm="/.test(html) ? html.replace(/data-wasm="[^"]*"/, 'data-wasm="/producer.wasm"') : html.replace(/data-app="[^"]*"/, '$& data-wasm="/producer.wasm"');
  const server = Bun.serve({ hostname: "127.0.0.1", port: 0, websocket: { message() {} }, fetch(req, server) {
    const path = new URL(req.url).pathname;requests.push(path);
    if (path === "/api/auth/viewer") return new Response("");
    if (path === "/api/villa/pets" || path === "/api/villa/garden") return Response.json({ pets: [] });
    if (path === "/coaxial/live" && server.upgrade(req)) return;
    if (path.startsWith("/api/playtest/")) return Response.json({});
    if (path === "/api/analytics/events") return new Response(null, { status: 204 });
    if (path === "/favicon.ico") return new Response(null, { status: 204 });
    if (path === "/") return new Response(html, { headers: { "content-type": "text/html" } });
    const file = route === "legacy" && legacyParticles && path === "/particles.wgsl" ? legacyParticles : route === "legacy" && legacyShader && path === `/${packageApp}.wgsl` ? legacyShader : path === "/producer.wasm" ? wasm : path.startsWith("/coaxial/") ? resolve(root, "coax/web", path.slice(9)) : path.startsWith("/package/") ? resolve(pack, path.slice(9)) : resolve(root, "apps/openagents-web", path.slice(1));
    try {
      const bytes = readFileSync(file), digest = sha(bytes);
      if (served.has(file) && served.get(file) !== digest) throw Error("served source changed");
      served.set(file, digest);
      const body = file === `${root}/coax/web/host.js` ? instrumented : bytes;
      const type = extname(file) === ".js" ? "text/javascript" : extname(file) === ".wasm" ? "application/wasm" : extname(file) === ".json" ? "application/json" : "application/octet-stream";
      return new Response(body, { headers: { "content-type": type } });
    } catch { return new Response("not found", { status: 404 }); }
  }});
  const chrome = await startChrome(`aethyr-golden-${route}-`, ["--disable-background-timer-throttling", "--disable-renderer-backgrounding", "--disable-backgrounding-occluded-windows"], 1);let ws: WebSocket | undefined;
  try {
    const pages = await (await fetch(`http://127.0.0.1:${chrome.port}/json/list`)).json();
    ws = new WebSocket(pages.find((p: any) => p.type === "page").webSocketDebuggerUrl);
    let serial = 0;const pending = new Map<number, any>();
    ws.onmessage = e => { const m = JSON.parse(String(e.data));if (!m.id) { if (m.method?.startsWith("Runtime.") || m.method === "Log.entryAdded") events.push(m);return; }const q = pending.get(m.id);pending.delete(m.id);clearTimeout(q?.timer);m.error ? q?.reject(m.error) : q?.resolve(m.result); };
    await new Promise<void>((r,j) => { ws!.onopen = () => r();ws!.onerror = j; });
    const call = (method: string, params: any = {}) => new Promise<any>((resolve,reject) => { const id = ++serial, timer = setTimeout(() => { pending.delete(id);reject(Error(method + " timeout")); }, 30000);pending.set(id, { resolve,reject,timer });ws!.send(JSON.stringify({ id,method,params })); });
    const evaluate = async (expression: string) => { const r = await call("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });if (r.exceptionDetails) throw Error(JSON.stringify(r.exceptionDetails));return r.result.value; };
    const browser = await call("Browser.getVersion");await call("Page.enable");await call("Runtime.enable");await call("Log.enable");
    await call("Emulation.setDeviceMetricsOverride", { width,height,deviceScaleFactor: 1,mobile: false });
    await call("Page.addScriptToEvaluateOnNewDocument", { source: "globalThis.coaxialClip=true;" });
    await call("Page.navigate", { url: `http://127.0.0.1:${server.port}/?${parameters}` });await call("Page.bringToFront");
    let status: any;const deadline = Date.now() + 60000;stage = "warmup";
    while (Date.now() < deadline) {
      await evaluate("globalThis.coaxialClipStep?.(1,Infinity)");
      status = await evaluate("({capture:globalThis.coaxial?.renderCapture, ready:globalThis.coaxial?.goldenStatus?.(), render:globalThis.coaxial?.renderStatus, loader:document.querySelector('#coaxial-load')?.textContent})");
      if (status?.loader?.includes("FAILED:") || status?.ready?.ready && (route === "legacy" || status?.render?.readiness === 2)) break;
      await Bun.sleep(50);
    }
    if (!status?.ready?.ready || route === "generic" && status?.render?.readiness !== 2) throw Error("renderer did not become ready: " + JSON.stringify(status));
    console.log(route + ": ready");stage = "start";
    if (start) for (const type of ["keyDown", "keyUp"]) await call("Input.dispatchKeyEvent", { type,key: "Enter",code: "Enter",windowsVirtualKeyCode: 13,nativeVirtualKeyCode: 13 });
    // ClipStep's batch API advances the producer by these exact fixed steps.
    // This golden observes its final frame; it is not a frame-rate benchmark.
    stage = "fixed producer steps";
    if (frames > 1) await evaluate(`globalThis.coaxialClipStep(${frames - 1},60)`);
    stage = "captured frame";await call("Page.bringToFront");
    await evaluate("globalThis.coaxial.captureFrame()");
    await evaluate("globalThis.coaxialClipStep(1,60)");
    stage = "readback";
    // This comparison consumes the final image and producer bytes. Physical
    // resource captures belong to check_render_host.ts; avoid transporting a
    // whole decoded texture array over CDP only to discard it afterward.
    const observation = `(()=>{const raw=globalThis.coaxial?.renderCapture;
      let capture;if(raw){const {buffers,textures,index_bytes,...used}=raw;capture=used;}
      return {capture,render:globalThis.coaxial?.renderStatus,loader:document.querySelector('#coaxial-load')?.textContent};})()`;
    for (let i = 0; i < 200; i++) { status = await evaluate(observation);if (status?.capture) break;await Bun.sleep(25); }
    if (!status?.capture) throw Error("no complete readback: " + JSON.stringify(status));
    const capture = status.capture;
    for (const [key, name] of [["bytes","gpu.bin"],["uniform_bytes","u.bin"],["vertex_bytes","hud.bin"],["draw_bytes","draws.bin"]]) {
      const bytes = new Uint8Array(capture[key] ?? []);writeFileSync(`${directory}/${name}`, bytes);capture[key] = { file: name,bytes: bytes.length,sha256: sha(bytes) };
    }
    delete capture.buffers;delete capture.textures;delete capture.index_bytes;
    const changes = [...served].filter(([file,digest]) => sha(readFileSync(file)) !== digest).map(([file]) => file);
    const exceptions = events.filter(e => e.method === "Runtime.exceptionThrown");
    const errors = events.filter(e => e.method === "Log.entryAdded" && e.params?.entry?.level === "error");
    const receipt = { route,browser,frames,start,query,width,height,legacy_shader_override: legacyShader,legacy_particle_override: legacyParticles,instrumentation_sha256: sha(instrumented),source_inputs: sourceBefore,served: Object.fromEntries(served),source_changes: changes,capture,render: status.render,requests,events,exceptions,errors };
    writeFileSync(`${directory}/receipt.json`, JSON.stringify(receipt,null,2) + "\n");
    if (changes.length || exceptions.length || errors.length) throw Error(route + " validation/source errors");
    results[route] = receipt;
  } catch (error) {
    writeFileSync(`${directory}/failure.json`, JSON.stringify({ route,stage,error: String(error),requests,events,source_inputs: sourceBefore },null,2) + "\n");throw error;
  } finally { ws?.close();server.stop(true);chrome.proc.kill("SIGKILL");await chrome.proc.exited; }
}
const a = results.legacy.capture, b = results.generic.capture;
const wasmDigest = (r: any) => Object.entries(r.source_inputs).find(([path]) => path.endsWith(".wasm"))?.[1];
const sameProducer = wasmDigest(results.legacy) === wasmDigest(results.generic);
const sameInputs = ["uniform_bytes","vertex_bytes","draw_bytes"].every(key => a[key].sha256 === b[key].sha256);
const sameShape = a.width === b.width && a.height === b.height && a.format === b.format;
const before = readFileSync(`${out}/legacy/gpu.bin`), after = readFileSync(`${out}/generic/gpu.bin`);
let channels = 0, pixels = 0, max = 0;
if (sameShape && before.length === after.length) for (let i = 0; i < before.length; i += 4) { let different = false;for (let k = 0; k < 4; k++) { const delta = Math.abs(before[i+k] - after[i+k]);if (delta) { channels++;different = true; }max = Math.max(max,delta); }if (different) pixels++; }
const changes = Object.keys(sourceBefore).filter(p => sha(readFileSync(p)) !== sourceBefore[p]);
const receipt = { schema: "aethyr.render.legacy-comparison.v1",same_producer_wasm: sameProducer,same_inputs: sameInputs,same_shape: sameShape,frames,start,query,width,height,legacy_shader_override: legacyShader,legacy_particle_override: legacyParticles,reused_generic: reuseGeneric,reused_legacy: reuseLegacy,source_changes: changes,legacy_sha256: sha(before),generic_sha256: sha(after),different_channels: channels,different_pixels: pixels,max_byte_delta: max,identical: sameProducer && sameInputs && sameShape && before.equals(after),scope: "exact matching captured producer bytes, legacy WebGPU pass executor vs shared renderer; supplied scene or particle variants are explicit diagnostic controls" };
writeFileSync(`${out}/comparison.json`, JSON.stringify(receipt,null,2) + "\n");console.log(JSON.stringify(receipt,null,2));
if (!sameProducer || !sameInputs || !sameShape || changes.length) throw Error("comparison inputs do not match");
clearTimeout(watchdog);
