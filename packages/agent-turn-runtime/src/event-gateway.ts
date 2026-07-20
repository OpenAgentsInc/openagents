import { Effect, Queue, Ref, Stream } from "effect";

/**
 * AFS-01 bounded, generation-fenced event gateway.
 *
 * A provider stream is untrusted about volume and timing. The kernel never lets
 * it grow an unbounded queue and never lets a superseded run mutate the active
 * turn. This gateway is the single main-owned transport between a provider
 * producer and the turn consumer:
 *
 * - It is bounded. A burst beyond capacity is dropped and counted, never queued
 *   without limit.
 * - It is generation-fenced. Every offer names its generation; an offer whose
 *   generation does not match the active generation is fenced and counted, and
 *   never reaches a consumer.
 *
 * The gateway carries no UI, no platform API, and no provider SDK type.
 */
export interface TurnEventGateway<A> {
  /**
   * Offer one item at a generation. Returns `true` only when the item was
   * accepted into the bounded queue at the active generation. A fenced or
   * capacity-dropped item returns `false` and is counted.
   */
  readonly offer: (generation: number, item: A) => Effect.Effect<boolean>;
  /** Advance the active generation. Every later offer at an older generation fences. */
  readonly setGeneration: (generation: number) => Effect.Effect<void>;
  /** The active generation. */
  readonly generation: Effect.Effect<number>;
  /** The accepted, in-generation item stream. It ends when the gateway shuts down. */
  readonly stream: Stream.Stream<A>;
  /** Items dropped because the bounded queue was full. */
  readonly droppedCount: Effect.Effect<number>;
  /** Items fenced because their generation did not match the active generation. */
  readonly fencedCount: Effect.Effect<number>;
  /** Current queued depth. It never exceeds capacity. */
  readonly depth: Effect.Effect<number>;
  /** Close the gateway; the consumer stream then ends. */
  readonly shutdown: Effect.Effect<void>;
}

/**
 * Create a scoped bounded, generation-fenced gateway. The backing queue is
 * released with the owning scope, so closing the turn scope frees the transport.
 */
export const makeTurnEventGateway = <A>(
  capacity: number,
  initialGeneration = 0,
): Effect.Effect<TurnEventGateway<A>, never, never> =>
  Effect.gen(function* () {
    const queue = yield* Queue.dropping<A>(capacity);
    const generationRef = yield* Ref.make(initialGeneration);
    const dropped = yield* Ref.make(0);
    const fenced = yield* Ref.make(0);

    const offer = (generation: number, item: A): Effect.Effect<boolean> =>
      Effect.gen(function* () {
        const active = yield* Ref.get(generationRef);
        if (generation !== active) {
          yield* Ref.update(fenced, (count) => count + 1);
          return false;
        }
        const accepted = yield* Queue.offer(queue, item);
        if (!accepted) yield* Ref.update(dropped, (count) => count + 1);
        return accepted;
      });

    return {
      offer,
      setGeneration: (generation: number) => Ref.set(generationRef, generation),
      generation: Ref.get(generationRef),
      stream: Stream.fromQueue(queue),
      droppedCount: Ref.get(dropped),
      fencedCount: Ref.get(fenced),
      depth: Queue.size(queue),
      shutdown: Queue.shutdown(queue),
    };
  });
