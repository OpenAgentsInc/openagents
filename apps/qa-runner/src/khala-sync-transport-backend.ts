// Khala Sync TRANSPORT backend for the qa-runner (ST-6, #8512).
//
// The explorer pointed at a SEAM, not a screen: a HEADLESS target that drives
// the REAL `createHttpKhalaSyncTransport` from `@openagentsinc/khala-sync-client`
// (bootstrap -> log page -> WebSocket connect) against a live deployment with a
// real cookie-less bearer, and CLASSIFIES the outcome. This is the standing
// generalization of the 2026-07-06 mobile "Loading threads forever" incident
// (docs/fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md §R6): the bug —
// `/api/sync/connect` never reading the `?token=` query bearer — was invisible
// to every fakes-on-both-sides test layer and was found in minutes by a
// throwaway script driving this exact seam. This backend makes that diagnosis
// loop a permanent, per-PR-runnable target.
//
// It is the terminal-backend's sibling: no browser, no video. It records the
// same public-safe `QaRunResult` (brain="khala-sync-transport-scenario",
// backend="khala-sync-transport"), one step per transport phase with latency
// detail, plus a `khala-sync-report.json` carrying the typed classification.
// Commitments run through the standard verify stage (#6192), so a connect that
// 401s or never reaches live is a REFUTED finding in the ledger — never a
// silent retry that presents as "loading".
//
// CLASSIFICATIONS (the incident taxonomy):
//   live                     — the WebSocket opened within the bound. Healthy.
//   connect_unauthenticated  — the WS never opened AND a raw pre-upgrade probe
//                              of /api/sync/connect answered 401: the seam is
//                              refusing the bearer (the exact incident bug).
//   connect_denied           — probe answered 403 (or the transport surfaced a
//                              typed unauthorized_scope/unknown_scope error):
//                              scope access is denied; terminal, not retryable.
//   silent_retry_loop        — every connect attempt failed with a RETRYABLE
//                              transport error while the probe says auth is
//                              fine (426 upgrade-required) or is unreachable:
//                              the client session would back off and retry
//                              forever without surfacing an error.
//   never_live               — a connect attempt neither opened nor failed
//                              within the bound (phase never reaches `live`).
//
// Determinism + fakes-in-CI: the transport, the probe fetch, and the clock are
// injectable. Unit tests inject fakes (no network); the real path builds the
// production HTTP transport. Auth is env-resolved or self-registered — NEVER
// hardcoded — and the raw bearer value never reaches any artifact (labels
// only; the public-safety tripwire re-checks every write).

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  BootstrapRequest,
  ClientGroupId,
  SyncSchemaVersion,
  SyncScope,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync";
import {
  createHttpKhalaSyncTransport,
  isAccessDeniedSignal,
  KHALA_SYNC_CONNECT_PATH,
  KhalaSyncTransportError,
  type KhalaSyncTransport,
} from "@openagentsinc/khala-sync-client";
import { Effect, Schema as S } from "effect";
import { assertPublicSafeResult, type QaRunResult, type QaRunStep } from "./result";
import type { Target } from "./target";
import { verifyCommitments, type Commitment } from "./verify";

const decodeScope = S.decodeUnknownSync(SyncScope);

// ---------------------------------------------------------------------------
// Auth resolution — env-first, self-register fallback, honest skip otherwise.
// ---------------------------------------------------------------------------

/** A resolved bearer identity for the run. The raw value stays inside
 *  `value()` and is never written to any artifact — only `source` is. */
export interface KhalaSyncRunAuth {
  /** Public-safe LABEL of where the bearer came from (never the value). */
  readonly source: "qa-env" | "agent-env" | "mobile-test-env" | "self-registered";
  /** The bearer value, re-read per request (matches the transport contract). */
  readonly value: () => string;
  /** The user id owning `scope.user.<id>`, when known for this bearer. */
  readonly ownerUserId?: string;
}

export type KhalaSyncAuthResolution =
  | { readonly kind: "resolved"; readonly auth: KhalaSyncRunAuth }
  | { readonly kind: "unavailable"; readonly reason: string };

