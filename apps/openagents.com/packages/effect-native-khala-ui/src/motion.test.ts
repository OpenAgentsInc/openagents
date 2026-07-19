import { describe, expect, test } from "vite-plus/test";
import {
  composeKhalaChoreographyPlans,
  checkKhalaChoreographyModel,
  planKhalaChoreography,
} from "./choreography.js";
import { khalaKeyframeToDom } from "./dom-motion.js";
import { khalaSvgIlluminationGradientId, makeKhalaIlluminationNativePlan } from "./illumination.js";
import {
  khalaEaseAmong,
  khalaEaseSteps,
  khalaEasingNames,
  khalaEasings,
  resolveKhalaMotionKeyframes,
  sampleKhalaEasing,
} from "./motion.js";
import {
  khalaCipherCharacters,
  khalaTextDurationMillis,
  makeKhalaTextDecipherFrames,
  makeKhalaTextSequenceFrames,
} from "./text.js";

describe("motion easings", () => {
  test("every easing pins the 0 and 1 endpoints", () => {
    // 31 named easings; sine/bounce/etc. carry float dust, so pin to 1e-12.
    expect(khalaEasingNames.length).toBe(31);
    for (const name of khalaEasingNames) {
      expect(khalaEasings[name](0)).toBeCloseTo(0, 12);
      expect(khalaEasings[name](1)).toBeCloseTo(1, 12);
    }
    // A representative polynomial easing is EXACT at both ends.
    expect(khalaEasings.inQuad(0)).toBe(0);
    expect(khalaEasings.inQuad(1)).toBe(1);
    expect(khalaEasings.outBounce(1)).toBe(1);
  });

  test("polynomial easings hit exact midpoint/anchor values", () => {
    expect(khalaEasings.linear(0.5)).toBe(0.5);
    expect(khalaEasings.inQuad(0.5)).toBe(0.25);
    expect(khalaEasings.outQuad(0.5)).toBe(0.75);
    expect(khalaEasings.inOutQuad(0.5)).toBe(0.5);
    expect(khalaEasings.inCubic(0.5)).toBe(0.125);
    // clamp01 guards out-of-range input on both sides.
    expect(khalaEasings.linear(-3)).toBe(0);
    expect(khalaEasings.inQuad(4)).toBe(1);
  });

  test("inQuad is strictly monotonic increasing across the unit interval", () => {
    let previous = khalaEasings.inQuad(0);
    for (let step = 1; step <= 10; step += 1) {
      const next = khalaEasings.inQuad(step / 10);
      expect(next).toBeGreaterThan(previous);
      previous = next;
    }
  });

  test("sampleKhalaEasing dispatches to the named pure fn", () => {
    expect(sampleKhalaEasing("inQuad", 0.5)).toBe(0.25);
    expect(sampleKhalaEasing("linear", 0.3)).toBe(0.3);
    expect(sampleKhalaEasing("outQuad", 0.5)).toBe(0.75);
  });

  test("khalaEaseAmong buckets progress across the value list", () => {
    const values = ["a", "b", "c"] as const;
    expect(khalaEaseAmong(values, 0)).toBe("a");
    expect(khalaEaseAmong(values, 0.33)).toBe("a"); // floor(0.99) -> 0
    expect(khalaEaseAmong(values, 0.34)).toBe("b"); // floor(1.02) -> 1
    expect(khalaEaseAmong(values, 0.9)).toBe("c"); // floor(2.7) -> 2
    expect(khalaEaseAmong(values, 1)).toBe("c"); // clamped to last index
  });

  test("khalaEaseSteps quantizes progress to a step grid", () => {
    expect(khalaEaseSteps(4, 0.24)).toBe(0); // floor(0.96) -> 0
    expect(khalaEaseSteps(4, 0.25)).toBe(0.25); // floor(1.0) -> 1 /4
    expect(khalaEaseSteps(4, 0.7)).toBe(0.5); // floor(2.8) -> 2 /4
    expect(khalaEaseSteps(4, 0.99)).toBe(0.75); // floor(3.96) -> 3 /4
    expect(khalaEaseSteps(4, 1)).toBe(1);
    // step count is clamped to >=1, so 0 collapses to a single bucket.
    expect(khalaEaseSteps(0, 0.9)).toBe(0);
  });

  test("resolveKhalaMotionKeyframes reverses on exit", () => {
    const preset = { _tag: "Property", property: "opacity", from: 0, to: 1 } as const;
    expect(resolveKhalaMotionKeyframes(preset, "enter")).toEqual([
      { offset: 0, values: { opacity: 0 } },
      { offset: 1, values: { opacity: 1 } },
    ]);
    expect(resolveKhalaMotionKeyframes(preset, "exit")).toEqual([
      { offset: 0, values: { opacity: 1 } },
      { offset: 1, values: { opacity: 0 } },
    ]);
  });
});

