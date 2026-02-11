import { Effect, Schema } from "effect"

import { SellerContractDecodeError } from "../errors/lightningErrors.js"
import { HttpMethod, L402Challenge } from "./l402.js"
import { Msats } from "./payment.js"

const TimestampMs = Schema.Int.pipe(Schema.nonNegative())
const PositiveInt = Schema.Int.pipe(Schema.positive())
const NonNegativeInt = Schema.Int.pipe(Schema.nonNegative())

const decodeWithTypedError = <A>(
  contract: string,
  schema: Schema.Schema<A>,
  input: unknown,
): Effect.Effect<A, SellerContractDecodeError> =>
  Schema.decodeUnknown(schema)(input).pipe(
    Effect.mapError((error) =>
      SellerContractDecodeError.make({
        contract,
        reason: String(error),
      }),
    ),
  )

const decodeWithTypedErrorSync = <A>(
  contract: string,
  schema: Schema.Schema<A>,
  input: unknown,
): A => {
  try {
    return Schema.decodeUnknownSync(schema)(input)
  } catch (error) {
    throw SellerContractDecodeError.make({
      contract,
      reason: String(error),
    })
  }
}

export const PaywallStatus = Schema.Literal("draft", "active", "paused", "archived")
export type PaywallStatus = typeof PaywallStatus.Type

export const SellerPolicyDenyCode = Schema.Literal(
  "paywall_inactive",
  "kill_switch_active",
  "host_not_allowlisted",
  "host_blocked",
  "amount_over_cap",
  "authorization_denied",
  "missing_proof",
)
export type SellerPolicyDenyCode = typeof SellerPolicyDenyCode.Type

export const PaywallRouteBinding = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  hostPattern: Schema.NonEmptyString,
  pathPattern: Schema.NonEmptyString,
  upstreamUrl: Schema.NonEmptyString,
  protocol: Schema.optional(Schema.Literal("http", "https")),
  priority: NonNegativeInt,
  timeoutMs: Schema.optional(PositiveInt),
})
export type PaywallRouteBinding = typeof PaywallRouteBinding.Type

export const PaywallPolicy = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  pricingMode: Schema.Literal("fixed_msats", "dynamic"),
  fixedAmountMsats: Schema.optional(Msats),
  maxAmountMsats: Schema.optional(Msats),
  allowedBuyerHosts: Schema.Array(Schema.NonEmptyString),
  blockedBuyerHosts: Schema.Array(Schema.NonEmptyString),
  maxRequestsPerMinute: Schema.optional(NonNegativeInt),
  maxRequestsPerDay: Schema.optional(NonNegativeInt),
  killSwitch: Schema.Boolean,
})
export type PaywallPolicy = typeof PaywallPolicy.Type

export const PaywallDefinition = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  name: Schema.NonEmptyString,
  description: Schema.optional(Schema.String),
  status: PaywallStatus,
  createdAtMs: TimestampMs,
  updatedAtMs: TimestampMs,
  route: PaywallRouteBinding,
  policy: PaywallPolicy,
  metadata: Schema.optional(Schema.Unknown),
})
export type PaywallDefinition = typeof PaywallDefinition.Type

export const L402ChallengeIssueRequest = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  requestId: Schema.NonEmptyString,
  host: Schema.NonEmptyString,
  path: Schema.NonEmptyString,
  method: Schema.optional(HttpMethod),
  quotedAmountMsats: Schema.optional(Msats),
  metadata: Schema.optional(Schema.Unknown),
})
export type L402ChallengeIssueRequest = typeof L402ChallengeIssueRequest.Type

export const L402ChallengeIssueResult = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  requestId: Schema.NonEmptyString,
  status: Schema.Literal("challenge_issued", "denied"),
  challenge: Schema.optional(L402Challenge),
  denyReasonCode: Schema.optional(SellerPolicyDenyCode),
  denyReason: Schema.optional(Schema.NonEmptyString),
  issuedAtMs: TimestampMs,
})
export type L402ChallengeIssueResult = typeof L402ChallengeIssueResult.Type

export const L402AuthorizationVerificationResult = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  requestId: Schema.NonEmptyString,
  status: Schema.Literal("authorized", "denied"),
  proofReference: Schema.optional(Schema.NonEmptyString),
  denyReasonCode: Schema.optional(SellerPolicyDenyCode),
  denyReason: Schema.optional(Schema.NonEmptyString),
  amountMsats: Schema.optional(Msats),
  verifiedAtMs: TimestampMs,
})
export type L402AuthorizationVerificationResult = typeof L402AuthorizationVerificationResult.Type

