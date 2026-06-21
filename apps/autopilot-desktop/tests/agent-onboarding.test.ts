import { describe, expect, it } from "bun:test"
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  autoDisplayName,
  autoSlug,
  buildOnboardingChildEnv,
  loadPersistedCredential,
  normalizeDisplayName,
  persistCredential,
  readNodeIdentity,
  redactToken,
  selfRegisterAgent,
  type NodeIdentity,
  type RegisterFetch,
} from "../src/bun/agent-onboarding"

const identity: NodeIdentity = {
  npub: "npub1examplepubkey000000000000000000000000000000000000000000000abc",
  nodeLabel: "studio-mac",
  pylonRef: "pylon.abc123def456",
}

// A fake `POST /api/agents/register` 201 response mirroring the real contract
// (apps/openagents.com/workers/api/src/agent-registration.ts).
const okRegisterResponse = (token = "oa_agent_freshlyMintedToken123") =>
  ({
    status: 201,
    json: async () => ({
      user: { id: "user_42", status: "active" },
      credential: { token, tokenPrefix: "oa_agent_fre" },
    }),
  }) as const

describe("redactToken", () => {
  it("never reveals the secret body, only the scheme prefix", () => {
    expect(redactToken("oa_agent_supersecretvalue")).toBe("oa_agent_…")
    expect(redactToken("oa_agent_supersecretvalue")).not.toContain(
      "supersecret",
    )
    expect(redactToken(null)).toBe("<none>")
    expect(redactToken("")).toBe("<none>")
    expect(redactToken("weird")).toBe("oa_…")
  })
})

