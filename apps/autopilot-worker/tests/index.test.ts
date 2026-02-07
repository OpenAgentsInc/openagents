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

  it("exposes tool contracts for UI introspection", async () => {
    const threadId = `tools-${Date.now()}`;
    const url = `http://example.com/agents/chat/${threadId}/tool-contracts`;

    const ctx = createExecutionContext();
    const response = await worker.fetch(new Request(url), env, ctx);
    await waitOnExecutionContext(ctx);
    expect(response.status).toBe(200);

    const json = (await response.json()) as any[];
    expect(Array.isArray(json)).toBe(true);
    const names = json.map((t) => t?.name).filter(Boolean);
    expect(names).toContain("get_time");
    expect(names).toContain("bootstrap_set_user_handle");

    const getTime = json.find((t) => t?.name === "get_time");
    expect(getTime?.inputSchemaJson).toBeTruthy();
  });

  it("exposes DSE signature contracts for UI introspection", async () => {
    const threadId = `sigs-${Date.now()}`;
    const url = `http://example.com/agents/chat/${threadId}/signature-contracts`;

    const ctx = createExecutionContext();
    const response = await worker.fetch(new Request(url), env, ctx);
    await waitOnExecutionContext(ctx);
    expect(response.status).toBe(200);

    const json = (await response.json()) as any[];
    expect(Array.isArray(json)).toBe(true);
    const ids = json.map((s) => s?.signatureId).filter(Boolean);
    expect(ids.length).toBeGreaterThan(0);
    expect(ids).toContain("@openagents/autopilot/bootstrap/ExtractUserHandle.v1");
  });

  it("exposes DSE module contracts for UI introspection", async () => {
    const threadId = `mods-${Date.now()}`;
    const url = `http://example.com/agents/chat/${threadId}/module-contracts`;

    const ctx = createExecutionContext();
    const response = await worker.fetch(new Request(url), env, ctx);
    await waitOnExecutionContext(ctx);
    expect(response.status).toBe(200);

    const json = (await response.json()) as any[];
    expect(Array.isArray(json)).toBe(true);
    const ids = json.map((m) => m?.moduleId).filter(Boolean);
    expect(ids.length).toBeGreaterThan(0);
    expect(ids).toContain("@openagents/autopilot/BootstrapFlow.v1");
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

  it("stores DSE artifacts, promotes active, and rolls back via DO SQLite", async () => {
    const threadId = `dse-${Date.now()}`;
    const base = `http://example.com/agents/chat/${threadId}`;

    const signatureId = "@openagents/test/Sig.v1";

    const artifact1 = {
      format: "openagents.dse.compiled_artifact",
      formatVersion: 1,
      signatureId,
      compiled_id: "sha256:test1",
      createdAt: new Date().toISOString(),
      hashes: {
        inputSchemaHash: "sha256:in",
        outputSchemaHash: "sha256:out",
        promptIrHash: "sha256:prompt",
        paramsHash: "sha256:test1"
      },
      params: {
        paramsVersion: 1,
        instruction: { text: "v1" },
        fewShot: { exampleIds: [] },
        decode: { mode: "strict_json", maxRepairs: 0 }
      },
      eval: { evalVersion: 1, kind: "unscored" },
      optimizer: { id: "test" },
      provenance: {}
    };

    const artifact2 = {
      ...artifact1,
      compiled_id: "sha256:test2",
      hashes: { ...artifact1.hashes, paramsHash: "sha256:test2" },
      params: { ...artifact1.params, instruction: { text: "v2" } }
    };

    // Store artifacts.
    for (const artifact of [artifact1, artifact2]) {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(`${base}/dse/artifacts`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(artifact)
        }),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.ok).toBe(true);
    }

    // Promote artifact1.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(`${base}/dse/active`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ signatureId, compiled_id: artifact1.compiled_id })
        }),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.ok).toBe(true);
      expect(json.compiled_id).toBe(artifact1.compiled_id);
    }

    // Promote artifact2.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(`${base}/dse/active`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ signatureId, compiled_id: artifact2.compiled_id })
        }),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.ok).toBe(true);
      expect(json.compiled_id).toBe(artifact2.compiled_id);
    }

    // Roll back to artifact1.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(`${base}/dse/rollback`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ signatureId })
        }),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.ok).toBe(true);
      expect(json.from).toBe(artifact2.compiled_id);
      expect(json.to).toBe(artifact1.compiled_id);
    }

    // Active pointer reflects rollback.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(`${base}/dse/active?signatureId=${encodeURIComponent(signatureId)}`),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.compiled_id).toBe(artifact1.compiled_id);
    }
  });

  it("installs a default active DSE artifact for Blueprint tool routing", async () => {
    const threadId = `dse-default-${Date.now()}`;
    const base = `http://example.com/agents/chat/${threadId}`;

    // Trigger DO initialization.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(`${base}/get-messages`),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      await response.json();
    }

    const signatureId = "@openagents/autopilot/blueprint/SelectTool.v1";

    const compiled_id = await (async () => {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(
          `${base}/dse/active?signatureId=${encodeURIComponent(signatureId)}`
        ),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.signatureId).toBe(signatureId);
      expect(typeof json.compiled_id).toBe("string");
      expect(String(json.compiled_id).startsWith("sha256:")).toBe(true);
      return String(json.compiled_id);
    })();

    // Artifact can be fetched by signatureId+compiled_id.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(
          `${base}/dse/artifacts?signatureId=${encodeURIComponent(signatureId)}&compiled_id=${encodeURIComponent(compiled_id)}`
        ),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.format).toBe("openagents.dse.compiled_artifact");
      expect(json.formatVersion).toBe(1);
      expect(json.signatureId).toBe(signatureId);
      expect(json.compiled_id).toBe(compiled_id);
      expect(json.optimizer?.id).toBe("default_install.v1");
    }
  });
});
