/**
 * NIP-OA / NIP-AA / NIP-AP attach helpers for community agents.
 *
 * Reuses the sealed-identity NIP-OA primitives from nostr-identity.
 * OpenAgents never holds the operator secret or the agent secret.
 */
import {
  buildAttestedAuthTemplate,
  signOwnerAuthTag,
  verifyOwnerAuthTag,
  type SarahNostrEventTemplate,
  type SarahOwnerAuthTag,
} from "../nostr-identity/index.ts";
import {
  CommunityMembershipError,
  FORBIDDEN_COMMUNITY_SECRET_FIELDS,
  type CommunityOwnerAuthTag,
  type CommunityPersonaRef,
} from "./types.ts";

const HEX64 = /^[0-9a-f]{64}$/;

export const assertHexPubkey = (pubkey: string, label: string): string => {
  const normalized = pubkey.trim().toLowerCase();
  if (!HEX64.test(normalized)) {
    throw new CommunityMembershipError(
      "anonymous_pubkey_refused",
      `community: ${label} must be a 64-hex pubkey`,
    );
  }
  return normalized;
};

/**
 * Fail closed if a public community payload names provider keys, agent homes,
 * or other secret-shaped fields. Same law as OMEGA-SW-02 applied to a stranger.
 */
export const assertCommunityPublicSafe = (
  value: unknown,
  path = "$",
): void => {
  if (value === null || value === undefined) return;
  if (typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((item, i) => assertCommunityPublicSafe(item, `${path}[${i}]`));
    return;
  }
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    const lower = key.toLowerCase();
    for (const forbidden of FORBIDDEN_COMMUNITY_SECRET_FIELDS) {
      if (lower === forbidden.toLowerCase() || lower.includes(forbidden.toLowerCase())) {
        const code =
          forbidden.toLowerCase().includes("provider") ||
          forbidden.includes("API_KEY")
            ? "provider_key_forbidden"
            : forbidden.toLowerCase().includes("home")
              ? "agent_home_mutation_forbidden"
              : "secret_shaped_payload";
        throw new CommunityMembershipError(
          code,
          `community: forbidden field at ${path}.${key}`,
        );
      }
    }
    if (typeof child === "string" && child.startsWith("nsec1")) {
      throw new CommunityMembershipError(
        "secret_shaped_payload",
        `community: nsec-shaped value at ${path}.${key}`,
      );
    }
    assertCommunityPublicSafe(child, `${path}.${key}`);
  }
};

/**
 * Operator (human) signs a NIP-OA auth tag for their agent key.
 * The operator secret stays on the operator machine. Callers that hold only
 * the public tag use `verifyAgentOwnerAttestation` instead.
 */
export const attachOwnerAttestation = (params: {
  readonly agentPubkey: string;
  readonly operatorSeckeyHex: string;
  readonly conditions?: string;
}): CommunityOwnerAuthTag => {
  const agentPubkey = assertHexPubkey(params.agentPubkey, "agentPubkey");
  const tag = signOwnerAuthTag({
    agentPubkey,
    conditions: params.conditions ?? "",
    ownerSeckeyHex: params.operatorSeckeyHex,
  }) as SarahOwnerAuthTag;

  if (!verifyOwnerAuthTag(tag, agentPubkey)) {
    throw new CommunityMembershipError(
      "agent_not_attested",
      "community: produced auth tag failed verification",
    );
  }
  if (tag[1] === agentPubkey) {
    throw new CommunityMembershipError(
      "agent_self_attestation",
      "community: self-attestation is forbidden",
    );
  }
  return tag as CommunityOwnerAuthTag;
};

