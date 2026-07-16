/**
 * Trusted peer profiles and registry admission (ACP-9 #8896).
 *
 * Profiles are declarative data validated by a strict fail-closed contract;
 * the trusted registry is the only launch authority; official-registry
 * snapshots are bounded discovery metadata; admission derives support state
 * from profile plus conformance evidence and quarantines anything a profile
 * did not declare.
 */

export {
  ACP_PEER_PROFILE_CONTRACT_VERSION,
  parseAcpTrustedPeerProfile,
  type AcpExecutableStrategy,
  type AcpPeerProfileParseResult,
  type AcpPeerProfileRejection,
  type AcpPeerProfileRejectionReason,
  type AcpProfileCapabilityState,
  type AcpProfileExtensionDirection,
  type AcpTrustedPeerProfile,
  type AcpVersionRange,
} from "./schema.ts";

export {
  createAcpTrustedPeerProfileRegistry,
  getAcpTrustedPeerProfile,
  ingestOfficialAcpRegistrySnapshot,
  resolveDiscoveryEntryToTrustedProfile,
  type AcpRegistryDiscoveryEntry,
  type AcpRegistryDiscoverySnapshot,
  type AcpRegistrySnapshotResult,
  type AcpTrustedPeerProfileRegistry,
  type AcpTrustedRegistryResult,
} from "./registry.ts";

export {
  ACP_UNKNOWN_PEER_ACKNOWLEDGEMENT,
  admitAcpPeerProfile,
  admitUnknownAcpPeerExperimental,
  buildAdmittedLaunchEnvironment,
  deriveAcpSupportState,
  evaluateAcpExecutableTrust,
  extractLeadingSemver,
  resolveAcpTrustedLaunchPlan,
  versionInRanges,
  type AcpAdmissionDiagnostics,
  type AcpAdmissionGrants,
  type AcpAdmittedSessionLaunch,
  type AcpConformanceEvidenceRecord,
  type AcpExecutableIdentityPin,
  type AcpExecutableProbe,
  type AcpExecutableTrustResult,
  type AcpLaunchEnvironmentResult,
  type AcpPeerAdmissionDecision,
  type AcpPeerAdmissionRefusalReason,
  type AcpPeerSupportState,
  type AcpTrustedLaunchPlan,
  type AcpUnknownPeerExperimentalAdmission,
  type AcpUnknownPeerExperimentalResult,
} from "./admission.ts";

export {
  GROK_ACP_VERSION_COMPATIBILITY,
  GROK_TRUSTED_PEER_PROFILE,
  grokAcpCompatibilityForVersion,
} from "./grok.ts";
export { CURSOR_TRUSTED_PEER_PROFILE } from "./cursor.ts";

import { CURSOR_TRUSTED_PEER_PROFILE } from "./cursor.ts";
import { GROK_TRUSTED_PEER_PROFILE } from "./grok.ts";
import { createAcpTrustedPeerProfileRegistry, type AcpTrustedRegistryResult } from "./registry.ts";

/**
 * Builds the default OpenAgents trusted registry containing the two required
 * reference profiles. Returns the full result union so callers observe a
 * rejected registry instead of an exception.
 */
export const createDefaultAcpTrustedPeerProfileRegistry = (): AcpTrustedRegistryResult =>
  createAcpTrustedPeerProfileRegistry([GROK_TRUSTED_PEER_PROFILE, CURSOR_TRUSTED_PEER_PROFILE]);