describe("text sequencing", () => {
  test("khalaCipherCharacters is the pinned alphabet", () => {
    expect(khalaCipherCharacters).toBe("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_");
  });

  test("khalaTextDurationMillis scales per grapheme with floor and cap", () => {
    expect(khalaTextDurationMillis("hello")).toBe(140); // 5 * 28
    expect(khalaTextDurationMillis("")).toBe(100); // floored to 100
    expect(khalaTextDurationMillis("a".repeat(100))).toBe(1600); // 2800 capped to 1600
    expect(khalaTextDurationMillis("aaaaaaaaaa", 200)).toBe(1000); // per-grapheme clamped to 100
  });

  test("makeKhalaTextSequenceFrames reveals graphemes progressively", () => {
    const frames = makeKhalaTextSequenceFrames("abc");
    expect(frames.length).toBe(4); // default frame count = min(32, len+1)
    expect(frames.map((frame) => frame.visualText)).toEqual(["", "a", "ab", "abc"]);
    expect(frames[0]!.offset).toBe(0);
    expect(frames.at(-1)!.offset).toBe(1);
    expect(frames.every((frame) => frame.accessibleText === "abc")).toBe(true);
  });

  test("makeKhalaTextSequenceFrames adds a caret except on the final frame", () => {
    const frames = makeKhalaTextSequenceFrames("abc", { caret: true });
    expect(frames.map((frame) => frame.visualText)).toEqual(["▌", "a▌", "ab▌", "abc"]);
  });

  test("makeKhalaTextSequenceFrames collapses when maxGraphemes truncates", () => {
    const frames = makeKhalaTextSequenceFrames("abcdef", { maxGraphemes: 3 });
    expect(frames).toEqual([{ offset: 1, visualText: "abcdef", accessibleText: "abcdef" }]);
  });

  test("makeKhalaTextSequenceFrames exit reverses the visible sequence", () => {
    const frames = makeKhalaTextSequenceFrames("abc", { direction: "exit" });
    expect(frames.map((frame) => frame.visualText)).toEqual(["abc", "ab", "a", ""]);
    expect(frames.map((frame) => frame.offset)).toEqual([0, 1 / 3, 2 / 3, 1]);
  });

  test("makeKhalaTextDecipherFrames is seed-deterministic and settles on the source", () => {
    const first = makeKhalaTextDecipherFrames("a b", 42);
    const second = makeKhalaTextDecipherFrames("a b", 42);
    expect(first).toEqual(second); // identical seed -> identical frames
    expect(first.at(-1)).toEqual({ offset: 1, visualText: "a b", accessibleText: "a b" });
    expect(first.every((frame) => frame.accessibleText === "a b")).toBe(true);
    // Whitespace graphemes are never ciphered.
    expect(first.every((frame) => frame.visualText[1] === " ")).toBe(true);
    // A different seed diverges before the stable final frame.
    const other = makeKhalaTextDecipherFrames("a b", 7);
    expect(other[1]!.visualText).not.toBe(first[1]!.visualText);
  });

  test("makeKhalaTextDecipherFrames collapses when maxGraphemes is exceeded", () => {
    const frames = makeKhalaTextDecipherFrames("abcdef", 1, { maxGraphemes: 3 });
    expect(frames).toEqual([{ offset: 1, visualText: "abcdef", accessibleText: "abcdef" }]);
  });
});

describe("dom keyframe projection", () => {
  test("khalaKeyframeToDom maps transform properties to CSS strings", () => {
    expect(khalaKeyframeToDom({ offset: 0.25, values: { x: 12 } })).toEqual({
      offset: 0.25,
      transform: "translateX(12px)",
    });
    expect(khalaKeyframeToDom({ offset: 0, values: { y: 5 } }).transform).toBe("translateY(5px)");
    expect(khalaKeyframeToDom({ offset: 0, values: { scale: 2 } }).transform).toBe("scale(2)");
    expect(khalaKeyframeToDom({ offset: 0, values: { rotate: 90 } }).transform).toBe(
      "rotate(90deg)",
    );
    expect(khalaKeyframeToDom({ offset: 0, values: { skew: 15 } }).transform).toBe("skew(15deg)");
  });

  test("khalaKeyframeToDom stringifies stroke props and passes opacity through", () => {
    expect(
      khalaKeyframeToDom({ offset: 1, values: { strokeDasharray: 100, strokeDashoffset: 40 } }),
    ).toEqual({
      offset: 1,
      strokeDasharray: "100",
      strokeDashoffset: "40",
    });
    expect(khalaKeyframeToDom({ offset: 0.5, values: { opacity: 0.3 } })).toEqual({
      offset: 0.5,
      opacity: 0.3,
    });
  });
});

