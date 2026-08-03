import { strictDecode } from "@openagentsinc/forensic-contract";
import { Data, Effect } from "effect";

import {
  FORENSIC_PROMPT_GOVERNANCE_STATE_VERSION,
  ForensicPromptActiveTransition,
  type ForensicPromptActiveTransition as ForensicPromptActiveTransitionType,
  type ForensicPromptGovernanceState,
} from "../schemas/forensic-prompt-optimization";
import { forensicPromptTransitionDigestMatches } from "../services/forensic-prompt-compiler";

export class ForensicPromptGovernanceError extends Data.TaggedError(
  "ForensicPromptGovernanceError",
)<{
  readonly code: "conflict" | "invalid_transition" | "storage_unavailable";
  readonly message: string;
  readonly retryable: boolean;
}> {}

export type ForensicPromptGovernanceSql = <T = Record<string, unknown>>(
  strings: TemplateStringsArray,
  ...values: ReadonlyArray<unknown>
) => Promise<Array<T>>;

export type ForensicPromptGovernanceStore = Readonly<{
  /**
   * Append one transition under compare-and-set on the caller's observed
   * revision. A decision taken against a stale read is rejected, never merged.
   */
  append: (
    ownerRef: string,
    transition: unknown,
    expectedRevision: number,
  ) => Effect.Effect<ForensicPromptGovernanceState, ForensicPromptGovernanceError>;
  read: (
    ownerRef: string,
  ) => Effect.Effect<ForensicPromptGovernanceState, ForensicPromptGovernanceError>;
}>;

type PointerRow = Readonly<{
  active_prompt_digest: string | null;
  revision: number | string;
}>;

type TransitionRow = Readonly<{
  transition_digest: string;
  transition_json: unknown;
}>;

const storageError = () =>
  new ForensicPromptGovernanceError({
    code: "storage_unavailable",
    message: "forensic prompt governance storage is unavailable",
    retryable: true,
  });

const invalidTransition = () =>
  new ForensicPromptGovernanceError({
    code: "invalid_transition",
    message: "forensic prompt transition failed immutable validation",
    retryable: false,
  });

const decodeTransition = (value: unknown): ForensicPromptActiveTransitionType => {
  let transition: ForensicPromptActiveTransitionType;
  try {
    transition = strictDecode(ForensicPromptActiveTransition, value);
  } catch {
    throw invalidTransition();
  }
  if (!forensicPromptTransitionDigestMatches(transition)) throw invalidTransition();
  if (!Number.isSafeInteger(transition.sequence) || transition.sequence < 1) {
    throw invalidTransition();
  }
  return transition;
};

const parsePayload = (value: unknown): unknown => {
  if (typeof value !== "string") return value;
  try {
    return JSON.parse(value) as unknown;
  } catch {
    throw invalidTransition();
  }
};

/**
 * The durable owner-scoped active prompt pointer and its append-only history.
 *
 * Two properties matter here. The pointer is never edited in place by a
 * decision: every change is a new appended transition, and the pointer is
 * derived from the last one. And every read re-verifies the stored transition
 * digests and the pointer/history agreement, so storage drift surfaces as an
 * error instead of being served as governance truth.
 */
