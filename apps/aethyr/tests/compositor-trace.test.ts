import assert from "node:assert/strict";
import test from "node:test";

const {
  identifyCanvasSurface,
  analyzeCompositorTrace,
  compositorObserverPlan,
} = await import(
  new URL("../tools/compositor-trace.mjs", import.meta.url).href
);

type Slice = {
  id: string;
  name: string;
  tsNs: string;
  durNs: string;
  category: string;
  pid: string;
  args: Record<string, unknown>;
};
const start = 1_000_000_000n;
const canvas = {
  backendNodeId: 42,
  rendererPid: "314",
  frameSink: { clientId: "7", sinkId: "99" },
};
function fixture(count = 60, durationMs = 1000) {
  const slices: Slice[] = [];
  const add = (
    name: string,
    at: bigint,
    args: Record<string, unknown> = {},
    durNs = 0n,
    category = "viz",
  ) => {
    const value = {
      id: String(slices.length + 1),
      name,
      tsNs: String(at),
      durNs: String(durNs),
      category,
      pid: "314",
      args,
    };
    slices.push(value);
    return value;
  };
  add("af.compositor.test.start", start, {}, 0n, "blink.user_timing");
  add(
    "af.compositor.test.end",
    start + BigInt(durationMs) * 1_000_000n,
    {},
    0n,
    "blink.user_timing",
  );
  for (let index = 0; index < count; index++) {
    const at =
      start + BigInt(Math.round((((index + 0.5) * durationMs) / count) * 1e6));
    const source = String(9_007_199_254_740_999n + BigInt(index));
    const display = String(10_007_199_254_740_999n + BigInt(index));
    add("Graphics.Pipeline", at - 2_000_000n, {
      "chrome_graphics_pipeline.step": "STEP_RECEIVE_COMPOSITOR_FRAME",
      "chrome_graphics_pipeline.frame_sink_id.frame_sink_client_id": "7",
      "chrome_graphics_pipeline.frame_sink_id.frame_sink_id": "99",
      "chrome_graphics_pipeline.surface_frame_trace_id": source,
    });
    add("Graphics.Pipeline", at - 1_000_000n, {
      "chrome_graphics_pipeline.step": "STEP_SURFACE_AGGREGATION",
      "chrome_graphics_pipeline.display_trace_id": display,
      "chrome_graphics_pipeline.aggregated_surface_frame_trace_ids[0]": source,
    });
    add(
      "PipelineReporter",
      at - 3_000_000n,
      {
        "frame_reporter.display_trace_id": display,
        "frame_reporter.state": "STATE_PRESENTED_ALL",
        "frame_reporter.has_missing_content": "0",
      },
      3_000_000n,
    );
    add("Display::FrameDisplayed", at);
  }
  return {
    schema: "aethyrforge.compositor-decoded.v1",
    slices,
    stats: [] as Record<string, unknown>[],
  };
}
const options = {
  canvas,
  startMark: "af.compositor.test.start",
  endMark: "af.compositor.test.end",
  dataLossOccurred: false,
  headless: false,
  warmupMs: 5000,
};
const pipeline = (input: ReturnType<typeof fixture>, step: string) =>
  input.slices.filter(
    (slice) => slice.args["chrome_graphics_pipeline.step"] === step,
  );

test("exact DOM owner and current surface range identify one embedded canvas", () => {
  const input = fixture();
  input.slices.push({
    id: "layer",
    name: "LayerTreeHostImpl:snapshot",
    tsNs: "0",
    durNs: "0",
    pid: "314",
    category: "layers",
    args: {
      "snapshot.layers[0].owner_node": "42",
      "snapshot.layers[0].layer_id": "8",
      "snapshot.layers[0].surface_range":
        "SurfaceRange(start: SurfaceId(FrameSinkId(1, 2), old), end: SurfaceId(FrameSinkId(7, 99), current))",
    },
  });
  const identity = identifyCanvasSurface(input, {
    backendNodeId: 42,
    layerIds: ["8"],
    dataLossOccurred: false,
  });
  assert.deepEqual(identity.frameSink, canvas.frameSink);
  assert.equal(identity.rendererPid, "314");
  assert.throws(
    () =>
      identifyCanvasSurface(input, {
        backendNodeId: 43,
        dataLossOccurred: false,
      }),
    /no unique/,
  );
  const foreign = structuredClone(input.slices.at(-1)!);
  foreign.pid = "315";
  input.slices.push(foreign);
  assert.throws(
    () =>
      identifyCanvasSurface(input, {
        backendNodeId: 42,
        dataLossOccurred: false,
      }),
    /no unique/,
  );
});

