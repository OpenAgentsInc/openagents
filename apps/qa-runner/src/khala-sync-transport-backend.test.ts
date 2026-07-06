// khala-sync-transport backend tests (ST-6, #8512) — fakes-in-CI: injected
// transport + probe fetch, NO network, NO real deployment. Covers the incident
// taxonomy end-to-end: healthy live, connect 401 (the mobile WS-auth bug
// class) as a REFUTED finding, 403 denial, silent-retry loops, never-live
// bounds, auth resolution precedence + honest skip, self-registration, scope
// redaction, and the public-safety tripwire over every artifact.

import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type {
  BootstrapResponse,
  LogPage,
} from "@openagentsinc/khala-sync";
import {
  KhalaSyncTransportError,
  type KhalaSyncTransport,
} from "@openagentsinc/khala-sync-client";
import { Effect } from "effect";
import {
  classifyConnectOutcome,
  isConnectFinding,
  khalaSyncTransportCommitments,
  redactScopeForArtifact,
  resolveKhalaSyncAuthFromEnv,
  runKhalaSyncTransportScenario,
  selfRegisterThrowawayAgent,
  type ConnectAttemptOutcome,
} from "./khala-sync-transport-backend";
import { assertPublicSafeResult, decodeQaRunResult } from "./result";
import { makeTarget } from "./target";

const target = () =>
  makeTarget({ name: "fake-staging", baseUrl: "https://staging.example.test" });

const outDir = () => mkdtempSync(join(tmpdir(), "qa-khala-sync-"));

const USER_SCOPE = "scope.user.usr_fake_owner_1";

// ── Fake transport ───────────────────────────────────────────────────────────

const fakeBootstrapResponse = {
  scope: USER_SCOPE,
  entities: [
    { entityType: "chat_thread", entityId: "t1", postImageJson: "{}" },
  ],
  cursor: 7,
} as unknown as BootstrapResponse;

const fakeLogPage = { entries: [], nextCursor: 7 } as unknown as LogPage;

type ConnectBehavior =
  | { kind: "open" }
  | { kind: "reject"; error: KhalaSyncTransportError }
  | { kind: "hang" };

function fakeTransport(connect: ConnectBehavior): KhalaSyncTransport {
  return {
    bootstrap: () => Effect.succeed(fakeBootstrapResponse),
    logPage: () => Effect.succeed(fakeLogPage),
    push: () =>
      Effect.fail(
        new KhalaSyncTransportError("http_status", false, "not used", {
          status: 405,
        }),
      ),
    connectLive: () =>
      connect.kind === "open"
        ? Effect.succeed({ close: () => undefined })
        : connect.kind === "reject"
          ? Effect.fail(connect.error)
          : Effect.promise(() => new Promise(() => undefined)),
  };
}

const closedBeforeOpen = () =>
  new KhalaSyncTransportError(
    "network",
    true,
    "khala-sync live socket closed before opening",
  );

const probeReturning =
  (status: number, code?: string): typeof globalThis.fetch =>
  (async () =>
    new Response(
      JSON.stringify(
        code !== undefined
          ? { code, messageSafe: "probe", retryable: false }
          : {},
      ),
      { status },
    )) as unknown as typeof globalThis.fetch;

// ── Pure classification ──────────────────────────────────────────────────────

describe("classifyConnectOutcome", () => {
  const failedRetryable: ConnectAttemptOutcome = {
    kind: "failed",
    latencyMs: 5,
    retryable: true,
    reason: "network",
    accessDenied: false,
  };

  test("an observed open is live regardless of earlier failures", () => {
    expect(
      classifyConnectOutcome({
        attempts: [failedRetryable, { kind: "opened", latencyMs: 12 }],
      }),
    ).toBe("live");
  });

  test("probe 401 is the incident class: connect_unauthenticated", () => {
    expect(
      classifyConnectOutcome({
        attempts: [failedRetryable, failedRetryable],
        probe: { status: 401, syncErrorCode: "unauthenticated" },
      }),
    ).toBe("connect_unauthenticated");
  });

  test("probe 403 or a typed scope denial is connect_denied", () => {
    expect(
      classifyConnectOutcome({
        attempts: [failedRetryable],
        probe: { status: 403, syncErrorCode: "unauthorized_scope" },
      }),
    ).toBe("connect_denied");
    expect(
      classifyConnectOutcome({
        attempts: [
          {
            kind: "failed",
            latencyMs: 5,
            retryable: false,
            reason: "sync_error",
            syncErrorCode: "unauthorized_scope",
            accessDenied: true,
          },
        ],
      }),
    ).toBe("connect_denied");
  });

  test("all-retryable failures with a healthy auth probe is silent_retry_loop", () => {
    expect(
      classifyConnectOutcome({
        attempts: [failedRetryable, failedRetryable, failedRetryable],
        probe: { status: 426, syncErrorCode: "invalid_request" },
      }),
    ).toBe("silent_retry_loop");
  });

  test("a bound-exceeded attempt is never_live", () => {
    expect(
      classifyConnectOutcome({ attempts: [{ kind: "timeout", boundMs: 50 }] }),
    ).toBe("never_live");
  });

  test("only live is not a finding", () => {
    expect(isConnectFinding("live")).toBe(false);
    for (const c of [
      "connect_unauthenticated",
      "connect_denied",
      "silent_retry_loop",
      "never_live",
    ] as const) {
      expect(isConnectFinding(c)).toBe(true);
    }
  });
});