describe("readNodeIdentity", () => {
  it("reads npub/nodeLabel/pylonRef from <home>/identity.json", () => {
    const home = mkdtempSync(join(tmpdir(), "ao-identity-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({
          nodeId: "pylon_x",
          pylonRef: identity.pylonRef,
          nodeLabel: identity.nodeLabel,
          publicKey: "deadbeef",
          npub: identity.npub,
          createdAt: "2026-06-18T00:00:00.000Z",
        }),
      )
      expect(readNodeIdentity(home)).toEqual(identity)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("returns null when identity.json is missing or malformed (not ready)", () => {
    const home = mkdtempSync(join(tmpdir(), "ao-identity-"))
    try {
      expect(readNodeIdentity(home)).toBeNull()
      writeFileSync(join(home, "identity.json"), "{not json")
      expect(readNodeIdentity(home)).toBeNull()
      writeFileSync(join(home, "identity.json"), JSON.stringify({ npub: "" }))
      expect(readNodeIdentity(home)).toBeNull()
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })
})

describe("autoDisplayName / autoSlug", () => {
  it("derives a neutral, length-bounded default display name", () => {
    const name = autoDisplayName(identity)
    expect(name).toBe("studio-mac (exampl)")
    expect(name.length).toBeLessThanOrEqual(120)
  })

  it("falls back to a generic label with no nodeLabel", () => {
    expect(
      autoDisplayName({ ...identity, nodeLabel: null }),
    ).toContain("Autopilot Desktop")
  })

  it("derives a schema-valid slug (lowercase, 3..80, [a-z0-9-])", () => {
    const slug = autoSlug(identity)!
    expect(slug).toMatch(/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/)
    expect(slug.length).toBeGreaterThanOrEqual(3)
    expect(slug.length).toBeLessThanOrEqual(80)
  })
})

describe("persistCredential / loadPersistedCredential", () => {
  it("round-trips a credential and writes it with 0600 perms", () => {
    const home = mkdtempSync(join(tmpdir(), "ao-cred-"))
    try {
      persistCredential(home, {
        token: "oa_agent_roundtrip",
        tokenPrefix: "oa_agent_rou",
        userId: "user_1",
        externalId: identity.npub,
        registeredAt: "2026-06-18T00:00:00.000Z",
      })
      const loaded = loadPersistedCredential(home)
      expect(loaded?.token).toBe("oa_agent_roundtrip")
      expect(loaded?.userId).toBe("user_1")
      // 0600: owner-only read/write.
      const mode = statSync(join(home, "agent-credential.json")).mode & 0o777
      expect(mode).toBe(0o600)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("rejects a persisted file without a valid oa_agent_ token", () => {
    const home = mkdtempSync(join(tmpdir(), "ao-cred-"))
    try {
      writeFileSync(
        join(home, "agent-credential.json"),
        JSON.stringify({ token: "not_a_real_token" }),
      )
      expect(loadPersistedCredential(home)).toBeNull()
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })
})

describe("selfRegisterAgent (AO-1)", () => {
  it("defers until the node has written its identity", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      let called = 0
      const result = await selfRegisterAgent({
        home,
        fetchImpl: (async () => {
          called += 1
          return okRegisterResponse()
        }) as RegisterFetch,
      })
      expect(result.outcome).toBe("identity_pending")
      expect(called).toBe(0)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("registers, persists the minted token, and posts the deterministic externalId", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub, nodeLabel: identity.nodeLabel, pylonRef: identity.pylonRef }),
      )
      let sentBody: Record<string, unknown> = {}
      const logs: string[] = []
      const result = await selfRegisterAgent({
        home,
        baseUrl: "https://openagents.com",
        log: m => logs.push(m),
        fetchImpl: (async (_url, init) => {
          sentBody = JSON.parse(init.body) as Record<string, unknown>
          expect(_url).toBe("https://openagents.com/api/agents/register")
          return okRegisterResponse("oa_agent_minted_xyz")
        }) as RegisterFetch,
      })

      expect(result.outcome).toBe("registered")
      // The npub is the idempotency anchor.
      expect(sentBody.externalId).toBe(identity.npub)
      expect(sentBody.displayName).toBe("studio-mac (exampl)")
      expect((sentBody.metadata as Record<string, unknown>).source).toBe(
        "autopilot-desktop",
      )
      // Token persisted to the managed home.
      const persisted = loadPersistedCredential(home)
      expect(persisted?.token).toBe("oa_agent_minted_xyz")
      expect(persisted?.externalId).toBe(identity.npub)
      // The token is NEVER in any log line (redaction).
      for (const line of logs) expect(line).not.toContain("oa_agent_minted_xyz")
      const fileText = readFileSync(join(home, "agent-credential.json"), "utf8")
      // The on-disk credential intentionally holds the token (0600) but the
      // logs must not — assert the redaction contract on the log surface.
      expect(fileText).toContain("oa_agent_minted_xyz")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("is idempotent: reuses a persisted token and never re-registers", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      persistCredential(home, {
        token: "oa_agent_existing",
        tokenPrefix: "oa_agent_exi",
        userId: "user_9",
        externalId: identity.npub,
        registeredAt: "2026-06-18T00:00:00.000Z",
      })
      let called = 0
      const result = await selfRegisterAgent({
        home,
        fetchImpl: (async () => {
          called += 1
          return okRegisterResponse()
        }) as RegisterFetch,
      })
      expect(result.outcome).toBe("reused")
      expect(called).toBe(0)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("is offline-tolerant: a network error defers, never throws or persists", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub }),
      )
      const result = await selfRegisterAgent({
        home,
        fetchImpl: (async () => {
          throw new Error("ECONNREFUSED")
        }) as RegisterFetch,
      })
      expect(result.outcome).toBe("deferred")
      expect(loadPersistedCredential(home)).toBeNull()
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("defers (does not duplicate) on a 409 conflict with no persisted token", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub }),
      )
      const result = await selfRegisterAgent({
        home,
        fetchImpl: (async () => ({
          status: 409,
          json: async () => ({ error: "agent_registration_conflict" }),
        })) as RegisterFetch,
      })
      expect(result.outcome).toBe("deferred")
      if (result.outcome === "deferred") {
        expect(result.reason).toBe("agent_registration_conflict")
      }
      expect(loadPersistedCredential(home)).toBeNull()
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("attaches an optional BOLT12 offer when provided", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub }),
      )
      let sentBody: Record<string, unknown> = {}
      await selfRegisterAgent({
        home,
        bolt12Offer: "lno1qtipoffer",
        fetchImpl: (async (_url, init) => {
          sentBody = JSON.parse(init.body) as Record<string, unknown>
          return okRegisterResponse()
        }) as RegisterFetch,
      })
      expect(sentBody.bolt12Offer).toBe("lno1qtipoffer")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  // AF-1 (#5898): the node Spark receive address lands as `spark_address` tip
  // readiness, is dropped when malformed, and never leaks into any log line.
  it("attaches a valid Spark address as sparkAddress when provided", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub }),
      )
      const spark = "sp1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"
      let sentBody: Record<string, unknown> = {}
      await selfRegisterAgent({
        home,
        sparkAddress: spark,
        fetchImpl: (async (_url, init) => {
          sentBody = JSON.parse(init.body) as Record<string, unknown>
          return okRegisterResponse()
        }) as RegisterFetch,
      })
      expect(sentBody.sparkAddress).toBe(spark)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("drops a malformed Spark address rather than posting it", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub }),
      )
      let sentBody: Record<string, unknown> = {}
      await selfRegisterAgent({
        home,
        // Not a Spark address (e.g. a transient error string). Must be dropped.
        sparkAddress: "ERROR: spark daemon offline",
        fetchImpl: (async (_url, init) => {
          sentBody = JSON.parse(init.body) as Record<string, unknown>
          return okRegisterResponse()
        }) as RegisterFetch,
      })
      expect("sparkAddress" in sentBody).toBe(false)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("never logs the raw Spark address (payment material)", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub }),
      )
      const spark = "sp1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2t"
      const logs: string[] = []
      await selfRegisterAgent({
        home,
        sparkAddress: spark,
        log: msg => logs.push(msg),
        fetchImpl: (async () => okRegisterResponse()) as RegisterFetch,
      })
      expect(logs.join("\n")).not.toContain(spark)
      expect(logs.join("\n")).not.toContain("sp1q")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })
})

