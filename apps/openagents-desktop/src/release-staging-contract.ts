/**
 * Target-aware Desktop staging contracts (DIST-03, #8916).
 *
 * One typed home for the three machine-readable documents the cross-platform
 * release program (docs/deploy/openagents-desktop-cross-platform-release.md,
 * §§3, 6, 9–10) requires from every target build worker:
 *
 *  1. `DesktopTargetBuildDescriptorSchema` — the explicit target build
 *     descriptor every packaging entrypoint (local or remote) must receive.
 *     Host inference (`process.platform`/`process.arch`) never selects a
 *     release target.
 *  2. `NativeComponentLedgerSchema` — the public-safe per-target native
 *     component ledger (name, version, target, digest, provenance,
 *     destination). Its identity is `nativeComponentLedgerDigest`, computed
 *     over canonical JSON so repeat staging from the same inputs produces the
 *     same ledger identity.
 *  3. `DesktopBuildReceiptSchema` — the build receipt binding source
 *     revision, version, lockfile identity, Electron/Node/pnpm versions, the
 *     target descriptor, the component-ledger reference, artifact inputs,
 *     and worker identity.
 *
 * Integration point for ReleaseSet v2 (#8915): the v2 manifest carries ONLY
 * the opaque reference strings produced here — `nativeComponentLedgerRef`
 * and `desktopBuildReceiptRef` (both `sha256:<hex>` over canonical JSON).
 * The finalizer imports this module to re-derive and compare those refs; it
 * never re-defines the ledger/receipt shapes.
 *
 * Determinism boundary (documented exception): ledger digests are computed
 * over the STAGED, pre-signature closure and are reproducible from the same
 * inputs. Outer artifact digests inside the receipt are NOT reproducible
 * across signing runs — Apple/Windows signature and notarization bytes are
 * nondeterministic by design. Repeat-staging oracles therefore compare
 * ledger identities, never signed-artifact digests.
 *
 * Unsigned development output is structurally inadmissible here: a receipt
 * cannot be constructed for a descriptor whose signing policy is
 * `unsigned-dev`, and any artifact name carrying the `-UNSIGNED-DEV` marker
 * fails the receipt schema. Public-safety: no field admits an absolute path,
 * hostname, credential, or free-form log text; every ref is bounded.
 */
import { createHash } from "node:crypto";
import { Schema } from "effect";

import {
  ReleaseVersionSchema,
  UpdateChannelSchema,
  parseReleaseVersion,
} from "./update-contract.ts";

export const TARGET_BUILD_DESCRIPTOR_SCHEMA_ID =
  "openagents.desktop.target_build_descriptor.v1" as const;
export const NATIVE_COMPONENT_LEDGER_SCHEMA_ID =
  "openagents.desktop.native_component_ledger.v1" as const;
export const BUILD_RECEIPT_SCHEMA_ID = "openagents.desktop.build_receipt.v1" as const;

// ---------------------------------------------------------------------------
// Target keys — the closed six-target enum from epic #8913 / ProductSpec §3
// ---------------------------------------------------------------------------

export const desktopTargetKeys = [
  "darwin-arm64",
  "darwin-x64",
  "win32-arm64",
  "win32-x64",
  "linux-arm64",
  "linux-x64",
] as const;
export type DesktopTargetKey = (typeof desktopTargetKeys)[number];
export const DesktopTargetKeySchema = Schema.Literals(desktopTargetKeys);

export const desktopTargetPlatforms = ["darwin", "win32", "linux"] as const;
export type DesktopTargetPlatform = (typeof desktopTargetPlatforms)[number];
export const desktopTargetArchitectures = ["arm64", "x64"] as const;
export type DesktopTargetArchitecture = (typeof desktopTargetArchitectures)[number];

export const desktopArtifactFormats = ["dmg", "zip", "nsis", "appimage", "deb", "rpm"] as const;
export type DesktopArtifactFormat = (typeof desktopArtifactFormats)[number];
export const DesktopArtifactFormatSchema = Schema.Literals(desktopArtifactFormats);

export interface DesktopTargetDefinition {
  readonly platform: DesktopTargetPlatform;
  readonly arch: DesktopTargetArchitecture;
  /** Explicit Rust target triple for owned native components (oa-desktop-audio). */
  readonly rustTargetTriple: string;
  /** Required package formats per ProductSpec §4. */
  readonly requiredFormats: ReadonlyArray<DesktopArtifactFormat>;
}

/**
 * The single authority mapping a target key to platform, architecture, the
 * explicit Rust triple, and the required formats. Workers consume this map;
 * they never infer any of it from the host.
 */
