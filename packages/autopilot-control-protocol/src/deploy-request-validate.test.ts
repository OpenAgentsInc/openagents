import { describe, expect, test } from "bun:test"

import { validateDeployRequest } from "./deploy-request-validate.js"

describe("deploy request validation", () => {
  test("accepts a Cloud Run production request", () => {
    expect(validateDeployRequest({
      target: "cloudrun",
      ref: "refs/heads/main",
      env: "production",
    })).toEqual({
      ok: true,
      target: "cloudrun",
      ref: "refs/heads/main",
      env: "production",
      errors: [],
    })
  })

  test("accepts a Workers request and defaults env to preview", () => {
    expect(validateDeployRequest({
      target: "workers",
      ref: "OpenAgentsInc/openagents#4932",
    })).toEqual({
      ok: true,
      target: "workers",
      ref: "OpenAgentsInc/openagents#4932",
      env: "preview",
      errors: [],
    })
  })

  test("trims refs before checking emptiness", () => {
    expect(validateDeployRequest({
      target: "workers",
      ref: "  release-2026-06-13  ",
      env: "preview",
    })).toEqual({
      ok: true,
      target: "workers",
      ref: "release-2026-06-13",
      env: "preview",
      errors: [],
    })
  })

  test("rejects an unknown target", () => {
    expect(validateDeployRequest({
      target: "kubernetes",
      ref: "refs/heads/main",
      env: "preview",
    })).toEqual({
      ok: false,
      target: null,
      ref: "refs/heads/main",
      env: "preview",
      errors: ["target must be cloudrun or workers"],
    })
  })

  test("rejects blank and non-string refs", () => {
    expect(validateDeployRequest({
      target: "cloudrun",
      ref: "   ",
      env: "preview",
    })).toEqual({
      ok: false,
      target: "cloudrun",
      ref: "",
      env: "preview",
      errors: ["ref is required"],
    })

    expect(validateDeployRequest({
      target: "cloudrun",
      ref: 4932,
      env: "preview",
    })).toEqual({
      ok: false,
      target: "cloudrun",
      ref: "",
      env: "preview",
      errors: ["ref is required"],
    })
  })

  test("rejects an invalid env and falls back to preview in the result", () => {
    expect(validateDeployRequest({
      target: "workers",
      ref: "refs/heads/main",
      env: "staging",
    })).toEqual({
      ok: false,
      target: "workers",
      ref: "refs/heads/main",
      env: "preview",
      errors: ["env must be production or preview"],
    })
  })

  test("accumulates target ref and env errors without throwing", () => {
    expect(validateDeployRequest({
      target: null,
      ref: [],
      env: false,
    })).toEqual({
      ok: false,
      target: null,
      ref: "",
      env: "preview",
      errors: [
        "target must be cloudrun or workers",
        "ref is required",
        "env must be production or preview",
      ],
    })
  })
})
