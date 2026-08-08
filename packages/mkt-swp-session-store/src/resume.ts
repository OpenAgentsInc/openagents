/**
 * App-wide resume planning (issue #9320 scope item 3) and the reload guard
 * (scope item 6).
 *
 * Resume is app-wide, not page-scoped: on load the host walks EVERY stored
 * session and re-attaches a subscription for each non-terminal one, plus
 * terminal-but-unclaimed ones. Each task carries a catch-up spec — a bounded
 * snapshot read (`since` the last persisted record) that must complete
 * before the live fold is trusted, the EOSE-snapshot-then-live pattern
 * proven in the Immortal client.
 *
 * External-effect idempotency on resume is owned by the store's effect
 * ledger (`SessionStore.priorEffectResult`): a persisted result suppresses
 * the callback, so a reload can never duplicate a funding broadcast or
 * payment (MKT §"Idempotency").
 */
import { isEffectPending, type StoredSwapSession } from "./model.js";

export type ResumeReason = "in_flight" | "terminal_unclaimed";

export interface ResumeCatchUpSpec {
  /** Authors whose records the catch-up read must cover. */
  readonly authors: ReadonlyArray<string>;
  /** Unix seconds of the newest persisted record; the snapshot reads from here. */
  readonly since: number;
  /**
   * The snapshot (EOSE) must complete before the live fold is trusted;
   * always true — carried explicitly so the host cannot miss the ordering.
   */
  readonly snapshotBeforeLive: true;
}

export interface ResumeTask {
  readonly sessionId: string;
  readonly relayUrl: string;
  readonly reason: ResumeReason;
  readonly catchUp: ResumeCatchUpSpec;
}

const newestRecordCreatedAt = (session: StoredSwapSession): number =>
  session.signedRecords.reduce((newest, record) => Math.max(newest, record.created_at), 0);

export const resumeReasonOf = (session: StoredSwapSession): ResumeReason | null => {
  if (!session.projection.terminal) return "in_flight";
  if (session.projection.unclaimedFunds) return "terminal_unclaimed";
  return null;
};

/**
 * The sessions the host must re-attach, in stored order. Every non-terminal
 * session resumes; a terminal session resumes while funds remain unclaimed.
 */
export const planResume = (sessions: ReadonlyArray<StoredSwapSession>): ReadonlyArray<ResumeTask> =>
  sessions.flatMap((session) => {
    const reason = resumeReasonOf(session);
    if (reason === null) return [];
    return [
      {
        sessionId: session.sessionId,
        relayUrl: session.relayUrl,
        reason,
        catchUp: {
          authors: [session.requesterPubkey, session.providerPubkey],
          since: newestRecordCreatedAt(session),
          snapshotBeforeLive: true,
        },
      },
    ];
  });

export interface ReloadGuardVerdict {
  /** True while navigation/reload must be guarded. */
  readonly blocked: boolean;
  /** Sessions with an irreversible external effect requested but unresulted. */
  readonly pendingSessionIds: ReadonlyArray<string>;
}

/**
 * A session has an irreversible effect pending when an external-effect
 * request was durably recorded and neither a result nor a definitive
 * failure has been: the wallet may be broadcasting or paying RIGHT NOW.
 * The host wires this to a beforeunload/navigation guard. A recorded
 * definitive failure (`SessionStore.recordEffectFailure` — the user
 * cancelled the prompt, the call was rejected) releases the guard; an
 * UNKNOWN outcome keeps it, because recovery stays honest either way —
 * the persisted request is exactly what the resume path replays — and the
 * guard spares the user the crash-window ceremony.
 */
export const reloadGuard = (sessions: ReadonlyArray<StoredSwapSession>): ReloadGuardVerdict => {
  const pendingSessionIds = sessions
    .filter((session) => session.effectLedger.some(isEffectPending))
    .map((session) => session.sessionId);
  return { blocked: pendingSessionIds.length > 0, pendingSessionIds };
};
