import { Effect, Schema as S, Stream } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  closePartialJson,
  emptyPartialJsonAccumulator,
  finalizePartialObject,
  foldPartialJsonDelta,
  isPartialView,
  parsePartialJson,
  streamPartialObjects,
} from "./partial-object-stream.ts";
import type { PartialView } from "./partial-object-stream.ts";

/** The nested target schema for the scripted delta sequences. */
const Recommendation = S.Struct({
  title: S.String,
  score: S.Number,
  author: S.Struct({ name: S.String, active: S.Boolean }),
  tags: S.Array(S.String),
});
type Recommendation = typeof Recommendation.Type;

/** A scripted delta sequence that cuts the JSON text mid-key, mid-string, mid-number, and mid-literal. */
const SCRIPTED_DELTAS: ReadonlyArray<string> = [
  '{"ti',
  'tle": "Par',
  'tial streams", "sc',
  'ore": 4',
  '2, "author": {"na',
  'me": "Ada", "acti',
  've": tr',
  'ue}, "ta',
  'gs": ["al',
  'pha", "be',
  'ta"]}',
];

const FULL_TEXT = SCRIPTED_DELTAS.join("");

const collectViews = (deltas: ReadonlyArray<string>): Promise<Array<PartialView<Recommendation>>> =>
  Effect.runPromise(
    Stream.runCollect(streamPartialObjects(Recommendation, Stream.fromIterable(deltas))),
  );

describe("closePartialJson — repair appends closers only", () => {
  test("closes an open string", () => {
    expect(closePartialJson('{"a": "x')).toBe('{"a": "x"}');
  });

  test("closes nested arrays/objects and completes a started literal", () => {
    expect(closePartialJson('[1, 2, {"b": fa')).toBe('[1, 2, {"b": false}]');
  });

  test("drops trailing incomplete tokens (dangling key, trailing dot, exponent tail)", () => {
    expect(closePartialJson('{"na')).toBe("{}");
    expect(closePartialJson('{"a": 12.')).toBe('{"a": 12}');
    expect(closePartialJson('{"a": 1e+')).toBe('{"a": 1}');
    expect(closePartialJson('{"s": "a\\u12')).toBe('{"s": "a"}');
  });
});

describe("parsePartialJson — total, never throws", () => {
  test("empty and whitespace-only input carry no value", () => {
    expect(parsePartialJson("")).toBeUndefined();
    expect(parsePartialJson("  \n\t ")).toBeUndefined();
  });

  test("complete JSON is an exact parse; a prefix is a repaired parse", () => {
    expect(parsePartialJson('{"a":1}')).toEqual({ value: { a: 1 }, state: "exact" });
    expect(parsePartialJson('{"a":1')).toEqual({ value: { a: 1 }, state: "repaired" });
  });

  test("unrepairable input reports no value instead of throwing", () => {
    expect(parsePartialJson('{"a": 1, "b": tru7')).toBeUndefined();
  });
});

