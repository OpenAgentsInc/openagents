#!/usr/bin/env bun

/**
 * Khala M8 head-to-head runner.
 *
 * Drives the crossy-road prompt against two lanes:
 *   - `khala`           -> OpenAgents OpenAI-compatible endpoint (model openagents/khala-code)
 *   - `frontier_baseline` -> a frontier OpenAI-compatible endpoint
 *
 * It collects tokens / cost / wall-clock and the `openagents` verification +
 * receipt refs per side, then emits a manifest in the EXACT shape consumed by
 * `reduce-head-to-head.mjs` (schema openagents.khala_head_to_head_evidence.v1).
 *
 * SAFETY / HONESTY RULES (enforced here, not just by the reducer):
 *   - The runner NEVER fabricates measurements. Fields the endpoint does not
 *     report stay null / empty / `not_measured` semantics, which keeps the
 *     reducer's closureAudit.canClose === false until real live evidence
 *     exists.
 *   - A stub/fixture endpoint produces a `fixture_scaffold` manifest. Only a
 *     real, non-stub endpoint produces `evidenceMode: "live"`, and even then
 *     all the missing live-evidence pieces (settlement receipts, Verse
 *     playback, in-world artifact, measured energy) remain blockers until they
 *     are actually present in the response.
 *   - No secrets, tokens, or local paths are written into the manifest.
 *
 * Usage:
 *   bun scripts/khala-demo/run-head-to-head.mjs [options]
 *
 * Options (all also read from env):
 *   --khala-base-url <url>      KHALA_BASE_URL      OpenAI-compatible base, e.g. https://openagents.com/v1
 *   --khala-token <token>       KHALA_AGENT_TOKEN   bearer token for the Khala lane
 *   --khala-model <id>          KHALA_MODEL         default openagents/khala-code
 *   --frontier-base-url <url>   FRONTIER_BASE_URL   OpenAI-compatible base for the baseline
 *   --frontier-token <token>    FRONTIER_TOKEN      bearer token for the baseline lane
 *   --frontier-model <id>       FRONTIER_MODEL      default frontier-baseline
 *   --stub                      KHALA_RUNNER_STUB=1 force the built-in deterministic stub transport
 *   --out <path>                write the manifest JSON to a file instead of stdout
 *
 * If no live base URL is supplied (or --stub is passed), the runner uses a
 * deterministic, public-safe stub transport so the harness is exercised end to
 * end without the owner-gated live gateway.
 */

import { writeFileSync } from "node:fs";

export const NOT_MEASURED = "not_measured";

export const CROSSY_ROAD_PROMPT =
  "build a really high quality single html file crossy road game with three.js";

const MANIFEST_SCHEMA = "openagents.khala_head_to_head_evidence.v1";
const DEFAULT_KHALA_MODEL = "openagents/khala-code";
const DEFAULT_FRONTIER_MODEL = "frontier-baseline";

const SCOPE = {
  issueRef: "github:OpenAgentsInc/openagents#6016",
  parentIssueRef: "github:OpenAgentsInc/openagents#6017",
  roadmapRef:
    "docs/inference/khala-buildout-roadmap.md#agent-demo--benchmark-metrics-and-publication-pack",
  runbookRef: "docs/inference/khala-head-to-head-demo.md",
  prompt: CROSSY_ROAD_PROMPT,
  benchmarkRef: "benchmark.khala.crossy_road_threejs.single_html.v1",
};

/**
 * Convert provider msat figures to USD only when we are explicitly told a
 * conversion rate. We never invent a price; without an msat figure and a rate
 * the dollars value stays 0 and the missing-cost blocker is recorded.
 */
function msatToUsd(msat, msatPerUsd) {
  if (
    typeof msat !== "number" ||
    !Number.isFinite(msat) ||
    typeof msatPerUsd !== "number" ||
    !Number.isFinite(msatPerUsd) ||
    msatPerUsd <= 0
  ) {
    return null;
  }
  return msat / msatPerUsd;
}

