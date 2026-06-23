import { Effect, Schema as S } from "effect";

// The branded refs are defined ONCE in ./index (the canonical brand authority).
// Re-using them here keeps a single nominal `ProviderSecretRef` brand across the
// web contract and the Probe/Pylon runtime contract.
import { ProviderAccountRef, ProviderAuthGrantRef, ProviderSecretRef } from "./index.js";

export { ProviderAccountRef, ProviderAuthGrantRef, ProviderSecretRef };

export const CHATGPT_CODEX_PROVIDER = "chatgpt_codex" as const;
export const GOOGLE_GEMINI_PROVIDER = "google_gemini" as const;

export const ChatGptCodexProvider = S.Literal(CHATGPT_CODEX_PROVIDER);
export type ChatGptCodexProvider = typeof ChatGptCodexProvider.Type;

export const GoogleGeminiProvider = S.Literal(GOOGLE_GEMINI_PROVIDER);
export type GoogleGeminiProvider = typeof GoogleGeminiProvider.Type;

export const ProbeProvider = S.Union([ChatGptCodexProvider, GoogleGeminiProvider]);
export type ProbeProvider = typeof ProbeProvider.Type;

export const ProviderAccountStatus = S.Literals([
  "pending",
  "connected",
  "expired",
  "denied",
  "disconnected",
  "unhealthy",
]);
export type ProviderAccountStatus = typeof ProviderAccountStatus.Type;

export const ProviderAccountHealth = S.Literals(["unknown", "healthy", "unhealthy", "requires_reauth"]);
export type ProviderAccountHealth = typeof ProviderAccountHealth.Type;

export const ProviderAuthMode = S.Literals(["chatgpt_device_code", "codex_device_auth", "manual_secret_ref"]);
export type ProviderAuthMode = typeof ProviderAuthMode.Type;

export const JsonValue: S.Schema<JsonValue> = S.Union([
  S.String,
  S.Number,
  S.Boolean,
  S.Null,
  S.Array(S.suspend(() => JsonValue)),
  S.Record(S.String, S.suspend(() => JsonValue)),
]);
export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | ReadonlyArray<JsonValue> | { readonly [key: string]: JsonValue };

export const PublicProviderAccount = S.Struct({
  provider: ProbeProvider,
  providerAccountRef: ProviderAccountRef,
  authMode: ProviderAuthMode,
  status: ProviderAccountStatus,
  health: ProviderAccountHealth,
  secretRef: S.optional(ProviderSecretRef),
  accountLabel: S.optional(S.String),
  planType: S.optional(S.String),
  operatorLabel: S.optional(S.String),
  operatorPriority: S.optional(S.Number),
  leaseLimit: S.optional(S.Number),
  lowCredit: S.optional(S.Boolean),
  cooldownUntil: S.optional(S.String),
  recentFailureClass: S.optional(S.String),
  reauthRequiredReason: S.optional(S.String),
  metadata: S.optional(S.Record(S.String, JsonValue)),
});
export type PublicProviderAccount = typeof PublicProviderAccount.Type;

export const ProbeAuthGrantRequest = S.Struct({
  provider: ProbeProvider,
  providerAccountRef: ProviderAccountRef,
  runnerSessionId: S.String,
  requestedAction: S.optional(S.String),
  threadId: S.optional(S.String),
  workroomId: S.optional(S.String),
});
export type ProbeAuthGrantRequest = typeof ProbeAuthGrantRequest.Type;

export class ProbePublicProjectionUnsafe extends S.TaggedErrorClass<ProbePublicProjectionUnsafe>()(
  "ProbePublicProjectionUnsafe",
  {
    path: S.String,
    reason: S.String,
  },
) {}

const PUBLIC_SECRET_REF_PREFIXES = [
  "secret://",
  "vault://",
  "gcp-secret://",
  "cloud-secret://",
  "provider-account://",
  "codex-auth://",
];

const SECRET_VALUE_PATTERNS: RegExp[] = [
  /\bsk-[A-Za-z0-9_-]{20,}\b/,
  /\bAIza[A-Za-z0-9_-]{20,}\b/,
  /\bBearer\s+[A-Za-z0-9._-]{20,}\b/i,
  /\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b/,
  /"?(refresh|access|id)_token"?\s*[:=]/i,
  /"?(client_secret|authorization_code|code_verifier)"?\s*[:=]/i,
  /\bOPENCODE_AUTH_CONTENT\b/,
  /\bauth\.json\b/i,
];

const SECRET_KEY_PATTERN =
  /(^|_)(token|refresh|access|authorization|oauth|password|credential|private_key|secret)(_|$)/i;

