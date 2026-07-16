/**
 * Trusted peer-profile registry and bounded official-registry ingestion
 * (ACP-9 #8896).
 *
 * Two registries exist and must never be conflated:
 *
 * 1. The trusted peer-profile registry is built only from profiles that pass
 *    the strict contract parser. It is the sole source of launch authority.
 * 2. Official Agent Client Protocol registry snapshots are discovery metadata
 *    only. Ingestion is pinned (digest-verified when a pin exists), bounded,
 *    validated against a local schema, and deterministic. A discovery entry
 *    carries no executable, argv, environment, installer, or extension
 *    fields, so it cannot confer execution or authority by itself.
 */

import { createHash } from "node:crypto";

import {
  type AcpPeerProfileRejection,
  type AcpTrustedPeerProfile,
  parseAcpTrustedPeerProfile,
} from "./schema.ts";

const MAX_TRUSTED_PROFILES = 64;

export type AcpTrustedPeerProfileRegistry = Readonly<{
  _tag: "AcpTrustedPeerProfileRegistry";
  contractVersion: 1;
  profiles: ReadonlyMap<string, AcpTrustedPeerProfile>;
  extensionOwners: ReadonlyMap<string, string>;
}>;

export type AcpTrustedRegistryResult =
  | Readonly<{ _tag: "RegistryReady"; registry: AcpTrustedPeerProfileRegistry }>
  | Readonly<{
      _tag: "RegistryRejected";
      reason:
        | "profile_rejected"
        | "duplicate_profile_id"
        | "extension_namespace_collision"
        | "unbounded_value";
      detail: string;
      path: string;
      profileRejection?: AcpPeerProfileRejection;
    }>;

export const createAcpTrustedPeerProfileRegistry = (
  candidates: ReadonlyArray<unknown>,
): AcpTrustedRegistryResult => {
  if (candidates.length > MAX_TRUSTED_PROFILES) {
    return {
      _tag: "RegistryRejected",
      reason: "unbounded_value",
      detail: `registry accepts at most ${String(MAX_TRUSTED_PROFILES)} trusted profiles`,
      path: "registry",
    };
  }
  const profiles = new Map<string, AcpTrustedPeerProfile>();
  const extensionOwners = new Map<string, string>();
  for (const [index, candidate] of candidates.entries()) {
    const parsed = parseAcpTrustedPeerProfile(candidate);
    if (parsed._tag === "PeerProfileRejected") {
      return {
        _tag: "RegistryRejected",
        reason: "profile_rejected",
        detail: parsed.detail,
        path: `registry[${String(index)}].${parsed.path}`,
        profileRejection: parsed,
      };
    }
    const profile = parsed.profile;
    if (profiles.has(profile.profileId)) {
      return {
        _tag: "RegistryRejected",
        reason: "duplicate_profile_id",
        detail: `profile id ${profile.profileId} is already registered`,
        path: `registry[${String(index)}].profileId`,
      };
    }
    for (const extension of profile.extensions) {
      const owner = extensionOwners.get(extension.method);
      if (owner !== undefined && owner !== profile.profileId) {
        return {
          _tag: "RegistryRejected",
          reason: "extension_namespace_collision",
          detail: `extension method ${extension.method} is already owned by profile ${owner}`,
          path: `registry[${String(index)}].extensions`,
        };
      }
      extensionOwners.set(extension.method, profile.profileId);
    }
    profiles.set(profile.profileId, profile);
  }
  return {
    _tag: "RegistryReady",
    registry: Object.freeze({
      _tag: "AcpTrustedPeerProfileRegistry" as const,
      contractVersion: 1 as const,
      profiles,
      extensionOwners,
    }),
  };
};

export const getAcpTrustedPeerProfile = (
  registry: AcpTrustedPeerProfileRegistry,
  profileId: string,
): AcpTrustedPeerProfile | undefined => registry.profiles.get(profileId);

/**
 * Bounded discovery projection of an official registry snapshot. This type
 * deliberately has no launch, argv, environment, installer, or extension
 * fields: discovery can suggest that a peer exists, never how to run it.
 */
export type AcpRegistryDiscoveryEntry = Readonly<{
  authority: "discovery-metadata-only";
  entryId: string;
  displayName: string;
  description: string;
  distributionKinds: ReadonlyArray<string>;
}>;

export type AcpRegistryDiscoverySnapshot = Readonly<{
  authority: "discovery-metadata-only";
  source: string;
  snapshotSha256: string;
  byteLength: number;
  entryCount: number;
  droppedDuplicateEntryIds: ReadonlyArray<string>;
  entries: ReadonlyArray<AcpRegistryDiscoveryEntry>;
}>;

export type AcpRegistrySnapshotResult =
  | Readonly<{ _tag: "RegistrySnapshotReady"; snapshot: AcpRegistryDiscoverySnapshot }>
  | Readonly<{
      _tag: "RegistrySnapshotRejected";
      reason:
        | "oversized_snapshot"
        | "entry_limit_exceeded"
        | "digest_mismatch"
        | "malformed_snapshot";
      detail: string;
    }>;

const SNAPSHOT_LIMITS = {
  maxBytes: 262_144,
  maxEntries: 128,
  maxTextLength: 512,
  maxDistributionKinds: 8,
} as const;

const ENTRY_ID = /^[a-z0-9][a-z0-9._-]{0,63}$/;

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const boundedText = (value: unknown): string | undefined =>
  typeof value === "string" && value.length > 0 && value.length <= SNAPSHOT_LIMITS.maxTextLength
    ? value
    : undefined;

