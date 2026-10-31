// Real host + Wasm + WebGPU: the engine-ready signal must not be mistaken for
// a submitted and completed scene. Assets are delayed by this local server.
import { afterAll, beforeAll, expect, test } from 'bun:test'
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { launch as launchChrome } from "./chrome"

const root = join(import.meta.dir, '../..'), temp = mkdtempSync(join(tmpdir(), 'coax-scene-ready-'))
let server: ReturnType<typeof Bun.serve>, chrome: ReturnType<typeof Bun.spawn>, ws: WebSocket
let serial = 0, gotIndex = false, pendingIndex: ((r: Response) => void) | undefined
let holdShaders = false, shaderWaiters: ((r: Response) => void)[] = []
const pending = new Map<number, (v: any) => void>()
function cdp(method: string, params: any = {}) {
  return new Promise<any>((resolve, reject) => {
    const id = ++serial, timeout = setTimeout(() => { pending.delete(id); reject(Error('CDP timeout ' + method)) }, 30000)
    pending.set(id, reply => { clearTimeout(timeout); reply.error ? reject(Error(JSON.stringify(reply.error))) : resolve(reply.result) })
    ws.send(JSON.stringify({ id, method, params }))
  })
}
async function evaluate(source: string) {
  const result = await cdp('Runtime.evaluate', { expression: source, awaitPromise: true, returnByValue: true })
  if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails))
  return result.result.value
}
async function until(test: () => Promise<boolean>, message: string) {
  const deadline = Date.now() + 45000
  while (Date.now() < deadline) { if (await test()) return; await Bun.sleep(50) }
  throw Error(message)
}
const face = `#include <stdint.h>
#define E(n) __attribute__((export_name(n)))
static uint32_t draws[4]={0,MODE==2 ? 0 : 3,0,MODE==3 ? 0 : 1}; static int arrived;
E("coaxial_layout") int layout(void){return 1;}
E("coaxial_state_size") int size(void){return 16;}
E("coaxial_vertex_floats") int vf(void){return 4;}
E("coaxial_max_vertices") int mv(void){return 6;}
E("coaxial_uniform_floats") int uf(void){return 4;}
E("coaxial_frame") int frame(int a,int b,int c,int w,int h,float p,float dt,int s){return 0;}
E("coaxial_draws") uintptr_t list(int s){return (uintptr_t)draws;}
E("coaxial_draw_count") int count(int s){return arrived && MODE!=1 ? 1 : 0;}
E("coaxial_asset_ready") void asset(int s,int i,int n){if(i==0 && n>0)arrived=1;}
`
const shader = `@vertex fn bg_vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4f {let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3));return vec4f(p[i],0,1);}
@fragment fn bg_fs()->@location(0) vec4f{return vec4f(0,0,0,1);}
@vertex fn fx_vs(@location(0) p:vec4f)->@builtin(position) vec4f{return p;}
@fragment fn fx_fs()->@location(0) vec4f{return vec4f(1);}
@vertex fn scene_vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4f {let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3));return vec4f(p[i],0.5,1);}
@fragment fn scene_fs()->@location(0) vec4f{return vec4f(1,1,1,1);}`
const indexBytes = new Uint8Array(new Uint32Array([0,1,2]).buffer)
beforeAll(async () => {
  writeFileSync(join(temp, 'face.c'), face)
  for (const mode of [0,1,2,3]) {
    const compile = Bun.spawnSync(['/opt/homebrew/opt/llvm/bin/clang', '--target=wasm32', '-nostdlib', '-std=c11', '-O1', `-DMODE=${mode}`, join(temp, 'face.c'), '-o', join(temp, `scene-${mode}.wasm`), '-Wl,--no-entry,--import-memory,--max-memory=67108864,--export=__heap_base,--allow-undefined'], { stdout: 'pipe', stderr: 'pipe' })
    if (compile.exitCode) throw Error(new TextDecoder().decode(compile.stderr))
  }
  server = Bun.serve({ port: 0, fetch(req) {
    const url = new URL(req.url), path = url.pathname
    if (path === '/host.js') return new Response(readFileSync(join(root, 'coax/web/host.js'), 'utf8') + `
      globalThis.sceneProbe = () => ({gpuReady:!!gpu?.bg, group:!!gpu?.sceneGroup, pending:pendingSceneEpoch, epoch:sceneEpoch});
      globalThis.heldGpuPromises=[];
      globalThis.holdGpu = () => { const q=gpu.device.queue, original=q.onSubmittedWorkDone.bind(q); q.onSubmittedWorkDone=()=>new Promise((resolve,reject)=>heldGpuPromises.push(()=>original().then(resolve,reject))); };
      globalThis.requestTestEngine=()=>beginEngine(); globalThis.reloadTestEngine=(bytes,generation)=>loadEngine(bytes,generation);
      globalThis.reloadTestShader=()=>loadShaders();
      globalThis.holdParticleBuild=()=>{const original=particlePipeline; particlePipeline=post=>new Promise((resolve,reject)=>{globalThis.releaseParticleBuild=()=>original(post).then(resolve,reject)})};
    `, { headers: { 'Content-Type': 'text/javascript' } })
    if (['/entropy.js','/entropy-import.js','/drop.js','/uploads.js','/voice.js','/gpui.js','/loader.js','/http-requests.js'].includes(path)) return new Response(readFileSync(join(root, 'coax/web', path.slice(1))), { headers: { 'Content-Type': 'text/javascript' } })
    if (/^\/scene-[0-3]\.wasm$/.test(path)) return new Response(readFileSync(join(temp, path.slice(1))), { headers: { 'Content-Type': 'application/wasm' } })
    if (path === '/scene.wgsl') return holdShaders ? new Promise<Response>(resolve => { shaderWaiters.push(resolve) }) : new Response(shader)
    if (path === '/indices') { gotIndex = true; return new Promise<Response>(resolve => { pendingIndex = resolve }) }
    if (path === '/missing') return new Response('not found', { status: 404 })
    return new Response(`<!doctype html><html><body><canvas id="coaxial"></canvas><script type="module" src="/host.js" data-app="scene" data-wasm="/scene-${url.searchParams.has('empty') ? 1 : url.searchParams.has('zeroindex') ? 2 : url.searchParams.has('zeroinstance') ? 3 : 0}.wasm" data-assets="index:${url.searchParams.has('missing') ? '/missing' : '/indices'}"></script></body></html>`, { headers: { 'Content-Type': 'text/html' } })
  } })
  chrome = launchChrome(join(temp, 'chrome'))
  let port = ''
  for (let i = 0; i < 200; i++) { try { port = readFileSync(join(temp, 'chrome/DevToolsActivePort'), 'utf8').split('\n')[0]; break } catch {} await Bun.sleep(50) }
  if (!port) throw Error('Chrome did not open a debugging port')
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json() as any[]
  ws = new WebSocket(targets.find(t => t.type === 'page').webSocketDebuggerUrl)
  await new Promise(resolve => ws.addEventListener('open', resolve, { once: true }))
  ws.addEventListener('message', e => { const r = JSON.parse(String(e.data)); pending.get(r.id)?.(r); pending.delete(r.id) })
  await cdp('Runtime.enable'); await cdp('Page.enable')
}, 90000)
afterAll(async () => { pendingIndex?.(new Response(indexBytes)); for (const reply of shaderWaiters) reply(new Response(shader)); ws?.close(); chrome?.kill(); if (chrome) await chrome.exited; server?.stop(true); rmSync(temp, { recursive: true, force: true }) })
async function navigate(query = '') {
  gotIndex = false; pendingIndex = undefined; holdShaders = false; shaderWaiters = []
  await cdp('Page.navigate', { url: `http://127.0.0.1:${server.port}/${query}` })
  await until(async () => !!await evaluate('globalThis.coaxial?.ready'), 'engine did not load')
}
test('engine readiness does not imply a scene; completion waits for asset and GPU work', async () => {
  await navigate()
  await until(async () => gotIndex && !!await evaluate('sceneProbe().gpuReady'), 'index request or GPU unavailable')
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  await evaluate('holdGpu()')
  pendingIndex!(new Response(indexBytes)); pendingIndex = undefined
  await until(async () => !!await evaluate('sceneProbe().group && heldGpuPromises.length === 1'), 'scene frame was not submitted')
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  await evaluate('heldGpuPromises.shift()()')
  await until(async () => !!await evaluate('coaxial.sceneReady'), 'scene completion not observed')
  await evaluate('requestTestEngine()'); expect(await evaluate('coaxial.sceneReady')).toBe(false)
  await Bun.sleep(300); expect(await evaluate('coaxial.sceneReady')).toBe(false)
}, 90000)
test('missing asset or an empty draw cannot signal a scene', async () => {
  await navigate('?missing')
  await until(async () => !!await evaluate('sceneProbe().gpuReady'), 'GPU unavailable')
  await Bun.sleep(500); expect(await evaluate('coaxial.sceneReady')).toBe(false)
  for (const query of ['?empty', '?zeroindex', '?zeroinstance']) {
    await navigate(query)
    await until(async () => gotIndex, 'index not requested')
    pendingIndex!(new Response(indexBytes)); pendingIndex = undefined
    await until(async () => !!await evaluate('sceneProbe().group'), 'scene group not created')
    await Bun.sleep(500); expect(await evaluate('coaxial.sceneReady')).toBe(false)
  }
}, 90000)
test('old GPU completion cannot mark a new engine generation ready', async () => {
  await navigate()
  await until(async () => gotIndex && !!await evaluate('sceneProbe().gpuReady'), 'index request or GPU unavailable')
  await evaluate('holdGpu()')
  pendingIndex!(new Response(indexBytes)); pendingIndex = undefined
  await until(async () => !!await evaluate('heldGpuPromises.length === 1'), 'first frame not submitted')
  // A newer request may wait arbitrarily before the new bytes are compiled.
  // Resolving an old frame during that window must not restore readiness.
  const generation = await evaluate('requestTestEngine()')
  await evaluate('heldGpuPromises.shift()()'); await Bun.sleep(500)
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  await evaluate(`(async()=>{const b=await fetch('/scene-1.wasm').then(r=>r.arrayBuffer()); await reloadTestEngine(b,${generation});})()`)
  await Bun.sleep(300); expect(await evaluate('coaxial.sceneReady')).toBe(false)
}, 90000)

