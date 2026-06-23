import { Schema as S } from "effect";

// ---------------------------------------------------------------------------
// Canonical Blueprint contract-export security contract.
//
// This is the ONE authority for:
//   - the `BlueprintContractExportSeed` shape (and its catalog entries), and
//   - the security-critical `IsPrivateDataSafe` private-data-safety predicate
//     family that decides whether a Blueprint projection / contract-export seed
//     is safe to expose.
//
// Previously these existed as hand-maintained copies in the openagents.com
// Worker (`workers/api/src/blueprint/exports/contract-export.ts`, with a WEAK
// regex-on-stringified-JSON predicate) and in the Probe/Pylon runtimes
// (`blueprint/contracts.ts`, with the stronger recursive field+value walk).
// They had drifted: the predicates checked DIFFERENT field sets. This package
// elevates the stronger recursive predicate as the single authority; the
// consumers re-export it.
// ---------------------------------------------------------------------------

export const BlueprintContractConsumer = S.Literals([
  "ai_agent",
  "nexus",
  "oa_node",
  "oa_workroomd",
  "probe",
  "psionic",
  "pylon",
  "treasury",
]);
export type BlueprintContractConsumer = typeof BlueprintContractConsumer.Type;

export const BlueprintContractStability = S.Literals(["seed", "stable"]);
export type BlueprintContractStability = typeof BlueprintContractStability.Type;

export const BlueprintContractPrivacyPolicy = S.Literals(["public_refs_only", "operator_refs_only"]);
export type BlueprintContractPrivacyPolicy = typeof BlueprintContractPrivacyPolicy.Type;

export const BlueprintJsonSchemaContract = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  id: S.String,
  jsonSchemaUrl: S.String,
  name: S.String,
  openApiComponentRef: S.String,
  privacyPolicy: BlueprintContractPrivacyPolicy,
  schemaRef: S.String,
  stability: BlueprintContractStability,
  versionRef: S.String,
});
export type BlueprintJsonSchemaContract = typeof BlueprintJsonSchemaContract.Type;

export const BlueprintOpenApiContract = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  id: S.String,
  method: S.String,
  operationRef: S.String,
  path: S.String,
  privacyPolicy: BlueprintContractPrivacyPolicy,
  requestSchemaRef: S.NullOr(S.String),
  responseSchemaRef: S.String,
  stability: BlueprintContractStability,
});
export type BlueprintOpenApiContract = typeof BlueprintOpenApiContract.Type;

export const BlueprintEventCatalogEntry = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  eventRef: S.String,
  id: S.String,
  payloadSchemaRef: S.String,
  privacyPolicy: BlueprintContractPrivacyPolicy,
  receiptRefs: S.Array(S.String),
  stability: BlueprintContractStability,
  topicRef: S.String,
});
export type BlueprintEventCatalogEntry = typeof BlueprintEventCatalogEntry.Type;

export const BlueprintReceiptCatalogEntry = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  evidenceSchemaRef: S.String,
  id: S.String,
  privacyPolicy: BlueprintContractPrivacyPolicy,
  receiptRef: S.String,
  retentionPolicyRef: S.String,
  stability: BlueprintContractStability,
});
export type BlueprintReceiptCatalogEntry = typeof BlueprintReceiptCatalogEntry.Type;

export const BlueprintContractExportSeed = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  eventCatalog: S.Array(BlueprintEventCatalogEntry),
  id: S.String,
  jsonSchemas: S.Array(BlueprintJsonSchemaContract),
  openApi: S.Array(BlueprintOpenApiContract),
  receiptCatalog: S.Array(BlueprintReceiptCatalogEntry),
  versionRef: S.String,
});
export type BlueprintContractExportSeed = typeof BlueprintContractExportSeed.Type;

export class BlueprintProjectionUnsafe extends S.TaggedErrorClass<BlueprintProjectionUnsafe>()(
  "BlueprintProjectionUnsafe",
  {
    path: S.String,
    reason: S.String,
  },
) {}

