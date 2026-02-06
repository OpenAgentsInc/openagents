import {
  env,
  createExecutionContext,
  waitOnExecutionContext
} from "cloudflare:test";
import { describe, it, expect } from "vitest";

const { default: worker } = await import("../src/server");

declare module "cloudflare:test" {
  interface ProvidedEnv extends Env {}
}

describe("Autopilot worker", () => {
  it("responds with Not found", async () => {
    const request = new Request("http://example.com");
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    expect(await response.text()).toBe("Not found");
    expect(response.status).toBe(404);
  });

  it("exports and imports a Blueprint via the DO endpoint", async () => {
    const threadId = `test-${Date.now()}`;
    const exportUrl = `http://example.com/agents/chat/${threadId}/blueprint`;

    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(new Request(exportUrl), env, ctx);
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.format).toBe("openagents.autopilot.blueprint");
      expect(json.formatVersion).toBe(1);
      expect(json.bootstrapState.threadId).toBe(threadId);
      expect(json.bootstrapState.status).toBe("pending");
      expect(json.docs.identity.name).toBe("Autopilot");
      expect(Array.isArray(json.memory)).toBe(true);
    }

    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(new Request(exportUrl), env, ctx);
      await waitOnExecutionContext(ctx);
      const json = (await response.json()) as any;
      json.docs.identity.name = "MyAgent";
      json.docs.identity.updatedAt = new Date().toISOString();

      const postCtx = createExecutionContext();
      const postResponse = await worker.fetch(
        new Request(exportUrl, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(json)
        }),
        env,
        postCtx
      );
      await waitOnExecutionContext(postCtx);
      expect(postResponse.status).toBe(200);
      const postJson = (await postResponse.json()) as any;
      expect(postJson.ok).toBe(true);
    }

    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(new Request(exportUrl), env, ctx);
      await waitOnExecutionContext(ctx);
      const json = (await response.json()) as any;
      expect(json.docs.identity.name).toBe("MyAgent");
    }
  });
});
