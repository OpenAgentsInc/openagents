import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, test } from "vite-plus/test";

import {
  OBSERVER_CHECK_REGISTRY,
  decodeObserverRegistry,
  parseCadenceMs,
  type ObserverCheck,
} from "./qa-observer-registry.js";
import {
  QA_OBSERVER_RESULTS_SCHEMA,
  artifactFileName,
  buildArtifact,
  buildDriftIssueBody,
  buildDriftIssueCommentCommand,
  buildDriftIssueCreateCommand,
  computeConsecutiveDriftRuns,
  evaluateExpectation,
  evaluateRule,
  readPriorRuns,
  renderCommand,
  runProbe,
  selectDueChecks,
  type ObserverCheckResult,
  type ProbeContext,
} from "./qa-observer.js";

const NOW_MS = Date.parse("2026-07-16T12:00:00.000Z");

const fixtureCheck = (overrides: Partial<ObserverCheck> = {}): ObserverCheck => ({
  cadence: "15m",
  expectation: {
    description: "tokensServed positive",
    rules: [{ kind: "number_gt", path: "tokensServed", value: 0 }],
  },
  id: "fixture.check",
  probe: { kind: "http", method: "GET", path: "/api/public/fixture" },
  severityOnDrift: "high",
  surface: "https://openagents.com/api/public/fixture",
  ...overrides,
});

const fixtureContext = (overrides: Partial<ProbeContext> = {}): ProbeContext => ({
  baseUrl: "https://openagents.example",
  env: {},
  execImpl: () => ({ status: 0, stderr: "", stdout: "{}" }),
  fetchImpl: () => {
    throw new Error("fetch not stubbed");
  },
  ...overrides,
});

const jsonResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" },
    status,
  });

describe("registry decode", () => {
  test("the seed registry decodes cleanly", () => {
    const decoded = decodeObserverRegistry(OBSERVER_CHECK_REGISTRY);
    expect("problems" in decoded ? decoded.problems : []).toEqual([]);
    if ("checks" in decoded) {
      expect(decoded.checks.length).toBeGreaterThanOrEqual(7);
    }
  });

  test("the seed registry covers the required surfaces", () => {
    const ids = OBSERVER_CHECK_REGISTRY.map((check) => check.id);
    expect(ids).toContain("public.khala_tokens_served");
    expect(ids).toContain("public.khala_tokens_served_history");
    expect(ids).toContain("public.khala_model_mix");
    expect(ids).toContain("public.khala_channel_mix");
    expect(ids).toContain("public.pylon_stats");
    expect(ids).toContain("forum.launch_status");
    expect(ids).toContain("khala_sync.capture_health");
  });

  test("the khala-sync liveness check honestly documents its admin gate", () => {
    const check = OBSERVER_CHECK_REGISTRY.find(
      (candidate) => candidate.id === "khala_sync.capture_health",
    );
    expect(check?.notes).toContain("Unrunnable without OPENAGENTS_ADMIN_API_TOKEN");
    expect(check?.probe).toMatchObject({ bearerEnv: "OPENAGENTS_ADMIN_API_TOKEN", kind: "http" });
  });

  test("rejects duplicate ids, bad severities, bad cadences, and bad rules", () => {
    const base = fixtureCheck();
    const decoded = decodeObserverRegistry([
      base,
      { ...base, severityOnDrift: "urgent" },
      { ...fixtureCheck({ id: "fixture.other" }), cadence: "sometimes" },
      {
        ...fixtureCheck({ id: "fixture.rules" }),
        expectation: { description: "bad", rules: [{ kind: "regex_matches", path: "x" }] },
      },
    ]);
    expect("problems" in decoded).toBe(true);
    if ("problems" in decoded) {
      expect(decoded.problems.join("\n")).toContain("duplicate id");
      expect(decoded.problems.join("\n")).toContain("severityOnDrift");
      expect(decoded.problems.join("\n")).toContain("cadence");
      expect(decoded.problems.join("\n")).toContain("unknown rule kind");
    }
  });

  test("rejects a non-array registry", () => {
    expect(decodeObserverRegistry({ checks: [] })).toEqual({
      problems: ["registry must be an array of checks"],
    });
  });

  test("parseCadenceMs parses supported units and rejects junk", () => {
    expect(parseCadenceMs("30s")).toBe(30_000);
    expect(parseCadenceMs("15m")).toBe(900_000);
    expect(parseCadenceMs("1h")).toBe(3_600_000);
    expect(parseCadenceMs("2d")).toBe(172_800_000);
    expect(parseCadenceMs("0m")).toBeUndefined();
    expect(parseCadenceMs("15")).toBeUndefined();
    expect(parseCadenceMs("soon")).toBeUndefined();
  });
});