/**
 * Resolve a bearer from env, in precedence order (#8512 req: env var or
 * self-registration, never hardcoded; skip cleanly when nothing resolves):
 *   1. QA_KHALA_SYNC_TOKEN (+ optional QA_KHALA_SYNC_OWNER_USER_ID)
 *   2. OPENAGENTS_AGENT_TOKEN
 *   3. KHALA_MOBILE_TEST_TOKEN (+ KHALA_MOBILE_TEST_OWNER_USER_ID) — the
 *      seeded mobile emulator credential the incident repro used.
 * Pure env read; the self-registration fallback is a separate, explicit
 * network step (`selfRegisterThrowawayAgent`) the CLI opts into.
 */
export function resolveKhalaSyncAuthFromEnv(
  env: Readonly<Record<string, string | undefined>> = process.env,
): KhalaSyncAuthResolution {
  const candidates: ReadonlyArray<{
    readonly source: KhalaSyncRunAuth["source"];
    readonly valueKey: string;
    readonly ownerKey?: string;
  }> = [
    {
      source: "qa-env",
      valueKey: "QA_KHALA_SYNC_TOKEN",
      ownerKey: "QA_KHALA_SYNC_OWNER_USER_ID",
    },
    { source: "agent-env", valueKey: "OPENAGENTS_AGENT_TOKEN" },
    {
      source: "mobile-test-env",
      valueKey: "KHALA_MOBILE_TEST_TOKEN",
      ownerKey: "KHALA_MOBILE_TEST_OWNER_USER_ID",
    },
  ];
  for (const candidate of candidates) {
    const raw = env[candidate.valueKey]?.trim();
    if (raw === undefined || raw === "") continue;
    const owner =
      candidate.ownerKey !== undefined ? env[candidate.ownerKey]?.trim() : undefined;
    return {
      kind: "resolved",
      auth: {
        source: candidate.source,
        value: () => raw,
        ...(owner !== undefined && owner !== "" ? { ownerUserId: owner } : {}),
      },
    };
  }
  return {
    kind: "unavailable",
    reason:
      "no bearer in env (QA_KHALA_SYNC_TOKEN / OPENAGENTS_AGENT_TOKEN / " +
      "KHALA_MOBILE_TEST_TOKEN)",
  };
}

/**
 * Self-register a THROWAWAY agent against `baseUrl` (the same
 * `POST /api/agents/register` flow the predeploy parallel-dispatch smoke
 * uses) and return its bearer + owning user id. The bearer value is held in
 * the closure only — never logged, never written. Failure is an honest
 * `unavailable`, not a throw: the caller skips cleanly.
 */
export async function selfRegisterThrowawayAgent(input: {
  readonly baseUrl: string;
  readonly runRef: string;
  readonly fetchImpl?: typeof globalThis.fetch;
}): Promise<KhalaSyncAuthResolution> {
  const fetchImpl = input.fetchImpl ?? globalThis.fetch.bind(globalThis);
  const base = input.baseUrl.replace(/\/+$/, "");
  const slugPart = input.runRef
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
  try {
    const response = await fetchImpl(`${base}/api/agents/register`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        displayName: "QA khala-sync-transport probe",
        externalId: `qa.khala-sync-transport.${input.runRef}`,
        metadata: { authority: "qa_runner_khala_sync_transport_backend" },
        slug: `qa-sync-probe-${slugPart}`.slice(0, 80),
      }),
    });
    if (response.status !== 201) {
      return {
        kind: "unavailable",
        reason: `agent self-registration answered HTTP ${response.status}`,
      };
    }
    const body = (await response.json().catch(() => undefined)) as
      | {
          readonly credential?: { readonly token?: string };
          readonly user?: { readonly id?: string };
        }
      | undefined;
    const value = body?.credential?.token;
    const ownerUserId = body?.user?.id;
    if (typeof value !== "string" || value.trim() === "") {
      return {
        kind: "unavailable",
        reason: "agent self-registration returned no usable bearer",
      };
    }
    return {
      kind: "resolved",
      auth: {
        source: "self-registered",
        value: () => value,
        ...(typeof ownerUserId === "string" && ownerUserId !== ""
          ? { ownerUserId }
          : {}),
      },
    };
  } catch (error) {
    return {
      kind: "unavailable",
      reason: `agent self-registration failed before a response: ${
        error instanceof Error ? error.message : String(error)
      }`,
    };
  }
}