describe("illumination plan", () => {
  test("khalaSvgIlluminationGradientId is a stable FNV-1a suffix", () => {
    expect(khalaSvgIlluminationGradientId("panel")).toBe("en-khala-illumination-1kvmddx");
    expect(khalaSvgIlluminationGradientId("a")).toBe("en-khala-illumination-1r9wi7g");
    // Deterministic and collision-distinct for different ids.
    expect(khalaSvgIlluminationGradientId("panel")).toBe(khalaSvgIlluminationGradientId("panel"));
    expect(khalaSvgIlluminationGradientId("panel")).not.toBe(khalaSvgIlluminationGradientId("a"));
  });

  test("makeKhalaIlluminationNativePlan clamps opacity and radius", () => {
    expect(makeKhalaIlluminationNativePlan({ color: "#fff", radius: 100 })).toEqual({
      kind: "static-outline",
      color: "#fff",
      opacity: 0.42, // default intensity
      radius: 100,
    });
    expect(makeKhalaIlluminationNativePlan({ color: "red", radius: 5 }).radius).toBe(8); // min radius
    expect(makeKhalaIlluminationNativePlan({ color: "red", radius: 5000 }).radius).toBe(1024); // max radius
    expect(
      makeKhalaIlluminationNativePlan({ color: "red", radius: 100, intensity: 2 }).opacity,
    ).toBe(1);
    expect(
      makeKhalaIlluminationNativePlan({ color: "red", radius: 100, intensity: -1 }).opacity,
    ).toBe(0);
  });
});

describe("choreography planning", () => {
  const children = [
    { id: "a", enterMillis: 100, exitMillis: 80 },
    { id: "b", enterMillis: 200, exitMillis: 120 },
  ];

  test("planKhalaChoreography parallel fans out with zero offsets", () => {
    expect(planKhalaChoreography({ manager: "parallel", target: "entered", children })).toEqual([
      { id: "a", target: "entered", offsetMillis: 0, durationMillis: 100 },
      { id: "b", target: "entered", offsetMillis: 0, durationMillis: 200 },
    ]);
    // exited target selects the exit durations.
    expect(
      planKhalaChoreography({ manager: "parallel", target: "exited", children }).map(
        (step) => step.durationMillis,
      ),
    ).toEqual([80, 120]);
  });

  test("planKhalaChoreography switch enters only the active child", () => {
    const plan = planKhalaChoreography({
      manager: "switch",
      target: "entered",
      children,
      activeId: "a",
    });
    expect(plan).toEqual([
      { id: "a", target: "entered", offsetMillis: 0, durationMillis: 100 },
      { id: "b", target: "exited", offsetMillis: 0, durationMillis: 120 },
    ]);
    expect(plan.filter((step) => step.target === "entered").length).toBe(1);
  });

  test("planKhalaChoreography sequence accumulates offsets by duration", () => {
    const plan = planKhalaChoreography({ manager: "sequence", target: "entered", children });
    expect(plan.map((step) => step.offsetMillis)).toEqual([0, 100]); // second starts after first duration
    const stagger = planKhalaChoreography({
      manager: "stagger",
      target: "entered",
      children,
      staggerMillis: 20,
    });
    expect(stagger.map((step) => step.offsetMillis)).toEqual([0, 20]); // index * stagger
  });

  test("composeKhalaChoreographyPlans merges last-writer vs combines spans", () => {
    const plans = [
      [{ id: "a", target: "entered" as const, offsetMillis: 0, durationMillis: 100 }],
      [{ id: "a", target: "exited" as const, offsetMillis: 50, durationMillis: 200 }],
    ];
    expect(composeKhalaChoreographyPlans(plans, "merge")).toEqual([
      { id: "a", target: "exited", offsetMillis: 50, durationMillis: 200 },
    ]);
    // combine spans both intervals: start=min(0,50)=0, end=max(100,250)=250.
    expect(composeKhalaChoreographyPlans(plans, "combine")).toEqual([
      { id: "a", target: "exited", offsetMillis: 0, durationMillis: 250 },
    ]);
  });

  test("checkKhalaChoreographyModel receipt is exhaustive and green", () => {
    expect(checkKhalaChoreographyModel()).toEqual({
      statesChecked: 48, // 6 managers * 2 targets * 4 child counts
      managersChecked: 6,
      switchExclusive: true,
      offsetsBounded: true,
      stableTargets: true,
    });
  });
});
