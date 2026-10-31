// Every browser engine boots from zeroed memory, as the host gives it, and
// must end with the struct defaults native gives `s: [1] T` (Orbit League ran
// without gravity for a day: spin_g = 1.0 read 0 in the browser). For each
// engine build.sh makes, this runs its built module's coaxial_state_init on a
// zeroed state and compares every defaulted part, byte for byte, with the
// native coax_init of that type. Needs apps/openagents-web/build.sh first.
import { expect, test } from "bun:test"
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { basename, join } from "node:path"

const root = join(import.meta.dir, "../.."), web = join(root, "apps/openagents-web")
const coax = join(root, "coax/bin/coax"), defaults = join(root, "coax/web/defaults.py")

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
    libraries: ["coax/web/project/landing-browser.coax", "coax/web/project/landing-fold.coax", "verse/player.coax", "verse/grid.coax"].map(p => join(root, p)) })
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
function stubs(module: WebAssembly.Module, memory: WebAssembly.Memory) {
  const imports: Record<string, Record<string, unknown>> = { env: { memory } }
  for (const i of WebAssembly.Module.imports(module)) {
    if (i.kind === "function") (imports[i.module] ||= {})[i.name] ||= () => 0
  }
  return imports as WebAssembly.Imports
}
// The browser's boot: a zeroed state above the module's data, then coaxial_state_init.
async function booted(wasm: string) {
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
  x.coaxial_state_init?.(base)
  return { x, bytes: (at: number, n: number) => new Uint8Array(memory.buffer, base + at, n) }
}

test("every engine boots from zeroed memory with native's struct defaults", async () => {
  const list = engines(), failures: string[] = [], checked: string[] = [], globals: string[] = []
  expect(list.length).toBeGreaterThan(20)
  for (let i = 0; i < list.length; i += 6) {
    await Promise.all(list.slice(i, i + 6).map(async e => {
      if (!existsSync(e.wasm)) { failures.push(`${e.name}: no ${e.wasm} (run apps/openagents-web/build.sh)`); return }
      const units = await generate(e)
      const plan = JSON.parse(run(["python3", defaults, "--plan", e.face, ...units]).toString())
      if (plan.globals.length) globals.push(`${e.name}: ${plan.globals.map((g: string[]) => g[1] + g[2]).join(" ")}`)
      if (!plan.state.length) return
      const refs = plan.state.map(([, type]: [string, string]) => native(plan.units[type], type))
      const { x, bytes } = await booted(e.wasm)
      if (!x.coaxial_state_init) { failures.push(`${e.name}: no coaxial_state_init export (built before defaults.py?)`); return }
      plan.state.forEach(([member, type]: [string, string], k: number) => {
        const at = x.coaxial_state_default(k), ref = refs[k], got = bytes(at, ref.length)
        let bad = -1
        for (let b = 0; b < ref.length; b++) if (got[b] !== ref[b]) { bad = b; break }
        if (bad >= 0) failures.push(`${e.name}: ${type}${member ? " " + member : ""} differs from native at byte ${bad}`)
        else checked.push(`${e.name} ${type}`)
      })
    }))
  }
  console.log(`states with defaults: ${checked.sort().join(", ")}\nface globals with defaults: ${globals.sort().join("; ")}`)
  expect(failures).toEqual([])
  // Orbit League's gravity is the case that shipped broken; it must be among them.
  expect(checked.some(c => c.startsWith("orbitleague "))).toBe(true)
  rmSync(work, { recursive: true, force: true })
}, 900_000)
