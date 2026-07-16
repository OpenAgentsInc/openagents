/**
 * Signed Desktop ReleaseSet v2 contract (#8915).
 *
 * ReleaseSet is the complete release-selection authority. Transport, object
 * metadata, pointers, and release mirrors are deliberately outside the trust
 * decision: callers must verify these canonical bytes, then the selected
 * artifact's signed digest and length, before invoking a platform verifier.
 */
import { createHash, createPublicKey, verify as edVerify } from "node:crypto";
import { Exit, Schema } from "effect";
import {
  type PinnedReleaseKey,
  type UpdateChannel,
  type UpdateManifest,
  type UpdateSignature,
  UpdateManifestSchema,
  UpdateSignatureSchema,
  isMonotonicUpgrade,
} from "./update-contract.ts";
import {
  type ReleaseSigningKey,
  deriveReleaseKeyPin,
  signReleasePayload,
} from "./release-publish.ts";
import {
  desktopArtifactFormats,
  desktopTargetKeys,
  type DesktopArtifactFormat,
  type DesktopTargetKey,
} from "./release-staging-contract.ts";

export const RELEASE_SET_SCHEMA_ID = "openagents.desktop.release_set.v2" as const;
export const RELEASE_SET_SCHEMA_VERSION = 2 as const;

export const releaseTargetKeys = desktopTargetKeys;
export type ReleaseTargetKey = DesktopTargetKey;
export const ReleaseTargetKeySchema = Schema.Literals(releaseTargetKeys);

export const macReleaseFormats = ["dmg", "zip"] as const;
export const windowsReleaseFormats = ["nsis"] as const;
export const linuxReleaseFormats = ["appimage", "deb", "rpm"] as const;
export const releaseFormats = desktopArtifactFormats;
export type ReleaseFormat = DesktopArtifactFormat;
export const MacReleaseFormatSchema = Schema.Literals(macReleaseFormats);
export const WindowsReleaseFormatSchema = Schema.Literals(windowsReleaseFormats);
export const LinuxReleaseFormatSchema = Schema.Literals(linuxReleaseFormats);
export const ReleaseFormatSchema = Schema.Literals(releaseFormats);

export const requiredFormatsByTarget: Readonly<Record<ReleaseTargetKey, readonly ReleaseFormat[]>> =
  {
    "darwin-arm64": macReleaseFormats,
    "darwin-x64": macReleaseFormats,
    "win32-arm64": windowsReleaseFormats,
    "win32-x64": windowsReleaseFormats,
    "linux-arm64": linuxReleaseFormats,
    "linux-x64": linuxReleaseFormats,
  };

export const preferredFormatByTarget: Readonly<Record<ReleaseTargetKey, ReleaseFormat>> = {
  "darwin-arm64": "dmg",
  "darwin-x64": "dmg",
  "win32-arm64": "nsis",
  "win32-x64": "nsis",
  "linux-arm64": "appimage",
  "linux-x64": "appimage",
};

export const minimumOsByTarget: Readonly<Record<ReleaseTargetKey, string>> = {
  "darwin-arm64": "13.5",
  "darwin-x64": "13.5",
  "win32-arm64": "10.0.26100",
  "win32-x64": "10.0.19045",
  "linux-arm64": "glibc 2.35",
  "linux-x64": "glibc 2.35",
};

const BoundedRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(240),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/-]*$/),
);
const Sha256 = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/));
const IsoInstant = Schema.String.check(
  Schema.isMinLength(20),
  Schema.isMaxLength(32),
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/),
);
const ArtifactName = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(160),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/),
);
const ImmutableHttpsUrl = Schema.String.check(
  Schema.isMinLength(9),
  Schema.isMaxLength(2_048),
  Schema.isPattern(/^https:\/\/[^\s]+$/),
);
const SourceRevision = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/));
const SemanticVersion = Schema.String.check(
  Schema.isMinLength(5),
  Schema.isMaxLength(40),
  Schema.isPattern(/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.(0|[1-9]\d*))?$/),
);
const OsVersion = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(40),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._ -]*$/),
);

