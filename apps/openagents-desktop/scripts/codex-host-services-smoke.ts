import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createCodexAppServerSupervisor } from "../src/codex-app-server-supervisor.ts"
import { makeCodexHostServices } from "../src/codex-host-services.ts"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact installed Codex executable")

const root = mkdtempSync(join(tmpdir(), "oa-codex-host-smoke-"))
const supervisor = createCodexAppServerSupervisor({ strictGeneratedDecoding: true })
try {
  writeFileSync(join(root, "seed.txt"), "seed", { mode: 0o600 })
  const lease = await supervisor.acquire({ binary, env: process.env, cwd: root, accountRef: "codex-current", hostTarget: "cap09-smoke" })
  const host = makeCodexHostServices({ lease, workspaceRoot: root, spoolRoot: join(root, ".spool"), receiptPath: join(root, ".receipts.json") })

  const body = Buffer.from("written through exact app-server").toString("base64")
  const writePayload = { relativePath: "live.txt", dataBase64: body }
  await host.writeFile("live.txt", body, host.authorize("fs_mutation", writePayload, host.snapshot().revision))
  const read = await host.readFile("live.txt")
  if (Buffer.from(read.dataBase64, "base64").toString("utf8") !== "written through exact app-server") throw new Error("live fs round trip differed")

  const input = { command: [process.execPath, "-e", "process.stdout.write('cap09-command-ok')"], cwd: ".", timeoutMs: 30_000 }
  const processId = await host.exec(input, host.authorize("command", input, host.snapshot().revision))
  const settled = await new Promise<ReturnType<typeof host.snapshot>["commands"][number]>((resolve, reject) => {
    const timeout = setTimeout(() => { remove(); reject(new Error("live command did not settle")) }, 30_000)
    const remove = host.subscribe(snapshot => {
      const command = snapshot.commands.find(value => value.processId === processId)
      if (command !== undefined && command.state !== "running") { clearTimeout(timeout); remove(); resolve(command) }
    })
  })
  if (settled.state !== "settled" || !settled.stdoutPreview.includes("cap09-command-ok")) throw new Error(`live command failed: ${JSON.stringify(settled)}`)
  if (readFileSync(join(root, "live.txt"), "utf8") !== "written through exact app-server") throw new Error("live fs write missing on host")
  host.close()
  console.log("Verified bounded filesystem and command services through the exact installed Codex app-server.")
} finally {
  supervisor.close()
  rmSync(root, { recursive: true, force: true })
}
