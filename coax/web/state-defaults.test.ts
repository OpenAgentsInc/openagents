// Every browser engine boots from zeroed memory, as the host gives it, and
// must end with the struct defaults native gives `s: [1] T` (Orbit League ran
// without gravity for a day: spin_g = 1.0 read 0 in the browser). The
// compiler declares coax_default_T in a unit's header for each of its types
// with declared defaults; the face calls it for a new state
// (coaxial_state_init) and for its own globals. For each engine build.sh
// makes, this runs the built module's coaxial_state_init on a zeroed state and
// compares the state, byte for byte, with the native coax_init of its type.
// It also builds the face natively and compares every face global of a
// defaulted type with that type's native coax_init. Needs
// apps/openagents-web/build.sh first.
import { expect, test } from "bun:test"
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { basename, dirname, join } from "node:path"

const root = join(import.meta.dir, "../.."), web = join(root, "apps/openagents-web")
const coax = join(root, "coax/bin/coax")

type Engine = { name: string; coax: string; face: string; wasm: string; libraries: string[]; steps?: string }
// build.sh's wasm.sh lines (with their continuations), and the Grid's build-home.sh units.
function engines(): Engine[] {
  const text = readFileSync(join(web, "build.sh"), "utf8").replace(/\\\n\s*/g, " ")
  const here = (p: string) => p.replace(/"/g, "").replace("$here/../..", root).replace("$here", web).replace("$m", join(root, "verse"))
  const list: Engine[] = []
  for (const line of text.split("\n")) {
    const m = line.match(/coax\/web\/wasm\.sh" (.*?)(?:&|$)/)
    if (!m) continue
    const [coaxFile, face, out, ...libraries] = m[1].trim().split(/\s+/).map(here)
    const steps = line.match(/COAXIAL_RUN_STEPS=(\d+)/)?.[1] // a library whose tables are computed at compile time
    list.push({ name: basename(out, ".wasm"), coax: coaxFile, face, wasm: out, libraries, steps })
  }
  list.push({ name: "newland", coax: join(web, "newland.coax"), face: join(web, "newland-face.c"), wasm: join(web, "build/newland.wasm"),
    libraries: ["coax/web/project/landing-browser.coax", "coax/web/project/landing-fold.coax", "verse/gridwalk.coax"].map(p => join(root, p)) })
  // The Grid's module also links the homepage reader's face, whose globals have defaults too.
  list.push({ name: "landing-reader", coax: join(root, "coax/web/project/landing-browser.coax"), face: join(root, "coax/web/project/landing-reader-face.c"),
    wasm: join(web, "build/newland.wasm"), libraries: [] })
  return list
}

const work = mkdtempSync(join(tmpdir(), "coax-state-defaults-"))
const run = (cmd: string[], cwd = root) => {
  const r = Bun.spawnSync(cmd, { cwd, stdout: "pipe", stderr: "pipe" })
  if (r.exitCode !== 0) throw Error(`${cmd.join(" ")}: ${r.stderr.toString().slice(0, 600)}`)
  return r.stdout
}
const unitName = (file: string, main: boolean) => {
  const b = basename(file, ".coax")
  return main ? b : b[0].toUpperCase() + b.slice(1)
}
async function generate(e: Engine) {
  const dir = join(work, e.name)
  Bun.spawnSync(["mkdir", "-p", dir])
  const units = [{ file: e.coax, name: unitName(e.coax, true) }, ...e.libraries.map(f => ({ file: f, name: unitName(f, false) }))]
  await Promise.all(units.map(async u => {
    const p = Bun.spawn([coax, "c", u.file, "--library", "--header", join(dir, u.name + ".h"), ...(e.steps ? ["--run-steps", e.steps] : [])], { cwd: root, stdout: "pipe", stderr: "pipe" })
    const c = await new Response(p.stdout).text()
    if (await p.exited !== 0) throw Error(`coax c ${u.file}: ${await new Response(p.stderr).text()}`)
    writeFileSync(join(dir, u.name + ".c"), c)
  }))
  return units.map(u => join(dir, u.name + ".c"))
}
// What a face must default, read from the compiler's headers and the face:
// its state (sizeof(coax_t_T), or a face struct of coax members) and its
// file-scope coax globals, each of a type whose header declares coax_default_T.
type Plan = { name: string | null; state: [string, string][]; globals: [string, string, string][]; units: Record<string, string> }
function plan(face: string, units: string[]): Plan {
  const defaulted: Record<string, string> = {} // type -> the unit whose C defines its coax_init
  for (const unit of units) {
    const header = readFileSync(unit.replace(/\.c$/, ".h"), "utf8")
    for (const m of header.matchAll(/^void coax_default_(\w+)\((coax_\w+) \*p\);$/gm)) defaulted[m[2]] ||= unit
  }
  const text = readFileSync(face, "utf8")
  const state: [string, string][] = []
  const size = text.match(/"coaxial_state_size"\)[^{]*\{[^}]*?sizeof\s*\(?\s*(\w+)/)
  const name = size ? size[1] : null
  if (name?.startsWith("coax_")) state.push(["", name])
  else if (name) {
    const typedef = text.match(new RegExp("typedef struct\\s*\\{([^}]*)\\}\\s*" + name + "\\s*;"))
    if (typedef) for (const m of typedef[1].matchAll(/(coax_\w+)\s+(\w+)\s*;/g)) state.push([m[2], m[1]])
  }
  const globals = [...text.matchAll(/^static (coax_\w+) (\w+)(\[[^\]]+\])?;/gm)].map(m => [m[1], m[2], m[3] ?? ""] as [string, string, string])
  const kept = state.filter(([, t]) => t in defaulted), used = globals.filter(([t]) => t in defaulted)
  const needed = new Set([...kept.map(([, t]) => t), ...used.map(([t]) => t)])
  return { name, state: kept, globals: used, units: Object.fromEntries([...needed].map(t => [t, defaulted[t]])) }
}
// The native default of a type: its unit's coax_init, written out as bytes.
function native(unit: string, type: string): Uint8Array {
  const dir = join(work, "native-" + basename(unit, ".c") + "-" + type)
  Bun.spawnSync(["mkdir", "-p", dir])
  writeFileSync(join(dir, "main.c"), `#include "${unit}"
#include <stdio.h>
static ${type} v;
int main(void) { v = ${type.replace("coax_t_", "coax_init_t_")}(); fwrite(&v, sizeof v, 1, stdout); return 0; }
`)
  const loose = process.platform === "darwin" ? "-Wl,-undefined,dynamic_lookup" : "-Wl,--unresolved-symbols=ignore-all"
  run(["cc", "-std=c11", "-O1", "-w", "-ffp-contract=off", "-I" + join(unit, ".."), join(dir, "main.c"), "-o", join(dir, "main"), "-lm", loose])
  return new Uint8Array(run([join(dir, "main")]))
}
// The face built natively with its units: where each defaulted member of a
// face-struct state sits, and every defaulted global's bytes after the face's
// constructors ran, as _initialize runs them in the browser.
function faceNative(e: Engine, units: string[], p: Plan) {
  const dir = join(work, "face-" + e.name)
  Bun.spawnSync(["mkdir", "-p", dir])
  const lines = [`#include "${e.face}"`, "#include <stdio.h>", "#include <string.h>",
    "static void coaxial_hex(const char *n, const void *p, size_t k) { printf(\"%s \", n); for (size_t i = 0; i < k; i++) printf(\"%02x\", ((const unsigned char *)p)[i]); printf(\"\\n\"); }",
    "int main(void) {"]
  for (const [member] of p.state) if (member) lines.push(`  printf("offset ${member} %zu\\n", offsetof(${p.name}, ${member}));`)
  // A global's reference is its type's coax_default_T in this same process, so string defaults
  // (pointers into the unit's data) compare equal; coax/tests/state_defaults.py checks that
  // coax_default_T writes what native coax_init gives.
  for (const [type, name, array] of p.globals) {
    const one = array ? `${name}[0]` : name
    lines.push(`  { static ${type} ref; memset(&ref, 0xab, sizeof ref); ${type.replace(/^coax_/, "coax_default_")}(&ref); coaxial_hex("ref:${name}", &ref, sizeof ref); (void)sizeof ${one}; }`)
    lines.push(`  coaxial_hex("${name}", &${name}, sizeof ${name});`)
  }
  lines.push("  return 0;", "}")
  writeFileSync(join(dir, "main.c"), lines.join("\n") + "\n")
  // The host's imports (and the other faces' data) aren't here. The harness never calls
  // them, so each missing symbol gets an unused stand-in, found by a first link.
  const link = (extra: string[]) => Bun.spawnSync(["cc", "-std=gnu11", "-O0", "-w", "-ffp-contract=off", "-I" + dirname(units[0]), "-I" + dirname(e.face),
    join(dir, "main.c"), ...units, ...extra, "-o", join(dir, "main"), "-lm"], { cwd: root, stdout: "pipe", stderr: "pipe" })
  const first = link([])
  if (first.exitCode !== 0) {
    const err = first.stderr.toString()
    const missing = new Set([...err.matchAll(/"_(\w+)", referenced from/g), ...err.matchAll(/undefined reference to `(\w+)'/g)].map(m => m[1]))
    if (!missing.size) throw Error(`cc ${e.face}: ${err.slice(0, 600)}`)
    writeFileSync(join(dir, "missing.c"), [...missing].map(n => `char ${n}[16];`).join("\n") + "\n")
    const second = link([join(dir, "missing.c")])
    if (second.exitCode !== 0) throw Error(`cc ${e.face}: ${second.stderr.toString().slice(0, 600)}`)
  }
  const offsets: Record<string, number> = {}, globals: Record<string, Uint8Array> = {}
  for (const line of run([join(dir, "main")]).toString().split("\n")) {
    const o = line.match(/^offset (\w+) (\d+)$/)
    if (o) { offsets[o[1]] = Number(o[2]); continue }
    const g = line.match(/^((?:ref:)?\w+) ([0-9a-f]*)$/)
    if (g) globals[g[1]] = Uint8Array.from(g[2].match(/../g) ?? [], h => parseInt(h, 16))
  }
  return { offsets, globals }
}
function stubs(module: WebAssembly.Module, memory: WebAssembly.Memory) {
  const imports: Record<string, Record<string, unknown>> = { env: { memory } }
  for (const i of WebAssembly.Module.imports(module)) {
    if (i.kind === "function") (imports[i.module] ||= {})[i.name] ||= () => 0
  }
  return imports as WebAssembly.Imports
}
// The browser's boot: a zeroed state above the module's data, then coaxial_state_init.
async function booted(wasm: string, init = true) {
  const module = await WebAssembly.compile(readFileSync(wasm))
  const memory = new WebAssembly.Memory({ initial: 128, maximum: 1024 }) // the host's
  for (let pages = 128; pages <= 1024; pages *= 2) {
    try { await WebAssembly.instantiate(module, stubs(module, memory)); break } // the module declares more (the Grid's stack)
    catch (e) { if (!(e instanceof WebAssembly.LinkError)) throw e; memory.grow(pages) }
  }
  const x = (await WebAssembly.instantiate(module, stubs(module, memory))).exports as any
  x._initialize?.()
  const size = x.coaxial_state_size()
  const base = Math.ceil(Math.max(x.__heap_base?.value ?? 0, memory.buffer.byteLength) / 65536) * 65536
  const need = base + size + 65536
  if (need > memory.buffer.byteLength) memory.grow(Math.ceil((need - memory.buffer.byteLength) / 65536))
  new Uint8Array(memory.buffer, base, size).fill(0)
  if (init) x.coaxial_state_init?.(base)
  return { x, bytes: (at: number, n: number) => new Uint8Array(memory.buffer, base + at, n) }
}

test("every engine boots from zeroed memory with native's struct defaults", async () => {
  const list = engines(), failures: string[] = [], checked: string[] = [], globals: string[] = [], sharp: string[] = []
  expect(list.length).toBeGreaterThan(20)
  for (let i = 0; i < list.length; i += 6) {
    await Promise.all(list.slice(i, i + 6).map(async e => {
      if (!existsSync(e.wasm)) { failures.push(`${e.name}: no ${e.wasm} (run apps/openagents-web/build.sh)`); return }
      const units = await generate(e)
      const p = plan(e.face, units)
      if (!p.state.length && !p.globals.length) return
      const refs: Record<string, Uint8Array> = {}
      for (const type of Object.keys(p.units)) {
        refs[type] = native(p.units[type], type)
        // Sharp: every defaulted type has a non-zero default, or a zeroed value would pass.
        if (refs[type].every(b => b === 0)) failures.push(`${e.name}: ${type}'s default is all zeros, so the comparison can't tell it from zeroed memory`)
      }
      const face = faceNative(e, units, p)
      const differs = (have: Uint8Array, ref: Uint8Array) => {
        for (let b = 0; b < ref.length; b++) if (have[b] !== ref[b]) return b
        return have.length === ref.length ? -1 : ref.length
      }
      // The face's globals, natively, after its constructors ran.
      for (const [type, name, array] of p.globals) {
        const have = face.globals[name], ref = face.globals["ref:" + name]
        if (ref && ref.length !== refs[type].length) failures.push(`${e.name}: ${type} has ${ref.length} bytes in the face, ${refs[type].length} natively`)
        if (!ref) { failures.push(`${e.name}: no reference for ${name}`); continue }
        if (!have) { failures.push(`${e.name}: global ${name} was not read`); continue }
        const count = array ? have.length / ref.length : 1
        let bad = -1
        for (let k = 0; k < count && bad < 0; k++) bad = differs(have.subarray(k * ref.length, (k + 1) * ref.length), ref) < 0 ? -1 : k
        if (have.length !== count * ref.length || bad >= 0) failures.push(`${e.name}: global ${name}${array && bad >= 0 ? `[${bad}]` : array} differs from native ${type}`)
        else globals.push(`${e.name} ${name}${array}`)
      }
      if (!p.state.length) return
      const { x, bytes } = await booted(e.wasm)
      if (!x.coaxial_state_init) { failures.push(`${e.name}: no coaxial_state_init export (the face must call coax_default_T on a new state)`); return }
      const zeroed = await booted(e.wasm, false)
      let skipped = true
      for (const [member, type] of p.state) {
        const at = member ? face.offsets[member] : 0, ref = refs[type]
        if (at === undefined) { failures.push(`${e.name}: no offset for ${member}`); continue }
        const bad = differs(bytes(at, ref.length), ref)
        if (bad >= 0) failures.push(`${e.name}: ${type}${member ? " " + member : ""} differs from native at byte ${bad}`)
        else checked.push(`${e.name} ${type}`)
        // The same state left zeroed, as the host hands it, must differ from native (Aethyr engine core, ec-state).
        if (differs(zeroed.bytes(at, ref.length), ref) >= 0) skipped = false
      }
      if (skipped) failures.push(`${e.name}: a zeroed state passes the comparison`)
      else sharp.push(e.name)
    }))
  }
  console.log(`states with defaults (${checked.length}): ${checked.sort().join(", ")}\nface globals with defaults (${globals.length}): ${globals.sort().join(", ")}`)
  expect(failures).toEqual([])
  // Orbit League's gravity is the case that shipped broken; it must be among them.
  expect(checked.some(c => c.startsWith("orbitleague "))).toBe(true)
  expect(sharp).toContain("orbitleague")
  expect(globals.some(g => g.startsWith("newland walk"))).toBe(true)
  rmSync(work, { recursive: true, force: true })
}, 900_000)
