import { Result, Schema as S } from "effect";

import {
  IdeProjectionGeneration,
  IdeProjectionRef,
  IdeProjectionTimestamp,
  hasForbiddenIdeProjectionMaterial,
} from "./ide-review-projection.js";

export const IDE_CODE_SHARE_BUNDLE_SCHEMA_LITERAL = "openagents.ide_code_share_bundle.v1" as const;

export const MAX_CODE_SHARE_ENTRIES = 500 as const;
export const MAX_CODE_SHARE_OMISSIONS = 2_000 as const;
export const MAX_CODE_SHARE_ENTRY_BYTES = 262_144 as const;
export const MAX_CODE_SHARE_TOTAL_BYTES = 8_388_608 as const;
export const MAX_CODE_SHARE_PATH_CHARS = 512 as const;
export const MAX_CODE_SHARE_RETENTION_DAYS = 365 as const;

const forbiddenBundleText = [
  /(?:^|[\s"'(=])\/(?:Users|home|root|private|var|etc|opt|tmp|workspace|mnt|srv|data|run)\//i,
  /(?:^|[\s"'(=])[a-z]:\\/i,
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/i,
  /\bAKIA[0-9A-Z]{16}\b/,
  /\b(?:ghp_|github_pat_|sk-)[A-Za-z0-9_-]{16,}\b/i,
  /\b(?:api[_-]?key|password|passwd|secret|token)\s*[:=]\s*\S+/i,
] as const;

const containsForbiddenBundleText = (value: string): boolean =>
  forbiddenBundleText.some((pattern) => pattern.test(value));

/**
 * A public-safe relative path label. It must not begin at a filesystem root,
 * carry a drive prefix, walk up with `..`, or hide with a leading dot segment.
 * The bundle exposes only allowlisted relative paths, never a host root.
 */
export const CodeSharePathLabel = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(MAX_CODE_SHARE_PATH_CHARS),
  S.makeFilter((value) => !value.startsWith("/") && !value.startsWith("\\"), {
    message: "path must be relative, not rooted",
  }),
  S.makeFilter((value) => !/^[a-z]:[\\/]/i.test(value), {
    message: "path must not carry a drive prefix",
  }),
  S.makeFilter((value) => !value.split(/[\\/]/).includes(".."), {
    message: "path must not contain a parent-directory segment",
  }),
  S.makeFilter((value) => !value.split(/[\\/]/).some((segment) => segment.startsWith(".")), {
    message: "path must not expose a hidden segment",
  }),
  S.makeFilter((value) => !containsForbiddenBundleText(value), {
    message: "path must not contain host or credential material",
  }),
).pipe(S.brand("CodeSharePathLabel"));
export type CodeSharePathLabel = typeof CodeSharePathLabel.Type;

/** A lowercase hex SHA-256 content digest. */
export const CodeShareDigest = S.String.check(S.isPattern(/^[0-9a-f]{64}$/)).pipe(
  S.brand("CodeShareDigest"),
);
export type CodeShareDigest = typeof CodeShareDigest.Type;

const NonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));
const PositiveRetentionDays = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(MAX_CODE_SHARE_RETENTION_DAYS),
);

const BoundedEntryText = S.String.check(
  S.isMaxLength(MAX_CODE_SHARE_ENTRY_BYTES),
  S.makeFilter((value) => !containsForbiddenBundleText(value), {
    message: "entry text must not contain host or credential material",
  }),
);

/**
 * One immutable, content-addressed public entry. Its `contentDigest` binds the
 * exact published bytes at the exact source generation. There is no host path,
 * credential, process handle, or write path in this shape.
 */
export const CodeShareEntry = S.Struct({
  entryRef: IdeProjectionRef,
  pathLabel: CodeSharePathLabel,
  languageRef: IdeProjectionRef,
  sourceGeneration: IdeProjectionGeneration,
  byteCount: NonNegativeInt.check(S.isLessThanOrEqualTo(MAX_CODE_SHARE_ENTRY_BYTES)),
  lineCount: NonNegativeInt,
  contentDigest: CodeShareDigest,
  content: BoundedEntryText,
  truncated: S.Boolean,
}).annotate({ identifier: "CodeShareEntry" });
export interface CodeShareEntry extends S.Schema.Type<typeof CodeShareEntry> {}

export const CodeShareOmissionReason = S.Literals([
  "not_allowlisted",
  "forbidden_material",
  "ignored",
  "hidden",
  "binary",
  "too_large",
  "private",
]);
export type CodeShareOmissionReason = typeof CodeShareOmissionReason.Type;

/** A recorded omission. It names the reason with an opaque ref, never the path. */
export const CodeShareOmission = S.Struct({
  omittedRef: IdeProjectionRef,
  reason: CodeShareOmissionReason,
}).annotate({ identifier: "CodeShareOmission" });
export interface CodeShareOmission extends S.Schema.Type<typeof CodeShareOmission> {}

export const CodeShareRetention = S.Struct({
  retainDays: PositiveRetentionDays,
  deleteAfter: IdeProjectionTimestamp,
}).annotate({ identifier: "CodeShareRetention" });
export interface CodeShareRetention extends S.Schema.Type<typeof CodeShareRetention> {}

/**
 * A publication-safe source descriptor. It carries only opaque generation-bound
 * refs for the origin session, project, worktree, and placement. It never
 * carries an attachment handle, a host root, or a runtime authority.
 */
export const CodeShareSource = S.Struct({
  sessionRef: IdeProjectionRef,
  projectRef: IdeProjectionRef,
  worktreeRef: IdeProjectionRef,
  placementRef: IdeProjectionRef,
  sourceGeneration: IdeProjectionGeneration,
}).annotate({ identifier: "CodeShareSource" });
export interface CodeShareSource extends S.Schema.Type<typeof CodeShareSource> {}

