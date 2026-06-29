// HTTP control-API tests (#6196): the full flow over the fetch handler, plus
// auth rejection. No port binding — Request in, Response out — deterministic.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { makeFetchHandler } from "./api-server";
import { QaControl } from "./control";
import { makeTokenVerifier } from "./control-auth";

let dir: string;
const TOKEN = "tok_test_secret";

const mkHandler = () => {
  const control = new QaControl({ storeDir: dir, proBaseUrl: "https://openagents.com" });
  const verifier = makeTokenVerifier([{ agent: "tester", token: TOKEN }]);
  return makeFetchHandler({ control, verifier });
};

const req = (method: string, path: string, opts: { token?: string; body?: unknown } = {}) =>
  new Request(`http://localhost${path}`, {
    method,
    headers: {
      ...(opts.token ? { authorization: `Bearer ${opts.token}` } : {}),
      ...(opts.body !== undefined ? { "content-type": "application/json" } : {}),
    },
    ...(opts.body !== undefined ? { body: JSON.stringify(opts.body) } : {}),
  });

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-api-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe("auth", () => {
  test("healthz needs no auth", async () => {
    const res = await mkHandler()(req("GET", "/healthz"));
    expect(res.status).toBe(200);
  });

  test("missing token => 401 with an OpenAI-style error envelope", async () => {
    const res = await mkHandler()(req("POST", "/runs", { body: {} }));
    expect(res.status).toBe(401);
    const body = (await res.json()) as { error: { message: string; type: string; code: string } };
    expect(body.error.type).toBe("invalid_request_error");
    expect(body.error.code).toBe("invalid_api_key");
  });

  test("invalid token => 401 (and does not echo the token)", async () => {
    const res = await mkHandler()(req("POST", "/runs", { token: "tok_wrong", body: {} }));
    expect(res.status).toBe(401);
    const body = (await res.json()) as { error: { message: string } };
    expect(body.error.message).not.toContain("tok_wrong");
  });
});

describe("full run flow over HTTP", () => {
  test("POST /runs -> GET /runs/:id -> GET /runs/:id/artifacts", async () => {
    const handler = mkHandler();

    const submit = await handler(
      req("POST", "/runs", { token: TOKEN, body: { scenario: "login-regression" } }),
    );
    expect(submit.status).toBe(202);
    const job = (await submit.json()) as { id: string; object: string; mode: string };
    expect(job.object).toBe("qa_control.run");
    expect(job.mode).toBe("mock");

    // poll status to completion
    let status = "queued";
    for (let i = 0; i < 50 && status !== "succeeded" && status !== "failed"; i++) {
      const s = await handler(req("GET", `/runs/${job.id}`, { token: TOKEN }));
      status = ((await s.json()) as { status: string }).status;
      if (status === "succeeded" || status === "failed") break;
      await new Promise((r) => setTimeout(r, 20));
    }
    expect(status).toBe("succeeded");

    const art = await handler(req("GET", `/runs/${job.id}/artifacts`, { token: TOKEN }));
    expect(art.status).toBe(200);
    const body = (await art.json()) as {
      object: string;
      proUrl: string;
      video: string | null;
      result: Record<string, unknown> | null;
      receipt: Record<string, unknown> | null;
      verify: string | null;
    };
    expect(body.object).toBe("qa_control.run_artifacts");
    expect(body.proUrl).toBe(`https://openagents.com/pro/runs/${job.id}`);
    expect(body.video).toBeTruthy();
    expect(body.result).not.toBeNull();
    expect(body.result!["status"]).toBe("pass");
    expect(body.receipt).not.toBeNull(); // additive receipt present
    // verify is the peer-lane field; absent here is honest null, not fabricated
    expect(body.verify).toBeNull();
  });

  test("a real run is refused with 403 not_armed when unarmed", async () => {
    const handler = mkHandler();
    const res = await handler(
      req("POST", "/runs", { token: TOKEN, body: { real: true, target: "https://openagents.com" } }),
    );
    expect(res.status).toBe(403);
    const body = (await res.json()) as { error: { code: string } };
    expect(body.error.code).toBe("not_armed");
  });
});

describe("full eval flow over HTTP", () => {
  test("POST /evals (2 variants) -> GET /evals/:id comparison", async () => {
    const handler = mkHandler();
    const submit = await handler(
      req("POST", "/evals", {
        token: TOKEN,
        body: {
          title: "baseline vs candidate",
          variants: [
            { id: "baseline", scenario: "login-regression" },
            { id: "candidate", scenario: "login-regression-wrong" },
          ],
        },
      }),
    );
    expect(submit.status).toBe(202);
    const job = (await submit.json()) as { id: string };

    let status = "queued";
    let body: { status: string; comparison: { variants: unknown[]; decisionGrade: boolean } | null } = {
      status,
      comparison: null,
    };
    for (let i = 0; i < 50; i++) {
      const s = await handler(req("GET", `/evals/${job.id}`, { token: TOKEN }));
      body = (await s.json()) as typeof body;
      status = body.status;
      if (status === "succeeded" || status === "failed") break;
      await new Promise((r) => setTimeout(r, 20));
    }
    expect(status).toBe("succeeded");
    expect(body.comparison).not.toBeNull();
    expect(body.comparison!.variants.length).toBe(2);
    expect(body.comparison!.decisionGrade).toBe(false);
  });

  test("eval with < 2 variants => 400", async () => {
    const handler = mkHandler();
    const res = await handler(
      req("POST", "/evals", { token: TOKEN, body: { variants: [{ id: "only" }] } }),
    );
    expect(res.status).toBe(400);
  });
});

describe("routing", () => {
  test("unknown route => 404", async () => {
    const res = await mkHandler()(req("GET", "/nope", { token: TOKEN }));
    expect(res.status).toBe(404);
  });
});
