// A recorded session replays bit for bit natively and in the browser (aethyr/replay.coax,
// Aethyr book chapter 5). aethyr/tests/replay-walk.coax records a walk, replays it and plants
// a fault; this builds it to wasm the way pages are built (coax/web/wasm.sh: -O2, no fast
// math, no contraction), runs it as the browser would, and compares every number with the
// native run of aethyr/examples/replay-a-walk.coax. The second check does the same for a recorded
// Grid session (verse/tests/grid-session.coax: the Grid's own session code, verse/gridwalk.coax,
// with keys, a drag, the wheel, the touch stick, a pinch and taps) and compares the digest of
// every step. Both sessions turn while they walk through angles where the hosts' sinf and cosf
// round differently, so either check fails if a step calls the C library's sin or cos.
//   bun test coax/web/replay-hosts.test.ts
import { expect, test } from "bun:test"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const root = join(import.meta.dir, "../..")
const run = (cmd: string[]) => {
  const r = Bun.spawnSync(cmd, { cwd: root, stdout: "pipe", stderr: "pipe" })
  if (r.exitCode !== 0) throw Error(`${cmd.join(" ")}: ${r.stderr.toString().slice(0, 800)}`)
  return r.stdout.toString()
}
const numbers = (text: string) => [...text.matchAll(/-?\d+/g)].map(m => BigInt(m[0]))
const instantiate = async (wasm: string) => {
  const module = await WebAssembly.compile(readFileSync(wasm))
  const memory = new WebAssembly.Memory({ initial: 256, maximum: 1024 })
  const imports: Record<string, Record<string, unknown>> = { env: { memory } }
  for (const i of WebAssembly.Module.imports(module)) {
    if (i.kind === "function") (imports[i.module] ||= {})[i.name] ||= () => 0
  }
  const instance = await WebAssembly.instantiate(module, imports as WebAssembly.Imports)
  const e = instance.exports as Record<string, Function>
  e._initialize?.()
  return e
}

test("a recorded walk gives the same digests natively and in wasm", async () => {
  const work = mkdtempSync(join(tmpdir(), "replay-hosts-"))
  try {
    const native = numbers(run(["coax/bin/coax", "run", "aethyr/examples/replay-a-walk.coax"]))
    const wasm = join(work, "replay-walk.wasm")
    run(["coax/web/wasm.sh", "aethyr/tests/replay-walk.coax", "aethyr/tests/replay-walk-face.c", wasm])
    const e = await instantiate(wasm)
    const walk = (mode: number) => BigInt(e.replay_walk(BigInt(mode)))
    const refused = walk(3)
    const browser = [refused / 1000000n, refused / 1000n % 1000n, refused % 1000n, walk(0), walk(1), 150n, walk(2)]
    // the native output: refused (3 numbers), recorded, replayed, "step 150", first divergent step
    expect(browser).toEqual(native)
    expect(native[3]).toBe(native[4])      // the replay matched the recording
    expect(native[3] > 0n).toBe(true)
    expect(native[6]).toBe(150n)           // the planted fault is found where it starts
  } finally {
    rmSync(work, { recursive: true, force: true })
  }
}, 600_000)

test("a recorded Grid session gives the same digest at every step natively and in wasm", async () => {
  const work = mkdtempSync(join(tmpdir(), "grid-hosts-"))
  try {
    const lines = run(["coax/bin/coax", "run", "verse/tests/grid-session.coax"]).trim().split("\n")
    const [records, steps] = numbers(lines[0])
    const native = lines.slice(1, 1 + Number(steps)).map(l => BigInt(l))
    expect(records).toBe(1n)                  // the whole session is one record
    expect(steps > 600n).toBe(true)
    expect(lines[1 + Number(steps)]).toMatch(/^session \d+$/)
    expect(lines[2 + Number(steps)]).toBe("replayed true")
    const wasm = join(work, "grid-session.wasm")
    run(["coax/web/wasm.sh", "verse/tests/grid-session.coax", "verse/tests/grid-session-face.c", wasm])
    const e = await instantiate(wasm)
    expect(BigInt(e.grid_play())).toBe(steps)
    expect(BigInt(e.grid_records())).toBe(1n)
    const browser = native.map((_, k) => BigInt(e.grid_mark(BigInt(k))))
    const first = browser.findIndex((d, k) => d !== native[k])
    expect(first).toBe(-1)                    // no step differs
    expect(BigInt(e.grid_mark(steps))).toBe(-1n)
    expect(`session ${BigInt(e.grid_session())}`).toBe(lines[1 + Number(steps)])
    expect(BigInt(e.grid_replayed())).toBe(1n)
  } finally {
    rmSync(work, { recursive: true, force: true })
  }
}, 600_000)