describe("expectation evaluation", () => {
  test("number_gt passes and drifts", () => {
    expect(
      evaluateRule(
        { kind: "number_gt", path: "tokensServed", value: 0 },
        { tokensServed: 5 },
        NOW_MS,
      ),
    ).toBeUndefined();
    expect(
      evaluateRule(
        { kind: "number_gt", path: "tokensServed", value: 0 },
        { tokensServed: 0 },
        NOW_MS,
      ),
    ).toContain("not a number > 0");
    expect(evaluateRule({ kind: "number_gt", path: "missing", value: 0 }, {}, NOW_MS)).toContain(
      "undefined",
    );
  });

  test("timestamp_within_ms handles ISO strings, epoch numbers, and staleness", () => {
    const fresh = new Date(NOW_MS - 60_000).toISOString();
    const stale = new Date(NOW_MS - 3_600_000).toISOString();
    const rule = { kind: "timestamp_within_ms", maxAgeMs: 900_000, path: "generatedAt" } as const;
    expect(evaluateRule(rule, { generatedAt: fresh }, NOW_MS)).toBeUndefined();
    expect(evaluateRule(rule, { generatedAt: NOW_MS - 1000 }, NOW_MS)).toBeUndefined();
    expect(evaluateRule(rule, { generatedAt: stale }, NOW_MS)).toContain("old");
    expect(evaluateRule(rule, { generatedAt: "not-a-date" }, NOW_MS)).toContain(
      "not a parseable timestamp",
    );
  });

  test("array_non_empty, string_equals, field_type, every_item_has_keys", () => {
    expect(
      evaluateRule({ kind: "array_non_empty", path: "series" }, { series: [1] }, NOW_MS),
    ).toBeUndefined();
    expect(
      evaluateRule({ kind: "array_non_empty", path: "series" }, { series: [] }, NOW_MS),
    ).toContain("non-empty array");
    expect(
      evaluateRule(
        { kind: "string_equals", path: "status", value: "healthy" },
        { status: "healthy" },
        NOW_MS,
      ),
    ).toBeUndefined();
    expect(
      evaluateRule(
        { kind: "string_equals", path: "status", value: "healthy" },
        { status: "stale" },
        NOW_MS,
      ),
    ).toContain('"stale"');
    expect(
      evaluateRule(
        { kind: "field_type", path: "available", type: "boolean" },
        { available: true },
        NOW_MS,
      ),
    ).toBeUndefined();
    expect(
      evaluateRule(
        { kind: "field_type", path: "available", type: "boolean" },
        { available: "yes" },
        NOW_MS,
      ),
    ).toContain("not a boolean");
    const items = { gates: [{ id: "a", state: "ready" }] };
    expect(
      evaluateRule(
        { keys: ["id", "state"], kind: "every_item_has_keys", path: "gates" },
        items,
        NOW_MS,
      ),
    ).toBeUndefined();
    expect(
      evaluateRule(
        { keys: ["id", "missing"], kind: "every_item_has_keys", path: "gates" },
        items,
        NOW_MS,
      ),
    ).toContain('missing key "missing"');
  });

  test("evaluateExpectation reports every failed rule", () => {
    const failures = evaluateExpectation(
      [
        { kind: "number_gt", path: "tokensServed", value: 0 },
        { kind: "array_non_empty", path: "series" },
      ],
      { series: [], tokensServed: 0 },
      NOW_MS,
    );
    expect(failures).toHaveLength(2);
  });

  test("nested dot paths resolve", () => {
    expect(
      evaluateRule(
        { kind: "number_gt", path: "staleness.maxStalenessSeconds", value: 0 },
        { staleness: { maxStalenessSeconds: 2 } },
        NOW_MS,
      ),
    ).toBeUndefined();
  });
});

