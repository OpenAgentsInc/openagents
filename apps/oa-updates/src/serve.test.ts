import { readFileSync } from "node:fs"
import { resolve } from "node:path"

import { describe, expect, test } from "vite-plus/test"

const serveSource = readFileSync(
  resolve(import.meta.dirname, "serve.ts"),
  "utf8",
)

// The Cloud Run entrypoint seeds exactly one surface. The Expo mobile OTA seed
// was retired on 2026-08-05 (#9325) after the owner confirmed there are no
// installed mobile users, so the entrypoint must never read a mobile seed
// variable again — a stale operator environment cannot resurrect a feed whose
// routes and resolver no longer exist.
describe("oa-updates entrypoint seeding surface", () => {
  test("seeds the Pylon release feed from OA_PYLON_RELEASES_DIST", () => {
    expect(serveSource).toContain("OA_PYLON_RELEASES_DIST")
    expect(serveSource).toContain("seedPylonReleases")
  })

  test.each([
    "OA_SEED_DIST",
    "OA_SEED_RUNTIME",
    "OA_SEED_PLATFORM",
    "OA_SEED_BRANCH",
    "OA_SEED_EXPO_CLIENT_PATH",
    "OA_SIGNING_KEY",
  ])("never reads the retired mobile OTA variable %s", (name) => {
    expect(serveSource).not.toContain(`process.env.${name}`)
  })

  test("no longer imports any Expo export/publish module", () => {
    const imported = [...serveSource.matchAll(/from\s+"([^"]+)"/g)].map(
      (match) => match[1],
    )
    expect(imported).toEqual([
      "@openagentsinc/runtime-platform",
      "./pylon-seed.ts",
      "./server.ts",
    ])
    expect(serveSource).not.toContain("seedFromDist(")
  })
})
