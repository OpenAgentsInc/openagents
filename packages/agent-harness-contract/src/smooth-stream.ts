import { Duration, Effect, Stream } from "effect";

/**
 * `smoothStream` — readable-pace text streaming (STREAM-04, #9132).
 *
 * Raw provider deltas arrive in ragged bursts. This operator re-chunks text
 * deltas at word, line, or custom-regex boundaries and paces each re-chunked
 * emission with a configurable delay, so streamed text renders smoothly
 * instead of jittering. The idea is re-derived from the AI SDK `smoothStream`
 * transform (`packages/ai/src/generate-text/smooth-stream.ts` in the `ai`
 * reference repo); the word/line regexes and the regex chunk-detection rule
 * (`buffer.slice(0, match.index) + match[0]`) mirror it exactly. No code is
 * vendored.
 *
 * The operator is deliberately chunk-vocabulary-agnostic: the caller supplies
 * `isTextDelta` (extract accumulate-able text from an element, or `undefined`
 * for pass-through) and `makeTextDelta` (rebuild an element carrying
 * re-chunked text), so the same operator serves the STREAM-02 UiMessageChunk
 * stream, the `HarnessStreamEvent` stream, or any other delta vocabulary.
 *
 * Guarantees:
 *
 * - **Byte identity.** Concatenating the text of every emitted text element
 *   equals concatenating the text of every input text element. Pacing and
 *   re-chunking never alter content.
 * - **Non-text is never paced.** An element for which `isTextDelta` returns
 *   `undefined` (tool call, finish, error, ...) flushes the pending text
 *   buffer immediately (remainder emitted unpaced) and passes through with no
 *   delay of its own.
 * - **End-of-stream flush.** Any text still buffered when the stream ends is
 *   emitted (unpaced) before the stream completes.
 *
 * Pacing detail: each ready chunk is emitted after an `Effect.sleep(delay)`
 * (`delay` BEFORE each paced emission). The AI SDK delays AFTER each enqueue,
 * which leaves a trailing delay that would hold up a following non-text
 * element; sleeping before each paced chunk keeps the inter-chunk spacing
 * identical while a non-text element or the final flush never waits on a
 * trailing text delay. `Stream.throttle` was considered and rejected: its
 * token bucket paces whole chunks of the combined stream, so it cannot exempt
 * non-text elements from pacing. `Effect.sleep` is fully deterministic under
 * the `TestClock`.
 *
 * On stream failure the pending buffer is dropped with the failure (mirrors
 * the AI SDK transform, which only flushes on normal completion).
 */

/**
 * Chunk boundary selection. `"word"` emits up to each trailing-whitespace
 * boundary (`/\S+\s+/m`, as in the AI SDK), `"line"` up to each newline run
 * (`/\n+/m`), or supply a custom `RegExp` whose first match ends the chunk
 * (the chunk is the buffer prefix through the end of the match).
 */
export type SmoothStreamChunking = "word" | "line" | RegExp;

/** Options for {@link smoothStream}. */
export interface SmoothStreamOptions<A> {
  /** Boundary rule for re-chunking buffered text. Default `"word"`. */
  readonly chunking?: SmoothStreamChunking;
  /**
   * Delay before each paced text emission. Default 10 milliseconds (the AI
   * SDK default). `Duration.zero` disables pacing while keeping re-chunking.
   */
  readonly delay?: Duration.Input;
  /**
   * Extract the accumulate-able text of an element, or `undefined` when the
   * element must pass through unpaced (tool calls, finish, errors, ...). An
   * empty string is still text: it accumulates (a no-op) and does not flush.
   */
  readonly isTextDelta: (element: A) => string | undefined;
  /**
   * Rebuild a text element carrying re-chunked `text`. `template` is the most
   * recent input text element, so identifiers and metadata carried by the
   * element survive re-chunking (the AI SDK equivalently re-enqueues with the
   * current `id`/`providerMetadata`).
   */
  readonly makeTextDelta: (template: A, text: string) => A;
}