describe("probe execution honesty", () => {
  test("http probe passes through a 200 JSON body", async () => {
    const outcome = await runProbe(
      fixtureCheck(),
      fixtureContext({ fetchImpl: async () => jsonResponse({ tokensServed: 42 }) }),
    );
    expect(outcome).toMatchObject({ body: { tokensServed: 42 }, outcome: "ran" });
  });

  test("http probe with a missing bearer env is unrunnable with the reason", async () => {
    const outcome = await runProbe(
      fixtureCheck({
        probe: {
          bearerEnv: "OPENAGENTS_ADMIN_API_TOKEN",
          kind: "http",
          method: "GET",
          path: "/api/internal/x",
        },
      }),
      fixtureContext(),
    );
    expect(outcome).toEqual({
      outcome: "unrunnable",
      reason:
        "requires bearer env OPENAGENTS_ADMIN_API_TOKEN which is not present in the environment",
    });
  });

  test("http probe sends the bearer when present and never echoes it", async () => {
    let seenAuth: string | null = null;
    const outcome = await runProbe(
      fixtureCheck({
        probe: {
          bearerEnv: "OPENAGENTS_ADMIN_API_TOKEN",
          kind: "http",
          method: "GET",
          path: "/api/internal/x",
        },
      }),
      fixtureContext({
        env: { OPENAGENTS_ADMIN_API_TOKEN: "super-secret" },
        fetchImpl: async (_url, init) => {
          seenAuth = new Headers(init?.headers).get("authorization");
          return jsonResponse({ status: "healthy" });
        },
      }),
    );
    expect(seenAuth).toBe("Bearer super-secret");
    expect(JSON.stringify(outcome)).not.toContain("super-secret");
  });

  test("http non-2xx and network failures surface as probe failures (drift), not passes", async () => {
    const failed = await runProbe(
      fixtureCheck(),
      fixtureContext({ fetchImpl: async () => jsonResponse({ error: "down" }, 503) }),
    );
    expect(failed).toMatchObject({
      outcome: "probe_failed",
      reason: "HTTP 503 from /api/public/fixture",
    });

    const network = await runProbe(
      fixtureCheck(),
      fixtureContext({
        fetchImpl: async () => {
          throw new Error("socket hang up");
        },
      }),
    );
    expect(network).toMatchObject({ outcome: "probe_failed" });
  });

  test("command probe: clean JSON stdout runs; nonzero exit is a probe failure; unspawnable is unrunnable", async () => {
    const check = fixtureCheck({ probe: { command: ["fake-probe"], kind: "command" } });
    expect(
      await runProbe(
        check,
        fixtureContext({ execImpl: () => ({ status: 0, stderr: "", stdout: '{"ok":true}' }) }),
      ),
    ).toMatchObject({ body: { ok: true }, outcome: "ran" });
    expect(
      await runProbe(
        check,
        fixtureContext({ execImpl: () => ({ status: 3, stderr: "boom", stdout: "" }) }),
      ),
    ).toMatchObject({ outcome: "probe_failed", reason: "command exited 3" });
    expect(
      await runProbe(
        check,
        fixtureContext({
          execImpl: () => {
            throw new Error("spawn fake-probe ENOENT");
          },
        }),
      ),
    ).toMatchObject({ outcome: "unrunnable" });
  });
});

describe("consecutive drift and issue filing", () => {
  test("a non-drift current status is always streak 0", () => {
    expect(computeConsecutiveDriftRuns("pass", "x", [{ x: "drift" }, { x: "drift" }])).toBe(0);
    expect(computeConsecutiveDriftRuns("unrunnable", "x", [{ x: "drift" }])).toBe(0);
  });

  test("streak counts current run plus consecutive prior drifts, broken by pass or absence", () => {
    expect(computeConsecutiveDriftRuns("drift", "x", [])).toBe(1);
    expect(
      computeConsecutiveDriftRuns("drift", "x", [
        { x: "drift" },
        { x: "drift" },
        { x: "pass" },
        { x: "drift" },
      ]),
    ).toBe(3);
    expect(computeConsecutiveDriftRuns("drift", "x", [{ x: "pass" }, { x: "drift" }])).toBe(1);
    expect(computeConsecutiveDriftRuns("drift", "x", [{ y: "drift" }, { x: "drift" }])).toBe(1);
    expect(computeConsecutiveDriftRuns("drift", "x", [{ x: "unrunnable" }, { x: "drift" }])).toBe(
      1,
    );
  });

  test("issue create/comment commands carry the check id, label, and bounded evidence", () => {
    const result: ObserverCheckResult = {
      consecutiveDriftRuns: 2,
      durationMs: 12,
      evidence: '{"tokensServed":0}',
      id: "public.khala_tokens_served",
      reason: "tokensServed=0 is not a number > 0",
      severityOnDrift: "high",
      status: "drift",
      surface: "https://openagents.com/api/public/khala-tokens-served",
    };
    const create = buildDriftIssueCreateCommand(result, "2026-07-16T12:00:00.000Z");
    expect(create.slice(0, 3)).toEqual(["gh", "issue", "create"]);
    expect(create).toContain("QA Observer drift: public.khala_tokens_served");
    expect(create).toContain("qa-observer");
    const body = buildDriftIssueBody(result, "2026-07-16T12:00:00.000Z");
    expect(body).toContain("Consecutive drifting runs: 2");
    expect(body).toContain('{"tokensServed":0}');
    const comment = buildDriftIssueCommentCommand(4321, result, "2026-07-16T12:00:00.000Z");
    expect(comment.slice(0, 4)).toEqual(["gh", "issue", "comment", "4321"]);
    expect(renderCommand(create)).toContain("gh issue create --repo OpenAgentsInc/openagents");
  });
});

