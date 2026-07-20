import { sha256Hex } from "./sha256.js";

/**
 * Deterministic canonical JSON: object keys are sorted, arrays keep order.
 *
 * Two structurally equal values always serialize to the same bytes, so a digest
 * over the canonical form is a stable content address. This mirrors the frozen
 * `compileThreadExportArtifact` canonicalization in `agent-runtime-schema` so
 * the DSE artifact digest and the frozen export digest share one rule. The input
 * is `unknown` because callers pass Effect-encoded structs at a serialization
 * boundary; the function walks JSON-shaped values and leaves primitives intact.
 */
const compareStrings = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

const canonicalize = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => compareStrings(left, right))
      .map(([key, child]) => [key, canonicalize(child)]),
  );
};

/** Serialize a value to its deterministic canonical string form. */
export const canonicalStringify = (value: unknown): string => JSON.stringify(canonicalize(value));

/** The lowercase-hex SHA-256 digest of the canonical serialization of `value`. */
export const canonicalDigest = (value: unknown): string => sha256Hex(canonicalStringify(value));
