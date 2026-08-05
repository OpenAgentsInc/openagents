/**
 * Canonical JSON serialisation and content digests for persisted records.
 *
 * Every stored envelope and every export carries a SHA-256 digest over this
 * canonical form (sorted object members, no insignificant whitespace,
 * RFC 8785-compatible for the JSON subset we persist). The digest is what
 * makes a torn write detectable: a partially written string cannot verify,
 * so it can never be loaded as a complete record.
 */
import { sha256 } from "@noble/hashes/sha2";
import { bytesToHex } from "@noble/hashes/utils";

/** JSON-serialisable values. Persisted payloads must satisfy this shape. */
export type Json = string | number | boolean | null | ReadonlyArray<Json> | { readonly [key: string]: Json };

class NotCanonicalisable extends Error {}

const canonicalise = (value: unknown, path: string): string => {
  if (value === null) return "null";
  switch (typeof value) {
    case "string":
      return JSON.stringify(value);
    case "boolean":
      return value ? "true" : "false";
    case "number":
      if (!Number.isFinite(value)) {
        throw new NotCanonicalisable(`${path}: non-finite number`);
      }
      return JSON.stringify(value);
    case "object":
      break;
    default:
      throw new NotCanonicalisable(`${path}: ${typeof value} is not JSON`);
  }
  if (Array.isArray(value)) {
    return `[${value.map((item, index) => canonicalise(item, `${path}[${index}]`)).join(",")}]`;
  }
  const members = Object.entries(value as Record<string, unknown>)
    .filter(([, member]) => member !== undefined)
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([key, member]) => `${JSON.stringify(key)}:${canonicalise(member, `${path}.${key}`)}`);
  return `{${members.join(",")}}`;
};

/**
 * Canonical JSON text. Throws on non-JSON values (undefined array members,
 * functions, non-finite numbers) — a persist path must never coerce silently.
 */
export const canonicalJson = (value: unknown): string => canonicalise(value, "$");

/** Lower-hex SHA-256 of the canonical JSON form. */
export const contentDigestHex = (value: unknown): string =>
  bytesToHex(sha256(new TextEncoder().encode(canonicalJson(value))));
