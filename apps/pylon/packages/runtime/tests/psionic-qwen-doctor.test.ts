import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { makePsionicQwenClient, modelRefFromModelId, runProbeCli } from "../src";

describe("Psionic Qwen doctor", () => {
  test("reports ready for a Psionic server with Qwen3.5 models", async () => {
    const client = await Effect.runPromise(makePsionicQwenClient({
      explicitBaseUrl: "http://127.0.0.1:18080",
      fetch: fakePsionicFetch({
        health: {
          ready: true,
          execution_engine: "psionic",
          supported_endpoints: ["/v1/chat/completions", "/v1/responses"],
        },
        models: {
          data: [
            {
              id: "qwen3.5-0.8b",
              artifact_digest: "afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5",
            },
            {
              id: "qwen3.5-2b",
              artifact_manifest_ref: "artifact.psionic.qwen35.2b.q8_0.manifest",
            },
          ],
        },
      }),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const readiness = await Effect.runPromise(client.doctor());

    expect(readiness.ready).toBe(true);
    expect(readiness.status).toBe("ready");
    expect(readiness.modelRefs).toContain("model.psionic.qwen35.0_8b.q8_0");
    expect(readiness.modelRefs).toContain("model.psionic.qwen35.2b.q8_0");
    expect(readiness.observedModelRefs).toContain("model.psionic.qwen35.0_8b.q8_0");
    expect(readiness.codingAgentSelection?.selectedModelRef).toBe("model.psionic.qwen35.2b.q8_0");
    expect(readiness.supportedEndpointRefs).toContain("endpoint.psionic.v1.chat_completions");
    expect(readiness.supportedEndpointRefs).toContain("endpoint.psionic.v1.responses");
    expect(readiness.blockerRefs).toEqual([]);
    expect(JSON.stringify(readiness.receipt)).not.toContain("18080?");
  });

  test("reports unreachable without throwing when Psionic is absent", async () => {
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: async () => {
        throw new Error("connection refused");
      },
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const readiness = await Effect.runPromise(client.doctor());

    expect(readiness.ready).toBe(false);
    expect(readiness.status).toBe("unreachable");
    expect(readiness.blockerRefs).toContain("blocker.psionic_qwen35.health_unreachable");
  });

  test("reports malformed without throwing for bad health JSON", async () => {
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: fakePsionicFetch({
        healthResponse: new Response("not json", { status: 200 }),
        models: { data: [] },
      }),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const readiness = await Effect.runPromise(client.doctor());

    expect(readiness.ready).toBe(false);
    expect(readiness.status).toBe("malformed");
    expect(readiness.blockerRefs).toContain("blocker.psionic_qwen35.health_unreachable");
  });

  test("blocks non-Psionic execution engines", async () => {
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: fakePsionicFetch({
        health: {
          ready: true,
          execution_engine: "llama_cpp",
          supported_endpoints: ["/v1/chat/completions"],
        },
        models: {
          data: [{ id: "qwen3.5-2b", artifact_manifest_ref: "artifact.psionic.qwen35.2b.q8_0.manifest" }],
        },
      }),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const readiness = await Effect.runPromise(client.doctor());

    expect(readiness.ready).toBe(false);
    expect(readiness.status).toBe("configured");
    expect(readiness.blockerRefs).toContain("blocker.psionic_qwen35.execution_engine_not_psionic");
  });

  test("CLI JSON doctor returns a typed failure without process crashes", async () => {
    const result = await Effect.runPromise(runProbeCli(["backend", "psionic", "doctor", "--json"], {
      fetch: fakePsionicFetch({
        health: {
          ready: true,
          execution_engine: "psionic",
        },
        models: {
          data: [],
        },
      }),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const payload = JSON.parse(result.stdout);

    expect(result.exitCode).toBe(1);
    expect(payload.profile.kind).toBe("psionic_qwen35");
    expect(payload.blockerRefs).toContain("blocker.psionic_qwen35.qwen35_model_missing");
  });

  test("CLI JSON smoke runs real inference even when the observed model is not launch-admitted", async () => {
    const result = await Effect.runPromise(runProbeCli([
      "backend",
      "psionic",
      "smoke",
      "--json",
      "--prompt",
      "Reply with exactly: psionic pylon live",
    ], {
      fetch: fakePsionicFetch({
        health: {
          ready: true,
          execution_engine: "psionic",
          default_model: "Qwen_Qwen3.5-0.8B-Q4_K_M.gguf",
          supported_endpoints: ["/v1/chat/completions", "/v1/responses"],
        },
        models: {
          data: [{
            id: "Qwen_Qwen3.5-0.8B-Q4_K_M.gguf",
            psionic_model_family: "qwen35",
            psionic_execution_engine: "psionic",
          }],
        },
        chat: {
          choices: [
            {
              message: { role: "assistant", content: "psionic pylon live" },
              finish_reason: "stop",
            },
          ],
          usage: { prompt_tokens: 21, completion_tokens: 5, total_tokens: 26 },
        },
      }),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const payload = JSON.parse(result.stdout);

    expect(result.exitCode).toBe(0);
    expect(payload.state).toBe("passed");
    expect(payload.inference).toBe("real_psionic_openai_compatible");
    expect(payload.model).toBe("Qwen_Qwen3.5-0.8B-Q4_K_M.gguf");
    expect(payload.text).toBe("psionic pylon live");
    expect(payload.readiness.ready).toBe(false);
    expect(payload.admissionBlockerRefs).toContain("blocker.psionic_qwen35.artifact_digest_unverified");
    expect(payload.admissionBlockerRefs).toContain("blocker.psionic_qwen35.qwen35_model_missing");
    expect(payload.receipt.contentRedacted).toBe(true);
  });

  test("CLI smoke blocks non-Psionic engines before sending a completion", async () => {
    let chatCalls = 0;
    const result = await Effect.runPromise(runProbeCli(["backend", "psionic", "smoke", "--json"], {
      fetch: fakePsionicFetch({
        health: {
          ready: true,
          execution_engine: "llama_cpp",
          default_model: "Qwen_Qwen3.5-0.8B-Q4_K_M.gguf",
          supported_endpoints: ["/v1/chat/completions"],
        },
        models: { data: ["Qwen_Qwen3.5-0.8B-Q4_K_M.gguf"] },
        onChat: () => {
          chatCalls += 1;
        },
      }),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const payload = JSON.parse(result.stdout);

    expect(result.exitCode).toBe(1);
    expect(payload.state).toBe("blocked");
    expect(payload.blockerRefs).toContain("blocker.psionic_qwen35.execution_engine_not_psionic");
    expect(chatCalls).toBe(0);
  });

  test("maps Qwen3.5 0.8B and 2B identifiers to public model refs", () => {
    expect(modelRefFromModelId("Qwen3.5-0.8B-Instruct-GGUF")).toEqual(["model.psionic.qwen35.0_8b.q8_0"]);
    expect(modelRefFromModelId("qwen35:2b-q8_0")).toEqual(["model.psionic.qwen35.2b.q8_0"]);
    expect(modelRefFromModelId("/Users/example/qwen3.5-2b.gguf")).toEqual(["model.psionic.qwen35.2b.q8_0"]);
    expect(modelRefFromModelId("gemma-2b")).toEqual([]);
  });
});

function fakePsionicFetch(input: {
  readonly health?: unknown;
  readonly healthResponse?: Response;
  readonly models?: unknown;
  readonly modelsResponse?: Response;
  readonly chat?: unknown;
  readonly onChat?: () => void;
}): typeof fetch {
  return async (url, init) => {
    const path = new URL(url.toString()).pathname;

    if (path === "/health") {
      return input.healthResponse ?? Response.json(input.health ?? { ready: true, execution_engine: "psionic" });
    }

    if (path === "/v1/models") {
      return input.modelsResponse ?? Response.json(input.models ?? { data: [] });
    }

    if (path === "/v1/chat/completions" && init?.method === "POST") {
      input.onChat?.();
      return Response.json(input.chat ?? {
        choices: [
          {
            message: { role: "assistant", content: "psionic pylon live" },
            finish_reason: "stop",
          },
        ],
      });
    }

    return new Response("not found", { status: 404 });
  };
}
