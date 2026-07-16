/**
 * Fail-closed peer admission (ACP-9 #8896).
 *
 * Admission combines a trusted registered profile with an executable
 * identity probe and conformance evidence. It is the only path that yields a
 * launch plan, and the launch plan is copied exclusively from the trusted
 * registry — caller-supplied argv is refused outright. Support state is
 * derived from profile version ranges plus evidence, never from a provider
 * name or wire version alone. Unknown observed capabilities and extension
 * methods are quarantined, not silently accepted, and unknown peers only run
 * through the explicit experimental flow with every risky grant disabled.
 *
 * This module is provider-neutral by construction: it must never grow
 * peer-specific conditionals. Provider quirks belong in the declarative
 * profiles and their separate extension modules.
 */

import type { AcpTrustedPeerProfileRegistry } from "./registry.ts";
import type { AcpTrustedPeerProfile, AcpVersionRange } from "./schema.ts";

export type AcpExecutableProbe = Readonly<{
  requestedExecutable: string;
  resolvedPath: string;
  realPath: string;
  sha256: string;
  /** Optional peer-specific digest of every executable dependency in its installation closure. */
  closureSha256?: string;
  reportedVersion: string;
  platform: Readonly<{ os: string; arch: string }>;
}>;

export type AcpExecutableIdentityPin = Readonly<{
  realPath: string;
  sha256: string;
  closureSha256?: string;
}>;

export type AcpConformanceEvidenceRecord = Readonly<{
  suiteId: string;
  kind: "fixture" | "live";
  result: "pass" | "fail";
  peerVersion: string;
  executableSha256?: string;
  installationClosureSha256?: string;
  platform?: Readonly<{ os: string; arch: string }>;
  recordedAt: string;
  artifactRef: string;
}>;

export const ACP_FULL_RELEASE_SUITE_ID = "acp-release-matrix-v1" as const;

export type AcpPeerSupportState = "supported" | "experimental" | "incompatible";

export type AcpTrustedLaunchPlan = Readonly<{
  _tag: "AcpTrustedLaunchPlan";
  source: "trusted-peer-profile-registry";
  profileId: string;
  profileRevision: number;
  strategy: AcpTrustedPeerProfile["launch"]["strategy"];
  executable: string;
  args: ReadonlyArray<string>;
  versionProbeArgs: ReadonlyArray<string>;
  allowedEnvKeys: ReadonlyArray<string>;
  requiredEnvKeys: ReadonlyArray<string>;
}>;

export type AcpAdmissionGrants = Readonly<{
  fsReadTextFile: boolean;
  fsWriteTextFile: boolean;
  terminal: boolean;
  permissionAutoApproval: false;
  vendorExtensionMethods: ReadonlyArray<string>;
  network: boolean;
}>;

export type AcpAdmissionDiagnostics = Readonly<{
  profileId: string;
  providerId: string;
  profileRevision: number;
  contractVersion: 1;
  schemaRelease: "schema-v1.19.0";
  supportState: AcpPeerSupportState | "experimental-unsupported" | "refused";
  peerVersion: string;
  executableBasename: string;
  executableSha256: string;
  registrySnapshotSha256?: string;
  evidenceArtifactRefs: ReadonlyArray<string>;
}>;

export type AcpPeerAdmissionRefusalReason =
  | "caller_launch_override_rejected"
  | "unknown_profile"
  | "platform_unsupported"
  | "identity_mismatch"
  | "version_unknown"
  | "version_denied"
  | "path_replacement"
  | "incompatible_peer"
  | "experimental_acknowledgement_required";

export type AcpPeerAdmissionDecision =
  | Readonly<{
      _tag: "PeerAdmitted";
      profileId: string;
      supportState: Exclude<AcpPeerSupportState, "incompatible">;
      peerVersion: string;
      launchPlan: AcpTrustedLaunchPlan;
      grants: AcpAdmissionGrants;
      identityPin: AcpExecutableIdentityPin;
      quarantinedCapabilities: ReadonlyArray<string>;
      quarantinedExtensionMethods: ReadonlyArray<string>;
      diagnostics: AcpAdmissionDiagnostics;
    }>
  | Readonly<{
      _tag: "PeerAdmissionRefused";
      reason: AcpPeerAdmissionRefusalReason;
      detail: string;
      diagnostics?: AcpAdmissionDiagnostics;
    }>;

const freezeDeep = <T>(value: T): T => {
  if (typeof value === "object" && value !== null) {
    for (const child of Object.values(value)) freezeDeep(child);
    Object.freeze(value);
  }
  return value;
};

