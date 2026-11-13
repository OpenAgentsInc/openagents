// Firm reason: 1 (it runs state-keep.js, the browser host's own module, as the browser does)
// A code reload keeps the game's state (#13411): the plan host.js applies on an engine swap.
//   bun test coax/web/state-keep.test.ts
// The tables are `coax layout`'s own text for the fixtures in tests/state-keep/ (one base state and
// one edit of it a file); the last test prints them again with this checkout's compiler, when it
// is built, so a change to the text's form fails here.
import { test, expect } from 'bun:test'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import { parseLayout, stateRoot, planState, applyState, stateNote } from './state-keep.js'

const dir = join(import.meta.dir, 'tests/state-keep')
const NAMES = ['base', 'reordered', 'added', 'removed', 'retyped', 'nested']
const table = (name: string) => readFileSync(join(dir, `${name}.layout`), 'utf8')
const sizeOf = (text: string, root = 'Game') => parseLayout(text).structs.get(root)!.size

// A field's place by path ("inner[0].pos.x"), through the table.
function at(text: string, path: string, root = 'Game') {
  const l = parseLayout(text)
  let type = root, offset = 0, size = l.structs.get(root)!.size
  for (const part of path.match(/[A-Za-z_]\w*|\[\d+\]/g) ?? []) {
    if (part[0] === '[') {
      const [, n, elem] = /^\[(\d+)\] (.+)$/.exec(type)!
      size /= +n; offset += +part.slice(1, -1) * size; type = elem
    } else {
      const f = l.structs.get(type)!.fields.find((x: any) => x.name === part)!
      offset += f.offset; size = f.size; type = f.type
    }
  }
  return { offset, size, type }
}
type Value = number | bigint
function put(view: DataView, text: string, path: string, value: Value) {
  const { offset, type } = at(text, path)
  if (type === 'f32') view.setFloat32(offset, value as number, true)
  else if (type === 'f64') view.setFloat64(offset, value as number, true)
  else if (type === 'int') view.setBigInt64(offset, BigInt(value), true)
  else if (type === 's32') view.setInt32(offset, Number(value), true)
  else if (type === 'u16') view.setUint16(offset, Number(value), true)
  else view.setUint8(offset, Number(value))   // bool, u8, an enum on u8
}
function get(view: DataView, text: string, path: string): Value {
  const { offset, type } = at(text, path)
  if (type === 'f32') return view.getFloat32(offset, true)
  if (type === 'f64') return view.getFloat64(offset, true)
  if (type === 'int') return Number(view.getBigInt64(offset, true))
  if (type === 's32') return view.getInt32(offset, true)
  if (type === 'u16') return view.getUint16(offset, true)
  return view.getUint8(offset)
}

// The base game mid-run: every field holds a value that tells it from the others.
const BASE: Record<string, Value> = {
  ready: 1, heat: 0.75, score: -1234, mode: 1 /* walk */, gone: 77,
  'items[0].id': 11, 'items[0].n': 1, 'items[1].id': -22, 'items[1].n': 2, 'items[2].id': 33, 'items[2].n': 3,
  'inner[0].hp': 9000000000, 'inner[0].pos.x': 1.5, 'inner[0].pos.y': -2.5,
  'inner[0].tag[0]': 65, 'inner[0].tag[1]': 66, 'inner[0].tag[2]': 67, 'inner[0].tag[3]': 68,
  'grid[0][0]': 1, 'grid[0][1]': 2, 'grid[1][0]': 3, 'grid[1][1]': 4,
}
// Runs the swap as host.js does: the old state's bytes, a new state holding `defaults` (what
// coaxial_state_init leaves), then the plan. The new state sits at 64 in a larger buffer, as in the arena.
function swap(to: string, defaults: Record<string, Value> = {}, old = BASE) {
  const a = table('base'), b = table(to)
  const before = new Uint8Array(sizeOf(a)), bv = new DataView(before.buffer)
  for (const [path, v] of Object.entries(old)) put(bv, a, path, v)
  bv.setUint32(at(a, 'name').offset, 0x1234, true); bv.setBigInt64(at(a, 'name').offset + 8, 5n, true)   // a string: pointer, count
  const buffer = new ArrayBuffer(64 + sizeOf(b) + 64), view = new DataView(buffer, 64, sizeOf(b))
  for (const [path, v] of Object.entries(defaults)) put(view, b, path, v)
  const plan = planState(a, sizeOf(a), b, sizeOf(b))!
  applyState(plan, before, buffer, 64)
  const around = new Uint8Array(buffer)
  for (let i = 0; i < 64; i++) expect(around[i] | around[64 + sizeOf(b) + i]).toBe(0)   // nothing outside the state
  return { plan, read: (path: string) => get(view, b, path), view, b }
}
const expectKept = (s: ReturnType<typeof swap>, paths: string[]) => { for (const p of paths) expect([p, s.read(p)]).toEqual([p, BASE[p]]) }