// ---------------------------------------------------------------------------
// Classification (pure — unit-testable on its own).
// ---------------------------------------------------------------------------

export type KhalaSyncConnectClassification =
  | "live"
  | "connect_unauthenticated"
  | "connect_denied"
  | "silent_retry_loop"
  | "never_live";

/** One connect attempt's observed outcome (public-safe fields only). */
export type ConnectAttemptOutcome =
  | { readonly kind: "opened"; readonly latencyMs: number }
  | {
      readonly kind: "failed";
      readonly latencyMs: number;
      readonly retryable: boolean;
      readonly reason: string;
      readonly httpStatus?: number;
      readonly syncErrorCode?: string;
      readonly accessDenied: boolean;
    }
  | { readonly kind: "timeout"; readonly boundMs: number };

/** The raw pre-upgrade auth probe result: a plain (non-upgrade) GET of
 *  /api/sync/connect. The route authenticates BEFORE checking the Upgrade
 *  header, so 426 means "auth passed" and 401/403 isolate the auth seam. */
export interface ConnectUpgradeProbe {
  readonly status: number;
  readonly syncErrorCode?: string;
}

/**
 * Classify the connect outcome from the observed attempts + the raw probe.
 * Pure. Priority: an observed open is `live`; a probe 401 is the incident
 * class (`connect_unauthenticated`); 403 / typed scope denial is
 * `connect_denied`; all-attempts-retryable without an auth verdict is the
 * `silent_retry_loop` signature (the session would back off and retry);
 * a bound-exceeded attempt with no failure is `never_live`.
 */
export function classifyConnectOutcome(input: {
  readonly attempts: ReadonlyArray<ConnectAttemptOutcome>;
  readonly probe?: ConnectUpgradeProbe;
}): KhalaSyncConnectClassification {
  if (input.attempts.some((a) => a.kind === "opened")) return "live";
  if (input.probe?.status === 401) return "connect_unauthenticated";
  if (
    input.probe?.status === 403 ||
    input.attempts.some((a) => a.kind === "failed" && a.accessDenied)
  ) {
    return "connect_denied";
  }
  const failures = input.attempts.filter((a) => a.kind === "failed");
  if (failures.length > 0 && failures.every((a) => a.retryable)) {
    return "silent_retry_loop";
  }
  if (input.attempts.some((a) => a.kind === "timeout")) return "never_live";
  // Non-retryable failure(s) without an auth verdict: still a loop-breaker for
  // the session, but the connect never reached live — report honestly.
  return "never_live";
}

/** True when the classification is a finding (anything but a healthy live). */
export const isConnectFinding = (
  classification: KhalaSyncConnectClassification,
): boolean => classification !== "live";

// ---------------------------------------------------------------------------
// Scope redaction — result.json is public-safe; non-public scope ids carry
// user/thread identifiers, so only the scope KIND is recorded verbatim.
// ---------------------------------------------------------------------------

export function redactScopeForArtifact(scope: string): string {
  if (scope.startsWith("scope.public.")) return scope;
  const kind = scope.split(".")[1] ?? "unknown";
  return `scope.${kind}.[redacted]`;
}

// ---------------------------------------------------------------------------
// The scenario runner.
// ---------------------------------------------------------------------------

export interface RunKhalaSyncTransportScenarioInput {
  readonly target: Target;
  /** The sync scope to drive, e.g. `scope.user.<id>` or `scope.public.tokens-served`. */
  readonly scope: string;
  /** Resolved bearer; omit for an anonymous public-scope run. */
  readonly auth?: KhalaSyncRunAuth;
  /** Directory artifacts (khala-sync-report.json, result.json) go in. */
  readonly artifactDir: string;
  /** Bound for EACH connect attempt to open (ms). Default 10_000. */
  readonly connectBoundMs?: number;
  /** Connect attempts before classifying a retry loop. Default 3. */
  readonly connectAttempts?: number;
  /** Log-page fetch size. Default 10. */
  readonly logPageLimit?: number;
  /** Injectable transport (tests inject a fake; default: the REAL
   *  `createHttpKhalaSyncTransport` against `target.baseUrl`). */
  readonly transport?: KhalaSyncTransport;
  /** Injectable fetch for the raw pre-upgrade probe. Defaults to the global
   *  fetch when the REAL transport is in play; when a fake `transport` is
   *  injected the probe stays off unless one is supplied (fakes-in-CI never
   *  touch the network). Pass `null` to disable explicitly. */
  readonly probeFetch?: typeof globalThis.fetch | null;
  /** Injectable clock for deterministic timestamps/latencies. */
  readonly now?: () => number;
}

