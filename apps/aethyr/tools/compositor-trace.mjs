import assert from "node:assert/strict";
import { distribution } from "../experiments/c-town/measurement.mjs";

const MAX_SLICES = 200000;
const integer = (v) => /^-?\d+$/.test(String(v));
const ns = (v) => {
  assert(integer(v), "A trace timestamp is missing.");
  return BigInt(v);
};
const ms = (v) => Number(v) / 1e6;
const no = (v) => v === false || v === "false" || v === 0 || v === "0";
function flatten(value, prefix = "", output = {}) {
  if (value && typeof value === "object") {
    for (const [key, item] of Object.entries(value)) {
      const name = prefix ? `${prefix}.${key}` : key;
      if (item && typeof item === "object") flatten(item, name, output);
      else if (
        typeof item === "string" &&
        item.length < 2_000_000 &&
        item.startsWith("{")
      ) {
        try {
          flatten(JSON.parse(item), name, output);
        } catch {
          output[name] = item;
        }
      } else output[name] = item;
    }
  }
  assert(
    Object.keys(output).length <= 100000,
    "Trace argument limit exceeded.",
  );
  return output;
}
function field(args, suffix) {
  const values = [
    ...new Set(
      Object.entries(args)
        .filter(([key]) => key === suffix || key.endsWith("." + suffix))
        .map(([, value]) => String(value)),
    ),
  ];
  assert(values.length <= 1, `Ambiguous trace field ${suffix}.`);
  return values[0];
}
const normalized = (slice) => ({ ...slice, args: flatten(slice.args ?? {}) });
function healthy(decoded, dataLossOccurred) {
  assert.equal(decoded?.schema, "aethyrforge.compositor-decoded.v1");
  assert(Array.isArray(decoded.slices) && decoded.slices.length <= MAX_SLICES);
  assert.equal(
    dataLossOccurred,
    false,
    "CDP reported lost or unavailable trace data.",
  );
  const losses = (decoded.stats ?? []).filter(
    (stat) =>
      Number(stat.value) > 0 &&
      (["error", "data_loss"].includes(stat.severity) ||
        /packet_loss|overwrit|discarded|buffer.*loss/i.test(stat.name)),
  );
  assert.deepEqual(
    losses,
    [],
    "The trace processor reported incomplete or invalid data.",
  );
}

/** DOM owner -> actual embedded canvas surface, before any pacing inference. */
export function identifyCanvasSurface(
  decoded,
  { backendNodeId, layerIds = [], rendererPid, dataLossOccurred = false },
) {
  healthy(decoded, dataLossOccurred);
  assert(Number.isSafeInteger(backendNodeId) && backendNodeId > 0);
  const candidates = new Map();
  for (const raw of decoded.slices.filter(
    (s) => s.name === "LayerTreeHostImpl:snapshot",
  )) {
    const slice = normalized(raw);
    if (rendererPid !== undefined && String(slice.pid) !== String(rendererPid))
      continue;
    for (const [key, value] of Object.entries(slice.args)) {
      if (!key.endsWith(".owner_node") || Number(value) !== backendNodeId)
        continue;
      const prefix = key.slice(0, -"owner_node".length);
      const range = slice.args[prefix + "surface_range"];
      if (typeof range !== "string") continue;
      const layer =
        slice.args[prefix + "layer_id"] ?? slice.args[prefix + "id"];
      if (layerIds.length && !layerIds.map(String).includes(String(layer)))
        continue;
      const end = range.split("end:").at(-1);
      const ids = [
        ...end.matchAll(/FrameSinkId(?:\[[^\]]*\])?\((\d+),\s*(\d+)\)/g),
      ];
      assert.equal(
        ids.length,
        1,
        "The canvas surface range has no unique current FrameSinkId.",
      );
      assert(
        slice.pid !== null && slice.pid !== undefined,
        "The canvas layer has no renderer process.",
      );
      const candidate = {
        backendNodeId,
        layerId: String(layer),
        rendererPid: String(slice.pid),
        frameSink: { clientId: ids[0][1], sinkId: ids[0][2] },
        surfaceRange: range,
      };
      candidates.set(
        `${candidate.rendererPid}/${candidate.frameSink.clientId}/${candidate.frameSink.sinkId}`,
        candidate,
      );
    }
  }
  assert.equal(
    candidates.size,
    1,
    "The DOM canvas has no unique traced embedded surface; direct-canvas attribution is unavailable.",
  );
  return {
    ...[...candidates.values()][0],
    method: "DOM owner and traced current SurfaceLayer range",
  };
}