test('a pending shader swap invalidates readiness and an older frame cannot restore it', async () => {
  await navigate()
  await until(async () => gotIndex && !!await evaluate('sceneProbe().gpuReady'), 'index request or GPU unavailable')
  await evaluate('holdGpu()')
  pendingIndex!(new Response(indexBytes)); pendingIndex = undefined
  await until(async () => !!await evaluate('heldGpuPromises.length === 1'), 'old shader frame not submitted')
  holdShaders = true; await evaluate('void (globalThis.shaderReload = reloadTestShader())')
  await until(async () => shaderWaiters.length === 1, 'shader reload not requested')
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  await evaluate('heldGpuPromises.shift()()'); await Bun.sleep(300)
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  shaderWaiters.shift()!(new Response(shader)); holdShaders = false
  expect(await evaluate('shaderReload')).toBe(true)
  await until(async () => !!await evaluate('heldGpuPromises.length === 1'), 'new shader frame not submitted')
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  await evaluate('heldGpuPromises.shift()()')
  await until(async () => !!await evaluate('coaxial.sceneReady'), 'new shader scene not completed')
}, 90000)
test('overlapping shader loads must all settle before a fresh scene can be ready', async () => {
  await navigate()
  await until(async () => gotIndex, 'index not requested')
  pendingIndex!(new Response(indexBytes)); pendingIndex = undefined
  await until(async () => !!await evaluate('coaxial.sceneReady'), 'initial scene not completed')
  holdShaders = true
  await evaluate('void (globalThis.oldShader = reloadTestShader()); void (globalThis.newShader = reloadTestShader())')
  await until(async () => shaderWaiters.length === 2, 'both shader reloads not requested')
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  shaderWaiters[1](new Response(shader)); expect(await evaluate('newShader')).toBe(true)
  await Bun.sleep(300); expect(await evaluate('coaxial.sceneReady')).toBe(false)
  shaderWaiters[0](new Response(shader)); holdShaders = false; shaderWaiters = []
  expect(await evaluate('oldShader')).toBe(true)
  await until(async () => !!await evaluate('coaxial.sceneReady'), 'settled scene not completed')
}, 90000)