// ---------------------------------------------------------------------------
// IsPrivateDataSafe predicate family (security-critical single authority).
// ---------------------------------------------------------------------------

const PRIVATE_FIELD_PATTERN =
  /(^|[._-])(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|id_token|invoice|mnemonic|oauth|password|payment_hash|payment_id|payment_preimage|payout_address|payout_destination|payout_target|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_runner|raw_source_archive|raw_webhook|refresh_token|runner_log|secret|source_archive|token|wallet|xprv)([._-]|$)/i;

const PRIVATE_CAMEL_FIELD_PATTERN =
  /^(accessToken|authorization|bearer|callbackUrl|callbackToken|clientSecret|customerEmail|customerName|idToken|invoice|mnemonic|oauth|password|paymentHash|paymentId|paymentPreimage|payoutAddress|payoutDestination|payoutTarget|preimage|privateKey|privateRepo|providerGrant|providerPayload|providerToken|rawEmail|rawPayload|rawPrompt|rawRunLog|rawRunner|rawSourceArchive|rawWebhook|refreshToken|runnerLog|secret|sourceArchive|token|wallet|xprv)$/i;

const PRIVATE_VALUE_PATTERN =
  /\b(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|id_token|invoice|mnemonic|oauth|payment_hash|payment_id|payment_preimage|payout_address|payout_destination|payout_target|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_runner|raw_source_archive|raw_webhook|refresh_token|runner_log|source_archive|wallet|xprv)\b/i;

export type BlueprintJsonPrimitive = string | number | boolean | null;
export type BlueprintJsonValue =
  | BlueprintJsonPrimitive
  | ReadonlyArray<BlueprintJsonValue>
  | { readonly [key: string]: BlueprintJsonValue };

export function blueprintPrivateFieldKey(key: string): boolean {
  return PRIVATE_FIELD_PATTERN.test(key) || PRIVATE_CAMEL_FIELD_PATTERN.test(key);
}

export function isBlueprintProjectionPrivateDataSafe(value: unknown): boolean {
  if (value === null || value === undefined) {
    return true;
  }

  if (typeof value === "string") {
    return !PRIVATE_VALUE_PATTERN.test(value);
  }

  if (Array.isArray(value)) {
    return value.every(isBlueprintProjectionPrivateDataSafe);
  }

  if (typeof value !== "object") {
    return true;
  }

  for (const [key, child] of Object.entries(value)) {
    if (blueprintPrivateFieldKey(key) || !isBlueprintProjectionPrivateDataSafe(child)) {
      return false;
    }
  }

  return true;
}

export function blueprintContractExportSeedIsPrivateDataSafe(seed: BlueprintContractExportSeed): boolean {
  return isBlueprintProjectionPrivateDataSafe(seed);
}

export function validateBlueprintContractExportSeed(seed: BlueprintContractExportSeed) {
  return blueprintContractExportSeedIsPrivateDataSafe(seed)
    ? { ok: true as const, seed }
    : { ok: false as const, reason: "contract export contains private-data-shaped material" as const };
}

export function sanitizeBlueprintProjection<T extends BlueprintJsonValue>(value: T): T {
  return sanitizeBlueprintJsonValue(value) as T;
}

function sanitizeBlueprintJsonValue(value: BlueprintJsonValue): BlueprintJsonValue {
  if (typeof value === "string") {
    return PRIVATE_VALUE_PATTERN.test(value) ? "[redacted]" : value;
  }

  if (Array.isArray(value)) {
    return value.map((entry) => sanitizeBlueprintJsonValue(entry));
  }

  if (value === null || typeof value !== "object") {
    return value;
  }

  const sanitized: Record<string, BlueprintJsonValue> = {};

  for (const [key, child] of Object.entries(value)) {
    if (blueprintPrivateFieldKey(key)) {
      continue;
    }

    sanitized[key] = sanitizeBlueprintJsonValue(child);
  }

  return sanitized;
}
