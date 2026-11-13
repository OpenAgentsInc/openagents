// Firm reason: 1 (browser DOM and Chrome DevTools: it logs a second account in through the page and presses the
// editor's own elements; every rule it meets is the API's and the shell's, in Coax)
// The Scripts pane's acceptance run in a maker's own realm (#13496; docs/editor/acceptance/README.md, "Scripts in
// your own realm"). A new Nostr key logs in (a new account, no admin), makes a realm with one character, opens it
// with the editor, builds the starter script, runs it (asked first, then run with its yes) and waits until the
// character comes to meet the player. Run it against a dev server started with the compile job:
//   OA_SCRIPT_JOB=$PWD/coax/web/script-job.sh coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9644 --fresh
//   ORIGIN=http://127.0.0.1:9644 SIGN=<the local API's work dir>/sign bun docs/editor/acceptance/realm-scripts-run.ts
// SIGN is the signer the local API builds (serve.sh prints the API's log; `sign` is beside it). It leaves
// realm-scripts-asked.png and realm-scripts-run.png in apps/openagents-web/build/.
import {writeFileSync, mkdtempSync} from 'node:fs'
import {join} from 'node:path'
import {tmpdir} from 'node:os'

const root = join(import.meta.dir, '../../..'), origin = process.env.ORIGIN!, sign = process.env.SIGN!
const keyFile = join(mkdtempSync(join(tmpdir(), 'realm-scripts-key-')), 'maker.key')
const out = process.env.OUT || join(root, 'apps/openagents-web/build')
const {startChrome} = await import(join(root, 'coax/web/chrome.ts'))

let serial = 0
const replies = new Map<number, (r: any) => void>(), logs: string[] = []
const {port} = await startChrome('realm-scripts-e2e-', ['--window-size=1280,800'])
const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === 'page')
const ws = new WebSocket(target.webSocketDebuggerUrl)
await new Promise(r => ws.addEventListener('open', r, {once: true}))
ws.addEventListener('message', e => {
  const r = JSON.parse(String(e.data))
  if (r.method === 'Runtime.exceptionThrown') logs.push('exception ' + JSON.stringify(r.params.exceptionDetails?.exception?.description ?? r.params))
  if (r.method === 'Runtime.consoleAPICalled') logs.push(r.params.type + ' ' + r.params.args.map((a: any) => a.value ?? a.description).join(' '))
  replies.get(r.id)?.(r); replies.delete(r.id)
})
function cdp(method: string, params: any = {}) {
  return new Promise<any>((resolve, reject) => {
    const id = ++serial, timer = setTimeout(() => { replies.delete(id); reject(Error('CDP timeout ' + method)) }, 60000)
    replies.set(id, r => { clearTimeout(timer); r.error ? reject(Error(JSON.stringify(r.error))) : resolve(r.result) })
    ws.send(JSON.stringify({id, method, params}))
  })
}
async function js(expression: string) {
  expression = `(() => { const D = document.querySelector('iframe')?.contentDocument ?? document; return ${expression} })()`
  const r = await cdp('Runtime.evaluate', {expression, awaitPromise: true, returnByValue: true})
  if (r.exceptionDetails) throw Error(JSON.stringify(r.exceptionDetails))
  return r.result.value
}
async function until(what: string, test: () => Promise<boolean>, ms = 30000) {
  const end = Date.now() + ms
  while (Date.now() < end) { try { if (await test()) return } catch {} await Bun.sleep(200) }
  let state = ''
  try { state = JSON.stringify(await js(`({href: location.href, has: typeof document.querySelector('iframe')?.contentWindow?.coaxial?.guardProbe, keys: 0, p: [0,1,64,65,2500,2501,2502].map(f => document.querySelector('iframe')?.contentWindow?.coaxial?.guardProbe?.(f))})`)) } catch (e) { state = String(e) }
  throw Error('timed out: ' + what + '\n' + state + '\n' + logs.slice(-15).join('\n'))
}
const step = (s: string) => console.log('ok  ' + s)
async function shot(name: string) {
  const r = await cdp('Page.captureScreenshot', {format: 'png'})
  writeFileSync(join(out, name), Buffer.from(r.data, 'base64')); console.log('    picture ' + join(out, name))
}
const probe = (f: number) => js(`document.querySelector('iframe')?.contentWindow?.coaxial?.guardProbe?.(${f}) ?? null`)

await cdp('Page.enable'); await cdp('Runtime.enable'); await cdp('Emulation.setFocusEmulationEnabled', {enabled: true})
await cdp('Page.navigate', {url: origin + '/api/auth/limits'})
await Bun.sleep(800)

// 1. A new Nostr key logs in: a new account, no admin.
const pk = new TextDecoder().decode(Bun.spawnSync([sign, '--new', keyFile]).stdout).trim()
const challenge = await js(`fetch('/api/auth/nostr/challenge',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json'},body:'{}'}).then(r=>r.json()).then(j=>j.challenge)`)
const body = JSON.stringify({challenge}), bodyFile = join(mkdtempSync(join(tmpdir(), 'e2e-')), 'login.json')
writeFileSync(bodyFile, body, {mode: 0o600})
const signed = Bun.spawnSync([sign, '--key', keyFile, 'POST', origin + '/api/auth/nostr', bodyFile], {stdout: 'pipe', stderr: 'pipe'})
if (signed.exitCode !== 0) throw Error('sign failed: ' + new TextDecoder().decode(signed.stderr))
const auth = new TextDecoder().decode(signed.stdout).trim()
const login = await js(`fetch('/api/auth/nostr',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json',Authorization:${JSON.stringify('Nostr ' + auth)}},body:${JSON.stringify(body)}}).then(async r=>[r.status, await r.text()])`)
if (login[0] !== 200) throw Error('login refused ' + login[0])
const viewer = await js(`fetch('/api/auth/viewer',{credentials:'same-origin'}).then(r=>r.text())`)
const admin = /(^|\n)A\|1(\n|$)/.test(viewer)
if (admin) throw Error('the test account is an admin')
step(`logged in with a new key ${pk.slice(0, 8)}…, not an admin`)

