/**
 * Trusted Agent Client Protocol peer-profile contract (ACP-9 #8896).
 *
 * A peer profile is declarative data. Parsing is fail-closed: unknown keys,
 * unknown enum members, unbounded values, shell-like launch strings, path
 * traversal, undeclared environment keys, invalid or unpinned version ranges,
 * and extension namespace collisions are all rejected with a typed reason.
 * A parsed profile never confers execution by itself; admission (admission.ts)
 * combines a registered profile with executable identity probes and
 * conformance evidence before any launch plan exists.
 */

import { STABLE_METHOD_MANIFEST, UNSTABLE_METHOD_MANIFEST } from "../generated/methods.ts";

export const ACP_PEER_PROFILE_CONTRACT_VERSION = 1 as const;

export type AcpVersionRange =
  | Readonly<{ kind: "exact"; version: string }>
  | Readonly<{ kind: "bounded"; fromInclusive: string; toExclusive: string }>;

export type AcpExecutableStrategy = "trusted-path-lookup" | "absolute-path";
export type AcpProfileCapabilityState = "supported" | "experimental" | "unsupported";
export type AcpProfileExtensionDirection = "client-to-agent" | "agent-to-client";

export type AcpTrustedPeerProfile = Readonly<{
  contractVersion: typeof ACP_PEER_PROFILE_CONTRACT_VERSION;
  protocol: "Agent Client Protocol";
  schemaRelease: "schema-v1.19.0";
  wireVersion: 1;
  profileId: string;
  providerId: string;
  profileRevision: number;
  display: Readonly<{ name: string; description: string }>;
  provenance: Readonly<{
    source: "openagents-trusted" | "official-registry-derived";
    auditRef: string;
    registrySnapshotSha256?: string;
  }>;
  versions: Readonly<{
    supported: ReadonlyArray<AcpVersionRange>;
    experimental: ReadonlyArray<AcpVersionRange>;
    denied: ReadonlyArray<AcpVersionRange>;
  }>;
  launch: Readonly<{
    strategy: AcpExecutableStrategy;
    executable: string;
    args: ReadonlyArray<string>;
    versionProbeArgs: ReadonlyArray<string>;
  }>;
  environment: Readonly<{
    allowedKeys: ReadonlyArray<string>;
    secretRefs: ReadonlyArray<
      Readonly<{ key: string; requirement: "required" | "optional"; secretRef: string }>
    >;
  }>;
  identity: Readonly<{
    expectedExecutableBasename: string;
    expectedAgentName: Readonly<{ kind: "exact" | "prefix"; value: string }>;
    versionExtraction: "leading-semver";
  }>;
  auth: Readonly<{
    policy: "advertised-methods-only";
    methods: ReadonlyArray<
      Readonly<{
        id: string;
        kind: "cached-token" | "api-key-secret" | "interactive-login";
        interaction: "none" | "owner-prompt" | "external-browser";
        secretRefKey?: string;
      }>
    >;
  }>;
  capabilities: ReadonlyArray<Readonly<{ capability: string; state: AcpProfileCapabilityState }>>;
  deviations: ReadonlyArray<Readonly<{ id: string; description: string }>>;
  configuration: Readonly<{
    modes: ReadonlyArray<string>;
    modelConfigOptionIds: ReadonlyArray<string>;
  }>;
  extensions: ReadonlyArray<
    Readonly<{
      method: string;
      direction: AcpProfileExtensionDirection;
      kind: "request" | "notification";
      extensionProfileVersion: number;
    }>
  >;
  sessionPolicy: Readonly<{
    ownership: "single-root-session" | "multi-session";
    restore: "unsupported" | "session-load" | "session-resume";
    cancellation: "session-cancel";
    shutdown: "dispose-process";
  }>;
  evidence: Readonly<{
    fixtureSuites: ReadonlyArray<string>;
    liveMatrixRequired: boolean;
    maxEvidenceAgeDays: number;
  }>;
  platforms: ReadonlyArray<Readonly<{ os: "darwin" | "linux" | "win32"; arch: "arm64" | "x64" }>>;
  install: Readonly<{
    kind: "external" | "npm" | "binary";
    packageName?: string;
    guidance: string;
  }>;
  redaction: Readonly<{ additionalSensitiveKeys: ReadonlyArray<string> }>;
}>;

