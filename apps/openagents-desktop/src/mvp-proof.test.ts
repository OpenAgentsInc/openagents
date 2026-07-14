import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"

import { mvpCodexReadyProbe, mvpProofEnvironmentFromArgv, mvpProofRequiredSteps, resolveMvpProofCommand, resolveMvpProofConfig } from "./mvp-proof.ts"

describe("installed MVP coding proof contract", () => {
  test("requires exact root and child coding artifacts across reload and restart", () => {
    expect(mvpProofRequiredSteps).toEqual([
      "shell",
      "codex-ready",
      "root-coding-turn",
      "root-artifact-verified",
      "child-coding-turn",
      "child-transcript",
      "child-artifact-verified",
      "renderer-reload-restored",
      "app-restart-restored",
    ])
  })

  test("is double-gated and mutually exclusive with other drivers", () => {
    const userData = "/tmp/openagents-mvp-user-data"
    expect(resolveMvpProofConfig({}, userData).enabled).toBe(false)
    const valid = resolveMvpProofConfig({
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
    }, userData)
    expect(valid).toEqual({
      enabled: true,
      conflict: false,
      outDir: path.join(userData, "mvp-proof"),
    })
    expect(resolveMvpProofConfig({
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_SMOKE: "1",
    }, userData).conflict).toBe(true)
  })

  test("proves the fixed ordinary Codex session without retired provider or account selectors", () => {
    expect(mvpCodexReadyProbe).toContain("shell-codex-engine")
    expect(mvpCodexReadyProbe).toContain("shell-note")
    expect(mvpCodexReadyProbe).toContain("send.disabled")
    expect(mvpCodexReadyProbe).not.toContain("shell-harness-select")
    expect(mvpCodexReadyProbe).not.toContain("CODEX_HOME")
    expect(mvpCodexReadyProbe).not.toContain("account")
  })

  test("launches with the package-owned Electron unless an installed executable is explicit", () => {
    expect(resolveMvpProofCommand(undefined, "/repo/apps/openagents-desktop")).toEqual([
      "/repo/apps/openagents-desktop/node_modules/.bin/electron",
      ".",
    ])
    expect(resolveMvpProofCommand(" /Applications/OpenAgents.app/Contents/MacOS/OpenAgents ", "/repo/app")).toEqual([
      "/Applications/OpenAgents.app/Contents/MacOS/OpenAgents",
    ])
  })

  test("reconstructs the closed isolated proof environment from packaged-app argv", () => {
    expect(mvpProofEnvironmentFromArgv([
      "/Applications/OpenAgents.app/Contents/MacOS/OpenAgents",
      "--openagents-mvp-proof",
      "--openagents-mvp-proof-user-data=/tmp/oa proof/user-data",
      "--openagents-mvp-proof-workspace=/tmp/oa proof/workspace",
      "--openagents-mvp-proof-receipts=/tmp/oa proof/receipts",
      "--openagents-mvp-proof-phase=initial",
    ])).toEqual({
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_MVP_PROOF_DIR: "/tmp/oa proof/receipts",
      OPENAGENTS_DESKTOP_MVP_PROOF_PHASE: "initial",
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_WORKSPACE_ROOT: "/tmp/oa proof/workspace",
      OPENAGENTS_DESKTOP_USER_DATA: "/tmp/oa proof/user-data",
    })
    expect(mvpProofEnvironmentFromArgv(["OpenAgents", "--openagents-mvp-proof"])).toBeNull()
    expect(mvpProofEnvironmentFromArgv(["OpenAgents"])).toBeNull()
  })

  test("waits across Effect Native render boundaries before declaring a proof control unavailable", () => {
    const source = readFileSync(new URL("./mvp-proof.ts", import.meta.url), "utf8")
    expect(source).toContain('await poll(click(key), value => value["clicked"] === true, 30_000)')
    expect(source).not.toContain('asRec(await evaluate(click(key)))')
  })

  test("uses current visible chat controls without restoring hidden spec tooling", () => {
    const source = readFileSync(new URL("./mvp-proof.ts", import.meta.url), "utf8")
    expect(source).toContain('requireField("shell-input", rootPrompt)')
    expect(source).toContain('requireClick("shell-note")')
    expect(source).toContain('verifyArtifact("root")')
    expect(source).toContain('verifyArtifact("child")')
    expect(source).not.toContain("proof for ${options.specPath}")
    expect(source).not.toContain('requireClick("workspace-product-spec")')
  })
})
