import { Duration, Effect, Fiber, Ref, Stream } from "effect";
import { TestClock } from "effect/testing";
import { describe, expect, test } from "vite-plus/test";

import { smoothStream } from "./smooth-stream.ts";

/** Minimal chunk vocabulary for the generic operator. */
type TestPart =
  | { readonly kind: "text-delta"; readonly id: string; readonly text: string }
  | { readonly kind: "tool-call"; readonly name: string }
  | { readonly kind: "finish" };

const text = (t: string, id = "msg-1"): TestPart => ({ kind: "text-delta", id, text: t });
const tool = (name: string): TestPart => ({ kind: "tool-call", name });
const finish: TestPart = { kind: "finish" };

const isTextDelta = (part: TestPart): string | undefined =>
  part.kind === "text-delta" ? part.text : undefined;

const makeTextDelta = (template: TestPart, t: string): TestPart => ({
  ...(template as Extract<TestPart, { kind: "text-delta" }>),
  text: t,
});

/** Concatenated text carried by the text elements of a part list. */
const joinedText = (parts: ReadonlyArray<TestPart>): string =>
  parts.map((p) => (p.kind === "text-delta" ? p.text : "")).join("");

const texts = (parts: ReadonlyArray<TestPart>): Array<string> =>
  parts.flatMap((p) => (p.kind === "text-delta" ? [p.text] : []));

/** Unpaced smoothing (re-chunking only) for boundary tests. */
const smoothZero = (options?: { chunking?: "word" | "line" | RegExp }) =>
  smoothStream<TestPart>({ ...options, delay: Duration.zero, isTextDelta, makeTextDelta });

describe("smoothStream — re-chunking boundaries", () => {
  test("word chunking re-emits ragged deltas at trailing-whitespace boundaries", async () => {
    const input = [text("Hel"), text("lo wor"), text("ld and"), text(" more"), text(" tail")];
    const out = await Effect.runPromise(
      Stream.runCollect(Stream.fromIterable(input).pipe(smoothZero())),
    );
    expect(texts(out)).toEqual(["Hello ", "world ", "and ", "more ", "tail"]);
  });

  test("word chunking keeps leading whitespace with the chunk (AI SDK prefix rule)", async () => {
    const input = [text("  double"), text("  spaced")];
    const out = await Effect.runPromise(
      Stream.runCollect(Stream.fromIterable(input).pipe(smoothZero())),
    );
    // `/\S+\s+/m` matches "double  "; the chunk is the buffer prefix through
    // the match, so the leading "  " rides along and nothing is dropped.
    expect(texts(out)).toEqual(["  double  ", "spaced"]);
    expect(joinedText(out)).toBe(joinedText(input));
  });

  test("line chunking emits up to each newline run", async () => {
    const input = [text("alpha\nbe"), text("ta\n\n"), text("gamma")];
    const out = await Effect.runPromise(
      Stream.runCollect(Stream.fromIterable(input).pipe(smoothZero({ chunking: "line" }))),
    );
    expect(texts(out)).toEqual(["alpha\n", "beta\n\n", "gamma"]);
  });

  test("custom RegExp chunking splits at each match", async () => {
    const input = [text("a,b"), text(",c")];
    const out = await Effect.runPromise(
      Stream.runCollect(Stream.fromIterable(input).pipe(smoothZero({ chunking: /[^,]*,/ }))),
    );
    expect(texts(out)).toEqual(["a,", "b,", "c"]);
  });

  test("re-chunked output text is byte-identical to input text", async () => {
    const input = [
      text("Hel"),
      text("lo  wor"),
      tool("read_file"),
      text("ld\n\nwith  unicode \u{1F680} an"),
      text("d a trailing fragment"),
      finish,
    ];
    const out = await Effect.runPromise(
      Stream.runCollect(Stream.fromIterable(input).pipe(smoothZero())),
    );
    expect(joinedText(out)).toBe(joinedText(input));
  });

  test("chunks carry the most recent text element as template", async () => {
    const input = [text("one ", "id-a"), text("two", "id-b")];
    const out = await Effect.runPromise(
      Stream.runCollect(Stream.fromIterable(input).pipe(smoothZero())),
    );
    expect(out).toEqual([text("one ", "id-a"), text("two", "id-b")]);
  });
});

