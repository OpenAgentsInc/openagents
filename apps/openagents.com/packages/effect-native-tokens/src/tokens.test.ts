import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";
import {
  colorTokens,
  decodeTheme,
  defaultTheme,
  encodeTheme,
  evaluateKhalaDimension,
  khalaPalette,
  khalaTheme,
  resolveKhalaSeparatorPaint,
  resolveKhalaStepsPaint,
  resolveKhalaStripPaint,
  spacingTokens,
  withAlpha,
} from "./index";

// --------------------------------------------------------------------------
// withAlpha — exact opaque-hex + alpha-byte concatenation, strict validation
// --------------------------------------------------------------------------

describe("withAlpha", () => {
  test("concatenates a validated opaque hex with a two-hex alpha byte", () => {
    expect(withAlpha("#3b82f6", "29")).toBe("#3b82f629");
    expect(withAlpha("#02040a", "db")).toBe("#02040adb");
    expect(withAlpha("#8fb3ff", "14")).toBe("#8fb3ff14");
  });

  test("rejects uppercase or three-digit hex (lowercase six-digit only)", () => {
    expect(() => withAlpha("#3B82F6", "29")).toThrow();
    expect(() => withAlpha("#fff", "0a")).toThrow();
    expect(() => withAlpha("3b82f6", "0a")).toThrow();
  });

  test("rejects a malformed alpha byte", () => {
    expect(() => withAlpha("#3b82f6", "2")).toThrow();
    expect(() => withAlpha("#3b82f6", "zz")).toThrow();
    expect(() => withAlpha("#3b82f6", "1a2")).toThrow();
  });
});

// --------------------------------------------------------------------------
// evaluateKhalaDimension — the closed dimension expression language
// --------------------------------------------------------------------------

const runDim = (expression: unknown, basis: unknown): number =>
  Effect.runSync(evaluateKhalaDimension(expression, basis));

// Effect.flip turns a failing effect into a succeeding one carrying the error;
// if the expression unexpectedly SUCCEEDED, flip fails and runSync throws.
const runDimError = (
  expression: unknown,
  basis: unknown,
): { readonly _tag: string; readonly reason?: string } =>
  Effect.runSync(Effect.flip(evaluateKhalaDimension(expression, basis))) as never;

describe("evaluateKhalaDimension", () => {
  test("Literal returns its value; Percentage resolves against the basis", () => {
    expect(runDim({ _tag: "Literal", value: 42 }, 0)).toBe(42);
    expect(runDim({ _tag: "Percentage", value: 50 }, 200)).toBe(100);
    expect(runDim({ _tag: "Percentage", value: 25 }, 80)).toBe(20);
  });

  test("Add and Subtract compute exact sums and differences", () => {
    expect(
      runDim(
        { _tag: "Add", left: { _tag: "Literal", value: 10 }, right: { _tag: "Literal", value: 5 } },
        0,
      ),
    ).toBe(15);
    expect(
      runDim(
        {
          _tag: "Subtract",
          left: { _tag: "Literal", value: 10 },
          right: { _tag: "Literal", value: 5 },
        },
        0,
      ),
    ).toBe(5);
  });

  test("Minimum and Maximum select the bounded operand", () => {
    expect(
      runDim(
        {
          _tag: "Minimum",
          left: { _tag: "Literal", value: 10 },
          right: { _tag: "Literal", value: 5 },
        },
        0,
      ),
    ).toBe(5);
    expect(
      runDim(
        {
          _tag: "Maximum",
          left: { _tag: "Literal", value: 10 },
          right: { _tag: "Literal", value: 5 },
        },
        0,
      ),
    ).toBe(10);
  });

  test("Scale multiplies and Divide divides exactly", () => {
    expect(runDim({ _tag: "Scale", value: { _tag: "Literal", value: 10 }, factor: 2 }, 0)).toBe(20);
    expect(runDim({ _tag: "Divide", value: { _tag: "Literal", value: 20 }, divisor: 4 }, 0)).toBe(
      5,
    );
  });

  test("a divisor of zero fails with KhalaDivisionByZeroError", () => {
    const error = runDimError(
      { _tag: "Divide", value: { _tag: "Literal", value: 10 }, divisor: 0 },
      0,
    );
    expect(error._tag).toBe("KhalaDivisionByZeroError");
    expect(error.reason).toBe("A Khala dimension divisor cannot be zero");
  });

  test("a negative resolved dimension fails with KhalaInvalidDimensionError", () => {
    const error = runDimError(
      {
        _tag: "Subtract",
        left: { _tag: "Literal", value: 5 },
        right: { _tag: "Literal", value: 10 },
      },
      0,
    );
    expect(error._tag).toBe("KhalaInvalidDimensionError");
  });

  test("an over-deep expression tree fails with KhalaExpressionBoundsError", () => {
    let expression: unknown = { _tag: "Literal", value: 1 };
    for (let index = 0; index < 12; index += 1) {
      expression = { _tag: "Add", left: expression, right: { _tag: "Literal", value: 1 } };
    }
    const error = runDimError(expression, 0);
    expect(error._tag).toBe("KhalaExpressionBoundsError");
  });

  test("an unknown expression tag fails during decode", () => {
    // flip only succeeds if the effect FAILED, so a thrown-free run proves the
    // unknown tag was rejected during schema decode.
    const error = Effect.runSync(
      Effect.flip(evaluateKhalaDimension({ _tag: "Multiply", value: 2 }, 0)),
    );
    expect(error).toBeDefined();
    expect(() =>
      Effect.runSync(evaluateKhalaDimension({ _tag: "Multiply", value: 2 }, 0)),
    ).toThrow();
  });
});