function numberOrNull(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/**
 * Build a single manifest `run` from an OpenAI-compatible chat completion
 * response and the wall-clock we measured around the call.
 *
 * `live` controls whether this run is allowed to claim `live` evidence. Even
 * when `live` is true, every field that the response did not actually provide
 * is recorded as missing (null / [] / blocker), never fabricated.
 */
export function buildRunFromCompletion({
  lane,
  label,
  model,
  provider,
  response,
  startedAt,
  completedAt,
  wallClockMs,
  live,
  msatPerUsd = null,
}) {
  const evidenceMode = live ? "live" : "fixture_scaffold";
  const oa = (response && typeof response === "object" && response.openagents) || {};
  const usage = (response && typeof response === "object" && response.usage) || {};

  const promptTokens = numberOrNull(usage.prompt_tokens) ?? 0;
  const completionTokens = numberOrNull(usage.completion_tokens) ?? 0;
  const totalTokens =
    numberOrNull(usage.total_tokens) ?? promptTokens + completionTokens;

  const costMsat = numberOrNull(oa.cost_msat) ?? 0;
  const priceMsat = numberOrNull(oa.price_msat) ?? costMsat;

  // Cost in USD is only real if the provider gave us cost_msat AND we were
  // given a conversion rate. Otherwise we keep 0 and flag it as a blocker so
  // the reducer's cost-per-accepted-outcome is honest.
  const derivedUsd = msatToUsd(costMsat, msatPerUsd);
  const costUsd = numberOrNull(oa.cost_usd) ?? derivedUsd ?? 0;

  const verificationClass =
    typeof oa.verification === "string" && oa.verification.length > 0
      ? oa.verification
      : "none";
  const accepted =
    verificationClass !== "none" &&
    verificationClass !== "seeded" &&
    oa.accepted !== false;

  const receiptRef =
    typeof oa.receipt === "string" && oa.receipt.length > 0
      ? oa.receipt
      : `${lane}.receipt.missing.${evidenceMode}`;
  const verdictRef =
    typeof oa.verdict_ref === "string" && oa.verdict_ref.length > 0
      ? oa.verdict_ref
      : `${lane}.verdict.${verificationClass}.${evidenceMode}`;
  const verifierRef =
    typeof oa.verifier_ref === "string" && oa.verifier_ref.length > 0
      ? oa.verifier_ref
      : `${lane}.verifier.missing.${evidenceMode}`;
  const artifactRef =
    typeof oa.artifact_ref === "string" && oa.artifact_ref.length > 0
      ? oa.artifact_ref
      : `${lane}.artifact.crossy_road.single_html.${evidenceMode}`;

  const coordinatorMode =
    typeof oa.coordinator_mode === "string" && oa.coordinator_mode.length > 0
      ? oa.coordinator_mode
      : lane === "khala"
        ? "fixture_conductor_shape"
        : "single_model";

  // Settlement: only claim settled when the response explicitly says so AND
  // gives us worker/validator receipt refs.
  const settlementReceiptRefs = Array.isArray(oa.settlement_receipt_refs)
    ? oa.settlement_receipt_refs.filter((ref) => typeof ref === "string" && ref.length > 0)
    : [];
  const settled = oa.settled === true && settlementReceiptRefs.length > 0;

  // Verse playback / in-world artifact: only present if the response provided
  // them. The M8 head-to-head live gateway is owner-gated, so by default these
  // are missing and become blockers.
  const versePlaybackRef =
    typeof oa.verse_playback_ref === "string" && oa.verse_playback_ref.length > 0
      ? oa.verse_playback_ref
      : null;
  const playableInWorldRef =
    typeof oa.playable_in_world_ref === "string" && oa.playable_in_world_ref.length > 0
      ? oa.playable_in_world_ref
      : null;
  const inWorldWorkUnits = numberOrNull(oa.in_world_work_units) ?? 0;
  const gatewayWorkUnits = numberOrNull(oa.gateway_work_units) ?? 0;

  // Energy telemetry: never estimated. Only a measured kWh + measurement ref
  // counts; otherwise AO/kWh stays not_measured downstream.
  const kwhMeasured = numberOrNull(oa.kwh_measured);
  const energyMeasurementRef =
    typeof oa.energy_measurement_ref === "string" && oa.energy_measurement_ref.length > 0
      ? oa.energy_measurement_ref
      : null;

  const acceptedBlockers = [];
  if (verificationClass === "none") {
    acceptedBlockers.push("blocker.khala_demo.verifier_verdict_missing");
  }
  if (!live) {
    acceptedBlockers.push("blocker.khala_demo.fixture_verifier_not_live");
  }

  const artifactBlockers = [];
  if (playableInWorldRef === null) {
    artifactBlockers.push(
      lane === "khala"
        ? "blocker.khala_demo.artifact_not_playable_in_world"
        : "blocker.khala_demo.frontier_artifact_not_playable_in_world",
    );
  }

  const settlementBlockers = [];
  if (!settled) {
    if (lane === "khala") {
      settlementBlockers.push("blocker.khala_demo.worker_settlement_missing");
      settlementBlockers.push("blocker.khala_demo.validator_settlement_missing");
    } else {
      settlementBlockers.push("blocker.khala_demo.frontier_no_settlement_expected");
    }
  }

  const verseBlockers = [];
  if (versePlaybackRef === null) {
    verseBlockers.push(
      lane === "khala"
        ? "blocker.khala_demo.verse_playback_missing"
        : "blocker.khala_demo.frontier_verse_playback_missing",
    );
  } else if (!live) {
    verseBlockers.push("blocker.khala_demo.verse_playback_fixture_only");
  }

  const energyBlockers = [];
  if (kwhMeasured === null || kwhMeasured <= 0 || energyMeasurementRef === null) {
    energyBlockers.push("blocker.khala_demo.energy_telemetry_missing");
  }

  const costBlockers = [];
  if (costUsd === 0) {
    costBlockers.push("blocker.khala_demo.cost_usd_not_measured");
  }

  const runBlockers = [];
  if (!live) {
    runBlockers.push("blocker.khala_demo.fixture_scaffold_not_live");
  }
  if (lane === "khala" && coordinatorMode !== "live_conductor") {
    runBlockers.push("blocker.khala_demo.m7_live_conductor_missing");
  }

  return {
    runId: `${lane}.crossy_road.${evidenceMode}.v1`,
    lane,
    label,
    model,
    provider,
    coordinator: {
      mode: coordinatorMode,
      policyRef:
        typeof oa.policy_ref === "string" && oa.policy_ref.length > 0
          ? oa.policy_ref
          : `${lane}.policy.${evidenceMode}.v1`,
      promoted: oa.coordinator_promoted === true,
    },
    evidenceMode,
    startedAt,
    completedAt,
    wallClockMs,
    usage: {
      promptTokens,
      completionTokens,
      totalTokens,
    },
    costUsd,
    costMsat,
    priceMsat,
    acceptedOutcome: {
      accepted,
      verificationClass,
      verdictRef,
      verifierRef,
      receiptRef,
      evidenceRefs: [artifactRef],
      blockerRefs: acceptedBlockers,
    },
    artifact: {
      kind: "single_html",
      artifactRef,
      playableInWorldRef,
      blockerRefs: artifactBlockers,
    },
    settlement: {
      settled,
      receiptRefs: settlementReceiptRefs,
      blockerRefs: settlementBlockers,
    },
    verse: {
      playbackRef: versePlaybackRef,
      sourceRefs: versePlaybackRef === null ? [] : [receiptRef],
      inWorldWorkUnits,
      gatewayWorkUnits,
      blockerRefs: verseBlockers,
    },
    energy: {
      kwhMeasured,
      measurementRef: energyMeasurementRef,
      blockerRefs: energyBlockers,
    },
    sourceRefs: [
      "docs/inference/khala-buildout-roadmap.md",
      "docs/inference/khala-head-to-head-demo.md",
    ],
    blockerRefs: [...runBlockers, ...costBlockers],
  };
}

/**
 * Deterministic, public-safe stub transport. It mimics the OpenAI-compatible
 * shape (choices + usage + the non-breaking `openagents` block) without any
 * settlement, Verse, in-world, or energy telemetry, so that a stub run keeps
 * the closure audit blocked exactly like the fixture scaffold.
 */
export function stubTransport({ lane, model }) {
  if (lane === "khala") {
    return {
      id: "chatcmpl_stub_khala",
      model,
      choices: [
        {
          index: 0,
          finish_reason: "stop",
          message: { role: "assistant", content: "<!doctype html><!-- stub crossy-road artifact -->" },
        },
      ],
      usage: { prompt_tokens: 18420, completion_tokens: 71180, total_tokens: 89600 },
      openagents: {
        receipt: "stub.receipt.khala.crossy_road.accepted.v1",
        route: "coding",
        workers: ["stub-coding-worker", "stub-validator"],
        verification: "test_passed",
        cost_msat: 1150000,
        price_msat: 1540000,
        settled: false,
        // Deliberately NO settlement_receipt_refs, verse_playback_ref,
        // playable_in_world_ref, kwh_measured, or live_conductor mode: the
        // stub is honest about being incomplete evidence.
        in_world_work_units: 3,
        gateway_work_units: 2,
      },
    };
  }
  return {
    id: "chatcmpl_stub_frontier",
    model,
    choices: [
      {
        index: 0,
        finish_reason: "stop",
        message: { role: "assistant", content: "<!doctype html><!-- stub frontier artifact -->" },
      },
    ],
    usage: { prompt_tokens: 21000, completion_tokens: 919000, total_tokens: 940000 },
    openagents: {
      receipt: "stub.receipt.frontier.crossy_road.accepted.v1",
      route: "default",
      workers: ["stub-frontier-model"],
      verification: "test_passed",
      cost_msat: 5947000,
      price_msat: 5947000,
      settled: false,
      in_world_work_units: 0,
      gateway_work_units: 1,
    },
  };
}

function parseSseFrames(text) {
  const frames = [];
  for (const rawFrame of text.split(/\r?\n\r?\n/)) {
    const dataLines = rawFrame
      .split(/\r?\n/)
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).replace(/^ /, ""));
    if (dataLines.length === 0) {
      continue;
    }
    const data = dataLines.join("\n");
    if (data === "[DONE]") {
      break;
    }
    try {
      frames.push(JSON.parse(data));
    } catch (error) {
      throw new Error(`stream frame was not JSON: ${error?.message ?? error}`);
    }
  }
  return frames;
}

