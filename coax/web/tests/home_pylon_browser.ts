// Real homepage interactions in muted headless Chrome. Original Wasm, clock,
// sockets and data remain untouched. Only read-only observation exports are
// captured; input and clipboard/navigation use the browser's ordinary paths.
// bun coax/web/tests/home_pylon_browser.ts ORIGIN [--out /tmp/home-pylons]
//   [--job-api http://127.0.0.1:8798/v1/systemone]
// --job-api sends one synthetic toy decision after the desktop watch/panel
// is ready, then observes its answer-counter increase before leaving.
import {existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {chrome, gpuFlags} from '../chrome'

const args = process.argv.slice(2)
const base = args[0]?.replace(/\/$/, '')
const outAt = args.indexOf('--out')
const prefix = outAt < 0 ? '/tmp/home-pylons' : args[outAt + 1]
const jobAt = args.indexOf('--job-api')
const jobAPI = jobAt < 0 ? undefined : args[jobAt + 1]
if (!base?.startsWith('http') || !prefix || (jobAt >= 0 && !jobAPI?.startsWith('http'))) throw Error('usage: home_pylon_browser.ts ORIGIN [--out PREFIX] [--job-api URL]')
if (jobAPI) new URL(jobAPI)
const origin = new URL(base).origin
const joinCommand = 'curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh && bash /tmp/join-pylon.sh --model elephant-1 --self-test'
const profile = mkdtempSync(join(tmpdir(), 'coax-home-pylons-'))
const proc = Bun.spawn([chrome(), '--headless=new', '--mute-audio', `--user-data-dir=${profile}`,
  '--no-first-run', '--hide-scrollbars', ...gpuFlags(), '--window-size=1440,900',
  '--remote-debugging-port=0', 'about:blank'], {stdout:'ignore', stderr:'ignore'})
let ws:WebSocket | undefined
const evidence:any = {base, at:new Date().toISOString(), job_api:jobAPI ?? null, scenarios:[]}

try {
  const portFile = join(profile, 'DevToolsActivePort')
  for (let i = 0; i < 200 && !existsSync(portFile); i++) await Bun.sleep(50)
  const port = readFileSync(portFile, 'utf8').split('\n')[0]
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
  const target = targets.find((t:any) => t.type === 'page')
  ws = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise<void>((resolve, reject) => {
    ws!.addEventListener('open', () => resolve(), {once:true})
    ws!.addEventListener('error', () => reject(Error('Chrome debugger connection failed')), {once:true})
  })
  let serial = 0, recording:any
  const pending = new Map<number, {resolve:(v:any) => void, reject:(e:any) => void, timer:ReturnType<typeof setTimeout>}>()
  ws.addEventListener('message', event => {
    const message = JSON.parse(String(event.data))
    const request = pending.get(message.id)
    if (request) {
      pending.delete(message.id)
      clearTimeout(request.timer)
      if (message.error) request.reject(Error(JSON.stringify(message.error)))
      else request.resolve(message.result)
    }
    if (!recording) return
    if (message.method === 'Runtime.exceptionThrown') recording.errors.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text)
    if (message.method === 'Network.webSocketCreated') recording.sockets++
    if (message.method === 'Network.webSocketFrameReceived') recording.received_frames++
    if (message.method === 'Network.webSocketFrameSent') recording.sent_frames++
    if (message.method === 'Network.requestWillBeSent' && message.params.type === 'Document') recording.documents.push(message.params.request.url)
    if (message.method === 'Network.responseReceived' && message.params.type === 'Document') recording.document_responses.push({url:message.params.response.url, status:message.params.response.status})
  })
  function send(method:string, params:any = {}) {
    return new Promise<any>((resolve, reject) => {
      const id = ++serial
      const timer = setTimeout(() => {pending.delete(id); reject(Error('Chrome debugger timed out: ' + method))}, 20000)
      pending.set(id, {resolve, reject, timer})
      ws!.send(JSON.stringify({id, method, params}))
    })
  }
  async function evaluate(expression:string) {
    const result = await send('Runtime.evaluate', {expression, returnByValue:true, awaitPromise:true, userGesture:true})
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text)
    return result.result?.value
  }
  async function until(predicate:() => Promise<boolean>, description:string, timeout = 10000) {
    const start = Date.now()
    while (Date.now() - start < timeout) {
      if (recording?.errors.length) throw Error(recording.errors.join('\n'))
      if (await predicate()) return
      await Bun.sleep(100)
    }
    throw Error(description + ' timed out')
  }
  const observe = () => evaluate('globalThis.homePylonObservation?.()')
  await send('Runtime.enable')
  await send('Page.enable')
  await send('Network.enable')
  await send('Emulation.setFocusEmulationEnabled', {enabled:true})
  await send('Browser.grantPermissions', {origin, permissions:['clipboardReadWrite', 'clipboardSanitizedWrite']})
  await send('Page.addScriptToEvaluateOnNewDocument', {source:`
    const originalInstantiate = WebAssembly.instantiate;
    WebAssembly.instantiate = async function(module, imports) {
      const result = await originalInstantiate.call(WebAssembly, module, imports);
      const x = (result.instance || result).exports;
      if (x.coaxial_land_pylon_control) {
        const memory = imports.env.memory;
        const state = Math.ceil(Math.max(Number(x.__heap_base.value), memory.buffer.byteLength) / 65536) * 65536;
        globalThis.homePylonObservation = () => {
          if (globalThis.coaxial?.ready !== true) return {ready:false};
          const count = x.coaxial_land_pylons(state);
          return {
            ready:true, sceneReady:globalThis.coaxial?.sceneReady === true,
            probe:globalThis.coaxial?.probe?.() ?? 0,
            ready_projects:x.coaxial_land_ready_projects(state),
            beacons:x.coaxial_land_beacons_received(state),
            refreshes:x.coaxial_land_refreshes(state), refused:x.coaxial_land_refused(state),
            count, online:x.coaxial_land_network_online(state), free:x.coaxial_land_network_free(state),
            open:x.coaxial_land_network_open(state), scroll:x.coaxial_land_network_scroll(state),
            controls:Array.from({length:4}, (_, which) => Array.from({length:4}, (_, axis) => x.coaxial_land_pylon_control(state, which, axis))),
            rows:Array.from({length:count}, (_, i) => ({status:x.coaxial_land_pylon_status(state, i),
              free:x.coaxial_land_pylon_free(state, i), answers:x.coaxial_land_pylon_answers(state, i),
              updates:x.coaxial_land_pylon_updates(state, i)}))
          };
        };
      }
      return result;
    };`})
  for (const viewport of [{name:'desktop', width:1440, height:900, touch:false}, {name:'phone', width:390, height:844, touch:true}]) {
    const start = Date.now()
    recording = {viewport, errors:[], sockets:0, received_frames:0, sent_frames:0, documents:[], document_responses:[]}
    evidence.scenarios.push(recording)
    await send('Emulation.setDeviceMetricsOverride', {width:viewport.width, height:viewport.height, deviceScaleFactor:1, mobile:viewport.touch})
    await send('Emulation.setTouchEmulationEnabled', {enabled:viewport.touch, maxTouchPoints:1})
    await send('Page.navigate', {url:base + '/'})
    await until(async () => {
      const state = await observe()
      return state?.ready === true && (state.probe & 255) === 255
    }, viewport.name + ' live homepage readiness', 90000)
    recording.home = await observe()
    recording.browser = await evaluate('({width:innerWidth,height:innerHeight,dpr:devicePixelRatio})')
    if (recording.home.open !== 0) throw Error(viewport.name + ' opened with the Pylon panel already visible')
    if (recording.browser.width !== viewport.width || recording.browser.height !== viewport.height) throw Error('unexpected browser viewport: ' + JSON.stringify(recording.browser))
    async function shot(kind:string) {
      const path = `${prefix}-${viewport.name}-${kind}.png`
      const capture = await send('Page.captureScreenshot', {format:'png'})
      writeFileSync(path, Buffer.from(capture.data, 'base64'))
      return path
    }
    async function click(which:number) {
      const state = await observe(), box = state.controls[which]
      if (!box.every(Number.isFinite) || box[2] <= box[0] || box[3] <= box[1]) throw Error('missing control rectangle ' + which)
      const x = (box[0] + box[2]) / 2, y = (box[1] + box[3]) / 2
      if (viewport.touch) {
        await send('Input.dispatchTouchEvent', {type:'touchStart', touchPoints:[{x, y, id:1, radiusX:1, radiusY:1, force:1}]})
        await send('Input.dispatchTouchEvent', {type:'touchEnd', touchPoints:[]})
      } else {
        await send('Input.dispatchMouseEvent', {type:'mousePressed', x, y, button:'left', buttons:1, clickCount:1})
        await send('Input.dispatchMouseEvent', {type:'mouseReleased', x, y, button:'left', buttons:0, clickCount:1})
      }
    }
    recording.home_screenshot = await shot('home')
    await click(0)
    await until(async () => (await observe())?.open === 1, viewport.name + ' Pylon panel open')
    await Bun.sleep(750)
    recording.panel = await observe()
    recording.panel_screenshot = await shot('panel')
    console.log('PANEL ' + JSON.stringify({viewport:viewport.name, screenshot:recording.panel_screenshot, probe:recording.panel.probe,
      pylons:recording.panel.count, online:recording.panel.online, free:recording.panel.free, beacons:recording.panel.beacons, input:viewport.touch ? 'touch' : 'mouse'}))
    if (jobAPI && viewport.name === 'desktop') {
      const before = await observe()
      const totalAnswers = (state:any) => state.rows.reduce((sum:number, row:any) => sum + row.answers, 0)
      const answersBefore = totalAnswers(before), started = Date.now()
      recording.job = {endpoint:jobAPI, answers_before:answersBefore, post_count:1}
      // This one request is issued by Bun through the existing operator API.
      // The page only receives its ordinary live signed relay observations.
      const response = await fetch(jobAPI, {
        method:'POST', redirect:'error', headers:{'content-type':'application/json'},
        body:JSON.stringify({model:'psionic-toy', state:'Water is wet.', questions:{wet:{type:'choice', criteria:{yes:null, no:null}}}}),
        signal:AbortSignal.timeout(60000),
      })
      recording.job.http_status = response.status
      if (response.status !== 200) throw Error('synthetic toy decision returned HTTP ' + response.status)
      const result = await response.json()
      recording.job.provider = result.service?.provider ?? null
      if (!/^[0-9a-f]{64}$/.test(recording.job.provider ?? '')) throw Error('synthetic toy decision did not identify a public Pylon provider')
      await until(async () => {
        const state = await observe()
        recording.job.answers_after = totalAnswers(state)
        recording.job.counter_delta = recording.job.answers_after - answersBefore
        return state.open === 1 && recording.job.counter_delta > 0
      }, 'desktop live answer-counter increase after synthetic toy decision', 60000)
      recording.job.elapsed_ms = Date.now() - started
      recording.job_screenshot = await shot('job')
      writeFileSync(prefix + '.json', JSON.stringify(evidence, null, 2) + '\n')
      console.log('JOB ' + JSON.stringify(recording.job))
    }
    await click(1)
    await until(async () => {
      // Never return or log unrelated clipboard contents if the write fails.
      recording.clipboard = await evaluate(`navigator.clipboard.readText().then(text => ({matches:text === ${JSON.stringify(joinCommand)},bytes:new TextEncoder().encode(text).length}), error => ({error:error.name}))`)
      return recording.clipboard?.matches === true && recording.clipboard.bytes === Buffer.byteLength(joinCommand)
    }, viewport.name + ' exact clipboard command')
    await click(2)
    await until(async () => await evaluate('location.pathname') === '/docs/connect-a-pylon', viewport.name + ' guide navigation')
    await until(async () => recording.document_responses.some((r:any) => new URL(r.url).pathname === '/docs/connect-a-pylon' && r.status === 200), viewport.name + ' guide HTTP response')
    recording.guide = await evaluate('location.href')
    recording.elapsed_ms = Date.now() - start
    if (!recording.sockets || !recording.sent_frames || !recording.received_frames) throw Error('homepage did not exchange real relay traffic')
    if (recording.errors.length) throw Error(recording.errors.join('\n'))
    writeFileSync(prefix + '.json', JSON.stringify(evidence, null, 2) + '\n')
    console.log('PASS ' + JSON.stringify({viewport:viewport.name, copied_bytes:recording.clipboard.bytes,
      guide:recording.guide, sockets:recording.sockets, received_frames:recording.received_frames,
      sent_frames:recording.sent_frames, errors:recording.errors, elapsed_ms:recording.elapsed_ms}))
  }
  evidence.passed = true
} catch (error) {
  evidence.passed = false
  evidence.failure = String(error)
  throw error
} finally {
  writeFileSync(prefix + '.json', JSON.stringify(evidence, null, 2) + '\n')
  ws?.close()
  proc.kill()
  await proc.exited
  rmSync(profile, {recursive:true, force:true})
}