test('reordered fields keep their values at their new offsets', () => {
  const s = swap('reordered')
  expectKept(s, Object.keys(BASE))
  expect(at(s.b, 'heat').offset).not.toBe(at(table('base'), 'heat').offset)
  expect(s.plan.added).toEqual([]); expect(s.plan.dropped).toEqual([]); expect(s.plan.resized).toEqual([])
  expect(s.plan.reset).toEqual([]); expect(s.plan.pointers).toEqual(['name'])
  expect(s.plan.kept).toBe(12)   // ready heat score mode gone grid; items' id n; inner's hp tag pos.x pos.y
  expect(s.plan.same).toBe(false)
})

test('added fields take their defaults; every old field is kept', () => {
  const s = swap('added', { shield: 2.5, lives: 3, 'inner[0].mp': 9 })
  expectKept(s, Object.keys(BASE))
  expect(s.read('shield')).toBe(2.5); expect(s.read('lives')).toBe(3); expect(s.read('inner[0].mp')).toBe(9)
  expect(s.plan.added).toEqual(['shield', 'inner.mp', 'lives'])
  expect(s.plan.dropped).toEqual([])
  expect(stateNote(s.plan)).toBe('state kept: 12 fields; new shield, inner.mp, lives; 1 pointer field at its default')
})

test('removed fields are dropped and named; the rest is kept', () => {
  const s = swap('removed')
  expectKept(s, Object.keys(BASE).filter((p) => p !== 'gone' && !p.includes('tag')))
  expect(s.plan.dropped).toEqual(['inner.tag', 'gone'])
  expect(s.plan.added).toEqual([])
  expect(s.plan.kept).toBe(10)
})

test('a retyped field is converted when the new type holds every old value, else it takes its default', () => {
  const s = swap('retyped', { ready: 5, gone: 0.5, 'items[3].id': 40, 'items[4].id': 50, 'grid[0][2]': 9, 'inner[0].tag[5]': 90 })
  expect(s.read('score')).toBe(-1234)                // s32 to int
  expect(s.read('heat')).toBe(0.75)                  // f32 to f64
  expect(s.read('mode')).toBe(3)                     // walk, by name: 1 in the old enum, 3 in the new
  expect(s.read('ready')).toBe(5)                    // bool to int: its default
  expect(s.read('gone')).toBe(0.5)                   // int to f32: its default
  expectKept(s, ['items[0].id', 'items[0].n', 'items[1].id', 'items[2].n', 'inner[0].hp', 'inner[0].pos.y', 'inner[0].tag[0]', 'inner[0].tag[3]'])
  expect(s.read('items[3].id')).toBe(40); expect(s.read('items[4].id')).toBe(50); expect(s.read('inner[0].tag[5]')).toBe(90)
  expect([s.read('grid[0][0]'), s.read('grid[0][1]'), s.read('grid[0][2]'), s.read('grid[1][0]'), s.read('grid[1][1]')]).toEqual([1, 2, 9, 3, 4])
  expect(s.plan.reset).toEqual(['ready: bool to int', 'gone: int to f32'])
  expect(s.plan.resized).toEqual(['items: [3] to [5]', 'inner.tag: [4] to [6]', 'grid[]: [2] to [3]'])
  expect(s.plan.added).toEqual([]); expect(s.plan.dropped).toEqual([])
})

test('an enum value whose member is gone takes the default', () => {
  const a = 'S  size 1  align 1\n       0     1  m: M\nM  enum u8  size 1\n  a = 0\n  b = 1\n'
  const b = 'S  size 1  align 1\n       0     1  m: M\nM  enum u8  size 1\n  b = 0\n  c = 1\n'
  const plan = planState(a, 1, b, 1)!
  const run = (old: number) => { const buffer = new ArrayBuffer(1); new Uint8Array(buffer)[0] = 1; applyState(plan, new Uint8Array([old]), buffer, 0); return new Uint8Array(buffer)[0] }
  expect(run(1)).toBe(0)   // b
  expect(run(0)).toBe(1)   // a is gone: the default the new state held
})

test('nested structs and arrays of structs are carried field by field', () => {
  const s = swap('nested', { 'inner[0].pos.z': 7, 'items[0].q': 100, 'items[2].q': 102 })
  expectKept(s, Object.keys(BASE))                   // items' id is now int: -22 and the rest widen
  expect(s.read('inner[0].pos.z')).toBe(7); expect(s.read('items[0].q')).toBe(100); expect(s.read('items[2].q')).toBe(102)
  expect(s.plan.added).toEqual(['items[].q', 'inner.pos.z'])   // once, not once an element
  expect(s.plan.kept).toBe(12)
})

