/**
 * Shared public-safe and authority-fence guards for the Sarah LBR lane.
 */

import {
  SARAH_LBR_FORBIDDEN_GRANT_PREFIXES,
  SARAH_LBR_PUBLIC_REF_PATTERN,
  SarahLbrLaneError,
} from "./types.ts";

const unsafeMaterialPattern =
  /(ANTHROPIC_API_KEY|OPENAI_API_KEY|MDK_ACCESS_TOKEN|SECRET|TOKEN=|-----BEGIN|mnemonic|payment_hash|payment_preimage|preimage|lnbc|lntb|lno1|file:\/\/|\/Users\/|\/home\/|C:\\|ssh:\/\/|private[_-]?repo|raw prompt|provider payload)/iu;

const FORBIDDEN_SECRET_KEYS = [
  "mnemonic",
  "nsec",
  "privateKey",
  "privateKeyHex",
  "privateKeyBytes",
  "seckey",
  "secretKey",
  "secretKeyHex",
  "seed",
  "seedHex",
  "rawKey",
  "bolt11",
  "payment_hash",
  "payment_preimage",
  "preimage",
  "invoice",
] as const;

export const failLane = (code: string, message: string): never => {
  throw new SarahLbrLaneError(code, message);
};

export const ensureNoUnsafeMaterial = (value: string, field: string): void => {
  if (unsafeMaterialPattern.test(value)) {
    failLane("unsafe_material", `${field} contains private or payment material`);
  }
};

export const ensurePublicRef = (value: string, field: string): string => {
  ensureNoUnsafeMaterial(value, field);
  if (!SARAH_LBR_PUBLIC_REF_PATTERN.test(value)) {
    failLane("invalid_ref", `${field} must be a public-safe ref`);
  }
  return value;
};

export const ensurePublicRefs = (
  values: ReadonlyArray<string>,
  field: string,
): ReadonlyArray<string> => {
  if (values.length === 0) {
    failLane("missing_ref", `${field} requires at least one ref`);
  }
  return values.map((value, index) =>
    ensurePublicRef(value, `${field}[${index}]`),
  );
};

/**
 * Community work units must never carry Sarah's principal grants.
 * A unit whose grant looks like Sarah's authority is refused.
 */
export const assertNotSarahGrant = (ref: string, field: string): void => {
  ensurePublicRef(ref, field);
  const lower = ref.toLowerCase();
  for (const prefix of SARAH_LBR_FORBIDDEN_GRANT_PREFIXES) {
    if (lower === prefix || lower.startsWith(prefix.toLowerCase())) {
      failLane(
        "sarah_grant_forbidden",
        `${field} must not carry a Sarah principal grant (${ref})`,
      );
    }
  }
};

export const assertWorkUnitGrantFence = (input: {
  readonly grantRef: string;
  readonly allowedActionRefs: ReadonlyArray<string>;
  readonly workUnitRef: string;
  readonly idempotencyRef: string;
}): void => {
  assertNotSarahGrant(input.grantRef, "grantRef");
  assertNotSarahGrant(input.workUnitRef, "workUnitRef");
  assertNotSarahGrant(input.idempotencyRef, "idempotencyRef");
  for (const [index, action] of input.allowedActionRefs.entries()) {
    assertNotSarahGrant(action, `allowedActionRefs[${index}]`);
  }
};

/** Walk plain objects/arrays and reject secret-shaped keys and payment leaves. */
export const assertLanePublicSafe = (value: unknown, path = "$"): void => {
  if (value === null || value === undefined) return;
  if (typeof value === "string") {
    ensureNoUnsafeMaterial(value, path);
    if (value.startsWith("nsec1")) {
      failLane("unsafe_material", `${path} contains nsec material`);
    }
    return;
  }
  if (typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((item, i) => assertLanePublicSafe(item, `${path}[${i}]`));
    return;
  }
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    const lower = key.toLowerCase();
    if (
      FORBIDDEN_SECRET_KEYS.some(
        (f) => lower === f.toLowerCase() || lower.includes(f.toLowerCase()),
      )
    ) {
      failLane("unsafe_material", `forbidden field ${path}.${key}`);
    }
    assertLanePublicSafe(child, `${path}.${key}`);
  }
};

export const nowUnixSeconds = (): number => Math.floor(Date.now() / 1000);

export const tagValues = (
  tags: ReadonlyArray<readonly string[]>,
  name: string,
): ReadonlyArray<string> =>
  tags.flatMap((tag) =>
    tag[0] === name && tag[1] !== undefined ? [tag[1]] : [],
  );

export const paramValues = (
  tags: ReadonlyArray<readonly string[]>,
  key: string,
): ReadonlyArray<string> =>
  tags.flatMap((tag) =>
    tag[0] === "param" && tag[1] === key && tag[2] !== undefined
      ? [tag[2]]
      : [],
  );

export const requiredParam = (
  tags: ReadonlyArray<readonly string[]>,
  key: string,
): string =>
  paramValues(tags, key)[0] ??
  failLane("missing_param", `missing LBR param ${key}`);