test('engine replacement during a shader load still requires a fresh settled frame', async () => {
  await navigate()
  await until(async () => gotIndex, 'index not requested')
  pendingIndex!(new Response(indexBytes)); pendingIndex = undefined
  await until(async () => !!await evaluate('coaxial.sceneReady'), 'initial scene not completed')
  holdShaders = true
  await evaluate('void (globalThis.delayedShader = reloadTestShader())')
  await until(async () => shaderWaiters.length === 1, 'shader reload not requested')
  const generation = await evaluate('requestTestEngine()')
  await evaluate(`(async()=>{const b=await fetch('/scene-0.wasm').then(r=>r.arrayBuffer()); await reloadTestEngine(b,${generation});})()`)
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  holdShaders = false; shaderWaiters.shift()!(new Response(shader))
  expect(await evaluate('delayedShader')).toBe(true)
  await until(async () => !!await evaluate('coaxial.sceneReady'), 'replacement engine scene did not complete')
}, 90000)

test('a delayed particle pipeline cannot make a scene ready before shader settlement', async () => {
  await navigate()
  await until(async () => gotIndex, 'index not requested')
  pendingIndex!(new Response(indexBytes)); pendingIndex = undefined
  await until(async () => !!await evaluate('coaxial.sceneReady'), 'initial scene not completed')
  await evaluate('holdParticleBuild(); void (globalThis.particleShader = reloadTestShader())')
  await until(async () => !!await evaluate('typeof releaseParticleBuild === "function"'), 'particle build was not held')
  expect(await evaluate('coaxial.sceneReady')).toBe(false)
  const generation = await evaluate('requestTestEngine()')
  await evaluate(`(async()=>{const b=await fetch('/scene-0.wasm').then(r=>r.arrayBuffer()); await reloadTestEngine(b,${generation});})()`)
  await Bun.sleep(300); expect(await evaluate('coaxial.sceneReady')).toBe(false)
  await evaluate('releaseParticleBuild()'); expect(await evaluate('particleShader')).toBe(true)
  await until(async () => !!await evaluate('coaxial.sceneReady'), 'scene after particle build did not complete')
}, 90000)