export const desktopTargets: Readonly<Record<DesktopTargetKey, DesktopTargetDefinition>> = {
  "darwin-arm64": {
    platform: "darwin",
    arch: "arm64",
    rustTargetTriple: "aarch64-apple-darwin",
    requiredFormats: ["dmg", "zip"],
  },
  "darwin-x64": {
    platform: "darwin",
    arch: "x64",
    rustTargetTriple: "x86_64-apple-darwin",
    requiredFormats: ["dmg", "zip"],
  },
  "win32-arm64": {
    platform: "win32",
    arch: "arm64",
    rustTargetTriple: "aarch64-pc-windows-msvc",
    requiredFormats: ["nsis"],
  },
  "win32-x64": {
    platform: "win32",
    arch: "x64",
    rustTargetTriple: "x86_64-pc-windows-msvc",
    requiredFormats: ["nsis"],
  },
  "linux-arm64": {
    platform: "linux",
    arch: "arm64",
    rustTargetTriple: "aarch64-unknown-linux-gnu",
    requiredFormats: ["appimage", "deb", "rpm"],
  },
  "linux-x64": {
    platform: "linux",
    arch: "x64",
    rustTargetTriple: "x86_64-unknown-linux-gnu",
    requiredFormats: ["appimage", "deb", "rpm"],
  },
};

// ---------------------------------------------------------------------------
// Bounded scalar schemas
// ---------------------------------------------------------------------------

export const Sha256HexSchema = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/));
export const GitRevisionSchema = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/));

/** Public-safe ref charset (mirrors the update-contract/diagnostics pin). */
const PublicRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(120),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);

/** Bounded artifact file name — never a path (no separators, no traversal). */
const ArtifactNameSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(120),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/),
);

/**
 * Public-safe RELATIVE destination inside the staged bundle. Rejects
 * absolute paths, drive letters, traversal, and backslashes so a ledger can
 * never leak a checkout or worker filesystem location.
 */
const BundleRelativePathSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(512),
  Schema.isPattern(/^(?![A-Za-z]:)(?!\/)(?!.*\\)(?!(?:.*\/)?\.\.(?:\/|$))[^\0\r\n]+$/u),
);

const Iso8601UtcSchema = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
);

const BoundedVersionStringSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(60),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._+-]*$/),
);

// ---------------------------------------------------------------------------
// Target build descriptor
// ---------------------------------------------------------------------------

export const desktopSigningPolicies = ["production", "unsigned-dev"] as const;
export type DesktopSigningPolicy = (typeof desktopSigningPolicies)[number];
export const DesktopSigningPolicySchema = Schema.Literals(desktopSigningPolicies);

const descriptorFields = {
  schema: Schema.Literal(TARGET_BUILD_DESCRIPTOR_SCHEMA_ID),
  product: Schema.Literal("OpenAgents"),
  targetKey: DesktopTargetKeySchema,
  channel: UpdateChannelSchema,
  version: ReleaseVersionSchema,
  sourceRevision: GitRevisionSchema,
  /** sha256 of the exact pnpm lockfile the worker must install from. */
  lockfileSha256: Sha256HexSchema,
  formats: Schema.Array(DesktopArtifactFormatSchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(6),
  ),
  signingPolicy: DesktopSigningPolicySchema,
  /** Optional reproducibility/build-invocation ref supplied by the coordinator. */
  invocationRef: Schema.optionalKey(PublicRefSchema),
};

export const DesktopTargetBuildDescriptorSchema = Schema.Struct(descriptorFields).check(
  Schema.makeFilter(
    (descriptor) => {
      const definition = desktopTargets[descriptor.targetKey];
      const unique = new Set(descriptor.formats);
      if (unique.size !== descriptor.formats.length) return "duplicate artifact formats";
      for (const format of descriptor.formats) {
        if (!definition.requiredFormats.includes(format)) {
          return `format ${format} is not defined for target ${descriptor.targetKey}`;
        }
      }
      const parsed = parseReleaseVersion(descriptor.version);
      if (parsed === null) return "unparseable version";
      if (descriptor.channel === "stable" && parsed.rc !== null) {
        return "stable channel rejects pre-release versions";
      }
      if (descriptor.channel === "rc" && parsed.rc === null) {
        return "rc channel requires an -rc.N pre-release version";
      }
      return undefined;
    },
    { title: "coherent target build descriptor" },
  ),
);
export type DesktopTargetBuildDescriptor = typeof DesktopTargetBuildDescriptorSchema.Type;

export const decodeDesktopTargetBuildDescriptor = (value: unknown): DesktopTargetBuildDescriptor =>
  Schema.decodeUnknownSync(DesktopTargetBuildDescriptorSchema)(value);

