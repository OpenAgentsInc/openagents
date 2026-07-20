import { Schema as S } from "effect";

import type { TurnProviderCandidate } from "@openagentsinc/agent-runtime-schema";

/**
 * `@openagentsinc/apple-fm-runtime` — neutral Apple Foundation Models provider
 * package (AFS-00 reservation, root export).
 *
 * The root export is portable and bundle-safe. It owns the frozen wire-version
 * source and the portable Apple FM identity. It must not import Node. The
 * `./node` subpath owns helper discovery, signature checks, spawn, readiness,
 * and shutdown. The `./testing` subpath owns fixtures and a fake transport.
 *
 * Packet AFS-02 moves the portable wire schemas, client, capability probe,
 * provider adapter, supervisor contract, receipt verification, fixtures, and
 * the Swift `foundation-bridge` source here from the nested Pylon runtime.
 * AFS-00 reserves the package, its subpath export map, and the single
 * wire-version source that AFS-02 generates the native manifest and the Desktop
 * staging pin from.
 */
export const APPLE_FM_RUNTIME_PACKAGE = "@openagentsinc/apple-fm-runtime" as const;
export const APPLE_FM_RUNTIME_RESERVED = true as const;

/** Apple FM is the local advisory inference candidate. It is never a router. */
export const APPLE_FM_TURN_CANDIDATE: TurnProviderCandidate = "apple_fm";

/**
 * The single frozen wire-version source. AFS-02 must generate the native helper
 * manifest, the Swift `bridgeVersion`, and the accepted Desktop staging pin from
 * these values so they cannot drift again.
 */
export const APPLE_FM_BRIDGE_WIRE_VERSION = "openagents.apple_fm.bridge.wire.v0.2" as const;
export const APPLE_FM_HELPER_PROTOCOL_VERSION = 1 as const;
export const APPLE_FM_BACKEND_KIND = "apple_fm_bridge" as const;
export const APPLE_FM_LOCAL_PROFILE_ID = "apple-fm-local" as const;
export const APPLE_FM_DEFAULT_MODEL_ID = "apple-foundation-model" as const;

/**
 * Canonical helper version. This is the Swift `foundation-bridge` source truth
 * at the AFS-00 snapshot (`main.swift` `bridgeVersion`). AFS-02 generates every
 * other copy from it.
 */
export const APPLE_FM_CANONICAL_HELPER_VERSION = "0.1.3" as const;

/**
 * The current Desktop staging pin at the AFS-00 snapshot
 * (`apps/openagents-desktop/scripts/stage-target.ts` `APPLE_FM_BRIDGE_VERSION`).
 * It disagrees with the canonical helper version. AFS-00 records this drift as a
 * finding; AFS-02 removes the second copy and generates it from the canonical
 * source. Do not "fix" the drift by editing this constant.
 */
export const APPLE_FM_DESKTOP_STAGING_VERSION_AT_SNAPSHOT = "0.1.1" as const;

/**
 * Compare the canonical helper version with a staging pin. AFS-02 reuses this
 * to fail closed when the generated staging value does not match the single
 * wire-version source.
 */
export const appleFmVersionSourcesAgree = (helperVersion: string, stagingVersion: string): boolean =>
  helperVersion === stagingVersion;

/**
 * Whether the recorded helper and Desktop staging versions agree. `false` at
 * the AFS-00 snapshot. This is the intentionally-recorded version-drift finding.
 */
export const APPLE_FM_VERSION_SOURCES_AGREE_AT_SNAPSHOT = appleFmVersionSourcesAgree(
  APPLE_FM_CANONICAL_HELPER_VERSION,
  APPLE_FM_DESKTOP_STAGING_VERSION_AT_SNAPSHOT,
);

/** Local data destination. Apple FM input stays on the device. */
export const APPLE_FM_DATA_DESTINATION = "on_device_local" as const;

/** Apple FM declares no external tool or action capability. */
export const APPLE_FM_SUPPORTS_EXTERNAL_ACTIONS = false as const;

/**
 * The frozen Apple FM unavailable-reason vocabulary. It matches the current
 * consumer contract so a decoder on any surface reaches the same fact.
 */
export const AppleFmUnavailableReason = S.Literals([
  "bridge_unreachable",
  "apple_intelligence_disabled",
  "unsupported_hardware",
  "model_unavailable",
  "permission_denied",
  "malformed_response",
  "not_ready",
  "unknown",
]);
export type AppleFmUnavailableReason = typeof AppleFmUnavailableReason.Type;