const basename = (path: string): string => {
  const segments = path.split("/");
  return segments[segments.length - 1] ?? path;
};

const SEMVER_TRIPLE = /(\d{1,6})\.(\d{1,6})\.(\d{1,6})/;

export const extractLeadingSemver = (text: string): string | undefined => {
  const match = SEMVER_TRIPLE.exec(text.slice(0, 256));
  return match === null
    ? undefined
    : `${String(Number(match[1]))}.${String(Number(match[2]))}.${String(Number(match[3]))}`;
};

const triple = (version: string): readonly [number, number, number] | undefined => {
  const match = SEMVER_TRIPLE.exec(version);
  if (match === null || match[0] !== version) return undefined;
  return [Number(match[1]), Number(match[2]), Number(match[3])];
};

const compareTriple = (
  a: readonly [number, number, number],
  b: readonly [number, number, number],
): number => a[0] - b[0] || a[1] - b[1] || a[2] - b[2];

export const versionInRanges = (
  version: string,
  ranges: ReadonlyArray<AcpVersionRange>,
): boolean => {
  const value = triple(version);
  if (value === undefined) return false;
  return ranges.some((range) => {
    if (range.kind === "exact") {
      const exact = triple(range.version);
      return exact !== undefined && compareTriple(value, exact) === 0;
    }
    const from = triple(range.fromInclusive);
    const to = triple(range.toExclusive);
    return (
      from !== undefined &&
      to !== undefined &&
      compareTriple(value, from) >= 0 &&
      compareTriple(value, to) < 0
    );
  });
};

export const resolveAcpTrustedLaunchPlan = (
  registry: AcpTrustedPeerProfileRegistry,
  profileId: string,
): AcpTrustedLaunchPlan | undefined => {
  const profile = registry.profiles.get(profileId);
  if (profile === undefined) return undefined;
  return freezeDeep({
    _tag: "AcpTrustedLaunchPlan" as const,
    source: "trusted-peer-profile-registry" as const,
    profileId: profile.profileId,
    profileRevision: profile.profileRevision,
    strategy: profile.launch.strategy,
    executable: profile.launch.executable,
    args: [...profile.launch.args],
    versionProbeArgs: [...profile.launch.versionProbeArgs],
    allowedEnvKeys: [...profile.environment.allowedKeys],
    requiredEnvKeys: profile.environment.secretRefs
      .filter((ref) => ref.requirement === "required")
      .map((ref) => ref.key),
  });
};

export type AcpLaunchEnvironmentResult =
  | Readonly<{ _tag: "LaunchEnvironmentReady"; env: Readonly<Record<string, string>> }>
  | Readonly<{
      _tag: "LaunchEnvironmentRejected";
      reason: "missing_required_secret";
      detail: string;
    }>;

/**
 * Filters a caller environment down to the profile's allowlisted keys.
 * Undeclared keys (including PATH-shadowing and loader-injection vectors) are
 * dropped; missing required secret keys fail closed. Values are never logged.
 */
export const buildAdmittedLaunchEnvironment = (
  plan: AcpTrustedLaunchPlan,
  environment: Readonly<Record<string, string | undefined>>,
): AcpLaunchEnvironmentResult => {
  const env: Record<string, string> = {};
  for (const key of plan.allowedEnvKeys) {
    const value = environment[key];
    if (typeof value === "string") env[key] = value;
  }
  for (const key of plan.requiredEnvKeys) {
    if (env[key] === undefined) {
      return {
        _tag: "LaunchEnvironmentRejected",
        reason: "missing_required_secret",
        detail: `required secret environment key ${key} is not present`,
      };
    }
  }
  return { _tag: "LaunchEnvironmentReady", env: Object.freeze(env) };
};

export type AcpExecutableTrustResult =
  | Readonly<{ _tag: "ExecutableTrusted"; peerVersion: string; pin: AcpExecutableIdentityPin }>
  | Readonly<{
      _tag: "ExecutableRejected";
      reason:
        | "platform_unsupported"
        | "identity_mismatch"
        | "version_unknown"
        | "version_denied"
        | "path_replacement";
      detail: string;
    }>;

