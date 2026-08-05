/**
 * Socket discipline for the relay subscription (issue #9321 scope 5),
 * harvested from the Boltz teardown §6 mechanics — as pure policy, so every
 * rule is testable without a socket:
 *
 * - exponential backoff: 1 s initial, 30 s max, factor 2, 50% jitter;
 * - 15 s connect timeout; application ping every 15 s with force-reconnect
 *   on an unanswered ping (catches half-open sockets);
 * - a stability reset: a frame arriving >= 10 s after open resets backoff
 *   and disengages the polling fallback;
 * - degrade to polling after 3 failed connect attempts (5 s interval,
 *   bulk requests chunked, change-only emission).
 *
 * Missed-event coverage is by resubscription replay, not a cursor: the
 * session fold is set-deterministic and idempotent, so replaying the EOSE
 * snapshot plus overlapping live events after a reconnect produces no
 * duplicate and no missing session events.
 */

export interface ReconnectPolicy {
  readonly initialDelayMs: number;
  readonly maxDelayMs: number;
  readonly factor: number;
  /** Fraction of the base delay used as the jitter range. */
  readonly jitterRatio: number;
  readonly connectTimeoutMs: number;
  readonly pingIntervalMs: number;
  /** A frame at least this long after open counts as a stable connection. */
  readonly stabilityResetMs: number;
  readonly degradeAfterFailures: number;
  readonly pollIntervalMs: number;
  readonly pollChunkSize: number;
}

export const DEFAULT_RECONNECT_POLICY: ReconnectPolicy = {
  initialDelayMs: 1_000,
  maxDelayMs: 30_000,
  factor: 2,
  jitterRatio: 0.5,
  connectTimeoutMs: 15_000,
  pingIntervalMs: 15_000,
  stabilityResetMs: 10_000,
  degradeAfterFailures: 3,
  pollIntervalMs: 5_000,
  pollChunkSize: 64,
};

/**
 * Backoff for the given attempt (0-based), with jitter from the injected
 * random source in [0, 1). Deterministic given the random value.
 */
export function backoffDelayMs(
  attempt: number,
  policy: ReconnectPolicy,
  random01: number,
): number {
  const base = Math.min(policy.initialDelayMs * policy.factor ** attempt, policy.maxDelayMs);
  const jitter = base * policy.jitterRatio * random01;
  return Math.min(base + jitter, policy.maxDelayMs * (1 + policy.jitterRatio));
}

export interface TransportState {
  readonly mode: "socket" | "polling";
  readonly attempt: number;
  readonly consecutiveFailures: number;
  /** True while the polling fallback is engaged alongside reconnect attempts. */
  readonly fallbackEngaged: boolean;
}

export const INITIAL_TRANSPORT_STATE: TransportState = {
  mode: "socket",
  attempt: 0,
  consecutiveFailures: 0,
  fallbackEngaged: false,
};

export type TransportEvent =
  | { readonly kind: "connect_failed" }
  | { readonly kind: "connect_timeout" }
  | { readonly kind: "open" }
  | { readonly kind: "frame"; readonly msAfterOpen: number }
  | { readonly kind: "ping_unanswered" }
  | { readonly kind: "closed" };

export interface TransportTransition {
  readonly state: TransportState;
  /** Force an immediate reconnect (half-open socket detected). */
  readonly forceReconnect: boolean;
  /** On reconnect/open, resubscribe: replay covers missed events. */
  readonly resubscribe: boolean;
}

export function reduceTransportEvent(
  state: TransportState,
  event: TransportEvent,
  policy: ReconnectPolicy,
): TransportTransition {
  switch (event.kind) {
    case "connect_failed":
    case "connect_timeout": {
      const failures = state.consecutiveFailures + 1;
      const degrade = failures >= policy.degradeAfterFailures;
      return {
        state: {
          mode: degrade ? "polling" : state.mode,
          attempt: state.attempt + 1,
          consecutiveFailures: failures,
          fallbackEngaged: degrade || state.fallbackEngaged,
        },
        forceReconnect: false,
        resubscribe: false,
      };
    }
    case "open":
      // Open alone is not stability: backoff resets only on a stable frame.
      return {
        state: { ...state, mode: "socket" },
        forceReconnect: false,
        resubscribe: true,
      };
    case "frame": {
      if (event.msAfterOpen >= policy.stabilityResetMs) {
        return {
          state: {
            mode: "socket",
            attempt: 0,
            consecutiveFailures: 0,
            fallbackEngaged: false,
          },
          forceReconnect: false,
          resubscribe: false,
        };
      }
      return { state, forceReconnect: false, resubscribe: false };
    }
    case "ping_unanswered":
      return {
        state: { ...state, consecutiveFailures: state.consecutiveFailures + 1 },
        forceReconnect: true,
        resubscribe: false,
      };
    case "closed":
      return { state, forceReconnect: true, resubscribe: false };
  }
}

/**
 * Change-only emission: the poller emits only when the serialised view
 * actually changes, matching socket semantics.
 */
export function shouldEmit(previousSerialized: string | null, nextSerialized: string): boolean {
  return previousSerialized !== nextSerialized;
}

/** Bulk poll chunking; one unknown id must not reject the whole batch. */
export function pollChunks<T>(ids: readonly T[], policy: ReconnectPolicy): readonly (readonly T[])[] {
  const chunks: T[][] = [];
  for (let index = 0; index < ids.length; index += policy.pollChunkSize) {
    chunks.push(ids.slice(index, index + policy.pollChunkSize));
  }
  return chunks;
}
