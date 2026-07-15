import { mkdirSync, mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { bundledCodexExecutableSha256 } from "@openagentsinc/codex-app-server-protocol/compatibility"
import { createCodexAppServerSupervisor } from "../src/codex-app-server-supervisor.ts"
import { makeCodexControlPlaneRegistry } from "../src/codex-control-plane.ts"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact packaged Codex executable")

const root = mkdtempSync(join(tmpdir(), "openagents-codex-control-plane-"))
const codexHome = join(root, "home")
mkdirSync(codexHome)
const supervisor = createCodexAppServerSupervisor({
  nativeJournalRoot: join(root, "native"),
  strictGeneratedDecoding: true,
})
const registry = makeCodexControlPlaneRegistry({ supervisor, receiptRoot: join(root, "receipts") })
try {
  const plane = await registry.forTarget({
    binary,
    binarySha256: bundledCodexExecutableSha256,
    env: { ...process.env, CODEX_HOME: codexHome },
    cwd: root,
    accountRef: "control-plane-smoke-isolated",
    hostTarget: "local-desktop-smoke",
  })
  const snapshot = plane.snapshot()
  if (snapshot.models.length === 0) throw new Error("model/list returned no app-server models")
  if (snapshot.config === null) throw new Error("config/read was not projected")
  if (!Array.isArray(snapshot.permissionProfiles) || !Array.isArray(snapshot.experimentalFeatures)) {
    throw new Error("policy control surfaces were not projected")
  }
  const defaultModel = snapshot.models.find(model => model.isDefault && !model.hidden)
    ?? snapshot.models.find(model => !model.hidden)
  if (defaultModel === undefined || !plane.gate({ type: "model", value: defaultModel.id }).allowed) {
    throw new Error("app-server advertised model was not admitted")
  }
  const optionalMethods = new Set([
    "account/rateLimits/read", "account/usage/read", "account/workspaceMessages/read", "collaborationMode/list",
  ])
  if (snapshot.errors.some(error => !optionalMethods.has(error.method))) {
    throw new Error(`unexpected degraded control read: ${JSON.stringify(snapshot.errors)}`)
  }
  console.log(`Verified Codex control plane with ${snapshot.models.length} model(s) and ${snapshot.errors.length} explicit optional-read degradation(s).`)
} finally {
  registry.close()
  supervisor.close()
  rmSync(root, { recursive: true, force: true })
}
