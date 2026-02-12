import { createHash } from "node:crypto";

const canonicalize = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map(canonicalize);
  }
  if (value && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, nested]) => [key, canonicalize(nested)] as const);
    return Object.fromEntries(entries);
  }
  return value;
};

export const stableJsonStringify = (value: unknown): string => JSON.stringify(canonicalize(value));

export const sha256Hex = (input: string): string => createHash("sha256").update(input).digest("hex");

export const configHashFromText = (input: string): string => `cfg_${sha256Hex(input)}`;

export const snapshotHashFromValue = (value: unknown): string => configHashFromText(stableJsonStringify(value));
