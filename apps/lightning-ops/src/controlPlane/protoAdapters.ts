import { Effect } from "effect";

import {
  decodeControlPlaneSecurityStateResponse as decodeLegacyControlPlaneSecurityStateResponse,
  decodeDeploymentIntentWriteResponse as decodeLegacyDeploymentIntentWriteResponse,
  decodeGatewayEventWriteResponse as decodeLegacyGatewayEventWriteResponse,
  decodeInvoiceLifecycleWriteResponse as decodeLegacyInvoiceLifecycleWriteResponse,
  decodeSecurityCredentialRoleWriteResponse as decodeLegacySecurityCredentialRoleWriteResponse,
  decodeSecurityGlobalWriteResponse as decodeLegacySecurityGlobalWriteResponse,
  decodeSecurityOwnerControlWriteResponse as decodeLegacySecurityOwnerControlWriteResponse,
  decodeSettlementWriteResponse as decodeLegacySettlementWriteResponse,
  decodeControlPlaneSnapshotResponse as decodeLegacySnapshotResponse,
} from "../contracts.js";

const INTEGER_FIELDS = new Set([
  "fixedAmountMsats",
  "maxPerRequestMsats",
  "quotaPerMinute",
  "quotaPerDay",
  "timeoutMs",
  "priority",
  "createdAtMs",
  "updatedAtMs",
  "appliedAtMs",
  "amountMsats",
  "settledAtMs",
  "version",
  "lastRotatedAtMs",
  "revokedAtMs",
  "retryAfterMs",
]);

const RAW_JSON_FIELDS = new Set(["metadata", "details", "diagnostics"]);

const PROTO_ENUM_VALUE_MAP: Record<string, string | undefined> = {
  PAYWALL_STATUS_UNSPECIFIED: undefined,
  PAYWALL_STATUS_ACTIVE: "active",
  PAYWALL_STATUS_PAUSED: "paused",
  PAYWALL_STATUS_ARCHIVED: "archived",
  PRICING_MODE_UNSPECIFIED: undefined,
  PRICING_MODE_FIXED: "fixed",
  ROUTE_PROTOCOL_UNSPECIFIED: undefined,
  ROUTE_PROTOCOL_HTTP: "http",
  ROUTE_PROTOCOL_HTTPS: "https",
  COMPILE_DIAGNOSTIC_CODE_UNSPECIFIED: undefined,
  COMPILE_DIAGNOSTIC_CODE_INVALID_PRICING_MODE: "invalid_pricing_mode",
  COMPILE_DIAGNOSTIC_CODE_MISSING_PRICING: "missing_pricing",
  COMPILE_DIAGNOSTIC_CODE_INVALID_ROUTE_PATTERN: "invalid_route_pattern",
  COMPILE_DIAGNOSTIC_CODE_INVALID_UPSTREAM_URL: "invalid_upstream_url",
  COMPILE_DIAGNOSTIC_CODE_MISSING_ROUTE_PROTOCOL: "missing_route_protocol",
  COMPILE_DIAGNOSTIC_CODE_DUPLICATE_ROUTE: "duplicate_route",
  COMPILE_DIAGNOSTIC_CODE_AMBIGUOUS_ROUTE: "ambiguous_route",
  COMPILE_DIAGNOSTIC_CODE_FIRST_MATCH_SHADOWED: "first_match_shadowed",
  COMPILE_DIAGNOSTIC_CODE_NO_COMPILABLE_ROUTES: "no_compilable_routes",
  COMPILE_DIAGNOSTIC_SEVERITY_UNSPECIFIED: undefined,
  COMPILE_DIAGNOSTIC_SEVERITY_ERROR: "error",
  COMPILE_DIAGNOSTIC_SEVERITY_WARN: "warn",
  DEPLOYMENT_INTENT_STATUS_UNSPECIFIED: undefined,
  DEPLOYMENT_INTENT_STATUS_PENDING: "pending",
  DEPLOYMENT_INTENT_STATUS_APPLIED: "applied",
  DEPLOYMENT_INTENT_STATUS_FAILED: "failed",
  DEPLOYMENT_INTENT_STATUS_ROLLED_BACK: "rolled_back",
  GATEWAY_EVENT_LEVEL_UNSPECIFIED: undefined,
  GATEWAY_EVENT_LEVEL_INFO: "info",
  GATEWAY_EVENT_LEVEL_WARN: "warn",
  GATEWAY_EVENT_LEVEL_ERROR: "error",
  INVOICE_LIFECYCLE_STATUS_UNSPECIFIED: undefined,
  INVOICE_LIFECYCLE_STATUS_OPEN: "open",
  INVOICE_LIFECYCLE_STATUS_SETTLED: "settled",
  INVOICE_LIFECYCLE_STATUS_CANCELED: "canceled",
  INVOICE_LIFECYCLE_STATUS_EXPIRED: "expired",
  PAYMENT_PROOF_TYPE_UNSPECIFIED: undefined,
  PAYMENT_PROOF_TYPE_LIGHTNING_PREIMAGE: "lightning_preimage",
  SECURITY_DENY_REASON_CODE_UNSPECIFIED: undefined,
  SECURITY_DENY_REASON_CODE_GLOBAL_PAUSE_ACTIVE: "global_pause_active",
  SECURITY_DENY_REASON_CODE_OWNER_KILL_SWITCH_ACTIVE: "owner_kill_switch_active",
  CREDENTIAL_ROLE_UNSPECIFIED: undefined,
  CREDENTIAL_ROLE_GATEWAY_INVOICE: "gateway_invoice",
  CREDENTIAL_ROLE_SETTLEMENT_READ: "settlement_read",
  CREDENTIAL_ROLE_OPERATOR_ADMIN: "operator_admin",
  CREDENTIAL_ROLE_STATUS_UNSPECIFIED: undefined,
  CREDENTIAL_ROLE_STATUS_ACTIVE: "active",
  CREDENTIAL_ROLE_STATUS_ROTATING: "rotating",
  CREDENTIAL_ROLE_STATUS_REVOKED: "revoked",
};

