import { Context, Effect, Layer, Result, Schema as S } from "effect";

import type { CodeShareDigestPort } from "./ide-code-share-bundle.js";
import {
  CodeShareBundle,
  CodeShareDigest,
  CodeShareEntry,
  CodeShareOmission,
  CodeShareOmissionReason,
  CodeSharePathLabel,
  CodeShareRetention,
  CodeShareSource,
  IDE_CODE_SHARE_BUNDLE_SCHEMA_LITERAL,
  MAX_CODE_SHARE_ENTRIES,
  MAX_CODE_SHARE_ENTRY_BYTES,
  MAX_CODE_SHARE_TOTAL_BYTES,
  canonicalCodeShareManifest,
  decodeCodeShareBundle,
} from "./ide-code-share-bundle.js";
import {
  IdeProjectionRef,
  IdeProjectionTimestamp,
  hasForbiddenIdeProjectionMaterial,
} from "./ide-review-projection.js";

const PositiveRetentionDays = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(365),
);
const PositiveLifetimeSeconds = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(31_536_000),
);

/** A source-declared classification for one candidate path. */
export const CodeShareCandidateClass = S.Literals([
  "text",
  "binary",
  "ignored",
  "hidden",
  "private",
]);
export type CodeShareCandidateClass = typeof CodeShareCandidateClass.Type;

/**
 * One candidate for publication. The authoritative placement declares the
 * allowlist and the classification. The builder never widens either: a
 * non-allowlisted, non-text, or forbidden candidate can only be omitted.
 */
export const CodeShareCandidate = S.Struct({
  entryRef: IdeProjectionRef,
  pathLabel: CodeSharePathLabel,
  languageRef: IdeProjectionRef,
  sourceGeneration: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  classification: CodeShareCandidateClass,
  allowlisted: S.Boolean,
  content: S.String.check(S.isMaxLength(MAX_CODE_SHARE_TOTAL_BYTES)),
}).annotate({ identifier: "CodeShareCandidate" });
export interface CodeShareCandidate extends S.Schema.Type<typeof CodeShareCandidate> {}

export const CodeShareBuildRequest = S.Struct({
  bundleRef: IdeProjectionRef,
  creatorRef: IdeProjectionRef,
  rendererVersion: IdeProjectionRef,
  source: CodeShareSource,
  candidates: S.Array(CodeShareCandidate).check(S.isMaxLength(4_000)),
  retainDays: PositiveRetentionDays,
  lifetimeSeconds: PositiveLifetimeSeconds,
  createdAt: IdeProjectionTimestamp,
}).annotate({ identifier: "CodeShareBuildRequest" });
export interface CodeShareBuildRequest extends S.Schema.Type<typeof CodeShareBuildRequest> {}

export const CodeShareBuilderFailureReason = S.Literals([
  "invalid_request",
  "forbidden_ref_material",
  "bundle_too_large",
  "invalid_bundle",
]);
export type CodeShareBuilderFailureReason = typeof CodeShareBuilderFailureReason.Type;

export class CodeShareBuilderFailure extends S.TaggedErrorClass<CodeShareBuilderFailure>()(
  "CodeShareBuilderFailure",
  {
    operation: S.Literal("CodeShareBuilder.build"),
    reason: CodeShareBuilderFailureReason,
  },
) {}

const failure = (reason: CodeShareBuilderFailureReason): CodeShareBuilderFailure =>
  new CodeShareBuilderFailure({ operation: "CodeShareBuilder.build", reason });

const decodeRequest = S.decodeUnknownResult(CodeShareBuildRequest);
const strictOptions = { onExcessProperty: "error" as const };

