// Vendored from packages/agent-experience-memory/src/sync.ts by scripts/vendor-memory.mjs — do not edit here.
// The drift guard (test/vendored-memory-drift.test.ts) fails when this copy
// no longer matches the canonical source.
import { Schema as S } from "effect";

import { computeEngramEventId, type EngramEvent } from "./engram.js";

/**
 * The engram sync seam (issue #222).
 *
 * The ledger is local and authoritative. Sync is how a copy of it reaches
 * somewhere else, and the whole design follows from one rule: **a turn never
 * waits for it, and never fails because of it.** A relay that is slow, down,
 * or gone must be indistinguishable from one that is fine, as far as the
 * caller is concerned — because memory that can break a conversation is worse
 * than memory that is briefly out of date.
 *
 * That gives the three properties the tests hold this to:
 *
 * 1. **Local-first.** `publish` records the engram as pending and returns
 *    immediately. The engram is already in the local ledger by then; sync is
 *    catching up, not gatekeeping.
 * 2. **Nothing is lost.** A failed publish stays queued and is retried. The
 *    queue only forgets an engram once a transport has acknowledged it, so a
 *    transport that is down for an hour costs an hour of latency and no
 *    engrams.
 * 3. **Degraded is a state, not an error.** The queue reports what it is
 *    holding and what failed last, so a caller can *say* it is behind rather
 *    than discovering it by silence.
 *
 * What is deliberately not here: relays, sockets, keys, encryption. A
 * transport is anything that satisfies `EngramTransport`, and the real Nostr
 * one lands behind this interface without any caller changing. The workspace
 * has a shared Nostr implementation (`nostr-effect`) that a real transport
 * should build on rather than reimplement.
 */

export const SYNC_SCHEMA_ID = "openagents.engram_sync.v1" as const;

/** Why a publish did not land. Distinct because they need distinct responses. */
export const SyncFailureReason = S.Literals([
  /** The transport could not be reached at all. Retry later, unchanged. */
  "unreachable",
  /** The transport reached and refused this engram. Retrying will not help. */
  "refused",
  /** The transport accepted the call and failed inside it. Retry later. */
  "failed",
]);
export type SyncFailureReason = typeof SyncFailureReason.Type;

export type PublishResult =
  | { readonly ok: true; readonly eventId: string }
  | { readonly ok: false; readonly reason: SyncFailureReason; readonly detail?: string };

/** A filter for what to fetch back. Absent fields mean "no constraint". */
export interface EngramFilter {
  readonly authors?: ReadonlyArray<string>;
  /** `d` tag values — engram slugs. */
  readonly slugs?: ReadonlyArray<string>;
  /** Unix seconds; inclusive. */
  readonly since?: number;
  readonly until?: number;
  readonly limit?: number;
}

/**
 * Whatever carries engrams somewhere else.
 *
 * Implementations must not throw: a transport that cannot answer returns a
 * failure, because a throw from here would reach the caller's turn and the
 * whole point is that it cannot.
 */
export interface EngramTransport {
  publish(event: EngramEvent): Promise<PublishResult>;
  fetch(filter: EngramFilter): Promise<ReadonlyArray<EngramEvent>>;
}

/** What the queue is holding, for a caller that wants to say so. */
export interface SyncStatus {
  readonly pending: number;
  readonly delivered: number;
  /** Engrams the transport refused outright. They are not retried. */
  readonly refused: number;
  readonly lastFailure?: { readonly reason: SyncFailureReason; readonly detail?: string };
}

const matches = (event: EngramEvent, filter: EngramFilter): boolean => {
  if (filter.authors !== undefined && !filter.authors.includes(event.pubkey)) return false;
  if (filter.since !== undefined && event.created_at < filter.since) return false;
  if (filter.until !== undefined && event.created_at > filter.until) return false;
  if (filter.slugs !== undefined) {
    const slug = event.tags.find((tag) => tag[0] === "d")?.[1];
    if (slug === undefined || !filter.slugs.includes(slug)) return false;
  }
  return true;
};

/**
 * A transport that keeps engrams in memory.
 *
 * The reference implementation and what the tests run against. It is also a
 * useful real transport for a single process that wants sync's shape without
 * a relay.
 */
export class MemoryTransport implements EngramTransport {
  private readonly events = new Map<string, EngramEvent>();
  /** Set to fail every publish, for degraded-mode tests. */
  reachable = true;

