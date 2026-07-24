/**
 * Owner appeal-identity registry for SARAH-CW-05.
 *
 * Spec §40.1 requirements:
 * 1. Single admitted location — clients read from here, never embed a copy.
 * 2. A ruling is a signed event from that key. Sarah cannot author one.
 * 3. Registration is auditable (revision + registeredByRef + supersedesRef).
 * 4. Rotation has a path (new revision supersedes prior).
 *
 * The owner must supply the public key. Until it exists, resolve returns
 * lifecycle "missing" with needsOwner: true (NEEDS_OWNER blocker).
 */
import { Schema as S } from "effect";

import {
  OwnerAppealIdentityMissingSchema,
  OwnerAppealIdentitySchema,
  type OwnerAppealIdentity,
  type OwnerAppealIdentityMissing,
  type OwnerAppealIdentityResolution,
} from "./types.ts";

/**
 * Admitted registry path relative to the repository root.
 * This is the single client-readable location for the public key.
 * Until the owner supplies an npub/pubkey, the file is absent and resolve
 * returns the missing resolution.
 */
export const OWNER_APPEAL_IDENTITY_REGISTRY_PATH =
  "docs/omega/owner-appeal-identity.json" as const;

/** Env overrides for local/test registration without committing a key. */
export const OWNER_APPEAL_PUBKEY_ENV = "OPENAGENTS_OWNER_APPEAL_PUBKEY" as const;
export const OWNER_APPEAL_NPUB_ENV = "OPENAGENTS_OWNER_APPEAL_NPUB" as const;

export const OWNER_APPEAL_IDENTITY_BLOCKER_REF =
  "needs_owner.owner_appeal_npub" as const;

const decodeIdentity = S.decodeUnknownSync(OwnerAppealIdentitySchema);
const decodeMissing = S.decodeUnknownSync(OwnerAppealIdentityMissingSchema);

export const missingOwnerAppealIdentity = (): OwnerAppealIdentityMissing =>
  decodeMissing({
    schema: "openagents.sarah.owner_appeal_identity.v1",
    packet: "SARAH-CW-05",
    lifecycle: "missing",
    needsOwner: true,
    blockerRef: OWNER_APPEAL_IDENTITY_BLOCKER_REF,
    summary:
      "Owner Nostr public key is not registered as the community appeal identity.",
  });

const HEX64 = /^[0-9a-f]{64}$/;
const NPUB = /^npub1[a-z0-9]{58}$/;

/**
 * Resolve the owner appeal identity from an optional admitted registration
 * payload and optional env public fields. Never accepts or returns secrets.
 */
export const resolveOwnerAppealIdentity = (input?: {
  readonly registration?: unknown;
  readonly envPubkey?: string | undefined;
  readonly envNpub?: string | undefined;
}): OwnerAppealIdentityResolution => {
  if (input?.registration !== undefined && input.registration !== null) {
    try {
      return decodeIdentity(input.registration, { onExcessProperty: "error" });
    } catch {
      return missingOwnerAppealIdentity();
    }
  }

  const envPubkey = input?.envPubkey?.trim().toLowerCase();
  if (envPubkey !== undefined && envPubkey !== "" && HEX64.test(envPubkey)) {
    const envNpub = input?.envNpub?.trim();
    const now = new Date().toISOString();
    return decodeIdentity(
      {
        schema: "openagents.sarah.owner_appeal_identity.v1",
        packet: "SARAH-CW-05",
        lifecycle: "admitted",
        pubkey: envPubkey,
        ...(envNpub !== undefined && NPUB.test(envNpub)
          ? { npub: envNpub }
          : {}),
        revision: 1,
        registeredAt: now,
        registeredByRef: "operator.env.OPENAGENTS_OWNER_APPEAL_PUBKEY",
        registrationRef: "registration.env.owner_appeal.v1",
      },
      { onExcessProperty: "error" },
    );
  }

  return missingOwnerAppealIdentity();
};

/**
 * Read process env for public appeal identity fields.
 * Does not touch Keychain, nsec, or any secret store.
 */
export const resolveOwnerAppealIdentityFromEnv = (
  env: NodeJS.ProcessEnv = process.env,
): OwnerAppealIdentityResolution =>
  resolveOwnerAppealIdentity({
    envPubkey: env[OWNER_APPEAL_PUBKEY_ENV],
    envNpub: env[OWNER_APPEAL_NPUB_ENV],
  });

/** True when an appeal can be verified against a registered owner key. */
export const isOwnerAppealIdentityReady = (
  resolution: OwnerAppealIdentityResolution,
): resolution is OwnerAppealIdentity =>
  resolution.lifecycle === "admitted" || resolution.lifecycle === "rotating";

/**
 * Build the next rotation registration. Callers persist it to the admitted
 * registry path; this helper only shapes the public record.
 */
export const buildOwnerAppealIdentityRotation = (input: {
  readonly current: OwnerAppealIdentity;
  readonly nextPubkey: string;
  readonly nextNpub?: string;
  readonly registeredByRef: string;
  readonly registrationRef: string;
  readonly registeredAt?: string;
}): OwnerAppealIdentity => {
  const nextPubkey = input.nextPubkey.trim().toLowerCase();
  if (!HEX64.test(nextPubkey)) {
    throw new Error("owner_appeal_identity: nextPubkey must be 64 hex chars");
  }
  if (nextPubkey === input.current.pubkey) {
    throw new Error("owner_appeal_identity: rotation requires a new pubkey");
  }
  return decodeIdentity(
    {
      schema: "openagents.sarah.owner_appeal_identity.v1",
      packet: "SARAH-CW-05",
      lifecycle: "admitted",
      pubkey: nextPubkey,
      ...(input.nextNpub !== undefined && NPUB.test(input.nextNpub)
        ? { npub: input.nextNpub }
        : {}),
      revision: input.current.revision + 1,
      registeredAt: input.registeredAt ?? new Date().toISOString(),
      registeredByRef: input.registeredByRef,
      supersedesRef: input.current.registrationRef,
      registrationRef: input.registrationRef,
    },
    { onExcessProperty: "error" },
  );
};
