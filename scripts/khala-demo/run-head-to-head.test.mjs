import { describe, expect, test } from "bun:test";

import { reduceKhalaHeadToHeadManifest } from "./reduce-head-to-head.mjs";
import {
  buildRunFromCompletion,
  CROSSY_ROAD_PROMPT,
  liveStreamTransport,
  liveTransport,
  reconstructCompletionFromSse,
  runHeadToHead,
  stubTransport,
} from "./run-head-to-head.mjs";

function counterClock(startMs = 1_000_000) {
  let now = startMs;
  return () => {
    const value = now;
    now += 1000;
    return value;
  };
}

describe("Khala head-to-head runner (stub)", () => {
  test("emits a reducer-consumable manifest that still refuses to close #6016", async () => {
    const manifest = await runHeadToHead({
      khalaTransport: stubTransport,
      frontierTransport: stubTransport,
      live: false,
      clock: counterClock(),
      generatedAt: "2026-06-22T18:00:00.000Z",
    });

    expect(manifest.schema).toBe("openagents.khala_head_to_head_evidence.v1");
    expect(manifest.evidenceMode).toBe("fixture_scaffold");
    expect(manifest.runs).toHaveLength(2);
    expect(manifest.runs.map((run) => run.lane)).toEqual(["khala", "frontier_baseline"]);

    // The reducer accepts it and computes a scoreboard.
    const metrics = reduceKhalaHeadToHeadManifest(manifest);
    expect(metrics.summary.runCount).toBe(2);

    const khala = metrics.scoreboard.find((run) => run.lane === "khala");
    expect(khala.tokens).toBe(89600);
    expect(khala.inWorldVsGatewaySplit.inWorldShare).toBe(0.6);

    // Stub evidence MUST keep the closure audit blocked.
    expect(metrics.closureAudit.canClose).toBe(false);
    expect(metrics.livePromotionAudit.status).toBe("blocked");
  });

  test("never fabricates settlement, verse, in-world, or energy telemetry", async () => {
    const manifest = await runHeadToHead({
      khalaTransport: stubTransport,
      frontierTransport: stubTransport,
      live: false,
      clock: counterClock(),
    });
    const khala = manifest.runs.find((run) => run.lane === "khala");

    expect(khala.settlement.settled).toBe(false);
    expect(khala.settlement.receiptRefs).toEqual([]);
    expect(khala.verse.playbackRef).toBeNull();
    expect(khala.artifact.playableInWorldRef).toBeNull();
    expect(khala.energy.kwhMeasured).toBeNull();
    expect(khala.energy.measurementRef).toBeNull();

    const metrics = reduceKhalaHeadToHeadManifest(manifest);
    const khalaMetric = metrics.scoreboard.find((run) => run.lane === "khala");
    expect(khalaMetric.acceptedOutcomesPerKwh).toBe("not_measured");
    expect(metrics.summary.acceptedOutcomesPerKwh).toBe("not_measured");
  });

  test("records the wall-clock measured around the transport call", async () => {
    const manifest = await runHeadToHead({
      khalaTransport: stubTransport,
      frontierTransport: stubTransport,
      live: false,
      clock: counterClock(),
    });
    for (const run of manifest.runs) {
      expect(run.wallClockMs).toBe(1000);
      expect(run.startedAt).toBeDefined();
      expect(run.completedAt).toBeDefined();
    }
  });
});

