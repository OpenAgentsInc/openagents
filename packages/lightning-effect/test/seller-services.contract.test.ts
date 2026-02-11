import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import type {
  L402AuthorizationVerificationResult,
  L402ChallengeIssueRequest,
  PaywallDefinition,
  SettlementRecord,
} from "../src/contracts/seller.js"
import { GatewayConfigCompilerService } from "../src/services/gatewayConfigCompiler.js"
import { InvoiceIssuerService } from "../src/services/invoiceIssuer.js"
import { PaywallRegistryService } from "../src/services/paywallRegistry.js"
import { SellerPolicyService } from "../src/services/sellerPolicy.js"
import { SettlementIngestService } from "../src/services/settlementIngest.js"
import { makeSellerDeterministicLayer } from "../src/adapters/sellerDeterministic.js"

const makePaywall = (
  paywallId: string,
  options?: {
    readonly ownerId?: string
    readonly status?: PaywallDefinition["status"]
    readonly hostPattern?: string
    readonly pathPattern?: string
    readonly upstreamUrl?: string
    readonly priority?: number
    readonly fixedAmountMsats?: number
    readonly maxAmountMsats?: number
    readonly allowedBuyerHosts?: ReadonlyArray<string>
    readonly blockedBuyerHosts?: ReadonlyArray<string>
    readonly killSwitch?: boolean
    readonly createdAtMs?: number
    readonly updatedAtMs?: number
  },
): PaywallDefinition => ({
  paywallId,
  ownerId: options?.ownerId ?? "owner_1",
  name: `Paywall ${paywallId}`,
  status: options?.status ?? "active",
  createdAtMs: options?.createdAtMs ?? 1_700_000_000_000,
  updatedAtMs: options?.updatedAtMs ?? 1_700_000_000_000,
  route: {
    paywallId,
    hostPattern: options?.hostPattern ?? "api.example.com",
    pathPattern: options?.pathPattern ?? "^/premium",
    upstreamUrl: options?.upstreamUrl ?? "https://upstream.example.com/premium",
    priority: options?.priority ?? 10,
  },
  policy: {
    paywallId,
    pricingMode: "fixed_msats",
    fixedAmountMsats: options?.fixedAmountMsats ?? 2_500,
    maxAmountMsats: options?.maxAmountMsats ?? 5_000,
    allowedBuyerHosts: [...(options?.allowedBuyerHosts ?? ["buyer.example.com"])],
    blockedBuyerHosts: [...(options?.blockedBuyerHosts ?? [])],
    killSwitch: options?.killSwitch ?? false,
  },
})

const makeChallengeRequest = (
  paywallId: string,
  options?: {
    readonly host?: string
    readonly quotedAmountMsats?: number
    readonly requestId?: string
  },
): L402ChallengeIssueRequest => ({
  paywallId,
  requestId: options?.requestId ?? "req_1",
  host: options?.host ?? "buyer.example.com",
  path: "/premium",
  method: "GET",
  quotedAmountMsats: options?.quotedAmountMsats ?? 2_500,
})

const makeSettlement = (
  settlementId: string,
  options?: {
    readonly paywallId?: string
    readonly ownerId?: string
    readonly requestId?: string
    readonly amountMsats?: number
    readonly status?: SettlementRecord["status"]
    readonly createdAtMs?: number
  },
): SettlementRecord => ({
  settlementId,
  paywallId: options?.paywallId ?? "paywall_1",
  ownerId: options?.ownerId ?? "owner_1",
  requestId: options?.requestId ?? `req_${settlementId}`,
  invoice: `lnoa_${settlementId}`,
  paymentId: `pay_${settlementId}`,
  amountMsats: options?.amountMsats ?? 2_500,
  status: options?.status ?? "settled",
  paymentProofType: "lightning_preimage",
  proofReference: `proof_${settlementId}`,
  settledAtMs: 1_700_000_000_100,
  createdAtMs: options?.createdAtMs ?? 1_700_000_000_000,
})

