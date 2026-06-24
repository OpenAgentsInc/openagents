// Unit tests for the PURE compose planner. These exercise titles, labels, and
// before/after layouts, and prove `buildComposePlan` is deterministic — no
// render, no I/O, no clock.

import { describe, expect, test } from "bun:test";
import {
  buildComposePlan,
  clampLabel,
  formatDuration,
  stepStripText,
} from "./build-plan.ts";
import { decodeComposePlan } from "./plan.ts";
import type { ComposeRunMeta } from "./plan.ts";

const baseMeta: ComposeRunMeta = {
  scenarioTitle: "openagents.com",
  goal: "Verify the login page works: open /login, confirm the form renders.",
  verdict: "pass",
  targetName: "openagents.com",
  targetBaseUrl: "https://openagents.com",
  brain: "khala",
  durationMs: 17088,
  steps: [
    { index: 0, kind: "navigate", label: "navigate to /login", status: "ok" },
    { index: 1, kind: "readText", label: "read text", status: "ok" },
    { index: 2, kind: "assert", label: "contains 'Log in'", status: "ok" },
  ],
  video: "session.mp4",
  screenshots: ["00-login-page.png"],
};

describe("helpers", () => {
  test("formatDuration renders seconds and minutes", () => {
    expect(formatDuration(17088)).toBe("17.1s");
    expect(formatDuration(500)).toBe("0.5s");
    expect(formatDuration(65000)).toBe("1m 5s");
    expect(formatDuration(0)).toBe("0s");
  });

  test("clampLabel truncates with ellipsis and is idempotent under limit", () => {
    expect(clampLabel("short", 64)).toBe("short");
    const long = "x".repeat(80);
    const clamped = clampLabel(long, 10);
    expect(clamped.length).toBe(10);
    expect(clamped.endsWith("…")).toBe(true);
  });

  test("stepStripText is ordered by index and flags failures", () => {
    const text = stepStripText([
      { index: 2, kind: "assert", label: "c", status: "failed" },
      { index: 0, kind: "navigate", label: "a", status: "ok" },
      { index: 1, kind: "readText", label: "b", status: "ok" },
    ]);
    expect(text).toBe("1 navigate  ·  2 readText  ·  3 assert ✗");
  });
});

describe("buildComposePlan — single", () => {
  test("produces a title card + single clip with verdict + labels", () => {
    const plan = buildComposePlan({ single: baseMeta });

    expect(plan.layout).toBe("single");
    expect(plan.brand).toBe("OpenAgents");
    expect(plan.width).toBe(1280);
    expect(plan.height).toBe(720);
    expect(plan.segments.length).toBe(2);

    const [title, clip] = plan.segments;
    expect(title.kind).toBe("title-card");
    if (title.kind === "title-card") {
      expect(title.badge?.verdict).toBe("pass");
      expect(title.badge?.text).toBe("PASS");
      // scenario title + goal subtitle + meta line + brand
      const texts = title.overlays.map((o) => o.text);
      expect(texts).toContain("openagents.com");
      expect(texts.some((t) => t.includes("Verify the login page"))).toBe(true);
      expect(texts.some((t) => t.includes("khala"))).toBe(true);
      expect(texts).toContain("OpenAgents");
    }

    expect(clip.kind).toBe("clip");
    if (clip.kind === "clip") {
      expect(clip.layout).toBe("single");
      expect(clip.sources).toEqual([{ dir: "run", path: "session.mp4" }]);
      expect(clip.badge?.verdict).toBe("pass");
      const stripped = clip.overlays.find((o) => o.text.includes("navigate"));
      expect(stripped).toBeDefined();
    }
  });

  test("fail verdict colors the badge FAIL", () => {
    const plan = buildComposePlan({ single: { ...baseMeta, verdict: "fail" } });
    const title = plan.segments[0];
    expect(title.kind).toBe("title-card");
    if (title.kind === "title-card") {
      expect(title.badge?.text).toBe("FAIL");
    }
  });

  test("omitting goal drops the subtitle overlay", () => {
    const { goal: _drop, ...noGoal } = baseMeta;
    void _drop;
    const plan = buildComposePlan({ single: noGoal });
    const title = plan.segments[0];
    if (title.kind === "title-card") {
      expect(title.overlays.some((o) => o.text.includes("Verify"))).toBe(false);
    }
  });

  test("showStepStrip=false drops the step strip overlay", () => {
    const plan = buildComposePlan({
      single: baseMeta,
      style: { showStepStrip: false },
    });
    const clip = plan.segments[1];
    if (clip.kind === "clip") {
      expect(clip.overlays.some((o) => o.text.includes("navigate"))).toBe(false);
    }
  });

  test("decodes against the ComposePlan schema", () => {
    const plan = buildComposePlan({ single: baseMeta });
    expect(() => decodeComposePlan(plan)).not.toThrow();
  });
});

