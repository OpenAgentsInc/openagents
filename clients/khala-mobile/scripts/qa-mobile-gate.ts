import { mkdir, writeFile } from "node:fs/promises"
import { dirname, resolve, relative } from "node:path"
import { spawn } from "node:child_process"

type GateCheckStatus = "pass" | "fail"

type GateCheck = Readonly<{
  command: readonly string[]
  id: string
  name: string
}>

type GateCheckReceipt = Readonly<{
  command: readonly string[]
  durationMs: number
  exitCode: number | null
  id: string
  name: string
  stderrTail: string
  stdoutTail: string
  status: GateCheckStatus
}>

type MobileGateReceipt = Readonly<{
  checks: readonly GateCheckReceipt[]
  generatedAt: string
  ok: boolean
  rollbackPosture: readonly string[]
  schema: "openagents.khala_mobile.release_gate.v1"
}>

const repoRoot = resolve(import.meta.dir, "../../..")
const mobileRoot = resolve(import.meta.dir, "..")
const receiptPath = resolve(
  repoRoot,
  process.env.KHALA_MOBILE_GATE_RECEIPT ??
    "var/qa-mobile-gate/khala-mobile-release-gate.latest.json",
)

const tail = (value: string): string => {
  const maxLength = 6000
  return value.length <= maxLength ? value : value.slice(value.length - maxLength)
}

const runCheck = async (check: GateCheck): Promise<GateCheckReceipt> => {
  const started = Date.now()
  const [cmd, ...args] = check.command
  if (cmd === undefined) throw new Error(`Gate check ${check.id} has no command`)
  let stdout = ""
  let stderr = ""
  const exitCode = await new Promise<number | null>((resolveExit, reject) => {
    const child = spawn(cmd, args, {
      cwd: mobileRoot,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    })
    child.stdout.on("data", chunk => {
      const text = String(chunk)
      stdout += text
      process.stdout.write(text)
    })
    child.stderr.on("data", chunk => {
      const text = String(chunk)
      stderr += text
      process.stderr.write(text)
    })
    child.on("error", reject)
    child.on("close", resolveExit)
  })
  return {
    command: check.command,
    durationMs: Date.now() - started,
    exitCode,
    id: check.id,
    name: check.name,
    stderrTail: tail(stderr),
    stdoutTail: tail(stdout),
    status: exitCode === 0 ? "pass" : "fail",
  }
}

const checks: readonly GateCheck[] = [
  {
    command: ["bun", "run", "typecheck"],
    id: "static.typecheck",
    name: "Static TypeScript check",
  },
  {
    command: ["bun", "run", "architecture:check"],
    id: "static.dependency_cruiser",
    name: "Dependency Cruiser architecture check",
  },
  {
    command: ["bun", "test"],
    id: "unit.mounts.contracts.generator_fixture",
    name: "Unit, mount, behavior-contract, generator-conformance, and fixture-tier tests",
  },
]

const receipts: GateCheckReceipt[] = []
for (const check of checks) {
  console.log(`\n[qa:mobile:gate] ${check.id} — ${check.name}`)
  const receipt = await runCheck(check)
  receipts.push(receipt)
  if (receipt.status === "fail") break
}

const report: MobileGateReceipt = {
  checks: receipts,
  generatedAt: new Date().toISOString(),
  ok: receipts.length === checks.length && receipts.every(receipt => receipt.status === "pass"),
  rollbackPosture: [
    "OTA JavaScript regression: hold or republish through OpenAgents Updates only.",
    "Native regression: hold the store build; do not use EAS or hosted submit.",
    "Contract or waiver change: keep pending blockers named until an oracle lands.",
  ],
  schema: "openagents.khala_mobile.release_gate.v1",
}

await mkdir(dirname(receiptPath), { recursive: true })
await writeFile(receiptPath, `${JSON.stringify(report, null, 2)}\n`)

const displayPath = relative(repoRoot, receiptPath)
console.log(`\n[qa:mobile:gate] receipt: ${displayPath}`)
if (!report.ok) {
  console.error("[qa:mobile:gate] FAILED")
  process.exit(1)
}
console.log("[qa:mobile:gate] PASS")
