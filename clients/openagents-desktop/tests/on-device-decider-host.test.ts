import { describe, expect, test } from "bun:test"
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"

import { createOnDeviceDeciderService } from "../src/bun/on-device-decider.js"

const fixedNow = "2026-06-29T00:00:00.000Z"

const request = {
  maxToolSelections: 2,
  modelCandidates: [
    { id: "codex-main" },
    { id: "local-small" },
  ],
  taskSummary: "Inspect a public TypeScript file and choose bounded tools.",
  toolCandidates: [
    { name: "list_files" },
    { name: "read_file" },
  ],
}

const spawnNever = (() => {
  throw new Error("spawn should not be needed for this test")
}) as unknown as typeof Bun.spawn

const createPackagedHelper = () => {
  const resourcesDir = mkdtempSync(join(tmpdir(), "openagents-decider-"))
  const helperPath = join(
    resourcesDir,
    "app",
    "apple-fm-bridge",
    "foundation-bridge",
  )
  mkdirSync(dirname(helperPath), { recursive: true })
  writeFileSync(helperPath, "#!/usr/bin/env bash\n")
  chmodSync(helperPath, 0o755)
  return { helperPath, resourcesDir }
}

describe("openagents desktop on-device decider host", () => {
  test("does not initialize a backend without explicit opt-in", async () => {
    const service = createOnDeviceDeciderService({
      arch: "arm64",
      env: {},
      now: () => fixedNow,
      platform: "darwin",
      spawn: spawnNever,
    })

    const status = await service.status()
    const decision = await service.decide(request)

    expect(status.state).toBe("disabled")
    expect(status.enabled).toBe(false)
    expect(decision.ok).toBe(false)
    expect(decision.status.state).toBe("disabled")
  })

  test("fails soft when Apple FM is enabled on unsupported hardware", async () => {
    const service = createOnDeviceDeciderService({
      arch: "x64",
      env: { OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER: "apple_fm" },
      now: () => fixedNow,
      platform: "linux",
      spawn: spawnNever,
    })

    const status = await service.status()

    expect(status.available).toBe(false)
    expect(status.state).toBe("not_supported")
    expect(status.blockerRefs).toContain(
      "blocker.openagents_desktop.on_device_decider.apple_fm.unsupported_platform",
    )
  })

  test("uses Apple FM chat completions as a small JSON decider when ready", async () => {
    const fetchFn = (async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.endsWith("/health")) {
        return Response.json({ ready: true })
      }
      if (url.endsWith("/v1/chat/completions")) {
        return Response.json({
          choices: [
            {
              message: {
                content: JSON.stringify({
                  confidence: 0.76,
                  reasonRefs: ["on_device_decider.reason.apple_fm_ready"],
                  selectedModelId: "local-small",
                  selectedToolNames: ["list_files", "read_file"],
                }),
              },
            },
          ],
        })
      }
      return new Response("not found", { status: 404 })
    }) as unknown as typeof fetch
    const service = createOnDeviceDeciderService({
      arch: "arm64",
      env: { OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER: "apple_fm" },
      fetchFn,
      now: () => fixedNow,
      platform: "darwin",
      spawn: spawnNever,
    })

    const result = await service.decide(request)

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.status.backendKind).toBe("apple_fm")
      expect(result.decision.selectedToolNames).toEqual([
        "list_files",
        "read_file",
      ])
      expect(result.decision.selectedModelId).toBe("local-small")
      expect(result.decision.noSpend).toBe(true)
      expect(result.decision.mainModelParityClaim).toBe(false)
    }
  })

  test("does not relaunch the Apple FM helper on every status poll", async () => {
    const { helperPath, resourcesDir } = createPackagedHelper()
    const spawned: Array<readonly string[]> = []
    const service = createOnDeviceDeciderService({
      arch: "arm64",
      env: { OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER: "apple_fm" },
      fetchFn: (async () => Response.json({ ready: false })) as unknown as typeof fetch,
      now: () => fixedNow,
      platform: "darwin",
      resourcesDir,
      spawn: ((command: readonly string[]) => {
        spawned.push([...command])
        return {
          exited: new Promise<number>(() => {}),
          kill() {},
        }
      }) as unknown as typeof Bun.spawn,
    })

    try {
      await service.status()
      await service.status()

      expect(spawned).toEqual([[helperPath, "--port", "11435"]])
    } finally {
      rmSync(resourcesDir, { force: true, recursive: true })
    }
  })

  test("keeps GPT-OSS local-only and rejects remote endpoints", async () => {
    const remote = createOnDeviceDeciderService({
      env: {
        OPENAGENTS_DESKTOP_GPT_OSS_DECIDER_URL: "https://api.example.com",
        OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER: "gpt_oss",
      },
      now: () => fixedNow,
      platform: "linux",
      spawn: spawnNever,
    })

    expect((await remote.status()).state).toBe("unconfigured")

    const fetchFn = (async () =>
      Response.json({
        choices: [
          {
            message: {
              content: JSON.stringify({
                confidence: 0.5,
                selectedModelId: "codex-main",
                selectedToolNames: ["read_file"],
              }),
            },
          },
        ],
      })) as unknown as typeof fetch
    const local = createOnDeviceDeciderService({
      env: {
        OPENAGENTS_DESKTOP_GPT_OSS_DECIDER_URL: "http://127.0.0.1:11434",
        OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER: "gpt_oss",
      },
      fetchFn,
      now: () => fixedNow,
      platform: "linux",
      spawn: spawnNever,
    })

    const result = await local.decide(request)

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.status.backendKind).toBe("gpt_oss")
      expect(result.decision.selectedToolNames).toEqual(["read_file"])
    }
  })
})