// --------------------------------------------------------------------------
// Paint resolvers — exact stop arrays, roles, and clamping
// --------------------------------------------------------------------------

describe("resolveKhalaStepsPaint", () => {
  test("a single step produces one full-width signal band", () => {
    expect(resolveKhalaStepsPaint(1)).toEqual({
      direction: "horizontal",
      repeating: false,
      stops: [
        { offset: 0, role: "signal" },
        { offset: 1, role: "signal" },
      ],
    });
  });

  test("two steps alternate signal and transparent across 2n-1 bands", () => {
    expect(resolveKhalaStepsPaint(2, "vertical", "structural")).toEqual({
      direction: "vertical",
      repeating: false,
      stops: [
        { offset: 0, role: "structural" },
        { offset: 1 / 3, role: "structural" },
        { offset: 1 / 3, role: "transparent" },
        { offset: 2 / 3, role: "transparent" },
        { offset: 2 / 3, role: "structural" },
        { offset: 1, role: "structural" },
      ],
    });
  });

  test("clamps the band count to [1, 32] so length cannot explode or vanish", () => {
    // round(0) -> 0 -> clamped up to 1 band -> 2 stops.
    expect(resolveKhalaStepsPaint(0).stops).toHaveLength(2);
    // 1000 -> clamped down to 32 bands -> total = 32*2-1 = 63 -> 126 stops.
    expect(resolveKhalaStepsPaint(1000).stops).toHaveLength(126);
  });
});

describe("resolveKhalaStripPaint", () => {
  test("empty roles fall back to a single structural band and repeat", () => {
    expect(resolveKhalaStripPaint([])).toEqual({
      direction: "horizontal",
      repeating: true,
      stops: [
        { offset: 0, role: "structural" },
        { offset: 1, role: "structural" },
      ],
    });
  });

  test("bounds the role list to 8 entries (two stops each)", () => {
    const strip = resolveKhalaStripPaint(
      Array.from({ length: 10 }, () => "signal" as const),
      "vertical",
    );
    expect(strip.direction).toBe("vertical");
    expect(strip.repeating).toBe(true);
    expect(strip.stops).toHaveLength(16);
    expect(strip.stops[15]).toEqual({ offset: 1, role: "signal" });
  });
});

describe("resolveKhalaSeparatorPaint", () => {
  test("the default end variant emits a structural head plus the nine-stop tail", () => {
    expect(resolveKhalaSeparatorPaint()).toEqual({
      direction: "horizontal",
      repeating: false,
      stops: [
        { offset: 0, role: "structural" },
        { offset: 0.8, role: "structural" },
        { offset: 0.8, role: "transparent" },
        { offset: 0.88, role: "transparent" },
        { offset: 0.88, role: "signal" },
        { offset: 0.92, role: "signal" },
        { offset: 0.92, role: "transparent" },
        { offset: 0.96, role: "transparent" },
        { offset: 0.96, role: "signal" },
        { offset: 1, role: "signal" },
      ],
    });
  });

  test("the both variant concatenates the six-stop head and nine-stop tail", () => {
    const paint = resolveKhalaSeparatorPaint("both", "vertical");
    expect(paint.direction).toBe("vertical");
    expect(paint.stops).toHaveLength(15);
    expect(paint.stops[0]).toEqual({ offset: 0, role: "signal" });
    expect(paint.stops[14]).toEqual({ offset: 1, role: "signal" });
  });
});

// --------------------------------------------------------------------------
// Theme codec round-trip
// --------------------------------------------------------------------------

describe("theme codec", () => {
  test("encode/decode round-trips khalaTheme to a deeply equal value", () => {
    const roundTripped = decodeTheme(encodeTheme(khalaTheme));
    expect(roundTripped).toEqual(khalaTheme);
  });

  test("decodeTheme rejects a structurally invalid theme", () => {
    expect(() => decodeTheme({ spacing: {}, color: {} })).toThrow();
    expect(() =>
      decodeTheme({
        ...encodeTheme(khalaTheme),
        color: { ...encodeTheme(khalaTheme).color, accent: "not-a-hex" },
      }),
    ).toThrow();
  });
});

// --------------------------------------------------------------------------
// Token-map / palette exact-value pins (drift guards)
// --------------------------------------------------------------------------

describe("token maps", () => {
  test("khalaPalette pins the brand blue and darkest gray/alpha steps", () => {
    expect(khalaPalette.blue["500"]).toBe("#3b82f6");
    expect(khalaPalette.blue["400"]).toBe("#5c96f8");
    expect(khalaPalette.gray["950"]).toBe("#05070d");
    expect(khalaPalette.alpha["16"]).toBe("29");
    expect(khalaPalette.alpha["8"]).toBe("14");
  });

  test("khalaTheme derives semantic roles from exact palette steps", () => {
    expect(khalaTheme.color.accent).toBe("#3b82f6");
    expect(khalaTheme.color.background).toBe("#05070d");
    expect(khalaTheme.color.stateSelected).toBe("#3b82f629");
    expect(khalaTheme.color.scrim).toBe("#02040adb");
  });

  test("defaultTheme keeps its own accent distinct from the khala theme", () => {
    expect(defaultTheme.color.accent).toBe("#2563eb");
  });

  test("token count arrays stay fixed so palette/token drift fails", () => {
    expect(colorTokens).toHaveLength(33);
    expect(colorTokens[0]).toBe("background");
    expect(colorTokens[colorTokens.length - 1]).toBe("syntaxOperator");
    expect(spacingTokens).toHaveLength(22);
  });
});