describe("seller service conformance", () => {
  it.effect("paywall registry implements upsert/get/list/status semantics", () =>
    Effect.gen(function* () {
      const registry = yield* PaywallRegistryService
      const created = makePaywall("paywall_1", {
        ownerId: "owner_alpha",
        createdAtMs: 1234,
        updatedAtMs: 1234,
      })

      const inserted = yield* registry.upsert(created)
      expect(inserted.paywallId).toBe("paywall_1")
      expect(inserted.createdAtMs).toBe(1234)
      expect(inserted.updatedAtMs).toBeGreaterThanOrEqual(0)

      const fetched = yield* registry.getById("paywall_1")
      expect(fetched?.name).toBe(created.name)

      const ownerRows = yield* registry.listByOwner("owner_alpha")
      expect(ownerRows.length).toBe(1)
      expect(ownerRows[0]?.paywallId).toBe("paywall_1")

      const paused = yield* registry.setStatus("paywall_1", "paused")
      expect(paused?.status).toBe("paused")

      const missing = yield* registry.setStatus("missing", "active")
      expect(missing).toBeNull()
    }).pipe(Effect.provide(makeSellerDeterministicLayer())),
  )

  it.effect("gateway compiler is deterministic and flags duplicate routes", () =>
    Effect.gen(function* () {
      const compiler = yield* GatewayConfigCompilerService
      const paywallA = makePaywall("paywall_a", { priority: 10 })
      const paywallB = makePaywall("paywall_b", { priority: 5 })

      const first = yield* compiler.compilePaywalls([paywallA, paywallB])
      const second = yield* compiler.compilePaywalls([paywallA, paywallB])

      expect(first.configHash).toBe(second.configHash)
      expect(first.routes[0]?.paywallId).toBe("paywall_b")
      expect(first.valid).toBe(false)
      expect(first.diagnostics).toContain("duplicate_route:api.example.com::^/premium")
    }).pipe(Effect.provide(makeSellerDeterministicLayer())),
  )

  it.effect("invoice issuer and policy services enforce deterministic seller rules", () =>
    Effect.gen(function* () {
      const issuer = yield* InvoiceIssuerService
      const policy = yield* SellerPolicyService

      const paywall = makePaywall("paywall_1", {
        allowedBuyerHosts: ["buyer.example.com"],
        fixedAmountMsats: 2_500,
        maxAmountMsats: 2_500,
      })
      const request = makeChallengeRequest("paywall_1")

      yield* policy.ensureChallengeAllowed(paywall, request)
      const challengeA = yield* issuer.issueChallenge(request, paywall)
      const challengeB = yield* issuer.issueChallenge(request, paywall)
      expect(challengeA.status).toBe("challenge_issued")
      expect(challengeA.challenge?.invoice).toBe(challengeB.challenge?.invoice)
      expect(challengeA.challenge?.amountMsats).toBe(2_500)

      const blocked = yield* Effect.either(
        policy.ensureChallengeAllowed(
          makePaywall("paywall_1", { blockedBuyerHosts: ["buyer.example.com"] }),
          request,
        ),
      )
      expect(blocked._tag).toBe("Left")
      if (blocked._tag === "Left") {
        expect(blocked.left._tag).toBe("SellerPolicyViolationError")
        if (blocked.left._tag === "SellerPolicyViolationError") {
          expect(blocked.left.code).toBe("host_blocked")
        }
      }

      const overCap = yield* Effect.either(
        policy.ensureChallengeAllowed(
          makePaywall("paywall_2", {
            fixedAmountMsats: 1_000,
            maxAmountMsats: 2_000,
            allowedBuyerHosts: ["buyer.example.com"],
          }),
          makeChallengeRequest("paywall_2", { quotedAmountMsats: 2_001 }),
        ),
      )
      expect(overCap._tag).toBe("Left")
      if (overCap._tag === "Left") {
        expect(overCap.left._tag).toBe("SellerPolicyViolationError")
        if (overCap.left._tag === "SellerPolicyViolationError") {
          expect(overCap.left.code).toBe("amount_over_cap")
        }
      }

      const missingProofResult: L402AuthorizationVerificationResult = {
        paywallId: "paywall_1",
        requestId: "req_missing",
        status: "authorized",
        amountMsats: 1_000,
        verifiedAtMs: 1_700_000_000_010,
      }
      const missingProof = yield* Effect.either(
        policy.ensureAuthorizationAllowed(paywall, missingProofResult),
      )
      expect(missingProof._tag).toBe("Left")
      if (missingProof._tag === "Left") {
        expect(missingProof.left._tag).toBe("SellerPolicyViolationError")
        if (missingProof.left._tag === "SellerPolicyViolationError") {
          expect(missingProof.left.code).toBe("missing_proof")
        }
      }

      const deniedAuthResult: L402AuthorizationVerificationResult = {
        paywallId: "paywall_1",
        requestId: "req_denied",
        status: "denied",
        denyReasonCode: "authorization_denied",
        denyReason: "forbidden",
        verifiedAtMs: 1_700_000_000_011,
      }
      const deniedAuth = yield* Effect.either(
        policy.ensureAuthorizationAllowed(paywall, deniedAuthResult),
      )
      expect(deniedAuth._tag).toBe("Left")
      if (deniedAuth._tag === "Left") {
        expect(deniedAuth.left._tag).toBe("SellerPolicyViolationError")
        if (deniedAuth.left._tag === "SellerPolicyViolationError") {
          expect(deniedAuth.left.code).toBe("authorization_denied")
        }
      }
    }).pipe(
      Effect.provide(
        makeSellerDeterministicLayer({
          defaultAmountMsats: 9_999,
          issuer: "openagents-test",
        }),
      ),
    ),
  )

  it.effect("settlement ingest is idempotent and sortable by paywall", () =>
    Effect.gen(function* () {
      const ingest = yield* SettlementIngestService
      const firstRecord = makeSettlement("set_1", { createdAtMs: 1_700_000_000_001 })
      const secondRecord = makeSettlement("set_2", { createdAtMs: 1_700_000_000_002 })

      yield* ingest.ingest(secondRecord)
      yield* ingest.ingest(firstRecord)

      const duplicate = yield* ingest.ingest({
        ...firstRecord,
        status: "failed",
      })
      expect(duplicate.status).toBe("settled")

      const listed = yield* ingest.listByPaywall("paywall_1")
      expect(listed.length).toBe(2)
      expect(listed[0]?.settlementId).toBe("set_1")
      expect(listed[1]?.settlementId).toBe("set_2")

      const byId = yield* ingest.getBySettlementId("set_1")
      expect(byId?.status).toBe("settled")
    }).pipe(Effect.provide(makeSellerDeterministicLayer())),
  )
})
