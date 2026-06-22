import { describe, expect, test } from "bun:test";

import { reduceKhalaHeadToHeadManifest } from "./reduce-head-to-head.mjs";
import {
  buildRunFromCompletion,
  completionFromSseText,
  CROSSY_ROAD_PROMPT,
  liveTransport,
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
  test("parses OpenAI SSE chunks into the completion envelope consumed by the runner", () => {
    const completion = completionFromSseText(
      [
        'data: {"id":"chatcmpl_stream","model":"openagents/khala-code","choices":[{"index":0,"delta":{"content":"<!doctype html>"},"finish_reason":null}]}',
        "",
        'data: {"id":"chatcmpl_stream","model":"openagents/khala-code","choices":[{"index":0,"delta":{"content":"<!-- crossy road -->"},"finish_reason":null}]}',
        "",
        'data: {"id":"chatcmpl_stream","model":"openagents/khala-code","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"openagents":{"receipt":"receipt.live","verification":"test_passed","cost_msat":42},"usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30}}',
        "",
        "data: [DONE]",
        "",
      ].join("\n"),
      "openagents/khala-code",
    );

    expect(completion.choices[0].message.content).toBe(
      "<!doctype html><!-- crossy road -->",
    );
    expect(completion.choices[0].finish_reason).toBe("stop");
    expect(completion.openagents.receipt).toBe("receipt.live");
    expect(completion.usage.total_tokens).toBe(30);
  });

  test("posts an OpenAI-compatible streaming chat completion with bearer auth", async () => {
    const calls = [];
    const fakeFetch = async (url, init) => {
      calls.push({ url, init });
      return new Response(
        [
          'data: {"id":"x","model":"openagents/khala-code","choices":[{"index":0,"delta":{"content":"hello"},"finish_reason":null}]}',
          "",
          'data: {"id":"x","model":"openagents/khala-code","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"openagents":{"receipt":"receipt.live","verification":"test_passed"}}',
          "",
          "data: [DONE]",
          "",
        ].join("\n"),
        { headers: { "content-type": "text/event-stream" } },
      );
    };

    const completion = await liveTransport({
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
    expect(body.stream).toBe(true);
    expect(completion.choices[0].message.content).toBe("hello");
    expect(completion.openagents.receipt).toBe("receipt.live");
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
