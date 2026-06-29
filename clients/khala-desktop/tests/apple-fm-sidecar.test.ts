import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"

import { createAppleFmSidecarHost } from "../src/bun/apple-fm-sidecar.js"
import {
  APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
} from "../src/shared/apple-fm-packaging.js"
import { APPLE_FM_CAPABILITY } from "../src/shared/apple-fm-readiness.js"

const fixedNow = "2026-06-29T00:00:00.000Z"

describe("Khala Desktop Apple FM sidecar host", () => {
  test("reports unsupported hosts without advertising Apple FM capacity", async () => {
    const host = createAppleFmSidecarHost({
      arch: "x64",
      env: {},
      platform: "linux",
      now: () => fixedNow,
    })

    const readiness = await host.readiness()

    expect(readiness.supported).toBe(false)
    expect(readiness.available).toBe(false)
    expect(readiness.state).toBe("not_supported")
    expect(readiness.blockerRefs).toContain(
      "blocker.khala_desktop.apple_fm.unsupported_platform",
    )
  })

  test("launches the packaged helper and keeps readiness public-safe", async () => {
    const resourcesDir = mkdtempSync(join(tmpdir(), "khala-apple-fm-sidecar-"))
    const helperPath = join(resourcesDir, APPLE_FM_BRIDGE_RESOURCES_SUBPATH)
    mkdirSync(dirname(helperPath), { recursive: true })
    writeFileSync(helperPath, "#!/usr/bin/env bash\n")
    chmodSync(helperPath, 0o755)

    const spawned: Array<ReadonlyArray<string>> = []
    const host = createAppleFmSidecarHost({
      arch: "arm64",
      env: {
        PYLON_CONTROL_TOKEN: "secret-control-token-1234",
        PYLON_CONTROL_URL: "http://127.0.0.1:4716",
      },
      fetchFn: (async () =>
        Response.json({
          ok: true,
          result: {
            available: true,
            status: "ready",
            advertisedCapabilities: [APPLE_FM_CAPABILITY],
            baseUrl: "http://127.0.0.1:11435",
            callbackUrl: "http://127.0.0.1/callback",
            controlToken: "secret-control-token-1234",
            helperPath,
            prompt: "private prompt text",
            supervisor: {
              health: "running",
              phase: "ready",
              supervised: true,
            },
          },
        })) as unknown as typeof fetch,
      now: () => fixedNow,
      platform: "darwin",
      resourcesDir,
      spawn: ((command: ReadonlyArray<string>) => {
        spawned.push([...command])
        return {
          exited: new Promise<number>(() => {}),
          kill() {},
        }
      }) as unknown as typeof Bun.spawn,
    })

    try {
      const readiness = await host.readiness()

      expect(spawned).toEqual([[helperPath, "--port", "11435"]])
      expect(readiness.available).toBe(true)
      expect(readiness.state).toBe("ready")
      expect(readiness.pylon?.supervisor?.contentRedacted).toBe(true)

      const serialized = JSON.stringify(readiness).toLowerCase()
      for (const forbidden of [
        "127.0.0.1",
        "callback",
        "private prompt",
        "secret-control-token",
        helperPath.toLowerCase(),
      ]) {
        expect(serialized.includes(forbidden)).toBe(false)
      }
    } finally {
      host.stop()
      rmSync(resourcesDir, { force: true, recursive: true })
    }
  })
})