export interface RunKhalaSyncTransportScenarioOutcome {
  readonly result: QaRunResult;
  readonly resultPath: string;
  readonly reportPath: string;
  readonly classification: KhalaSyncConnectClassification;
}

/** The commitments this scenario declares (verify stage, #6192). A connect
 *  that 401s or never reaches live REFUTES `connect-reaches-live`. */
export function khalaSyncTransportCommitments(): ReadonlyArray<Commitment> {
  return [
    {
      id: "bootstrap-ok",
      claim: "POST /api/sync/bootstrap returns a decodable snapshot page for the scope",
      evidence: "step-pass",
      match: "bootstrap",
    },
    {
      id: "log-page-ok",
      claim: "GET /api/sync/log returns a decodable catch-up page from the snapshot cursor",
      evidence: "step-pass",
      match: "log page",
    },
    {
      id: "connect-reaches-live",
      claim:
        "WS /api/sync/connect opens (reaches live) within the bound for this bearer — " +
        "no 401/403, no silent retry loop",
      evidence: "step-pass",
      match: "connect live",
    },
  ];
}

const REPORT_FILE = "khala-sync-report.json";

const attemptSummary = (
  a: ConnectAttemptOutcome,
): Record<string, string | number | boolean> =>
  a.kind === "opened"
    ? { kind: a.kind, latencyMs: a.latencyMs }
    : a.kind === "failed"
      ? {
          kind: a.kind,
          latencyMs: a.latencyMs,
          retryable: a.retryable,
          reason: a.reason,
          accessDenied: a.accessDenied,
          ...(a.httpStatus !== undefined ? { httpStatus: a.httpStatus } : {}),
          ...(a.syncErrorCode !== undefined ? { syncErrorCode: a.syncErrorCode } : {}),
        }
      : { kind: a.kind, boundMs: a.boundMs };

/**
 * Drive the real (or injected) Khala Sync transport end-to-end against the
 * target and emit artifacts. Honest: a 401'd or never-live connect is a FAIL
 * with a REFUTED `connect-reaches-live` finding; there is no silent skip and
 * no fabricated pass. The raw bearer never reaches any artifact.
 */
