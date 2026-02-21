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

export const GatewayEventLevel = Schema.Literal("info", "warn", "error");
export type GatewayEventLevel = typeof GatewayEventLevel.Type;

export const GatewayEventRecord = Schema.Struct({
  eventId: Schema.NonEmptyString,
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  eventType: Schema.NonEmptyString,
  level: GatewayEventLevel,
  requestId: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
  createdAtMs: TimestampMs,
});
export type GatewayEventRecord = typeof GatewayEventRecord.Type;

export const GatewayEventWriteResponse = Schema.Struct({
  ok: Schema.Boolean,
  event: GatewayEventRecord,
});
export type GatewayEventWriteResponse = typeof GatewayEventWriteResponse.Type;

export const InvoiceLifecycleStatus = Schema.Literal("open", "settled", "canceled", "expired");
export type InvoiceLifecycleStatus = typeof InvoiceLifecycleStatus.Type;

export const PaymentProofType = Schema.Literal("lightning_preimage");
export type PaymentProofType = typeof PaymentProofType.Type;

export const ControlPlaneInvoiceRecord = Schema.Struct({
  invoiceId: Schema.NonEmptyString,
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  amountMsats: PositiveMsats,
  status: InvoiceLifecycleStatus,
  paymentHash: Schema.optional(Schema.String),
  paymentRequest: Schema.optional(Schema.String),
  paymentProofRef: Schema.optional(Schema.String),
  requestId: Schema.optional(Schema.String),
  createdAtMs: TimestampMs,
  updatedAtMs: TimestampMs,
  settledAtMs: Schema.optional(TimestampMs),
});
export type ControlPlaneInvoiceRecord = typeof ControlPlaneInvoiceRecord.Type;

export const ControlPlaneSettlementRecord = Schema.Struct({
  settlementId: Schema.NonEmptyString,
  paywallId: Schema.NonEmptyString,
  ownerId: Schema.NonEmptyString,
  invoiceId: Schema.optional(Schema.String),
  amountMsats: PositiveMsats,
  paymentProofRef: Schema.NonEmptyString,
  requestId: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
  createdAtMs: TimestampMs,
});
export type ControlPlaneSettlementRecord = typeof ControlPlaneSettlementRecord.Type;

export const InvoiceLifecycleWriteResponse = Schema.Struct({
  ok: Schema.Boolean,
  changed: Schema.Boolean,
  invoice: ControlPlaneInvoiceRecord,
});
export type InvoiceLifecycleWriteResponse = typeof InvoiceLifecycleWriteResponse.Type;

export const SettlementWriteResponse = Schema.Struct({
  ok: Schema.Boolean,
  existed: Schema.Boolean,
  settlement: ControlPlaneSettlementRecord,
  invoice: Schema.optional(ControlPlaneInvoiceRecord),
});
export type SettlementWriteResponse = typeof SettlementWriteResponse.Type;

export const SecurityDenyReasonCode = Schema.Literal("global_pause_active", "owner_kill_switch_active");
export type SecurityDenyReasonCode = typeof SecurityDenyReasonCode.Type;

export const CredentialRole = Schema.Literal("gateway_invoice", "settlement_read", "operator_admin");
export type CredentialRole = typeof CredentialRole.Type;

export const CredentialRoleStatus = Schema.Literal("active", "rotating", "revoked");
export type CredentialRoleStatus = typeof CredentialRoleStatus.Type;

export const ControlPlaneSecurityGlobal = Schema.Struct({
  stateId: Schema.NonEmptyString,
  globalPause: Schema.Boolean,
  denyReasonCode: Schema.optional(Schema.Literal("global_pause_active")),
  denyReason: Schema.optional(Schema.String),
  updatedBy: Schema.optional(Schema.String),
  updatedAtMs: TimestampMs,
});
export type ControlPlaneSecurityGlobal = typeof ControlPlaneSecurityGlobal.Type;

export const ControlPlaneOwnerSecurityControl = Schema.Struct({
  ownerId: Schema.NonEmptyString,
  killSwitch: Schema.Boolean,
  denyReasonCode: Schema.optional(Schema.Literal("owner_kill_switch_active")),
  denyReason: Schema.optional(Schema.String),
  updatedBy: Schema.optional(Schema.String),
  updatedAtMs: TimestampMs,
});
export type ControlPlaneOwnerSecurityControl = typeof ControlPlaneOwnerSecurityControl.Type;

export const ControlPlaneCredentialRoleState = Schema.Struct({
  role: CredentialRole,
  status: CredentialRoleStatus,
  version: NonNegativeInt,
  fingerprint: Schema.optional(Schema.String),
  note: Schema.optional(Schema.String),
  updatedAtMs: TimestampMs,
  lastRotatedAtMs: Schema.optional(TimestampMs),
  revokedAtMs: Schema.optional(TimestampMs),
});
export type ControlPlaneCredentialRoleState = typeof ControlPlaneCredentialRoleState.Type;