/** Versioned prototype; never rewrites existing GPU-fence qualification results. */
export function analyzeCompositorTrace(
  decoded,
  {
    canvas,
    startMark,
    endMark,
    dataLossOccurred,
    mode = "diagnostic",
    minHz = 60,
    expectedSubmittedFrames = null,
    headless = null,
    warmupMs = null,
  },
) {
  const result = {
    schema: "aethyrforge.compositor-pacing.v1",
    ok: false,
    attributed: false,
    pacingQualified: false,
    physicalPresentation: null,
    browserFeedback:
      headless === false
        ? "macOS155 CVDisplayLink-derived presentation estimate"
        : "Headless or unknown compositor mode; headed presentation is unavailable",
    headless,
    checks: [],
    freshFrames: [],
    diagnostics: {},
    limitations: [
      "Browser feedback on this Mac predicts display time using CVDisplayLink and a latch heuristic; it does not measure physical scanout.",
      "The prototype does not alter existing fence gates. One attributed trace cannot qualify all product workloads.",
      "Graphics.Pipeline STEP_DRAW_AND_SWAP CPU duration is not presentation time. Only explicitly linked feedback endpoints are accepted.",
    ],
  };
  const check = (id, condition, detail) => {
    result.checks.push({ id, ok: Boolean(condition), detail });
    return condition;
  };
  try {
    healthy(decoded, dataLossOccurred);
    assert(["qualification", "diagnostic"].includes(mode));
    assert(Number.isFinite(minHz) && minHz >= 30 && minHz <= 240);
    assert(
      canvas?.frameSink &&
        integer(canvas.frameSink.clientId) &&
        integer(canvas.frameSink.sinkId),
    );
    const slices = decoded.slices.map(normalized);
    const marker = (name) => {
      const found = slices.filter(
        (s) => s.name === name && s.category?.includes("blink.user_timing"),
      );
      assert.equal(
        found.length,
        1,
        `The window marker ${name} is missing or ambiguous.`,
      );
      assert.equal(
        String(found[0].pid),
        String(canvas.rendererPid),
        "The recording clock belongs to another renderer.",
      );
      return ns(found[0].tsNs);
    };
    const start = marker(startMark),
      end = marker(endMark);
    assert(end > start);
    const durationMs = ms(end - start),
      budget = 1000 / minHz;
    result.window = {
      startNs: String(start),
      endNs: String(end),
      durationMs,
      minHz,
      mode,
      expectedSubmittedFrames,
    };
    const displayed = slices
      .filter((s) => s.name === "Display::FrameDisplayed")
      .map((s) => ns(s.tsNs));
    result.diagnostics.unattributedDisplayCount = displayed.filter(
      (t) => t >= start && t < end,
    ).length;
    const pipeline = slices.filter((s) => s.name === "Graphics.Pipeline");
    const receiveBySource = new Map();
    for (const s of pipeline) {
      if (
        field(s.args, "chrome_graphics_pipeline.step") !==
        "STEP_RECEIVE_COMPOSITOR_FRAME"
      )
        continue;
      if (
        field(s.args, "frame_sink_id.frame_sink_client_id") !==
          String(canvas.frameSink.clientId) ||
        field(s.args, "frame_sink_id.frame_sink_id") !==
          String(canvas.frameSink.sinkId)
      )
        continue;
      const id = field(
        s.args,
        "chrome_graphics_pipeline.surface_frame_trace_id",
      );
      assert(
        integer(id) && id !== "0",
        "The canvas update has no unique surface-frame identity.",
      );
      const old = receiveBySource.get(id);
      assert(
        !old,
        "The canvas repeated a surface-frame identity; freshness is ambiguous.",
      );
      receiveBySource.set(id, s);
    }
    assert(
      receiveBySource.size,
      "No updates from the exact canvas FrameSink were received.",
    );
    const displaysBySource = new Map();
    for (const s of pipeline) {
      if (
        field(s.args, "chrome_graphics_pipeline.step") !==
        "STEP_SURFACE_AGGREGATION"
      )
        continue;
      const display = field(
        s.args,
        "chrome_graphics_pipeline.display_trace_id",
      );
      if (!display) continue;
      assert(integer(display));
      for (const [key, value] of Object.entries(s.args)) {
        if (
          !/chrome_graphics_pipeline\.aggregated_surface_frame_trace_ids(?:\[\d+\]|\.\d+)?$/.test(
            key,
          ) ||
          !receiveBySource.has(String(value))
        )
          continue;
        const source = String(value),
          receive = receiveBySource.get(source);
        assert(
          ns(s.tsNs) >= ns(receive.tsNs),
          "Canvas aggregation predates its update.",
        );
        const list = displaysBySource.get(source) ?? new Set();
        list.add(display);
        displaysBySource.set(source, list);
      }
    }
    const feedbackByDisplay = new Map();
    for (const s of slices.filter((s) => s.name === "PipelineReporter")) {
      const id =
        field(s.args, "frame_reporter.display_trace_id") ??
        field(s.args, "chrome_frame_reporter.display_trace_id");
      if (!id) continue;
      const state =
        field(s.args, "frame_reporter.state") ??
        field(s.args, "chrome_frame_reporter.state");
      const missing =
        field(s.args, "frame_reporter.has_missing_content") ??
        field(s.args, "chrome_frame_reporter.has_missing_content");
      const record = {
        at: ns(s.tsNs) + ns(s.durNs),
        valid:
          state === "STATE_PRESENTED_ALL" && no(missing) && ns(s.durNs) >= 0n,
        reporterId: String(s.id),
        state,
        missing,
      };
      const previous = feedbackByDisplay.get(id) ?? [];
      previous.push(record);
      feedbackByDisplay.set(id, previous);
    }
    // Some trace adapters expose the display ID on the explicitly asynchronous span.
    // Stock155 may omit it; never guess from a nearby CPU slice or callback order.
    for (const s of slices.filter(
      (s) => s.name === "Graphics.Pipeline.DrawAndSwap",
    )) {
      const id = field(s.args, "display_trace_id");
      if (!id || ns(s.durNs) < 0n || field(s.args, "status") === "canceled")
        continue;
      const previous = feedbackByDisplay.get(id) ?? [];
      previous.push({
        at: ns(s.tsNs) + ns(s.durNs),
        valid: true,
        reporterId: String(s.id),
        state: "explicit-async-feedback-span",
      });
      feedbackByDisplay.set(id, previous);
    }
    const claimedDisplays = new Map(),
      unresolved = [];
    let repeatedPresentations = 0;
    for (const [source, receive] of receiveBySource) {
      const candidates = [];
      for (const display of displaysBySource.get(source) ?? []) {
        const feedback = feedbackByDisplay.get(display);
        if (!feedback?.length || !feedback.every((value) => value.valid)) {
          unresolved.push({
            source,
            display,
            reason: "No exact successful feedback endpoint",
          });
          continue;
        }
        const times = new Set(feedback.map((value) => String(value.at)));
        assert.equal(
          times.size,
          1,
          "Conflicting presentation feedback for one display frame.",
        );
        const at = feedback[0].at;
        assert(
          at >= ns(receive.tsNs),
          "Presentation predates its fresh canvas update.",
        );
        assert(
          displayed.some((t) => (t > at ? t - at : at - t) <= 50000n),
          "The attributed feedback has no FrameDisplayed confirmation.",
        );
        candidates.push({
          at,
          display,
          reporters: feedback.map((value) => value.reporterId),
        });
      }
      candidates.sort((a, b) => (a.at < b.at ? -1 : a.at > b.at ? 1 : 0));
      if (!candidates.length) continue;
      repeatedPresentations += candidates.length - 1;
      const first = candidates[0];
      if (first.at < start || first.at >= end) continue;
      assert(
        !claimedDisplays.has(first.display),
        "Multiple fresh canvas updates claim the same presented image.",
      );
      claimedDisplays.set(first.display, source);
      result.freshFrames.push({
        sourceFrameId: source,
        displayId: first.display,
        receivedNs: receive.tsNs,
        presentedNs: String(first.at),
        feedbackSliceIds: first.reporters,
      });
    }
    result.freshFrames.sort((a, b) =>
      ns(a.presentedNs) < ns(b.presentedNs) ? -1 : 1,
    );
    const unresolvedInside = unresolved.filter((value) => {
      const at = ns(receiveBySource.get(value.source).tsNs);
      return at >= start && at < end;
    });
    check("feedback.complete", !unresolvedInside.length, {
      unresolved: unresolvedInside.slice(0, 32),
      total: unresolvedInside.length,
    });
    assert(
      result.freshFrames.length >= 2,
      "Fewer than two fresh canvas frames have exact presentation attribution.",
    );
    const times = result.freshFrames.map((frame) => ns(frame.presentedNs));
    const intervals = times.slice(1).map((value, i) => ms(value - times[i]));
    const stats = distribution(intervals);
    const count = result.freshFrames.length;
    result.attributed = true;
    result.summary = {
      durationMs,
      freshFrames: count,
      freshFramesPerSecond: (count * 1000) / durationMs,
      intervalMs: stats,
      repeatedPresentationsExcluded: repeatedPresentations,
      receivedCanvasUpdates: receiveBySource.size,
      distinctPresentedDisplays: claimedDisplays.size,
      displayedEventCount: result.diagnostics.unattributedDisplayCount,
      expectedSubmittedFrames,
      submittedDifference:
        expectedSubmittedFrames === null
          ? null
          : count - expectedSubmittedFrames,
    };
    check("fresh.count", count >= Math.floor((durationMs * minHz) / 1000) - 1, {
      actual: count,
      required: Math.floor((durationMs * minHz) / 1000) - 1,
    });
    check(
      "fresh.coverage",
      ms(times[0] - start) <= budget * 1.5 &&
        ms(end - times.at(-1)) <= budget * 1.5,
      "Fresh presentations span the actual recording window.",
    );
    check(
      "fresh.cadence",
      intervals.every((value) => value > 0) &&
        stats.p95 <= (17.5 * 60) / minHz &&
        stats.p99 <= (20 * 60) / minHz &&
        stats.max <= (33.34 * 60) / minHz &&
        intervals.filter((value) => value > (25 * 60) / minHz).length /
          intervals.length <=
          0.001,
      stats,
    );
    check(
      "fresh.no-long-stall",
      intervals.every((value) => value <= (50 * 60) / minHz),
      "No attributed fresh-frame interval exceeds the scaled 50 ms limit.",
    );
    if (expectedSubmittedFrames !== null)
      check(
        "application.count-consistent",
        Number.isInteger(expectedSubmittedFrames) &&
          Math.abs(count - expectedSubmittedFrames) <= 2,
        "Two explicit boundary frames allowed; no UI-only frames can substitute for canvas updates.",
      );
    check(
      "browser.headed",
      headless === false,
      "Headless or unknown mode cannot establish headed Mac compositor pacing.",
    );
    result.ok = result.checks.every((value) => value.ok);
    result.pacingQualified =
      mode === "qualification" &&
      durationMs >= 30000 &&
      warmupMs >= 5000 &&
      result.ok;
  } catch (error) {
    result.error = String(error?.message ?? error);
    result.checks.push({
      id: "trace.attribution",
      ok: false,
      detail: result.error,
    });
  }
  return result;
}

export function compositorObserverPlan(repeats = 2) {
  assert(Number.isInteger(repeats) && repeats >= 1 && repeats <= 8);
  return Array.from({ length: repeats }, (_, i) =>
    i % 2 ? ["on", "off", "off", "on"] : ["off", "on", "on", "off"],
  );
}

/** Reanalysis can repair parsing, never erase a failed collection or cleanup. */
export function compositorSessionVerdict(report) {
  const windows = report.windows?.filter((row) => row.trace) ?? [];
  return (
    report.collection?.complete === true &&
    report.cleanup?.complete === true &&
    report.collection.error === null &&
    report.cleanup.error === null &&
    Array.isArray(report.errors) &&
    report.errors.length === 0 &&
    report.attributed === true &&
    windows.length >= 2 &&
    windows.every((row) => row.compositor?.ok === true)
  );
}