export const SettlementRecord = Schema.Struct({
  settlementId: Schema.NonEmptyString,
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  requestId: Schema.NonEmptyString,
  invoice: Schema.NonEmptyString,
  paymentId: Schema.optional(Schema.NonEmptyString),
  amountMsats: Msats,
  status: Schema.Literal("pending", "settled", "failed", "expired", "canceled"),
  paymentProofType: Schema.Literal("lightning_preimage"),
  proofReference: Schema.optional(Schema.NonEmptyString),
  settledAtMs: Schema.optional(TimestampMs),
  failureReason: Schema.optional(Schema.String),
  createdAtMs: TimestampMs,
})
export type SettlementRecord = typeof SettlementRecord.Type

export const PayoutInstruction = Schema.Struct({
  payoutId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  paywallId: Schema.NonEmptyString,
  amountMsats: Msats,
  destination: Schema.NonEmptyString,
  status: Schema.Literal("queued", "processing", "completed", "failed", "canceled"),
  requestedAtMs: TimestampMs,
  processedAtMs: Schema.optional(TimestampMs),
  failureReason: Schema.optional(Schema.String),
})
export type PayoutInstruction = typeof PayoutInstruction.Type

export const GatewayCompiledRoute = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  hostPattern: Schema.NonEmptyString,
  pathPattern: Schema.NonEmptyString,
  upstreamUrl: Schema.NonEmptyString,
  priority: NonNegativeInt,
  status: PaywallStatus,
  priceMsats: Msats,
})
export type GatewayCompiledRoute = typeof GatewayCompiledRoute.Type

export const GatewayDeploymentSnapshot = Schema.Struct({
  deploymentId: Schema.NonEmptyString,
  configHash: Schema.NonEmptyString,
  imageDigest: Schema.optional(Schema.NonEmptyString),
  status: Schema.Literal("pending", "deployed", "failed", "rolled_back"),
  appliedAtMs: Schema.optional(TimestampMs),
  diagnostics: Schema.Array(Schema.NonEmptyString),
  rollbackFromConfigHash: Schema.optional(Schema.NonEmptyString),
})
export type GatewayDeploymentSnapshot = typeof GatewayDeploymentSnapshot.Type

export const GatewayConfigCompileResult = Schema.Struct({
  configHash: Schema.NonEmptyString,
  compiledConfig: Schema.String,
  routes: Schema.Array(GatewayCompiledRoute),
  diagnostics: Schema.Array(Schema.NonEmptyString),
  valid: Schema.Boolean,
})
export type GatewayConfigCompileResult = typeof GatewayConfigCompileResult.Type

export const decodePaywallRouteBinding = (input: unknown) =>
  decodeWithTypedError("PaywallRouteBinding", PaywallRouteBinding, input)
export const decodePaywallRouteBindingSync = (input: unknown) =>
  decodeWithTypedErrorSync("PaywallRouteBinding", PaywallRouteBinding, input)
export const encodePaywallRouteBinding = Schema.encode(PaywallRouteBinding)
export const encodePaywallRouteBindingSync = Schema.encodeSync(PaywallRouteBinding)

export const decodePaywallPolicy = (input: unknown) =>
  decodeWithTypedError("PaywallPolicy", PaywallPolicy, input)
export const decodePaywallPolicySync = (input: unknown) =>
  decodeWithTypedErrorSync("PaywallPolicy", PaywallPolicy, input)
export const encodePaywallPolicy = Schema.encode(PaywallPolicy)
export const encodePaywallPolicySync = Schema.encodeSync(PaywallPolicy)

export const decodePaywallDefinition = (input: unknown) =>
  decodeWithTypedError("PaywallDefinition", PaywallDefinition, input)
export const decodePaywallDefinitionSync = (input: unknown) =>
  decodeWithTypedErrorSync("PaywallDefinition", PaywallDefinition, input)
export const encodePaywallDefinition = Schema.encode(PaywallDefinition)
export const encodePaywallDefinitionSync = Schema.encodeSync(PaywallDefinition)

export const decodeL402ChallengeIssueRequest = (input: unknown) =>
  decodeWithTypedError("L402ChallengeIssueRequest", L402ChallengeIssueRequest, input)