test("fresh canvas display joins retain 64-bit IDs and never imply physical scanout", () => {
  const input = fixture(),
    before = structuredClone(input);
  const result = analyzeCompositorTrace(input, {
    ...options,
    expectedSubmittedFrames: 60,
  });
  assert.equal(result.ok, true, result.error);
  assert.equal(result.attributed, true);
  assert.equal(result.pacingQualified, false);
  assert.equal(result.summary.freshFrames, 60);
  assert.equal(result.summary.freshFramesPerSecond, 60);
  assert.equal(result.freshFrames[0].sourceFrameId, "9007199254740999");
  assert.equal(result.physicalPresentation, null);
  assert.deepEqual(input, before);
});

test("UI-only display events and a stale canvas do not become fresh canvas frames", () => {
  const input = fixture(1);
  const template = input.slices.find(
    (slice) => slice.name === "Display::FrameDisplayed",
  )!;
  for (let index = 0; index < 60; index++)
    input.slices.push({
      ...template,
      id: `ui-${index}`,
      tsNs: String(start + BigInt(index) * 16_666_667n),
    });
  const result = analyzeCompositorTrace(input, options);
  assert.equal(result.ok, false);
  assert.equal(result.attributed, false);
  assert.match(result.error, /Fewer than two fresh/);
  assert.ok(result.diagnostics.unattributedDisplayCount >= 60);
});

test("repeated presentation of an unchanged canvas is excluded from fresh count", () => {
  const input = fixture();
  const original = pipeline(input, "STEP_SURFACE_AGGREGATION")[0];
  const clone = structuredClone(original);
  clone.id = "repeat";
  clone.args["chrome_graphics_pipeline.display_trace_id"] = "888";
  clone.tsNs = String(start + 15_000_000n);
  input.slices.push(
    clone,
    {
      id: "repeat-feedback",
      name: "PipelineReporter",
      tsNs: String(start + 15_000_000n),
      durNs: "1000000",
      pid: "314",
      category: "viz",
      args: {
        "frame_reporter.display_trace_id": "888",
        "frame_reporter.state": "STATE_PRESENTED_ALL",
        "frame_reporter.has_missing_content": "0",
      },
    },
    {
      id: "repeat-display",
      name: "Display::FrameDisplayed",
      tsNs: String(start + 16_000_000n),
      durNs: "0",
      pid: "314",
      category: "viz",
      args: {},
    },
  );
  const result = analyzeCompositorTrace(input, options);
  assert.equal(result.ok, true, result.error);
  assert.equal(result.summary.freshFrames, 60);
  assert.equal(result.summary.repeatedPresentationsExcluded, 1);
});

test("lost traces, missing IDs, ambiguous clocks, incomplete feedback, and content loss fail", () => {
  const lost = analyzeCompositorTrace(fixture(), {
    ...options,
    dataLossOccurred: true,
  });
  assert.equal(lost.ok, false);
  const processorLoss = fixture();
  processorLoss.stats.push({
    name: "traced_buf_trace_writer_packet_loss",
    severity: "data_loss",
    value: "1",
  });
  assert.equal(analyzeCompositorTrace(processorLoss, options).ok, false);
  const missing = fixture();
  delete pipeline(missing, "STEP_RECEIVE_COMPOSITOR_FRAME")[0].args[
    "chrome_graphics_pipeline.surface_frame_trace_id"
  ];
  assert.equal(analyzeCompositorTrace(missing, options).ok, false);
  const clocks = fixture();
  clocks.slices.push({ ...clocks.slices[0] });
  assert.equal(analyzeCompositorTrace(clocks, options).ok, false);
  const partial = fixture();
  partial.slices.find((slice) => slice.name === "PipelineReporter")!.args[
    "frame_reporter.has_missing_content"
  ] = "1";
  const partialResult = analyzeCompositorTrace(partial, options);
  assert.equal(
    partialResult.checks.find(
      (check: { id: string }) => check.id === "feedback.complete",
    ).ok,
    false,
  );
  assert.equal(partialResult.ok, false);
  const noConfirmation = fixture();
  noConfirmation.slices = noConfirmation.slices.filter(
    (slice) => slice.name !== "Display::FrameDisplayed",
  );
  assert.equal(analyzeCompositorTrace(noConfirmation, options).ok, false);
});