export const evaluateAcpExecutableTrust = (
  input: Readonly<{
    profile: AcpTrustedPeerProfile;
    probe: AcpExecutableProbe;
    priorPin?: AcpExecutableIdentityPin;
  }>,
): AcpExecutableTrustResult => {
  const { profile, probe, priorPin } = input;
  if (
    !profile.platforms.some(
      (platform) => platform.os === probe.platform.os && platform.arch === probe.platform.arch,
    )
  ) {
    return {
      _tag: "ExecutableRejected",
      reason: "platform_unsupported",
      detail: `platform ${probe.platform.os}/${probe.platform.arch} is not declared by the profile`,
    };
  }
  if (probe.requestedExecutable !== profile.launch.executable) {
    return {
      _tag: "ExecutableRejected",
      reason: "identity_mismatch",
      detail: "probe was taken for a different executable than the profile launches",
    };
  }
  if (basename(probe.resolvedPath) !== profile.identity.expectedExecutableBasename) {
    return {
      _tag: "ExecutableRejected",
      reason: "identity_mismatch",
      detail: "resolved executable basename does not match the profile identity",
    };
  }
  if (
    priorPin !== undefined &&
    (priorPin.realPath !== probe.realPath ||
      priorPin.sha256 !== probe.sha256 ||
      priorPin.closureSha256 !== probe.closureSha256)
  ) {
    return {
      _tag: "ExecutableRejected",
      reason: "path_replacement",
      detail:
        "executable identity changed since it was pinned (PATH shadowing, symlink retarget, or post-install replacement)",
    };
  }
  const peerVersion = extractLeadingSemver(probe.reportedVersion);
  if (peerVersion === undefined) {
    return {
      _tag: "ExecutableRejected",
      reason: "version_unknown",
      detail: "version probe output does not contain a pinned x.y.z version",
    };
  }
  if (versionInRanges(peerVersion, profile.versions.denied)) {
    return {
      _tag: "ExecutableRejected",
      reason: "version_denied",
      detail: `peer version ${peerVersion} is in a denied range`,
    };
  }
  return {
    _tag: "ExecutableTrusted",
    peerVersion,
    pin: Object.freeze({
      realPath: probe.realPath,
      sha256: probe.sha256,
      ...(probe.closureSha256 === undefined ? {} : { closureSha256: probe.closureSha256 }),
    }),
  };
};

const evidenceIsFresh = (
  record: AcpConformanceEvidenceRecord,
  maxEvidenceAgeDays: number,
  now: Date,
): boolean => {
  const recorded = Date.parse(record.recordedAt);
  if (Number.isNaN(recorded)) return false;
  const ageMs = now.getTime() - recorded;
  return ageMs >= 0 && ageMs <= maxEvidenceAgeDays * 86_400_000;
};

export const deriveAcpSupportState = (
  input: Readonly<{
    profile: AcpTrustedPeerProfile;
    peerVersion: string;
    executableSha256: string;
    installationClosureSha256?: string;
    platform: Readonly<{ os: string; arch: string }>;
    evidence: ReadonlyArray<AcpConformanceEvidenceRecord>;
    now: Date;
  }>,
): AcpPeerSupportState => {
  const {
    profile,
    peerVersion,
    executableSha256,
    installationClosureSha256,
    platform,
    evidence,
    now,
  } = input;
  if (versionInRanges(peerVersion, profile.versions.denied)) return "incompatible";
  const relevant = evidence.filter((record) => record.peerVersion === peerVersion);
  if (relevant.some((record) => record.result === "fail")) return "incompatible";
  const inSupported = versionInRanges(peerVersion, profile.versions.supported);
  const inExperimental = versionInRanges(peerVersion, profile.versions.experimental);
  if (!inSupported && !inExperimental) return "incompatible";
  if (!inSupported) return "experimental";
  const fresh = relevant.filter(
    (record) =>
      record.result === "pass" && evidenceIsFresh(record, profile.evidence.maxEvidenceAgeDays, now),
  );
  const fixturesSatisfied = profile.evidence.fixtureSuites.every((suiteId) =>
    fresh.some((record) => record.kind === "fixture" && record.suiteId === suiteId),
  );
  const liveSatisfied =
    !profile.evidence.liveMatrixRequired ||
    fresh.some(
      (record) =>
        record.kind === "live" &&
        record.suiteId === ACP_FULL_RELEASE_SUITE_ID &&
        record.executableSha256 === executableSha256 &&
        record.platform?.os === platform.os &&
        record.platform.arch === platform.arch &&
        (installationClosureSha256 === undefined ||
          record.installationClosureSha256 === installationClosureSha256),
    );
  return fixturesSatisfied && liveSatisfied ? "supported" : "experimental";
};

const capabilityGrant = (
  profile: AcpTrustedPeerProfile,
  supportState: AcpPeerSupportState,
  capability: string,
): boolean =>
  supportState === "supported" &&
  profile.capabilities.some(
    (entry) => entry.capability === capability && entry.state === "supported",
  );

