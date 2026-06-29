import { describe, expect, it } from "bun:test"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { persistCredential } from "../src/bun/agent-onboarding"
import {
  claimForumTipRecipientReadiness,
  isForumTipReady,
  loadTipReadyReceipt,
  type ClaimFetch,
} from "../src/bun/forum-tip-recipient"
import {
  FORUM_LOOP_MAX_WRITES_PER_DAY,
  recordForumWriteAttempt,
} from "../src/bun/forum-loop-bounds"

// AF-2 (#5899): automated forum tip-recipient readiness claim. Receive-only,
// idempotent, offline-tolerant, secrets-safe.

const NPUB =
  "npub1examplepubkey000000000000000000000000000000000000000000000abc"
const SPARK = "sp1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2t"

const seedHome = (opts: { credential?: boolean; identity?: boolean } = {}) => {
  const home = mkdtempSync(join(tmpdir(), "ftr-"))
  if (opts.identity !== false) {
    writeFileSync(join(home, "identity.json"), JSON.stringify({ npub: NPUB }))
  }
  if (opts.credential !== false) {
    persistCredential(home, {
      token: "oa_agent_tipClaimToken123",
      tokenPrefix: "oa_agent_tip",
      userId: "u1",
      externalId: NPUB,
      registeredAt: "2026-06-21T00:00:00.000Z",
    })
  }
  return home
}

const okClaimResponse = (tippingAvailable = true) =>
  ({
    status: 201,
    json: async () => ({ tipRecipientReadiness: { tippingAvailable } }),
  }) as const

describe("claimForumTipRecipientReadiness (AF-2)", () => {
  it("defers when the wallet is not receive-ready", async () => {
    const home = seedHome()
    try {
      const res = await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: false,
        sparkAddress: SPARK,
        fetchImpl: (async () => {
          throw new Error("must not call the network")
        }) as ClaimFetch,
      })
      expect(res.outcome).toBe("wallet_not_ready")
      expect(isForumTipReady(home)).toBe(false)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("is not_registered until an agent credential is persisted", async () => {
    const home = seedHome({ credential: false })
    try {
      const res = await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: SPARK,
        fetchImpl: (async () => okClaimResponse()) as ClaimFetch,
      })
      expect(res.outcome).toBe("not_registered")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("defers (spark_pending) when no usable Spark address is available", async () => {
    const home = seedHome()
    try {
      const res = await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: "not-a-spark-address",
        fetchImpl: (async () => okClaimResponse()) as ClaimFetch,
      })
      expect(res.outcome).toBe("spark_pending")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("claims: posts bearer + idempotency key + spark address + public-safe refs, persists a receipt", async () => {
    const home = seedHome()
    try {
      let sentUrl = ""
      let sentHeaders: Record<string, string> = {}
      let sentBody: Record<string, unknown> = {}
      const res = await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: SPARK,
        fetchImpl: (async (url, init) => {
          sentUrl = url
          sentHeaders = init.headers
          sentBody = JSON.parse(init.body) as Record<string, unknown>
          return okClaimResponse(true)
        }) as ClaimFetch,
      })
      expect(res.outcome).toBe("claimed")
      expect(sentUrl).toContain("/api/forum/tip-recipient-wallets/claims")
      expect(sentHeaders.authorization).toBe("Bearer oa_agent_tipClaimToken123")
      expect(sentHeaders["idempotency-key"]).toBeTruthy()
      expect(sentHeaders["user-agent"]).toBeTruthy()
      expect(sentBody.sparkAddress).toBe(SPARK)
      // Public-safe redacted refs only — never wallet material.
      expect(String(sentBody.walletRef)).toContain("wallet.public.")
      expect(String(sentBody.receiveCapabilityRef)).toContain(
        "receive_capability.public.",
      )
      expect(sentBody.readinessRefs).toEqual([
        "readiness.public.spark_address.offline_receive_ready",
      ])
      // Receipt persisted → observable tip-ready.
      expect(isForumTipReady(home)).toBe(true)
      expect(loadTipReadyReceipt(home)?.tippingAvailable).toBe(true)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("is idempotent: a persisted receipt short-circuits without a network call", async () => {
    const home = seedHome()
    try {
      // First claim lands.
      await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: SPARK,
        fetchImpl: (async () => okClaimResponse()) as ClaimFetch,
      })
      // Second attempt must not hit the network.
      const res = await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: SPARK,
        fetchImpl: (async () => {
          throw new Error("must not re-claim")
        }) as ClaimFetch,
      })
      expect(res.outcome).toBe("reused")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("treats a 409 conflict as already-claimed (reused, never duplicated)", async () => {
    const home = seedHome()
    try {
      const res = await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: SPARK,
        fetchImpl: (async () => ({
          status: 409,
          json: async () => ({}),
        })) as ClaimFetch,
      })
      expect(res.outcome).toBe("reused")
      expect(isForumTipReady(home)).toBe(true)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("is offline-tolerant: a network error defers, never throws or persists", async () => {
    const home = seedHome()
    try {
      const res = await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: SPARK,
        fetchImpl: (async () => {
          throw new Error("offline")
        }) as ClaimFetch,
      })
      expect(res.outcome).toBe("deferred")
      expect(isForumTipReady(home)).toBe(false)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("AF-5: backs off (rate_capped) when the daily forum-write cap is exhausted", async () => {
    const home = seedHome()
    try {
      for (let i = 0; i < FORUM_LOOP_MAX_WRITES_PER_DAY; i++) {
        recordForumWriteAttempt(home)
      }
      let posted = false
      const res = await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: SPARK,
        fetchImpl: (async () => {
          posted = true
          return okClaimResponse()
        }) as ClaimFetch,
      })
      expect(res.outcome).toBe("rate_capped")
      expect(posted).toBe(false)
      expect(isForumTipReady(home)).toBe(false)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("never logs the agent token or the raw Spark address", async () => {
    const home = seedHome()
    try {
      const logs: string[] = []
      await claimForumTipRecipientReadiness({
        home,
        walletReceiveReady: true,
        sparkAddress: SPARK,
        log: m => logs.push(m),
        fetchImpl: (async () => okClaimResponse()) as ClaimFetch,
      })
      const joined = logs.join("\n")
      expect(joined).not.toContain("oa_agent_tipClaimToken123")
      expect(joined).not.toContain(SPARK)
      expect(joined).not.toContain("sp1q")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })
})
