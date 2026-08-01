import { Schema as S } from "effect";

import { forensicSha256Digest, strictDecode } from "./canonical.ts";
import { Exactness, NonNegativeInteger, Sha256Digest } from "./primitives.ts";
import {
  CleanupState,
  CoverageStatus,
  ForensicRunSchema,
  ForensicRunState,
  type ForensicRun,
} from "./run.ts";

export const FORENSIC_RUN_PUBLIC_PROJECTION_VERSION =
  "openagents.forensic_run_public_projection.v1" as const;

export const PublicUsageTruthSchema = S.Struct({
  inputTokens: NonNegativeInteger,
  cachedInputTokens: NonNegativeInteger,
  outputTokens: NonNegativeInteger,
  reasoningTokens: NonNegativeInteger,
  totalTokens: NonNegativeInteger,
  exactness: Exactness,
});
export interface PublicUsageTruth extends S.Schema.Type<typeof PublicUsageTruthSchema> {}

export const ForensicRunPublicProjectionSchema = S.Struct({
  schema: S.Literal(FORENSIC_RUN_PUBLIC_PROJECTION_VERSION),
  runDigest: Sha256Digest,
  sourceDigest: Sha256Digest,
  promptDigest: Sha256Digest,
  modelDigest: Sha256Digest,
  workerImageDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  state: ForensicRunState,
  coverageStatus: CoverageStatus,
  cleanupState: CleanupState,
  findingCount: NonNegativeInteger,
  hypothesisCount: NonNegativeInteger,
  errorCount: NonNegativeInteger,
  lastEventSequence: NonNegativeInteger,
  totalDurationMilliseconds: S.optionalKey(NonNegativeInteger),
  totalDurationExactness: S.optionalKey(Exactness),
  usage: S.optionalKey(PublicUsageTruthSchema),
})
  .pipe(
    S.check(
      S.makeFilter(
        (projection) =>
          (projection.totalDurationMilliseconds === undefined) ===
          (projection.totalDurationExactness === undefined),
        { message: "public duration value and exactness must appear together" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicRunPublicProjection" });
export interface ForensicRunPublicProjection extends S.Schema.Type<
  typeof ForensicRunPublicProjectionSchema
> {}

export const projectForensicRunPublicSafe = (input: unknown): ForensicRunPublicProjection => {
  const run = strictDecode(ForensicRunSchema, input);
  const projection = {
    schema: FORENSIC_RUN_PUBLIC_PROJECTION_VERSION,
    runDigest: forensicSha256Digest(run),
    sourceDigest: run.sourceDigest,
    promptDigest: run.promptDigest,
    modelDigest: run.modelDigest,
    workerImageDigest: run.workerImageDigest,
    workerProfileDigest: run.workerProfileDigest,
    state: run.state,
    coverageStatus: run.coverageStatus,
    cleanupState: run.cleanupState,
    findingCount: run.findingRefs.length,
    hypothesisCount: run.hypothesisRefs.length,
    errorCount: run.errorRefs.length,
    lastEventSequence: run.lastEventSequence,
    ...(run.totalDuration === undefined
      ? {}
      : {
          totalDurationMilliseconds: run.totalDuration.milliseconds,
          totalDurationExactness: run.totalDuration.exactness,
        }),
    ...(run.usage === undefined
      ? {}
      : {
          usage: {
            inputTokens: run.usage.inputTokens,
            cachedInputTokens: run.usage.cachedInputTokens,
            outputTokens: run.usage.outputTokens,
            reasoningTokens: run.usage.reasoningTokens,
            totalTokens: run.usage.totalTokens,
            exactness: run.usage.exactness,
          },
        }),
  };
  return strictDecode(ForensicRunPublicProjectionSchema, projection);
};

export const assertPublicProjectionOrigin = (
  run: ForensicRun,
  projection: ForensicRunPublicProjection,
): boolean =>
  projection.runDigest === forensicSha256Digest(run) &&
  projection.sourceDigest === run.sourceDigest &&
  projection.promptDigest === run.promptDigest;
