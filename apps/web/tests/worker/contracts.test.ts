import { env, createExecutionContext, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const { default: worker } = await import("../../src/effuse-host/worker");

describe("apps/web worker contracts endpoints", () => {
  it("GET /api/contracts/tools returns tool contracts (no-store)", async () => {
    const request = new Request("http://example.com/api/contracts/tools", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("content-type") ?? "").toContain("application/json");

    const json = (await response.json()) as any;
    expect(Array.isArray(json)).toBe(true);
    expect(json.length).toBeGreaterThan(0);
    expect(typeof json[0]?.name).toBe("string");
    expect(typeof json[0]?.description).toBe("string");
    expect(json[0]?.inputSchemaJson).toBeTruthy();
  });

  it("GET /api/contracts/signatures returns signature contracts (no-store)", async () => {
    const request = new Request("http://example.com/api/contracts/signatures", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("content-type") ?? "").toContain("application/json");

    const json = (await response.json()) as any;
    expect(Array.isArray(json)).toBe(true);
    expect(json.length).toBeGreaterThan(0);
    expect(typeof json[0]?.signatureId).toBe("string");
  });

  it("GET /api/contracts/modules returns module contracts (no-store)", async () => {
    const request = new Request("http://example.com/api/contracts/modules", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("content-type") ?? "").toContain("application/json");

    const json = (await response.json()) as any;
    expect(Array.isArray(json)).toBe(true);
    expect(json.length).toBeGreaterThan(0);
    expect(typeof json[0]?.moduleId).toBe("string");
  });

  it("rejects non-GET methods", async () => {
    const request = new Request("http://example.com/api/contracts/tools", { method: "POST" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(405);
  });
});