export const ReleaseSetArtifactSchema = Schema.Struct({
  target: ReleaseTargetKeySchema,
  format: ReleaseFormatSchema,
  version: SemanticVersion,
  sourceRevision: SourceRevision,
  name: ArtifactName,
  url: ImmutableHttpsUrl,
  objectIdentity: BoundedRef,
  sha256: Sha256,
  byteLength: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThan(0),
    Schema.isLessThanOrEqualTo(16 * 1_024 * 1_024 * 1_024),
  ),
  componentLedgerSha256: Sha256,
  componentLedgerRef: BoundedRef,
  buildReceiptRef: BoundedRef,
  signingPolicyId: BoundedRef,
});
export type ReleaseSetArtifact = typeof ReleaseSetArtifactSchema.Type;

export const ReleaseSetTargetSchema = Schema.Struct({
  target: ReleaseTargetKeySchema,
  minimumOs: OsVersion,
  preferredFormat: ReleaseFormatSchema,
  artifacts: Schema.Array(ReleaseSetArtifactSchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(3),
  ),
});
export type ReleaseSetTarget = typeof ReleaseSetTargetSchema.Type;

const DigestBoundDocumentSchema = Schema.Struct({ ref: BoundedRef, sha256: Sha256 });
export const ReleaseSetSchema = Schema.Struct({
  schema: Schema.Literal(RELEASE_SET_SCHEMA_ID),
  schemaVersion: Schema.Literal(RELEASE_SET_SCHEMA_VERSION),
  app: Schema.Literal("openagents-desktop"),
  channel: Schema.Literals(["stable", "rc"]),
  version: SemanticVersion,
  sourceRevision: SourceRevision,
  publishedAt: IsoInstant,
  signingPolicy: Schema.Struct({
    id: BoundedRef,
    algorithm: Schema.Literal("ed25519"),
    keyId: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64)),
  }),
  releaseNotes: Schema.Struct({
    summary: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(4_096)),
    human: DigestBoundDocumentSchema,
    agent: DigestBoundDocumentSchema,
  }),
  targets: Schema.Array(ReleaseSetTargetSchema).check(
    Schema.isMinLength(releaseTargetKeys.length),
    Schema.isMaxLength(releaseTargetKeys.length),
  ),
});
export type ReleaseSet = typeof ReleaseSetSchema.Type;

// Compile each Effect decoder once. The exact projection check below then
// makes unknown object keys fail instead of being silently stripped.
const decodeReleaseSetExit = Schema.decodeUnknownExit(ReleaseSetSchema);
const decodeReleaseSetArtifactExit = Schema.decodeUnknownExit(ReleaseSetArtifactSchema);
const decodeUpdateManifestExit = Schema.decodeUnknownExit(UpdateManifestSchema);
const decodeUpdateSignatureExit = Schema.decodeUnknownExit(UpdateSignatureSchema);

const decodeReleaseSetSchema = (value: unknown): ReleaseSet | null => {
  const result = decodeReleaseSetExit(value);
  return Exit.isSuccess(result) ? result.value : null;
};

export const decodeReleaseSetArtifact = (value: unknown): ReleaseSetArtifact | null => {
  const result = decodeReleaseSetArtifactExit(value);
  if (!Exit.isSuccess(result)) return null;
  return canonicalJson(value) === canonicalJson(result.value) ? result.value : null;
};

const decodeUpdateManifestSchema = (value: unknown): UpdateManifest | null => {
  const result = decodeUpdateManifestExit(value);
  return Exit.isSuccess(result) ? result.value : null;
};

const decodeUpdateSignatureSchema = (value: unknown): UpdateSignature | null => {
  const result = decodeUpdateSignatureExit(value);
  return Exit.isSuccess(result) ? result.value : null;
};

const canonicalValue = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .toSorted(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
      .map(([key, child]) => [key, canonicalValue(child)]),
  );
};

export const canonicalJson = (value: unknown): string => JSON.stringify(canonicalValue(value));

const expectedArtifactName = (
  version: string,
  channel: UpdateChannel,
  target: ReleaseTargetKey,
  format: ReleaseFormat,
): string => {
  const [platform, architecture] = target.split("-") as [
    "darwin" | "win32" | "linux",
    "arm64" | "x64",
  ];
  if (platform === "darwin")
    return `OpenAgents-${version}-${channel}-darwin-${architecture}.${format}`;
  if (platform === "win32")
    return `OpenAgents-${version}-${channel}-win32-${architecture}-setup.exe`;
  const extension = format === "appimage" ? "AppImage" : format;
  return `OpenAgents-${version}-${channel}-linux-${architecture}.${extension}`;
};