describe("artifact shape and cadence", () => {
  const passResult: ObserverCheckResult = {
    consecutiveDriftRuns: 0,
    durationMs: 5,
    id: "a",
    severityOnDrift: "medium",
    status: "pass",
    surface: "s",
  };
  const driftHigh: ObserverCheckResult = {
    consecutiveDriftRuns: 1,
    durationMs: 5,
    id: "b",
    reason: "broken",
    severityOnDrift: "high",
    status: "drift",
    surface: "s",
  };
  const unrunnable: ObserverCheckResult = {
    consecutiveDriftRuns: 0,
    durationMs: 0,
    id: "c",
    reason: "requires bearer env X",
    severityOnDrift: "critical",
    status: "unrunnable",
    surface: "s",
  };

  test("buildArtifact counts states and exits 1 only on high-severity drift", () => {
    const artifact = buildArtifact(
      [passResult, driftHigh, unrunnable],
      "2026-07-16T12:00:00.000Z",
      "https://openagents.com",
    );
    expect(artifact.schemaVersion).toBe(QA_OBSERVER_RESULTS_SCHEMA);
    expect(artifact.summary).toEqual({
      checksTotal: 3,
      drift: 1,
      exitCode: 1,
      highSeverityDrift: 1,
      pass: 1,
      unrunnable: 1,
    });
    const calm = buildArtifact(
      [passResult, { ...driftHigh, severityOnDrift: "medium" }],
      "2026-07-16T12:00:00.000Z",
      "https://openagents.com",
    );
    expect(calm.summary.exitCode).toBe(0);
  });

  test("unrunnable never counts as pass", () => {
    const artifact = buildArtifact(
      [unrunnable],
      "2026-07-16T12:00:00.000Z",
      "https://openagents.com",
    );
    expect(artifact.summary.pass).toBe(0);
    expect(artifact.summary.unrunnable).toBe(1);
  });

  test("artifactFileName is filesystem-safe and dated", () => {
    expect(artifactFileName("2026-07-16T12:00:00.000Z")).toBe(
      "qa-observer-run-2026-07-16T12-00-00Z.json",
    );
  });

  test("readPriorRuns reads artifacts newest-first and skips junk", () => {
    const dir = mkdtempSync(join(tmpdir(), "qa-observer-test-"));
    const runFile = (runAt: string, status: string) =>
      JSON.stringify({
        results: [{ id: "a", status }],
        runAt,
        schemaVersion: QA_OBSERVER_RESULTS_SCHEMA,
      });
    writeFileSync(join(dir, "one.json"), runFile("2026-07-16T10:00:00.000Z", "drift"));
    writeFileSync(join(dir, "two.json"), runFile("2026-07-16T11:00:00.000Z", "pass"));
    writeFileSync(join(dir, "junk.json"), "not json");
    writeFileSync(join(dir, "other-schema.json"), JSON.stringify({ schemaVersion: "other.v1" }));
    const runs = readPriorRuns(dir);
    expect(runs.map((run) => run.runAt)).toEqual([
      "2026-07-16T11:00:00.000Z",
      "2026-07-16T10:00:00.000Z",
    ]);
    expect(runs[0]?.statuses).toEqual({ a: "pass" });
    expect(readPriorRuns(join(dir, "does-not-exist"))).toEqual([]);
  });

  test("selectDueChecks respects cadence and --all", () => {
    const check = fixtureCheck({ cadence: "1h" });
    const recent = [
      {
        runAt: new Date(NOW_MS - 10 * 60_000).toISOString(),
        statuses: { [check.id]: "pass" as const },
      },
    ];
    const old = [
      {
        runAt: new Date(NOW_MS - 2 * 3_600_000).toISOString(),
        statuses: { [check.id]: "pass" as const },
      },
    ];
    expect(selectDueChecks([check], recent, NOW_MS, false)).toEqual([]);
    expect(selectDueChecks([check], old, NOW_MS, false)).toEqual([check]);
    expect(selectDueChecks([check], [], NOW_MS, false)).toEqual([check]);
    expect(selectDueChecks([check], recent, NOW_MS, true)).toEqual([check]);
  });
});
