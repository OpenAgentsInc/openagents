/**
 * Replay CLI for the Tassadar PoC: re-executes a committed fixture and
 * verdicts the claimed trace digest. Usage:
 *   bun run src/replay-cli.ts <fixture.json> --claimed-digest <hex> \
 *     --validator-device <ref> [--tamper-step <n>]
 */
import { readFileSync } from "node:fs"
import { hostname } from "node:os"
import { verifyTassadarFullReplay, verifyTassadarWindow } from "./replay.js"
import { executeTassadarNumericModel } from "./numeric-executor.js"

const args = process.argv.slice(2)
const fixturePath = args[0]
const flag = (name: string): string | undefined => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : undefined
}

if (!fixturePath) {
  console.error("usage: replay-cli <fixture.json> [--claimed-digest <hex>] [--validator-device <ref>] [--tamper-step <n>]")
  process.exit(2)
}

const fixture = JSON.parse(readFileSync(fixturePath, "utf8"))
const validatorDeviceRef =
  flag("--validator-device") ?? `device.${hostname().toLowerCase()}`
const claimedDigest = flag("--claimed-digest") ?? fixture.expectedTraceDigest
const tamperStep = flag("--tamper-step")

const run = async () => {
  if (tamperStep !== undefined) {
    const trace = await executeTassadarNumericModel(fixture.model, fixture.steps)
    const step = Number(tamperStep)
    const claimedRows = trace.stepOutputs
      .slice(step, step + 3)
      .map((row, offset) =>
        offset === 1 ? row.map((value, i) => (i === 1 ? value + 1n : value)) : row,
      )
    const verdict = await verifyTassadarWindow({
      claimedRows,
      model: fixture.model,
      steps: fixture.steps,
      validatorDeviceRef,
      windowStart: step,
    })
    console.log(JSON.stringify({ mode: "tampered_window_demo", verdict }, (_, v) => (typeof v === "bigint" ? v.toString() : v), 2))
    process.exit(verdict.outcome === "rejected" ? 0 : 1)
  }
  const verdict = await verifyTassadarFullReplay({
    claimedTraceDigest: claimedDigest,
    model: fixture.model,
    steps: fixture.steps,
    validatorDeviceRef,
  })
  console.log(JSON.stringify({ mode: "full_replay", verdict }, (_, v) => (typeof v === "bigint" ? v.toString() : v), 2))
  process.exit(verdict.outcome === "verified" ? 0 : 1)
}

run()
