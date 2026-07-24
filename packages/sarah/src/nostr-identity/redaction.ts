import { FORBIDDEN_SARAH_NOSTR_SECRET_FIELDS } from "./types.ts";

export class SarahNostrSecretLeakError extends Error {
  readonly fieldPath: string;
  constructor(fieldPath: string) {
    super(`sarah_nostr_identity: forbidden secret field at ${fieldPath}`);
    this.name = "SarahNostrSecretLeakError";
    this.fieldPath = fieldPath;
  }
}

const isForbiddenName = (name: string): boolean => {
  const lower = name.toLowerCase();
  return FORBIDDEN_SARAH_NOSTR_SECRET_FIELDS.some(
    (f) => f.toLowerCase() === lower || lower.includes(f.toLowerCase()),
  );
};

/**
 * Fail closed if a public-safe payload names secret-shaped fields.
 * Walks plain objects and arrays; does not follow prototypes.
 */
export const assertSarahNostrPublicSafe = (
  value: unknown,
  path = "$",
): void => {
  if (value === null || value === undefined) return;
  if (typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((item, i) => assertSarahNostrPublicSafe(item, `${path}[${i}]`));
    return;
  }
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    if (isForbiddenName(key)) {
      throw new SarahNostrSecretLeakError(`${path}.${key}`);
    }
    // Hex-looking 64-char values under secret-ish names already caught.
    // Also reject nsec-looking strings anywhere in string leaves of known bad shapes.
    if (typeof child === "string" && child.startsWith("nsec1")) {
      throw new SarahNostrSecretLeakError(`${path}.${key}(nsec)`);
    }
    assertSarahNostrPublicSafe(child, `${path}.${key}`);
  }
};

/** JSON-stable public projection helper that redacts by rejection. */
export const toPublicSafeJson = (value: unknown): string => {
  assertSarahNostrPublicSafe(value);
  return JSON.stringify(value);
};
