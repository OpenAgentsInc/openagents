import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import path from "node:path"

import { mvpCodexReadyProbe, mvpProofEnvironmentFromArgv, mvpProofRequiredSteps, resolveMvpProofCommand, resolveMvpProofConfig } from "./mvp-proof.ts"

describe("ProductSpec-native MVP proof contract", () => {
  test("requires the exact execution, child, verification, and owner-gate journey", () => {
    expect(mvpProofRequiredSteps).toEqual([
      "shell",
      "codex-ready",
      "product-spec-open",
      "plan-accepted",
      "root-packet-turn",
      "root-packet-verified",
      "child-packet-turn",
      "child-transcript",
      "child-packet-verified",
      "renderer-reload-restored",
      "app-restart-restored",
      "owner-gate-pending",
    ])
  })

  test("is double-gated, mutually exclusive, and refuses unsafe spec paths", () => {
    const userData = "/tmp/openagents-mvp-user-data"
    expect(resolveMvpProofConfig({}, userData).enabled).toBe(false)
    const valid = resolveMvpProofConfig({
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH: "specs/mvp.product-spec.md",
    }, userData)
    expect(valid).toEqual({
      enabled: true,
      conflict: false,
      outDir: path.join(userData, "mvp-proof"),
      specPath: "specs/mvp.product-spec.md",
    })
    for (const specPath of ["", "../escape.product-spec.md", "/tmp/x.product-spec.md", "specs/not-a-spec.md"]) {
      expect(resolveMvpProofConfig({
        OPENAGENTS_DESKTOP_MVP_PROOF: "1",
        OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
        OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH: specPath,
      }, userData).conflict).toBe(true)
    }
    expect(resolveMvpProofConfig({
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_SMOKE: "1",
      OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH: "specs/mvp.product-spec.md",
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
      "--openagents-mvp-proof-spec=specs/mvp.product-spec.md",
      "--openagents-mvp-proof-phase=initial",
    ])).toEqual({
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_MVP_PROOF_DIR: "/tmp/oa proof/receipts",
      OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH: "specs/mvp.product-spec.md",
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

  test("requires agent-produced evidence before independent host verification", () => {
    const source = readFileSync(new URL("./mvp-proof.ts", import.meta.url), "utf8")
    expect(source).toContain('.replace(/\\s+/g, "_") === "evidence_present"')
    expect(source).toContain("packet did not retain agent-produced evidence")
    expect(source).not.toContain('requireClick(`product-spec-evidence-${packetRef}`)')
  })
})