describe("smoothStream — non-text pass-through and flush", () => {
  test("a non-text element flushes the pending buffer and passes through in order", async () => {
    const input = [text("hel"), text("lo wo"), tool("bash"), text("rld!"), finish];
    const out = await Effect.runPromise(
      Stream.runCollect(Stream.fromIterable(input).pipe(smoothZero())),
    );
    expect(out).toEqual([text("hello "), text("wo"), tool("bash"), text("rld!"), finish]);
    expect(joinedText(out)).toBe(joinedText(input));
  });

  test("end of stream flushes the incomplete remainder", async () => {
    const input = [text("no boundary her"), text("e")];
    const out = await Effect.runPromise(
      Stream.runCollect(Stream.fromIterable(input).pipe(smoothZero())),
    );
    expect(texts(out)).toEqual(["no ", "boundary ", "here"]);
  });

  test("an empty stream and a non-text-only stream emit no synthetic elements", async () => {
    const none = await Effect.runPromise(Stream.runCollect(Stream.empty.pipe(smoothZero())));
    expect(none).toEqual([]);

    // Delay is one hour and this runs on the LIVE clock: it completes only
    // because non-text elements never schedule a pacing sleep.
    const passThrough = await Effect.runPromise(
      Stream.runCollect(
        Stream.fromIterable<TestPart>([tool("bash"), finish]).pipe(
          smoothStream({ delay: Duration.hours(1), isTextDelta, makeTextDelta }),
        ),
      ),
    );
    expect(passThrough).toEqual([tool("bash"), finish]);
  });
});

describe("smoothStream — pacing under TestClock", () => {
  test("emissions wait for the delay; non-text is released without extra pacing", async () => {
    const delayMs = 100;
    const program = Effect.gen(function* () {
      const seen = yield* Ref.make<ReadonlyArray<TestPart>>([]);
      const input: ReadonlyArray<TestPart> = [text("hello world and "), tool("bash")];
      const fiber = yield* Stream.fromIterable(input).pipe(
        smoothStream({
          delay: Duration.millis(delayMs),
          isTextDelta,
          makeTextDelta,
        }),
        Stream.runForEach((part) => Ref.update(seen, (parts) => [...parts, part])),
        Effect.forkChild,
      );

      // Let the collector run to its first pacing sleep.
      yield* Effect.yieldNow;
      yield* Effect.yieldNow;
      const snapshots: Array<ReadonlyArray<TestPart>> = [];
      snapshots.push(yield* Ref.get(seen));

      // One millisecond short of the delay: pacing actually waits.
      yield* TestClock.adjust(Duration.millis(delayMs - 1));
      snapshots.push(yield* Ref.get(seen));

      // Chunk 1 at t = 100 ms.
      yield* TestClock.adjust(Duration.millis(1));
      snapshots.push(yield* Ref.get(seen));

      // Chunk 2 at t = 200 ms.
      yield* TestClock.adjust(Duration.millis(delayMs));
      snapshots.push(yield* Ref.get(seen));

      // Chunk 3 at t = 300 ms — and the tool call in the SAME instant,
      // because non-text pass-through adds no pacing of its own.
      yield* TestClock.adjust(Duration.millis(delayMs));
      snapshots.push(yield* Ref.get(seen));

      yield* Fiber.join(fiber);
      return snapshots;
    });

    const snapshots = await Effect.runPromise(program.pipe(Effect.provide(TestClock.layer())));
    expect(snapshots[0]).toEqual([]);
    expect(snapshots[1]).toEqual([]);
    expect(snapshots[2]).toEqual([text("hello ")]);
    expect(snapshots[3]).toEqual([text("hello "), text("world ")]);
    expect(snapshots[4]).toEqual([text("hello "), text("world "), text("and "), tool("bash")]);
  });

  test("paced output remains byte-identical and correctly ordered", async () => {
    const program = Effect.gen(function* () {
      const input: ReadonlyArray<TestPart> = [
        text("alpha be"),
        text("ta gam"),
        tool("read_file"),
        text("ma delta"),
        finish,
      ];
      const fiber = yield* Stream.fromIterable(input).pipe(
        smoothStream({ delay: Duration.millis(10), isTextDelta, makeTextDelta }),
        Stream.runCollect,
        Effect.forkChild,
      );
      // More than enough virtual time for every paced chunk.
      yield* TestClock.adjust(Duration.seconds(10));
      const out = yield* Fiber.join(fiber);
      return { input, out };
    });

    const { input, out } = await Effect.runPromise(program.pipe(Effect.provide(TestClock.layer())));
    expect(texts(out)).toEqual(["alpha ", "beta ", "gam", "ma ", "delta"]);
    expect(out.filter((p) => p.kind !== "text-delta")).toEqual([tool("read_file"), finish]);
    expect(joinedText(out)).toBe(joinedText(input));
  });
});
