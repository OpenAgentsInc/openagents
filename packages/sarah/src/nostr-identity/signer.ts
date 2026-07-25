import { Schema as S } from "effect";

import {
  generateSecretKeyBytes,
  parseSecretMaterial,
  publicKeyFromSecret,
  signEventTemplate,
} from "./crypto.ts";
import { makeNip44OwnerCipher } from "./nip44-cipher.ts";
import { assertSarahNostrPublicSafe } from "./redaction.ts";
import type { SarahNostrCipher } from "../nostr-turn/types.ts";
import {
  SARAH_NOSTR_IDENTITY_SCHEMA,
  SARAH_NOSTR_IDENTITY_SECRET_ENV,
  SARAH_NOSTR_IDENTITY_SECRET_ID,
  SARAH_NOSTR_PRINCIPAL,
  SarahNostrPublicIdentity,
  type SarahNostrEventTemplate,
  type SarahNostrLifecycleState,
  type SarahNostrSignedEvent,
  type SarahNostrSigner,
} from "./types.ts";

const decodeIdentity = S.decodeUnknownSync(SarahNostrPublicIdentity);

/**
 * Build sealed signer + NIP-44 owner cipher from one secret load.
 * The secret is copied into closures; caller may zero the source buffer.
 */
export const createSealedSarahNostrStack = (params: {
  readonly secretKey: Uint8Array;
  readonly ownerPubkeyHex: string;
  readonly lifecycle?: SarahNostrLifecycleState;
}): {
  readonly signer: SarahNostrSigner;
  readonly cipher: SarahNostrCipher;
} => {
  const signer = createSealedSarahNostrSigner(params);
  const cipher = makeNip44OwnerCipher({
    sarahSecretKey: params.secretKey,
    ownerPubkeyHex: params.ownerPubkeyHex,
  });
  return { signer, cipher };
};

/**
 * Create a sealed Sarah signer that holds secret bytes only in a closure.
 * The returned port has no method that can recover those bytes.
 */
export const createSealedSarahNostrSigner = (params: {
  readonly secretKey: Uint8Array;
  readonly lifecycle?: SarahNostrLifecycleState;
}): SarahNostrSigner => {
  // Copy so the caller's buffer can be zeroed without affecting the signer.
  const sealed = new Uint8Array(params.secretKey);
  const pubkey = publicKeyFromSecret(sealed);
  const lifecycle = params.lifecycle ?? "active";

  const publicIdentity = decodeIdentity({
    schema: SARAH_NOSTR_IDENTITY_SCHEMA,
    principal: SARAH_NOSTR_PRINCIPAL,
    pubkey,
    lifecycle,
    custodySecretId: SARAH_NOSTR_IDENTITY_SECRET_ID,
  });
  assertSarahNostrPublicSafe(publicIdentity);

  return {
    getPublicKey: () => pubkey,
    getPublicIdentity: () => publicIdentity,
    signEvent: (template: SarahNostrEventTemplate): SarahNostrSignedEvent => {
      if (lifecycle === "revoked" || lifecycle === "archived") {
        throw new Error(`sarah_nostr_identity: cannot sign in lifecycle=${lifecycle}`);
      }
      const signed = signEventTemplate(sealed, template);
      assertSarahNostrPublicSafe(signed);
      return signed;
    },
  };
};

/** Generate a fresh sealed Sarah signer (tests and offline key mint). */
export const generateSarahNostrSigner = (
  lifecycle: SarahNostrLifecycleState = "active",
): SarahNostrSigner =>
  createSealedSarahNostrSigner({
    secretKey: generateSecretKeyBytes(),
    lifecycle,
  });

export type SarahNostrSecretSource =
  | { readonly kind: "env"; readonly envName?: string }
  | { readonly kind: "raw"; readonly material: string };

/**
 * Load Sarah's key from the admitted Cloud Run / Secret Manager mount path.
 *
 * After a successful bind, clears the env slot so the raw material is not
 * re-readable from process.env. The sealed signer retains the only copy.
 */
export const loadSarahNostrSignerFromSecretManagerMount = (
  source: SarahNostrSecretSource = { kind: "env" },
  lifecycle: SarahNostrLifecycleState = "active",
): SarahNostrSigner => {
  let material: string | undefined;
  if (source.kind === "raw") {
    material = source.material;
  } else {
    const envName = source.envName ?? SARAH_NOSTR_IDENTITY_SECRET_ENV;
    material = process.env[envName];
    if (material === undefined || material.trim() === "") {
      throw new Error(`sarah_nostr_identity: missing Secret Manager mount at env ${envName}`);
    }
    // Clear the mount slot after read — process-local sealed copy only.
    delete process.env[envName];
  }

  const secretKey = parseSecretMaterial(material);
  // Best-effort wipe of the string is impossible in JS; avoid retaining refs.
  material = undefined;
  return createSealedSarahNostrSigner({ secretKey, lifecycle });
};

/**
 * Load one Secret Manager mount into the sealed Sarah signing and owner-cipher
 * closures. The parsed key is erased before this function returns.
 */
export const loadSealedSarahNostrStackFromSecretManagerMount = (params: {
  readonly ownerPubkeyHex: string;
  readonly expectedSarahPubkeyHex?: string;
  readonly envName?: string;
  readonly lifecycle?: SarahNostrLifecycleState;
}): ReturnType<typeof createSealedSarahNostrStack> => {
  const envName = params.envName ?? SARAH_NOSTR_IDENTITY_SECRET_ENV;
  let material = process.env[envName];
  if (material === undefined) {
    throw new Error(`sarah_nostr_identity: missing Secret Manager mount at env ${envName}`);
  }
  delete process.env[envName];
  if (material.trim() === "") {
    material = undefined;
    throw new Error(`sarah_nostr_identity: missing Secret Manager mount at env ${envName}`);
  }

  let secretKey: Uint8Array | undefined;
  try {
    secretKey = parseSecretMaterial(material);
    material = undefined;
    if (
      params.expectedSarahPubkeyHex !== undefined &&
      publicKeyFromSecret(secretKey) !== params.expectedSarahPubkeyHex
    ) {
      throw new Error("sarah_nostr_identity: mounted identity does not match admitted Sarah");
    }
    const stack = createSealedSarahNostrStack({
      secretKey,
      ownerPubkeyHex: params.ownerPubkeyHex,
      ...(params.lifecycle !== undefined ? { lifecycle: params.lifecycle } : {}),
    });
    return stack;
  } finally {
    material = undefined;
    secretKey?.fill(0);
  }
};

/** Lifecycle transition table. */
const ALLOWED: Record<SarahNostrLifecycleState, ReadonlyArray<SarahNostrLifecycleState>> = {
  active: ["rotating", "revoked"],
  rotating: ["active", "archived"],
  revoked: ["archived"],
  archived: [],
};

export const assertLifecycleTransition = (
  from: SarahNostrLifecycleState,
  to: SarahNostrLifecycleState,
): void => {
  if (!ALLOWED[from].includes(to)) {
    throw new Error(`sarah_nostr_identity: illegal lifecycle ${from} → ${to}`);
  }
};