export function completionFromSseText(text, fallbackModel) {
  const frames = parseSseFrames(text);
  if (frames.length === 0) {
    throw new Error("stream ended without chat completion chunks");
  }

  let id = null;
  let model = fallbackModel;
  let finishReason = null;
  let openagents = undefined;
  let usage = undefined;
  const contentParts = [];

  for (const frame of frames) {
    if (typeof frame.id === "string" && frame.id.length > 0) {
      id = frame.id;
    }
    if (typeof frame.model === "string" && frame.model.length > 0) {
      model = frame.model;
    }
    if (frame.openagents !== undefined) {
      openagents = frame.openagents;
    }
    if (frame.usage !== undefined) {
      usage = frame.usage;
    }

    const choice = Array.isArray(frame.choices) ? frame.choices[0] : undefined;
    const delta = choice && typeof choice === "object" ? choice.delta : undefined;
    if (delta && typeof delta.content === "string") {
      contentParts.push(delta.content);
    }
    if (choice && typeof choice.finish_reason === "string") {
      finishReason = choice.finish_reason;
    }
  }

  return {
    choices: [
      {
        finish_reason: finishReason ?? "stop",
        index: 0,
        message: { content: contentParts.join(""), role: "assistant" },
      },
    ],
    id: id ?? "chatcmpl_stream",
    model,
    object: "chat.completion",
    ...(usage === undefined ? {} : { usage }),
    ...(openagents === undefined ? {} : { openagents }),
  };
}

