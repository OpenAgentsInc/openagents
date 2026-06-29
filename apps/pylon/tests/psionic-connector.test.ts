import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { inspectPsionicConnector } from "../src/psionic-connector"
import { assertPublicProjectionSafe, ensurePylonLocalState, projectPublicStatus } from "../src/state"

async function withTempDir<T>(fn: (dir: string) => Promise<T>) {
  const dir = await mkdtemp(join(tmpdir(), "pylon-psionic-connector-test-"))
  try {
    return await fn(dir)
  } finally {
    await rm(dir, { recursive: true, force: true })
  }
}

describe("Pylon Psionic connector state", () => {
  test("reports absent without Psionic hints and does not probe or download on startup", async () => {
    let calls = 0
    const state = await inspectPsionicConnector({
      env: { PATH: "" },
      fetch: async () => {
        calls += 1
        throw new Error("should not probe")
      },
      now: new Date("2026-06-10T00:00:00.000Z"),
    })

    expect(calls).toBe(0)
    expect(state.phase).toBe("absent")
    expect(state.downloadsOnStartup).toBe(false)
    expect(state.blockerRefs).toContain("blocker.psionic_qwen35.connector_unconfigured")
    expect(state.service.configured).toBe(false)
    expect(state.binary.configured).toBe(false)
    assertPublicProjectionSafe(state)
  })

  test("reports configured when a Psionic binary is discoverable but no service endpoint is configured", async () => {
    await withTempDir(async (dir) => {
      await writeFile(join(dir, "psionic-openai-server"), "#!/bin/sh\n")
      const state = await inspectPsionicConnector({
        env: { PATH: dir },
        now: new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(state.phase).toBe("configured")
      expect(state.binary).toMatchObject({
        configured: true,
        binaryRef: "binary.psionic.psionic_openai_server",
        sourceRef: "source.psionic.binary.path",
      })
      expect(state.service.configured).toBe(false)
      expect(state.blockerRefs).toContain("blocker.psionic_qwen35.service_unconfigured")
      expect(JSON.stringify(state)).not.toContain(dir)
      assertPublicProjectionSafe(state)
    })
  })

  test("negotiates capability refs for a ready configured Psionic service", async () => {
    const state = await inspectPsionicConnector({
      env: {
        PATH: "",
        PYLON_PSIONIC_BASE_URL: "http://127.0.0.1:18080",
      },
      fetch: fakePsionicFetch({
        health: {
          ready: true,
          execution_engine: "psionic",
          supported_endpoints: ["/v1/chat/completions", "/v1/responses"],
        },
        models: {
          data: [
            {
              id: "qwen3.5-0.8b",
              artifact_digest: "afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5",
            },
            {
              id: "qwen3.5-2b",
              artifact_manifest_ref: "artifact.psionic.qwen35.2b.q8_0.manifest",
            },
          ],
        },
      }),
      now: new Date("2026-06-10T00:00:00.000Z"),
    })

    expect(state.phase).toBe("negotiated")
    expect(state.blockerRefs).toEqual([])
    expect(state.capabilityRefs).toContain("capability.psionic.connector.attach_existing")
    expect(state.modelRefs).toContain("model.psionic.qwen35.0_8b.q8_0")
    expect(state.modelRefs).toContain("model.psionic.qwen35.2b.q8_0")
    expect(state.service.endpointRefs).toContain("endpoint.psionic.v1.chat_completions")
    expect(state.service.endpointRefs).toContain("endpoint.psionic.v1.responses")
    expect(state.receiptRefs[0]).toMatch(/^receipt\.psionic\.qwen35\.availability\./)
    expect(JSON.stringify(state)).not.toContain("18080")
    assertPublicProjectionSafe(state)
  })

  test("refuses explicit bad service and explicit bad binary with typed blocker refs", async () => {
    const unreachable = await inspectPsionicConnector({
      env: {
        PATH: "",
        PYLON_PSIONIC_BASE_URL: "http://127.0.0.1:18081",
      },
      fetch: async () => {
        throw new Error("connection refused")
      },
      now: new Date("2026-06-10T00:00:00.000Z"),
    })
    const badBinary = await inspectPsionicConnector({
      env: {
        PATH: "",
        PYLON_PSIONIC_BIN: "/tmp/pylon-test-missing-psionic",
      },
      now: new Date("2026-06-10T00:00:00.000Z"),
    })

    expect(unreachable.phase).toBe("refused")
    expect(unreachable.blockerRefs).toContain("blocker.psionic_qwen35.health_unreachable")
    expect(unreachable.refusalRefs).toContain("blocker.psionic_qwen35.health_unreachable")
    expect(badBinary.phase).toBe("refused")
    expect(badBinary.blockerRefs).toContain("blocker.psionic_qwen35.binary_missing")
    expect(badBinary.blockerRefs).toContain("blocker.psionic_qwen35.service_unconfigured")
    assertPublicProjectionSafe(unreachable)
    assertPublicProjectionSafe(badBinary)
  })

  test("projects connector state in public status without leaking configured endpoints", async () => {
    await withTempDir(async (home) => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Psionic Status"]),
        { PYLON_HOME: home },
        "linux",
      )
      const localState = await ensurePylonLocalState(summary)
      const connector = await inspectPsionicConnector({
        env: {
          PATH: "",
          PYLON_PSIONIC_BASE_URL: "http://127.0.0.1:18080",
        },
        fetch: fakePsionicFetch({
          health: {
            ready: true,
            execution_engine: "psionic",
            supported_endpoints: ["/v1/chat/completions"],
          },
          models: {
            data: [
              {
                id: "qwen3.5-2b",
                artifact_manifest_ref: "artifact.psionic.qwen35.2b.q8_0.manifest",
              },
            ],
          },
        }),
        now: new Date("2026-06-10T00:00:00.000Z"),
      })
      const projected = projectPublicStatus(localState, undefined, connector)
      const json = JSON.stringify(projected)

      expect(projected.state.psionicConnector?.phase).toBe("negotiated")
      expect(json).not.toContain("18080")
      expect(json).not.toContain("PYLON_PSIONIC_BASE_URL")
      expect(json).not.toContain("private")
      assertPublicProjectionSafe(projected)
    })
  })
})

function fakePsionicFetch(input: {
  health: Record<string, unknown>
  models: Record<string, unknown>
}): typeof fetch {
  return async (request) => {
    const url = new URL(request instanceof Request ? request.url : String(request))
    if (url.pathname === "/health") {
      return Response.json(input.health)
    }
    if (url.pathname === "/v1/models") {
      return Response.json(input.models)
    }
    return Response.json({ error: "not found" }, { status: 404 })
  }
}