describe("buildRunFromCompletion honesty", () => {
  test("flags missing cost when the response carries no cost_msat", () => {
    const run = buildRunFromCompletion({
      lane: "khala",
      label: "no cost",
      model: "openagents/khala-code",
      provider: "openagents",
      response: { usage: { total_tokens: 100 }, openagents: { verification: "test_passed" } },
      startedAt: "2026-06-22T16:00:00.000Z",
      completedAt: "2026-06-22T16:01:00.000Z",
      wallClockMs: 60000,
      live: false,
    });
    expect(run.costUsd).toBe(0);
    expect(run.blockerRefs).toContain("blocker.khala_demo.cost_usd_not_measured");
  });

  test("treats a missing verifier verdict as not-accepted with a blocker", () => {
    const run = buildRunFromCompletion({
      lane: "khala",
      label: "no verdict",
      model: "openagents/khala-code",
      provider: "openagents",
      response: { usage: { total_tokens: 100 }, openagents: {} },
      startedAt: "2026-06-22T16:00:00.000Z",
      completedAt: "2026-06-22T16:01:00.000Z",
      wallClockMs: 60000,
      live: true,
    });
    expect(run.acceptedOutcome.accepted).toBe(false);
    expect(run.acceptedOutcome.verificationClass).toBe("none");
    expect(run.acceptedOutcome.blockerRefs).toContain(
      "blocker.khala_demo.verifier_verdict_missing",
    );
  });

  test("derives USD from cost_msat only when a conversion rate is supplied", () => {
    const base = {
      lane: "khala",
      label: "rate",
      model: "openagents/khala-code",
      provider: "openagents",
      response: {
        usage: { total_tokens: 100 },
        openagents: { verification: "test_passed", cost_msat: 1_000_000 },
      },
      startedAt: "2026-06-22T16:00:00.000Z",
      completedAt: "2026-06-22T16:01:00.000Z",
      wallClockMs: 60000,
      live: true,
    };
    const withoutRate = buildRunFromCompletion(base);
    expect(withoutRate.costUsd).toBe(0);

    const withRate = buildRunFromCompletion({ ...base, msatPerUsd: 1_000_000 });
    expect(withRate.costUsd).toBe(1);
  });

  test("honors explicit live settlement / verse / energy evidence when present", () => {
    const run = buildRunFromCompletion({
      lane: "khala",
      label: "full live",
      model: "openagents/khala",
      provider: "openagents",
      response: {
        usage: { total_tokens: 100 },
        openagents: {
          verification: "test_passed",
          cost_usd: 5,
          cost_msat: 1000,
          price_msat: 1200,
          receipt: "github:OpenAgentsInc/openagents#6016-receipt",
          coordinator_mode: "live_conductor",
          coordinator_promoted: true,
          settled: true,
          settlement_receipt_refs: [
            "github:OpenAgentsInc/openagents#6016-worker",
            "github:OpenAgentsInc/openagents#6016-validator",
          ],
          verse_playback_ref: "github:OpenAgentsInc/openagents#6016-verse",
          playable_in_world_ref: "github:OpenAgentsInc/openagents#6016-world",
          kwh_measured: 0.5,
          energy_measurement_ref: "github:OpenAgentsInc/openagents#6016-energy",
        },
      },
      startedAt: "2026-06-22T16:00:00.000Z",
      completedAt: "2026-06-22T16:01:00.000Z",
      wallClockMs: 60000,
      live: true,
    });
    expect(run.settlement.settled).toBe(true);
    expect(run.settlement.receiptRefs).toHaveLength(2);
    expect(run.verse.playbackRef).not.toBeNull();
    expect(run.artifact.playableInWorldRef).not.toBeNull();
    expect(run.energy.kwhMeasured).toBe(0.5);
    expect(run.coordinator.mode).toBe("live_conductor");
    expect(run.blockerRefs).not.toContain("blocker.khala_demo.m7_live_conductor_missing");
  });
});

describe("liveTransport wiring", () => {
  test("posts an OpenAI-compatible chat completion with bearer auth", async () => {
    const calls = [];
    const fakeFetch = async (url, init) => {
      calls.push({ url, init });
      return {
        ok: true,
        async json() {
          return { id: "x", choices: [], usage: { total_tokens: 1 }, openagents: {} };
        },
      };
    };

    await liveTransport({
      baseUrl: "https://openagents.com/v1/",
      token: "agent-token",
      model: "openagents/khala-code",
      prompt: CROSSY_ROAD_PROMPT,
      fetchImpl: fakeFetch,
    });

    expect(calls).toHaveLength(1);
    expect(calls[0].url).toBe("https://openagents.com/v1/chat/completions");
    expect(calls[0].init.headers.authorization).toBe("Bearer agent-token");
    const body = JSON.parse(calls[0].init.body);
    expect(body.model).toBe("openagents/khala-code");
    expect(body.messages[0].content).toBe(CROSSY_ROAD_PROMPT);
    expect(body.stream).toBe(false);
  });

  test("throws with a public-safe error on a non-ok response", async () => {
    const fakeFetch = async () => ({
      ok: false,
      status: 402,
      statusText: "Payment Required",
      async text() {
        return "insufficient credits";
      },
    });
    await expect(
      liveTransport({
        baseUrl: "https://openagents.com/v1",
        token: null,
        model: "openagents/khala-code",
        prompt: CROSSY_ROAD_PROMPT,
        fetchImpl: fakeFetch,
      }),
    ).rejects.toThrow("402");
  });
});

// Build the SSE wire bytes the gateway emits for a streamed completion: a series
// of `chat.completion.chunk` frames, the terminal `openagents` block on the FINAL
// chunk, then `data: [DONE]`.
function buildKhalaSseWire(deltas, { openagents, usage, finishReason = "stop" } = {}) {
  const frames = [];
  deltas.forEach((delta, index) => {
    const last = index === deltas.length - 1;
    frames.push(
      `data: ${JSON.stringify({
        id: "chatcmpl_stream_x",
        model: "openagents/khala-code",
        object: "chat.completion.chunk",
        choices: [
          {
            index: 0,
            delta: delta === "" ? {} : { content: delta },
            finish_reason: last ? finishReason : null,
          },
        ],
        ...(last && usage !== undefined ? { usage } : {}),
        ...(last && openagents !== undefined ? { openagents } : {}),
      })}\n\n`,
    );
  });
  return `${frames.join("")}data: [DONE]\n\n`;
}

