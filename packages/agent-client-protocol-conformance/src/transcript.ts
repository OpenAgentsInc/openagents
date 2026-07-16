import { createHash } from "node:crypto";

const sensitiveKey =
  /authorization|cookie|api[-_]?key|token|secret|password|credential|prompt|content|cwd|path|env/i;
const sensitiveValue = /\b(?:xai|sk|bearer)[-_][A-Za-z0-9._-]{8,}\b/gi;

export const sanitizeDurableValue = (value: unknown, key = ""): unknown => {
  if (sensitiveKey.test(key)) return "[REDACTED]";
  if (typeof value === "string") return value.replace(sensitiveValue, "[REDACTED]");
  if (Array.isArray(value)) return value.map((item) => sanitizeDurableValue(item));
  if (value !== null && typeof value === "object") {
    const record = value as Record<string, unknown>;
    const namedSecret = typeof record.name === "string" && sensitiveKey.test(record.name);
    return Object.fromEntries(
      Object.entries(record).map(([childKey, child]) => [
        childKey,
        namedSecret && childKey === "value" ? "[REDACTED]" : sanitizeDurableValue(child, childKey),
      ]),
    );
  }
  return value;
};

export type AcpTranscriptEntry = Readonly<{
  sequence: number;
  atMs: number;
  generation: number;
  direction: "inbound" | "outbound" | "stderr" | "lifecycle";
  byteLength: number;
  sha256: string;
  method?: string;
  requestId?: string | number | null;
  sessionId?: string;
  native: unknown;
}>;

export const sanitizeTranscript = (
  rows: ReadonlyArray<
    Readonly<{
      generation: number;
      direction: AcpTranscriptEntry["direction"];
      atMs?: number;
      native: unknown;
    }>
  >,
): ReadonlyArray<AcpTranscriptEntry> =>
  rows.map((row, index) => {
    const native = sanitizeDurableValue(row.native);
    const encoded = JSON.stringify(native);
    const record =
      native !== null && typeof native === "object" ? (native as Record<string, unknown>) : {};
    const params =
      record.params !== null && typeof record.params === "object"
        ? (record.params as Record<string, unknown>)
        : {};
    return {
      sequence: index + 1,
      atMs: row.atMs ?? index,
      generation: row.generation,
      direction: row.direction,
      byteLength: Buffer.byteLength(encoded),
      sha256: createHash("sha256").update(encoded).digest("hex"),
      ...(typeof record.method === "string" ? { method: record.method } : {}),
      ...("id" in record ? { requestId: record.id as string | number | null } : {}),
      ...(typeof params.sessionId === "string" ? { sessionId: params.sessionId } : {}),
      native,
    };
  });

export const assertSecretAbsent = (value: unknown, secrets: ReadonlyArray<string>): void => {
  const durable = JSON.stringify(value);
  for (const secret of secrets)
    if (durable.includes(secret))
      throw new Error("secret escaped into durable conformance evidence");
};
