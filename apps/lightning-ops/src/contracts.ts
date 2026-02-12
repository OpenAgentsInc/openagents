import { Schema } from "effect";

const TimestampMs = Schema.Int.pipe(Schema.nonNegative());
const PositiveMsats = Schema.Int.pipe(Schema.positive());
const NonNegativeMsats = Schema.Int.pipe(Schema.nonNegative());
const NonNegativeInt = Schema.Int.pipe(Schema.nonNegative());

export const PaywallStatus = Schema.Literal("active", "paused", "archived");
export type PaywallStatus = typeof PaywallStatus.Type;

export const PricingMode = Schema.Literal("fixed");
export type PricingMode = typeof PricingMode.Type;

export const ControlPlanePaywallPolicy = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  pricingMode: PricingMode,
  fixedAmountMsats: NonNegativeMsats,
  maxPerRequestMsats: Schema.optional(PositiveMsats),
  allowedHosts: Schema.optional(Schema.Array(Schema.NonEmptyString)),
  blockedHosts: Schema.optional(Schema.Array(Schema.NonEmptyString)),
  quotaPerMinute: Schema.optional(NonNegativeInt),
  quotaPerDay: Schema.optional(NonNegativeInt),
  killSwitch: Schema.Boolean,
  createdAtMs: TimestampMs,
  updatedAtMs: TimestampMs,
});
export type ControlPlanePaywallPolicy = typeof ControlPlanePaywallPolicy.Type;

export const ControlPlanePaywallRoute = Schema.Struct({
  routeId: Schema.NonEmptyString,
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  hostPattern: Schema.NonEmptyString,
  pathPattern: Schema.NonEmptyString,
  upstreamUrl: Schema.NonEmptyString,
  protocol: Schema.Literal("http", "https"),
  timeoutMs: NonNegativeInt,
  priority: NonNegativeInt,
  createdAtMs: TimestampMs,
  updatedAtMs: TimestampMs,
});
export type ControlPlanePaywallRoute = typeof ControlPlanePaywallRoute.Type;

export const ControlPlanePaywall = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  name: Schema.NonEmptyString,
  description: Schema.optional(Schema.String),
  status: PaywallStatus,
  requestId: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
  createdAtMs: TimestampMs,
  updatedAtMs: TimestampMs,
  policy: ControlPlanePaywallPolicy,
  routes: Schema.Array(ControlPlanePaywallRoute),
});
export type ControlPlanePaywall = typeof ControlPlanePaywall.Type;

export const ControlPlaneSnapshotResponse = Schema.Struct({
  ok: Schema.Boolean,
  paywalls: Schema.Array(ControlPlanePaywall),
});
export type ControlPlaneSnapshotResponse = typeof ControlPlaneSnapshotResponse.Type;

export const CompileDiagnosticCode = Schema.Literal(
  "invalid_pricing_mode",
  "missing_pricing",
  "invalid_route_pattern",
  "invalid_upstream_url",
  "missing_route_protocol",
  "duplicate_route",
  "ambiguous_route",
  "first_match_shadowed",
  "no_compilable_routes",
);
export type CompileDiagnosticCode = typeof CompileDiagnosticCode.Type;

export const CompileDiagnostic = Schema.Struct({
  code: CompileDiagnosticCode,
  severity: Schema.Literal("error", "warn"),
  message: Schema.NonEmptyString,
  paywallId: Schema.optional(Schema.NonEmptyString),
  routeId: Schema.optional(Schema.NonEmptyString),
  relatedRouteId: Schema.optional(Schema.NonEmptyString),
  details: Schema.optional(Schema.Unknown),
});
export type CompileDiagnostic = typeof CompileDiagnostic.Type;

export const CompiledApertureRule = Schema.Struct({
  id: Schema.NonEmptyString,
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  hostPattern: Schema.NonEmptyString,
  pathPattern: Schema.NonEmptyString,
  upstreamUrl: Schema.NonEmptyString,
  protocol: Schema.Literal("http", "https"),
  timeoutMs: NonNegativeInt,
  priority: NonNegativeInt,
  amountMsats: PositiveMsats,
});
export type CompiledApertureRule = typeof CompiledApertureRule.Type;

export const CompiledApertureArtifact = Schema.Struct({
  configHash: Schema.NonEmptyString,
  apertureYaml: Schema.String,
  rules: Schema.Array(CompiledApertureRule),
  diagnostics: Schema.Array(CompileDiagnostic),
  ruleCount: NonNegativeInt,
  valid: Schema.Boolean,
});
export type CompiledApertureArtifact = typeof CompiledApertureArtifact.Type;

export const DeploymentIntentStatus = Schema.Literal("pending", "applied", "failed", "rolled_back");
export type DeploymentIntentStatus = typeof DeploymentIntentStatus.Type;

export const DeploymentIntentRecord = Schema.Struct({
  deploymentId: Schema.NonEmptyString,
  paywallId: Schema.optional(Schema.NonEmptyString),
  ownerId: Schema.optional(Schema.NonEmptyString),
  configHash: Schema.NonEmptyString,
  imageDigest: Schema.optional(Schema.String),
  status: DeploymentIntentStatus,
  diagnostics: Schema.optional(Schema.Unknown),
  appliedAtMs: Schema.optional(TimestampMs),
  rolledBackFrom: Schema.optional(Schema.String),
  createdAtMs: TimestampMs,
  updatedAtMs: TimestampMs,
});
export type DeploymentIntentRecord = typeof DeploymentIntentRecord.Type;

export const DeploymentIntentWriteResponse = Schema.Struct({
  ok: Schema.Boolean,
  deployment: DeploymentIntentRecord,
});
export type DeploymentIntentWriteResponse = typeof DeploymentIntentWriteResponse.Type;

export const CompiledRunSummary = Schema.Struct({
  configHash: Schema.NonEmptyString,
  ruleCount: NonNegativeInt,
  valid: Schema.Boolean,
  diagnostics: Schema.Array(CompileDiagnostic),
  deploymentStatus: DeploymentIntentStatus,
  deploymentId: Schema.NonEmptyString,
});
export type CompiledRunSummary = typeof CompiledRunSummary.Type;

export const decodeControlPlaneSnapshotResponse = Schema.decodeUnknown(ControlPlaneSnapshotResponse);
export const decodeDeploymentIntentWriteResponse = Schema.decodeUnknown(DeploymentIntentWriteResponse);
export const decodeCompiledRunSummary = Schema.decodeUnknown(CompiledRunSummary);