export const decodeL402ChallengeIssueRequestSync = (input: unknown) =>
  decodeWithTypedErrorSync("L402ChallengeIssueRequest", L402ChallengeIssueRequest, input)
export const encodeL402ChallengeIssueRequest = Schema.encode(L402ChallengeIssueRequest)
export const encodeL402ChallengeIssueRequestSync = Schema.encodeSync(L402ChallengeIssueRequest)

export const decodeL402ChallengeIssueResult = (input: unknown) =>
  decodeWithTypedError("L402ChallengeIssueResult", L402ChallengeIssueResult, input)
export const decodeL402ChallengeIssueResultSync = (input: unknown) =>
  decodeWithTypedErrorSync("L402ChallengeIssueResult", L402ChallengeIssueResult, input)
export const encodeL402ChallengeIssueResult = Schema.encode(L402ChallengeIssueResult)
export const encodeL402ChallengeIssueResultSync = Schema.encodeSync(L402ChallengeIssueResult)

export const decodeL402AuthorizationVerificationResult = (input: unknown) =>
  decodeWithTypedError(
    "L402AuthorizationVerificationResult",
    L402AuthorizationVerificationResult,
    input,
  )
export const decodeL402AuthorizationVerificationResultSync = (input: unknown) =>
  decodeWithTypedErrorSync(
    "L402AuthorizationVerificationResult",
    L402AuthorizationVerificationResult,
    input,
  )
export const encodeL402AuthorizationVerificationResult = Schema.encode(
  L402AuthorizationVerificationResult,
)
export const encodeL402AuthorizationVerificationResultSync = Schema.encodeSync(
  L402AuthorizationVerificationResult,
)

export const decodeSettlementRecord = (input: unknown) =>
  decodeWithTypedError("SettlementRecord", SettlementRecord, input)
export const decodeSettlementRecordSync = (input: unknown) =>
  decodeWithTypedErrorSync("SettlementRecord", SettlementRecord, input)
export const encodeSettlementRecord = Schema.encode(SettlementRecord)
export const encodeSettlementRecordSync = Schema.encodeSync(SettlementRecord)

export const decodePayoutInstruction = (input: unknown) =>
  decodeWithTypedError("PayoutInstruction", PayoutInstruction, input)
export const decodePayoutInstructionSync = (input: unknown) =>
  decodeWithTypedErrorSync("PayoutInstruction", PayoutInstruction, input)
export const encodePayoutInstruction = Schema.encode(PayoutInstruction)
export const encodePayoutInstructionSync = Schema.encodeSync(PayoutInstruction)

export const decodeGatewayCompiledRoute = (input: unknown) =>
  decodeWithTypedError("GatewayCompiledRoute", GatewayCompiledRoute, input)
export const decodeGatewayCompiledRouteSync = (input: unknown) =>
  decodeWithTypedErrorSync("GatewayCompiledRoute", GatewayCompiledRoute, input)
export const encodeGatewayCompiledRoute = Schema.encode(GatewayCompiledRoute)
export const encodeGatewayCompiledRouteSync = Schema.encodeSync(GatewayCompiledRoute)

export const decodeGatewayDeploymentSnapshot = (input: unknown) =>
  decodeWithTypedError("GatewayDeploymentSnapshot", GatewayDeploymentSnapshot, input)
export const decodeGatewayDeploymentSnapshotSync = (input: unknown) =>
  decodeWithTypedErrorSync("GatewayDeploymentSnapshot", GatewayDeploymentSnapshot, input)
export const encodeGatewayDeploymentSnapshot = Schema.encode(GatewayDeploymentSnapshot)
export const encodeGatewayDeploymentSnapshotSync = Schema.encodeSync(GatewayDeploymentSnapshot)

export const decodeGatewayConfigCompileResult = (input: unknown) =>
  decodeWithTypedError("GatewayConfigCompileResult", GatewayConfigCompileResult, input)
export const decodeGatewayConfigCompileResultSync = (input: unknown) =>
  decodeWithTypedErrorSync("GatewayConfigCompileResult", GatewayConfigCompileResult, input)
export const encodeGatewayConfigCompileResult = Schema.encode(GatewayConfigCompileResult)
export const encodeGatewayConfigCompileResultSync = Schema.encodeSync(GatewayConfigCompileResult)