// 2. Its player key, and a realm of its own with a character.
const player = await js(`fetch('/api/playtest/player',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json'},body:'{}'}).then(r=>r.json())`)
if (!player?.key) throw Error('no player key: ' + JSON.stringify(player))
await js(`localStorage.setItem('verse.player', ${JSON.stringify(JSON.stringify(player))})`)
const board = {board: {name: 'Greeter Glade ' + Date.now(), features: [{feature: 'character', name: 'Mott', role: 'the ferryman'}]}}
const made = await js(`fetch('/api/playtest/realms',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json','X-Verse-Player':${JSON.stringify(player.key)}},body:${JSON.stringify(JSON.stringify(board))}}).then(async r=>[r.status, await r.text()])`)
if (made[0] !== 200) throw Error('realm not made: ' + made.join(' '))
const slug = JSON.parse(made[1]).slug
step(`made the realm ${slug} with one character`)

// 3. Its world, as its maker: the scripts list answers (the Scripts button's condition).
const list = await js(`fetch('/api/worlds/${slug}/scripts',{credentials:'same-origin',headers:{'X-Verse-Player':${JSON.stringify(player.key)}}}).then(r=>r.status)`)
if (list !== 200) throw Error('the maker gets no script list: ' + list)
step('the maker reads the realm\'s script list (200)')

// 4. The realm's page with the editor.
// A dev server has no page at /worlds/SLUG (production writes one to the site's bucket): the made realm opens at
// /worlds/built#SLUG there. It is held in a frame of this origin's, as the app shell holds /worlds/SLUG?edit.
await js(`(() => { document.documentElement.innerHTML = '<body style="margin:0"></body>'; const f = document.createElement('iframe'); f.style.cssText = 'border:0;width:1280px;height:800px'; f.src = '/worlds/built?edit#${slug}'; document.body.appendChild(f); return true })()`)
await Bun.sleep(500)
await until('the realm laid out with its character', async () => (await probe(0)) === 1 && (await probe(65)) === 1 && (await probe(64)) > 0, 60000)
await until('the page is the owner\'s', async () => (await probe(1)) === 1, 20000)
await until('the Scripts button', async () => js(`!!D.getElementById('scripts')`), 30000)
const home = [await probe(1700), await probe(1701)], you = [await probe(10), await probe(11)]
const dist = async () => Math.hypot((await probe(1700)) - (await probe(10)), (await probe(1701)) - (await probe(11))) / 100
const d0 = await dist()
step(`the editor opened with Scripts; the character stands ${d0.toFixed(2)} m from you at (${home}) cm, you at (${you}) cm`)
if (await probe(2501) !== 0) throw Error('a script row before any run')
await Bun.sleep(1500)
if (Math.abs((await dist()) - d0) > 0.01) throw Error('the character moved with no script')
step('with no script the character keeps its place')

await js(`D.getElementById('scripts').click()`)
await until('the pane', async () => js(`!!D.getElementById('scripts.build')`), 10000)
// the starter is the greeter on a world with one lane
await until('the starter source', async () => js(`(D.getElementById('scripts.text')?.value ?? '').includes('#script "my-greeter"')`), 10000)
await js(`D.getElementById('scripts.build').click()`)
await until('built', async () => js(`!!D.getElementById('scripts.run')`), 90000)
step('built the starter script (my-greeter) on the server\'s compile job')

await js(`D.getElementById('scripts.run').click()`)
await until('asked first', async () => js(`!!D.getElementById('scripts.confirm')`), 20000)
const asked = await js(`D.getElementById('scripts.grows')?.innerText ?? D.getElementById('scripts.pane').innerText`)
step('Run asked first: ' + asked.replace(/\s+/g, ' ').slice(0, 160))
await shot('realm-scripts-asked.png')
await js(`D.getElementById('scripts.confirm').click()`)
await until('the row reaches the page', async () => (await probe(2501)) === 1 && (await probe(2500)) >= 1, 30000)
step(`Run with these: the world is at revision ${await probe(2500)}, one script row`)
await until('the script is called', async () => (await probe(2502)) > 20, 30000)
await until('the character comes to meet you', async () => (await dist()) < 2.5, 30000)
const d1 = await dist()
step(`the script runs (${await probe(2502)} calls): the character walked from ${d0.toFixed(2)} m to ${d1.toFixed(2)} m of you`)
await shot('realm-scripts-run.png')
const pane1 = await js(`D.getElementById('scripts.pane').innerText`)
console.log('    pane: ' + pane1.replace(/\s+/g, ' ').slice(0, 400))
const errs = logs.filter(l => l.startsWith('exception') || l.startsWith('error'))
if (errs.length) console.log('    page errors:\n' + errs.slice(-10).join('\n'))
console.log('PASS')
process.exit(0)
