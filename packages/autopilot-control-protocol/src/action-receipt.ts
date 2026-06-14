// #5002 typed action receipts + offline action queue. A bridge write action
// (decision.resolve / session.cancel) resolves to exactly one typed outcome so
// every client (desktop, web, Expo) renders the same receipt language. Pure +
// transport-agnostic: callers pass the HTTP result (or a network failure) in,
// and get a classified outcome out.

export type ActionOutcome =
  | "applied" // the node performed the action
  | "duplicate" // already applied (exactly-once); the original stands
  | "expired" // the decision/lease window has passed
  | "revoked" // the pairing credential was revoked
  | "stale" // the action targeted a superseded state
  | "unauthorized" // missing capability / invalid credential
  | "unsupported" // the node doesn't expose this verb
  | "overloaded" // node is rate-limiting / temporarily unavailable
  | "offline" // never reached the node (queue + retry)
  | "error" // any other failure

export type ClassifyActionInput = {
  // True when the request never reached the node (fetch threw / no network).
  networkError?: boolean
  // HTTP status, when a response was received.
  status?: number
  // Whether the node reported ok (the `{ ok: true }` envelope).
  ok?: boolean
  // The parsed `result` body, when present (e.g. { duplicate, applied, revoked }).
  body?: { duplicate?: unknown; applied?: unknown; revoked?: unknown; stale?: unknown } | null
}

// Classify a single write-action result into exactly one typed outcome. Body
// signals (duplicate/revoked/stale) win over a 2xx "applied" so exactly-once
// and revocation are never masked.
export function classifyActionOutcome(input: ClassifyActionInput): ActionOutcome {
  if (input.networkError === true) return "offline"

  const body = input.body ?? undefined
  if (body?.duplicate === true) return "duplicate"
  if (body?.revoked === true) return "revoked"
  if (body?.stale === true) return "stale"

  const status = input.status
  if (status !== undefined) {
    if (status === 401 || status === 403) return "unauthorized"
    if (status === 404 || status === 501) return "unsupported"
    if (status === 409) return "stale"
    if (status === 410) return "expired"
    if (status === 429 || status === 503) return "overloaded"
    if (status >= 500) return "error"
  }

  if (input.ok === true || (status !== undefined && status >= 200 && status < 300)) {
    return body?.applied === false ? "error" : "applied"
  }
  return "error"
}

// Is a queued action worth retrying when connectivity returns? Offline and
// overloaded are transient; everything else is terminal (the receipt stands).
export function isRetryableOutcome(outcome: ActionOutcome): boolean {
  return outcome === "offline" || outcome === "overloaded"
}

// ── Offline action queue ───────────────────────────────────────────────────
// A bounded, expiring queue for write actions taken while disconnected. Pure:
// the caller injects `nowMs`. Drained (oldest-first) when the bridge returns;
// entries past their TTL are dropped (and reported) rather than replayed stale.

export type QueuedAction<A> = {
  id: string
  action: A
  enqueuedAtMs: number
  expiresAtMs: number
}

export type DrainResult<A> = {
  ready: QueuedAction<A>[] // live entries to replay, oldest-first
  expired: QueuedAction<A>[] // dropped (past TTL) — surface as "expired" receipts
}

export type ActionQueue<A> = {
  enqueue: (input: { id: string; action: A; nowMs: number }) => void
  // Returns live entries to replay and the expired entries to drop; both are
  // removed from the queue (a replay attempt is the caller's responsibility —
  // re-enqueue on a fresh `offline`/`overloaded` outcome).
  drain: (nowMs: number) => DrainResult<A>
  size: () => number
}

export function createActionQueue<A>(opts: { ttlMs: number; maxSize?: number } = { ttlMs: 600_000 }): ActionQueue<A> {
  const ttlMs = opts.ttlMs
  const maxSize = opts.maxSize ?? 100
  let entries: QueuedAction<A>[] = []

  return {
    enqueue({ id, action, nowMs }) {
      // De-dup by id (a re-queued action replaces its prior entry).
      entries = entries.filter((e) => e.id !== id)
      entries.push({ id, action, enqueuedAtMs: nowMs, expiresAtMs: nowMs + ttlMs })
      // Bound the queue: drop the oldest beyond maxSize.
      if (entries.length > maxSize) entries = entries.slice(entries.length - maxSize)
    },
    drain(nowMs) {
      const ready: QueuedAction<A>[] = []
      const expired: QueuedAction<A>[] = []
      for (const entry of entries) {
        if (entry.expiresAtMs <= nowMs) expired.push(entry)
        else ready.push(entry)
      }
      entries = []
      return { ready, expired }
    },
    size: () => entries.length,
  }
}
