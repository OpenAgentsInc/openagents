import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"

import { PYLON_CLIENT_VERSION, PYLON_VERSION } from "../src/version"

// The `bun --compile` binary does NOT bundle package.json — the running Pylon
// reports `src/version.ts` PYLON_VERSION (status, heartbeat/presence) and the OTA
// self-updater compares against it. If version.ts drifts BELOW package.json, the
// shipped binary self-reports an older version, the updater sees "feed > self"
// forever and auto-update-loops, and /api/pylons shows the wrong version. That is
// exactly the rc.13 incident (package.json bumped to rc.13, version.ts left at
// rc.12). This test makes that drift fail in the suite, not in production.
describe("Pylon release version sync", () => {
  const pkg = JSON.parse(
    readFileSync(join(import.meta.dir, "../package.json"), "utf8"),
  ) as { version: string }

  test("src/version.ts PYLON_VERSION matches package.json version", () => {
    expect(PYLON_VERSION).toBe(pkg.version)
  })

  test("PYLON_CLIENT_VERSION composes the same version", () => {
    expect(PYLON_CLIENT_VERSION).toBe(`openagents.pylon@${pkg.version}`)
  })
})