test('numbers widen only when no value is lost', () => {
  const a = 'S  size 16  align 4\n       0     1  a: s8\n       1     1  b: u8\n       2     2  c: u16\n       4     4  d: f32\n       8     4  e: u32\n      12     4  f: s32\n'
  const b = 'S  size 40  align 8\n       0     4  a: s32\n       4     2  b: s16\n       6     1  c: u8\n       8     8  d: f64\n      16     8  e: int\n      24     4  f: u32\n      32     8  g: u64\n'
  const old = new Uint8Array(16), ov = new DataView(old.buffer)
  ov.setInt8(0, -5); ov.setUint8(1, 250); ov.setUint16(2, 513, true); ov.setFloat32(4, -0.5, true); ov.setUint32(8, 4000000000, true); ov.setInt32(12, -1, true)
  const buffer = new ArrayBuffer(40), nv = new DataView(buffer)
  nv.setUint8(6, 42); nv.setUint32(24, 99, true)
  const plan = planState(a, 16, b, 40)!
  applyState(plan, old, buffer, 0)
  expect(nv.getInt32(0, true)).toBe(-5); expect(nv.getInt16(4, true)).toBe(250); expect(nv.getFloat64(8, true)).toBe(-0.5); expect(nv.getBigInt64(16, true)).toBe(4000000000n)
  expect(nv.getUint8(6)).toBe(42); expect(nv.getUint32(24, true)).toBe(99)   // narrowed, and signed to unsigned: defaults
  expect(plan.reset).toEqual(['c: u16 to u8', 'f: s32 to u32']); expect(plan.added).toEqual(['g']); expect(plan.kept).toBe(4)
})

test('a signed number never becomes unsigned, and a shorter array takes only what fits', () => {
  const a = 'S  size 8  align 2\n       0     1  a: s8\n       1     1  (padding)\n       2     2  b: s16\n       4     4  c: [4] u8\n'
  const b = 'S  size 12  align 4\n       0     2  a: u16\n       2     2  (padding)\n       4     4  b: u32\n       8     2  c: [2] u8\n      10     1  z: u8\n      11     1  (padding)\n'
  const old = new Uint8Array([0xff, 0, 0xfe, 0xff, 1, 2, 3, 4]), buffer = new ArrayBuffer(12), out = new Uint8Array(buffer)
  out.set([7, 0, 0, 0, 8, 0, 0, 0, 0, 0, 9, 0])
  const plan = planState(a, 8, b, 12)!
  applyState(plan, old, buffer, 0)
  expect([...out]).toEqual([7, 0, 0, 0, 8, 0, 0, 0, 1, 2, 9, 0])   // a and b keep their defaults; c's first two; z, a new field, its default
  expect(plan.reset).toEqual(['a: s8 to u16', 'b: s16 to u32']); expect(plan.resized).toEqual(['c: [4] to [2]']); expect(plan.added).toEqual(['z']); expect(plan.kept).toBe(1)
})

test('an unchanged state is `same`; carried to a new place, all but its pointers go with it', () => {
  const a = table('base'), plan = planState(a, sizeOf(a), a, sizeOf(a))!
  expect(plan.same).toBe(true)
  const s = swap('base')
  expectKept(s, Object.keys(BASE))
  expect(s.view.getUint32(at(a, 'name').offset, true)).toBe(0)   // the string's pointer stays the new state's
  expect(planState(a, sizeOf(a), table('added'), sizeOf(table('added')))!.same).toBe(false)
})

test('the same two tables always give the same plan', () => {
  for (const name of NAMES) {
    const a = table('base'), b = table(name)
    const one = planState(a, sizeOf(a), b, sizeOf(b))!, two = planState(a, sizeOf(a), b, sizeOf(b))!
    const text = (p: any) => JSON.stringify({ ...p, maps: p.maps.map((m: Map<bigint, bigint>) => [...m].map(([k, v]) => [String(k), String(v)])) })
    expect(text(one)).toBe(text(two))
  }
})

test('the root is the struct of the state\'s size; a tie or no match is refused', () => {
  const l = parseLayout(table('base'))
  expect(stateRoot(l, 96)).toBe('Game'); expect(stateRoot(l, 97)).toBe('')
  const two = parseLayout('A  size 8  align 8\n       0     8  x: int\nB  size 8  align 8\n       0     8  y: int\n')
  expect(stateRoot(two, 8)).toBe(''); expect(stateRoot(two, 8, 'B')).toBe('B')
  expect(planState(table('base'), 5, table('base'), 96)).toBe(null)
})

test('sums, enums and generic names parse as the compiler prints them', () => {
  const l = parseLayout('coax-layout-sums-lp64.v1\n[] HandleItem?  sum size 24  align 8  tag u32@0  payload@8\n  0: void\n  1: [] HandleItem\nPair(int)  size 16  align 8\n       0     8  a: int\n       8     8  b: int\nK  enum s8  size 1\n  lo = -1\n  hi = 1\n')
  expect(l.sums.get('[] HandleItem?')).toEqual({ size: 24, payload: 8, variants: ['void', '[] HandleItem'] })
  expect(l.structs.get('Pair(int)')!.fields.map((f: any) => f.name)).toEqual(['a', 'b'])
  expect([...l.enums.get('K')!.members]).toEqual([['lo', -1n], ['hi', 1n]])
})

const compiler = join(import.meta.dir, '../bin/coax')
test.skipIf(!existsSync(compiler))('the fixtures\' tables are what this compiler prints', () => {
  for (const name of NAMES) {
    const run = spawnSync(compiler, ['layout', join(dir, `${name}.coax`), '--library'], { encoding: 'utf8' })
    expect([name, run.status, run.stdout]).toEqual([name, 0, table(name)])
  }
})
