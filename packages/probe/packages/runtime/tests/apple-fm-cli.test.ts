import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { APPLE_FM_DEFAULT_MODEL_ID, runProbeCli } from "../src";

describe("Probe CLI Apple FM commands", () => {
  test("probe apple-fm status reports a ready fake bridge without inference", async () => {
    const seenMethods: string[] = [];
    const fetchImpl: typeof fetch = async (input, init) => {
      seenMethods.push(init?.method ?? "GET");
      expect(new URL(String(input)).pathname).toBe("/health");
      return Response.json({
        ready: true,
        modelId: APPLE_FM_DEFAULT_MODEL_ID,
        platform: "fake-apple-silicon",
        version: "test",
      });
    };

    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "status", "--base-url", "http://127.0.0.1:11439"], {
        fetch: fetchImpl,
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("status: ready");
    expect(result.stdout).toContain("kind: apple_fm_bridge");
    expect(result.stdout).toContain("platform: fake-apple-silicon");
    expect(result.stdout).toContain("\"contentRedacted\":true");
    expect(seenMethods).toEqual(["GET"]);
  });

  test("probe apple-fm status reports unsupported fake hardware as non-ready", async () => {
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "status"], {
        fetch: async () =>
          Response.json({
            ready: false,
            modelId: APPLE_FM_DEFAULT_MODEL_ID,
            unavailableReason: "unsupported_hardware",
            message: "Apple Foundation Models are unavailable on this host.",
          }),
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(result.exitCode).toBe(1);
    expect(result.stdout).toContain("status: unsupported");
    expect(result.stdout).toContain("unavailableReason: unsupported_hardware");
    expect(result.stdout).toContain("Apple Foundation Models are unavailable");
  });

  test("probe apple-fm status reports unreachable bridge without generic success", async () => {
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "status"], {
        fetch: async () => {
          throw new Error("connection refused");
        },
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(result.exitCode).toBe(1);
    expect(result.stdout).toContain("status: unreachable");
    expect(result.stdout).toContain("unavailableReason: bridge_unreachable");
    expect(result.stdout).toContain("\"ready\":false");
  });

  test("probe apple-fm smoke requires readiness before completing plain text", async () => {
    const seenPaths: string[] = [];
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "smoke", "--base-url", "http://127.0.0.1:11439", "--prompt", "hello"], {
        fetch: async (input, init) => {
          const url = new URL(String(input));
          seenPaths.push(`${init?.method ?? "GET"} ${url.pathname}`);

          if (url.pathname === "/health") {
            return Response.json({
              ready: true,
              modelId: APPLE_FM_DEFAULT_MODEL_ID,
            });
          }

          expect(url.pathname).toBe("/v1/chat/completions");
          expect(init?.method).toBe("POST");
          const body = JSON.parse(String(init?.body));
          expect(body.messages).toEqual([{ role: "user", content: "hello" }]);

          return Response.json({
            id: "fake_completion_1",
            model: APPLE_FM_DEFAULT_MODEL_ID,
            choices: [
              {
                index: 0,
                message: {
                  role: "assistant",
                  content: "probe apple fm smoke ok",
                },
                finish_reason: "stop",
              },
            ],
            usage: {
              prompt_tokens: 2,
              completion_tokens: 5,
              total_tokens: 7,
            },
          });
        },
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("probe: probe apple fm smoke ok");
    expect(result.stdout).toContain("usage: truth=estimated prompt=2 completion=5 total=7");
    expect(result.stdout).toContain("\"kind\":\"probe_backend_transcript\"");
    expect(seenPaths).toEqual(["GET /health", "POST /v1/chat/completions"]);
  });

  test("probe apple-fm smoke refuses to complete when health is unavailable", async () => {
    const seenPaths: string[] = [];
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "smoke"], {
        fetch: async (input, init) => {
          const url = new URL(String(input));
          seenPaths.push(`${init?.method ?? "GET"} ${url.pathname}`);
          return Response.json({
            ready: false,
            modelId: APPLE_FM_DEFAULT_MODEL_ID,
            unavailableReason: "model_unavailable",
            message: "model not ready",
          });
        },
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(result.exitCode).toBe(1);
    expect(result.stdout).toContain("Apple FM smoke failed");
    expect(result.stdout).toContain("failureClass: model_unavailable");
    expect(result.stdout).toContain("model not ready");
    expect(seenPaths).toEqual(["GET /health"]);
  });

  test("probe apple-fm smoke surfaces typed completion errors", async () => {
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "smoke"], {
        fetch: async (input) => {
          const url = new URL(String(input));

          if (url.pathname === "/health") {
            return Response.json({
              ready: true,
              modelId: APPLE_FM_DEFAULT_MODEL_ID,
            });
          }

          return Response.json(
            {
              error: {
                code: "foundation_model_refused",
                message: "Foundation Models refused the request.",
              },
            },
            { status: 500 },
          );
        },
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(result.exitCode).toBe(1);
    expect(result.stdout).toContain("failureClass: completion_http_500");
    expect(result.stdout).toContain("Foundation Models refused the request.");
    expect(result.stdout).toContain("\"kind\":\"probe_backend_failure\"");
  });

  test("probe apple-fm tool-stream-demo reads files relative to the Probe workspace root", async () => {
    let callbackUrl = "";
    let callbackToken = "";
    let callbackOutput = "";
    const result = await Effect.runPromise(
      runProbeCli([
        "apple-fm",
        "tool-stream-demo",
        "--base-url",
        "http://127.0.0.1:11439",
        "--path",
        "README.md",
        "--prompt",
        "Use the read_file tool to inspect README.md.",
      ], {
        fetch: async (input, init) => {
          const url = new URL(String(input));

          if (url.pathname === "/health") {
            return Response.json({
              ready: true,
              modelId: APPLE_FM_DEFAULT_MODEL_ID,
            });
          }

          if (url.pathname === "/v1/sessions") {
            const body = JSON.parse(String(init?.body));
            callbackUrl = body.tool_callback.url;
            callbackToken = body.tool_callback.session_token;
            expect(body.tools).toHaveLength(1);
            expect(body.tools[0].name).toBe("read_file");
            expect(body.tools[0].arguments_schema.properties.path.enum).toEqual(["README.md"]);

            return Response.json({
              session: {
                id: "sess-cli-tool-demo",
              },
            });
          }

          const callbackResponse = await fetch(callbackUrl, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              session_token: callbackToken,
              tool_name: "read_file",
              arguments: {
                generation_id: "gen-cli-1",
                content: {
                  path: "README.md",
                },
                is_complete: true,
              },
            }),
          });
          const callbackBody = await callbackResponse.json() as { readonly output?: string };
          callbackOutput = callbackBody.output ?? "";

          return new Response(
            [
              "event: completed",
              "data: {\"kind\":\"completed\",\"model\":\"apple-foundation-model\",\"output\":\"README.md first heading is Probe.\"}",
              "",
            ].join("\n"),
            {
              headers: {
                "Content-Type": "text/event-stream",
              },
            },
          );
        },
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("blueprintLookupId: blueprint_signature_lookup.apple_fm.tool_stream_demo");
    expect(result.stdout).toContain("blueprintTools: tool.probe.read_file:read_file");
    expect(result.stdout).toContain("programRunRef: program_run.probe.apple_fm.sess-cli-tool-demo");
    expect(result.stdout).toContain("programRunInputSnapshotHash: sha256:");
    expect(result.stdout).toContain("tool: read_file success");
    expect(result.stdout).toContain("final: README.md first heading is Probe.");
    expect(callbackOutput).toContain("# Probe");
  });
});