describe("buildComposePlan — before/after", () => {
  const before: ComposeRunMeta = { ...baseMeta, verdict: "fail" };
  const after: ComposeRunMeta = { ...baseMeta, verdict: "pass" };

  test("produces a side-by-side clip with two sources and variant chips", () => {
    const plan = buildComposePlan({ before, after });

    expect(plan.layout).toBe("side-by-side");
    const [title, clip] = plan.segments;

    expect(title.kind).toBe("title-card");
    if (title.kind === "title-card") {
      // title shows the AFTER verdict (state we reached)
      expect(title.badge?.verdict).toBe("pass");
      expect(title.overlays.some((o) => o.text.includes("Before / After"))).toBe(true);
    }

    expect(clip.kind).toBe("clip");
    if (clip.kind === "clip") {
      expect(clip.layout).toBe("side-by-side");
      expect(clip.sources).toEqual([
        { dir: "before", path: "session.mp4" },
        { dir: "after", path: "session.mp4" },
      ]);
      const texts = clip.overlays.map((o) => o.text);
      expect(texts).toContain("Before");
      expect(texts).toContain("After");
      expect(texts.some((t) => t.includes("FAIL") && t.includes("PASS"))).toBe(true);
    }
  });

  test("custom variant labels are respected", () => {
    const plan = buildComposePlan({
      before: { ...before, variantLabel: "main" },
      after: { ...after, variantLabel: "fix-6187" },
    });
    const clip = plan.segments[1];
    if (clip.kind === "clip") {
      const texts = clip.overlays.map((o) => o.text);
      expect(texts).toContain("main");
      expect(texts).toContain("fix-6187");
    }
  });

  test("throws when neither single nor a complete pair is given", () => {
    expect(() => buildComposePlan({})).toThrow();
    expect(() => buildComposePlan({ before })).toThrow();
  });
});

describe("determinism", () => {
  test("same input yields byte-identical plans across runs", () => {
    const a = JSON.stringify(buildComposePlan({ single: baseMeta }));
    const b = JSON.stringify(buildComposePlan({ single: baseMeta }));
    expect(a).toBe(b);
  });

  test("before/after determinism", () => {
    const input = {
      before: { ...baseMeta, verdict: "fail" as const },
      after: baseMeta,
    };
    const a = JSON.stringify(buildComposePlan(input));
    const b = JSON.stringify(buildComposePlan(input));
    expect(a).toBe(b);
  });

  test("custom style overrides are applied deterministically", () => {
    const plan = buildComposePlan({
      single: baseMeta,
      style: { width: 1920, height: 1080, brand: "Khala", titleCardSeconds: 4 },
    });
    expect(plan.width).toBe(1920);
    expect(plan.height).toBe(1080);
    expect(plan.brand).toBe("Khala");
    const title = plan.segments[0];
    if (title.kind === "title-card") {
      expect(title.durationSeconds).toBe(4);
    }
  });
});