  publish(event: EngramEvent): Promise<PublishResult> {
    if (!this.reachable) {
      return Promise.resolve({ ok: false, reason: "unreachable", detail: "transport is down" });
    }
    if (event.id !== computeEngramEventId(event)) {
      return Promise.resolve({ ok: false, reason: "refused", detail: "event id does not verify" });
    }
    this.events.set(event.id, event);
    return Promise.resolve({ ok: true, eventId: event.id });
  }

  fetch(filter: EngramFilter): Promise<ReadonlyArray<EngramEvent>> {
    if (!this.reachable) return Promise.resolve([]);
    const found = [...this.events.values()]
      .filter((event) => matches(event, filter))
      .sort((left, right) =>
        left.created_at !== right.created_at
          ? left.created_at - right.created_at
          : left.id.localeCompare(right.id),
      );
    return Promise.resolve(filter.limit === undefined ? found : found.slice(0, filter.limit));
  }

  /** Everything the transport holds, for assertions. */
  stored(): ReadonlyArray<EngramEvent> {
    return [...this.events.values()];
  }
}

/**
 * The queue between the ledger and a transport.
 *
 * `publish` is synchronous from the caller's side: it enqueues and returns.
 * `drain` is what actually talks to the transport, and a host calls it
 * whenever it likes — after a turn, on a timer, at exit. Nothing about the
 * caller's turn depends on when that happens.
 */
export class EngramSyncQueue {
  private readonly queue: Array<EngramEvent> = [];
  private readonly deliveredIds = new Set<string>();
  private readonly refusedIds = new Set<string>();
  private lastFailure: SyncStatus["lastFailure"];
  private draining = false;

  constructor(private readonly transport: EngramTransport) {}

  /**
   * Enqueue an engram for delivery. Returns nothing to wait on.
   *
   * An engram already delivered or already refused is not enqueued twice, so
   * a caller that republishes its whole ledger costs one pass, not one
   * delivery per pass.
   */
  publish(event: EngramEvent): void {
    if (this.deliveredIds.has(event.id) || this.refusedIds.has(event.id)) return;
    if (this.queue.some((queued) => queued.id === event.id)) return;
    this.queue.push(event);
  }

  /**
   * Try to deliver everything queued.
   *
   * Returns how many landed. An unreachable or failing transport leaves the
   * engram queued for the next drain; a refusal is terminal, because retrying
   * something the transport has judged invalid only repeats the judgement.
   * Never throws — a transport that throws anyway is treated as failing.
   */
  async drain(): Promise<number> {
    if (this.draining) return 0;
    this.draining = true;
    try {
      let delivered = 0;
      // Copy: a publish during a drain lands in the next one rather than
      // mutating the array being walked.
      const attempting = [...this.queue];
      for (const event of attempting) {
        let result: PublishResult;
        try {
          result = await this.transport.publish(event);
        } catch (cause) {
          result = {
            ok: false,
            reason: "failed",
            detail: cause instanceof Error ? cause.message : String(cause),
          };
        }
        if (result.ok) {
          this.deliveredIds.add(event.id);
          this.removeFromQueue(event.id);
          delivered += 1;
          continue;
        }
        this.lastFailure = {
          reason: result.reason,
          ...(result.detail === undefined ? {} : { detail: result.detail }),
        };
        if (result.reason === "refused") {
          this.refusedIds.add(event.id);
          this.removeFromQueue(event.id);
        }
        // "unreachable" and "failed" stay queued for the next drain.
      }
      return delivered;
    } finally {
      this.draining = false;
    }
  }

  private removeFromQueue(eventId: string): void {
    const at = this.queue.findIndex((queued) => queued.id === eventId);
    if (at >= 0) this.queue.splice(at, 1);
  }

  /** Fetch from the transport. An unreachable transport yields nothing. */
  async fetch(filter: EngramFilter): Promise<ReadonlyArray<EngramEvent>> {
    try {
      return await this.transport.fetch(filter);
    } catch {
      return [];
    }
  }

  status(): SyncStatus {
    return {
      pending: this.queue.length,
      delivered: this.deliveredIds.size,
      refused: this.refusedIds.size,
      ...(this.lastFailure === undefined ? {} : { lastFailure: this.lastFailure }),
    };
  }

  /** Whether the queue is holding anything the transport has not taken. */
  behind(): boolean {
    return this.queue.length > 0;
  }
}
