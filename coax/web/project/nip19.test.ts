// The project page's NIP-19 names (apps/openagents-web/project-names.coax),
// built natively, against nostr-tools' encoder (nostr-effect/nip19, the same
// API and TLV order) for every key, event and coordinate in the captured
// relay transcript coax/lib/nostr/fixtures/coax_relay.jsonl.
//   cd coax/web/project && CC=gcc bun test nip19.test.ts
import { afterAll, beforeAll, expect, test } from 'bun:test'
import { CString, dlopen, FFIType, ptr } from 'bun:ffi'
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
afterAll(() => { lib?.close(); page?.close(); rmSync(temp, { recursive: true, force: true }) })

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

// The page itself, built natively from apps/openagents-web/project.coax with
// project-names.coax as its library and the face's own naming step
// (project-names-face.h): the names it draws are nostr-tools', and with them
// in, every panel and an open task's detail still fit, at a desktop, a tablet
// and a phone, inside the vertex budget (the page's detail_fits, step by step
// so the face's names go in between opening a task and drawing it).
const pageDriver = `#load "${join(app, 'project.coax')}";
pd_draw :: (g: [] Page, out: [] f32, w: f32, h: f32) -> int #export_c { return draw_page(g, out, w, h); }
pd_samples :: (g: [] Page, pick: [] int) -> int #export_c { return samples(g[0].w[..], pick); }
pd_open :: (g: [] Page, r: int) #export_c { open_task(g[0].s[..], g[0].w[..], r); }
pd_scroll_end :: (g: [] Page) #export_c { g[0].s[0].scroll = g[0].s[0].scroll_max; }
pd_page :: (g: [] Page, page: int) #export_c { g[0].s[0].page = page; }
pd_fits :: (g: [] Page, out: [] f32, n: int, w: f32, h: f32, with_detail: bool) -> int #export_c {
  l := project_layout(w, h, 2, g[0].w[..]);
  if n < 300 || n >= MAX_VERTICES || n % 6 != 0 { return 10; }
  if !on_canvas(out, n, w, h, 2) { return 11; }
  if !with_detail { return ifx n > 3000 then 0 else 12; }
  if !l.wide && !within(out, 0, n, detail_box(l, w, h, 2), 2) { return 13; }
  if l.wide && !same_box(g[0].s[0].panel, l.scene) { return 14; }
  return 0;
}
pd_max :: () -> int #export_c { return PAGE_OUT; }
`
const pageProbe = `#include "page.h"
#include "project-names-face.h"
#include <stdlib.h>
#include <string.h>
static coax_t_Page *g; static float *out;
static coax_v_t_Page G(void) { return (coax_v_t_Page){g, 1}; }
static coax_v_f32 OUT(void) { return (coax_v_f32){out, pd_max()}; }
static int draw(float w, float h) { project_names(&g->s.e[0], &g->w.e[0]); return (int)pd_draw(G(), OUT(), w, h); }
void page_new(int page) { g = calloc(1, sizeof *g); out = calloc(pd_max(), sizeof(float)); pd_page(G(), page); }
void page_free(void) { free(g); free(out); }
// Every panel at this size, then each sample task's detail at its top and its
// end: 0 when all fit, else what didn't (10s the panels, 100s + the detail at
// its top, 200s at its end, 4 an open task without its nevent).
int page_fits(float sw, float sh, int most) {
  float w = sw * 2, h = sh * 2;
  int n = draw(w, h); n = draw(w, h);   // the first frame picks the project; the face names it before the next
  int e = (int)pd_fits(G(), OUT(), n, w, h, 0); if (e) return e;
  int64_t pick[3]; int64_t k = pd_samples(G(), (coax_v_s64){pick, 3});
  for (int i = 0; i < k && i < most; i++) {
    pd_open(G(), pick[i]);
    for (int end = 0; end < 2; end++) {
      if (end) pd_scroll_end(G());
      n = draw(w, h);
      e = (int)pd_fits(G(), OUT(), n, w, h, 1); if (e) return 100 * (end + 1) + e;
      if (g->s.e[0].nevent_n == 0) return 4;
    }
  }
  return 0;
}
// 0 the drawn naddr, 1 the npub, 2 the open task's nevent; 3 its hex id.
const char *page_text(int k) {
  coax_t_Work *w = &g->w.e[0]; coax_t_ProjectState *s = &g->s.e[0];
  if (k == 0) return (const char *)w->naddr_buf.e;
  if (k == 1) return (const char *)w->npub_buf.e;
  if (k == 2) return (const char *)s->nevent.e;
  return (const char *)s->event.e;
}
int page_len(int k) { coax_t_Work *w = &g->w.e[0]; coax_t_ProjectState *s = &g->s.e[0]; return (int)(k == 0 ? w->naddr_n : k == 1 ? w->npub_n : k == 2 ? s->nevent_n : s->event_n); }
`
let page: any
const coax = () => process.env.COAX ?? join(root, 'coax/bin/coax')
test('the page draws nostr-tools\' names, and every panel and a task\'s detail fit at a desktop, a tablet and a phone', () => {
  writeFileSync(join(temp, 'page.coax'), pageDriver)
  writeFileSync(join(temp, 'page.c'), run([coax(), 'c', join(temp, 'page.coax'), '--library', '--header', join(temp, 'page.h')]))
  writeFileSync(join(temp, 'Project-names.c'), run([coax(), 'c', join(app, 'project-names.coax'), '--library', '--header', join(temp, 'Project-names.h')]))
  writeFileSync(join(temp, 'Project-capacity.c'), run([coax(), 'c', join(app, 'project-capacity.coax'), '--library']))
  writeFileSync(join(temp, 'probe.c'), pageProbe)
  run([process.env.CC ?? 'cc', '-std=c11', '-O1', '-w', '-shared', '-fPIC', join(temp, 'page.c'), join(temp, 'Project-names.c'), join(temp, 'Project-capacity.c'), join(temp, 'probe.c'),
    '-I' + temp, '-I' + app, '-lm', '-o', join(temp, 'page.so')])
  const I = FFIType.i32, F = FFIType.f32
  page = dlopen(join(temp, 'page.so'), { page_new: { args: [I] }, page_free: {}, page_fits: { args: [F, F, I], returns: I },
    page_text: { args: [I], returns: FFIType.ptr }, page_len: { args: [I], returns: I } })
  const text = (k: number) => { const n = page.symbols.page_len(k); return n > 0 ? new CString(page.symbols.page_text(k), 0, n).toString() : '' }
  const baked = JSON.parse(readFileSync(join(app, 'projects/coax/project.json'), 'utf8'))
  // the real page (0) opens tasks; the demo (1) has no detail, its panels only
  for (const p of [0, 1]) {
    for (const [w, h, most] of [[1440, 900, 2], [768, 1024, 3], [320, 568, 3]]) {
      page.symbols.page_new(p)
      try {
        expect([p, w, h, page.symbols.page_fits(w, h, p === 0 ? most : 0)]).toEqual([p, w, h, 0])
        if (p === 1) continue
        expect(text(0)).toBe(naddrEncode({ kind: 30621, pubkey: baked.pubkey, identifier: baked.slug, relays: [baked.relay] }))
        expect(text(1)).toBe(npubEncode(baked.pubkey))
        const id = text(3); expect(id).toMatch(HEX)
        expect(text(2)).toBe(neventEncode({ id, relays: [baked.relay], author: baked.pubkey, kind: 1621 }))
      } finally { page.symbols.page_free() }
    }
  }
}, 120000)
