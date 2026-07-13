import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import path from "node:path"

const root = import.meta.dir
const read = (relative: string): string => readFileSync(path.join(root, relative), "utf8")

describe("Open in Codex trusted Desktop integration", () => {
  test("main owns ProductSpec identity, quiescence, post-image capture, and launch", () => {
    const main = read("main.ts")

    expect(main).toContain("codexHandoffBindings.recordPacketAdmission(result.value, request.packetRef)")
    expect(main).toContain("codexHandoffBindings.bindNextTurn({")
    expect(main).toContain("exactThreadProof: () => null")
    expect(main).toContain("codexLocal.interrupt(request.turnRef)")
    expect(main.indexOf('gitGithubService.run({ op: "status" })')).toBeGreaterThan(main.indexOf("quiesce: async"))
    expect(main).toContain('runOpen(["-a", "Codex", authority.workspaceRoot])')
    expect(main).toContain("if (!isTrustedRuntimeGatewaySender(event))")
    expect(main).toContain("decodeCodexHandoffOpenRequest(raw)")
  })

  test("preload exposes one fixed channel and decodes both sides", () => {
    const preload = read("preload.cts")

    expect(preload).toContain("decodeCodexHandoffOpenRequest(value)")
    expect(preload).toContain("ipcRenderer.invoke(CodexHandoffOpenChannel, request)")
    expect(preload).toContain("decodeCodexHandoffOpenResult(")
  })

  test("renderer requests handoff by refs and renders the bounded control", () => {
    const shell = read("renderer/shell.ts")

    expect(shell).toContain("DesktopCodexHandoffRequested")
    expect(shell).toContain("threadRef: current.activeThreadId!")
    expect(shell).toContain("turnRef,")
    expect(shell).toContain('key: "shell-open-in-codex"')
    expect(shell).toContain('label: "Open in Codex"')
    expect(shell).not.toContain("specDigest: current")
    expect(shell).not.toContain("workPacketRef: current")
  })
})
