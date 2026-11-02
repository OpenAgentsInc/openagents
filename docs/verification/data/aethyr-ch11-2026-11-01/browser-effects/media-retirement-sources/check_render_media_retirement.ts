// A delayed real image completion must not resurrect a retired renderer.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve, dirname, extname } from "node:path";
import { createHash } from "node:crypto";
const [wasmArg, packageArg, outputArg, fault = ""] = process.argv.slice(2);
if (!wasmArg || !packageArg || !outputArg || !["", "keep-cache"].includes(fault)) throw Error("usage: check_render_media_retirement.ts SEEDED_VILLA.wasm PACKAGE_DIR OUTDIR [keep-cache]");
const root = resolve(dirname(import.meta.path), "../.."), wasm = resolve(wasmArg), pack = resolve(packageArg), out = resolve(outputArg);
mkdirSync(out, { recursive: true });
const sha = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex"), inputs = new Map<string, string>(), requests: string[] = [], events: any[] = [];
for (const path of [wasm, import.meta.path, `${root}/coax/web/host.js`, `${root}/coax/web/render.js`, `${pack}/render.json`]) inputs.set(path, sha(readFileSync(path)));
let html = readFileSync(`${root}/apps/openagents-web/realms/hecates-cemetery/index.html`, "utf8");
html = html.replace(/data-render="[^"]*"/, 'data-render="/package/render.json"').replace(/data-app="[^"]*"/, '$& data-wasm="/test.wasm"');
let rejectShader = false;
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch(req) {
  const path = new URL(req.url).pathname; requests.push(path);
  if (path === "/") return new Response(html, { headers: { "content-type": "text/html" } });
  if (path === "/api/auth/viewer") return new Response("");
  const file = path === "/test.wasm" ? wasm : path.startsWith("/api/villa/memorial/fixture0/media/") ? `${root}/apps/openagents-web/og/default.jpg` : path.startsWith("/package/") ? resolve(pack, path.slice(9)) : path.startsWith("/coaxial/") ? resolve(root, "coax/web", path.slice(9)) : resolve(root, "apps/openagents-web", path.slice(1));
  try {
    let bytes = readFileSync(file); const digest = sha(bytes);
    if (inputs.has(file) && inputs.get(file) !== digest) throw Error("served source changed"); inputs.set(file, digest);
    if (file.endsWith("/render.js") && fault) {
      const text = bytes.toString(), needle = "this.mediaCache?.clear();";
      if (text.split(needle).length !== 2) throw Error("fault site changed");
      bytes = Buffer.from(text.replace(needle, "/* planted: retained retired cache */"));
    }
    if (rejectShader && path.startsWith("/package/") && extname(file) === ".wgsl") bytes = Buffer.concat([bytes, Buffer.from("\n// planted replacement digest mismatch\n")]);
    return new Response(bytes, { headers: { "content-type": file.endsWith(".js") ? "text/javascript" : file.endsWith(".wasm") ? "application/wasm" : file.endsWith(".json") ? "application/json" : "application/octet-stream" } });
  } catch { return new Response("not found", { status: 404 }); }
} });
const chrome = await startChrome("aethyr-media-retirement-", ["--disable-background-timer-throttling", "--disable-renderer-backgrounding"], 1); let ws: WebSocket | undefined;
try {
  const pages = await (await fetch(`http://127.0.0.1:${chrome.port}/json/list`)).json(); ws = new WebSocket(pages.find((p: any) => p.type === "page").webSocketDebuggerUrl);
  let serial = 0; const pending = new Map<number, any>();
  ws.onmessage = (e) => { const message = JSON.parse(String(e.data)); if (!message.id) { events.push(message); return; } const q = pending.get(message.id); if (!q) return; pending.delete(message.id); clearTimeout(q.timer); message.error ? q.reject(message.error) : q.resolve(message.result); };
  await new Promise<void>((resolve, reject) => { ws!.onopen = () => resolve(); ws!.onerror = reject; });
  const call = (method: string, params: any = {}) => new Promise<any>((resolve, reject) => { const id = ++serial, timer = setTimeout(() => { pending.delete(id); reject(Error(`${method} timed out`)); }, 120000); pending.set(id, { resolve, reject, timer }); ws!.send(JSON.stringify({ id, method, params })); });
  const evaluate = async (expression: string) => { const r = await call("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }); if (r.exceptionDetails) throw Error(JSON.stringify(r.exceptionDetails)); return r.result.value; };
  const browser = await call("Browser.getVersion"); await call("Page.enable"); await call("Runtime.enable"); await call("Emulation.setDeviceMetricsOverride", { width: 320, height: 240, deviceScaleFactor: 1, mobile: false });
  await call("Page.addScriptToEvaluateOnNewDocument", { source: `globalThis.coaxialClip=true;(()=>{let release;const gate=new Promise(r=>release=r),original=fetch;globalThis.retireRelease=release;globalThis.retireDecoded=0;globalThis.retireClosed=0;const bitmap=createImageBitmap;globalThis.createImageBitmap=async(...a)=>{const b=await bitmap(...a);retireDecoded++;return b};const close=ImageBitmap.prototype.close;ImageBitmap.prototype.close=function(){retireClosed++;return close.call(this)};globalThis.fetch=async(...args)=>{const response=await original(...args);const input=args[0],url=new URL(typeof input==='string'?input:input instanceof URL?input.href:input.url,location.href);if(!url.pathname.startsWith('/api/villa/memorial/fixture0/media/'))return response;const bytes=await response.arrayBuffer();await gate;return new Response(bytes,{status:response.status,headers:response.headers})}})()` });
  await call("Page.navigate", { url: `http://127.0.0.1:${server.port}/?probe&golden&probe-manual` });
  const until = Date.now() + 60000;
  while (Date.now() < until) { await evaluate("globalThis.coaxialClipStep?.(1,60)"); if (await evaluate("!!globalThis.coaxial?.renderStatus")) break; await Bun.sleep(50); }
  for (const type of ["keyDown", "keyUp"]) await call("Input.dispatchKeyEvent", { type, key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 });
  await evaluate("coaxial.renderFixture()");
  while (Date.now() < until) { await evaluate("coaxialClipStep(1,60)"); if (await evaluate("coaxial.renderMediaObjects().length>0")) break; await Bun.sleep(50); }
  const waiting = await evaluate("(()=>{globalThis.retiredMedia=coaxial.renderMediaObjects()[0];return {present:!!retiredMedia,ready:!!retiredMedia?.ready,decoded:retireDecoded}})()");
  if (!waiting.present || waiting.ready || waiting.decoded) throw Error("media was not genuinely pending");
  rejectShader = true;
  const replacement = await evaluate("coaxial.reloadEngine().then(()=>({failed:false}),e=>({failed:true,error:e.message}))"); rejectShader = false;
  const stopped = await evaluate("coaxial.renderBindingState()");
  await evaluate("(async()=>{const {RenderHost}=await import('/coaxial/render.js');globalThis.lateAllocations=0;const allocate=RenderHost.prototype.mediaAllocate;RenderHost.prototype.mediaAllocate=function(...args){lateAllocations++;return allocate.apply(this,args)};globalThis.lateTextures=0;const create=GPUDevice.prototype.createTexture;GPUDevice.prototype.createTexture=function(...args){lateTextures++;return create.apply(this,args)};retireRelease()})()");
  const completed = Date.now() + 10000; let after: any;
  do { after = await evaluate("({decoded:retireDecoded,closed:retireClosed,allocations:lateAllocations,textures:lateTextures,ready:retiredMedia.ready,hasTexture:!!retiredMedia.texture,current:coaxial.renderMediaObjects().length,binding:coaxial.renderBindingState()})"); if (after.closed) break; await Bun.sleep(50); } while (Date.now() < completed);
  const changes = [...inputs].filter(([p, h]) => sha(readFileSync(p)) !== h).map(([p]) => p);
  const passed = replacement.failed && stopped.stopped && !stopped.coherent && after.decoded === 1 && after.closed === 1 && after.allocations === 0 && after.textures === 0 && !after.ready && !after.hasTexture && after.current === 0 && after.binding.stopped && !after.binding.coherent && changes.length === 0;
  const receipt = { schema: "aethyr.render.media-retirement.v1", status: passed ? "passed" : "failed", fault, browser, waiting, replacement, stopped, after, inputs: Object.fromEntries(inputs), source_changes: changes, requests, events };
  writeFileSync(`${out}/qualification.json`, JSON.stringify(receipt, null, 2) + "\n"); console.log(JSON.stringify({ status: receipt.status, fault, waiting, replacement, after }));
  if (!passed) throw Error("retired media completion escaped its owner");
} catch (error) { writeFileSync(`${out}/failure.json`, JSON.stringify({ error: String(error), requests, events }, null, 2) + "\n"); throw error; }
finally { ws?.close(); server.stop(true); chrome.proc.kill(); await chrome.proc.exited; }
