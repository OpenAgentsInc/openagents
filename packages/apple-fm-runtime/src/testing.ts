import { APPLE_FM_CANONICAL_HELPER_VERSION, APPLE_FM_DEFAULT_MODEL_ID } from "./identity.js";
import type { AppleFmCompletionTurn, AppleFmProbe } from "./client.js";

/**
 * `@openagentsinc/apple-fm-runtime/testing` — fixtures and a fake transport
 * (AFS-02).
 *
 * These are the shared contract/conformance/lifecycle/wire/decoder fixtures.
 * Desktop and Pylon decode the SAME fixtures through their thin adapters, so a
 * shape drift is caught in one place. Fixtures carry NO helper path, URL,
 * token, raw tool result, or private file data.
 */

export const APPLE_FM_RUNTIME_TESTING_RESERVED = true as const;

/** A ready health payload the fake bridge returns, pinned to the single source. */
export const appleFmHealthFixture = {
  ready: true,
  model: APPLE_FM_DEFAULT_MODEL_ID,
  modelId: APPLE_FM_DEFAULT_MODEL_ID,
  platform: "darwin",
  version: APPLE_FM_CANONICAL_HELPER_VERSION,
} as const;

/** A valid plain-text answer completion payload from the bridge. */
export const appleFmAnswerCompletionFixture = {
  model: APPLE_FM_DEFAULT_MODEL_ID,
  choices: [{ index: 0, message: { role: "assistant", content: "Hello there" }, finishReason: "stop" }],
  usage: { truth: "estimated", promptTokens: 3, completionTokens: 2, totalTokens: 5 },
} as const;

/** A valid structured route-recommendation the local model may emit as JSON. */
export const appleFmRecommendationJsonFixture = JSON.stringify({
  candidate: "codex",
  taskClass: "delegate",
  reasonCode: "needs_delegation",
  confidence: 0.82,
});

/** A recommendation whose lane is NOT in the owner-bound set (unavailable-agent). */
export const appleFmUnavailableAgentJsonFixture = JSON.stringify({
  candidate: "grok_acp",
  taskClass: "delegate",
  reasonCode: "needs_delegation",
  confidence: 0.9,
});

/** A structured output claiming action authority (must never dispatch). */
export const appleFmActionClaimJsonFixture = JSON.stringify({
  candidate: "codex",
  taskClass: "delegate",
  reasonCode: "needs_delegation",
  confidence: 0.9,
  action: { tool: "shell", command: "rm -rf /" },
});

/** A malformed structured route attempt (bad confidence, unknown fields). */
export const appleFmMalformedRecommendationJsonFixture = JSON.stringify({
  candidate: "codex",
  taskClass: "delegate",
  reasonCode: "needs_delegation",
  confidence: 42,
});

/** A plain advisory answer (no structured route) — safe answer fallback. */
export const appleFmPlainAnswerFixture = "You can read the README with `cat README.md`.";

/** An empty output (must refuse, never dispatch). */
export const appleFmEmptyOutputFixture = "   ";

/** A completion turn fixture for the answer path. */
export const appleFmAnswerTurnFixture: AppleFmCompletionTurn = {
  outcome: "completed",
  text: "Hello there",
  usageTruth: "estimated",
  promptTokens: 3,
  completionTokens: 2,
  totalTokens: 5,
};

/** A ready probe fixture. */
export const appleFmReadyProbeFixture: AppleFmProbe = {
  status: "ready",
  ready: true,
  model: APPLE_FM_DEFAULT_MODEL_ID,
  profileId: "apple-fm-local",
  usageTruth: "estimated",
};

export interface FakeAppleFmBridgeOptions {
  readonly ready?: boolean;
  readonly healthBody?: unknown;
  /** Raw (possibly non-JSON) health body — for the malformed-response path. */
  readonly healthRawText?: string;
  readonly completionBody?: unknown;
  readonly completionStatus?: number;
  readonly healthStatus?: number;
  /** Force a transport failure on every request. */
  readonly unreachable?: boolean;
}

/**
 * A deterministic fake `fetch` for the Apple FM loopback bridge. It answers the
 * `/health` and `/v1/chat/completions` endpoints from injected bodies, with no
 * network, so contract/conformance/lifecycle tests run on any host.
 */
export const makeFakeAppleFmBridge = (options: FakeAppleFmBridgeOptions = {}): typeof fetch => {
  const healthBody = options.healthBody ?? (options.ready === false ? { ready: false, unavailableReason: "not_ready" } : appleFmHealthFixture);
  const completionBody = options.completionBody ?? appleFmAnswerCompletionFixture;
  const fakeFetch = async (input: RequestInfo | URL): Promise<Response> => {
    if (options.unreachable === true) throw new Error("connect ECONNREFUSED 127.0.0.1:11435");
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    if (url.endsWith("/health")) {
      return new Response(options.healthRawText ?? JSON.stringify(healthBody), {
        status: options.healthStatus ?? 200,
        headers: { "content-type": "application/json" },
      });
    }
    if (url.includes("/chat/completions")) {
      return new Response(JSON.stringify(completionBody), {
        status: options.completionStatus ?? 200,
        headers: { "content-type": "application/json" },
      });
    }
    return new Response("{}", { status: 404, headers: { "content-type": "application/json" } });
  };
  return fakeFetch as typeof fetch;
};