describe("normalizeDisplayName (AO-3)", () => {
  it("trims, collapses whitespace, and clamps to 120 chars", () => {
    expect(normalizeDisplayName("  My   Studio  Agent ")).toBe("My Studio Agent")
    expect(normalizeDisplayName("x".repeat(200))?.length).toBe(120)
  })
  it("returns null for blank/missing input (falls back to auto name)", () => {
    expect(normalizeDisplayName("   ")).toBeNull()
    expect(normalizeDisplayName(null)).toBeNull()
    expect(normalizeDisplayName(undefined)).toBeNull()
  })
})

describe("selfRegisterAgent display name (AO-3)", () => {
  it("a user-chosen name REPLACES the auto display name in the registration", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-name-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub, nodeLabel: identity.nodeLabel }),
      )
      let sentBody: Record<string, unknown> = {}
      const result = await selfRegisterAgent({
        home,
        displayName: "  Chris's Studio Node  ",
        fetchImpl: (async (_url, init) => {
          sentBody = JSON.parse(init.body) as Record<string, unknown>
          return okRegisterResponse("oa_agent_named")
        }) as RegisterFetch,
      })
      expect(result.outcome).toBe("registered")
      // The chosen, normalized name is what registers — not the auto default.
      expect(sentBody.displayName).toBe("Chris's Studio Node")
      expect(sentBody.displayName).not.toBe(autoDisplayName(identity))
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("a blank chosen name falls back to the neutral auto display name", async () => {
    const home = mkdtempSync(join(tmpdir(), "ao-reg-name-"))
    try {
      writeFileSync(
        join(home, "identity.json"),
        JSON.stringify({ npub: identity.npub, nodeLabel: identity.nodeLabel }),
      )
      let sentBody: Record<string, unknown> = {}
      await selfRegisterAgent({
        home,
        displayName: "   ",
        fetchImpl: (async (_url, init) => {
          sentBody = JSON.parse(init.body) as Record<string, unknown>
          return okRegisterResponse()
        }) as RegisterFetch,
      })
      expect(sentBody.displayName).toBe(autoDisplayName(identity))
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })
})

describe("buildOnboardingChildEnv (AO-2)", () => {
  it("sets all three switches when a token is present", () => {
    const env = buildOnboardingChildEnv({
      base: { PYLON_HOME: "/home/.pylon-local" },
      agentToken: "oa_agent_live",
    })
    expect(env.PYLON_HOME).toBe("/home/.pylon-local")
    expect(env.PYLON_OPENAGENTS_BASE_URL).toBe("https://openagents.com")
    expect(env.OPENAGENTS_AGENT_TOKEN).toBe("oa_agent_live")
    expect(env.PYLON_ASSIGNMENT_WORKER).toBe("1")
  })

  it("stays isolated with no token: no product URL, no token, no assignment worker", () => {
    const env = buildOnboardingChildEnv({
      base: { PYLON_HOME: "/home/.pylon-local" },
      agentToken: null,
    })
    expect(env.PYLON_OPENAGENTS_BASE_URL).toBeUndefined()
    expect(env.OPENAGENTS_AGENT_TOKEN).toBeUndefined()
    expect(env.PYLON_ASSIGNMENT_WORKER).toBeUndefined()
  })

  it("respects explicit operator overrides and does not mutate base", () => {
    const base = {
      PYLON_HOME: "/h",
      PYLON_OPENAGENTS_BASE_URL: "https://staging.example",
      PYLON_ASSIGNMENT_WORKER: "0",
    }
    const env = buildOnboardingChildEnv({ base, agentToken: "oa_agent_x" })
    expect(env.PYLON_OPENAGENTS_BASE_URL).toBe("https://staging.example")
    // explicit "0" override is preserved (operator opt-out).
    expect(env.PYLON_ASSIGNMENT_WORKER).toBe("0")
    // base is untouched (pure).
    expect(base.PYLON_OPENAGENTS_BASE_URL).toBe("https://staging.example")
    expect(Object.keys(base)).not.toContain("OPENAGENTS_AGENT_TOKEN")
  })

  it("honors a custom base URL", () => {
    const env = buildOnboardingChildEnv({
      base: {},
      agentToken: "oa_agent_x",
      baseUrl: "https://preview.openagents.com",
    })
    expect(env.PYLON_OPENAGENTS_BASE_URL).toBe("https://preview.openagents.com")
  })
})