/**
 * Chunking regexes, mirrored from the AI SDK `CHUNKING_REGEXPS`
 * (`word: /\S+\s+/m`, `line: /\n+/m`).
 */
const WORD_REGEX = /\S+\s+/m;
const LINE_REGEX = /\n+/m;

/**
 * Build the chunk detector for a chunking rule. Returns the buffer prefix up
 * to and including the first match, or `undefined` when no complete chunk is
 * buffered — the AI SDK regex branch rule.
 */
const detectorFor = (chunking: SmoothStreamChunking) => {
  const regex = chunking === "word" ? WORD_REGEX : chunking === "line" ? LINE_REGEX : chunking;
  return (buffer: string): string | undefined => {
    // A caller-supplied global/sticky regex is stateful via `lastIndex`;
    // reset so detection always scans from the start of the buffer.
    regex.lastIndex = 0;
    const match = regex.exec(buffer);
    if (match === null) return undefined;
    return buffer.slice(0, match.index) + match[0];
  };
};

/**
 * Re-chunk and pace text deltas; pass every other element through unpaced.
 * See the module doc above for the exact semantics.
 *
 * ```ts
 * const smoothed = deltas.pipe(
 *   smoothStream({
 *     delay: Duration.millis(10),
 *     isTextDelta: (chunk) => (chunk.kind === "text-delta" ? chunk.text : undefined),
 *     makeTextDelta: (template, text) => ({ ...template, text }),
 *   }),
 * );
 * ```
 */
export const smoothStream =
  <A>(options: SmoothStreamOptions<A>) =>
  <E, R>(self: Stream.Stream<A, E, R>): Stream.Stream<A, E, R> =>
    Stream.suspend(() => {
      const detect = detectorFor(options.chunking ?? "word");
      const delay = options.delay ?? Duration.millis(10);
      const paced = Duration.toMillis(delay) > 0;

      // Mutable accumulation state. `Stream.suspend` re-runs this thunk on
      // every run of the returned stream, so each run gets fresh state.
      let buffer = "";
      let template: A | undefined = undefined;

      /** Pop every complete chunk currently in the buffer, in order. */
      const drainReady = (): Array<A> => {
        const out: Array<A> = [];
        let chunk: string | undefined;
        while (template !== undefined && (chunk = detect(buffer)) !== undefined) {
          out.push(options.makeTextDelta(template, chunk));
          buffer = buffer.slice(chunk.length);
        }
        return out;
      };

      /** Emit the incomplete remainder (non-text boundary or end of stream). */
      const flushRemainder = (): Array<A> => {
        if (buffer.length === 0 || template === undefined) return [];
        const remainder = options.makeTextDelta(template, buffer);
        buffer = "";
        return [remainder];
      };

      /** Ready chunks, each preceded by the pacing delay. */
      const pacedChunks = (chunks: ReadonlyArray<A>): Stream.Stream<A> =>
        paced
          ? Stream.fromIterable(chunks).pipe(
              Stream.mapEffect((chunk) => Effect.as(Effect.sleep(delay), chunk)),
            )
          : Stream.fromIterable(chunks);

      const transformed = self.pipe(
        Stream.flatMap((element) =>
          // Sequential (default concurrency 1): state mutation happens at
          // inner-stream run time, in input order.
          Stream.suspend(() => {
            const text = options.isTextDelta(element);
            if (text === undefined) {
              // Non-text: flush pending text, pass through — no pacing.
              return Stream.fromIterable([...flushRemainder(), element]);
            }
            buffer += text;
            template = element;
            return pacedChunks(drainReady());
          }),
        ),
      );

      // End-of-stream flush of whatever never reached a chunk boundary.
      return Stream.concat(
        transformed,
        Stream.suspend(() => Stream.fromIterable(flushRemainder())),
      );
    });
