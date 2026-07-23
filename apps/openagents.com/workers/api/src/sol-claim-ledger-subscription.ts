/**
 * Sol claim-ledger live-filter subscription runtime (forge Stage 2, #9185).
 *
 * The prior slices in this issue built, from the bottom up:
 *   - `@openagentsinc/forge-protocol` — the pure projection of the Sol claim
 *     record to and from unsigned NIP-34 event templates.
 *   - `./sol-claim-ledger-relay` — sign, serialize to NIP-01 relay frames, parse
 *     back, verify the schnorr signature, recover the typed entry, and build the
 *     repository-coordinate read filter (`solClaimLedgerRepositoryFilter`).
 *   - `./sol-claim-ledger-store` — a durable `EventStore`: append signed events
 *     and query them back by repository coordinate, verified on both edges.
 *
 * The top residual was a *live-filter subscription runtime*: a service that
 * streams the repository-coordinate filter to connected agents so that when a
 * new matching claim-ledger event is appended to the store, every subscriber of
 * that coordinate receives it live — the fan-out half of a NIP-01 `REQ`
 * subscription, where the durable coordinate query owns the initial backlog and
 * this runtime owns "everything published after you connected".
 *
 * Design:
 * - The runtime wraps a `SolClaimLedgerEventStore`. `append` delegates to the
 *   store (which owns all verification and dedup), then — only when the event
 *   is *newly* stored — publishes it to an in-process Effect `PubSub`. A dedup
 *   (already-seen id) never re-delivers, so each distinct event reaches a
 *   subscriber at most once from the live path.
 * - `subscribe(repositoryCoordinate, options?)` registers a PubSub subscription
 *   BEFORE it returns (scoped, closed with the subscribing scope), so an event
 *   appended immediately after `subscribe` is buffered and delivered — there is
 *   no subscribe/append race. It returns a `Stream` narrowed to that repository
 *   coordinate and the same kind / work-item / author / since predicate the
 *   durable query applies, so a subscriber and a fresh coordinate read filter
 *   identically (single source of truth: `solClaimLedgerEventMatchesOptions`).
 * - The delivered value is already-verified: the store verifies every event on
 *   append, and the published object never leaves the process, so the live path
 *   trusts what the store already admitted rather than re-verifying in memory.
 *
 * Effect / Node host contract throughout; no new dependency and no cross-repo
 * edit (the runtime consumes the store, which consumes the already-pinned
 * `nostr-effect` signer through the relay module).
 */
import { Effect, PubSub, Scope, Stream } from "effect";

import {
  type SignedNostrEvent,
  type SolClaimLedgerFilterOptions,
  type VerifiedSolClaimLedgerEntry,
  SolClaimLedgerRelayFrameError,
  parseRelayEventMessage,
} from "./sol-claim-ledger-relay";
import {
  type SolClaimLedgerAppendError,
  type SolClaimLedgerAppendResult,
  type SolClaimLedgerEventStore,
  solClaimLedgerEventMatchesOptions,
} from "./sol-claim-ledger-store";

// =============================================================================
// Delivered value
// =============================================================================

/**
 * One live delivery to a subscriber: the verified typed ledger entry plus the
 * bounded index fields a filter narrows on. Carries exactly what a subscriber
 * needs to react (the entry) and to have been selected (coordinate + index
 * fields), and nothing else — no secret key ever reaches this shape.
 */
export type PublishedSolClaimLedgerEvent = Readonly<{
  /** The verified typed entry (work item / claim / claim-status / claim-release). */
  entry: VerifiedSolClaimLedgerEntry;
  /** The NIP-01 event id (content address). */
  eventId: string;
  /** The NIP-34 repository coordinate (`30617:<pubkey>:<repoId>`) it was filed under. */
  repositoryCoordinate: string;
  /** The event kind (one of the five Sol claim-ledger kinds). */
  kind: number;
  /** The signer public key. */
  pubkey: string;
  /** The event `created_at` in whole epoch seconds. */
  createdAtEpochSeconds: number;
  /** The Sol work-item ref the entry belongs to. */
  workItemRef: string;
}>;

// =============================================================================
// Runtime
// =============================================================================

/**
 * A live-filter subscription runtime over a durable claim-ledger `EventStore`.
 *
 * `append` / `appendFromRelayMessage` persist through the store and fan a newly
 * stored event out to every matching live subscriber. `subscribe` returns a
 * scoped `Stream` of the events published to one repository coordinate after
 * the subscription registers. `queryRepositoryCoordinate` and `count` pass
 * straight through so the runtime is a drop-in facade over the store: read the
 * durable backlog with a query, then follow the live tail with a subscription.
 */