export type AcpPeerProfileRejectionReason =
  | "malformed_profile"
  | "unsupported_contract_version"
  | "unbounded_value"
  | "invalid_identifier"
  | "unknown_executable_strategy"
  | "shell_string_rejected"
  | "path_traversal_rejected"
  | "undeclared_environment_key"
  | "invalid_version_range"
  | "version_range_conflict"
  | "extension_namespace_collision";

export type AcpPeerProfileRejection = Readonly<{
  _tag: "PeerProfileRejected";
  reason: AcpPeerProfileRejectionReason;
  detail: string;
  path: string;
}>;

export type AcpPeerProfileParseResult =
  | Readonly<{ _tag: "PeerProfileParsed"; profile: AcpTrustedPeerProfile }>
  | AcpPeerProfileRejection;

const LIMITS = {
  identifier: 64,
  shortText: 256,
  longText: 2_000,
  argToken: 512,
  smallList: 16,
  mediumList: 32,
  extensionList: 32,
  capabilityList: 64,
  versionComponent: 999_999,
} as const;

class Rejection extends Error {
  constructor(readonly rejection: AcpPeerProfileRejection) {
    super(rejection.detail);
  }
}

const fail = (reason: AcpPeerProfileRejectionReason, path: string, detail: string): never => {
  throw new Rejection({ _tag: "PeerProfileRejected", reason, path, detail });
};

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const requireObject = (
  value: unknown,
  path: string,
  keys: ReadonlyArray<string>,
): Record<string, unknown> => {
  if (!isObject(value)) fail("malformed_profile", path, "value must be a plain object");
  const record = value as Record<string, unknown>;
  for (const key of Object.keys(record)) {
    if (!keys.includes(key)) {
      fail(
        "malformed_profile",
        `${path}.${key}`,
        "unknown key is not part of the profile contract",
      );
    }
  }
  return record;
};

const requireString = (value: unknown, path: string, max: number, min = 1): string => {
  if (typeof value !== "string") fail("malformed_profile", path, "value must be a string");
  const text = value as string;
  if (text.length < min) fail("malformed_profile", path, "value must not be empty");
  if (text.length > max) {
    fail("unbounded_value", path, `value exceeds the ${String(max)}-character bound`);
  }
  return text;
};

const requireLiteral = <T extends string | number>(
  value: unknown,
  path: string,
  allowed: ReadonlyArray<T>,
  reason: AcpPeerProfileRejectionReason = "malformed_profile",
): T => {
  if (!allowed.includes(value as T)) {
    fail(reason, path, `value must be one of: ${allowed.map(String).join(", ")}`);
  }
  return value as T;
};

const requireArray = (value: unknown, path: string, max: number): ReadonlyArray<unknown> => {
  if (!Array.isArray(value)) fail("malformed_profile", path, "value must be an array");
  const items = value as ReadonlyArray<unknown>;
  if (items.length > max) {
    fail("unbounded_value", path, `array exceeds the ${String(max)}-entry bound`);
  }
  return items;
};

const requireBoolean = (value: unknown, path: string): boolean => {
  if (typeof value !== "boolean") fail("malformed_profile", path, "value must be a boolean");
  return value as boolean;
};

const IDENTIFIER = /^[a-z0-9][a-z0-9-]{0,63}$/;
const ENVIRONMENT_KEY = /^[A-Z][A-Z0-9_]{0,63}$/;
const CAPABILITY_KEY = /^[A-Za-z][A-Za-z0-9_.]{0,127}$/;
const SEMVER = /^(0|[1-9]\d{0,5})\.(0|[1-9]\d{0,5})\.(0|[1-9]\d{0,5})$/;
// Conservative launch-token charset: no whitespace, quoting, expansion,
// redirection, globbing, or command-separator characters can appear at all.
const SAFE_LAUNCH_TOKEN = /^[A-Za-z0-9@%+=:,./_-]+$/;
const SHA256_HEX = /^[a-f0-9]{64}$/;
const NPM_PACKAGE = /^(@[a-z0-9][a-z0-9._-]{0,63}\/)?[a-z0-9][a-z0-9._-]{0,127}$/;

