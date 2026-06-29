import { describe, expect, test } from "bun:test"

import {
  AppleFmSidecarManager,
  discoverAppleFmHelper,
  sidecarBlockerRefs,
} from "../src/bun/apple-fm-sidecar.js"
import {
  packagedAppleFmBridgePath,
  verifyPackagedAppleFmBridge,
} from "../src/shared/apple-fm-packaging.js"

const now = () => new Date("2026-06-29T12:00:00.000Z")

describe("Khala desktop Apple FM sidecar", () => {
  test("uses the Khala app Resources path for packaged verification", () => {
    expect(packagedAppleFmBridgePath("build/stable-macos-arm64/Khala.app")).toBe(
      "build/stable-macos-arm64/Khala.app/Contents/Resources/app/apple-fm-bridge/foundation-bridge",
    )

    const result = verifyPackagedAppleFmBridge({
      candidates: [{ bundleDir: "build/stable-macos-arm64/Khala.app", env: "stable" }],
      probe: (helperPath) => ({
        executable: helperPath.includes("apple-fm-bridge"),
        exists: true,
        nativeExecutable: true,
        nonEmpty: true,
      }),
    })
    expect(result).toMatchObject({
      ok: true,
      verifiedEnv: "stable",
    })
  })

  test("discovers packaged helper without exposing the helper path in status", async () => {
    const manager = new AppleFmSidecarManager({
      arch: "arm64",
      env: {},
      exists: (path) => path.endsWith("/app/apple-fm-bridge/foundation-bridge"),
      fetch: (async () =>
        new Response(JSON.stringify({ ready: true }), { status: 200 })) as unknown as typeof fetch,
      now,
      platform: "darwin",
      resourcesDir: "/Applications/Khala.app/Contents/Resources",
    })

    const status = await manager.status()
    expect(status).toMatchObject({
      available: true,
      blockerRefs: [],
      contentRedacted: true,
      helperSource: "packaged-resource",
      launchedByApp: false,
      state: "adopted",
    })
    expect(JSON.stringify(status)).not.toContain("/Applications")
    expect(JSON.stringify(status)).not.toContain("127.0.0.1")
  })

  test("fails closed on unsupported hosts and missing helpers", async () => {
    const unsupported = await new AppleFmSidecarManager({
      arch: "x64",
      now,
      platform: "darwin",
    }).status()
    expect(unsupported).toMatchObject({
      available: false,
      blockerRefs: ["blocker.khala_desktop.apple_fm.unsupported_host"],
      state: "not_supported",
    })

    const missing = await new AppleFmSidecarManager({
      arch: "arm64",
      exists: () => false,
      now,
      platform: "darwin",
    }).status()
    expect(missing).toMatchObject({
      available: false,
      blockerRefs: ["blocker.khala_desktop.apple_fm.helper_missing"],
      state: "helper_missing",
    })
  })

  test("autostarts only when explicitly armed", async () => {
    const fetchUnavailable = (async () =>
      new Response(JSON.stringify({ ready: false }), { status: 503 })) as unknown as typeof fetch
    const spawned: string[][] = []
    const runtime = {
      arch: "arm64",
      cwd: "/repo/clients/khala-desktop",
      env: { KHALA_DESKTOP_APPLE_FM_AUTOSTART: "1" },
      exists: (path: string) => path === "/repo/apps/pylon/bin/foundation-bridge",
      fetch: fetchUnavailable,
      now,
      platform: "darwin",
      spawn: (command: readonly string[]) => {
        spawned.push([...command])
        return { kill() {} }
      },
    }

    const status = await new AppleFmSidecarManager(runtime).status()
    expect(status).toMatchObject({
      available: false,
      helperSource: "source-wrapper",
      launchedByApp: true,
      state: "launching",
    })
    expect(spawned).toHaveLength(1)
    expect(spawned[0]).toEqual([
      "/repo/apps/pylon/bin/foundation-bridge",
      "--port",
      "11435",
    ])
  })

  test("blocker refs stay typed and public-safe", () => {
    expect(sidecarBlockerRefs("failed")).toEqual([
      "blocker.khala_desktop.apple_fm.launch_failed",
    ])
    expect(
      discoverAppleFmHelper({
        cwd: "/repo/clients/khala-desktop",
        env: { OPENAGENTS_APPLE_FM_BRIDGE_PATH: "/custom/foundation-bridge" },
        exists: (path) => path === "/custom/foundation-bridge",
      }),
    ).toMatchObject({ source: "env" })
  })
})