describe("reconstructCompletionFromSse", () => {
  test("concatenates content deltas and attaches the terminal openagents block", () => {
    const wire = buildKhalaSseWire(["<!doctype ", "html>", "<!-- crossy -->"], {
      usage: { prompt_tokens: 10, completion_tokens: 200, total_tokens: 210 },
      openagents: {
        requested_model: "openagents/khala-code",
        served_model: "fireworks-coder",
        worker: "fireworks",
        lane: "coding",
        verification: "test_passed",
        verified: true,
        receipt: "oa_receipt_stream_1",
      },
    });
    const body = reconstructCompletionFromSse(wire);
    expect(body.choices[0].message.content).toBe(
      "<!doctype html><!-- crossy -->",
    );
    expect(body.choices[0].finish_reason).toBe("stop");
    expect(body.usage.total_tokens).toBe(210);
    expect(body.openagents.receipt).toBe("oa_receipt_stream_1");
    expect(body.openagents.verification).toBe("test_passed");

    // It feeds buildRunFromCompletion exactly like the non-streaming shape.
    const run = buildRunFromCompletion({
      lane: "khala",
      label: "stream",
      model: "openagents/khala-code",
      provider: "openagents",
      response: body,
      startedAt: "2026-06-22T16:00:00.000Z",
      completedAt: "2026-06-22T16:03:00.000Z",
      wallClockMs: 180000,
      live: true,
    });
    expect(run.usage.totalTokens).toBe(210);
    expect(run.acceptedOutcome.verificationClass).toBe("test_passed");
    expect(run.acceptedOutcome.receiptRef).toBe("oa_receipt_stream_1");
  });

  test("fires onToken per content delta", () => {
    const wire = buildKhalaSseWire(["a", "b", "c"]);
    const tokens = [];
    reconstructCompletionFromSse(wire, { onToken: (t) => tokens.push(t) });
    expect(tokens).toEqual(["a", "b", "c"]);
  });
});

describe("liveStreamTransport (the interactive default — 524 fix)", () => {
  test("posts stream:true and reconstructs the completion from a byte-stream body", async () => {
    const wire = buildKhalaSseWire(["full ", "answer"], {
      openagents: {
        requested_model: "openagents/khala-code",
        served_model: "fireworks-coder",
        worker: "fireworks",
        lane: "coding",
        verification: "test_passed",
        receipt: "oa_receipt_live_stream",
      },
    });
    let sentBody = null;
    const fakeFetch = async (_url, init) => {
      sentBody = JSON.parse(init.body);
      // A real ReadableStream body so the incremental reader path runs.
      const stream = new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode(wire));
          controller.close();
        },
      });
      return { ok: true, body: stream, async text() { return wire; } };
    };

    const tokens = [];
    const body = await liveStreamTransport({
      baseUrl: "https://openagents.com/v1",
      token: "agent-token",
      model: "openagents/khala-code",
      prompt: CROSSY_ROAD_PROMPT,
      fetchImpl: fakeFetch,
      onToken: (t) => tokens.push(t),
    });

    expect(sentBody.stream).toBe(true);
    expect(body.choices[0].message.content).toBe("full answer");
    expect(body.openagents.receipt).toBe("oa_receipt_live_stream");
    // Tokens streamed live during consumption.
    expect(tokens.join("")).toBe("full answer");
  });

  test("falls back to res.text() when the body has no reader", async () => {
    const wire = buildKhalaSseWire(["x", "y"]);
    const fakeFetch = async () => ({
      ok: true,
      async text() {
        return wire;
      },
    });
    const body = await liveStreamTransport({
      baseUrl: "https://openagents.com/v1",
      token: null,
      model: "openagents/khala-code",
      prompt: CROSSY_ROAD_PROMPT,
      fetchImpl: fakeFetch,
    });
    expect(body.choices[0].message.content).toBe("xy");
  });

  test("throws a public-safe error on a non-ok response", async () => {
    const fakeFetch = async () => ({
      ok: false,
      status: 524,
      statusText: "A Timeout Occurred",
      async text() {
        return "cloudflare 524";
      },
    });
    await expect(
      liveStreamTransport({
        baseUrl: "https://openagents.com/v1",
        token: null,
        model: "openagents/khala-code",
        prompt: CROSSY_ROAD_PROMPT,
        fetchImpl: fakeFetch,
      }),
    ).rejects.toThrow("524");
  });
});