const requireIdentifier = (value: unknown, path: string): string => {
  const text = requireString(value, path, LIMITS.identifier);
  if (!IDENTIFIER.test(text)) {
    fail("invalid_identifier", path, "identifier must match ^[a-z0-9][a-z0-9-]{0,63}$");
  }
  return text;
};

const hasTraversal = (token: string): boolean =>
  token === ".." || token.startsWith("../") || token.endsWith("/..") || token.includes("/../");

const requireLaunchToken = (value: unknown, path: string): string => {
  const text = requireString(value, path, LIMITS.argToken);
  if (!SAFE_LAUNCH_TOKEN.test(text)) {
    fail(
      "shell_string_rejected",
      path,
      "launch token contains characters outside the trusted argv charset",
    );
  }
  if (hasTraversal(text)) {
    fail("path_traversal_rejected", path, "launch token contains a parent-directory traversal");
  }
  return text;
};

const versionTriple = (value: unknown, path: string): readonly [number, number, number] => {
  const text = requireString(value, path, 24);
  const match = SEMVER.exec(text);
  if (match === null) {
    fail(
      "invalid_version_range",
      path,
      "version must be a pinned numeric x.y.z (no *, latest, ^, ~, or partial versions)",
    );
  }
  const parts = (match as RegExpExecArray).slice(1).map(Number) as [number, number, number];
  if (parts.some((part) => part > LIMITS.versionComponent)) {
    fail("unbounded_value", path, "version component exceeds the numeric bound");
  }
  return parts;
};

const compareTriple = (
  a: readonly [number, number, number],
  b: readonly [number, number, number],
): number => a[0] - b[0] || a[1] - b[1] || a[2] - b[2];

type NormalizedRange = Readonly<{
  from: readonly [number, number, number];
  toExclusive: readonly [number, number, number] | null; // null means the single exact version
}>;

const parseVersionRange = (
  value: unknown,
  path: string,
): { range: AcpVersionRange; normalized: NormalizedRange } => {
  const record = requireObject(value, path, ["kind", "version", "fromInclusive", "toExclusive"]);
  const kind = requireLiteral(
    record.kind,
    `${path}.kind`,
    ["exact", "bounded"],
    "invalid_version_range",
  );
  if (kind === "exact") {
    if ("fromInclusive" in record || "toExclusive" in record) {
      fail("invalid_version_range", path, "exact range must not carry bounds");
    }
    const triple = versionTriple(record.version, `${path}.version`);
    return {
      range: { kind: "exact", version: requireString(record.version, `${path}.version`, 24) },
      normalized: { from: triple, toExclusive: null },
    };
  }
  if ("version" in record)
    fail("invalid_version_range", path, "bounded range must not carry version");
  const from = versionTriple(record.fromInclusive, `${path}.fromInclusive`);
  const to = versionTriple(record.toExclusive, `${path}.toExclusive`);
  if (compareTriple(from, to) >= 0) {
    fail("invalid_version_range", path, "fromInclusive must be strictly below toExclusive");
  }
  return {
    range: {
      kind: "bounded",
      fromInclusive: requireString(record.fromInclusive, `${path}.fromInclusive`, 24),
      toExclusive: requireString(record.toExclusive, `${path}.toExclusive`, 24),
    },
    normalized: { from, toExclusive: to },
  };
};

const rangesOverlap = (a: NormalizedRange, b: NormalizedRange): boolean => {
  const aFrom = a.from;
  const aTo = a.toExclusive ?? a.from;
  const bFrom = b.from;
  const bTo = b.toExclusive ?? b.from;
  const aClosed = a.toExclusive === null;
  const bClosed = b.toExclusive === null;
  if (aClosed && bClosed) return compareTriple(aFrom, bFrom) === 0;
  if (aClosed) return compareTriple(aFrom, bFrom) >= 0 && compareTriple(aFrom, bTo) < 0;
  if (bClosed) return compareTriple(bFrom, aFrom) >= 0 && compareTriple(bFrom, aTo) < 0;
  return compareTriple(aFrom, bTo) < 0 && compareTriple(bFrom, aTo) < 0;
};

