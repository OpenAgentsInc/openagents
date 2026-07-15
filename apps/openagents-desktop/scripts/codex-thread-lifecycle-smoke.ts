import { mkdirSync, mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { bundledCodexExecutableSha256 } from "@openagentsinc/codex-app-server-protocol/compatibility"
import { createCodexAppServerSupervisor } from "../src/codex-app-server-supervisor.ts"
import { makeCodexThreadLifecycleRegistry } from "../src/codex-thread-lifecycle.ts"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact packaged Codex executable")
const root = mkdtempSync(join(tmpdir(), "openagents-codex-lifecycle-"))
const home = join(root, "home"); mkdirSync(home)
const supervisor = createCodexAppServerSupervisor({ nativeJournalRoot: join(root, "native"), strictGeneratedDecoding: true })
const registry = makeCodexThreadLifecycleRegistry({ supervisor, receiptRoot: join(root, "receipts") })
try {
  const lifecycle = await registry.forTarget({ binary, binarySha256: bundledCodexExecutableSha256, env: { ...process.env, CODEX_HOME: home }, cwd: root, accountRef: "lifecycle-smoke", hostTarget: "local-desktop-smoke" })
  const started = await lifecycle.start({ model: "gpt-5.6-sol", cwd: root, approvalPolicy: "never", sandbox: "read-only", ephemeral: false, threadSource: "appServer" }) as { thread?: { id?: string } }
  const threadId = started.thread?.id
  if (!threadId) throw new Error("thread/start omitted thread id")
  const thread = await lifecycle.read(threadId, true)
  if (thread.id !== threadId || thread.limitations.includes("ephemeral_history")) throw new Error("persistent thread projection was not honest")
  const page = await lifecycle.runHistory({ kind: "history_page", sessionsRoot: "/unused", threadRef: threadId, offset: 0, limit: 10 }) as { selectedThreadRef?: string } | null
  if (page?.selectedThreadRef !== threadId) throw new Error("native history projection did not round-trip")
  const ephemeral = await lifecycle.start({ model: "gpt-5.6-sol", cwd: root, approvalPolicy: "never", sandbox: "read-only", ephemeral: true, threadSource: "appServer" }) as { thread?: { id?: string } }
  if (!ephemeral.thread?.id || !(await lifecycle.read(ephemeral.thread.id)).limitations.includes("ephemeral_history")) throw new Error("ephemeral limitation was not visible")
  console.log(`Verified app-server lifecycle start/read/native-history plus explicit ephemeral limits for ${threadId}.`)
} finally {
  registry.close(); supervisor.close(); rmSync(root, { recursive: true, force: true })
}