/**
 * Live transport: POST an OpenAI-compatible streaming chat completion. The
 * gateway emits OpenAI SSE chunks and attaches `openagents` only to the terminal
 * frame, so the runner reconstructs a normal completion envelope before feeding
 * the existing manifest builder.
 */
export async function liveTransport({ baseUrl, token, model, prompt, fetchImpl = fetch }) {
  const url = `${baseUrl.replace(/\/$/, "")}/chat/completions`;
  const headers = { "content-type": "application/json" };
  if (token) {
    headers.authorization = `Bearer ${token}`;
  }
  const res = await fetchImpl(url, {
    method: "POST",
    headers,
    body: JSON.stringify({
      model,
      messages: [{ role: "user", content: prompt }],
      temperature: 0.2,
      stream: true,
    }),
  });
  if (!res.ok) {
    const detail = await res.text().catch(() => "");
    throw new Error(`lane request failed: ${res.status} ${res.statusText} ${detail.slice(0, 200)}`);
  }
  const text = await res.text();
  return completionFromSseText(text, model);
}

/**
 * Run one lane: time the transport call, then build the manifest run.
 */
export async function runLane({ lane, label, model, provider, transport, live, msatPerUsd, clock = Date.now }) {
  const startMs = clock();
  const startedAt = new Date(startMs).toISOString();
  const response = await transport({ lane, model });
  const endMs = clock();
  const completedAt = new Date(endMs).toISOString();
  const wallClockMs = Math.max(0, endMs - startMs);

  return buildRunFromCompletion({
    lane,
    label,
    model,
    provider,
    response,
    startedAt,
    completedAt,
    wallClockMs,
    live,
    msatPerUsd,
  });
}

