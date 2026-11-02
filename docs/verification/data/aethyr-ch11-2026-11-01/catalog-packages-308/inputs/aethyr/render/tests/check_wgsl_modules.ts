// Compile every distinct emitted package module in muted headless WebGPU.
// This is a shader-validity gate; host execution and image parity are separate.
import { startChrome } from "../../../coax/web/chrome.ts";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve, basename } from "node:path";
import { createHash } from "node:crypto";

const [packageRoot, output] = process.argv.slice(2);
if (!packageRoot || !output) throw Error("usage: check_wgsl_modules.ts PACKAGE_PARENT RESULT.json");
const sha = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const digest = (path: string) => sha(readFileSync(path));
const snapshot = new URL("./catalog-6445ee0050.json", import.meta.url).pathname;
const profiles = JSON.parse(readFileSync(snapshot, "utf8")).profiles;
const inputs: Record<string, string> = {};
for (const path of [import.meta.path, snapshot, new URL("../../../coax/web/chrome.ts", import.meta.url).pathname]) inputs[path] = digest(path);
const modules = new Map<string, { hash: string, code: string, owners: { profile: string, binding_set: number }[] }>();
let declared = 0;
for (const profile of profiles) {
  const directory = resolve(packageRoot, profile.slug), path = join(directory, "render.json");
  inputs[path] = digest(path);
  const manifest = JSON.parse(readFileSync(path, "utf8"));
  if (manifest.schema !== "aethyr.render.package.v1" || manifest.slug !== profile.slug) throw Error("package identity differs");
  for (const row of manifest.modules) {
    if (basename(row.wgsl_file) !== row.wgsl_file) throw Error("unsafe sibling path");
    const source = join(directory, row.wgsl_file), bytes = readFileSync(source), hash = sha(bytes);
    if (hash !== row.wgsl_sha256) throw Error("module bytes differ from package hash");
    inputs[source] = hash;
    let module = modules.get(hash);
    if (!module) modules.set(hash, module = { hash, code: bytes.toString("utf8"), owners: [] });
    module.owners.push({ profile: profile.slug, binding_set: row.binding_set }); declared++;
  }
}
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response("<!doctype html><title>package shader checks</title>", { headers: { "content-type": "text/html" } }) });
const chrome = await startChrome("aethyr-modules-", [], 1);
let socket: WebSocket | undefined;
try {
  const pages = await (await fetch(`http://127.0.0.1:${chrome.port}/json/list`)).json();
  socket = new WebSocket(pages.find((page: any) => page.type === "page").webSocketDebuggerUrl);
  const pending = new Map<number, { resolve: (value: any) => void, reject: (error: any) => void, timer: ReturnType<typeof setTimeout> }>();
  let serial = 0;
  socket.onmessage = event => { const reply = JSON.parse(String(event.data)), call = pending.get(reply.id); if (!call) return; clearTimeout(call.timer); pending.delete(reply.id); reply.error ? call.reject(reply.error) : call.resolve(reply.result); };
  await new Promise<void>((done, fail) => { socket!.onopen = () => done(); socket!.onerror = fail; });
  const call = (method: string, params: any = {}) => new Promise<any>((done, fail) => { const id = ++serial, timer = setTimeout(() => { pending.delete(id); fail(Error(`${method} timed out`)); }, 60000); pending.set(id, { resolve: done, reject: fail, timer }); socket!.send(JSON.stringify({ id, method, params })); });
  const browser = await call("Browser.getVersion");
  await call("Page.enable"); await call("Page.navigate", { url: `http://127.0.0.1:${server.port}/` }); await Bun.sleep(250);
  const expression = `(async () => {
    const adapter=await navigator.gpu.requestAdapter(); if(!adapter) throw Error("no WebGPU adapter");
    const device=await adapter.requestDevice(),uncaptured=[],observations=[];
    device.addEventListener("uncapturederror",event=>uncaptured.push(event.error.message));
    for(const row of ${JSON.stringify([...modules.values()])}){
      device.pushErrorScope("validation"); const module=device.createShaderModule({label:row.hash,code:row.code});
      const info=await module.getCompilationInfo(),error=await device.popErrorScope();
      observations.push({hash:row.hash,owners:row.owners,messages:[...info.messages].map(m=>({type:m.type,message:m.message,line:m.lineNum,column:m.linePos})),validation_error:error?.message??null});
    }
    device.pushErrorScope("validation"); const planted=device.createShaderModule({code:"not a WGSL program"});
    const rejected=await planted.getCompilationInfo(),refusal=await device.popErrorScope(),info=adapter.info;
    device.destroy();return {device:{vendor:info.vendor,architecture:info.architecture,device:info.device,description:info.description},observations,uncaptured,control:{errors:[...rejected.messages].filter(m=>m.type==="error").length,validation_error:refusal?.message??null}};
  })()`;
  const response = await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (response.exceptionDetails) throw Error(JSON.stringify(response.exceptionDetails));
  const result = response.result.value;
  const changes = Object.keys(inputs).filter(path => digest(path) !== inputs[path]);
  const errors = result.observations.reduce((n: number, row: any) => n + row.messages.filter((m: any) => m.type === "error").length + Number(!!row.validation_error), 0);
  const warnings = result.observations.reduce((n: number, row: any) => n + row.messages.filter((m: any) => m.type === "warning").length, 0);
  const pass = !changes.length && !errors && !result.uncaptured.length && result.observations.length === modules.size && result.control.errors > 0 && !!result.control.validation_error;
  const receipt = { schema: "aethyr.render.wgsl-compilation.v1", status: pass ? "pass" : "fail", profiles: profiles.length, declared_modules: declared, unique_modules: modules.size, errors, warnings, browser, ...result, source_inputs: inputs, source_changes: changes,
    scope: "All distinct actual package WGSL bytes compiled by headless WebGPU; each package module retains its owner mapping. Includes a malformed-program refusal control. This does not certify pipelines, draw execution or image parity." };
  writeFileSync(output, JSON.stringify(receipt, null, 2) + "\n");
  if (!pass) throw Error("WGSL compilation qualification failed");
  console.log(JSON.stringify({ profiles: profiles.length, declared_modules: declared, unique_modules: modules.size, errors, warnings }));
} finally {
  socket?.close(); server.stop(true); chrome.proc.kill(); await chrome.proc.exited;
}
