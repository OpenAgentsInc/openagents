/**
 * The custody tripwire and the SWAP-4 boundary.
 *
 * The MKT-SWP client custody boundary is explicit: snapshots contain signed
 * public records, exit templates or complete pre-signed transactions, public
 * commitments, and external effect results; recursive tripwires reject
 * seeds, private or claim/refund keys, preimages, macaroons, NWC connection
 * strings, and signing nonces. This module is the store-side enforcement of
 * that rule: every payload is scanned before persist, export, and import.
 *
 * This is NOT key management. The browser key-material design — generation,
 * the rescue ceremony, the secret store — is SWAP-4 (openagents#9319) and is
 * not built here. The injection point is `SwapSecretStoreProbe` plus the
 * `secretHandles` field on the session record: opaque, non-secret handles
 * that the SWAP-4 store will resolve. Nothing behind a handle ever passes
 * through this package.
 *
 * The tripwire matches bounded, well-known member names and value prefixes.
 * That is deterministic parsing of bounded fields on an already-selected
 * path, which the workspace semantic-routing rule permits; it is a tripwire
 * (defence in depth behind the engine's own recursive validator), not the
 * primary custody proof.
 */
import { Effect } from "effect";

import { SecretMaterialError } from "./errors.js";

/** Member names that must never appear in a persisted or exported payload. */
export const FORBIDDEN_SECRET_MEMBER_NAMES: ReadonlyArray<string> = [
  "mnemonic",
  "seed",
  "seedwords",
  "seedphrase",
  "privatekey",
  "privkey",
  "secretkey",
  "claimprivatekey",
  "refundprivatekey",
  "preimage",
  "paymentpreimage",
  "macaroon",
  "nwc",
  "nwcurl",
  "nwcuri",
  "connectionstring",
  "signingnonce",
  "secnonce",
  "musignonce",
  "xprv",
  "nsec",
];

/** String-value prefixes that denote serialized key material. */
const FORBIDDEN_VALUE_PREFIXES = ["xprv", "tprv", "nsec1"] as const;

const normaliseMemberName = (name: string): string =>
  name.toLowerCase().replaceAll(/[_\-\s]/gu, "");

const forbiddenNames = new Set(FORBIDDEN_SECRET_MEMBER_NAMES);

const scan = (value: unknown, path: string): string | null => {
  if (typeof value === "string") {
    for (const prefix of FORBIDDEN_VALUE_PREFIXES) {
      if (value.startsWith(prefix)) return path;
    }
    return null;
  }
  if (typeof value !== "object" || value === null) return null;
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index += 1) {
      const hit = scan(value[index], `${path}[${index}]`);
      if (hit !== null) return hit;
    }
    return null;
  }
  for (const [key, member] of Object.entries(value)) {
    const memberPath = `${path}.${key}`;
    if (forbiddenNames.has(normaliseMemberName(key))) return memberPath;
    const hit = scan(member, memberPath);
    if (hit !== null) return hit;
  }
  return null;
};

/**
 * Fail with `SecretMaterialError` (identifier `swp_secret_material_forbidden`)
 * if the payload appears to carry key material. The offending path is
 * reported; the offending VALUE is never included, logged, or echoed.
 */
export const assertNoSecretMaterial = (
  value: unknown,
  rootPath: string,
): Effect.Effect<void, SecretMaterialError> => {
  const hit = scan(value, rootPath);
  return hit === null
    ? Effect.void
    : Effect.fail(new SecretMaterialError({ path: hit, identifier: "swp_secret_material_forbidden" }));
};

/**
 * SWAP-4 (openagents#9319) injection point. The rescue ceremony and the
 * secret store are out of scope here; History and Resume only need to ask
 * "is the material behind this handle present on this device?" to
 * distinguish a signed-exit path from a keyless (pre-signed package) path.
 * Until SWAP-4 lands, hosts may pass `noSecretStoreProbe`.
 */
export interface SwapSecretStoreProbe {
  readonly hasSecret: (handle: string) => Effect.Effect<boolean>;
}

/** Probe for hosts without a SWAP-4 secret store: nothing is resolvable. */
export const noSecretStoreProbe: SwapSecretStoreProbe = {
  hasSecret: () => Effect.succeed(false),
};