export const admitAcpPeerProfile = (
  input: Readonly<{
    registry: AcpTrustedPeerProfileRegistry;
    profileId: string;
    probe: AcpExecutableProbe;
    evidence: ReadonlyArray<AcpConformanceEvidenceRecord>;
    now: Date;
    priorPin?: AcpExecutableIdentityPin;
    observedAgentName?: string;
    observedAgentCapabilityKeys?: ReadonlyArray<string>;
    observedExtensionMethods?: ReadonlyArray<string>;
    /**
     * Present only when a caller attempts to override the launch command.
     * Admission always refuses it: launch authority is registry-only.
     */
    requestedLaunchOverride?: Readonly<{
      executable?: string;
      args?: ReadonlyArray<string>;
      env?: Readonly<Record<string, string>>;
    }>;
  }>,
): AcpPeerAdmissionDecision => {
  if (input.requestedLaunchOverride !== undefined) {
    return {
      _tag: "PeerAdmissionRefused",
      reason: "caller_launch_override_rejected",
      detail:
        "launch commands, argv, and environment come only from the trusted peer-profile registry",
    };
  }
  const profile = input.registry.profiles.get(input.profileId);
  if (profile === undefined) {
    return {
      _tag: "PeerAdmissionRefused",
      reason: "unknown_profile",
      detail: `profile ${input.profileId} is not in the trusted registry; unknown peers require the explicit experimental flow`,
    };
  }
  const diagnostics = (
    supportState: AcpAdmissionDiagnostics["supportState"],
    peerVersion: string,
    evidenceArtifactRefs: ReadonlyArray<string>,
  ): AcpAdmissionDiagnostics =>
    freezeDeep({
      profileId: profile.profileId,
      providerId: profile.providerId,
      profileRevision: profile.profileRevision,
      contractVersion: profile.contractVersion,
      schemaRelease: profile.schemaRelease,
      supportState,
      peerVersion,
      executableBasename: basename(input.probe.resolvedPath),
      executableSha256: input.probe.sha256,
      ...(profile.provenance.registrySnapshotSha256 === undefined
        ? {}
        : { registrySnapshotSha256: profile.provenance.registrySnapshotSha256 }),
      evidenceArtifactRefs,
    });

  const trust = evaluateAcpExecutableTrust({
    profile,
    probe: input.probe,
    ...(input.priorPin === undefined ? {} : { priorPin: input.priorPin }),
  });
  if (trust._tag === "ExecutableRejected") {
    return {
      _tag: "PeerAdmissionRefused",
      reason: trust.reason,
      detail: trust.detail,
      diagnostics: diagnostics("refused", "unverified", []),
    };
  }

  if (input.observedAgentName !== undefined) {
    const expected = profile.identity.expectedAgentName;
    const observed = input.observedAgentName.toLowerCase();
    const value = expected.value.toLowerCase();
    const matches = expected.kind === "exact" ? observed === value : observed.startsWith(value);
    if (!matches) {
      return {
        _tag: "PeerAdmissionRefused",
        reason: "identity_mismatch",
        detail: "initialize peer identity does not match the profile expectation",
        diagnostics: diagnostics("refused", trust.peerVersion, []),
      };
    }
  }

  const supportState = deriveAcpSupportState({
    profile,
    peerVersion: trust.peerVersion,
    executableSha256: input.probe.sha256,
    ...(input.probe.closureSha256 === undefined
      ? {}
      : { installationClosureSha256: input.probe.closureSha256 }),
    platform: input.probe.platform,
    evidence: input.evidence,
    now: input.now,
  });
  const evidenceRefs = input.evidence
    .filter((record) => record.peerVersion === trust.peerVersion)
    .map((record) => record.artifactRef);
  if (supportState === "incompatible") {
    return {
      _tag: "PeerAdmissionRefused",
      reason: "incompatible_peer",
      detail: `peer version ${trust.peerVersion} is incompatible under the profile's version and evidence policy`,
      diagnostics: diagnostics("refused", trust.peerVersion, evidenceRefs),
    };
  }

  const launchPlan = resolveAcpTrustedLaunchPlan(input.registry, profile.profileId);
  if (launchPlan === undefined) {
    return {
      _tag: "PeerAdmissionRefused",
      reason: "unknown_profile",
      detail: "trusted launch plan resolution failed",
    };
  }

  const declaredCapabilityKeys = new Set(profile.capabilities.map((entry) => entry.capability));
  const quarantinedCapabilities = (input.observedAgentCapabilityKeys ?? []).filter(
    (key) => !declaredCapabilityKeys.has(key),
  );
  const declaredExtensionMethods = new Set(profile.extensions.map((entry) => entry.method));
  const quarantinedExtensionMethods = (input.observedExtensionMethods ?? []).filter(
    (method) => !declaredExtensionMethods.has(method),
  );

  const grants: AcpAdmissionGrants = freezeDeep({
    fsReadTextFile: capabilityGrant(profile, supportState, "fs.readTextFile"),
    fsWriteTextFile: capabilityGrant(profile, supportState, "fs.writeTextFile"),
    terminal: capabilityGrant(profile, supportState, "terminal"),
    permissionAutoApproval: false as const,
    vendorExtensionMethods:
      supportState === "supported" ? profile.extensions.map((entry) => entry.method) : [],
    network: capabilityGrant(profile, supportState, "network"),
  });

  return freezeDeep({
    _tag: "PeerAdmitted" as const,
    profileId: profile.profileId,
    supportState,
    peerVersion: trust.peerVersion,
    launchPlan,
    grants,
    identityPin: trust.pin,
    quarantinedCapabilities,
    quarantinedExtensionMethods,
    diagnostics: diagnostics(supportState, trust.peerVersion, evidenceRefs),
  });
};