export const makePostgresForensicPromptGovernanceStore = (
  sql: ForensicPromptGovernanceSql,
): ForensicPromptGovernanceStore => {
  const read: ForensicPromptGovernanceStore["read"] = (ownerRef) =>
    Effect.gen(function* () {
      const pointerRows = yield* Effect.tryPromise({
        try: () => sql<PointerRow>`
          SELECT active_prompt_digest, revision
            FROM forensic_prompt_active_pointers
           WHERE owner_ref = ${ownerRef}
        `,
        catch: storageError,
      });
      const transitionRows = yield* Effect.tryPromise({
        try: () => sql<TransitionRow>`
          SELECT transition_digest, transition_json
            FROM forensic_prompt_transitions
           WHERE owner_ref = ${ownerRef}
           ORDER BY sequence ASC
        `,
        catch: storageError,
      });
      return yield* Effect.try({
        try: (): ForensicPromptGovernanceState => {
          const history = transitionRows.map((row, index) => {
            const transition = decodeTransition(parsePayload(row.transition_json));
            if (
              transition.transitionDigest !== row.transition_digest ||
              transition.sequence !== index + 1
            ) {
              throw invalidTransition();
            }
            return transition;
          });
          const pointer = pointerRows[0];
          const revision = pointer === undefined ? 0 : Number(pointer.revision);
          const activePromptDigest = pointer?.active_prompt_digest ?? null;
          if (
            !Number.isSafeInteger(revision) ||
            revision !== history.length ||
            (history.at(-1)?.activePromptDigest ?? null) !== activePromptDigest
          ) {
            throw invalidTransition();
          }
          return {
            activePromptDigest,
            history,
            ownerRef,
            revision,
            schema: FORENSIC_PROMPT_GOVERNANCE_STATE_VERSION,
          };
        },
        catch: (error) => (error instanceof ForensicPromptGovernanceError ? error : storageError()),
      });
    });

  return {
    append: (ownerRef, value, expectedRevision) =>
      Effect.gen(function* () {
        const transition = yield* Effect.try({
          try: () => decodeTransition(value),
          catch: (error) =>
            error instanceof ForensicPromptGovernanceError ? error : storageError(),
        });
        if (
          !Number.isSafeInteger(expectedRevision) ||
          expectedRevision < 0 ||
          transition.sequence !== expectedRevision + 1
        ) {
          return yield* new ForensicPromptGovernanceError({
            code: "conflict",
            message: "forensic prompt transition sequence is stale",
            retryable: false,
          });
        }
        const rows = yield* Effect.tryPromise({
          try: () => sql<PointerRow>`
            WITH locked AS (
              SELECT pg_advisory_xact_lock(hashtextextended(${ownerRef}, 0))
            ), current_pointer AS (
              SELECT active_prompt_digest, revision
                FROM forensic_prompt_active_pointers, locked
               WHERE owner_ref = ${ownerRef}
            ), admitted AS (
              SELECT 1
               WHERE (
                 NOT EXISTS (SELECT 1 FROM current_pointer)
                 AND ${expectedRevision}::bigint = 0
                 AND ${transition.priorActivePromptDigest}::text IS NULL
               ) OR EXISTS (
                 SELECT 1 FROM current_pointer
                  WHERE revision = ${expectedRevision}::bigint
                    AND active_prompt_digest
                        IS NOT DISTINCT FROM ${transition.priorActivePromptDigest}::text
               )
            ), inserted AS (
              INSERT INTO forensic_prompt_transitions
                (owner_ref, sequence, transition_ref, transition_digest,
                 transition_json, decided_at, created_at)
              SELECT ${ownerRef}, ${transition.sequence}, ${transition.transitionRef},
                     ${transition.transitionDigest}, ${JSON.stringify(transition)}::jsonb,
                     ${transition.decidedAt}, NOW()
                FROM admitted
              ON CONFLICT DO NOTHING
              RETURNING 1
            )
            INSERT INTO forensic_prompt_active_pointers
              (owner_ref, active_prompt_digest, revision, updated_at)
            SELECT ${ownerRef}, ${transition.activePromptDigest}, ${transition.sequence}, NOW()
              FROM inserted
            ON CONFLICT (owner_ref) DO UPDATE
              SET active_prompt_digest = EXCLUDED.active_prompt_digest,
                  revision = EXCLUDED.revision,
                  updated_at = EXCLUDED.updated_at
              WHERE forensic_prompt_active_pointers.revision = ${expectedRevision}::bigint
            RETURNING active_prompt_digest, revision
          `,
          catch: storageError,
        });
        if (rows[0] === undefined) {
          return yield* new ForensicPromptGovernanceError({
            code: "conflict",
            message: "forensic prompt active pointer changed or transition ref conflicted",
            retryable: false,
          });
        }
        return yield* read(ownerRef);
      }),
    read,
  };
};
