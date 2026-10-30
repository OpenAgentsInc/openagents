// The project page's NIP-19 names (apps/openagents-web/project-names.coax),
// built natively, against nostr-tools' encoder (nostr-effect/nip19, the same
// API and TLV order) for every key, event and coordinate in the captured
// relay transcript coax/lib/nostr/fixtures/coax_relay.jsonl.
//   cd coax/web/project && CC=gcc bun test nip19.test.ts
import { afterAll, beforeAll, expect, test } from 'bun:test'
import { dlopen, FFIType, ptr } from 'bun:ffi'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { naddrEncode, neventEncode, npubEncode } from 'nostr-effect/nip19'

const root = join(import.meta.dir, '../../..'), app = join(root, 'apps/openagents-web')
const temp = mkdtempSync(join(tmpdir(), 'coax-nip19-'))
const RELAY = 'wss://relay.openagents.com'
const HEX = /^[0-9a-f]{64}$/

const driver = `#load "${join(app, 'project-names.coax')}";
probe_npub :: (key: string, out: [] u8) -> int #export_c { return name_npub(key, out); }
probe_nevent :: (id: string, relay: string, author: string, kind: int, out: [] u8) -> int #export_c { return name_nevent(id, relay, author, kind, out); }
probe_coord :: (coord: string, relay: string, out: [] u8) -> int #export_c { return name_coord(coord, relay, out); }
`
const shim = `#include "names.h"
static coax_string S(const char *p, int n) { return (coax_string){p, n}; }
static coax_v_u8 O(char *p, int n) { return (coax_v_u8){(uint8_t *)p, n}; }
int npub(const char *k, int kn, char *o, int on) { return (int)probe_npub(S(k, kn), O(o, on)); }
int nevent(const char *i, int in, const char *r, int rn, const char *a, int an, int kind, char *o, int on) { return (int)probe_nevent(S(i, in), S(r, rn), S(a, an), kind, O(o, on)); }
int coord(const char *c, int cn, const char *r, int rn, char *o, int on) { return (int)probe_coord(S(c, cn), S(r, rn), O(o, on)); }
`
let lib: any
const run = (command: string[]) => {
  const r = Bun.spawnSync(command, { cwd: root, stdout: 'pipe', stderr: 'pipe' })
  if (r.exitCode !== 0) throw Error(new TextDecoder().decode(r.stderr))
  return r.stdout
}
beforeAll(() => {
  writeFileSync(join(temp, 'names.coax'), driver)
  writeFileSync(join(temp, 'names.c'), run([process.env.COAX ?? join(root, 'coax/bin/coax'), 'c', join(temp, 'names.coax'), '--library', '--header', join(temp, 'names.h')]))
  writeFileSync(join(temp, 'shim.c'), shim)
  run([process.env.CC ?? 'cc', '-std=c11', '-O1', '-shared', '-fPIC', join(temp, 'names.c'), join(temp, 'shim.c'), '-I' + temp, '-o', join(temp, 'names.so')])
  const I = FFIType.i32, P = FFIType.ptr
  lib = dlopen(join(temp, 'names.so'), {
    npub: { args: [P, I, P, I], returns: I },
    nevent: { args: [P, I, P, I, P, I, I, P, I], returns: I },
    coord: { args: [P, I, P, I, P, I], returns: I },
  })
}, 60000)
afterAll(() => { lib?.close(); rmSync(temp, { recursive: true, force: true }) })

// A string's bytes with a spare byte (bun:ffi wants a non-empty buffer).
const b = (s: string) => Buffer.from(s + '\0')
const call = (f: (out: Buffer) => number) => { const out = Buffer.alloc(512); const n = f(out); return out.subarray(0, n).toString() }
const coaxNpub = (k: string) => call((o) => lib.symbols.npub(ptr(b(k)), k.length, ptr(o), o.length))
const coaxNevent = (id: string, relay: string, author: string, kind: number) =>
  call((o) => lib.symbols.nevent(ptr(b(id)), id.length, ptr(b(relay)), relay.length, ptr(b(author)), author.length, kind, ptr(o), o.length))
const coaxCoord = (c: string, relay: string) => call((o) => lib.symbols.coord(ptr(b(c)), Buffer.byteLength(c), ptr(b(relay)), relay.length, ptr(o), o.length))

// Everything the transcript names: keys, events (with what's known of each)
// and coordinates (with the hint their tag carries, if any).
const frames = readFileSync(join(root, 'coax/lib/nostr/fixtures/coax_relay.jsonl'), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l))
const events = frames.filter((f) => f[0] === 'EVENT').map((f) => f[2])
const keys = new Set<string>(), ids = new Set<string>(), coords = new Map<string, Set<string>>()
const coordOf = (c: string, hint: string) => { if (!coords.has(c)) coords.set(c, new Set()); coords.get(c)!.add(hint) }
for (const e of events) {
  keys.add(e.pubkey)
  if (e.kind >= 30000 && e.kind < 40000) coordOf(`${e.kind}:${e.pubkey}:${e.tags.find((t: string[]) => t[0] === 'd')?.[1] ?? ''}`, '')
  for (const t of e.tags) {
    if ((t[0] === 'p' || t[0] === 'P') && HEX.test(t[1])) keys.add(t[1])
    if (['e', 'E', 'item', 'parent', 'blocked-by'].includes(t[0]) && HEX.test(t[1])) ids.add(t[1])
    if ((t[0] === 'a' || t[0] === 'A') && /^\d+:[0-9a-f]{64}:/.test(t[1])) { coordOf(t[1], t[2] ?? ''); keys.add(t[1].split(':')[1]) }
  }
}

test('the transcript names enough to check', () => {
  expect(events.length).toBeGreaterThan(200)
  expect(keys.size).toBeGreaterThan(0); expect(ids.size).toBeGreaterThan(100); expect(coords.size).toBeGreaterThan(2)
})
test('every key is the npub nostr-tools spells', () => {
  for (const k of keys) expect(coaxNpub(k)).toBe(npubEncode(k))
})
test('every event is the nevent nostr-tools spells: bare, with the relay hint, and with author and kind', () => {
  for (const e of events) {
    expect(coaxNevent(e.id, '', '', -1)).toBe(neventEncode({ id: e.id, relays: [] }))
    expect(coaxNevent(e.id, RELAY, e.pubkey, e.kind)).toBe(neventEncode({ id: e.id, relays: [RELAY], author: e.pubkey, kind: e.kind }))
    expect(coaxNevent(e.id, '', e.pubkey, -1)).toBe(neventEncode({ id: e.id, relays: [], author: e.pubkey }))
  }
  for (const id of ids) expect(coaxNevent(id, RELAY, '', -1)).toBe(neventEncode({ id, relays: [RELAY] }))
})
test('every coordinate is the naddr nostr-tools spells, with its tag\'s relay hint and with the page\'s', () => {
  for (const [c, hints] of coords) {
    const [kind, pubkey, ...rest] = c.split(':'), identifier = rest.join(':')
    for (const hint of [...hints, '', RELAY]) {
      expect(coaxCoord(c, hint)).toBe(naddrEncode({ kind: Number(kind), pubkey, identifier, relays: hint ? [hint] : [] }))
    }
  }
})
test('what isn\'t a key, an event or a coordinate gets no name', () => {
  const k = [...keys][0]
  expect(coaxNpub(k.toUpperCase())).toBe(''); expect(coaxNpub(k.slice(1))).toBe('')
  expect(coaxNevent('x', RELAY, k, 1)).toBe(''); expect(coaxCoord(`30621;${k}:coax`, RELAY)).toBe('')
})
