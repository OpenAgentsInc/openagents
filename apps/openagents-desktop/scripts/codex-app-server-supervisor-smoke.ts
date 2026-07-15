import { mkdirSync, mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { bundledCodexExecutableSha256 } from "@openagentsinc/codex-app-server-protocol/compatibility"
import { decodeBundledServerRequestResponse } from "@openagentsinc/codex-app-server-protocol/decode"
import { createCodexAppServerSupervisor } from "../src/codex-app-server-supervisor.ts"
import { denyCodexReverseRpc } from "../src/codex-reverse-rpc-arbiter.ts"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact packaged Codex executable")

const root = mkdtempSync(join(tmpdir(), "openagents-codex-supervisor-"))
const codexHome = join(root, "home")
mkdirSync(codexHome)
const supervisor = createCodexAppServerSupervisor({
  nativeJournalRoot: join(root, "native"),
  strictGeneratedDecoding: true,
})
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
  if (first.nativeEnvelopes({ method: "thread/start" }).length !== 2) {
    throw new Error("strict generated decoding did not retain both thread/start responses")
  }
  for (const method of [
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/permissions/requestApproval",
    "item/tool/requestUserInput",
    "item/tool/call",
    "mcpServer/elicitation/request",
    "currentTime/read",
  ]) {
    if (decodeBundledServerRequestResponse(method, denyCodexReverseRpc(method))._tag !== "Decoded") {
      throw new Error(`generated reverse-RPC smoke failed for ${method}`)
    }
  }
  first.release()
  second.release()
  console.log(`Verified one Codex app-server generation multiplexed ${leftId} and ${rightId}.`)
} finally {
  supervisor.close()
  rmSync(root, { recursive: true, force: true })
}