const maybeParseIntegerString = (value: string): number | undefined => {
  if (!/^-?\d+$/.test(value)) {
    return undefined;
  }

  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : undefined;
};

const toCamelCase = (value: string): string =>
  value.includes("_")
    ? value.replace(/_([a-z])/g, (_, character: string) => character.toUpperCase())
    : value;

const normalizeProtoValue = (value: unknown, key?: string): unknown => {
  if (Array.isArray(value)) {
    return value.map((entry) => normalizeProtoValue(entry));
  }

  if (value !== null && typeof value === "object") {
    const record = value as Record<string, unknown>;
    const normalized: Record<string, unknown> = {};

    for (const [rawKey, rawEntry] of Object.entries(record)) {
      if (rawEntry === undefined) {
        continue;
      }

      const entryKey = toCamelCase(rawKey);
      if (RAW_JSON_FIELDS.has(entryKey)) {
        normalized[entryKey] = rawEntry;
        continue;
      }

      const nextValue = normalizeProtoValue(rawEntry, entryKey);
      if (nextValue !== undefined) {
        normalized[entryKey] = nextValue;
      }
    }

    return normalized;
  }

  if (typeof value === "string") {
    if (key && INTEGER_FIELDS.has(key)) {
      const parsed = maybeParseIntegerString(value);
      if (parsed !== undefined) {
        return parsed;
      }
    }

    if (Object.hasOwn(PROTO_ENUM_VALUE_MAP, value)) {
      return PROTO_ENUM_VALUE_MAP[value];
    }
  }

  return value;
};

const decodeWithProtoFallback = <A>(
  raw: unknown,
  decode: (input: unknown) => Effect.Effect<A, unknown>,
): Effect.Effect<A, unknown> => {
  const normalized = normalizeProtoValue(raw);
  return decode(raw).pipe(Effect.orElse(() => decode(normalized)));
};

export const decodeControlPlaneSnapshotResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacySnapshotResponse);

export const decodeDeploymentIntentWriteResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacyDeploymentIntentWriteResponse);

export const decodeGatewayEventWriteResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacyGatewayEventWriteResponse);

export const decodeInvoiceLifecycleWriteResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacyInvoiceLifecycleWriteResponse);

export const decodeSettlementWriteResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacySettlementWriteResponse);

export const decodeControlPlaneSecurityStateResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacyControlPlaneSecurityStateResponse);

export const decodeSecurityGlobalWriteResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacySecurityGlobalWriteResponse);

export const decodeSecurityOwnerControlWriteResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacySecurityOwnerControlWriteResponse);

export const decodeSecurityCredentialRoleWriteResponseFromAny = (raw: unknown) =>
  decodeWithProtoFallback(raw, decodeLegacySecurityCredentialRoleWriteResponse);

export const normalizeControlPlaneProtoPayload = (raw: unknown): unknown => normalizeProtoValue(raw);
