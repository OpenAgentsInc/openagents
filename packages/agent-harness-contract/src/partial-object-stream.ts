import type { Effect } from "effect";
import { Schema as S, Stream } from "effect";

/**
 * Partial-object streaming for guided structured output (STREAM-06, #9134).
 *
 * A structured-generation lane emits the JSON text of a target schema as an
 * incremental sequence of text deltas. This module folds those deltas into
 * best-effort PARTIAL views of the target object so a surface can show
 * progress while the lane is still emitting.
 *
 * The one law of this module: **a partial is never a validated value.**
 *
 * - A partial view is typed as {@link PartialView} — a distinct wrapper whose
 *   payload stays `unknown`. The type system rejects a `PartialView<A>` where
 *   a validated `A` is required.
 * - The ONLY path to a validated `A` is {@link finalizePartialObject}, the
 *   full Schema decode of the complete text. Terminal authority stays with
 *   that decode. Routing, acceptance, and policy gates must consume only the
 *   validated value, never a partial view.
 * - A malformed accumulation DEGRADES: the fold keeps the last good partial
 *   and emits nothing new. It never throws mid-stream, and it never invents
 *   object fields — repair appends only closing delimiters (`"`,`}`,`]`) and
 *   the unambiguous remainder of an already-started JSON literal
 *   (`true` / `false` / `null`).
 *
 * The partial-JSON repair below is an honest small re-derivation of the AI
 * SDK's partial-object machinery (studied as reference, not vendored):
 * `packages/ai/src/util/fix-json.ts` (single linear scan over the states of
 * the JSON grammar, cut at the last valid index, append closers from the open
 * scan stack) and `packages/ai/src/util/parse-partial-json.ts` (try the exact
 * parse first, then parse the repaired text, else report failure).
 */

// ---------------------------------------------------------------------------
// Partial-JSON repair (fix-json re-derivation)
// ---------------------------------------------------------------------------

/**
 * One frame of the scan. The names track the JSON grammar states
 * (https://www.json.org). Exactly one frame is on the stack per open
 * container, so the closing pass appends exactly one closer per frame.
 */
type ScanFrame =
  | "root"
  | "complete"
  | "string"
  | "string-escape"
  | "string-unicode"
  | "literal"
  | "number"
  | "object-open"
  | "object-key"
  | "object-key-escape"
  | "object-colon"
  | "object-value"
  | "object-next"
  | "object-member"
  | "array-open"
  | "array-next"
  | "array-item";

const isDigit = (char: string): boolean => char >= "0" && char <= "9";

const isHexDigit = (char: string): boolean =>
  isDigit(char) || (char >= "a" && char <= "f") || (char >= "A" && char <= "F");

const JSON_LITERALS = ["true", "false", "null"] as const;

/**
 * Close an incomplete JSON prefix into parseable JSON.
 *
 * A single linear scan finds the last index at which the input can still be
 * completed by appending closers only. The result is the input cut at that
 * index plus one closer per open frame: `"` for an open string, `}` for an
 * open object, `]` for an open array, and the remainder of an already-started
 * literal. A trailing incomplete token (a dangling key, a lone `:`/`,`, a
 * bare `-`, a trailing exponent marker) is dropped by the cut.
 *
 * Invalid (non-prefix) input is not repaired: the closed text may still fail
 * `JSON.parse`, and {@link parsePartialJson} then reports no value.
 */
