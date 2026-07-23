import { Schema as S } from "effect";
import { HarnessCursor } from "./stream.ts";

/**
 * Resume state returned by `detach()` / `stop()` and re-imported by
 * `start({ resumeFrom })`. The `data` payload is adapter-defined and validated
 * by the adapter's own `lifecycleStateSchema` at re-import time — the framework
 * carries it opaquely but persists it in the durable turn journal, so unlike
 * the AI SDK (which hands unvalidated resume state to the caller) a corrupt or
 * cross-adapter payload is caught before it reaches the runtime.
 */
export const HarnessResumeState = S.Struct({
  harnessId: S.NonEmptyString,
  sessionId: S.NonEmptyString,
  /** Opaque adapter-defined payload; validated by the adapter on re-import. */
  data: S.Unknown,
});
export interface HarnessResumeState extends S.Schema.Type<typeof HarnessResumeState> {}

/**
 * Continuation state returned by `suspendTurn()` and re-imported by
 * `start({ continueFrom })` then `continueTurn()`. It pins the exact replay
 * cursor at the slice boundary: the next slice's attach replays from
 * `cursor + 1` with no gap and no duplicate. `lossy` records honestly whether
 * the runtime kept the live turn (bridge/attach: lossless) or the turn had to
 * be re-driven from persisted state (host-resident rerun: the tail after
 * `cursor` is recomputed).
 */
export const HarnessContinuationState = S.Struct({
  harnessId: S.NonEmptyString,
  sessionId: S.NonEmptyString,
  turnId: S.NonEmptyString,
  /** Last stream-event sequence delivered before the suspend. */
  cursor: HarnessCursor,
  /** True when continuation re-drives the turn (recomputed tail) rather than attaching. */
  lossy: S.Boolean,
  /** Opaque adapter-defined payload; validated by the adapter on re-import. */
  data: S.Unknown,
});
export interface HarnessContinuationState extends S.Schema.Type<typeof HarnessContinuationState> {}