export interface SolClaimLedgerSubscriptionRuntime {
  readonly append: (
    event: SignedNostrEvent,
  ) => Effect.Effect<SolClaimLedgerAppendResult, SolClaimLedgerAppendError>;
  readonly appendFromRelayMessage: (
    raw: string,
  ) => Effect.Effect<
    SolClaimLedgerAppendResult,
    SolClaimLedgerAppendError | SolClaimLedgerRelayFrameError
  >;
  readonly subscribe: (
    repositoryCoordinate: string,
    options?: SolClaimLedgerFilterOptions,
  ) => Effect.Effect<Stream.Stream<PublishedSolClaimLedgerEvent>, never, Scope.Scope>;
  readonly queryRepositoryCoordinate: SolClaimLedgerEventStore["queryRepositoryCoordinate"];
  readonly count: SolClaimLedgerEventStore["count"];
}

const workItemRefOf = (entry: VerifiedSolClaimLedgerEntry): string => {
  switch (entry.type) {
    case "work_item":
      return entry.workItem.work_item_ref;
    case "claim":
      return entry.claim.work_item_ref;
    case "claim_status":
      return entry.claimStatus.work_item_ref;
    case "claim_release":
      return entry.claimRelease.work_item_ref;
  }
};

const toPublished = (
  event: SignedNostrEvent,
  result: SolClaimLedgerAppendResult,
): PublishedSolClaimLedgerEvent => ({
  entry: result.entry,
  eventId: result.eventId,
  repositoryCoordinate: result.repositoryCoordinate,
  kind: event.kind,
  pubkey: event.pubkey,
  createdAtEpochSeconds: event.created_at,
  workItemRef: workItemRefOf(result.entry),
});

/**
 * A published event matches a subscriber iff it is on the subscriber's
 * repository coordinate AND passes the same kind / work-item / author / since
 * predicate the durable query uses. `limit` is a backlog-cap only and is not
 * meaningful for an open live stream, so it is ignored here.
 */
const matchesSubscriber = (
  published: PublishedSolClaimLedgerEvent,
  repositoryCoordinate: string,
  options: SolClaimLedgerFilterOptions,
): boolean =>
  published.repositoryCoordinate === repositoryCoordinate &&
  solClaimLedgerEventMatchesOptions(
    {
      kind: published.kind,
      pubkey: published.pubkey,
      createdAt: published.createdAtEpochSeconds,
      workItemRef: published.workItemRef,
    },
    options,
  );

/**
 * Assemble a live-filter subscription runtime over a durable `EventStore`.
 *
 * The returned runtime holds one process-local `PubSub` fan-out. It performs no
 * background work of its own, so it needs no scope to construct; each
 * `subscribe` call owns the scope of its own subscription.
 */
export const makeSolClaimLedgerSubscriptionRuntime = (
  store: SolClaimLedgerEventStore,
): Effect.Effect<SolClaimLedgerSubscriptionRuntime> =>
  Effect.gen(function* () {
    const pubsub = yield* PubSub.unbounded<PublishedSolClaimLedgerEvent>();

    // Publish only newly stored events; a dedup (already-seen id) never
    // re-delivers, so each distinct event reaches a subscriber at most once.
    const publishIfNew = (
      event: SignedNostrEvent,
      result: SolClaimLedgerAppendResult,
    ): Effect.Effect<SolClaimLedgerAppendResult> =>
      result.stored
        ? PubSub.publish(pubsub, toPublished(event, result)).pipe(Effect.as(result))
        : Effect.succeed(result);

    const append: SolClaimLedgerSubscriptionRuntime["append"] = (event) =>
      store.append(event).pipe(Effect.flatMap((result) => publishIfNew(event, result)));

    // Route the relay-frame path through the same signed-event append so there
    // is exactly one publish site, and a bad frame is a typed failure.
    const appendFromRelayMessage: SolClaimLedgerSubscriptionRuntime["appendFromRelayMessage"] = (
      raw,
    ) =>
      Effect.try({
        try: () => parseRelayEventMessage(raw),
        catch: (error) =>
          error instanceof SolClaimLedgerRelayFrameError
            ? error
            : new SolClaimLedgerRelayFrameError(
                (error instanceof Error ? error.message : String(error))
                  .replaceAll(/\s+/g, " ")
                  .slice(0, 200),
              ),
      }).pipe(Effect.flatMap(append));

    const subscribe: SolClaimLedgerSubscriptionRuntime["subscribe"] = (
      repositoryCoordinate,
      options = {},
    ) =>
      // Register the subscription eagerly (scoped) so an event appended right
      // after `subscribe` returns is buffered and delivered — no race.
      PubSub.subscribe(pubsub).pipe(
        Effect.map((subscription) =>
          Stream.fromSubscription(subscription).pipe(
            Stream.filter((published) =>
              matchesSubscriber(published, repositoryCoordinate, options),
            ),
          ),
        ),
      );

    return {
      append,
      appendFromRelayMessage,
      subscribe,
      queryRepositoryCoordinate: store.queryRepositoryCoordinate,
      count: store.count,
    } satisfies SolClaimLedgerSubscriptionRuntime;
  });
