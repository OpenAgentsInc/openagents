// publish-trace tests (#6210): emit ATIF -> redact -> publish to a FAKE local
// ingest (deterministic, NO network) -> return a uuid + /trace/{uuid} URL.
//
// Proves: the posted body is REDACTED before publish (no secret reaches the
// wire); the agent bearer token + Idempotency-Key are sent; the returned URL is
// https://openagents.com/trace/{uuid}; an UNARMED publish is an HONEST NO-OP (no
// fabricated uuid); a transport failure is an honest no-op (not a throw).

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { mapKhalaRunToAtif, type AtifTrajectory } from "./atif";
import type { QaRunResult } from "./result";
import {
  makeSessionTrace,
  type KhalaSessionTrace,
} from "./session-trace";
import {
  type BlobFetchLike,
  blobRefsFromTrajectory,
  buildTrajectoryFromRunDir,
  type FetchLike,
  idempotencyKeyForTrajectory,
  publishRunDir,
  publishTrace,
  resolvePublishConfig,
  runDirBlobSource,
} from "./publish-trace";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-publish-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const sampleResult = (): QaRunResult => ({
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
});

const sampleTrace = (): KhalaSessionTrace =>
  makeSessionTrace({
    goal: "Verify /login renders the sign-in form",
    target: { name: "openagents.com", baseUrl: "https://openagents.com" },
    model: "openagents/khala",
    beats: [
      { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
      { kind: "browser", action: "assert", targetHint: "sign-in copy", status: "ok" },
    ],
    inputs: [],
    outputs: [],
  });

const sampleTrajectory = (): AtifTrajectory =>
  mapKhalaRunToAtif({ result: sampleResult(), trace: sampleTrace(), sessionId: "login-trace" });

// A fake local ingest: records the request, returns a deterministic uuid. The
// uuid is derived from the Idempotency-Key so a replay is observable.
interface Captured {
  url: string;
  headers: Record<string, string>;
  body: string;
}
const fakeIngest = (opts?: { status?: number; uuid?: string; replay?: boolean }) => {
  const captured: Captured[] = [];
  const fetch: FetchLike = async (url, init) => {
    captured.push({ url, headers: init.headers, body: init.body });
    const status = opts?.status ?? 201;
    const ok = status >= 200 && status < 300;
    const uuid = opts?.uuid ?? "00000000-0000-4000-8000-000000000abc";
    const payload = ok
      ? { uuid, url: `/trace/${uuid}`, visibility: "unlisted", replay: opts?.replay ?? false }
      : { error: "trace_public_safety_rejected", findings: ["secret_material"] };
    return {
      ok,
      status,
      text: async () => JSON.stringify(payload),
    };
  };
  return { fetch, captured };
};

const armedConfig = { url: "https://openagents.com/api/traces", token: "oa_agent_TESTTOKEN123456" };

// ---------------------------------------------------------------------------
// resolvePublishConfig — env-armed
// ---------------------------------------------------------------------------

describe("resolvePublishConfig", () => {
  test("unarmed (no env) -> undefined (honest no-op signal)", () => {
    expect(resolvePublishConfig({})).toBeUndefined();
  });

  test("armed: URL + token -> config; bare base URL gets /api/traces appended", () => {
    const cfg = resolvePublishConfig({
      QA_TRACE_PUBLISH_URL: "https://openagents.com",
      QA_TRACE_PUBLISH_TOKEN: "oa_agent_abc",
    });
    expect(cfg).toEqual({ url: "https://openagents.com/api/traces", token: "oa_agent_abc" });
  });

  test("a full /api/traces URL is left untouched; visibility is honored", () => {
    const cfg = resolvePublishConfig({
      QA_TRACE_PUBLISH_URL: "https://staging.example/api/traces",
      OPENAGENTS_AGENT_PENDING_TOKEN: "oa_agent_xyz",
      QA_TRACE_PUBLISH_VISIBILITY: "public",
    });
    expect(cfg).toEqual({
      url: "https://staging.example/api/traces",
      token: "oa_agent_xyz",
      visibility: "public",
    });
  });

  test("URL without a token is NOT armed (no partial config)", () => {
    expect(resolvePublishConfig({ QA_TRACE_PUBLISH_URL: "https://x" })).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// publishTrace — armed happy path against a fake ingest
// ---------------------------------------------------------------------------

describe("publishTrace (fake ingest, no network)", () => {
  test("redacts -> publishes -> returns the /trace/{uuid} URL", async () => {
    const { fetch, captured } = fakeIngest();
    const result = await Effect.runPromise(
      publishTrace({
        trajectory: sampleTrajectory(),
        config: armedConfig,
        fetch,
        log: () => {},
      }),
    );
    expect(result.published).toBe(true);
    if (!result.published) throw new Error("expected published");
    expect(result.uuid).toBe("00000000-0000-4000-8000-000000000abc");
    expect(result.url).toBe(
      "https://openagents.com/trace/00000000-0000-4000-8000-000000000abc",
    );
    // sent exactly one POST with the agent bearer + idempotency key
    expect(captured.length).toBe(1);
    expect(captured[0]!.url).toBe("https://openagents.com/api/traces");
    expect(captured[0]!.headers.authorization).toBe(`Bearer ${armedConfig.token}`);
    expect(captured[0]!.headers["idempotency-key"]).toBeTruthy();
    // the posted body carries a valid trajectory + the blob refs
    const posted = JSON.parse(captured[0]!.body) as {
      trajectory: AtifTrajectory;
      blobRefs?: ReadonlyArray<{ kind: string; r2Key: string; contentType?: string }>;
    };
    expect(posted.trajectory.schema_version).toBe("ATIF-v1.7");
    expect(posted.blobRefs).toEqual([
      { kind: "video", r2Key: "session.mp4", contentType: "video/mp4" },
      { kind: "screenshot", r2Key: "step-1.png", contentType: "image/png" },
    ]);
  });

  test("REDACTION runs before publish: NO secret appears in the posted body", async () => {
    const { fetch, captured } = fakeIngest();
    // Inject a secret into a trajectory note/message (would never normally pass
    // the emitter, but proves the redact-before-publish step independently).
    const traj = sampleTrajectory();
    const leaky: AtifTrajectory = {
      ...traj,
      notes:
        `${traj.notes ?? ""} debug bearer oa_agent_LEAKEDSECRETVALUE123 ` +
        "and sk-or-abc123def456ghi789jkl and /Users/secretuser/work",
    };
    await Effect.runPromise(
      publishTrace({ trajectory: leaky, config: armedConfig, fetch, log: () => {} }),
    );
    const body = captured[0]!.body;
    expect(body).not.toContain("oa_agent_LEAKEDSECRETVALUE123");
    expect(body).not.toContain("sk-or-abc123def456ghi789jkl");
    expect(body).not.toContain("secretuser");
    expect(body).toContain("[REDACTED:");
  });

  test("idempotency key is stable for the same trajectory", () => {
    const a = idempotencyKeyForTrajectory(sampleTrajectory());
    const b = idempotencyKeyForTrajectory(sampleTrajectory());
    expect(a).toBe(b);
    expect(a.startsWith("qa-trace-")).toBe(true);
  });

  test("an idempotent replay (HTTP 200) is surfaced honestly", async () => {
    const { fetch } = fakeIngest({ status: 200, replay: true });
    const result = await Effect.runPromise(
      publishTrace({ trajectory: sampleTrajectory(), config: armedConfig, fetch, log: () => {} }),
    );
    expect(result.published).toBe(true);
    if (result.published) expect(result.replay).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Honest no-op paths
// ---------------------------------------------------------------------------

describe("publishTrace honest no-op", () => {
  test("UNARMED (no config, no env) -> published:false, no fabricated uuid, fetch never called", async () => {
    // Clear any env that could arm the env-fallback for this assertion.
    const keys = [
      "QA_TRACE_PUBLISH_URL",
      "QA_TRACE_PUBLISH_TOKEN",
      "QA_TRACE_PUBLISH_VISIBILITY",
      "OPENAGENTS_AGENT_TOKEN",
      "OPENAGENTS_AGENT_PENDING_TOKEN",
    ] as const;
    const saved = new Map<string, string | undefined>();
    for (const k of keys) {
      saved.set(k, process.env[k]);
      delete process.env[k];
    }
    try {
      let fetchCalled = false;
      const result = await Effect.runPromise(
        publishTrace({
          trajectory: sampleTrajectory(),
          fetch: (async () => {
            fetchCalled = true;
            throw new Error("fetch must NOT be called when unarmed");
          }) as unknown as FetchLike,
          log: () => {},
        }),
      );
      expect(result.published).toBe(false);
      if (!result.published) expect(result.kind).toBe("unconfigured");
      expect(fetchCalled).toBe(false);
    } finally {
      for (const k of keys) {
        const v = saved.get(k);
        if (v === undefined) delete process.env[k];
        else process.env[k] = v;
      }
    }
  });

  test("a transport failure is an honest no-op (not a throw)", async () => {
    const throwingFetch: FetchLike = async () => {
      throw new Error("ECONNREFUSED");
    };
    const result = await Effect.runPromise(
      publishTrace({
        trajectory: sampleTrajectory(),
        config: armedConfig,
        fetch: throwingFetch,
        log: () => {},
      }),
    );
    expect(result.published).toBe(false);
    if (!result.published) {
      expect(result.kind).toBe("error");
      expect(result.reason).toContain("ECONNREFUSED");
    }
  });

  test("a tripwire rejection (HTTP 422) is an honest no-op", async () => {
    const { fetch } = fakeIngest({ status: 422 });
    const result = await Effect.runPromise(
      publishTrace({ trajectory: sampleTrajectory(), config: armedConfig, fetch, log: () => {} }),
    );
    expect(result.published).toBe(false);
    if (!result.published) expect(result.kind).toBe("error");
  });
});

// ---------------------------------------------------------------------------
// buildTrajectoryFromRunDir — both with and without session-trace.json
// ---------------------------------------------------------------------------

describe("buildTrajectoryFromRunDir", () => {
  test("uses session-trace.json when present", () => {
    writeFileSync(join(dir, "result.json"), JSON.stringify(sampleResult()));
    writeFileSync(join(dir, "session-trace.json"), JSON.stringify(sampleTrace()));
    const traj = buildTrajectoryFromRunDir(dir, { sessionId: "run-1" });
    expect(traj.schema_version).toBe("ATIF-v1.7");
    expect(traj.session_id).toBe("run-1");
    // user goal + 2 actions + verdict
    expect(traj.steps.length).toBe(4);
  });

  test("SYNTHESIZES a trace when session-trace.json is absent (fixed-step runs)", () => {
    writeFileSync(join(dir, "result.json"), JSON.stringify(sampleResult()));
    const traj = buildTrajectoryFromRunDir(dir, { sessionId: "run-2" });
    expect(traj.schema_version).toBe("ATIF-v1.7");
    expect(traj.steps.length).toBe(4);
    // model is the public-safe own-infra id
    expect(traj.agent.model_name).toBe("openagents/khala");
  });

  test("throws when result.json is missing", () => {
    expect(() => buildTrajectoryFromRunDir(dir)).toThrow(/missing result.json/);
  });
});

describe("blobRefsFromTrajectory", () => {
  test("maps the recorded video + screenshots to ingest blob refs", () => {
    const refs = blobRefsFromTrajectory(sampleTrajectory());
    expect(refs).toEqual([
      { kind: "video", r2Key: "session.mp4", contentType: "video/mp4" },
      { kind: "screenshot", r2Key: "step-1.png", contentType: "image/png" },
    ]);
  });
});

// ---------------------------------------------------------------------------
// publishRunDir — end-to-end from a run dir against the fake ingest
// ---------------------------------------------------------------------------

describe("publishRunDir (end-to-end, fake ingest)", () => {
  test("run dir -> ATIF -> redact -> publish -> /trace/{uuid}", async () => {
    writeFileSync(join(dir, "result.json"), JSON.stringify(sampleResult()));
    writeFileSync(join(dir, "session-trace.json"), JSON.stringify(sampleTrace()));
    const { fetch, captured } = fakeIngest();
    const result = await Effect.runPromise(
      publishRunDir({ runDir: dir, sessionId: "run-3", config: armedConfig, fetch, log: () => {} }),
    );
    expect(result.published).toBe(true);
    if (result.published) {
      expect(result.url).toContain("/trace/");
      expect(result.url.startsWith("https://openagents.com/trace/")).toBe(true);
    }
    // the published body has no /Users/ path (redaction ran on the real dir build)
    expect(captured[0]!.body).not.toContain(dir);
  });
});

// ---------------------------------------------------------------------------
// Media-blob upload on publish (#6223): bytes go to /api/traces/{uuid}/blob/...
// ---------------------------------------------------------------------------

// A fake blob receiver: records each uploaded blob's URL + raw bytes.
const fakeBlobStore = (opts?: { status?: number }) => {
  const objects = new Map<string, Uint8Array>();
  const captured: { url: string; headers: Record<string, string>; bytes: Uint8Array }[] = [];
  const blobFetch: BlobFetchLike = async (url, init) => {
    captured.push({ url, headers: init.headers, bytes: init.body });
    const status = opts?.status ?? 201;
    const ok = status >= 200 && status < 300;
    if (ok) objects.set(url, init.body);
    return { ok, status, text: async () => JSON.stringify({ bytes: init.body.byteLength }) };
  };
  return { blobFetch, captured, objects };
};

describe("publish uploads media blob bytes (#6223, fake store)", () => {
  test("after publish, each blobRef's bytes are POSTed to /api/traces/{uuid}/blob/{r2Key}", async () => {
    writeFileSync(join(dir, "result.json"), JSON.stringify(sampleResult()));
    writeFileSync(join(dir, "session-trace.json"), JSON.stringify(sampleTrace()));
    // real artifact bytes on disk
    writeFileSync(join(dir, "session.mp4"), Buffer.from([0, 1, 2, 3, 4]));
    writeFileSync(join(dir, "step-1.png"), Buffer.from([9, 9, 9]));

    const { fetch } = fakeIngest({ uuid: "uuid-blob-1" });
    const { blobFetch, captured, objects } = fakeBlobStore();
    const result = await Effect.runPromise(
      publishRunDir({
        runDir: dir,
        sessionId: "run-blob",
        config: armedConfig,
        fetch,
        blobFetch,
        log: () => {},
      }),
    );
    expect(result.published).toBe(true);
    if (!result.published) throw new Error("expected published");

    // both blobRefs uploaded; reported honestly
    expect(result.blobUpload?.uploaded).toEqual(["session.mp4", "step-1.png"]);
    expect(result.blobUpload?.skipped).toEqual([]);

    // bytes really went to the per-uuid blob endpoint with the agent bearer
    expect(captured.length).toBe(2);
    expect(captured[0]!.url).toBe(
      "https://openagents.com/api/traces/uuid-blob-1/blob/session.mp4",
    );
    expect(captured[0]!.headers.authorization).toBe(`Bearer ${armedConfig.token}`);
    expect(captured[0]!.headers["content-type"]).toBe("video/mp4");
    expect([...objects.get(captured[0]!.url)!]).toEqual([0, 1, 2, 3, 4]);
    expect([...objects.get(captured[1]!.url)!]).toEqual([9, 9, 9]);
  });

  test("a blobRef with NO file on disk is SKIPPED honestly (not faked)", async () => {
    writeFileSync(join(dir, "result.json"), JSON.stringify(sampleResult()));
    writeFileSync(join(dir, "session-trace.json"), JSON.stringify(sampleTrace()));
    // only the video exists; step-1.png is absent
    writeFileSync(join(dir, "session.mp4"), Buffer.from([1, 2, 3]));

    const { fetch } = fakeIngest({ uuid: "uuid-blob-2" });
    const { blobFetch, captured } = fakeBlobStore();
    const result = await Effect.runPromise(
      publishRunDir({ runDir: dir, config: armedConfig, fetch, blobFetch, log: () => {} }),
    );
    expect(result.published).toBe(true);
    if (!result.published) throw new Error("expected published");
    expect(result.blobUpload?.uploaded).toEqual(["session.mp4"]);
    expect(result.blobUpload?.skipped.map((s) => s.r2Key)).toEqual(["step-1.png"]);
    // only the present file was POSTed
    expect(captured.length).toBe(1);
  });

  test("a per-blob HTTP failure is recorded as a skip; the publish still succeeds", async () => {
    writeFileSync(join(dir, "result.json"), JSON.stringify(sampleResult()));
    writeFileSync(join(dir, "session-trace.json"), JSON.stringify(sampleTrace()));
    writeFileSync(join(dir, "session.mp4"), Buffer.from([1, 2, 3]));
    writeFileSync(join(dir, "step-1.png"), Buffer.from([4, 5]));

    const { fetch } = fakeIngest({ uuid: "uuid-blob-3" });
    const { blobFetch } = fakeBlobStore({ status: 500 });
    const result = await Effect.runPromise(
      publishRunDir({ runDir: dir, config: armedConfig, fetch, blobFetch, log: () => {} }),
    );
    // the trace itself is published even though blob upload failed
    expect(result.published).toBe(true);
    if (!result.published) throw new Error("expected published");
    expect(result.blobUpload?.uploaded).toEqual([]);
    expect(result.blobUpload?.skipped).toHaveLength(2);
  });

  test("no blobSource (publishTrace alone) => no upload attempted, no blobUpload report", async () => {
    const { fetch } = fakeIngest();
    let blobCalled = false;
    const result = await Effect.runPromise(
      publishTrace({
        trajectory: sampleTrajectory(),
        config: armedConfig,
        fetch,
        blobFetch: (async () => {
          blobCalled = true;
          throw new Error("blobFetch must NOT be called without a blobSource");
        }) as unknown as BlobFetchLike,
        log: () => {},
      }),
    );
    expect(result.published).toBe(true);
    if (result.published) expect(result.blobUpload).toBeUndefined();
    expect(blobCalled).toBe(false);
  });
});

describe("runDirBlobSource (#6223)", () => {
  test("reads bytes + infers content type from a run dir", () => {
    writeFileSync(join(dir, "session.mp4"), Buffer.from([1, 2, 3]));
    const source = runDirBlobSource(dir);
    const resolved = source("session.mp4");
    expect(resolved).toBeDefined();
    expect([...(resolved!.bytes)]).toEqual([1, 2, 3]);
    expect(resolved!.contentType).toBe("video/mp4");
  });

  test("returns undefined (honest skip) for an absent file", () => {
    expect(runDirBlobSource(dir)("nope.mp4")).toBeUndefined();
  });

  test("refuses a path that escapes the run dir", () => {
    expect(runDirBlobSource(dir)("../escape.mp4")).toBeUndefined();
    expect(runDirBlobSource(dir)("/etc/passwd")).toBeUndefined();
  });
});
