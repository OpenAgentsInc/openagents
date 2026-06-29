import { describe, expect, test } from "bun:test"
import { createDeployCloudActions } from "../src/node/deploy-cloud"

// CL-26 "Deploy to Cloud": the node-side action validates via the shared
// protocol cores, fail-safe gates execution behind OA_DEPLOY_ENABLE=1, and
// fire-and-forgets the deploy command. These tests inject a recording spawn so
// they NEVER actually deploy anything.

function recordingSpawn() {
  const calls: Array<{ command: string; args: string[] }> = []
  return { spawn: (command: string, args: string[]) => calls.push({ command, args }), calls }
}

describe("CL-26 deploy.cloud node action", () => {
  test("rejects an invalid request and never spawns", async () => {
    const rec = recordingSpawn()
    const actions = createDeployCloudActions({ isEnabled: () => true, spawn: rec.spawn })

    // bad target + empty ref
    const result = (await actions.deployCloud({ target: "nope", ref: "" })) as {
      accepted: boolean
      reason: string
      errors?: string[]
    }

    expect(result.accepted).toBe(false)
    expect(result.reason).toBe("invalid_request")
    expect(Array.isArray(result.errors)).toBe(true)
    expect(result.errors?.some((e) => e.includes("target"))).toBe(true)
    expect(result.errors?.some((e) => e.includes("ref"))).toBe(true)
    // nothing ran
    expect(rec.calls.length).toBe(0)
  })

  test("disabled (OA_DEPLOY_ENABLE unset) → not accepted and never spawns even for a valid request", async () => {
    const rec = recordingSpawn()
    const actions = createDeployCloudActions({ isEnabled: () => false, spawn: rec.spawn })

    const result = (await actions.deployCloud({ target: "cloudrun", ref: "main", env: "production" })) as {
      accepted: boolean
      reason: string
    }

    expect(result.accepted).toBe(false)
    expect(result.reason).toBe("deploy_disabled")
    // fail-safe: NOTHING runs by default
    expect(rec.calls.length).toBe(0)
  })

  test("the default gate is fail-safe (OA_DEPLOY_ENABLE not '1' → disabled)", async () => {
    const rec = recordingSpawn()
    // No isEnabled override: createDeployCloudActions reads OA_DEPLOY_ENABLE.
    // The test runner does not set it to "1", so this must be disabled.
    const prior = Bun.env.OA_DEPLOY_ENABLE
    delete Bun.env.OA_DEPLOY_ENABLE
    try {
      const actions = createDeployCloudActions({ spawn: rec.spawn })
      const result = (await actions.deployCloud({ target: "cloudrun", ref: "main" })) as {
        accepted: boolean
        reason: string
      }
      expect(result.accepted).toBe(false)
      expect(result.reason).toBe("deploy_disabled")
      expect(rec.calls.length).toBe(0)
    } finally {
      if (prior === undefined) delete Bun.env.OA_DEPLOY_ENABLE
      else Bun.env.OA_DEPLOY_ENABLE = prior
    }
  })

  test("enabled + valid → accepted, spawns the built command, records last deploy", async () => {
    const rec = recordingSpawn()
    const actions = createDeployCloudActions({
      isEnabled: () => true,
      spawn: rec.spawn,
      now: () => new Date("2026-06-13T12:00:00.000Z"),
    })

    const result = (await actions.deployCloud({ target: "cloudrun", ref: "main", env: "production" })) as {
      accepted: boolean
      reason: string
      target?: string
      ref?: string
      env?: string
      startedAt?: string
    }

    expect(result.accepted).toBe(true)
    expect(result.target).toBe("cloudrun")
    expect(result.ref).toBe("main")
    expect(result.env).toBe("production")
    expect(result.startedAt).toBe("2026-06-13T12:00:00.000Z")

    // The shared buildDeployCommand produced exactly this for cloudrun.
    expect(rec.calls.length).toBe(1)
    expect(rec.calls[0]?.command).toBe("deploy-cloudrun.sh")
    expect(rec.calls[0]?.args).toEqual(["main", "--env", "production"])
  })

  test("workers target builds the wrangler command when enabled", async () => {
    const rec = recordingSpawn()
    const actions = createDeployCloudActions({ isEnabled: () => true, spawn: rec.spawn })

    const result = (await actions.deployCloud({ target: "workers", ref: "main", env: "preview" })) as {
      accepted: boolean
    }

    expect(result.accepted).toBe(true)
    expect(rec.calls.length).toBe(1)
    expect(rec.calls[0]?.command).toBe("wrangler")
    expect(rec.calls[0]?.args).toEqual(["deploy", "--env", "preview"])
  })
})

describe("CL-26 deploy.status projection", () => {
  test("a fresh node with no deploy projects an unknown/unavailable status", async () => {
    const actions = createDeployCloudActions({ isEnabled: () => true, spawn: () => {} })
    const status = (await actions.deployStatus()) as {
      state: string
      url: string | null
      deployedAt: string | null
      message: string
    }
    expect(status.state).toBe("unknown")
    expect(status.url).toBeNull()
    expect(status.deployedAt).toBeNull()
    expect(typeof status.message).toBe("string")
  })

  test("after a queued deploy, status projects the 'queued' shape", async () => {
    const actions = createDeployCloudActions({ isEnabled: () => true, spawn: () => {} })
    await actions.deployCloud({ target: "cloudrun", ref: "main", env: "production" })

    const status = (await actions.deployStatus()) as {
      state: string
      url: string | null
      deployedAt: string | null
      message: string
    }
    expect(status.state).toBe("queued")
    // queued: no live url / deployedAt yet, but a human message is present.
    expect(status.url).toBeNull()
    expect(typeof status.message).toBe("string")
    expect(status.message.length).toBeGreaterThan(0)
  })
})