const semanticProblem = (releaseSet: ReleaseSet): string | null => {
  if (releaseSet.channel === "stable" && releaseSet.version.includes("-rc.")) {
    return "stable_channel_prerelease";
  }
  if (releaseSet.channel === "rc" && !releaseSet.version.includes("-rc."))
    return "rc_channel_stable_version";
  if (releaseSet.targets.map((row) => row.target).join(",") !== releaseTargetKeys.join(",")) {
    return "target_set_incomplete_or_not_canonical";
  }
  const objectIds = new Set<string>();
  const urls = new Set<string>();
  for (const row of releaseSet.targets) {
    if (row.minimumOs !== minimumOsByTarget[row.target]) return "minimum_os_policy_conflict";
    if (row.preferredFormat !== preferredFormatByTarget[row.target])
      return "preferred_format_invalid";
    const required = requiredFormatsByTarget[row.target];
    if (row.artifacts.map((artifact) => artifact.format).join(",") !== required.join(",")) {
      return "format_set_incomplete_or_not_canonical";
    }
    for (const artifact of row.artifacts) {
      if (artifact.target !== row.target) return "artifact_target_conflict";
      if (artifact.version !== releaseSet.version) return "artifact_version_conflict";
      if (artifact.sourceRevision !== releaseSet.sourceRevision) return "artifact_source_conflict";
      if (artifact.signingPolicyId !== releaseSet.signingPolicy.id)
        return "signing_policy_conflict";
      if (
        artifact.name !==
        expectedArtifactName(releaseSet.version, releaseSet.channel, row.target, artifact.format)
      ) {
        return "artifact_identity_invalid";
      }
      let url: URL;
      try {
        url = new URL(artifact.url);
      } catch {
        return "artifact_url_invalid";
      }
      if (
        url.protocol !== "https:" ||
        url.username !== "" ||
        url.password !== "" ||
        url.search !== "" ||
        url.hash !== ""
      ) {
        return "artifact_url_invalid";
      }
      try {
        if (decodeURIComponent(url.pathname.split("/").at(-1) ?? "") !== artifact.name)
          return "artifact_url_name_mismatch";
      } catch {
        return "artifact_url_invalid";
      }
      if (objectIds.has(artifact.objectIdentity) || urls.has(artifact.url))
        return "duplicate_artifact_identity";
      objectIds.add(artifact.objectIdentity);
      urls.add(artifact.url);
    }
  }
  return null;
};

export type DecodeReleaseSetResult =
  | { readonly ok: true; readonly releaseSet: ReleaseSet }
  | {
      readonly ok: false;
      readonly reason: "schema_invalid" | "unknown_field" | "semantic_invalid";
      readonly detail?: string;
    };

export const decodeReleaseSet = (value: unknown): DecodeReleaseSetResult => {
  const decoded = decodeReleaseSetSchema(value);
  if (decoded === null) return { ok: false, reason: "schema_invalid" };
  if (canonicalJson(value) !== canonicalJson(decoded))
    return { ok: false, reason: "unknown_field" };
  const problem = semanticProblem(decoded);
  return problem === null
    ? { ok: true, releaseSet: decoded }
    : { ok: false, reason: "semantic_invalid", detail: problem };
};

export const canonicalizeReleaseSet = (value: unknown): Uint8Array => {
  const decoded = decodeReleaseSet(value);
  if (!decoded.ok)
    throw new Error(
      `ReleaseSet rejected: ${decoded.reason}${decoded.detail === undefined ? "" : `:${decoded.detail}`}`,
    );
  return new TextEncoder().encode(canonicalJson(decoded.releaseSet));
};

export const releaseSetVerificationFailures = [
  "malformed_signature_envelope",
  "kid_not_pinned",
  "payload_sha256_mismatch",
  "signature_invalid",
  "payload_not_canonical",
  "release_set_invalid",
  "channel_mismatch",
  "signing_policy_mismatch",
] as const;
export type ReleaseSetVerificationFailure = (typeof releaseSetVerificationFailures)[number];
export type ReleaseSetVerificationResult =
  | { readonly ok: true; readonly releaseSet: ReleaseSet }
  | { readonly ok: false; readonly reason: ReleaseSetVerificationFailure };