// ── Auth resolution ──────────────────────────────────────────────────────────

describe("resolveKhalaSyncAuthFromEnv", () => {
  test("QA_KHALA_SYNC_TOKEN wins, with its owner id", () => {
    const resolved = resolveKhalaSyncAuthFromEnv({
      QA_KHALA_SYNC_TOKEN: "oa_agent_qa",
      QA_KHALA_SYNC_OWNER_USER_ID: "usr_qa",
      OPENAGENTS_AGENT_TOKEN: "oa_agent_other",
    });
    expect(resolved.kind).toBe("resolved");
    if (resolved.kind !== "resolved") return;
    expect(resolved.auth.source).toBe("qa-env");
    expect(resolved.auth.value()).toBe("oa_agent_qa");
    expect(resolved.auth.ownerUserId).toBe("usr_qa");
  });

  test("falls back to the seeded mobile test credential", () => {
    const resolved = resolveKhalaSyncAuthFromEnv({
      KHALA_MOBILE_TEST_TOKEN: "oa_agent_mobile",
      KHALA_MOBILE_TEST_OWNER_USER_ID: "usr_mobile",
    });
    expect(resolved.kind).toBe("resolved");
    if (resolved.kind !== "resolved") return;
    expect(resolved.auth.source).toBe("mobile-test-env");
    expect(resolved.auth.ownerUserId).toBe("usr_mobile");
  });

  test("skips cleanly (unavailable, with a reason) when nothing is set", () => {
    const resolved = resolveKhalaSyncAuthFromEnv({});
    expect(resolved.kind).toBe("unavailable");
    if (resolved.kind !== "unavailable") return;
    expect(resolved.reason).toContain("QA_KHALA_SYNC_TOKEN");
  });
});

describe("selfRegisterThrowawayAgent", () => {
  test("registers against /api/agents/register and returns bearer + owner id", async () => {
    let seenUrl = "";
    const resolved = await selfRegisterThrowawayAgent({
      baseUrl: "https://staging.example.test/",
      runRef: "unit-run",
      fetchImpl: (async (url: string | URL | Request) => {
        seenUrl = String(url);
        return new Response(
          JSON.stringify({
            credential: { token: "oa_agent_fresh", tokenPrefix: "oa_agent_f" },
            user: { id: "usr_fresh", status: "active" },
          }),
          { status: 201 },
        );
      }) as unknown as typeof globalThis.fetch,
    });
    expect(seenUrl).toBe("https://staging.example.test/api/agents/register");
    expect(resolved.kind).toBe("resolved");
    if (resolved.kind !== "resolved") return;
    expect(resolved.auth.source).toBe("self-registered");
    expect(resolved.auth.value()).toBe("oa_agent_fresh");
    expect(resolved.auth.ownerUserId).toBe("usr_fresh");
  });

  test("a non-201 answer is honest unavailable, never a throw", async () => {
    const resolved = await selfRegisterThrowawayAgent({
      baseUrl: "https://staging.example.test",
      runRef: "unit-run",
      fetchImpl: (async () =>
        new Response("{}", { status: 429 })) as unknown as typeof globalThis.fetch,
    });
    expect(resolved.kind).toBe("unavailable");
  });
});

// ── Scope redaction ──────────────────────────────────────────────────────────

describe("redactScopeForArtifact", () => {
  test("public scopes stay verbatim; identifying scopes redact the id", () => {
    expect(redactScopeForArtifact("scope.public.tokens-served")).toBe(
      "scope.public.tokens-served",
    );
    expect(redactScopeForArtifact(USER_SCOPE)).toBe("scope.user.[redacted]");
    expect(redactScopeForArtifact("scope.thread.th_1")).toBe(
      "scope.thread.[redacted]",
    );
  });
});

// ── Scenario runs (fake transport, no network) ───────────────────────────────

const auth = () => ({
  source: "qa-env" as const,
  value: () => "oa_agent_fake_value",
  ownerUserId: "usr_fake_owner_1",
});