/** Verify a NIP-OA tag binds agentPubkey to a distinct operator. */
export const verifyAgentOwnerAttestation = (params: {
  readonly agentPubkey: string;
  readonly ownerAuthTag: readonly string[];
  readonly expectedOperatorPubkey?: string;
}): { readonly operatorPubkey: string; readonly conditions: string } => {
  const agentPubkey = assertHexPubkey(params.agentPubkey, "agentPubkey");
  const tag = params.ownerAuthTag;
  if (tag.length < 4 || tag[0] !== "auth") {
    throw new CommunityMembershipError(
      "agent_not_attested",
      "community: auth tag must be [\"auth\", owner, conditions, sig]",
    );
  }
  if (!verifyOwnerAuthTag(tag, agentPubkey)) {
    throw new CommunityMembershipError(
      "agent_not_attested",
      "community: NIP-OA verification failed",
    );
  }
  const operatorPubkey = assertHexPubkey(tag[1] ?? "", "operatorPubkey");
  if (operatorPubkey === agentPubkey) {
    throw new CommunityMembershipError(
      "agent_self_attestation",
      "community: self-attestation is forbidden",
    );
  }
  if (
    params.expectedOperatorPubkey !== undefined &&
    operatorPubkey !== assertHexPubkey(params.expectedOperatorPubkey, "expectedOperator")
  ) {
    throw new CommunityMembershipError(
      "agent_operator_mismatch",
      "community: auth tag operator does not match member",
    );
  }
  return { operatorPubkey, conditions: tag[2] ?? "" };
};

/**
 * Build a NIP-AA AUTH template (kind 22242) that carries exactly one NIP-OA tag.
 * The agent signs this template. The relay must admit attested keys only.
 */
export const buildCommunityAttestedAuthTemplate = (params: {
  readonly challenge: string;
  readonly relayUrl: string;
  readonly ownerAuthTag: readonly string[];
  readonly createdAt?: number;
}): SarahNostrEventTemplate => {
  if (params.ownerAuthTag.length < 4 || params.ownerAuthTag[0] !== "auth") {
    throw new CommunityMembershipError(
      "agent_not_attested",
      "community: AUTH requires a NIP-OA auth tag",
    );
  }
  return buildAttestedAuthTemplate({
    challenge: params.challenge,
    relayUrl: params.relayUrl,
    ownerAuthTag: params.ownerAuthTag,
    ...(params.createdAt !== undefined ? { createdAt: params.createdAt } : {}),
  });
};

/**
 * Extract the single NIP-OA auth tag from a NIP-AA AUTH template or signed event.
 * Refuse anonymous AUTH (no auth tag) and multi-tag AUTH.
 */
export const extractAuthTagFromAuthEvent = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
): CommunityOwnerAuthTag => {
  const authTags = tags.filter((t) => t[0] === "auth");
  if (authTags.length === 0) {
    throw new CommunityMembershipError(
      "anonymous_pubkey_refused",
      "community: relay must refuse AUTH without NIP-OA attestation",
    );
  }
  if (authTags.length !== 1) {
    throw new CommunityMembershipError(
      "agent_not_attested",
      "community: AUTH must carry exactly one NIP-OA auth tag",
    );
  }
  const tag = authTags[0]!;
  if (tag.length < 4) {
    throw new CommunityMembershipError(
      "agent_not_attested",
      "community: auth tag is incomplete",
    );
  }
  return ["auth", tag[1]!, tag[2]!, tag[3]!] as CommunityOwnerAuthTag;
};

/**
 * Admit an agent pubkey for relay auth only when NIP-OA verifies and binds
 * a distinct operator. Anonymous and self-attested keys are refused.
 */
export const admitAttestedAgentKey = (params: {
  readonly agentPubkey: string;
  readonly authEventTags: ReadonlyArray<ReadonlyArray<string>>;
  readonly expectedOperatorPubkey?: string;
}): { readonly operatorPubkey: string; readonly ownerAuthTag: CommunityOwnerAuthTag } => {
  const agentPubkey = assertHexPubkey(params.agentPubkey, "agentPubkey");
  const ownerAuthTag = extractAuthTagFromAuthEvent(params.authEventTags);
  const { operatorPubkey } = verifyAgentOwnerAttestation({
    agentPubkey,
    ownerAuthTag,
    ...(params.expectedOperatorPubkey !== undefined
      ? { expectedOperatorPubkey: params.expectedOperatorPubkey }
      : {}),
  });
  return { operatorPubkey, ownerAuthTag };
};

/** Public-safe NIP-AP persona projection. Rejects secret-shaped fields. */
export const attachPersonaRef = (
  persona: CommunityPersonaRef,
): CommunityPersonaRef => {
  assertCommunityPublicSafe(persona);
  if (persona.kind !== 30175) {
    throw new CommunityMembershipError(
      "secret_shaped_payload",
      "community: persona kind must be 30175 (NIP-AP)",
    );
  }
  return {
    kind: 30175,
    dTag: persona.dTag,
    displayName: persona.displayName,
    declaredCapabilities: [...persona.declaredCapabilities],
  };
};
