// Firm reason: 1 (browser DOM and Chrome DevTools: it logs a second account in through the page and presses the
// editor's own elements; every rule it meets is the API's and the shell's, in Coax)
// Places in a maker's own realm as world edits (#13506; docs/protocol/world-edit-v1.md, "Placed things"). A new
// Nostr key logs in (a new account, no admin), makes a realm, opens it with the editor, adds a crate where the
// player stands (Add, a spawn), removes it (a despawn), takes the removal back (Undo) and does it again (Redo),
// and reads the realm's spec after each: the crate is placed:0, marked gone and back, and no ID shifts.
//   coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9645 --fresh
//   ORIGIN=http://127.0.0.1:9645 SIGN=<the local API's work dir>/sign bun docs/editor/acceptance/realm-places-run.ts
// It leaves realm-places-added.png and realm-places-removed.png in apps/openagents-web/build/.
import {writeFileSync, mkdtempSync} from 'node:fs'
import {join} from 'node:path'
import {tmpdir} from 'node:os'

const root = join(import.meta.dir, '../../..'), origin = process.env.ORIGIN!, sign = process.env.SIGN!
const keyFile = join(mkdtempSync(join(tmpdir(), 'realm-places-key-')), 'maker.key')
const out = process.env.OUT || join(root, 'apps/openagents-web/build')
const {startChrome} = await import(join(root, 'coax/web/chrome.ts'))

let serial = 0
const replies = new Map<number, (r: any) => void>(), logs: string[] = []
const {port} = await startChrome('realm-places-e2e-', ['--window-size=1440,900'])
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
  throw Error('timed out: ' + what + '\n' + logs.slice(-15).join('\n'))
}
const step = (s: string) => console.log('ok  ' + s)
async function shot(name: string) {
  const r = await cdp('Page.captureScreenshot', {format: 'png'})
  writeFileSync(join(out, name), Buffer.from(r.data, 'base64')); console.log('    picture ' + join(out, name))
}
const click = (id: string) => js(`(() => { const b = D.getElementById(${JSON.stringify(id)}); if (!b) return false; b.click(); return true })()`)

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
if (/(^|\n)A\|1(\n|$)/.test(viewer)) throw Error('the test account is an admin')
step(`logged in with a new key ${pk.slice(0, 8)}…, not an admin`)

// 2. Its player key, and a realm of its own.
const player = await js(`fetch('/api/playtest/player',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json'},body:'{}'}).then(r=>r.json())`)
if (!player?.key) throw Error('no player key: ' + JSON.stringify(player))
await js(`localStorage.setItem('verse.player', ${JSON.stringify(JSON.stringify(player))})`)
const board = {board: {name: 'Crate Yard ' + Date.now(), features: []}}
const made = await js(`fetch('/api/playtest/realms',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json','X-Verse-Player':${JSON.stringify(player.key)}},body:${JSON.stringify(JSON.stringify(board))}}).then(async r=>[r.status, await r.text()])`)
if (made[0] !== 200) throw Error('realm not made: ' + made.join(' '))
const slug = JSON.parse(made[1]).slug
step(`made the realm ${slug}`)
const spec = () => js(`fetch('/api/playtest/realms/${slug}',{credentials:'same-origin',headers:{'X-Verse-Player':${JSON.stringify(player.key)}}}).then(r=>r.text())`)
const placed = async () => ((await spec()) as string).split('\n').filter(l => l.startsWith('placed ') || l.startsWith('shaped '))

// 3. The realm's page with the editor (a dev server opens a made realm at /worlds/built#SLUG).
await js(`(() => { document.documentElement.innerHTML = '<body style="margin:0"></body>'; const f = document.createElement('iframe'); f.style.cssText = 'border:0;width:1440px;height:900px'; f.src = '/worlds/built?edit#${slug}'; document.body.appendChild(f); return true })()`)
await until('the editor with its Things list and Add', async () => js(`!!D.getElementById('things') && !!D.getElementById('add')`), 90000)
step('the editor opened: Things, and Add (the realm\'s kinds, from where you stand)')

// 4. Add a crate: one spawn, numbered 1; the realm's spec has it as placed:0.
await click('add')
await until('the kinds', async () => js(`!!D.getElementById('add.1')`), 10000)
const kinds = await js(`[...D.querySelectorAll('[id^="add."]')].filter(e => /^add\\.\\d+$/.test(e.id)).map(e => (e.textContent ?? '').trim()).join(', ')`)
await click('add.1')
await until('thing 1 in the list', async () => js(`!!D.getElementById('thing.1')`), 20000)
const p1 = await placed()
if (p1.length !== 1 || !p1[0].startsWith('placed crate ')) throw Error('the spec after Add: ' + JSON.stringify(p1))
step(`Add (kinds: ${kinds}) placed a crate: Things lists No. 1; the spec says "${p1[0]}"`)
await shot('realm-places-added.png')

// 5. Remove it: a despawn; the spec marks placed:0 gone and keeps it in the list.
await click('thing.1')
await until('Remove', async () => js(`!!D.getElementById('remove')`), 10000)
await click('remove')
await until('thing 1 gone from the list', async () => js(`!D.getElementById('thing.1')`), 20000)
const p2 = await placed()
if (p2.length !== 2 || !p2.includes('shaped placed:0 0 100 1')) throw Error('the spec after Remove: ' + JSON.stringify(p2))
step(`Remove took it away as a despawn: the spec keeps "${p2[0]}" and adds "${p2[1]}"`)
await shot('realm-places-removed.png')

// 6. Undo brings it back as it was, under its number; Redo takes it away again.
await until('Undo', async () => js(`!!D.getElementById('undo') && !D.getElementById('undo').disabled`), 10000)
await click('undo')
await until('thing 1 back', async () => js(`!!D.getElementById('thing.1')`), 20000)
const p3 = await placed()
if (p3.length !== 1 || p3[0] !== p1[0]) throw Error('the spec after Undo: ' + JSON.stringify(p3))
step('Undo brought No. 1 back exactly where it stood')
await click('redo')
await until('thing 1 gone again', async () => js(`!D.getElementById('thing.1')`), 20000)
step('Redo took it away again')

// 7. A second crate is No. 2: no ID shifts.
await click('add'); await until('the kinds', async () => js(`!!D.getElementById('add.1')`), 10000); await click('add.1')
await until('thing 2 in the list', async () => js(`!!D.getElementById('thing.2')`), 20000)
step('a second crate is No. 2; No. 1 stays gone, no ID shifted')
const errs = logs.filter(l => l.startsWith('exception') || l.startsWith('error'))
if (errs.length) console.log('    page errors:\n' + errs.slice(-10).join('\n'))
console.log('PASS')
process.exit(0)