const RESERVED_METHOD_NAMES: ReadonlySet<string> = new Set(
  [...STABLE_METHOD_MANIFEST.members, ...UNSTABLE_METHOD_MANIFEST.members].map((member) =>
    String(member.method),
  ),
);

const EXTENSION_METHOD = /^_?[a-z][a-z0-9._-]{0,63}\/[A-Za-z][A-Za-z0-9._/-]{0,63}$/;

const freezeDeep = <T>(value: T): T => {
  if (typeof value === "object" && value !== null) {
    for (const child of Object.values(value)) freezeDeep(child);
    Object.freeze(value);
  }
  return value;
};

const TOP_LEVEL_KEYS = [
  "contractVersion",
  "protocol",
  "schemaRelease",
  "wireVersion",
  "profileId",
  "providerId",
  "profileRevision",
  "display",
  "provenance",
  "versions",
  "launch",
  "environment",
  "identity",
  "auth",
  "capabilities",
  "deviations",
  "configuration",
  "extensions",
  "sessionPolicy",
  "evidence",
  "platforms",
  "install",
  "redaction",
] as const;

export const parseAcpTrustedPeerProfile = (input: unknown): AcpPeerProfileParseResult => {
  try {
    const root = requireObject(input, "profile", [...TOP_LEVEL_KEYS]);
    for (const key of TOP_LEVEL_KEYS) {
      if (!(key in root))
        fail("malformed_profile", `profile.${key}`, "required section is missing");
    }
    if (root.contractVersion !== ACP_PEER_PROFILE_CONTRACT_VERSION) {
      fail(
        "unsupported_contract_version",
        "profile.contractVersion",
        `only contract version ${String(ACP_PEER_PROFILE_CONTRACT_VERSION)} is admitted`,
      );
    }
    requireLiteral(root.protocol, "profile.protocol", ["Agent Client Protocol"]);
    requireLiteral(root.schemaRelease, "profile.schemaRelease", ["schema-v1.19.0"]);
    requireLiteral(root.wireVersion, "profile.wireVersion", [1]);
    const profileId = requireIdentifier(root.profileId, "profile.profileId");
    const providerId = requireIdentifier(root.providerId, "profile.providerId");
    if (
      typeof root.profileRevision !== "number" ||
      !Number.isSafeInteger(root.profileRevision) ||
      root.profileRevision < 1 ||
      root.profileRevision > 1_000_000
    ) {
      fail("malformed_profile", "profile.profileRevision", "revision must be a positive integer");
    }

    const display = requireObject(root.display, "profile.display", ["name", "description"]);
    const displayName = requireString(display.name, "profile.display.name", LIMITS.shortText);
    const displayDescription = requireString(
      display.description,
      "profile.display.description",
      LIMITS.longText,
    );

    const provenance = requireObject(root.provenance, "profile.provenance", [
      "source",
      "auditRef",
      "registrySnapshotSha256",
    ]);
    const provenanceSource = requireLiteral(provenance.source, "profile.provenance.source", [
      "openagents-trusted",
      "official-registry-derived",
    ]);
    const auditRef = requireString(
      provenance.auditRef,
      "profile.provenance.auditRef",
      LIMITS.shortText,
    );
    let registrySnapshotSha256: string | undefined;
    if (provenanceSource === "official-registry-derived") {
      const digest = requireString(
        provenance.registrySnapshotSha256,
        "profile.provenance.registrySnapshotSha256",
        64,
      );
      if (!SHA256_HEX.test(digest)) {
        fail(
          "malformed_profile",
          "profile.provenance.registrySnapshotSha256",
          "registry-derived provenance requires a lowercase sha-256 hex digest",
        );
      }
      registrySnapshotSha256 = digest;
    } else if ("registrySnapshotSha256" in provenance) {
      fail(
        "malformed_profile",
        "profile.provenance.registrySnapshotSha256",
        "snapshot digest is only valid for official-registry-derived provenance",
      );
    }

    const versionsRecord = requireObject(root.versions, "profile.versions", [
      "supported",
      "experimental",
      "denied",
    ]);
    const parseRangeLane = (
      lane: "supported" | "experimental" | "denied",
    ): { ranges: ReadonlyArray<AcpVersionRange>; normalized: ReadonlyArray<NormalizedRange> } => {
      const items = requireArray(
        versionsRecord[lane],
        `profile.versions.${lane}`,
        LIMITS.smallList,
      );
      const parsed = items.map((item, index) =>
        parseVersionRange(item, `profile.versions.${lane}[${String(index)}]`),
      );
      return { ranges: parsed.map((p) => p.range), normalized: parsed.map((p) => p.normalized) };
    };
    const supported = parseRangeLane("supported");
    const experimental = parseRangeLane("experimental");
    const denied = parseRangeLane("denied");
    if (supported.ranges.length === 0 && experimental.ranges.length === 0) {
      fail(
        "invalid_version_range",
        "profile.versions",
        "at least one supported or experimental range is required",
      );
    }
    for (const [laneName, lane] of [
      ["supported", supported],
      ["experimental", experimental],
    ] as const) {
      for (const range of lane.normalized) {
        if (denied.normalized.some((deniedRange) => rangesOverlap(range, deniedRange))) {
          fail(
            "version_range_conflict",
            `profile.versions.${laneName}`,
            `${laneName} range overlaps a denied range`,
          );
        }
      }
    }

    const launch = requireObject(root.launch, "profile.launch", [
      "strategy",
      "executable",
      "args",
      "versionProbeArgs",
    ]);
    const strategy = requireLiteral(
      launch.strategy,
      "profile.launch.strategy",
      ["trusted-path-lookup", "absolute-path"],
      "unknown_executable_strategy",
    );
    const executable = requireLaunchToken(launch.executable, "profile.launch.executable");
    if (strategy === "trusted-path-lookup" && executable.includes("/")) {
      fail(
        "path_traversal_rejected",
        "profile.launch.executable",
        "trusted-path-lookup executable must be a bare command name without path separators",
      );
    }
    if (strategy === "absolute-path" && !executable.startsWith("/")) {
      fail(
        "malformed_profile",
        "profile.launch.executable",
        "absolute-path executable must begin with /",
      );
    }
    const args = requireArray(launch.args, "profile.launch.args", LIMITS.smallList).map(
      (item, index) => requireLaunchToken(item, `profile.launch.args[${String(index)}]`),
    );
    const versionProbeArgs = requireArray(
      launch.versionProbeArgs,
      "profile.launch.versionProbeArgs",
      LIMITS.smallList,
    ).map((item, index) =>
      requireLaunchToken(item, `profile.launch.versionProbeArgs[${String(index)}]`),
    );

    const environment = requireObject(root.environment, "profile.environment", [
      "allowedKeys",
      "secretRefs",
    ]);
    const allowedKeys = requireArray(
      environment.allowedKeys,
      "profile.environment.allowedKeys",
      LIMITS.smallList,
    ).map((item, index) => {
      const key = requireString(item, `profile.environment.allowedKeys[${String(index)}]`, 64);
      if (!ENVIRONMENT_KEY.test(key)) {
        fail(
          "invalid_identifier",
          `profile.environment.allowedKeys[${String(index)}]`,
          "environment key must match ^[A-Z][A-Z0-9_]{0,63}$",
        );
      }
      return key;
    });
    if (new Set(allowedKeys).size !== allowedKeys.length) {
      fail(
        "malformed_profile",
        "profile.environment.allowedKeys",
        "environment keys must be unique",
      );
    }
    const secretRefs = requireArray(
      environment.secretRefs,
      "profile.environment.secretRefs",
      LIMITS.smallList,
    ).map((item, index) => {
      const path = `profile.environment.secretRefs[${String(index)}]`;
      const record = requireObject(item, path, ["key", "requirement", "secretRef"]);
      const key = requireString(record.key, `${path}.key`, 64);
      if (!allowedKeys.includes(key)) {
        fail(
          "undeclared_environment_key",
          `${path}.key`,
          "secret reference targets an environment key outside allowedKeys",
        );
      }
      return {
        key,
        requirement: requireLiteral(record.requirement, `${path}.requirement`, [
          "required",
          "optional",
        ]),
        secretRef: requireString(record.secretRef, `${path}.secretRef`, LIMITS.shortText),
      };
    });

    const identity = requireObject(root.identity, "profile.identity", [
      "expectedExecutableBasename",
      "expectedAgentName",
      "versionExtraction",
    ]);
    const expectedExecutableBasename = requireLaunchToken(
      identity.expectedExecutableBasename,
      "profile.identity.expectedExecutableBasename",
    );
    if (expectedExecutableBasename.includes("/")) {
      fail(
        "path_traversal_rejected",
        "profile.identity.expectedExecutableBasename",
        "expected basename must not contain path separators",
      );
    }
    const expectedAgentNameRecord = requireObject(
      identity.expectedAgentName,
      "profile.identity.expectedAgentName",
      ["kind", "value"],
    );
    const expectedAgentName = {
      kind: requireLiteral(
        expectedAgentNameRecord.kind,
        "profile.identity.expectedAgentName.kind",
        ["exact", "prefix"],
      ),
      value: requireString(
        expectedAgentNameRecord.value,
        "profile.identity.expectedAgentName.value",
        LIMITS.shortText,
      ),
    };
    requireLiteral(identity.versionExtraction, "profile.identity.versionExtraction", [
      "leading-semver",
    ]);

    const auth = requireObject(root.auth, "profile.auth", ["policy", "methods"]);
    requireLiteral(auth.policy, "profile.auth.policy", ["advertised-methods-only"]);
    const authMethods = requireArray(auth.methods, "profile.auth.methods", LIMITS.smallList).map(
      (item, index) => {
        const path = `profile.auth.methods[${String(index)}]`;
        const record = requireObject(item, path, ["id", "kind", "interaction", "secretRefKey"]);
        const secretRefKey =
          "secretRefKey" in record
            ? requireString(record.secretRefKey, `${path}.secretRefKey`, 64)
            : undefined;
        if (secretRefKey !== undefined && !allowedKeys.includes(secretRefKey)) {
          fail(
            "undeclared_environment_key",
            `${path}.secretRefKey`,
            "auth secret reference targets an environment key outside allowedKeys",
          );
        }
        return {
          id: requireString(record.id, `${path}.id`, LIMITS.identifier),
          kind: requireLiteral(record.kind, `${path}.kind`, [
            "cached-token",
            "api-key-secret",
            "interactive-login",
          ]),
          interaction: requireLiteral(record.interaction, `${path}.interaction`, [
            "none",
            "owner-prompt",
            "external-browser",
          ]),
          ...(secretRefKey === undefined ? {} : { secretRefKey }),
        };
      },
    );
    if (new Set(authMethods.map((method) => method.id)).size !== authMethods.length) {
      fail("malformed_profile", "profile.auth.methods", "auth method ids must be unique");
    }

    const capabilities = requireArray(
      root.capabilities,
      "profile.capabilities",
      LIMITS.capabilityList,
    ).map((item, index) => {
      const path = `profile.capabilities[${String(index)}]`;
      const record = requireObject(item, path, ["capability", "state"]);
      const capability = requireString(record.capability, `${path}.capability`, 128);
      if (!CAPABILITY_KEY.test(capability)) {
        fail("invalid_identifier", `${path}.capability`, "capability key has an invalid shape");
      }
      return {
        capability,
        state: requireLiteral(record.state, `${path}.state`, [
          "supported",
          "experimental",
          "unsupported",
        ]),
      };
    });
    if (new Set(capabilities.map((entry) => entry.capability)).size !== capabilities.length) {
      fail("malformed_profile", "profile.capabilities", "capability keys must be unique");
    }

    const deviations = requireArray(root.deviations, "profile.deviations", LIMITS.mediumList).map(
      (item, index) => {
        const path = `profile.deviations[${String(index)}]`;
        const record = requireObject(item, path, ["id", "description"]);
        return {
          id: requireIdentifier(record.id, `${path}.id`),
          description: requireString(record.description, `${path}.description`, LIMITS.longText),
        };
      },
    );

    const configuration = requireObject(root.configuration, "profile.configuration", [
      "modes",
      "modelConfigOptionIds",
    ]);
    const modes = requireArray(
      configuration.modes,
      "profile.configuration.modes",
      LIMITS.mediumList,
    ).map((item, index) =>
      requireString(item, `profile.configuration.modes[${String(index)}]`, LIMITS.identifier),
    );
    const modelConfigOptionIds = requireArray(
      configuration.modelConfigOptionIds,
      "profile.configuration.modelConfigOptionIds",
      LIMITS.mediumList,
    ).map((item, index) =>
      requireString(
        item,
        `profile.configuration.modelConfigOptionIds[${String(index)}]`,
        LIMITS.shortText,
      ),
    );

    const extensions = requireArray(
      root.extensions,
      "profile.extensions",
      LIMITS.extensionList,
    ).map((item, index) => {
      const path = `profile.extensions[${String(index)}]`;
      const record = requireObject(item, path, [
        "method",
        "direction",
        "kind",
        "extensionProfileVersion",
      ]);
      const method = requireString(record.method, `${path}.method`, 128);
      if (!EXTENSION_METHOD.test(method)) {
        fail(
          "extension_namespace_collision",
          `${path}.method`,
          "extension method must carry a vendor namespace (namespace/method)",
        );
      }
      if (RESERVED_METHOD_NAMES.has(method)) {
        fail(
          "extension_namespace_collision",
          `${path}.method`,
          "extension method collides with a pinned upstream protocol method",
        );
      }
      const extensionProfileVersion = record.extensionProfileVersion;
      if (
        typeof extensionProfileVersion !== "number" ||
        !Number.isSafeInteger(extensionProfileVersion) ||
        extensionProfileVersion < 1 ||
        extensionProfileVersion > 1_000
      ) {
        fail(
          "malformed_profile",
          `${path}.extensionProfileVersion`,
          "extension profile version must be a positive integer",
        );
      }
      return {
        method,
        direction: requireLiteral(record.direction, `${path}.direction`, [
          "client-to-agent",
          "agent-to-client",
        ]),
        kind: requireLiteral(record.kind, `${path}.kind`, ["request", "notification"]),
        extensionProfileVersion: extensionProfileVersion as number,
      };
    });
    const extensionKeys = extensions.map((entry) => `${entry.direction} ${entry.method}`);
    if (new Set(extensionKeys).size !== extensionKeys.length) {
      fail(
        "extension_namespace_collision",
        "profile.extensions",
        "duplicate extension method/direction pair",
      );
    }

    const sessionPolicy = requireObject(root.sessionPolicy, "profile.sessionPolicy", [
      "ownership",
      "restore",
      "cancellation",
      "shutdown",
    ]);
    const parsedSessionPolicy = {
      ownership: requireLiteral(sessionPolicy.ownership, "profile.sessionPolicy.ownership", [
        "single-root-session",
        "multi-session",
      ]),
      restore: requireLiteral(sessionPolicy.restore, "profile.sessionPolicy.restore", [
        "unsupported",
        "session-load",
        "session-resume",
      ]),
      cancellation: requireLiteral(
        sessionPolicy.cancellation,
        "profile.sessionPolicy.cancellation",
        ["session-cancel"],
      ),
      shutdown: requireLiteral(sessionPolicy.shutdown, "profile.sessionPolicy.shutdown", [
        "dispose-process",
      ]),
    };

    const evidence = requireObject(root.evidence, "profile.evidence", [
      "fixtureSuites",
      "liveMatrixRequired",
      "maxEvidenceAgeDays",
    ]);
    const fixtureSuites = requireArray(
      evidence.fixtureSuites,
      "profile.evidence.fixtureSuites",
      LIMITS.smallList,
    ).map((item, index) =>
      requireString(item, `profile.evidence.fixtureSuites[${String(index)}]`, LIMITS.shortText),
    );
    if (fixtureSuites.length === 0) {
      fail("malformed_profile", "profile.evidence.fixtureSuites", "at least one suite is required");
    }
    const maxEvidenceAgeDays = evidence.maxEvidenceAgeDays;
    if (
      typeof maxEvidenceAgeDays !== "number" ||
      !Number.isSafeInteger(maxEvidenceAgeDays) ||
      maxEvidenceAgeDays < 1 ||
      maxEvidenceAgeDays > 365
    ) {
      fail(
        "malformed_profile",
        "profile.evidence.maxEvidenceAgeDays",
        "evidence freshness must be between 1 and 365 days",
      );
    }
    const parsedEvidence = {
      fixtureSuites,
      liveMatrixRequired: requireBoolean(
        evidence.liveMatrixRequired,
        "profile.evidence.liveMatrixRequired",
      ),
      maxEvidenceAgeDays: maxEvidenceAgeDays as number,
    };

    const platforms = requireArray(root.platforms, "profile.platforms", LIMITS.smallList).map(
      (item, index) => {
        const path = `profile.platforms[${String(index)}]`;
        const record = requireObject(item, path, ["os", "arch"]);
        return {
          os: requireLiteral(record.os, `${path}.os`, ["darwin", "linux", "win32"]),
          arch: requireLiteral(record.arch, `${path}.arch`, ["arm64", "x64"]),
        };
      },
    );
    if (platforms.length === 0) {
      fail("malformed_profile", "profile.platforms", "at least one platform is required");
    }
    if (
      new Set(platforms.map((platform) => `${platform.os}/${platform.arch}`)).size !==
      platforms.length
    ) {
      fail("malformed_profile", "profile.platforms", "platform entries must be unique");
    }

    const install = requireObject(root.install, "profile.install", [
      "kind",
      "packageName",
      "guidance",
    ]);
    const installKind = requireLiteral(install.kind, "profile.install.kind", [
      "external",
      "npm",
      "binary",
    ]);
    let packageName: string | undefined;
    if (installKind === "npm") {
      packageName = requireString(install.packageName, "profile.install.packageName", 214);
      if (!NPM_PACKAGE.test(packageName)) {
        fail("invalid_identifier", "profile.install.packageName", "npm package name is invalid");
      }
    } else if ("packageName" in install) {
      fail(
        "malformed_profile",
        "profile.install.packageName",
        "packageName is only valid for npm installs",
      );
    }
    const guidance = requireString(install.guidance, "profile.install.guidance", LIMITS.longText);

    const redaction = requireObject(root.redaction, "profile.redaction", [
      "additionalSensitiveKeys",
    ]);
    const additionalSensitiveKeys = requireArray(
      redaction.additionalSensitiveKeys,
      "profile.redaction.additionalSensitiveKeys",
      LIMITS.mediumList,
    ).map((item, index) =>
      requireString(
        item,
        `profile.redaction.additionalSensitiveKeys[${String(index)}]`,
        LIMITS.shortText,
      ),
    );

    const profile: AcpTrustedPeerProfile = {
      contractVersion: ACP_PEER_PROFILE_CONTRACT_VERSION,
      protocol: "Agent Client Protocol",
      schemaRelease: "schema-v1.19.0",
      wireVersion: 1,
      profileId,
      providerId,
      profileRevision: root.profileRevision as number,
      display: { name: displayName, description: displayDescription },
      provenance: {
        source: provenanceSource,
        auditRef,
        ...(registrySnapshotSha256 === undefined ? {} : { registrySnapshotSha256 }),
      },
      versions: {
        supported: supported.ranges,
        experimental: experimental.ranges,
        denied: denied.ranges,
      },
      launch: { strategy, executable, args, versionProbeArgs },
      environment: { allowedKeys, secretRefs },
      identity: {
        expectedExecutableBasename,
        expectedAgentName,
        versionExtraction: "leading-semver",
      },
      auth: { policy: "advertised-methods-only", methods: authMethods },
      capabilities,
      deviations,
      configuration: { modes, modelConfigOptionIds },
      extensions,
      sessionPolicy: parsedSessionPolicy,
      evidence: parsedEvidence,
      platforms,
      install: {
        kind: installKind,
        ...(packageName === undefined ? {} : { packageName }),
        guidance,
      },
      redaction: { additionalSensitiveKeys },
    };
    return { _tag: "PeerProfileParsed", profile: freezeDeep(structuredClone(profile)) };
  } catch (error) {
    if (error instanceof Rejection) return error.rejection;
    return {
      _tag: "PeerProfileRejected",
      reason: "malformed_profile",
      path: "profile",
      detail: "profile candidate could not be evaluated",
    };
  }
};