test("CPU swap steps cannot replace missing presentation feedback", () => {
  const input = fixture();
  input.slices = input.slices.filter(
    (slice) => slice.name !== "PipelineReporter",
  );
  for (const slice of pipeline(input, "STEP_SURFACE_AGGREGATION"))
    input.slices.push({
      ...slice,
      id: `cpu-${slice.id}`,
      args: {
        ...slice.args,
        "chrome_graphics_pipeline.step": "STEP_DRAW_AND_SWAP",
      },
    });
  const result = analyzeCompositorTrace(input, options);
  assert.equal(result.ok, false);
  assert.equal(result.attributed, false);
});

test("headless or unknown mode cannot qualify headed pacing and 30 seconds is required", () => {
  for (const headless of [true, null]) {
    const result = analyzeCompositorTrace(fixture(), { ...options, headless });
    assert.equal(result.attributed, true);
    assert.equal(result.ok, false);
    assert.equal(result.pacingQualified, false);
  }
  assert.equal(
    analyzeCompositorTrace(fixture(1800, 30000), {
      ...options,
      mode: "qualification",
      expectedSubmittedFrames: 1800,
    }).pacingQualified,
    true,
  );
  assert.equal(
    analyzeCompositorTrace(fixture(), { ...options, mode: "qualification" })
      .pacingQualified,
    false,
  );
});

test("slow frames, duplicate fresh display claims, and app-count mismatch cannot pass", () => {
  const slow = analyzeCompositorTrace(fixture(30), options);
  assert.equal(slow.ok, false);
  assert.equal(
    slow.checks.find((check: { id: string }) => check.id === "fresh.count").ok,
    false,
  );
  const duplicate = fixture();
  const aggregations = pipeline(duplicate, "STEP_SURFACE_AGGREGATION");
  aggregations[1].args[
    "chrome_graphics_pipeline.aggregated_surface_frame_trace_ids[1]"
  ] =
    aggregations[0].args[
      "chrome_graphics_pipeline.aggregated_surface_frame_trace_ids[0]"
    ];
  aggregations[0].args["chrome_graphics_pipeline.display_trace_id"] =
    aggregations[1].args["chrome_graphics_pipeline.display_trace_id"];
  assert.equal(analyzeCompositorTrace(duplicate, options).ok, false);
  assert.equal(
    analyzeCompositorTrace(fixture(), {
      ...options,
      expectedSubmittedFrames: 120,
    }).ok,
    false,
  );
});

test("observer cost uses explicit ABBA then BAAB orders", () => {
  assert.deepEqual(compositorObserverPlan(2), [
    ["off", "on", "on", "off"],
    ["on", "off", "off", "on"],
  ]);
  assert.throws(() => compositorObserverPlan(0));
});

test("reanalysis cannot turn collection or cleanup failures into success", async () => {
  const { compositorSessionVerdict } = await import(
    new URL("../tools/compositor-trace.mjs", import.meta.url).href
  );
  const report = {
    collection: { complete: true, error: null as string | null },
    cleanup: { complete: true, error: null as string | null },
    errors: [] as string[],
    attributed: true,
    windows: [
      { trace: {}, compositor: { ok: true } },
      { trace: {}, compositor: { ok: true } },
    ],
  };
  assert.equal(compositorSessionVerdict(report), true);
  for (const field of ["collection", "cleanup"] as const) {
    const copy = structuredClone(report);
    copy[field].complete = false;
    assert.equal(compositorSessionVerdict(copy), false);
    copy[field].complete = true;
    copy[field].error = "Original failure";
    assert.equal(compositorSessionVerdict(copy), false);
  }
  assert.equal(
    compositorSessionVerdict({ ...report, errors: ["A browser error"] }),
    false,
  );
  assert.equal(
    compositorSessionVerdict({ ...report, collection: undefined }),
    false,
  );
  assert.equal(compositorSessionVerdict({ ...report, windows: [] }), false);
});

test("short warmup cannot qualify a sustained compositor window", () => {
  const result = analyzeCompositorTrace(fixture(1800, 30000), {
    ...options,
    mode: "qualification",
    warmupMs: 500,
  });
  assert.equal(result.ok, true);
  assert.equal(result.pacingQualified, false);
});
