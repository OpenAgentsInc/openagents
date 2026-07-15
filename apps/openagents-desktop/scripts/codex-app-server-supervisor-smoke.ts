import { mkdirSync, mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { bundledCodexExecutableSha256 } from "@openagentsinc/codex-app-server-protocol/compatibility"
import { createCodexAppServerSupervisor } from "../src/codex-app-server-supervisor.ts"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact packaged Codex executable")

const root = mkdtempSync(join(tmpdir(), "openagents-codex-supervisor-"))
const codexHome = join(root, "home")
mkdirSync(codexHome)
const supervisor = createCodexAppServerSupervisor()
try {
  const target = {
    binary,
    binarySha256: bundledCodexExecutableSha256,
    env: { ...process.env, CODEX_HOME: codexHome },
    cwd: root,
    accountRef: "smoke-isolated",
    hostTarget: "local-desktop-smoke",
  }
  const [first, second] = await Promise.all([
    supervisor.acquire(target),
    supervisor.acquire(target),
  ])
  const start = (lease: typeof first) => lease.request("thread/start", {
    model: "gpt-5.6-sol",
    cwd: root,
    approvalPolicy: "never",
    sandbox: "read-only",
    ephemeral: true,
    threadSource: "appServer",
  }) as Promise<{ thread?: { id?: string } }>
  const [left, right] = await Promise.all([start(first), start(second)])
  const leftId = left.thread?.id
  const rightId = right.thread?.id
  if (!leftId || !rightId || leftId === rightId) {
    throw new Error("two-thread multiplex smoke did not return distinct thread ids")
  }
  if (first.state().status !== "ready" || first.state().generation !== second.state().generation) {
    throw new Error("leases did not share one ready connection generation")
  }
  first.release()
  second.release()
  console.log(`Verified one Codex app-server generation multiplexed ${leftId} and ${rightId}.`)
} finally {
  supervisor.close()
  rmSync(root, { recursive: true, force: true })
}
