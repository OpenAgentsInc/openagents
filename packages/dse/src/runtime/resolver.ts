import { Effect, Schema as S } from "effect";

import {
  CandidateArtifact,
  ReleasedArtifactPointer,
  RollbackReceipt,
  ROLLBACK_RECEIPT_SCHEMA_LITERAL,
  candidateArtifactDigest,
  releasedArtifactRefFor,
  type CompiledProgram,
  type DseTimestamp,
} from "../contract/index.js";

/**
 * Offline released-artifact resolution and rollback.
 *
 * The runtime resolves a released artifact from checked-in bytes: it decodes the
 * pointer and the candidate bytes, recomputes the digest over all candidate
 * bytes, and admits the compiled program only when every reference agrees. A
 * missing, altered, unreviewed, or incompatible artifact fails closed with a
 * typed error. The runtime has no compile or promotion authority.
 */

export class ArtifactResolutionError extends S.TaggedErrorClass<ArtifactResolutionError>()(
  "dse/ArtifactResolutionError",
  {
    reason: S.Literals(["missing", "altered", "unreviewed", "incompatible", "malformed"]),
    detail: S.String,
  },
) {}

const decodePointer = S.decodeUnknownEffect(ReleasedArtifactPointer);
const decodeCandidate = S.decodeUnknownEffect(CandidateArtifact);
const decodeRollback = S.decodeUnknownSync(RollbackReceipt);

export interface ResolveArgs {
  /** The checked-in released pointer bytes (owner-provided JSON). */
  readonly pointer: unknown;
  /** The checked-in candidate artifact bytes the pointer released. */
  readonly candidateBytes: unknown;
  /** The signature the caller expects to serve. */
  readonly expectedSignatureId: string;
}

export interface ResolvedArtifact {
  readonly pointer: ReleasedArtifactPointer;
  readonly candidate: CandidateArtifact;
  readonly program: CompiledProgram;
}

/**
 * Resolve and verify a released artifact offline. Order of the closed failures:
 * malformed bytes, unreviewed pointer (no promotion), incompatible signature,
 * altered bytes (digest mismatch).
 */
export const resolveReleasedArtifact = (
  args: ResolveArgs,
): Effect.Effect<ResolvedArtifact, ArtifactResolutionError> =>
  Effect.gen(function* () {
    const pointer = yield* decodePointer(args.pointer).pipe(
      Effect.mapError(
        (error) => new ArtifactResolutionError({ reason: "malformed", detail: String(error) }),
      ),
    );
    const candidate = yield* decodeCandidate(args.candidateBytes).pipe(
      Effect.mapError(
        (error) => new ArtifactResolutionError({ reason: "malformed", detail: String(error) }),
      ),
    );

    // Unreviewed: the pointer must name a promotion and match its candidate.
    if (pointer.candidateId !== candidate.candidateId) {
      return yield* new ArtifactResolutionError({
        reason: "unreviewed",
        detail: "released pointer does not name the supplied candidate",
      });
    }

    // Incompatible: the served signature must match what the caller expects.
    if (
      pointer.signatureId !== args.expectedSignatureId ||
      candidate.signatureId !== args.expectedSignatureId ||
      candidate.program.signatureId !== args.expectedSignatureId
    ) {
      return yield* new ArtifactResolutionError({
        reason: "incompatible",
        detail: "artifact signature does not match the expected signature",
      });
    }

    // Altered: the digest must cover all candidate bytes, and the frozen
    // released record must carry the same digest and content-addressed ref.
    const recomputed = candidateArtifactDigest(candidate);
    const expectedRef = releasedArtifactRefFor(candidate.signatureId, recomputed);
    if (
      recomputed !== candidate.digest ||
      recomputed !== pointer.released.digest ||
      pointer.released.artifactRef !== expectedRef
    ) {
      return yield* new ArtifactResolutionError({
        reason: "altered",
        detail: "candidate digest does not match the released record",
      });
    }

    return { pointer, candidate, program: candidate.program };
  });

export interface RollbackArgs {
  /** The currently released pointer being rolled back. */
  readonly current: ReleasedArtifactPointer;
  /** The prior released pointer to restore. */
  readonly prior: ReleasedArtifactPointer;
  readonly reason: string;
  readonly now: () => typeof DseTimestamp.Type;
}

export type RollbackResult =
  | {
      readonly ok: true;
      readonly restored: ReleasedArtifactPointer;
      readonly receipt: RollbackReceipt;
    }
  | { readonly ok: false; readonly reason: "signature_mismatch" | "not_a_prior_release" };

/**
 * Roll back to a prior released artifact. The prior pointer must serve the same
 * signature and must not be the current release. The restored pointer is the
 * prior pointer unchanged, and the receipt records the transition.
 */
export const rollback = (args: RollbackArgs): RollbackResult => {
  if (args.current.signatureId !== args.prior.signatureId) {
    return { ok: false, reason: "signature_mismatch" };
  }
  if (args.current.candidateId === args.prior.candidateId) {
    return { ok: false, reason: "not_a_prior_release" };
  }
  const receipt = decodeRollback({
    schema: ROLLBACK_RECEIPT_SCHEMA_LITERAL,
    signatureId: args.current.signatureId,
    fromCandidateId: args.current.candidateId,
    toCandidateId: args.prior.candidateId,
    restoredArtifactRef: args.prior.released.artifactRef,
    reason: args.reason,
    rolledBackAt: args.now(),
  });
  return { ok: true, restored: args.prior, receipt };
};
