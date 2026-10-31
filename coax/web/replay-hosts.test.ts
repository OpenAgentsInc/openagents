// A recorded session replays bit for bit natively and in the browser (aethyr/replay.coax,
// Aethyr book chapter 5). aethyr/tests/replay-walk.coax records a walk, replays it and plants
// a fault; this builds it to wasm the way pages are built (coax/web/wasm.sh: -O2, no fast
// math, no contraction), runs it as the browser would, and compares every number with the
// native run of aethyr/examples/replay-a-walk.coax.
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

test("a recorded walk gives the same digests natively and in wasm", async () => {
  const work = mkdtempSync(join(tmpdir(), "replay-hosts-"))
  try {
    const native = numbers(run(["coax/bin/coax", "run", "aethyr/examples/replay-a-walk.coax"]))
    const wasm = join(work, "replay-walk.wasm")
    run(["coax/web/wasm.sh", "aethyr/tests/replay-walk.coax", "aethyr/tests/replay-walk-face.c", wasm])
    const module = await WebAssembly.compile(readFileSync(wasm))
    const memory = new WebAssembly.Memory({ initial: 256, maximum: 1024 })
    const imports: Record<string, Record<string, unknown>> = { env: { memory } }
    for (const i of WebAssembly.Module.imports(module)) {
      if (i.kind === "function") (imports[i.module] ||= {})[i.name] ||= () => 0
    }
    const instance = await WebAssembly.instantiate(module, imports as WebAssembly.Imports)
    const e = instance.exports as Record<string, Function>
    e._initialize?.()
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
