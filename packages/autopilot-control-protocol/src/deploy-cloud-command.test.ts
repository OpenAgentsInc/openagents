import { describe, expect, test } from "vite-plus/test"

import { buildDeployCommand } from "./deploy-cloud-command.js"

describe("deploy cloud command builder", () => {
  test("builds a Cloud Run production command", () => {
    expect(buildDeployCommand({
      target: "cloudrun",
      ref: "refs/heads/main",
      env: "production",
    })).toEqual({
      ok: true,
      command: "deploy-cloudrun.sh",
      args: ["refs/heads/main", "--env", "production"],
      reason: "deploy command ready",
    })
  })

  test("builds a Cloud Run preview command with a trimmed ref", () => {
    expect(buildDeployCommand({
      target: "cloudrun",
      ref: "  release-2026-06-13  ",
      env: "preview",
    })).toEqual({
      ok: true,
      command: "deploy-cloudrun.sh",
      args: ["release-2026-06-13", "--env", "preview"],
      reason: "deploy command ready",
    })
  })

  test("rejects an unknown target", () => {
    expect(buildDeployCommand({
      target: "kubernetes",
      ref: "refs/heads/main",
      env: "preview",
    } as unknown as Parameters<typeof buildDeployCommand>[0])).toEqual({
      ok: false,
      command: "",
      args: [],
      reason: "target must be cloudrun",
    })
  })

  test("rejects blank refs", () => {
    expect(buildDeployCommand({
      target: "cloudrun",
      ref: "   ",
      env: "preview",
    })).toEqual({
      ok: false,
      command: "",
      args: [],
      reason: "ref is required",
    })
  })

  test("rejects an invalid env", () => {
    expect(buildDeployCommand({
      target: "cloudrun",
      ref: "refs/heads/main",
      env: "staging",
    } as unknown as Parameters<typeof buildDeployCommand>[0])).toEqual({
      ok: false,
      command: "",
      args: [],
      reason: "env must be production or preview",
    })
  })
})