export async function runKhalaSyncTransportScenario(
  input: RunKhalaSyncTransportScenarioInput,
): Promise<RunKhalaSyncTransportScenarioOutcome> {
  const now = input.now ?? (() => Date.now());
  const connectBoundMs = input.connectBoundMs ?? 10_000;
  const connectAttempts = input.connectAttempts ?? 3;
  const logPageLimit = input.logPageLimit ?? 10;
  mkdirSync(input.artifactDir, { recursive: true });

  const scope = decodeScope(input.scope);
  const scopeRef = redactScopeForArtifact(input.scope);
  const authSource = input.auth?.source ?? "anonymous";
  const authValue = input.auth?.value ?? (() => "");

  const transport =
    input.transport ??
    createHttpKhalaSyncTransport({
      baseUrl: input.target.baseUrl,
      authToken: authValue,
    });

  const startedAtMs = now();
  const steps: QaRunStep[] = [];
  let failure: string | undefined;
  const record = (
    kind: string,
    status: "ok" | "failed",
    label: string,
    detail?: Record<string, string | number | boolean>,
  ) =>
    steps.push({
      index: steps.length,
      kind,
      label,
      status,
      ...(detail ? { detail } : {}),
    });

  // Public-safe error summary: typed reason + status/code only — never a raw
  // message (transport messages are safe today, but the artifact contract
  // must not depend on that staying true).
  const errorSummary = (
    error: unknown,
  ): { reason: string; httpStatus?: number; syncErrorCode?: string } =>
    error instanceof KhalaSyncTransportError
      ? {
          reason: error.reason,
          ...(error.details?.status !== undefined
            ? { httpStatus: error.details.status }
            : {}),
          ...(error.details?.syncError?.code !== undefined
            ? { syncErrorCode: error.details.syncError.code }
            : {}),
        }
      : { reason: "unknown_error" };

  record("resolve-auth", "ok", `resolve auth (${authSource})`, {
    authSource,
    scopeRef,
  });

  // ── Phase 1: bootstrap ─────────────────────────────────────────────────────
  let cursor: SyncVersionWatermark = SyncVersionWatermark.make(0);
  let bootstrapOk = false;
  {
    const t0 = now();
    try {
      const response = await Effect.runPromise(
        transport.bootstrap(
          new BootstrapRequest({
            protocolVersion: 1,
            schemaVersion: SyncSchemaVersion.make(1),
            scope,
            clientGroupId: ClientGroupId.make(`qa-khala-sync-${startedAtMs}`),
          }),
        ),
      );
      const latencyMs = now() - t0;
      if (response.cursor !== undefined) cursor = response.cursor;
      bootstrapOk = true;
      record("bootstrap", "ok", "bootstrap snapshot page", {
        latencyMs,
        entities: response.entities.length,
        cursor: Number(cursor),
      });
    } catch (error) {
      const summary = errorSummary(error);
      record("bootstrap", "failed", "bootstrap snapshot page", {
        latencyMs: now() - t0,
        ...summary,
      });
      failure = `bootstrap failed: ${summary.reason}${
        summary.httpStatus !== undefined ? ` (HTTP ${summary.httpStatus})` : ""
      }`;
    }
  }

  // ── Phase 2: log page (only meaningful after a bootstrap cursor) ──────────
  if (bootstrapOk) {
    const t0 = now();
    try {
      const page = await Effect.runPromise(
        transport.logPage(scope, cursor, logPageLimit),
      );
      record("log-page", "ok", "log page catch-up", {
        latencyMs: now() - t0,
        entries: page.entries.length,
      });
    } catch (error) {
      const summary = errorSummary(error);
      record("log-page", "failed", "log page catch-up", {
        latencyMs: now() - t0,
        ...summary,
      });
      failure = failure ?? `log page failed: ${summary.reason}`;
    }
  }

  // ── Phase 3: connect live (the incident seam) ──────────────────────────────
  // Attempted even when bootstrap/log failed: the WS auth path is independent
  // (the incident had HTTP green + WS red) and its classification is the
  // headline signal of this backend.
  const attempts: ConnectAttemptOutcome[] = [];
  for (let attempt = 0; attempt < connectAttempts; attempt++) {
    const t0 = now();
    const connect = Effect.runPromise(
      transport.connectLive(scope, cursor, {
        onFrame: () => undefined,
        onClose: () => undefined,
      }),
    );
    // Bound each attempt; a socket that opens AFTER the bound is closed so the
    // run leaks nothing.
    let timer: ReturnType<typeof setTimeout> | undefined;
    const bound = new Promise<"timeout">((resolve) => {
      timer = setTimeout(() => resolve("timeout"), connectBoundMs);
    });
    try {
      const raced = await Promise.race([
        connect.then((socket) => ({ socket })),
        bound,
      ]);
      if (raced === "timeout") {
        attempts.push({ kind: "timeout", boundMs: connectBoundMs });
        connect.then((s) => s.close()).catch(() => undefined);
        break; // a bound-exceeded attempt will not heal by retrying here
      }
      attempts.push({ kind: "opened", latencyMs: now() - t0 });
      raced.socket.close();
      break;
    } catch (error) {
      const summary = errorSummary(error);
      attempts.push({
        kind: "failed",
        latencyMs: now() - t0,
        retryable:
          error instanceof KhalaSyncTransportError ? error.retryable : false,
        accessDenied: isAccessDeniedSignal(error),
        ...summary,
      });
      if (!(error instanceof KhalaSyncTransportError && error.retryable)) break;
    } finally {
      if (timer !== undefined) clearTimeout(timer);
    }
  }

  // ── Phase 4: raw pre-upgrade auth probe (only when connect never opened) ──
  // The connect route authenticates BEFORE the Upgrade-header check, so a
  // plain GET isolates the auth verdict the WS client cannot see:
  //   426 -> auth passed (upgrade required)   401/403 -> the incident class.
  let probe: ConnectUpgradeProbe | undefined;
  const opened = attempts.some((a) => a.kind === "opened");
  const probeFetch =
    input.probeFetch === null
      ? undefined
      : (input.probeFetch ??
        (input.transport === undefined
          ? globalThis.fetch.bind(globalThis)
          : undefined));
  if (!opened && probeFetch !== undefined) {
    try {
      const query = new URLSearchParams({ scope: input.scope, cursor: "0" });
      const bearer = authValue();
      if (bearer !== "") query.set("token", bearer);
      const response = await probeFetch(
        `${input.target.baseUrl.replace(/\/+$/, "")}${KHALA_SYNC_CONNECT_PATH}?${query.toString()}`,
        { method: "GET" },
      );
      const body = (await response.json().catch(() => undefined)) as
        | { readonly code?: string }
        | undefined;
      probe = {
        status: response.status,
        ...(typeof body?.code === "string" ? { syncErrorCode: body.code } : {}),
      };
      record("probe-connect-auth", "ok", "probe connect upgrade auth", {
        httpStatus: probe.status,
        ...(probe.syncErrorCode !== undefined
          ? { syncErrorCode: probe.syncErrorCode }
          : {}),
      });
    } catch {
      // Probe unreachable: classification proceeds from attempts alone.
    }
  }

  const classification = classifyConnectOutcome({
    attempts,
    ...(probe !== undefined ? { probe } : {}),
  });
  const openedAttempt = attempts.find((a) => a.kind === "opened");
  if (classification === "live" && openedAttempt !== undefined) {
    record("connect-live", "ok", "connect live (websocket)", {
      latencyMs: openedAttempt.latencyMs,
      attempts: attempts.length,
      classification,
    });
  } else {
    record("connect-live", "failed", "connect live (websocket)", {
      attempts: attempts.length,
      classification,
      ...(probe !== undefined ? { probeHttpStatus: probe.status } : {}),
    });
    failure =
      failure ??
      `connect never reached live: classification=${classification}` +
        (probe !== undefined ? ` (upgrade probe HTTP ${probe.status})` : "");
  }

  const endedAtMs = now();

  // ── Artifact 1: the typed classification report ────────────────────────────
  const reportPath = join(input.artifactDir, REPORT_FILE);
  const report = {
    schemaVersion: "openagents.qa_runner.khala_sync_transport_report.v1",
    targetName: input.target.name,
    targetBaseUrl: input.target.baseUrl,
    scopeRef,
    authSource,
    classification,
    finding: isConnectFinding(classification),
    connectAttempts: attempts.map(attemptSummary),
    ...(probe !== undefined ? { upgradeProbe: probe } : {}),
  };
  assertPublicSafeResult(report);
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);

  // ── Artifact 2: result.json (shared public-safe schema + verify verdict) ──
  const status: "pass" | "fail" = failure === undefined ? "pass" : "fail";
  const verify = verifyCommitments({
    commitments: khalaSyncTransportCommitments(),
    steps,
    runStatus: status,
  });
  const result: QaRunResult = {
    schemaVersion: "openagents.qa_runner.result.v1",
    status,
    target: { name: input.target.name, baseUrl: input.target.baseUrl },
    brain: "khala-sync-transport-scenario",
    backend: "khala-sync-transport",
    startedAt: new Date(startedAtMs).toISOString(),
    endedAt: new Date(endedAtMs).toISOString(),
    durationMs: endedAtMs - startedAtMs,
    steps,
    artifacts: { screenshots: [REPORT_FILE] },
    ...(failure !== undefined ? { failure } : {}),
    verify,
  };
  assertPublicSafeResult(result);
  const resultPath = join(input.artifactDir, "result.json");
  writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);

  return { result, resultPath, reportPath, classification };
}