export function isPublicSecretRef(value: string): value is ProviderSecretRef {
  return PUBLIC_SECRET_REF_PREFIXES.some((prefix) => value.startsWith(prefix));
}

export function assertPublicSecretRef(value: string, fieldName = "secretRef"): asserts value is ProviderSecretRef {
  const result = Effect.runSync(requirePublicSecretRef(value, fieldName));

  if (result !== value) {
    throw new Error(`${fieldName} must be a public secret reference, not raw credential material`);
  }
}

export function requirePublicSecretRef(
  value: string,
  fieldName = "secretRef",
): Effect.Effect<ProviderSecretRef, ProbePublicProjectionUnsafe> {
  return isPublicSecretRef(value)
    ? Effect.succeed(value)
    : Effect.fail(
        new ProbePublicProjectionUnsafe({
          path: fieldName,
          reason: "must be a public secret reference, not raw credential material",
        }),
      );
}

export function containsSecretMaterial(value: unknown): boolean {
  if (typeof value !== "string") {
    return false;
  }

  if (isPublicSecretRef(value)) {
    return false;
  }

  return SECRET_VALUE_PATTERNS.some((pattern) => pattern.test(value));
}

export function canIssueProviderAccountGrant(account: PublicProviderAccount): boolean {
  return (
    (account.provider === CHATGPT_CODEX_PROVIDER || account.provider === GOOGLE_GEMINI_PROVIDER) &&
    account.status === "connected" &&
    account.health === "healthy" &&
    typeof account.secretRef === "string" &&
    isPublicSecretRef(account.secretRef)
  );
}

export function canSelectProviderAccountForLease(account: PublicProviderAccount, now: Date = new Date()): boolean {
  if (!canIssueProviderAccountGrant(account)) {
    return false;
  }

  if (account.lowCredit === true) {
    return false;
  }

  if (account.cooldownUntil !== undefined) {
    const cooldownUntilMs = Date.parse(account.cooldownUntil);

    if (Number.isNaN(cooldownUntilMs) || cooldownUntilMs > now.getTime()) {
      return false;
    }
  }

  if (account.leaseLimit !== undefined && account.leaseLimit <= 0) {
    return false;
  }

  return true;
}

export function assertProbePublicProjection(value: unknown, path = "projection"): void {
  Effect.runSync(validateProbePublicProjection(value, path));
}

export function validateProbePublicProjection(
  value: unknown,
  path = "projection",
): Effect.Effect<void, ProbePublicProjectionUnsafe> {
  if (value === null || value === undefined) {
    return Effect.void;
  }

  if (containsSecretMaterial(value)) {
    return Effect.fail(new ProbePublicProjectionUnsafe({ path, reason: "contains raw credential material" }));
  }

  if (Array.isArray(value)) {
    return Effect.all(
      value.map((entry, index) => validateProbePublicProjection(entry, `${path}[${index}]`)),
    ).pipe(Effect.asVoid);
  }

  if (typeof value !== "object") {
    return Effect.void;
  }

  return Effect.gen(function* () {
    for (const [key, entry] of Object.entries(value)) {
      const childPath = `${path}.${key}`;

      if (key === "secretRef" || key === "providerSecretRef") {
        if (typeof entry !== "string") {
          return yield* Effect.fail(
            new ProbePublicProjectionUnsafe({
              path: childPath,
              reason: "must be a public secret reference string",
            }),
          );
        }

        yield* requirePublicSecretRef(entry, childPath);
        continue;
      }

      if (SECRET_KEY_PATTERN.test(key)) {
        return yield* Effect.fail(
          new ProbePublicProjectionUnsafe({
            path: childPath,
            reason: "is not allowed in a public Probe projection",
          }),
        );
      }

      yield* validateProbePublicProjection(entry, childPath);
    }
  });
}

export function sanitizeProbePublicProjection<T extends JsonValue>(value: T): T {
  return sanitizeJsonValue(value) as T;
}

function sanitizeJsonValue(value: JsonValue): JsonValue {
  if (typeof value === "string") {
    return containsSecretMaterial(value) ? "[redacted]" : value;
  }

  if (Array.isArray(value)) {
    return value.map((entry) => sanitizeJsonValue(entry));
  }

  if (value === null || typeof value !== "object") {
    return value;
  }

  const sanitized: Record<string, JsonValue> = {};

  for (const [key, entry] of Object.entries(value)) {
    if (key === "secretRef" || key === "providerSecretRef") {
      sanitized[key] = typeof entry === "string" && isPublicSecretRef(entry) ? entry : "[redacted]";
      continue;
    }

    sanitized[key] = SECRET_KEY_PATTERN.test(key) ? "[redacted]" : sanitizeJsonValue(entry);
  }

  return sanitized;
}