// ---------------------------------------------------------------------------
// Version-first immutable artifact names (ProductSpec §6)
// ---------------------------------------------------------------------------

const artifactFormatSuffixes: Readonly<Record<DesktopArtifactFormat, string>> = {
  dmg: ".dmg",
  zip: ".zip",
  nsis: "-setup.exe",
  appimage: ".AppImage",
  deb: ".deb",
  rpm: ".rpm",
};

export interface DesktopReleaseSetArtifactNameInput {
  readonly version: string;
  readonly channel: string;
  readonly targetKey: DesktopTargetKey;
  readonly format: DesktopArtifactFormat;
}

/**
 * Canonical, version-first, immutable ReleaseSet v2 artifact basename:
 * `OpenAgents-<version>-<channel>-<platform>-<arch><format suffix>` —
 * exactly the ProductSpec §6 name table. The v1 macOS basename produced by
 * `desktopReleaseArtifactName` remains a bounded migration input only.
 */
export const desktopReleaseSetArtifactName = (
  input: DesktopReleaseSetArtifactNameInput,
): string => {
  const safeSegment = /^[A-Za-z0-9][A-Za-z0-9._-]*$/u;
  if (!safeSegment.test(input.version)) throw new Error("Invalid release artifact version");
  if (input.channel !== "stable" && input.channel !== "rc") {
    throw new Error("Invalid release artifact channel");
  }
  const definition = desktopTargets[input.targetKey];
  if (definition === undefined) throw new Error("Invalid release artifact target");
  if (!definition.requiredFormats.includes(input.format)) {
    throw new Error(`Format ${input.format} is not defined for target ${input.targetKey}`);
  }
  const suffix = artifactFormatSuffixes[input.format];
  return `OpenAgents-${input.version}-${input.channel}-${definition.platform}-${definition.arch}${suffix}`;
};

// ---------------------------------------------------------------------------
// Native component ledger
// ---------------------------------------------------------------------------

export const nativeComponentProvenances = [
  /** Built in-tree from an owned workspace crate (e.g. oa-desktop-audio). */
  "workspace-crate",
  /** Copied from the exact lockfile-pinned dependency for the target. */
  "locked-dependency",
  /** A checked-in application resource copied through the build allowlist. */
  "application-resource",
] as const;
export type NativeComponentProvenance = (typeof nativeComponentProvenances)[number];
export const NativeComponentProvenanceSchema = Schema.Literals(nativeComponentProvenances);

export const NativeComponentLedgerEntrySchema = Schema.Struct({
  /** Public component name (package name or owned component name). */
  name: Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(214),
    Schema.isPattern(/^[@A-Za-z0-9][@A-Za-z0-9._/-]*$/),
  ),
  version: BoundedVersionStringSchema,
  targetKey: DesktopTargetKeySchema,
  sha256: Sha256HexSchema,
  byteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  provenance: NativeComponentProvenanceSchema,
  /** Relative destination inside the staged bundle — never an absolute path. */
  destination: BundleRelativePathSchema,
});
export type NativeComponentLedgerEntry = typeof NativeComponentLedgerEntrySchema.Type;

export const NativeComponentLedgerSchema = Schema.Struct({
  schema: Schema.Literal(NATIVE_COMPONENT_LEDGER_SCHEMA_ID),
  targetKey: DesktopTargetKeySchema,
  channel: UpdateChannelSchema,
  version: ReleaseVersionSchema,
  sourceRevision: GitRevisionSchema,
  components: Schema.Array(NativeComponentLedgerEntrySchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(256),
  ),
}).check(
  Schema.makeFilter(
    (ledger) => {
      const destinations = new Set<string>();
      for (const component of ledger.components) {
        if (component.targetKey !== ledger.targetKey) {
          return `component ${component.name} targets ${component.targetKey}, ledger targets ${ledger.targetKey}`;
        }
        if (destinations.has(component.destination)) {
          return `duplicate component destination ${component.destination}`;
        }
        destinations.add(component.destination);
      }
      return undefined;
    },
    { title: "single-target ledger with unique destinations" },
  ),
);
export type NativeComponentLedger = typeof NativeComponentLedgerSchema.Type;

export const decodeNativeComponentLedger = (value: unknown): NativeComponentLedger =>
  Schema.decodeUnknownSync(NativeComponentLedgerSchema)(value);

// ---------------------------------------------------------------------------
// Canonical JSON + reference digests
// ---------------------------------------------------------------------------

