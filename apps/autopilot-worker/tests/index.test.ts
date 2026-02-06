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

  it("seeds a welcome message on first get-messages call", async () => {
    const threadId = `welcome-${Date.now()}`;
    const url = `http://example.com/agents/chat/${threadId}/get-messages`;

    const ctx1 = createExecutionContext();
    const res1 = await worker.fetch(new Request(url), env, ctx1);
    await waitOnExecutionContext(ctx1);
    expect(res1.status).toBe(200);
    const messages1 = (await res1.json()) as any[];
    expect(Array.isArray(messages1)).toBe(true);
    expect(messages1.length).toBe(1);
    expect(messages1[0]?.role).toBe("assistant");

    const ctx2 = createExecutionContext();
    const res2 = await worker.fetch(new Request(url), env, ctx2);
    await waitOnExecutionContext(ctx2);
    const messages2 = (await res2.json()) as any[];
    expect(messages2.length).toBe(1);
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

  it("resets the agent Blueprint state", async () => {
    const threadId = `reset-${Date.now()}`;
    const blueprintUrl = `http://example.com/agents/chat/${threadId}/blueprint`;
    const resetUrl = `http://example.com/agents/chat/${threadId}/reset-agent`;

    // Seed a non-default Blueprint via import.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(new Request(blueprintUrl), env, ctx);
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      json.docs.identity.name = "NotAutopilot";
      json.docs.identity.updatedAt = new Date().toISOString();

      const postCtx = createExecutionContext();
      const postResponse = await worker.fetch(
        new Request(blueprintUrl, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(json)
        }),
        env,
        postCtx
      );
      await waitOnExecutionContext(postCtx);
      expect(postResponse.status).toBe(200);
      // Drain body to avoid leaving the request hanging in isolated storage mode.
      await postResponse.json();
    }

    // Reset.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(resetUrl, { method: "POST" }),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.ok).toBe(true);
    }

    // Blueprint returns to defaults.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(new Request(blueprintUrl), env, ctx);
      await waitOnExecutionContext(ctx);
      const json = (await response.json()) as any;
      expect(json.docs.identity.name).toBe("Autopilot");
      expect(json.bootstrapState.status).toBe("pending");
    }
  });
});