export const closePartialJson = (input: string): string => {
  const stack: Array<ScanFrame> = ["root"];
  let lastValidIndex = -1;
  let literalStart = -1;
  let unicodeDigits = 0;

  /** Start a value: replace the current frame with `continuation`, then push the value frame. */
  const openValue = (char: string, index: number, continuation: ScanFrame): void => {
    switch (char) {
      case '"': {
        lastValidIndex = index;
        stack.pop();
        stack.push(continuation, "string");
        break;
      }
      case "t":
      case "f":
      case "n": {
        lastValidIndex = index;
        literalStart = index;
        stack.pop();
        stack.push(continuation, "literal");
        break;
      }
      case "-": {
        // A bare minus cannot be cut into a number yet.
        stack.pop();
        stack.push(continuation, "number");
        break;
      }
      case "{": {
        lastValidIndex = index;
        stack.pop();
        stack.push(continuation, "object-open");
        break;
      }
      case "[": {
        lastValidIndex = index;
        stack.pop();
        stack.push(continuation, "array-open");
        break;
      }
      default: {
        if (isDigit(char)) {
          lastValidIndex = index;
          stack.pop();
          stack.push(continuation, "number");
        }
        // Whitespace (or garbage) before a value: no transition.
        break;
      }
    }
  };

  const afterObjectValue = (char: string, index: number): void => {
    if (char === ",") {
      stack.pop();
      stack.push("object-member");
    } else if (char === "}") {
      lastValidIndex = index;
      stack.pop();
    }
  };

  const afterArrayValue = (char: string, index: number): void => {
    if (char === ",") {
      stack.pop();
      stack.push("array-item");
    } else if (char === "]") {
      lastValidIndex = index;
      stack.pop();
    }
  };

  for (let i = 0; i < input.length; i++) {
    const char = input[i]!;
    const frame = stack[stack.length - 1]!;

    switch (frame) {
      case "root": {
        openValue(char, i, "complete");
        break;
      }
      case "complete": {
        // Trailing input after a complete root value is ignored.
        break;
      }
      case "object-open": {
        if (char === '"') {
          stack.pop();
          stack.push("object-key");
        } else if (char === "}") {
          lastValidIndex = i;
          stack.pop();
        }
        break;
      }
      case "object-member": {
        if (char === '"') {
          stack.pop();
          stack.push("object-key");
        }
        break;
      }
      case "object-key": {
        // A key never advances the valid index: an incomplete key is dropped
        // whole, because `{"na"` cannot be closed into valid JSON.
        if (char === '"') {
          stack.pop();
          stack.push("object-colon");
        } else if (char === "\\") {
          stack.push("object-key-escape");
        }
        break;
      }
      case "object-key-escape": {
        stack.pop();
        break;
      }
      case "object-colon": {
        if (char === ":") {
          stack.pop();
          stack.push("object-value");
        }
        break;
      }
      case "object-value": {
        openValue(char, i, "object-next");
        break;
      }
      case "object-next": {
        afterObjectValue(char, i);
        break;
      }
      case "array-open": {
        if (char === "]") {
          lastValidIndex = i;
          stack.pop();
        } else {
          lastValidIndex = i;
          openValue(char, i, "array-next");
        }
        break;
      }
      case "array-next": {
        if (char === ",") {
          stack.pop();
          stack.push("array-item");
        } else if (char === "]") {
          lastValidIndex = i;
          stack.pop();
        } else {
          lastValidIndex = i;
        }
        break;
      }
      case "array-item": {
        openValue(char, i, "array-next");
        break;
      }
      case "string": {
        if (char === '"') {
          lastValidIndex = i;
          stack.pop();
        } else if (char === "\\") {
          stack.push("string-escape");
        } else {
          lastValidIndex = i;
        }
        break;
      }
      case "string-escape": {
        stack.pop();
        if (char === "u") {
          unicodeDigits = 0;
          stack.push("string-unicode");
        } else {
          lastValidIndex = i;
        }
        break;
      }
      case "string-unicode": {
        if (isHexDigit(char)) {
          unicodeDigits++;
          if (unicodeDigits === 4) {
            stack.pop();
            lastValidIndex = i;
          }
        }
        break;
      }
      case "number": {
        if (isDigit(char)) {
          lastValidIndex = i;
        } else if (char === "e" || char === "E" || char === "." || char === "-" || char === "+") {
          // A number cannot be cut directly after these; wait for a digit.
        } else if (char === "," || char === "}" || char === "]") {
          stack.pop();
          const parent = stack[stack.length - 1];
          if (parent === "object-next") {
            afterObjectValue(char, i);
          } else if (parent === "array-next") {
            afterArrayValue(char, i);
          }
        } else {
          stack.pop();
        }
        break;
      }
      case "literal": {
        const partial = input.slice(literalStart, i + 1);
        if (JSON_LITERALS.some((literal) => literal.startsWith(partial))) {
          lastValidIndex = i;
        } else {
          stack.pop();
          const parent = stack[stack.length - 1];
          if (parent === "object-next") {
            afterObjectValue(char, i);
          } else if (parent === "array-next") {
            afterArrayValue(char, i);
          }
        }
        break;
      }
    }
  }

  let result = input.slice(0, lastValidIndex + 1);

  for (let i = stack.length - 1; i >= 0; i--) {
    switch (stack[i]!) {
      case "string": {
        result += '"';
        break;
      }
      case "object-open":
      case "object-key":
      case "object-colon":
      case "object-value":
      case "object-next":
      case "object-member": {
        result += "}";
        break;
      }
      case "array-open":
      case "array-next":
      case "array-item": {
        result += "]";
        break;
      }
      case "literal": {
        const partial = input.slice(literalStart);
        const match = JSON_LITERALS.find((literal) => literal.startsWith(partial));
        if (match !== undefined) {
          result += match.slice(partial.length);
        }
        break;
      }
      // "string-escape", "string-unicode", and "object-key-escape" sit on top
      // of the frame that owns the closer; they add nothing themselves.
      default: {
        break;
      }
    }
  }

  return result;
};