describe("streamPartialObjects — progressive partial views", () => {
  test("fields appear as their JSON completes; unchanged parses emit nothing", async () => {
    const views = await collectViews(SCRIPTED_DELTAS);

    // 11 deltas, 10 views: the `ue}, "ta` delta only closes already-seen
    // values and starts an incomplete key, so its parse does not change.
    expect(views).toHaveLength(10);
    expect(views.every(isPartialView)).toBe(true);

    expect(views[0]!.value).toEqual({});
    expect(views[1]!.value).toEqual({ title: "Par" });
    expect(views[2]!.value).toEqual({ title: "Partial streams" });
    expect(views[3]!.value).toEqual({ title: "Partial streams", score: 4 });
    expect(views[4]!.value).toEqual({
      title: "Partial streams",
      score: 42,
      author: {},
    });
    expect(views[5]!.value).toEqual({
      title: "Partial streams",
      score: 42,
      author: { name: "Ada" },
    });
    expect(views[6]!.value).toEqual({
      title: "Partial streams",
      score: 42,
      author: { name: "Ada", active: true },
    });
    expect(views[7]!.value).toEqual({
      title: "Partial streams",
      score: 42,
      author: { name: "Ada", active: true },
      tags: ["al"],
    });
    expect(views[8]!.value).toEqual({
      title: "Partial streams",
      score: 42,
      author: { name: "Ada", active: true },
      tags: ["alpha", "be"],
    });

    // Every mid-stream view is a repaired parse; only the completed text
    // parses exactly. None of them is a validated value.
    expect(views.slice(0, 9).every((view) => view.state === "repaired")).toBe(true);
    expect(views[9]!.state).toBe("exact");
    expect(views[9]!.value).toEqual(JSON.parse(FULL_TEXT));
  });

  test("a malformed tail degrades to the last good partial and never fails the stream", async () => {
    const Simple = S.Struct({ a: S.Number, b: S.Boolean });
    const views = await Effect.runPromise(
      Stream.runCollect(
        streamPartialObjects(Simple, Stream.fromIterable(['{"a": 1, "b":', " tru7", "%%%"])),
      ),
    );
    // Only the first delta produced a good partial; the poisoned tail emits
    // nothing new and throws nothing.
    expect(views).toHaveLength(1);
    expect(views[0]!.value).toEqual({ a: 1 });
    expect(views[0]!.state).toBe("repaired");
  });

  test("empty and whitespace-only deltas emit no views", async () => {
    const Simple = S.Struct({ a: S.Number });
    const whitespaceOnly = await Effect.runPromise(
      Stream.runCollect(streamPartialObjects(Simple, Stream.fromIterable(["", "  ", "\n"]))),
    );
    expect(whitespaceOnly).toHaveLength(0);

    const mixed = await Effect.runPromise(
      Stream.runCollect(
        streamPartialObjects(Simple, Stream.fromIterable(["", "  ", '{"a"', "", ": 1}"])),
      ),
    );
    expect(mixed.map((view) => view.value)).toEqual([{}, { a: 1 }]);
  });
});

describe("foldPartialJsonDelta — pure accumulator", () => {
  test("degrading keeps the last good partial in the accumulator", () => {
    const first = foldPartialJsonDelta(emptyPartialJsonAccumulator, '{"a": 1, "b":');
    expect(first.next?.value).toEqual({ a: 1 });
    const poisoned = foldPartialJsonDelta(first.accumulator, " tru7");
    expect(poisoned.next).toBeUndefined();
    expect(poisoned.accumulator.lastGood?.value).toEqual({ a: 1 });
  });
});

describe("finalizePartialObject — the only path to a validated value", () => {
  test("a completed stream finalizes to the schema-validated value equal to the plain decode", async () => {
    const finalized = await Effect.runPromise(finalizePartialObject(Recommendation, FULL_TEXT));
    const plain = S.decodeUnknownSync(Recommendation)(JSON.parse(FULL_TEXT));
    expect(finalized).toEqual(plain);
    expect(finalized).toEqual({
      title: "Partial streams",
      score: 42,
      author: { name: "Ada", active: true },
      tags: ["alpha", "beta"],
    });
  });

  test("malformed full text fails the terminal decode with SchemaError", async () => {
    const error = await Effect.runPromise(
      Effect.flip(finalizePartialObject(Recommendation, '{"title": "x", "score":')),
    );
    expect(error._tag).toBe("SchemaError");
  });

  test("schema-violating full text fails even though a partial view existed", async () => {
    const text = '{"title": 7}';
    const views = await collectViews([text]);
    expect(views).toHaveLength(1); // The display affordance exists...
    const error = await Effect.runPromise(Effect.flip(finalizePartialObject(Recommendation, text)));
    expect(error._tag).toBe("SchemaError"); // ...but validation still rejects.
  });
});

describe("PartialView — type-level guard", () => {
  test("a PartialView<T> never satisfies T, and its payload stays unknown", async () => {
    const views = await collectViews(SCRIPTED_DELTAS);
    const view = views[views.length - 1]!;

    // @ts-expect-error — a partial is never a validated value: PartialView<Recommendation> is not a Recommendation.
    const validated: Recommendation = view;
    void validated;

    // @ts-expect-error — the payload is untyped: it has not passed the schema.
    void view.value.title;

    // @ts-expect-error — views of different targets never unify.
    const other: PartialView<{ readonly other: number }> = view;
    void other;

    // The honest runtime read: an unknown payload behind the tagged wrapper.
    expect(view._tag).toBe("PartialView");
    expect(view.value).toEqual(JSON.parse(FULL_TEXT));
  });
});
