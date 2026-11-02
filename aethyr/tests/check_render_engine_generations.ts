// Real imported-memory writes distinguish obsolete engine generations.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve, extname } from "node:path";
import { createHash } from "node:crypto";
const [initialArg, alternateArg, packageArg, outArg, mode = "before"] = process.argv.slice(2);
if (!initialArg || !alternateArg || !packageArg || !outArg || !["before", "during", "overlap"].includes(mode)) throw Error("usage: check_render_engine_generations.ts INITIAL.wasm ALTERNATE.wasm PACKAGE OUT [before|during|overlap]");
const root = resolve(dirname(import.meta.path), "../.."), initial = resolve(initialArg), alternate = resolve(alternateArg), pack = resolve(packageArg), out = resolve(outArg);
mkdirSync(out, { recursive: true }); const inputs = new Map<string, string>(), requests: string[] = [], events: any[] = [], digest = (b: Uint8Array) => createHash("sha256").update(b).digest("hex");
let html = readFileSync(`${root}/apps/openagents-web/realms/hecates-cemetery/index.html`, "utf8");
html = html.replace(/data-render="[^"]*"/, 'data-render="/package/render.json"').replace(/data-app="[^"]*"/, '$& data-wasm="/test.wasm"');
for (const p of [initial, alternate, import.meta.path, `${root}/coax/web/host.js`, `${root}/coax/web/render.js`, `${pack}/render.json`]) inputs.set(p, digest(readFileSync(p)));
let engines = 0;
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch(req) {
  const path = new URL(req.url).pathname; requests.push(path);
  if (path === "/") return new Response(html, { headers: { "content-type": "text/html" } });
  if (path === "/api/auth/viewer") return new Response("");
  if (path === "/test.wasm") { engines++; if (engines === 3 && mode !== "overlap") return new Response("planted replacement fetch failure", { status: 404 }); }
  const file = path === "/test.wasm" ? engines === 2 ? alternate : initial : path.startsWith("/package/") ? resolve(pack, path.slice(9)) : path.startsWith("/coaxial/") ? resolve(root, "coax/web", path.slice(9)) : resolve(root, "apps/openagents-web", path.slice(1));
  try { const bytes = readFileSync(file), sha = digest(bytes); if (inputs.has(file) && inputs.get(file) !== sha) throw Error("source changed"); inputs.set(file, sha); return new Response(bytes, { headers: { "content-type": extname(file) === ".js" ? "text/javascript" : extname(file) === ".wasm" ? "application/wasm" : extname(file) === ".json" ? "application/json" : "application/octet-stream" } }); } catch { return new Response("not found", { status: 404 }); }
} });
const chrome = await startChrome("aethyr-engine-generations-", ["--disable-background-timer-throttling", "--disable-renderer-backgrounding"], 1); let ws: WebSocket | undefined;
try {
  const pages = await (await fetch(`http://127.0.0.1:${chrome.port}/json/list`)).json(); ws = new WebSocket(pages.find((p: any) => p.type === "page").webSocketDebuggerUrl);
  let serial = 0; const pending = new Map<number, any>();
  ws.onmessage = (e) => { const m = JSON.parse(String(e.data)); if (!m.id) { events.push(m); return; } const q = pending.get(m.id); if (!q) return; pending.delete(m.id); clearTimeout(q.timer); m.error ? q.reject(m.error) : q.resolve(m.result); };
  await new Promise<void>((r, j) => { ws!.onopen = () => r(); ws!.onerror = j; });
  const call = (method: string, params: any = {}) => new Promise<any>((resolve, reject) => { const id = ++serial, timer = setTimeout(() => { pending.delete(id); reject(Error(`${method} timeout`)); }, 120000); pending.set(id, { resolve, reject, timer }); ws!.send(JSON.stringify({ id, method, params })); });
  const evaluate = async (expression: string) => { const r = await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }); if (r.exceptionDetails) throw Error(JSON.stringify(r.exceptionDetails)); return r.result.value; };
  const browser = await call("Browser.getVersion"); await call("Page.enable"); await call("Runtime.enable"); await call("Emulation.setDeviceMetricsOverride", { width: 160, height: 120, deviceScaleFactor: 1, mobile: false });
  await call("Page.addScriptToEvaluateOnNewDocument", { source: "globalThis.coaxialClip=true;" }); await call("Page.navigate", { url: `http://127.0.0.1:${server.port}/?probe&golden&probe-manual` });
  let until = Date.now() + 60000;
  while (Date.now() < until) { await evaluate("globalThis.coaxialClipStep?.(1,60)"); if (await evaluate("!!globalThis.coaxial?.renderStatus")) break; await Bun.sleep(30); }
  for (const type of ["keyDown", "keyUp"]) await call("Input.dispatchKeyEvent", { type, key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 });
  while (Date.now() < until) { await evaluate("coaxialClipStep(1,60)"); if (await evaluate("coaxial.renderStatus?.submitted&&coaxial.renderStatus.readiness===2")) break; await Bun.sleep(30); }
  const before = await evaluate("({marker:coaxial.renderEngineMarker(),binding:coaxial.renderBindingState()})");
  if (before.marker !== 0x13579bdf || !before.binding.coherent) throw Error("initial engine is not the actual marked producer");
  await evaluate(`(()=>{globalThis.instantiations=0;globalThis.nativeInstantiations=0;globalThis.replacementPhase='idle';globalThis.activeInstantiations=0;globalThis.overlapping=false;let release;const gate=new Promise(r=>release=r);globalThis.releaseReplacement=release;const compile=WebAssembly.compile,instantiate=WebAssembly.instantiate;let hold=true;WebAssembly.compile=async(...args)=>{const module=await compile(...args);if(${mode === "before"}&&hold){hold=false;replacementPhase='held';await gate}return module};WebAssembly.instantiate=async(...args)=>{instantiations++;activeInstantiations++;if(activeInstantiations>1)overlapping=true;nativeInstantiations++;const result=await instantiate(...args);if(${mode !== "before"}&&hold){hold=false;replacementPhase='held';await gate}activeInstantiations--;return result};globalThis.firstReplacement=coaxial.reloadEngine().then(value=>({value}),error=>({error:error.message}))})()`);
  until = Date.now() + 60000; while (Date.now() < until) { if (await evaluate("replacementPhase==='held'")) break; await Bun.sleep(30); }
  const held = await evaluate("({phase:replacementPhase,marker:coaxial.renderEngineMarker(),binding:coaxial.renderBindingState(),nativeInstantiations})");
  if (held.phase !== "held") throw Error("replacement did not reach the controlled physical boundary");
  await evaluate("globalThis.secondReplacement=coaxial.reloadEngine().then(value=>({value}),error=>({error:error.message}));'started'");
  if (mode !== "overlap") await evaluate("secondReplacement"); else await Bun.sleep(500);
  await evaluate("releaseReplacement()"); const first = await evaluate("firstReplacement"), second = await evaluate("secondReplacement");
  await evaluate("coaxialClipStep(1,Infinity)"); const after = await evaluate("({marker:coaxial.renderEngineMarker(),binding:coaxial.renderBindingState(),nativeInstantiations,overlapping,render:coaxial.renderStatus})");
  const validation = await evaluate("coaxial.renderSettled()"); const changes = [...inputs].filter(([p, h]) => digest(readFileSync(p)) !== h).map(([p]) => p);
  const passed = changes.length === 0 && (mode === "before" ? first.value === false && !!second.error && after.nativeInstantiations === 0 && after.marker === 0x13579bdf && after.binding.coherent && validation?.ok : mode === "during" ? held.binding.stopped && first.value === false && !!second.error && after.binding.stopped && !after.binding.coherent && after.marker === null : held.binding.stopped && first.value === false && second.value === true && !after.overlapping && after.nativeInstantiations === 2 && after.marker === 0x13579bdf && after.binding.coherent && validation?.ok);
  const receipt = { schema: "aethyr.render.engine-generations.v1", status: passed ? "passed" : "failed", mode, browser, before, held, first, second, after, validation, inputs: Object.fromEntries(inputs), source_changes: changes, requests, events };
  writeFileSync(`${out}/qualification.json`, JSON.stringify(receipt, null, 2) + "\n"); console.log(JSON.stringify({ status: receipt.status, mode, before, held, first, second, after, validation })); if (!passed) throw Error("obsolete engine affected live shared memory or execution");
} catch (error) { writeFileSync(`${out}/failure.json`, JSON.stringify({ error: String(error), requests, events }, null, 2) + "\n"); throw error; }
finally { ws?.close(); server.stop(true); chrome.proc.kill(); await chrome.proc.exited; }