/**
 * The immutable public code-share bundle. A public link resolves exactly one of
 * these. It is a fixed, read-only snapshot. It carries no mutation, agent,
 * provider, terminal, repository, or attachment authority, and `visibility` and
 * `audience` are fixed at the public, anonymous value.
 */
export const CodeShareBundle = S.Struct({
  schema: S.Literal(IDE_CODE_SHARE_BUNDLE_SCHEMA_LITERAL),
  bundleRef: IdeProjectionRef,
  visibility: S.Literal("public"),
  audience: S.Literal("public_anonymous"),
  creatorRef: IdeProjectionRef,
  source: CodeShareSource,
  rendererVersion: IdeProjectionRef,
  entries: S.Array(CodeShareEntry).check(S.isMaxLength(MAX_CODE_SHARE_ENTRIES)),
  omissions: S.Array(CodeShareOmission).check(S.isMaxLength(MAX_CODE_SHARE_OMISSIONS)),
  entryCount: NonNegativeInt,
  omittedCount: NonNegativeInt,
  totalByteCount: NonNegativeInt.check(S.isLessThanOrEqualTo(MAX_CODE_SHARE_TOTAL_BYTES)),
  truncated: S.Boolean,
  retention: CodeShareRetention,
  createdAt: IdeProjectionTimestamp,
  expiresAt: IdeProjectionTimestamp,
  manifestDigest: CodeShareDigest,
})
  .pipe(
    S.check(
      S.makeFilter((bundle) => bundle.entries.length === bundle.entryCount, {
        message: "entryCount must match the published entry set",
      }),
      S.makeFilter((bundle) => bundle.omissions.length === bundle.omittedCount, {
        message: "omittedCount must match the recorded omission set",
      }),
      S.makeFilter((bundle) => Date.parse(bundle.expiresAt) > Date.parse(bundle.createdAt), {
        message: "bundle expiry must follow creation",
      }),
      S.makeFilter(
        (bundle) => Date.parse(bundle.retention.deleteAfter) >= Date.parse(bundle.expiresAt),
        { message: "retention deletion must not precede expiry" },
      ),
    ),
  )
  .annotate({ identifier: "CodeShareBundle" });
export interface CodeShareBundle extends S.Schema.Type<typeof CodeShareBundle> {}

/** Decode an untrusted bundle and reject every field outside the allowlist. */
export const decodeCodeShareBundle = (input: unknown) =>
  S.decodeUnknownEffect(CodeShareBundle)(input, { onExcessProperty: "error" });

const decodeBundleResult = S.decodeUnknownResult(CodeShareBundle);

/** A pure port for a content-addressing digest. Kept injectable so the package holds no Node, Web Crypto, or platform hashing dependency. */
export interface CodeShareDigestPort {
  readonly digest: (canonical: string) => string;
}

/**
 * The canonical byte string a `manifestDigest` binds. It excludes only the
 * digest itself, so any change to any published content, omission, source,
 * retention, or lifetime field changes the digest.
 */
export const canonicalCodeShareManifest = (
  bundle: Omit<CodeShareBundle, "manifestDigest">,
): string =>
  JSON.stringify([
    bundle.schema,
    bundle.bundleRef,
    bundle.visibility,
    bundle.audience,
    bundle.creatorRef,
    [
      bundle.source.sessionRef,
      bundle.source.projectRef,
      bundle.source.worktreeRef,
      bundle.source.placementRef,
      bundle.source.sourceGeneration,
    ],
    bundle.rendererVersion,
    bundle.entries.map((entry) => [
      entry.entryRef,
      entry.pathLabel,
      entry.languageRef,
      entry.sourceGeneration,
      entry.byteCount,
      entry.lineCount,
      entry.contentDigest,
      entry.truncated,
    ]),
    bundle.omissions.map((omission) => [omission.omittedRef, omission.reason]),
    bundle.entryCount,
    bundle.omittedCount,
    bundle.totalByteCount,
    bundle.truncated,
    [bundle.retention.retainDays, bundle.retention.deleteAfter],
    bundle.createdAt,
    bundle.expiresAt,
  ]);

/**
 * Recompute the manifest digest with the same port and compare it. Any mutation
 * of a published bundle field breaks this check. This is the immutability law:
 * a bundle cannot be written back or edited without breaking its integrity.
 */
export const verifyCodeShareBundleIntegrity = (
  bundle: CodeShareBundle,
  port: CodeShareDigestPort,
): boolean => {
  const decoded = decodeBundleResult(bundle, { onExcessProperty: "error" });
  if (Result.isFailure(decoded)) {
    return false;
  }
  const { manifestDigest, ...unsigned } = bundle;
  if (canonicalContainsForbiddenMaterial(bundle)) {
    return false;
  }
  // Content-addressing law: every frozen entry must hash to its recorded digest,
  // so an edited body cannot ride under an unchanged manifest.
  for (const entry of bundle.entries) {
    if (port.digest(entry.content) !== entry.contentDigest) {
      return false;
    }
  }
  return port.digest(canonicalCodeShareManifest(unsigned)) === manifestDigest;
};

/**
 * A redaction re-scan across the whole published surface. A verified public
 * bundle must never carry host paths, credentials, or other forbidden material
 * in any entry, omission, ref, or metadata field.
 */
export const canonicalContainsForbiddenMaterial = (bundle: CodeShareBundle): boolean =>
  hasForbiddenIdeProjectionMaterial(bundle);
