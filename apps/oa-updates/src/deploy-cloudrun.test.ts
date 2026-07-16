import { execFileSync } from "node:child_process"

import { describe, expect, test } from "vite-plus/test"

const script = new URL("../scripts/deploy-cloudrun.sh", import.meta.url).pathname

const deployArgs = (environment: Record<string, string>): readonly string[] =>
  execFileSync("bash", [script], {
    encoding: "utf8",
    env: {
      PATH: process.env.PATH ?? "/usr/bin:/bin",
      OA_UPDATES_DEPLOY_DRY_RUN: "1",
      OA_PUBLIC_URL: "https://updates.openagents.test",
      OA_SIGNING_SECRET: "fixture-signing-key:1",
      ...environment,
    },
  }).trim().split("\n")

describe("oa-updates additive Cloud Run deploy command", () => {
  test("Desktop-only update preserves existing mobile env and secrets", () => {
    const args = deployArgs({
      OA_RELEASE_SET_BUCKET: "openagents-release-fixture",
      OA_RELEASE_SET_PINS_PATH: "/app/openagents-desktop-dist/release-set-pins.json",
    })
    expect(args).toContain("--update-env-vars")
    expect(args).not.toContain("--set-env-vars")
    expect(args).toContain("--update-secrets")
    const env = args[args.indexOf("--update-env-vars") + 1]
    expect(env).toContain("OA_RELEASE_SET_BUCKET=openagents-release-fixture")
    expect(env).not.toContain("OA_SEED_DIST=")
  })

  test("mobile-only update preserves existing Desktop v2 env", () => {
    const args = deployArgs({
      OA_SEED_DIST: "/app/dist",
      OA_SEED_RUNTIME: "fixture-runtime",
      OA_SEED_BRANCH: "openagents-production",
    })
    expect(args).toContain("--update-env-vars")
    const env = args[args.indexOf("--update-env-vars") + 1]
    expect(env).toContain("OA_SEED_DIST=/app/dist")
    expect(env).not.toContain("OA_RELEASE_SET_BUCKET=")
  })
})
