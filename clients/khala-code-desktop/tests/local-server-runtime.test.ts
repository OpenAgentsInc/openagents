import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_LOCAL_SERVER_CAPABILITIES,
  projectKhalaCodeLocalServerManager,
} from "../src/shared/local-server-runtime"
import type { KhalaCodeDesktopRuntimeStatus } from "../src/shared/rpc"

const status = (
  capability: KhalaCodeDesktopRuntimeStatus["capability"],
  state: KhalaCodeDesktopRuntimeStatus["status"],
  reason: string,
): KhalaCodeDesktopRuntimeStatus => ({
  app: "Khala Code Desktop",
  available: state === "ready",
  capability,
  observedAt: "2026-07-05T00:00:00.000Z",
  ok: true,
  reason,
  status: state,
})

describe("Khala Code local server runtime contract", () => {
  test("defines the required OpenCode-parity server capabilities", () => {
    expect(KHALA_CODE_LOCAL_SERVER_CAPABILITIES.map(capability => capability.id)).toEqual([
      "health",
      "auth",
      "origin_policy",
      "identity",
      "project_routes",
      "session_routes",
      "provider_models",
      "stream_events",
      "tool_calls",
      "permissions",
      "file_api",
      "terminal_api",
      "review_api",
      "lifecycle",
    ])
    expect(KHALA_CODE_LOCAL_SERVER_CAPABILITIES.every(capability => capability.required)).toBe(true)
  })

  test("projects Pylon, Codex app-server bridge, and AI SDK Core without making Codex the whole boundary", () => {
    const projection = projectKhalaCodeLocalServerManager({
      runtimeStatuses: [
        status("pylon", "ready", "Pylon is online."),
        status("codex_harness", "ready", "Codex app-server ready."),
        status("coding", "ready", "Coding runtime ready."),
      ],
    })

    expect(projection.defaultRuntime).toBe("khala_local_server")
    expect(projection.ownershipBoundary).toContain("Khala owns the local server contract")
    expect(projection.ownershipBoundary).toContain("Codex app-server remains an important bridge")
    expect(projection.rows.find(row => row.kind === "khala_local_server")?.state).toBe("planned")
    expect(projection.rows.find(row => row.kind === "pylon")?.state).toBe("ready")
    expect(projection.rows.find(row => row.kind === "codex_app_server")?.reason).toContain(
      "not the whole Khala server strategy",
    )
    expect(projection.rows.find(row => row.kind === "ai_sdk_core")?.reason).toContain("AI SDK stream parts")
  })

  test("redacts credential-like runtime details from manager rows", () => {
    const projection = projectKhalaCodeLocalServerManager({
      runtimeStatuses: [
        status("pylon", "error", "failed with Authorization Bearer sk-test-secret-token"),
      ],
    })

    const pylon = projection.rows.find(row => row.kind === "pylon")
    expect(pylon?.state).toBe("unavailable")
    expect(pylon?.detail).not.toContain("sk-test-secret-token")
    expect(projection.credentialPolicy).toContain("Remote server credentials stay out")
  })
})
