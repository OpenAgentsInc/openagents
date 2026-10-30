// The shipped homepage UI through its Wasm ABI. Public throwaway signed
// fixtures pass through the ordinary socket/C verification path; no relay,
// browser, clipboard, production key or admitted-state shortcut is involved.
import {expect, test} from 'bun:test'
import {readFileSync} from 'node:fs'
import {join} from 'node:path'

const root = join(import.meta.dir, '../../..')
const joinCommand = 'curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh && bash /tmp/join-pylon.sh --model elephant-1 --self-test'
const generated = Bun.spawnSync(['python3', '-c', `
import hashlib, json, sys
from pathlib import Path
sys.path.insert(0, str(Path(sys.argv[1])/'coax/web/tests'))
from newland_fixtures import fixtures, sign
f = fixtures()
f['many_beacons'] = []
for i in range(10):
    body = json.loads(f['beacon']['content'])
    body['pylon'] = 'ui-node-%02d' % i
    body['label'] = 'UI fixture %02d' % i
    content = json.dumps(body, sort_keys=True, separators=(',', ':'))
    tags = [['d', body['pylon']], ['t', 'oa:pylon-beacon:v1'],
            ['x', hashlib.sha256(content.encode()).hexdigest()],
            ['expiration', str(body['valid_until'])]]
    f['many_beacons'].append(sign(30200, tags, content, f['now']))
print(json.dumps(f, separators=(',', ':')))
`, root])
if (generated.exitCode) throw Error(generated.stderr.toString())
const fixture = JSON.parse(generated.stdout.toString())
type Viewport = {width:number, height:number, dpr:number}
type Box = [number, number, number, number]

async function homepage(viewport:Viewport) {
  const memory = new WebAssembly.Memory({initial:1024, maximum:1024})
  const decode = (at:number, n:number) => new TextDecoder().decode(new Uint8Array(memory.buffer, at, n))
  const encode = new TextEncoder()
  const pending:{id:number, kind:number, text:string}[] = []
  const sent:{id:number, value:any}[] = []
  let nextSocket = 0, now = fixture.now, x:any, state = 0
  const env = {
    memory,
    coaxial_http:(_u: number, _n: number, _b: number, _bn: number, flags: number) => flags & 4 ? -1 : 0,
    coaxial_storage_get: () => 2, coaxial_storage_put: () => 2,
    coaxial_now_seconds:() => BigInt(now),
    coaxial_socket_open:(at:number, n:number) => {
      expect(decode(at, n)).toBe('wss://relay.openagents.com')
      const id = nextSocket++
      pending.push({id, kind:0, text:''})
      return id
    },
    coaxial_socket_send:(id:number, at:number, n:number) => sent.push({id, value:JSON.parse(decode(at, n))}),
    coaxial_socket_close:(id:number) => pending.push({id, kind:2, text:''}),
  }
  const wasi = {fd_fdstat_get:() => 52, fd_seek:() => 52, fd_write:() => {throw Error('unexpected Wasm diagnostic')}}
  const {instance} = await WebAssembly.instantiate(readFileSync(join(root, 'apps/openagents-web/build/newland.wasm')), {env, wasi_snapshot_preview1:wasi})
  x = instance.exports
  x._initialize()
  const align = (n:number) => (n + 15) & ~15
  state = align(Number(x.__heap_base.value) + 1024)
  const vertices = align(state + x.coaxial_state_size() + 16)
  const uniforms = align(vertices + x.coaxial_max_vertices() * x.coaxial_vertex_floats() * 4 + 16)
  if (uniforms + x.coaxial_uniform_floats() * 4 >= memory.buffer.byteLength) throw Error('homepage arena capacity')
  function deliver(id:number, kind:number, text:string) {
    const bytes = encode.encode(text)
    if (bytes.length > x.coaxial_inbox_cap()) throw Error('fixture frame capacity')
    new Uint8Array(memory.buffer, x.coaxial_inbox(state), bytes.length).set(bytes)
    x.coaxial_message(state, 256 + 4 * id + kind, bytes.length)
  }
  function frame(dt = 0.1) {
    const n = x.coaxial_frame(state, vertices, uniforms, viewport.width, viewport.height, viewport.dpr, dt, 1)
    expect(n).toBeGreaterThan(100)
    expect(n).toBeLessThanOrEqual(x.coaxial_max_vertices())
    while (pending.length) {
      const event = pending.shift()!
      deliver(event.id, event.kind, event.text)
    }
  }
  const box = (which:number):Box => [0, 1, 2, 3].map(axis => x.coaxial_land_pylon_control(state, which, axis)) as Box
  const center = (b:Box) => [(b[0] + b[2]) / 2, (b[1] + b[3]) / 2]
  const pointer = (a:number, b:number, phase:number, id = 1) => x.coaxial_pointer(state, a, b, phase, viewport.width / viewport.dpr < 600 ? 1 : 0, id)
  function click(which:number) {
    const [a, b] = center(box(which))
    pointer(a, b, 0)
    pointer(a, b, 2)
  }
  function beacon(event:any) {
    const watch = sent.find(message => message.value[0] === 'REQ' && message.value[1] === 'home-beacons')
    if (!watch) throw Error('homepage did not subscribe to signed beacons')
    deliver(watch.id, 1, JSON.stringify(['EVENT', 'home-beacons', event]))
  }
  function open() {
    click(0)
    frame(0.3)
    expect(x.coaxial_land_network_open(state)).toBe(1)
  }
  frame()
  return {
    x, state, frame, box, center, pointer, click, beacon, open,
    setNow:(value:number) => {now = value},
    action:() => x.coaxial_action(state),
    outbox:() => decode(x.coaxial_outbox(state), x.coaxial_outbox_len(state)),
    clearAction:() => x.coaxial_action_done(state),
    online:() => x.coaxial_land_network_online(state),
    free:() => x.coaxial_land_network_free(state),
    scroll:() => x.coaxial_land_network_scroll(state),
  }
}

