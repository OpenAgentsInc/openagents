type JsonValue =
  | null
  | boolean
  | number
  | string
  | ReadonlyArray<JsonValue>
  | Readonly<{ [key: string]: JsonValue }>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeNumber(n: number): number | null {
  // Match JSON.stringify behavior for non-finite numbers.
  return Number.isFinite(n) ? n : null;
}

function toJsonValue(
  input: unknown,
  ctx: { readonly seen: WeakSet<object>; readonly inArray: boolean }
): JsonValue {
  if (input === null) return null;

  switch (typeof input) {
    case "string":
    case "boolean":
      return input;
    case "number":
      return normalizeNumber(input);
    case "undefined":
      // JSON.stringify drops undefined in objects; in arrays it becomes null.
      return ctx.inArray ? null : null;
    case "bigint":
    case "function":
    case "symbol":
      throw new Error(
        `Value is not JSON-serializable (type: ${typeof input})`
      );
  }

  // object
  if (input instanceof Date) return input.toISOString();

  if (Array.isArray(input)) {
    return input.map((v) => toJsonValue(v, { ...ctx, inArray: true }));
  }

  if (!isRecord(input)) {
    // Map/Set/TypedArray/etc: force explicit conversion by caller.
    throw new Error(
      `Value is not JSON-serializable (unsupported object: ${Object.prototype.toString.call(
        input
      )})`
    );
  }

  if (ctx.seen.has(input)) {
    throw new Error("Value is not JSON-serializable (cycle detected)");
  }
  ctx.seen.add(input);

  const keys = Object.keys(input).sort();
  const out: Record<string, JsonValue> = {};
  for (const k of keys) {
    const v = input[k];
    if (v === undefined) continue; // match JSON.stringify on objects
    out[k] = toJsonValue(v, { ...ctx, inArray: false });
  }

  ctx.seen.delete(input);
  return out;
}

/**
 * Deterministic JSON string suitable for hashing.
 *
 * Rules:
 * - object keys are sorted lexicographically
 * - undefined keys are omitted (object) / become null (array)
 * - NaN/Infinity become null
 * - Date becomes ISO string
 * - cycles and non-JSON objects throw
 */
export function canonicalJson(value: unknown): string {
  if (value === undefined) {
    throw new Error("Value is not JSON-serializable (top-level undefined)");
  }
  const normalized = toJsonValue(value, { seen: new WeakSet<object>(), inArray: false });
  return JSON.stringify(normalized);
}