export const verifySignedReleaseSet = (
  payloadBytes: Uint8Array,
  signatureValue: unknown,
  pin: PinnedReleaseKey,
  expectedChannel: UpdateChannel,
): ReleaseSetVerificationResult => {
  const envelope = decodeUpdateSignatureSchema(signatureValue);
  if (envelope === null || envelope.alg !== "ed25519" || pin.alg !== "ed25519") {
    return { ok: false, reason: "malformed_signature_envelope" };
  }
  if (envelope.kid !== pin.kid) return { ok: false, reason: "kid_not_pinned" };
  if (createHash("sha256").update(payloadBytes).digest("hex") !== envelope.sha256) {
    return { ok: false, reason: "payload_sha256_mismatch" };
  }
  try {
    const publicKey = createPublicKey({
      key: { kty: "OKP", crv: "Ed25519", x: pin.x },
      format: "jwk",
    });
    if (!edVerify(null, payloadBytes, publicKey, Buffer.from(envelope.signature, "base64url"))) {
      return { ok: false, reason: "signature_invalid" };
    }
  } catch {
    return { ok: false, reason: "signature_invalid" };
  }
  let raw: unknown;
  try {
    raw = JSON.parse(new TextDecoder().decode(payloadBytes));
  } catch {
    return { ok: false, reason: "release_set_invalid" };
  }
  const decoded = decodeReleaseSet(raw);
  if (!decoded.ok) return { ok: false, reason: "release_set_invalid" };
  const canonical = canonicalizeReleaseSet(decoded.releaseSet);
  if (!Buffer.from(canonical).equals(Buffer.from(payloadBytes)))
    return { ok: false, reason: "payload_not_canonical" };
  if (decoded.releaseSet.channel !== expectedChannel)
    return { ok: false, reason: "channel_mismatch" };
  if (decoded.releaseSet.signingPolicy.keyId !== pin.kid)
    return { ok: false, reason: "signing_policy_mismatch" };
  return { ok: true, releaseSet: decoded.releaseSet };
};

export const verifyReleaseSetArtifact = (
  artifact: ReleaseSetArtifact,
  bytes: Uint8Array,
): boolean =>
  bytes.byteLength === artifact.byteLength &&
  createHash("sha256").update(bytes).digest("hex") === artifact.sha256;

export type FinalizeReleaseSetResult = Readonly<{
  releaseSet: ReleaseSet;
  payloadBytes: Uint8Array;
  envelope: UpdateSignature;
  pin: PinnedReleaseKey;
}>;

/** Complete-set finalizer: no partial or unverified matrix can reach signing. */
export const finalizeReleaseSet = (
  input: Readonly<{
    candidate: unknown;
    artifactBytesByObjectIdentity: ReadonlyMap<string, Uint8Array>;
    currentReleaseSet?: unknown;
    key: ReleaseSigningKey;
  }>,
): FinalizeReleaseSetResult => {
  const decoded = decodeReleaseSet(input.candidate);
  if (!decoded.ok)
    throw new Error(
      `finalize refused: ${decoded.reason}${decoded.detail === undefined ? "" : `:${decoded.detail}`}`,
    );
  const releaseSet = decoded.releaseSet;
  if (input.currentReleaseSet !== undefined) {
    const current = decodeReleaseSet(input.currentReleaseSet);
    if (!current.ok || current.releaseSet.channel !== releaseSet.channel)
      throw new Error("finalize refused: current_release_set_invalid");
    const monotonic = isMonotonicUpgrade(
      current.releaseSet.version,
      releaseSet.version,
      releaseSet.channel,
    );
    if (!monotonic.admissible) throw new Error(`finalize refused: ${monotonic.reason}`);
  }
  const artifacts = releaseSet.targets.flatMap((row) => row.artifacts);
  if (input.artifactBytesByObjectIdentity.size !== artifacts.length)
    throw new Error("finalize refused: artifact_byte_set_incomplete");
  for (const artifact of artifacts) {
    const bytes = input.artifactBytesByObjectIdentity.get(artifact.objectIdentity);
    if (bytes === undefined || !verifyReleaseSetArtifact(artifact, bytes)) {
      throw new Error(`finalize refused: artifact_verification_failed:${artifact.objectIdentity}`);
    }
  }
  const payloadBytes = canonicalizeReleaseSet(releaseSet);
  if (releaseSet.signingPolicy.keyId !== deriveReleaseKeyPin(input.key).kid)
    throw new Error("finalize refused: signing_key_policy_mismatch");
  const signed = signReleasePayload(payloadBytes, input.key);
  const verified = verifySignedReleaseSet(
    payloadBytes,
    signed.envelope,
    signed.pin,
    releaseSet.channel,
  );
  if (!verified.ok) throw new Error(`finalize self-verification failed: ${verified.reason}`);
  return { releaseSet, payloadBytes, envelope: signed.envelope, pin: signed.pin };
};