/**
 * Drive both lanes and assemble the head-to-head manifest.
 *
 * `live` is true only when BOTH lanes have a real base URL and the stub was not
 * forced. The manifest's top-level evidenceMode and per-run evidenceMode follow
 * from that. The external reported claims and publication block stay as
 * unverified / draft scaffolding — promoting them is owner-gated and out of the
 * runner's scope.
 */
export async function runHeadToHead(options = {}) {
  const {
    khalaTransport,
    frontierTransport,
    live = false,
    msatPerUsd = null,
    khalaModel = DEFAULT_KHALA_MODEL,
    frontierModel = DEFAULT_FRONTIER_MODEL,
    khalaProvider = live ? "openagents" : "openagents-stub",
    frontierProvider = live ? "external" : "external-stub",
    generatedAt = new Date().toISOString(),
    clock = Date.now,
  } = options;

  const khalaRun = await runLane({
    lane: "khala",
    label: live ? "Khala live run" : "Khala stub run",
    model: khalaModel,
    provider: khalaProvider,
    transport: khalaTransport,
    live,
    msatPerUsd,
    clock,
  });

  const frontierRun = await runLane({
    lane: "frontier_baseline",
    label: live ? "Frontier baseline live run" : "Frontier baseline stub run",
    model: frontierModel,
    provider: frontierProvider,
    transport: frontierTransport,
    live,
    msatPerUsd,
    clock,
  });

  const evidenceMode = live ? "live" : "fixture_scaffold";

  return {
    schema: MANIFEST_SCHEMA,
    manifestRef: live
      ? "live.khala.head_to_head.demo.v1"
      : "stub.khala.head_to_head.demo.v1",
    evidenceMode,
    generatedAt,
    scope: SCOPE,
    runs: [khalaRun, frontierRun],
    externalReportedClaims: [
      {
        claimRef: "reported.sakana_fugu_ultra.crossy_road.external_unverified.v1",
        label: "Sakana Fugu Ultra reported comparison",
        citationStatus: "reported_without_primary_url",
        tokens: 89000,
        costUsd: 7.32,
        wallClockMs: 1320000,
        verdictSummary:
          "Reported faster and cheaper; issues included inverted turn direction, wonky camera, no SFX, and not identical to Crossy Road.",
        blockerRefs: ["blocker.khala_demo.external_claim_primary_url_missing"],
      },
      {
        claimRef: "reported.claude_opus_4_8_ultracode.crossy_road.external_unverified.v1",
        label: "Claude Opus 4.8 Ultracode reported comparison",
        citationStatus: "reported_without_primary_url",
        tokens: 940000,
        costUsd: 37.85,
        wallClockMs: 4740000,
        verdictSummary:
          "Reported higher quality; issues included retry loops and wrong restart character position.",
        blockerRefs: ["blocker.khala_demo.external_claim_primary_url_missing"],
      },
    ],
    publication: {
      status: "draft_scaffold",
      publicationRef: null,
      claimUpgradeRefs: [],
      blockerRefs: [
        "blocker.khala_demo.live_runs_missing",
        "blocker.khala_demo.publication_missing",
        "blocker.khala_demo.owner_signed_claim_upgrade_missing",
      ],
    },
  };
}