function insideViewport(box:Box, viewport:Viewport) {
  expect(box.every(Number.isFinite)).toBe(true)
  expect(box[2]).toBeGreaterThan(box[0])
  expect(box[3]).toBeGreaterThan(box[1])
  expect(box[0]).toBeGreaterThanOrEqual(0)
  expect(box[1]).toBeGreaterThanOrEqual(0)
  expect(box[2]).toBeLessThanOrEqual(viewport.width)
  expect(box[3]).toBeLessThanOrEqual(viewport.height)
}
function disjoint(a:Box, b:Box) {
  return a[2] <= b[0] || b[2] <= a[0] || a[3] <= b[1] || b[3] <= a[1]
}

for (const viewport of [
  {width:1440, height:900, dpr:1},
  {width:390, height:844, dpr:1},
  {width:780, height:1688, dpr:2},
]) test(`homepage Pylon controls copy the complete command and open the guide at ${viewport.width}x${viewport.height}@${viewport.dpr}`, async () => {
  const h = await homepage(viewport)
  expect(h.x.coaxial_land_network_open(h.state)).toBe(0)
  insideViewport(h.box(0), viewport)
  h.open()
  for (const control of [1, 2, 3]) insideViewport(h.box(control), viewport)
  expect(disjoint(h.box(1), h.box(2))).toBe(true)
  expect(disjoint(h.box(1), h.box(3))).toBe(true)
  expect(disjoint(h.box(2), h.box(3))).toBe(true)
  expect(h.action()).toBe(0)
  h.click(1)
  expect(h.action()).toBe(2)
  expect(h.outbox()).toBe(joinCommand)
  expect(h.x.coaxial_outbox_len(h.state)).toBe(Buffer.byteLength(joinCommand))
  h.clearAction()
  expect(h.action()).toBe(0)
  expect(h.outbox()).toBe('')
  h.click(2)
  expect(h.action()).toBe(1)
  expect(h.outbox()).toBe('/docs/connect-a-pylon')
  h.clearAction()
  h.click(3)
  h.frame(0.3)
  expect(h.x.coaxial_land_network_open(h.state)).toBe(0)
  expect(h.action()).toBe(0)
})