export type AcpUnknownPeerExperimentalAdmission = Readonly<{
  _tag: "UnknownPeerExperimentalAdmission";
  supportState: "experimental-unsupported";
  grants: Readonly<{
    fsReadTextFile: false;
    fsWriteTextFile: false;
    terminal: false;
    permissionAutoApproval: false;
    vendorExtensionMethods: readonly [];
    network: false;
  }>;
  limits: Readonly<{ maxSessions: 1; requestTimeoutMs: number }>;
  diagnostics: Readonly<{
    supportState: "experimental-unsupported";
    executableBasename: string;
    executableSha256: string;
    peerVersion: string;
  }>;
}>;

export type AcpUnknownPeerExperimentalResult =
  | AcpUnknownPeerExperimentalAdmission
  | Readonly<{
      _tag: "PeerAdmissionRefused";
      reason: "experimental_acknowledgement_required";
      detail: string;
    }>;

export const ACP_UNKNOWN_PEER_ACKNOWLEDGEMENT =
  "unsupported-experimental-peer-with-minimum-safe-capabilities" as const;

/**
 * Explicit developer/experimental flow for peers with no trusted profile.
 * Filesystem, terminal, permission auto-approval, vendor extensions, and
 * network behavior are all disabled; the peer gets only the stable prompt
 * loop under strict resource bounds and a clear non-support status.
 */
export const admitUnknownAcpPeerExperimental = (
  input: Readonly<{
    acknowledgement: string;
    probe: AcpExecutableProbe;
  }>,
): AcpUnknownPeerExperimentalResult => {
  if (input.acknowledgement !== ACP_UNKNOWN_PEER_ACKNOWLEDGEMENT) {
    return {
      _tag: "PeerAdmissionRefused",
      reason: "experimental_acknowledgement_required",
      detail: `unknown peers require the explicit acknowledgement literal "${ACP_UNKNOWN_PEER_ACKNOWLEDGEMENT}"`,
    };
  }
  return freezeDeep({
    _tag: "UnknownPeerExperimentalAdmission" as const,
    supportState: "experimental-unsupported" as const,
    grants: {
      fsReadTextFile: false as const,
      fsWriteTextFile: false as const,
      terminal: false as const,
      permissionAutoApproval: false as const,
      vendorExtensionMethods: [] as const,
      network: false as const,
    },
    limits: { maxSessions: 1 as const, requestTimeoutMs: 10_000 },
    diagnostics: {
      supportState: "experimental-unsupported" as const,
      executableBasename: basename(input.probe.resolvedPath),
      executableSha256: input.probe.sha256,
      peerVersion: extractLeadingSemver(input.probe.reportedVersion) ?? "unverified",
    },
  });
};

/**
 * Minimal session-bridge handoff stub (ACP-4 #8891 / ACP-5 #8892 own the real
 * session runtime). An admitted launch is the only object those layers may
 * accept to start a peer process.
 */
export type AcpAdmittedSessionLaunch = Readonly<{
  launchPlan: AcpTrustedLaunchPlan;
  grants: AcpAdmissionGrants;
  identityPin: AcpExecutableIdentityPin;
}>;