const canonicalize = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === "object") {
    const record = value as Record<string, unknown>;
    const sorted: Record<string, unknown> = {};
    for (const key of Object.keys(record).sort()) sorted[key] = canonicalize(record[key]);
    return sorted;
  }
  return value;
};

/** Stable canonical JSON: deep-sorted object keys, no insertion-order drift. */
export const canonicalJson = (value: unknown): string => JSON.stringify(canonicalize(value));

const sha256Hex = (text: string): string => createHash("sha256").update(text, "utf8").digest("hex");

/**
 * Deterministic ledger identity: sha256 over canonical JSON with components
 * ordered by destination. Repeat staging from the same inputs MUST reproduce
 * this digest bit-for-bit (the pre-signature closure is deterministic; only
 * signer/notary bytes are exempt, and they never enter the ledger).
 */
export const nativeComponentLedgerDigest = (ledger: NativeComponentLedger): string =>
  sha256Hex(
    canonicalJson({
      ...ledger,
      components: [...ledger.components].sort((a, b) => a.destination.localeCompare(b.destination)),
    }),
  );

export const nativeComponentLedgerRef = (ledger: NativeComponentLedger): string =>
  `sha256:${nativeComponentLedgerDigest(ledger)}`;

// ---------------------------------------------------------------------------
// Build receipt
// ---------------------------------------------------------------------------

export const DesktopBuildArtifactInputSchema = Schema.Struct({
  name: ArtifactNameSchema,
  format: DesktopArtifactFormatSchema,
  sha256: Sha256HexSchema,
  byteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
});
export type DesktopBuildArtifactInput = typeof DesktopBuildArtifactInputSchema.Type;

export const DesktopBuildReceiptSchema = Schema.Struct({
  schema: Schema.Literal(BUILD_RECEIPT_SCHEMA_ID),
  descriptor: DesktopTargetBuildDescriptorSchema,
  componentLedger: Schema.Struct({
    sha256: Sha256HexSchema,
    componentCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  }),
  toolchain: Schema.Struct({
    electron: BoundedVersionStringSchema,
    node: BoundedVersionStringSchema,
    pnpm: BoundedVersionStringSchema,
  }),
  artifacts: Schema.Array(DesktopBuildArtifactInputSchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(6),
  ),
  worker: Schema.Struct({
    /** Opaque owned-runner ref — never a hostname, address, or path. */
    workerRef: PublicRefSchema,
    /** Public-safe host identity class (e.g. `owned-mac-arm64`). */
    hostClass: PublicRefSchema,
  }),
  completedAt: Iso8601UtcSchema,
}).check(
  Schema.makeFilter(
    (receipt) => {
      // Unsigned development output is structurally inadmissible: no receipt.
      if (receipt.descriptor.signingPolicy !== "production") {
        return "unsigned-dev builds are ineligible for build receipts";
      }
      const expected = new Set(receipt.descriptor.formats);
      const seen = new Set<string>();
      for (const artifact of receipt.artifacts) {
        if (artifact.name.includes("UNSIGNED-DEV")) {
          return `artifact ${artifact.name} carries the UNSIGNED-DEV marker`;
        }
        if (!expected.has(artifact.format)) {
          return `artifact format ${artifact.format} is not in the descriptor format set`;
        }
        if (seen.has(artifact.format)) return `duplicate artifact format ${artifact.format}`;
        seen.add(artifact.format);
        const canonical = desktopReleaseSetArtifactName({
          version: receipt.descriptor.version,
          channel: receipt.descriptor.channel,
          targetKey: receipt.descriptor.targetKey,
          format: artifact.format,
        });
        if (artifact.name !== canonical) {
          return `artifact ${artifact.name} is not the canonical name ${canonical}`;
        }
      }
      if (seen.size !== expected.size) {
        return "receipt artifacts do not cover every descriptor format";
      }
      return undefined;
    },
    { title: "production receipt with canonical, complete artifact set" },
  ),
);
export type DesktopBuildReceipt = typeof DesktopBuildReceiptSchema.Type;

export const decodeDesktopBuildReceipt = (value: unknown): DesktopBuildReceipt =>
  Schema.decodeUnknownSync(DesktopBuildReceiptSchema)(value);

/**
 * Opaque receipt reference for ReleaseSet v2 (#8915): `sha256:<hex>` over the
 * receipt's canonical JSON. Signed-artifact digests inside the receipt make
 * this ref build-run-specific; cross-run determinism claims belong to the
 * ledger ref, never the receipt ref.
 */
export const desktopBuildReceiptRef = (receipt: DesktopBuildReceipt): string =>
  `sha256:${sha256Hex(canonicalJson(receipt))}`;