const freezeDeep = <T>(value: T): T => {
  if (typeof value === "object" && value !== null) {
    for (const child of Object.values(value)) freezeDeep(child);
    Object.freeze(value);
  }
  return value;
};

export const ingestOfficialAcpRegistrySnapshot = (
  input: Readonly<{ source: string; rawJson: string; expectedSha256?: string }>,
): AcpRegistrySnapshotResult => {
  const byteLength = Buffer.byteLength(input.rawJson, "utf8");
  if (byteLength > SNAPSHOT_LIMITS.maxBytes) {
    return {
      _tag: "RegistrySnapshotRejected",
      reason: "oversized_snapshot",
      detail: `snapshot exceeds the ${String(SNAPSHOT_LIMITS.maxBytes)}-byte bound`,
    };
  }
  const snapshotSha256 = createHash("sha256").update(input.rawJson, "utf8").digest("hex");
  if (input.expectedSha256 !== undefined && input.expectedSha256 !== snapshotSha256) {
    return {
      _tag: "RegistrySnapshotRejected",
      reason: "digest_mismatch",
      detail: "snapshot content does not match the pinned sha-256 digest",
    };
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(input.rawJson);
  } catch {
    return {
      _tag: "RegistrySnapshotRejected",
      reason: "malformed_snapshot",
      detail: "snapshot is not valid JSON",
    };
  }
  const agents = isObject(parsed) && Array.isArray(parsed.agents) ? parsed.agents : undefined;
  if (agents === undefined) {
    return {
      _tag: "RegistrySnapshotRejected",
      reason: "malformed_snapshot",
      detail: "snapshot must be an object with an agents array",
    };
  }
  if (agents.length > SNAPSHOT_LIMITS.maxEntries) {
    return {
      _tag: "RegistrySnapshotRejected",
      reason: "entry_limit_exceeded",
      detail: `snapshot exceeds the ${String(SNAPSHOT_LIMITS.maxEntries)}-entry bound`,
    };
  }
  const projected: Array<AcpRegistryDiscoveryEntry & { originalIndex: number }> = [];
  for (const [index, candidate] of agents.entries()) {
    if (!isObject(candidate)) {
      return {
        _tag: "RegistrySnapshotRejected",
        reason: "malformed_snapshot",
        detail: `agents[${String(index)}] is not an object`,
      };
    }
    const displayName = boundedText(candidate.name);
    if (displayName === undefined) {
      return {
        _tag: "RegistrySnapshotRejected",
        reason: "malformed_snapshot",
        detail: `agents[${String(index)}].name must be a bounded non-empty string`,
      };
    }
    const rawId =
      boundedText(candidate.id) ?? displayName.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-");
    const entryId = rawId.replaceAll(/^-+|-+$/g, "").slice(0, 64);
    if (!ENTRY_ID.test(entryId)) {
      return {
        _tag: "RegistrySnapshotRejected",
        reason: "malformed_snapshot",
        detail: `agents[${String(index)}] does not yield a valid entry id`,
      };
    }
    const distributions = isObject(candidate.distribution)
      ? Object.keys(candidate.distribution)
      : Array.isArray(candidate.distributions)
        ? candidate.distributions
            .map((entry) => (isObject(entry) ? entry.kind : entry))
            .filter((kind): kind is string => typeof kind === "string")
        : [];
    projected.push({
      authority: "discovery-metadata-only",
      entryId,
      displayName,
      description: boundedText(candidate.description) ?? "",
      distributionKinds: distributions
        .filter((kind) => kind.length <= 64)
        .slice(0, SNAPSHOT_LIMITS.maxDistributionKinds)
        .toSorted(),
      originalIndex: index,
    });
  }
  // Deterministic duplicate resolution: sort by (entryId, originalIndex) and
  // keep the first occurrence of each id.
  projected.sort((a, b) => a.entryId.localeCompare(b.entryId) || a.originalIndex - b.originalIndex);
  const seen = new Set<string>();
  const droppedDuplicateEntryIds: Array<string> = [];
  const entries: Array<AcpRegistryDiscoveryEntry> = [];
  for (const { originalIndex: _originalIndex, ...entry } of projected) {
    if (seen.has(entry.entryId)) {
      droppedDuplicateEntryIds.push(entry.entryId);
      continue;
    }
    seen.add(entry.entryId);
    entries.push(entry);
  }
  return {
    _tag: "RegistrySnapshotReady",
    snapshot: freezeDeep({
      authority: "discovery-metadata-only" as const,
      source: input.source.slice(0, SNAPSHOT_LIMITS.maxTextLength),
      snapshotSha256,
      byteLength,
      entryCount: entries.length,
      droppedDuplicateEntryIds,
      entries,
    }),
  };
};

/**
 * Discovery-to-trust resolution: a discovery entry maps to launch authority
 * only when a trusted profile explicitly claims the same provider or profile
 * id. Everything else stays metadata.
 */
export const resolveDiscoveryEntryToTrustedProfile = (
  registry: AcpTrustedPeerProfileRegistry,
  entry: AcpRegistryDiscoveryEntry,
): AcpTrustedPeerProfile | undefined => {
  const direct = registry.profiles.get(entry.entryId);
  if (direct !== undefined) return direct;
  for (const profile of registry.profiles.values()) {
    if (profile.providerId === entry.entryId) return profile;
  }
  return undefined;
};