describe("runKhalaSyncTransportScenario", () => {
  test("healthy run: bootstrap + log page + connect all ok, verdict CONFIRMED", async () => {
    const outcome = await runKhalaSyncTransportScenario({
      target: target(),
      scope: USER_SCOPE,
      auth: auth(),
      artifactDir: outDir(),
      transport: fakeTransport({ kind: "open" }),
    });
    expect(outcome.result.status).toBe("pass");
    expect(outcome.classification).toBe("live");
    expect(outcome.result.backend).toBe("khala-sync-transport");
    expect(outcome.result.brain).toBe("khala-sync-transport-scenario");
    expect(outcome.result.verify?.verdict).toBe("CONFIRMED");
    // Latency detail is recorded per phase.
    const bootstrap = outcome.result.steps.find((s) => s.kind === "bootstrap");
    expect(bootstrap?.status).toBe("ok");
    expect(typeof bootstrap?.detail?.latencyMs).toBe("number");
    // result.json decodes through the shared schema.
    const persisted = decodeQaRunResult(
      JSON.parse(readFileSync(outcome.resultPath, "utf8")),
    );
    expect(persisted.status).toBe("pass");
  });

  test("the incident class: connect 401 -> connect_unauthenticated + REFUTED finding", async () => {
    const outcome = await runKhalaSyncTransportScenario({
      target: target(),
      scope: USER_SCOPE,
      auth: auth(),
      artifactDir: outDir(),
      transport: fakeTransport({ kind: "reject", error: closedBeforeOpen() }),
      probeFetch: probeReturning(401, "unauthenticated"),
      connectAttempts: 2,
    });
    expect(outcome.result.status).toBe("fail");
    expect(outcome.classification).toBe("connect_unauthenticated");
    expect(outcome.result.verify?.verdict).toBe("REFUTED");
    const finding = outcome.result.verify?.findings.find(
      (f) => f.id === "connect-reaches-live",
    );
    expect(finding?.verdict).toBe("REFUTED");
    // The report carries the typed classification + the probe verdict.
    const report = JSON.parse(readFileSync(outcome.reportPath, "utf8")) as {
      classification: string;
      finding: boolean;
      upgradeProbe: { status: number; syncErrorCode: string };
      connectAttempts: ReadonlyArray<{ retryable: boolean }>;
    };
    expect(report.classification).toBe("connect_unauthenticated");
    expect(report.finding).toBe(true);
    expect(report.upgradeProbe.status).toBe(401);
    expect(report.upgradeProbe.syncErrorCode).toBe("unauthenticated");
    // The retry-loop signature is visible: both attempts failed retryable.
    expect(report.connectAttempts).toHaveLength(2);
    expect(report.connectAttempts.every((a) => a.retryable)).toBe(true);
  });

  test("connect 403 via typed scope denial -> connect_denied (no probe needed)", async () => {
    const outcome = await runKhalaSyncTransportScenario({
      target: target(),
      scope: USER_SCOPE,
      auth: auth(),
      artifactDir: outDir(),
      transport: fakeTransport({
        kind: "reject",
        error: new KhalaSyncTransportError("http_status", false, "denied", {
          status: 403,
        }),
      }),
      probeFetch: null,
    });
    expect(outcome.result.status).toBe("fail");
    expect(outcome.classification).toBe("connect_denied");
    expect(outcome.result.verify?.verdict).toBe("REFUTED");
  });

  test("retryable failures with healthy probe auth -> silent_retry_loop", async () => {
    const outcome = await runKhalaSyncTransportScenario({
      target: target(),
      scope: USER_SCOPE,
      auth: auth(),
      artifactDir: outDir(),
      transport: fakeTransport({ kind: "reject", error: closedBeforeOpen() }),
      probeFetch: probeReturning(426, "invalid_request"),
      connectAttempts: 3,
    });
    expect(outcome.classification).toBe("silent_retry_loop");
    expect(outcome.result.status).toBe("fail");
    const connectStep = outcome.result.steps.find((s) => s.kind === "connect-live");
    expect(connectStep?.detail?.attempts).toBe(3);
  });

  test("a connect that never settles within the bound -> never_live", async () => {
    const outcome = await runKhalaSyncTransportScenario({
      target: target(),
      scope: USER_SCOPE,
      auth: auth(),
      artifactDir: outDir(),
      transport: fakeTransport({ kind: "hang" }),
      probeFetch: null,
      connectBoundMs: 25,
    });
    expect(outcome.classification).toBe("never_live");
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("never_live");
  });

  test("artifacts are public-safe: no bearer value, no raw scope id, tripwire green", async () => {
    const dir = outDir();
    const outcome = await runKhalaSyncTransportScenario({
      target: target(),
      scope: USER_SCOPE,
      auth: auth(),
      artifactDir: dir,
      transport: fakeTransport({ kind: "reject", error: closedBeforeOpen() }),
      probeFetch: probeReturning(401, "unauthenticated"),
      connectAttempts: 1,
    });
    for (const path of [outcome.resultPath, outcome.reportPath]) {
      const text = readFileSync(path, "utf8");
      expect(text).not.toContain("oa_agent_fake_value");
      expect(text).not.toContain("usr_fake_owner_1");
      assertPublicSafeResult(JSON.parse(text));
    }
    // The redacted scope ref IS present so a reviewer knows the scope kind.
    expect(readFileSync(outcome.reportPath, "utf8")).toContain(
      "scope.user.[redacted]",
    );
  });

  test("commitments name the three seam phases", () => {
    expect(khalaSyncTransportCommitments().map((c) => c.id)).toEqual([
      "bootstrap-ok",
      "log-page-ok",
      "connect-reaches-live",
    ]);
  });
});
