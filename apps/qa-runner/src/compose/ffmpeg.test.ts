// Unit tests for the ffmpeg arg builder (no spawning). These confirm the
// filtergraph is well-formed for single and side-by-side plans and that
// drawtext escaping is safe.

import { describe, expect, test } from "bun:test";
import { buildComposePlan } from "./build-plan.ts";
import { buildFfmpegArgs, escapeDrawText } from "./ffmpeg.ts";
import type { ComposeRunMeta } from "./plan.ts";

const meta: ComposeRunMeta = {
  scenarioTitle: "openagents.com",
  goal: "open /login: confirm form",
  verdict: "pass",
  targetName: "openagents.com",
  targetBaseUrl: "https://openagents.com",
  brain: "khala",
  durationMs: 17088,
  steps: [{ index: 0, kind: "navigate", label: "go", status: "ok" }],
  video: "session.mp4",
  screenshots: ["00-login-page.png"],
};

describe("escapeDrawText", () => {
  test("escapes colons, percent, backslash; normalizes apostrophes", () => {
    expect(escapeDrawText("a:b")).toBe("a\\:b");
    expect(escapeDrawText("100%")).toBe("100\\%");
    expect(escapeDrawText("a\\b")).toBe("a\\\\b");
    expect(escapeDrawText("it's")).toBe("it’s");
    expect(escapeDrawText("line1\nline2")).toBe("line1 line2");
  });
});

describe("buildFfmpegArgs — single", () => {
  test("builds one input, a color title card, a scaled clip, and concat", () => {
    const plan = buildComposePlan({ single: meta });
    const { args, inputCount } = buildFfmpegArgs(plan, { run: "/runs/demo" }, "/out/x.mp4");

    expect(inputCount).toBe(1);
    expect(args[0]).toBe("-y");
    expect(args).toContain("-i");
    // resolved absolute source path
    expect(args.some((a) => a.endsWith("/runs/demo/session.mp4"))).toBe(true);

    const fcIdx = args.indexOf("-filter_complex");
    expect(fcIdx).toBeGreaterThan(-1);
    const graph = args[fcIdx + 1];
    expect(graph).toContain("color=c=");
    expect(graph).toContain("concat=n=2:v=1:a=0[outv]");
    expect(graph).toContain("drawtext=");

    expect(args).toContain("[outv]");
    expect(args).toContain("libx264");
    expect(args[args.length - 1]).toBe("/out/x.mp4");
  });

  test("missing run dir throws", () => {
    const plan = buildComposePlan({ single: meta });
    expect(() => buildFfmpegArgs(plan, {}, "/out/x.mp4")).toThrow();
  });
});

describe("buildFfmpegArgs — drawtext fallback", () => {
  test("drawText=false degrades boxed overlays to drawbox and drops plain text", () => {
    const plan = buildComposePlan({ single: meta });
    const graph = buildFfmpegArgs(
      plan,
      { run: "/runs/demo" },
      "/out/x.mp4",
      { drawText: false },
    ).args;
    const fc = graph[graph.indexOf("-filter_complex") + 1];
    expect(fc).not.toContain("drawtext=");
    // verdict badge + pills survive as drawbox
    expect(fc).toContain("drawbox=");
    // still a valid concat graph
    expect(fc).toContain("concat=n=2:v=1:a=0[outv]");
  });

  test("drawText=true (default) uses drawtext", () => {
    const plan = buildComposePlan({ single: meta });
    const fc = buildFfmpegArgs(plan, { run: "/r" }, "/o.mp4").args[
      buildFfmpegArgs(plan, { run: "/r" }, "/o.mp4").args.indexOf("-filter_complex") + 1
    ];
    expect(fc).toContain("drawtext=");
  });
});

describe("buildFfmpegArgs — side-by-side", () => {
  test("builds two inputs, hstack, and concat", () => {
    const plan = buildComposePlan({
      before: { ...meta, verdict: "fail" },
      after: meta,
    });
    const { args, inputCount } = buildFfmpegArgs(
      plan,
      { before: "/runs/b", after: "/runs/a" },
      "/out/ba.mp4",
    );

    expect(inputCount).toBe(2);
    const graph = args[args.indexOf("-filter_complex") + 1];
    expect(graph).toContain("hstack=inputs=2");
    expect(graph).toContain("concat=n=2:v=1:a=0[outv]");
    expect(args.some((a) => a.endsWith("/runs/b/session.mp4"))).toBe(true);
    expect(args.some((a) => a.endsWith("/runs/a/session.mp4"))).toBe(true);
  });
});
