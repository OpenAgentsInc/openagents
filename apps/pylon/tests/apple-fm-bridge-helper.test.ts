import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  APPLE_FM_BRIDGE_PATH_ENV,
  discoverAppleFmBridgeHelper,
} from "../src/node/apple-fm-bridge-helper"

function touch(path: string) {
  mkdirSync(path.split("/").slice(0, -1).join("/"), { recursive: true })
  writeFileSync(path, "")
}

function withTempDir<T>(fn: (dir: string) => T): T {
  const dir = mkdtempSync(join(tmpdir(), "pylon-apple-fm-bridge-"))
  try {
    return fn(dir)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

describe("Apple FM bridge helper discovery", () => {
  test("prefers an explicit helper path from the environment", () => {
    withTempDir((dir) => {
      const explicit = join(dir, "custom-foundation-bridge")
      touch(explicit)

      const discovered = discoverAppleFmBridgeHelper({
        cwd: join(dir, "apps", "pylon"),
        env: { [APPLE_FM_BRIDGE_PATH_ENV]: explicit },
      })

      expect(discovered).toEqual({ path: explicit, source: "env" })
    })
  })

  test("discovers the source wrapper before the raw SwiftPM release binary", () => {
    withTempDir((dir) => {
      const pylonRoot = join(dir, "apps", "pylon")
      const wrapper = join(pylonRoot, "bin", "foundation-bridge")
      const sourceBuild = join(pylonRoot, "swift", "foundation-bridge", ".build", "release", "foundation-bridge")
      touch(wrapper)
      touch(sourceBuild)

      const discovered = discoverAppleFmBridgeHelper({
        cwd: pylonRoot,
        env: {},
      })

      expect(discovered).toEqual({ path: wrapper, source: "source-wrapper" })
    })
  })

  test("discovers a raw SwiftPM release binary from the repo root", () => {
    withTempDir((dir) => {
      const sourceBuild = join(dir, "apps", "pylon", "swift", "foundation-bridge", ".build", "release", "foundation-bridge")
      touch(sourceBuild)

      const discovered = discoverAppleFmBridgeHelper({
        cwd: dir,
        env: {},
      })

      expect(discovered).toEqual({ path: sourceBuild, source: "source-build" })
    })
  })

  test("falls back to packaged desktop resources", () => {
    withTempDir((dir) => {
      const resourcesDir = join(dir, "Contents", "Resources")
      const packaged = join(resourcesDir, "app", "apple-fm-bridge", "foundation-bridge")
      touch(packaged)

      const discovered = discoverAppleFmBridgeHelper({
        cwd: join(dir, "not-a-repo"),
        env: {},
        resourcesDir,
      })

      expect(discovered).toEqual({ path: packaged, source: "packaged-resource" })
    })
  })

  test("returns null when no helper candidate exists", () => {
    withTempDir((dir) => {
      expect(
        discoverAppleFmBridgeHelper({
          cwd: dir,
          env: {},
          resourcesDir: join(dir, "missing-resources"),
        }),
      ).toBeNull()
    })
  })
})
