import {
  env,
  createExecutionContext,
  waitOnExecutionContext
} from "cloudflare:test";
import { describe, it, expect, vi } from "vitest";

vi.mock("@cloudflare/sandbox", () => ({
  Sandbox: class {},
  getSandbox: () => {
    throw new Error("Sandbox not available in tests");
  }
}));
vi.mock("@cloudflare/sandbox/opencode", () => ({
  createOpencodeServer: vi.fn(),
  proxyToOpencode: vi.fn()
}));

const { default: worker } = await import("../src/server");

declare module "cloudflare:test" {
  interface ProvidedEnv extends Env {}
}

describe("LiteClaw worker", () => {
  it("responds with Not found", async () => {
    const request = new Request("http://example.com");
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    expect(await response.text()).toBe("Not found");
    expect(response.status).toBe(404);
  });
});