test('homepage Pylon panel cancels controls and scroll gestures never activate a command or guide', async () => {
  const viewport = {width:390, height:844, dpr:1}
  const h = await homepage(viewport)
  for (const event of fixture.many_beacons) h.beacon(event)
  h.frame()
  expect(h.online()).toBe(10)
  h.open()
  for (const control of [1, 2]) {
    const [a, b] = h.center(h.box(control))
    h.pointer(a, b, 0)
    h.pointer(a, b, 3)
    h.pointer(a, b, 2)
    expect(h.action()).toBe(0)
    expect(h.outbox()).toBe('')
  }
  const scrollBefore = h.scroll()
  h.x.coaxial_wheel(h.state, 200)
  h.frame()
  expect(h.scroll()).toBeGreaterThan(scrollBefore)
  const afterWheel = h.scroll()
  h.pointer(viewport.width / 2, viewport.height - 160, 0)
  h.pointer(viewport.width / 2, viewport.height - 220, 1)
  h.pointer(viewport.width / 2, viewport.height - 260, 1)
  h.pointer(viewport.width / 2, viewport.height - 260, 2)
  h.frame()
  expect(h.scroll()).toBeGreaterThan(afterWheel)
  const [a, b] = h.center(h.box(2))
  h.pointer(a, b, 0)
  h.pointer(a, b - 60, 1)
  h.pointer(a, b - 100, 1)
  h.pointer(a, b - 100, 2)
  h.frame()
  expect(h.action()).toBe(0)
  expect(h.outbox()).toBe('')
  expect(h.x.coaxial_land_network_open(h.state)).toBe(1)
  expect(h.scroll()).toBeGreaterThan(scrollBefore)
})

test('homepage Pylon panel counts only fresh verified availability and updates without reopening', async () => {
  const h = await homepage({width:1440, height:900, dpr:1})
  h.open()
  expect(h.online()).toBe(0)
  expect(h.free()).toBe(0)
  h.beacon(fixture.invalid_beacons.digest)
  h.beacon({...fixture.beacon, sig:'0'.repeat(128)})
  h.frame()
  expect(h.online()).toBe(0)
  expect(h.free()).toBe(0)
  h.beacon(fixture.beacon)
  h.beacon(fixture.beacon)
  h.frame()
  expect(h.online()).toBe(1)
  expect(h.free()).toBe(JSON.parse(fixture.beacon.content).slots.free)
  h.setNow(fixture.now + 1)
  h.beacon(fixture.updated_beacon)
  h.frame()
  expect(h.online()).toBe(1)
  expect(h.free()).toBe(0)
  h.setNow(fixture.now + 302)
  h.frame()
  expect(h.online()).toBe(0)
  expect(h.free()).toBe(0)
  expect(h.x.coaxial_land_network_open(h.state)).toBe(1)
})

test('Pylon scroll regression: the remaining finger continues from the current scroll after another finger lifts', async () => {
  const h = await homepage({width:390, height:844, dpr:1})
  for (const event of fixture.many_beacons) h.beacon(event)
  h.frame()
  h.open()
  h.x.coaxial_wheel(h.state, 100)
  expect(h.scroll()).toBe(100)
  h.pointer(195, 400, 0, 1)
  h.pointer(195, 300, 1, 1)
  expect(h.scroll()).toBe(200)
  h.pointer(195, 500, 0, 2)
  h.pointer(195, 300, 2, 1)
  h.pointer(195, 490, 1, 2)
  expect(h.scroll()).toBe(210)
  h.pointer(195, 490, 2, 2)
  // Also keep the finger that already moved: its held coordinates must be
  // current when the second finger leaves and the drag origin is rebased.
  h.pointer(195, 400, 0, 1)
  h.pointer(195, 300, 1, 1)
  expect(h.scroll()).toBe(310)
  h.pointer(195, 500, 0, 2)
  h.pointer(195, 500, 2, 2)
  h.pointer(195, 290, 1, 1)
  expect(h.scroll()).toBe(320)
  h.pointer(195, 290, 2, 1)
  expect(h.action()).toBe(0)
  expect(h.x.coaxial_land_network_open(h.state)).toBe(1)
})

test('Pylon scroll regression: CSS wheel deltas preserve their distance at DPR 2', async () => {
  const h = await homepage({width:780, height:1688, dpr:2})
  for (const event of fixture.many_beacons) h.beacon(event)
  h.frame()
  h.open()
  expect(h.scroll()).toBe(0)
  h.x.coaxial_wheel(h.state, 100)
  expect(h.scroll()).toBe(100)
  expect(h.action()).toBe(0)
  expect(h.x.coaxial_land_network_open(h.state)).toBe(1)
})
