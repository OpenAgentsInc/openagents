// END-TO-END VERIFICATION (#6210), runnable in-repo so it uses the workspace's
// pinned Effect. Proves the qa-runner emit -> redact -> publish pipeline produces
// a body the REAL worker-side acceptance gates (schema decode + structural
// validator + public-safety tripwire from apps/openagents.com) accept, and that
// the returned shareable URL is /trace/{uuid} (never /pro/evals). FAKE local
// ingest — deterministic, NO network, NO spend.
//
// This is a verification harness kept as a test so it runs in CI alongside the
// unit tests; it is NOT excluded from the default `test` script intentionally.

import { describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { mapKhalaRunToAtif } from "./atif";
import { makeSessionTrace } from "./session-trace";
import type { QaRunResult } from "./result";
import { publishTrace, type FetchLike } from "./publish-trace";
import {
  ATIF_PINNED_SCHEMA_VERSION,
  atifTraceTripwire,
  decodeAtifTrajectorySync,
  validateAtifTrajectory,
} from "../../openagents.com/workers/api/src/atif-trace-schema";

describe("#6210 end-to-end: emit -> redact -> (real worker gates) -> /trace/{uuid}", () => {
  test("the emitted/redacted trajectory passes the REAL worker schema + validator + tripwire and returns /trace/{uuid}", async () => {
    const result: QaRunResult = {
      schemaVersion: "openagents.qa_runner.result.v1",
      status: "pass",
      target: { name: "openagents.com", baseUrl: "https://openagents.com" },
      brain: "khala",
      backend: "local",
      startedAt: "2026-06-24T00:00:00.000Z",
      endedAt: "2026-06-24T00:00:02.000Z",
      durationMs: 2000,
      steps: [
        { index: 0, kind: "navigate", label: "open /login", status: "ok" },
        { index: 1, kind: "assert", label: 'body contains "Log in to OpenAgents"', status: "ok" },
      ],
      artifacts: { video: "session.mp4", videoFormat: "mp4", screenshots: ["step-1.png"] },
    };
    const trace = makeSessionTrace({
      goal: "Verify /login renders sign-in",
      target: { name: "openagents.com", baseUrl: "https://openagents.com" },
      model: "openagents/khala",
      beats: [
        { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
        { kind: "browser", action: "assert", targetHint: "sign-in copy", status: "ok" },
      ],
      inputs: [],
      outputs: [],
    });
    const trajectory = mapKhalaRunToAtif({ result, trace, sessionId: "login-trace" });
    expect(trajectory.schema_version).toBe(ATIF_PINNED_SCHEMA_VERSION);

    let postedBody = "";
    const token = "oa_agent_VERIFYTOKEN123456";
    // A FAKE ingest that runs the REAL worker-side acceptance gates.
    const fakeIngest: FetchLike = async (_url, init) => {
      postedBody = init.body;
      expect(init.headers.authorization).toBe(`Bearer ${token}`);
      expect(init.headers["idempotency-key"]).toBeTruthy();
      const body = JSON.parse(init.body) as { trajectory: unknown };
      const decoded = decodeAtifTrajectorySync(body.trajectory); // REAL schema decode
      expect(decoded.schema_version).toBe(ATIF_PINNED_SCHEMA_VERSION);
      expect(validateAtifTrajectory(decoded)).toEqual([]); // REAL structural validator
      expect(atifTraceTripwire(decoded)).toEqual([]); // REAL public-safety tripwire
      const uuid = "11111111-2222-4333-8444-555566667777";
      return {
        ok: true,
        status: 201,
        text: async () =>
          JSON.stringify({ uuid, url: `/trace/${uuid}`, visibility: "unlisted", replay: false }),
      };
    };

    const out = await Effect.runPromise(
      publishTrace({
        trajectory,
        config: { url: "https://openagents.com/api/traces", token },
        fetch: fakeIngest,
        log: () => {},
      }),
    );

    expect(out.published).toBe(true);
    if (!out.published) throw new Error("expected published");
    expect(out.url).toBe(
      "https://openagents.com/trace/11111111-2222-4333-8444-555566667777",
    );
    expect(out.url).not.toContain("/pro/evals");
    // redaction ran before publish: the agent token never reached the wire.
    expect(postedBody).not.toContain("VERIFYTOKEN");
  });
});