// ---------------------------------------------------------------------------
// Partial-JSON parse
// ---------------------------------------------------------------------------

/** How a partial value was obtained. `"exact"` parsed as-is; `"repaired"` parsed after {@link closePartialJson}. */
export type PartialParseState = "exact" | "repaired";

/** A best-effort partial JSON value. NOT schema-validated. */
export interface PartialJsonValue {
  readonly value: unknown;
  readonly state: PartialParseState;
}

/**
 * Parse accumulated structured-output text into a best-effort partial value.
 * Returns `undefined` for empty/whitespace input and for text that stays
 * unparseable after repair — the caller degrades to its last good partial.
 * Never throws.
 */
export const parsePartialJson = (text: string): PartialJsonValue | undefined => {
  if (text.trim().length === 0) {
    return undefined;
  }
  try {
    return { value: JSON.parse(text), state: "exact" };
  } catch {
    // Not complete JSON yet — try the repaired form.
  }
  const repaired = closePartialJson(text);
  if (repaired.length === 0) {
    return undefined;
  }
  try {
    return { value: JSON.parse(repaired), state: "repaired" };
  } catch {
    return undefined;
  }
};

// ---------------------------------------------------------------------------
// Delta accumulator
// ---------------------------------------------------------------------------

/** Fold state for {@link foldPartialJsonDelta}: accumulated text plus the last good partial. */
export interface PartialJsonAccumulator {
  readonly text: string;
  readonly lastGood: PartialJsonValue | undefined;
  /** Stable serialization of `lastGood.value`, used for change detection. */
  readonly lastGoodKey: string | undefined;
}

export const emptyPartialJsonAccumulator: PartialJsonAccumulator = {
  text: "",
  lastGood: undefined,
  lastGoodKey: undefined,
};

export interface PartialJsonFoldResult {
  readonly accumulator: PartialJsonAccumulator;
  /** The NEW good partial when this delta changed the parse; `undefined` on no change or degrade. */
  readonly next: PartialJsonValue | undefined;
}

/**
 * Fold one text delta into the accumulator. Pure and total: a delta that
 * makes the accumulation unparseable degrades (keeps the last good partial,
 * emits nothing) instead of throwing.
 */
