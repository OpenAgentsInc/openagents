import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import * as SellerContracts from "../src/contracts/seller.js"

const route: SellerContracts.PaywallRouteBinding = {
  paywallId: "paywall_1",
  hostPattern: "api.example.com",
  pathPattern: "^/premium",
  upstreamUrl: "https://upstream.example.com/premium",
  protocol: "https",
  priority: 10,
  timeoutMs: 2_000,
}

const policy: SellerContracts.PaywallPolicy = {
  paywallId: "paywall_1",
  pricingMode: "fixed_msats",
  fixedAmountMsats: 2_500,
  maxAmountMsats: 5_000,
  allowedBuyerHosts: ["buyer.example.com"],
  blockedBuyerHosts: [],
  maxRequestsPerMinute: 60,
  maxRequestsPerDay: 1_000,
  killSwitch: false,
}

const paywall: SellerContracts.PaywallDefinition = {
  paywallId: "paywall_1",
  ownerId: "user_1",
  name: "Premium Feed",
  description: "Paid feed",
  status: "active",
  createdAtMs: 1_700_000_000_000,
  updatedAtMs: 1_700_000_000_000,
  route,
  policy,
  metadata: { tier: "gold" },
}

const challengeRequest: SellerContracts.L402ChallengeIssueRequest = {
  paywallId: "paywall_1",
  requestId: "req_1",
  host: "buyer.example.com",
  path: "/premium",
  method: "GET",
  quotedAmountMsats: 2_500,
  metadata: { plan: "pro" },
}

const challengeResult: SellerContracts.L402ChallengeIssueResult = {
  paywallId: "paywall_1",
  requestId: "req_1",
  status: "challenge_issued",
  challenge: {
    invoice: "lnoa_invoice_1",
    macaroon: "macaroon_1",
    amountMsats: 2_500,
    issuer: "openagents",
  },
  issuedAtMs: 1_700_000_000_001,
}

const verification: SellerContracts.L402AuthorizationVerificationResult = {
  paywallId: "paywall_1",
  requestId: "req_1",
  status: "authorized",
  proofReference: "proof_1",
  amountMsats: 2_500,
  verifiedAtMs: 1_700_000_000_002,
}

const settlement: SellerContracts.SettlementRecord = {
  settlementId: "set_1",
  paywallId: "paywall_1",
  ownerId: "user_1",
  requestId: "req_1",
  invoice: "lnoa_invoice_1",
  paymentId: "pay_1",
  amountMsats: 2_500,
  status: "settled",
  paymentProofType: "lightning_preimage",
  proofReference: "proof_1",
  settledAtMs: 1_700_000_000_003,
  createdAtMs: 1_700_000_000_002,
}

const payout: SellerContracts.PayoutInstruction = {
  payoutId: "po_1",
  ownerId: "user_1",
  paywallId: "paywall_1",
  amountMsats: 2_500,
  destination: "node_pubkey",
  status: "queued",
  requestedAtMs: 1_700_000_000_004,
}

const routeCompiled: SellerContracts.GatewayCompiledRoute = {
  paywallId: "paywall_1",
  ownerId: "user_1",
  hostPattern: "api.example.com",
  pathPattern: "^/premium",
  upstreamUrl: "https://upstream.example.com/premium",
  priority: 10,
  status: "active",
  priceMsats: 2_500,
}

const deployment: SellerContracts.GatewayDeploymentSnapshot = {
  deploymentId: "dep_1",
  configHash: "cfg_hash_1",
  imageDigest: "sha256:abc",
  status: "deployed",
  diagnostics: [],
  appliedAtMs: 1_700_000_000_005,
}

const compiledConfig: SellerContracts.GatewayConfigCompileResult = {
  configHash: "cfg_hash_1",
  compiledConfig: "route|paywall_1|...",
  routes: [routeCompiled],
  diagnostics: [],
  valid: true,
}

