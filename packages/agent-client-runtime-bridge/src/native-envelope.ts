import { createHash } from "node:crypto";

export type AcpPeerProfile = "grok" | "cursor" | "standard" | (string & {});

export type AcpRuntimeNativeEnvelope = Readonly<{
  schema: "openagents.acp_native_event.v1";
  peer: Readonly<{ profile: AcpPeerProfile; protocolVersion: number; connectionRef: string }>;
  processGeneration: number;
  method: string;
  requestId?: string | number;
  updateId: string;
  sessionId?: string;
  observedAt: string;
  nativeTimestamp?: string;
  discriminant: string;
  extensionNamespace?: string;
  validatedPayload: unknown;
  validationStatus: "validated" | "decode-failure";
  nativeSha256: string;
  byteLength: number;
  retention: "private-native";
}>;

export type NativeEnvelopeFailure = Readonly<{
  kind: "native-envelope-rejected";
  reason: "invalid-input" | "payload-too-large" | "unserializable";
  safeDetail: string;
}>;

export const DEFAULT_MAX_NATIVE_BYTES = 1_048_576;

const deepFreeze = (value: unknown, seen = new WeakSet<object>()): unknown => {
  if (value === null || typeof value !== "object" || seen.has(value)) return value;
  seen.add(value);
  for (const child of Object.values(value)) deepFreeze(child, seen);
  return Object.freeze(value);
};

const namespaceOf = (discriminant: string): string | undefined => {
  const slash = discriminant.indexOf("/");
  return slash > 0 ? discriminant.slice(0, slash) : undefined;
};

export const createAcpRuntimeNativeEnvelope = (
  input: Readonly<{
    profile: AcpPeerProfile;
    protocolVersion: number;
    connectionRef: string;
    processGeneration: number;
    method: string;
    requestId?: string | number;
    updateId: string;
    sessionId?: string;
    observedAt: string;
    nativeTimestamp?: string;
    discriminant: string;
    validatedPayload: unknown;
    validationStatus?: "validated" | "decode-failure";
    maxBytes?: number;
  }>,
): AcpRuntimeNativeEnvelope | NativeEnvelopeFailure => {
  if (
    !input.profile ||
    !input.connectionRef ||
    !input.method ||
    !input.updateId ||
    !input.discriminant ||
    !Number.isSafeInteger(input.processGeneration) ||
    input.processGeneration < 0 ||
    input.protocolVersion !== 1 ||
    !Number.isFinite(Date.parse(input.observedAt))
  )
    return {
      kind: "native-envelope-rejected",
      reason: "invalid-input",
      safeDetail: "invalid ACP native envelope metadata",
    };
  let encoded: string;
  try {
    encoded = JSON.stringify(input.validatedPayload);
  } catch {
    return {
      kind: "native-envelope-rejected",
      reason: "unserializable",
      safeDetail: "native payload is not JSON serializable",
    };
  }
  if (encoded === undefined)
    return {
      kind: "native-envelope-rejected",
      reason: "unserializable",
      safeDetail: "native payload is not JSON serializable",
    };
  const byteLength = Buffer.byteLength(encoded);
  if (byteLength > (input.maxBytes ?? DEFAULT_MAX_NATIVE_BYTES)) {
    return {
      kind: "native-envelope-rejected",
      reason: "payload-too-large",
      safeDetail: `native payload exceeds ${input.maxBytes ?? DEFAULT_MAX_NATIVE_BYTES} bytes`,
    };
  }
  const nativeSha256 = createHash("sha256").update(encoded).digest("hex");
  const validatedPayload: unknown = deepFreeze(JSON.parse(encoded));
  const extensionNamespace = namespaceOf(input.discriminant);
  const envelope: AcpRuntimeNativeEnvelope = {
    schema: "openagents.acp_native_event.v1",
    peer: Object.freeze({
      profile: input.profile,
      protocolVersion: input.protocolVersion,
      connectionRef: input.connectionRef,
    }),
    processGeneration: input.processGeneration,
    method: input.method,
    ...(input.requestId === undefined ? {} : { requestId: input.requestId }),
    updateId: input.updateId,
    ...(input.sessionId === undefined ? {} : { sessionId: input.sessionId }),
    observedAt: input.observedAt,
    ...(input.nativeTimestamp === undefined ? {} : { nativeTimestamp: input.nativeTimestamp }),
    discriminant: input.discriminant,
    ...(extensionNamespace === undefined ? {} : { extensionNamespace }),
    validatedPayload,
    validationStatus: input.validationStatus ?? "validated",
    nativeSha256,
    byteLength,
    retention: "private-native",
  };
  return Object.freeze(envelope);
};

export interface AcpNativeEvidenceStore {
  put(envelope: AcpRuntimeNativeEnvelope): Promise<{ readonly rawEventRef: string }>;
}

export type BoundedAcpNativeEvidenceStore = AcpNativeEvidenceStore &
  Readonly<{
    get(rawEventRef: string): AcpRuntimeNativeEnvelope | undefined;
    size(): number;
    byteLength(): number;
  }>;

export const createBoundedAcpNativeEvidenceStore = (
  limits: Readonly<{ maxEntries: number; maxBytes: number }>,
): BoundedAcpNativeEvidenceStore => {
  const entries = new Map<string, AcpRuntimeNativeEnvelope>();
  let bytes = 0;
  return {
    async put(envelope) {
      const identity = `${envelope.peer.profile}:${envelope.peer.connectionRef}:${envelope.processGeneration}:${envelope.sessionId ?? "none"}:${envelope.updateId}`;
      const rawEventRef = `native.acp.${createHash("sha256").update(identity).digest("hex").slice(0, 32)}`;
      const existing = entries.get(rawEventRef);
      if (existing !== undefined) {
        if (existing.nativeSha256 !== envelope.nativeSha256)
          throw new Error("conflicting ACP native evidence identity");
        return { rawEventRef };
      }
      if (entries.size >= limits.maxEntries || bytes + envelope.byteLength > limits.maxBytes)
        throw new Error("ACP native evidence store overloaded");
      entries.set(rawEventRef, envelope);
      bytes += envelope.byteLength;
      return { rawEventRef };
    },
    get: (rawEventRef) => entries.get(rawEventRef),
    size: () => entries.size,
    byteLength: () => bytes,
  };
};

export const redactAcpEvidence = (value: unknown): unknown => {
  const secret =
    /(authorization|api[-_]?key|token|password|secret|cookie|content|output|prompt|env)/i;
  const secretValue =
    /(?:bearer\s+[A-Za-z0-9._-]+|(?:sk|xai|ghp|github_pat)[-_][A-Za-z0-9_-]{8,})/i;
  const visit = (entry: unknown, depth: number): unknown => {
    if (depth > 6) return "[truncated]";
    if (Array.isArray(entry)) return entry.slice(0, 64).map((item) => visit(item, depth + 1));
    if (entry && typeof entry === "object") {
      return Object.fromEntries(
        Object.entries(entry as Record<string, unknown>)
          .slice(0, 128)
          .map(([key, child]) => [key, secret.test(key) ? "[redacted]" : visit(child, depth + 1)]),
      );
    }
    if (typeof entry === "string" && secretValue.test(entry)) return "[redacted]";
    return typeof entry === "string" && entry.length > 2_000 ? `${entry.slice(0, 2_000)}…` : entry;
  };
  return visit(value, 0);
};