export const foldPartialJsonDelta = (
  accumulator: PartialJsonAccumulator,
  delta: string,
): PartialJsonFoldResult => {
  if (delta.length === 0) {
    return { accumulator, next: undefined };
  }
  const text = accumulator.text + delta;
  const parsed = parsePartialJson(text);
  if (parsed === undefined) {
    return {
      accumulator: {
        text,
        lastGood: accumulator.lastGood,
        lastGoodKey: accumulator.lastGoodKey,
      },
      next: undefined,
    };
  }
  const key = JSON.stringify(parsed.value);
  if (key === accumulator.lastGoodKey) {
    return {
      accumulator: {
        text,
        lastGood: accumulator.lastGood,
        lastGoodKey: accumulator.lastGoodKey,
      },
      next: undefined,
    };
  }
  return {
    accumulator: { text, lastGood: parsed, lastGoodKey: key },
    next: parsed,
  };
};

// ---------------------------------------------------------------------------
// PartialView — the type guard
// ---------------------------------------------------------------------------

/**
 * A progress view of a not-yet-validated `A`.
 *
 * LAW: a partial is a display/progress affordance only — NEVER a validated
 * value. The payload is deliberately `unknown` and the wrapper is a distinct
 * tagged shape, so a `PartialView<A>` can never flow where a validated `A`
 * is required, and two views of different targets never unify. The only path
 * to a validated `A` is {@link finalizePartialObject}.
 */
export interface PartialView<in out A> {
  readonly _tag: "PartialView";
  /** The best-effort partial payload. Untyped on purpose: it has NOT passed the schema. */
  readonly value: unknown;
  readonly state: PartialParseState;
  /** Phantom link to the target schema type. Never present at runtime. */
  readonly "~partialTarget"?: (_: A) => A;
}

const toPartialView = <A>(parsed: PartialJsonValue): PartialView<A> => ({
  _tag: "PartialView",
  value: parsed.value,
  state: parsed.state,
});

/** Runtime guard for a {@link PartialView} wrapper. */
export const isPartialView = (input: unknown): input is PartialView<unknown> =>
  typeof input === "object" &&
  input !== null &&
  "_tag" in input &&
  (input as { readonly _tag: unknown })._tag === "PartialView";

// ---------------------------------------------------------------------------
// Stream combinator + terminal decode
// ---------------------------------------------------------------------------

/**
 * Fold a stream of structured-output text deltas into a stream of partial
 * views of `schema`'s type. A new view is emitted only when the accumulated
 * parse changes. Malformed accumulation degrades (the last emitted view
 * stands); the stream never fails from parse trouble. The schema argument
 * anchors the view's target type only — no partial is ever decoded by it.
 */
export const streamPartialObjects = <Sch extends S.Top, E, R>(
  schema: Sch,
  textDeltas: Stream.Stream<string, E, R>,
): Stream.Stream<PartialView<Sch["Type"]>, E, R> => {
  void schema; // Type-level anchor only: partials are never validated.
  return Stream.mapAccum(
    textDeltas,
    () => emptyPartialJsonAccumulator,
    (accumulator, delta) => {
      const folded = foldPartialJsonDelta(accumulator, delta);
      return [
        folded.accumulator,
        folded.next === undefined ? [] : [toPartialView<Sch["Type"]>(folded.next)],
      ] as const;
    },
  );
};

/**
 * The ONLY path from streamed text to a validated value: the full Schema
 * decode of the complete text (`S.decodeUnknownEffect` over
 * `S.fromJsonString`). Fails with `SchemaError` for malformed JSON and for
 * schema violations. No partial view ever substitutes for this decode.
 */
export const finalizePartialObject = <Sch extends S.Top>(
  schema: Sch,
  fullText: string,
): Effect.Effect<Sch["Type"], S.SchemaError, Sch["DecodingServices"]> =>
  S.decodeUnknownEffect(S.fromJsonString(schema))(fullText);