const forbiddenText = [
  /(?:^|[\s"'(=])\/(?:Users|home|root|private|var|etc|opt|tmp|workspace|mnt|srv|data|run)\//i,
  /(?:^|[\s"'(=])[a-z]:\\/i,
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/i,
  /\bAKIA[0-9A-Z]{16}\b/,
  /\b(?:ghp_|github_pat_|sk-)[A-Za-z0-9_-]{16,}\b/i,
  /\b(?:api[_-]?key|password|passwd|secret|token)\s*[:=]\s*\S+/i,
] as const;

const contentIsForbidden = (value: string): boolean =>
  forbiddenText.some((pattern) => pattern.test(value));

const classifyOmission = (candidate: CodeShareCandidate): CodeShareOmissionReason | undefined => {
  if (!candidate.allowlisted) return "not_allowlisted";
  if (candidate.classification === "binary") return "binary";
  if (candidate.classification === "ignored") return "ignored";
  if (candidate.classification === "hidden") return "hidden";
  if (candidate.classification === "private") return "private";
  if (contentIsForbidden(candidate.content)) return "forbidden_material";
  if (candidate.content.length > MAX_CODE_SHARE_ENTRY_BYTES) return "too_large";
  return undefined;
};

const countLines = (value: string): number => (value.length === 0 ? 0 : value.split("\n").length);

/**
 * Compile an immutable, allowlisted, content-addressed public bundle. The scan
 * runs on the exact bytes that become the frozen content, so no TOCTOU window
 * exists between scan and publish: the manifest digest binds the scanned bytes.
 */
export const compileCodeShareBundle = (
  input: unknown,
  port: CodeShareDigestPort,
): Result.Result<CodeShareBundle, CodeShareBuilderFailure> => {
  const requestResult = decodeRequest(input, strictOptions);
  if (Result.isFailure(requestResult)) {
    return Result.fail(failure("invalid_request"));
  }
  const request = requestResult.success;

  if (
    hasForbiddenIdeProjectionMaterial({
      bundleRef: request.bundleRef,
      creatorRef: request.creatorRef,
      rendererVersion: request.rendererVersion,
      source: request.source,
      candidateRefs: request.candidates.map((candidate) => ({
        entryRef: candidate.entryRef,
        pathLabel: candidate.pathLabel,
        languageRef: candidate.languageRef,
      })),
    })
  ) {
    return Result.fail(failure("forbidden_ref_material"));
  }

  const entries: Array<CodeShareEntry> = [];
  const omissions: Array<CodeShareOmission> = [];
  let totalByteCount = 0;

  for (const candidate of request.candidates) {
    const omissionReason = classifyOmission(candidate);
    if (omissionReason !== undefined) {
      omissions.push(
        CodeShareOmission.make({ omittedRef: candidate.entryRef, reason: omissionReason }),
      );
      continue;
    }
    const byteCount = candidate.content.length;
    const entry = CodeShareEntry.make({
      entryRef: candidate.entryRef,
      pathLabel: candidate.pathLabel,
      languageRef: candidate.languageRef,
      sourceGeneration: candidate.sourceGeneration,
      byteCount,
      lineCount: countLines(candidate.content),
      contentDigest: port.digest(candidate.content) as CodeShareDigest,
      content: candidate.content,
      truncated: false,
    });
    entries.push(entry);
    totalByteCount += byteCount;
  }

  if (entries.length > MAX_CODE_SHARE_ENTRIES || totalByteCount > MAX_CODE_SHARE_TOTAL_BYTES) {
    return Result.fail(failure("bundle_too_large"));
  }

  const createdMillis = Date.parse(request.createdAt);
  if (!Number.isFinite(createdMillis)) {
    return Result.fail(failure("invalid_request"));
  }
  const expiresAt = new Date(createdMillis + request.lifetimeSeconds * 1_000).toISOString();
  const deleteAfter = new Date(
    createdMillis + request.lifetimeSeconds * 1_000 + request.retainDays * 86_400_000,
  ).toISOString();

  const unsigned = {
    schema: IDE_CODE_SHARE_BUNDLE_SCHEMA_LITERAL,
    bundleRef: request.bundleRef,
    visibility: "public" as const,
    audience: "public_anonymous" as const,
    creatorRef: request.creatorRef,
    source: request.source,
    rendererVersion: request.rendererVersion,
    entries,
    omissions,
    entryCount: entries.length,
    omittedCount: omissions.length,
    totalByteCount,
    truncated: omissions.length > 0,
    retention: CodeShareRetention.make({ retainDays: request.retainDays, deleteAfter }),
    createdAt: request.createdAt,
    expiresAt,
  };
  const manifestDigest = port.digest(canonicalCodeShareManifest(unsigned)) as CodeShareDigest;

  const bundleResult = S.decodeUnknownResult(CodeShareBundle)(
    { ...unsigned, manifestDigest },
    strictOptions,
  );
  return Result.isFailure(bundleResult)
    ? Result.fail(failure("invalid_bundle"))
    : Result.succeed(bundleResult.success);
};

export interface CodeShareBuilderInterface {
  readonly build: (input: unknown) => Effect.Effect<CodeShareBundle, CodeShareBuilderFailure>;
}

export class CodeShareBuilder extends Context.Service<
  CodeShareBuilder,
  CodeShareBuilderInterface
>()("ide-runtime.CodeShareBuilder") {}

/** A digest port service. A host layer supplies a real SHA-256 without the package importing a platform hash. */
export class CodeShareDigestService extends Context.Service<
  CodeShareDigestService,
  CodeShareDigestPort
>()("ide-runtime.CodeShareDigestService") {}

export const CodeShareBuilderLayer = Layer.effect(
  CodeShareBuilder,
  Effect.gen(function* () {
    const port = yield* CodeShareDigestService;
    const build = Effect.fn("CodeShareBuilder.build")((input: unknown) =>
      Effect.fromResult(compileCodeShareBundle(input, port)),
    );
    return CodeShareBuilder.of({ build });
  }),
);

export { decodeCodeShareBundle };