describe("seller contracts", () => {
  it.effect("decodes every seller contract asynchronously", () =>
    Effect.gen(function* () {
      expect((yield* SellerContracts.decodePaywallRouteBinding(route)).hostPattern).toBe(
        route.hostPattern,
      )
      expect((yield* SellerContracts.decodePaywallPolicy(policy)).pricingMode).toBe("fixed_msats")
      expect((yield* SellerContracts.decodePaywallDefinition(paywall)).paywallId).toBe(
        paywall.paywallId,
      )
      expect(
        (yield* SellerContracts.decodeL402ChallengeIssueRequest(challengeRequest)).requestId,
      ).toBe(challengeRequest.requestId)
      expect((yield* SellerContracts.decodeL402ChallengeIssueResult(challengeResult)).status).toBe(
        "challenge_issued",
      )
      expect(
        (
          yield* SellerContracts.decodeL402AuthorizationVerificationResult(verification)
        ).status,
      ).toBe("authorized")
      expect((yield* SellerContracts.decodeSettlementRecord(settlement)).settlementId).toBe(
        settlement.settlementId,
      )
      expect((yield* SellerContracts.decodePayoutInstruction(payout)).payoutId).toBe(
        payout.payoutId,
      )
      expect((yield* SellerContracts.decodeGatewayCompiledRoute(routeCompiled)).paywallId).toBe(
        routeCompiled.paywallId,
      )
      expect((yield* SellerContracts.decodeGatewayDeploymentSnapshot(deployment)).status).toBe(
        "deployed",
      )
      expect((yield* SellerContracts.decodeGatewayConfigCompileResult(compiledConfig)).valid).toBe(
        true,
      )
    }),
  )

  it.effect("encodes every seller contract asynchronously", () =>
    Effect.gen(function* () {
      expect((yield* SellerContracts.encodePaywallRouteBinding(route)).paywallId).toBe(
        route.paywallId,
      )
      expect((yield* SellerContracts.encodePaywallPolicy(policy)).paywallId).toBe(
        policy.paywallId,
      )
      expect((yield* SellerContracts.encodePaywallDefinition(paywall)).paywallId).toBe(
        paywall.paywallId,
      )
      expect(
        (yield* SellerContracts.encodeL402ChallengeIssueRequest(challengeRequest)).requestId,
      ).toBe(challengeRequest.requestId)
      expect(
        (yield* SellerContracts.encodeL402ChallengeIssueResult(challengeResult)).requestId,
      ).toBe(challengeResult.requestId)
      expect(
        (
          yield* SellerContracts.encodeL402AuthorizationVerificationResult(verification)
        ).requestId,
      ).toBe(verification.requestId)
      expect((yield* SellerContracts.encodeSettlementRecord(settlement)).settlementId).toBe(
        settlement.settlementId,
      )
      expect((yield* SellerContracts.encodePayoutInstruction(payout)).payoutId).toBe(
        payout.payoutId,
      )
      expect((yield* SellerContracts.encodeGatewayCompiledRoute(routeCompiled)).paywallId).toBe(
        routeCompiled.paywallId,
      )
      expect((yield* SellerContracts.encodeGatewayDeploymentSnapshot(deployment)).deploymentId).toBe(
        deployment.deploymentId,
      )
      expect(
        (yield* SellerContracts.encodeGatewayConfigCompileResult(compiledConfig)).configHash,
      ).toBe(compiledConfig.configHash)
    }),
  )

  it.effect("supports sync decode/encode and typed deterministic decode errors", () =>
    Effect.gen(function* () {
      expect(SellerContracts.decodePaywallRouteBindingSync(route).paywallId).toBe(route.paywallId)
      expect(SellerContracts.decodePaywallPolicySync(policy).paywallId).toBe(policy.paywallId)
      expect(SellerContracts.decodePaywallDefinitionSync(paywall).paywallId).toBe(paywall.paywallId)
      expect(
        SellerContracts.decodeL402ChallengeIssueRequestSync(challengeRequest).requestId,
      ).toBe(challengeRequest.requestId)
      expect(
        SellerContracts.decodeL402ChallengeIssueResultSync(challengeResult).requestId,
      ).toBe(challengeResult.requestId)
      expect(
        SellerContracts.decodeL402AuthorizationVerificationResultSync(verification).requestId,
      ).toBe(verification.requestId)
      expect(SellerContracts.decodeSettlementRecordSync(settlement).settlementId).toBe(
        settlement.settlementId,
      )
      expect(SellerContracts.decodePayoutInstructionSync(payout).payoutId).toBe(payout.payoutId)
      expect(SellerContracts.decodeGatewayCompiledRouteSync(routeCompiled).paywallId).toBe(
        routeCompiled.paywallId,
      )
      expect(SellerContracts.decodeGatewayDeploymentSnapshotSync(deployment).deploymentId).toBe(
        deployment.deploymentId,
      )
      expect(
        SellerContracts.decodeGatewayConfigCompileResultSync(compiledConfig).configHash,
      ).toBe(compiledConfig.configHash)

      expect(SellerContracts.encodePaywallRouteBindingSync(route).paywallId).toBe(route.paywallId)
      expect(SellerContracts.encodePaywallPolicySync(policy).paywallId).toBe(policy.paywallId)
      expect(SellerContracts.encodePaywallDefinitionSync(paywall).paywallId).toBe(paywall.paywallId)
      expect(
        SellerContracts.encodeL402ChallengeIssueRequestSync(challengeRequest).requestId,
      ).toBe(challengeRequest.requestId)
      expect(
        SellerContracts.encodeL402ChallengeIssueResultSync(challengeResult).requestId,
      ).toBe(challengeResult.requestId)
      expect(
        SellerContracts.encodeL402AuthorizationVerificationResultSync(verification).requestId,
      ).toBe(verification.requestId)
      expect(SellerContracts.encodeSettlementRecordSync(settlement).settlementId).toBe(
        settlement.settlementId,
      )
      expect(SellerContracts.encodePayoutInstructionSync(payout).payoutId).toBe(payout.payoutId)
      expect(SellerContracts.encodeGatewayCompiledRouteSync(routeCompiled).paywallId).toBe(
        routeCompiled.paywallId,
      )
      expect(SellerContracts.encodeGatewayDeploymentSnapshotSync(deployment).deploymentId).toBe(
        deployment.deploymentId,
      )
      expect(
        SellerContracts.encodeGatewayConfigCompileResultSync(compiledConfig).configHash,
      ).toBe(compiledConfig.configHash)

      const invalidPaywall = yield* Effect.either(
        SellerContracts.decodePaywallDefinition({
          ...paywall,
          status: "running",
        }),
      )
      expect(invalidPaywall._tag).toBe("Left")
      if (invalidPaywall._tag === "Left") {
        expect(invalidPaywall.left._tag).toBe("SellerContractDecodeError")
        if (invalidPaywall.left._tag === "SellerContractDecodeError") {
          expect(invalidPaywall.left.contract).toBe("PaywallDefinition")
        }
      }

      const invalidSettlement = yield* Effect.either(
        SellerContracts.decodeSettlementRecord({
          ...settlement,
          amountMsats: -1,
        }),
      )
      expect(invalidSettlement._tag).toBe("Left")
      if (invalidSettlement._tag === "Left") {
        expect(invalidSettlement.left._tag).toBe("SellerContractDecodeError")
        if (invalidSettlement.left._tag === "SellerContractDecodeError") {
          expect(invalidSettlement.left.contract).toBe("SettlementRecord")
        }
      }

      expect(() =>
        SellerContracts.decodePaywallDefinitionSync({
          ...paywall,
          paywallId: "",
        }),
      ).toThrowError(
        expect.objectContaining({
          _tag: "SellerContractDecodeError",
          contract: "PaywallDefinition",
        }),
      )
    }),
  )
})