function parseArgs(argv) {
  const opts = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--stub") {
      opts.stub = true;
      continue;
    }
    if (arg.startsWith("--")) {
      const key = arg.slice(2);
      const next = argv[i + 1];
      if (next === undefined || next.startsWith("--")) {
        opts[key] = true;
      } else {
        opts[key] = next;
        i += 1;
      }
    }
  }
  return opts;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const env = process.env;

  const khalaBaseUrl = args["khala-base-url"] || env.KHALA_BASE_URL || null;
  const khalaToken = args["khala-token"] || env.KHALA_AGENT_TOKEN || null;
  const khalaModel = args["khala-model"] || env.KHALA_MODEL || DEFAULT_KHALA_MODEL;
  const frontierBaseUrl = args["frontier-base-url"] || env.FRONTIER_BASE_URL || null;
  const frontierToken = args["frontier-token"] || env.FRONTIER_TOKEN || null;
  const frontierModel = args["frontier-model"] || env.FRONTIER_MODEL || DEFAULT_FRONTIER_MODEL;
  const msatPerUsd = numberOrNull(Number(args["msat-per-usd"] || env.KHALA_MSAT_PER_USD));

  const forceStub = args.stub === true || env.KHALA_RUNNER_STUB === "1";
  // Live only when the stub is not forced AND both lanes have a real base URL.
  const live = !forceStub && Boolean(khalaBaseUrl) && Boolean(frontierBaseUrl);

  const khalaTransport = live
    ? () =>
        liveTransport({
          baseUrl: khalaBaseUrl,
          token: khalaToken,
          model: khalaModel,
          prompt: CROSSY_ROAD_PROMPT,
        })
    : stubTransport;
  const frontierTransport = live
    ? () =>
        liveTransport({
          baseUrl: frontierBaseUrl,
          token: frontierToken,
          model: frontierModel,
          prompt: CROSSY_ROAD_PROMPT,
        })
    : stubTransport;

  const manifest = await runHeadToHead({
    khalaTransport,
    frontierTransport,
    live,
    msatPerUsd,
    khalaModel,
    frontierModel,
  });

  const json = `${JSON.stringify(manifest, null, 2)}\n`;
  const outPath = args.out;
  if (typeof outPath === "string") {
    writeFileSync(outPath, json);
    process.stderr.write(
      `wrote ${manifest.evidenceMode} manifest to ${outPath} (live=${live})\n`,
    );
  } else {
    process.stdout.write(json);
  }
}

if (import.meta.main) {
  main().catch((error) => {
    process.stderr.write(`run-head-to-head failed: ${error?.message ?? error}\n`);
    process.exit(1);
  });
}