export type HostPlatform = "darwin" | "win32" | "linux";
export type HostArchitecture = "arm64" | "x64";
export type ApplicationArchitecture = HostArchitecture;
const numericVersionParts = (value: string): readonly number[] =>
  (value.match(/\d+/g) ?? []).map(Number);

export const selectReleaseArtifact = (
  input: Readonly<{
    releaseSet: ReleaseSet;
    installedChannel: UpdateChannel;
    installedVersion: string;
    platform: HostPlatform;
    architecture: HostArchitecture;
    /** Architecture of the running executable; distinct under Rosetta. */
    applicationArchitecture?: ApplicationArchitecture;
    hostVersion: string;
  }>,
):
  | {
      readonly ok: true;
      readonly target: ReleaseTargetKey;
      readonly artifact: ReleaseSetArtifact;
      readonly transition: "same_architecture" | "full_artifact_architecture_migration";
    }
  | {
      readonly ok: false;
      readonly reason:
        | "channel_mismatch"
        | "not_monotonic"
        | "target_unavailable"
        | "minimum_os_not_met"
        | "ambiguous_target"
        | "preferred_format_invalid";
    } => {
  if (input.releaseSet.channel !== input.installedChannel)
    return { ok: false, reason: "channel_mismatch" };
  if (
    !isMonotonicUpgrade(input.installedVersion, input.releaseSet.version, input.installedChannel)
      .admissible
  ) {
    return { ok: false, reason: "not_monotonic" };
  }
  const target = `${input.platform}-${input.architecture}` as ReleaseTargetKey;
  const rows = input.releaseSet.targets.filter((candidate) => candidate.target === target);
  if (rows.length > 1) return { ok: false, reason: "ambiguous_target" };
  const row = rows[0];
  if (row === undefined) return { ok: false, reason: "target_unavailable" };
  if (row.preferredFormat !== preferredFormatByTarget[target]) {
    return { ok: false, reason: "preferred_format_invalid" };
  }
  const artifacts = row.artifacts.filter((candidate) => candidate.format === row.preferredFormat);
  if (artifacts.length > 1) return { ok: false, reason: "ambiguous_target" };
  const artifact = artifacts[0];
  if (artifact === undefined || artifact.target !== target)
    return { ok: false, reason: "target_unavailable" };
  const minimum = numericVersionParts(row.minimumOs);
  const host = numericVersionParts(input.hostVersion);
  for (let index = 0; index < Math.max(minimum.length, host.length); index += 1) {
    const hostPart = host[index] ?? 0;
    const minimumPart = minimum[index] ?? 0;
    if (hostPart > minimumPart) break;
    if (hostPart < minimumPart) return { ok: false, reason: "minimum_os_not_met" };
  }
  return {
    ok: true,
    target,
    artifact,
    transition:
      input.applicationArchitecture !== undefined &&
      input.applicationArchitecture !== input.architecture
        ? "full_artifact_architecture_migration"
        : "same_architecture",
  };
};

/**
 * Bounded v1 migration: only the historical macOS arm64 single-artifact
 * contract is readable, through 2026-10-14 inclusive. It is returned as v1,
 * never projected into or silently reinterpreted as ReleaseSet v2. New
 * targets and all new publication remain v2-only.
 */
export const V1_MIGRATION_END = "2026-10-14T23:59:59Z" as const;
export type ReleaseSelection =
  | { readonly kind: "v2"; readonly releaseSet: ReleaseSet }
  | { readonly kind: "v1-darwin-arm64"; readonly manifest: UpdateManifest };

export const decodeReleaseSelection = (
  value: unknown,
  observedAt: string,
): ReleaseSelection | null => {
  const v2 = decodeReleaseSet(value);
  if (v2.ok) return { kind: "v2", releaseSet: v2.releaseSet };
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/.test(observedAt)) return null;
  if (observedAt > V1_MIGRATION_END) return null;
  const v1 = decodeUpdateManifestSchema(value);
  if (v1 === null || !/-(?:arm64)\.(?:dmg|zip)$/.test(v1.artifactName)) return null;
  return { kind: "v1-darwin-arm64", manifest: v1 };
};