export const ControlPlaneSecurityGate = Schema.Struct({
  allowed: Schema.Boolean,
  denyReasonCode: Schema.optional(SecurityDenyReasonCode),
  denyReason: Schema.optional(Schema.String),
});
export type ControlPlaneSecurityGate = typeof ControlPlaneSecurityGate.Type;

export const ControlPlaneSecurityStateResponse = Schema.Struct({
  ok: Schema.Boolean,
  global: ControlPlaneSecurityGlobal,
  ownerControls: Schema.Array(ControlPlaneOwnerSecurityControl),
  credentialRoles: Schema.Array(ControlPlaneCredentialRoleState),
});
export type ControlPlaneSecurityStateResponse = typeof ControlPlaneSecurityStateResponse.Type;

export const SecurityGlobalWriteResponse = Schema.Struct({
  ok: Schema.Boolean,
  global: ControlPlaneSecurityGlobal,
});
export type SecurityGlobalWriteResponse = typeof SecurityGlobalWriteResponse.Type;

export const SecurityOwnerControlWriteResponse = Schema.Struct({
  ok: Schema.Boolean,
  ownerControl: ControlPlaneOwnerSecurityControl,
});
export type SecurityOwnerControlWriteResponse = typeof SecurityOwnerControlWriteResponse.Type;

export const SecurityCredentialRoleWriteResponse = Schema.Struct({
  ok: Schema.Boolean,
  role: ControlPlaneCredentialRoleState,
});
export type SecurityCredentialRoleWriteResponse = typeof SecurityCredentialRoleWriteResponse.Type;

export const CompiledRunSummary = Schema.Struct({
  configHash: Schema.NonEmptyString,
  ruleCount: NonNegativeInt,
  valid: Schema.Boolean,
  diagnostics: Schema.Array(CompileDiagnostic),
  deploymentStatus: DeploymentIntentStatus,
  deploymentId: Schema.NonEmptyString,
});
export type CompiledRunSummary = typeof CompiledRunSummary.Type;

export const ReconcileFailureCode = Schema.Literal(
  "compile_validation_failed",
  "active_lookup_failed",
  "deploy_apply_failed",
  "health_check_failed",
  "challenge_check_failed",
  "proxy_check_failed",
  "rollback_failed",
);
export type ReconcileFailureCode = typeof ReconcileFailureCode.Type;

export const ReconcileTerminalStatus = Schema.Literal("applied", "failed", "rolled_back");
export type ReconcileTerminalStatus = typeof ReconcileTerminalStatus.Type;

export const ReconcileRunSummary = Schema.Struct({
  requestId: Schema.NonEmptyString,
  executionPath: Schema.Literal("hosted-node"),
  configHash: Schema.NonEmptyString,
  ruleCount: NonNegativeInt,
  valid: Schema.Boolean,
  diagnostics: Schema.Array(CompileDiagnostic),
  deploymentStatus: ReconcileTerminalStatus,
  deploymentId: Schema.NonEmptyString,
  failureCode: Schema.optional(ReconcileFailureCode),
  imageDigest: Schema.optional(Schema.NonEmptyString),
  rolledBackFrom: Schema.optional(Schema.NonEmptyString),
  healthOk: Schema.Boolean,
  challengeOk: Schema.Boolean,
  proxyOk: Schema.Boolean,
});
export type ReconcileRunSummary = typeof ReconcileRunSummary.Type;

export const decodeControlPlaneSnapshotResponse = Schema.decodeUnknown(ControlPlaneSnapshotResponse);
export const decodeDeploymentIntentWriteResponse = Schema.decodeUnknown(DeploymentIntentWriteResponse);
export const decodeGatewayEventWriteResponse = Schema.decodeUnknown(GatewayEventWriteResponse);
export const decodeInvoiceLifecycleWriteResponse = Schema.decodeUnknown(InvoiceLifecycleWriteResponse);
export const decodeSettlementWriteResponse = Schema.decodeUnknown(SettlementWriteResponse);
export const decodeControlPlaneSecurityStateResponse = Schema.decodeUnknown(ControlPlaneSecurityStateResponse);
export const decodeSecurityGlobalWriteResponse = Schema.decodeUnknown(SecurityGlobalWriteResponse);
export const decodeSecurityOwnerControlWriteResponse = Schema.decodeUnknown(SecurityOwnerControlWriteResponse);
export const decodeSecurityCredentialRoleWriteResponse = Schema.decodeUnknown(SecurityCredentialRoleWriteResponse);
export const decodeCompiledRunSummary = Schema.decodeUnknown(CompiledRunSummary);
export const decodeReconcileRunSummary = Schema.decodeUnknown(ReconcileRunSummary);
