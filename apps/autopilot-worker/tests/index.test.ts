import {
  env,
  createExecutionContext,
  waitOnExecutionContext
} from "cloudflare:test";
import { describe, it, expect } from "vitest";

const { default: worker } = await import("../src/server");
const { MessageType } = await import("../src/chatProtocol");

declare module "cloudflare:test" {
  interface ProvidedEnv extends Env {}
}

const makeAiStream = (lines: ReadonlyArray<string>): ReadableStream => {
  const encoder = new TextEncoder();
  return new ReadableStream({
    start(controller) {
      for (const line of lines) {
        controller.enqueue(encoder.encode(`${line}\n`));
      }
      controller.close();
    }
  });
};

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
    expect(names).toContain("lightning_l402_fetch");
    expect(names).toContain("lightning_paywall_create");
    expect(names).toContain("lightning_paywall_update");
    expect(names).toContain("lightning_paywall_settlement_list");

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

  it("records model receipts with finish.usage and tool receipts (stubbed Workers AI)", async () => {
    const threadId = `ai-receipts-${Date.now()}`;

    let call = 0;
    const stubAi = {
      run: async (_model: any, input: any) => {
        const wantsStream = Boolean(input && typeof input === "object" && (input as any).stream);
        if (!wantsStream) {
          return {
            choices: [{ message: { content: "ok" }, finish_reason: "stop" }],
            usage: { prompt_tokens: 1, completion_tokens: 1 }
          };
        }

        call++;

        if (call === 1) {
          return makeAiStream([
            'data: {"response":"Calling tool...","usage":{"prompt_tokens":12,"completion_tokens":3}}',
            'data: {"tool_calls":[{"index":0,"id":"call_0","type":"function","function":{"name":"bootstrap_set_user_handle","arguments":"{\\"handle\\":\\"Chris\\"}"}}]}',
            "data: [DONE]"
          ]);
        }

        return makeAiStream([
          'data: {"response":"Done.","usage":{"prompt_tokens":13,"completion_tokens":4}}',
          "data: [DONE]"
        ]);
      }
    };
    const envAny = env as any;
    const aiBinding = (envAny.AI ??= {}) as any;
    const originalRun = aiBinding.run;

    aiBinding.run = stubAi.run;
    try {

    // Connect to the agent DO over WebSocket (PartyServer / Agents).
    const wsCtx = createExecutionContext();
    const wsRes = await worker.fetch(
      new Request(`http://example.com/agents/chat/${threadId}`, {
        headers: { Upgrade: "websocket" }
      }),
      env,
      wsCtx
    );
    await waitOnExecutionContext(wsCtx);
    expect(wsRes.status).toBe(101);

    const ws = (wsRes as any).webSocket as WebSocket | undefined;
    expect(ws).toBeTruthy();
    ws!.accept();

    const requestId = `req-${Date.now()}`;
    const userMsgId = "user_1";
    const hugeText = "x".repeat(200_000);
    const wireParts: any[] = [];

    await new Promise<void>((resolve, reject) => {
      const timeoutId = setTimeout(() => reject(new Error("timeout waiting for done")), 2_000);

      const onMessage = (event: MessageEvent) => {
        if (typeof (event as any).data !== "string") return;
        let parsed: any;
        try {
          parsed = JSON.parse((event as any).data);
        } catch {
          return;
        }

        if (parsed?.type === MessageType.CF_AGENT_USE_CHAT_RESPONSE && parsed?.id === requestId) {
          const bodyText = typeof parsed?.body === "string" ? parsed.body : "";
          if (bodyText.trim()) {
            try {
              const part = JSON.parse(bodyText);
              if (part && typeof part === "object" && typeof (part as any).type === "string") {
                wireParts.push(part);
              }
            } catch {
              // ignore non-json bodies (e.g. error strings)
            }
          }

          if (parsed?.error) {
            clearTimeout(timeoutId);
            ws.removeEventListener("message", onMessage);
            reject(new Error("chat stream returned error"));
            return;
          }

          if (parsed?.done) {
            clearTimeout(timeoutId);
            ws.removeEventListener("message", onMessage);
            resolve();
          }
        }
      };

      ws.addEventListener("message", onMessage);

      ws.send(
        JSON.stringify({
          id: requestId,
          type: MessageType.CF_AGENT_USE_CHAT_REQUEST,
          init: {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({
              id: threadId,
              messages: [
                {
                  id: userMsgId,
                  role: "user",
                  parts: [{ type: "text", text: hugeText }]
                }
              ],
              trigger: "submit-message",
              messageId: userMsgId
            })
          }
        })
      );
    });

    // WebSocket should be closed to avoid vitest hanging on open handles.
    const closePromise = new Promise<void>((resolve) => {
      ws.addEventListener("close", () => resolve(), { once: true } as any);
    });
    try {
      ws.close(1000, "done");
    } catch {
      // ignore
    }
    await Promise.race([closePromise, new Promise<void>((r) => setTimeout(r, 250))]);

    // Ensure our stub binding was actually used.
    expect(call).toBeGreaterThan(0);

    // Wire protocol: Effect AI Response stream parts must be JSON and include tool call/result parts.
    expect(wireParts.length).toBeGreaterThan(0);
    const types = wireParts.map((p) => p?.type).filter(Boolean);
    expect(types).toContain("tool-call");
    expect(types).toContain("tool-result");
    expect(types).toContain("finish");
    expect(types).not.toContain("reasoning-start");
    expect(types).not.toContain("reasoning-delta");
    expect(types).not.toContain("reasoning-end");

    const toolCall = wireParts.find((p) => p?.type === "tool-call" && p?.name === "bootstrap_set_user_handle");
    expect(toolCall?.id).toBe("call_0");
    expect(toolCall?.params?.handle).toBe("Chris");

    const toolResult = wireParts.find((p) => p?.type === "tool-result" && p?.name === "bootstrap_set_user_handle");
    expect(toolResult?.id).toBe("call_0");
    expect(toolResult?.isFailure).toBe(false);
    expect(toolResult?.result?.ok).toBe(true);

    // Tool call should have executed and persisted into the Blueprint state.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(`http://example.com/agents/chat/${threadId}/blueprint`),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const blueprint = (await response.json()) as any;
      expect(blueprint?.docs?.user?.addressAs).toBe("Chris");
    }

    // Model receipts include finish.usage and obey prompt token cap.
    {
      const ctx = createExecutionContext();
      const response = await worker.fetch(
        new Request(`http://example.com/agents/chat/${threadId}/ai/receipts?limit=10`),
        env,
        ctx
      );
      await waitOnExecutionContext(ctx);
      expect(response.status).toBe(200);
      const receipts = (await response.json()) as any[];
      expect(receipts.length).toBeGreaterThan(0);

      const withFinish = receipts.find((r) => r && typeof r === "object" && "finish" in r) as any;
      expect(withFinish?.format).toBe("openagents.ai.model_receipt");
      expect(withFinish?.finish?.usage?.inputTokens).toBeTypeOf("number");
      expect(withFinish?.finish?.usage?.outputTokens).toBeTypeOf("number");
      expect(withFinish?.maxPromptTokens).toBe(8000);
      expect(withFinish?.promptTokenEstimate).toBeLessThanOrEqual(8000);

      // BlobRef discipline: receipts reference prompts/outputs by BlobRef (no huge inline payloads).
      expect(Array.isArray(withFinish?.promptBlobs)).toBe(true);
      expect(Array.isArray(withFinish?.outputBlobs)).toBe(true);
      expect(withFinish?.promptBlobs?.length).toBeGreaterThan(0);
      expect(withFinish?.outputBlobs?.length).toBeGreaterThan(0);
      expect(typeof withFinish?.promptBlobs?.[0]?.id).toBe("string");
      expect(typeof withFinish?.promptBlobs?.[0]?.hash).toBe("string");
      expect(withFinish?.promptBlobs?.[0]?.mime).toBe("application/json");
      expect(withFinish?.promptBlobs?.[0]?.size).toBeTypeOf("number");
      // A huge input message should be truncated before prompt serialization.
      expect(withFinish?.promptBlobs?.[0]?.size).toBeLessThan(hugeText.length);
    }

    // Tool receipts exist for the tool call.
    {
      // In isolated storage mode, DO writes triggered by WebSocket message
      // handling can be slightly laggy. Retry briefly to avoid flakes.
      const deadline = Date.now() + 1_000;
      let receipts: any[] = [];

      while (Date.now() < deadline) {
        const ctx = createExecutionContext();
        const response = await worker.fetch(
          new Request(
            `http://example.com/agents/chat/${threadId}/ai/tool-receipts?limit=10`
          ),
          env,
          ctx
        );
        await waitOnExecutionContext(ctx);
        expect(response.status).toBe(200);
        receipts = (await response.json()) as any[];
        if (receipts.length > 0) break;
        await new Promise((r) => setTimeout(r, 50));
      }

      expect(receipts.length).toBeGreaterThan(0);
      const names = receipts.map((r) => r?.toolName).filter(Boolean);
      expect(names).toContain("bootstrap_set_user_handle");

      const bootstrap = receipts.find((r) => r && r.toolName === "bootstrap_set_user_handle") as any;
      expect(Array.isArray(bootstrap?.inputBlobs)).toBe(true);
      expect(Array.isArray(bootstrap?.outputBlobs)).toBe(true);
      expect(bootstrap?.inputBlobs?.length).toBeGreaterThan(0);
      expect(bootstrap?.outputBlobs?.length).toBeGreaterThan(0);
      expect(bootstrap?.inputBlobs?.[0]?.mime).toBe("application/json");
      expect(bootstrap?.inputBlobs?.[0]?.size).toBeTypeOf("number");
      expect(typeof bootstrap?.paramsHash).toBe("string");
      expect(typeof bootstrap?.outputHash).toBe("string");
      expect(bootstrap?.paramsHash?.startsWith("sha256:")).toBe(true);
      expect(bootstrap?.outputHash?.startsWith("sha256:")).toBe(true);
      expect(typeof bootstrap?.latencyMs).toBe("number");
      expect(Array.isArray(bootstrap?.sideEffects)).toBe(true);
    }
    } finally {
      if (typeof originalRun === "function") {
        aiBinding.run = originalRun;
      } else {
        delete aiBinding.run;
      }
    }
  });
});
