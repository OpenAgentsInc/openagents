import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  APPLE_FM_DEFAULT_MODEL_ID,
  makeAppleFmClient,
  makeAppleFmToolCallbackSession,
  type AppleFmToolDefinition,
} from "../src";

describe("Apple FM snapshot streaming", () => {
  test("emits replacement snapshots and a separate final commit", async () => {
    const client = await Effect.runPromise(
      makeAppleFmClient({
        explicitBaseUrl: "http://127.0.0.1:11439",
        fetch: async (input, init) => {
          const url = new URL(String(input));
          expect(url.pathname).toBe("/v1/chat/completions");
          expect(init?.method).toBe("POST");
          const body = JSON.parse(String(init?.body));
          expect(body.stream).toBe(true);
          expect(body.streamMode).toBe("snapshot");

          return new Response(
            [
              JSON.stringify({ sequence: 0, content: "partial answer" }),
              JSON.stringify({ sequence: 1, content: "complete answer", finish_reason: "stop" }),
            ].join("\n"),
            {
              headers: {
                "Content-Type": "application/x-ndjson",
              },
            },
          );
        },
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );
    const result = await Effect.runPromise(
      client.streamPlainTextSnapshots([{ role: "user", content: "stream a response" }]),
    );

    expect(result.snapshots.map((snapshot) => snapshot.content)).toEqual(["partial answer", "complete answer"]);
    expect(result.completion.text).toBe("complete answer");
    expect(result.events.map((event) => event.kind)).toEqual([
      "assistant_stream_started",
      "assistant_snapshot",
      "assistant_snapshot",
      "assistant_stream_finished",
      "assistant_final_commit",
    ]);
    expect(result.events.some((event) => event.kind === "assistant_final_commit" && event.receipt !== undefined)).toBe(true);
    expect(JSON.stringify(result.events)).not.toContain("token_delta");
  });

  test("does not accumulate snapshots as deltas", async () => {
    const client = await Effect.runPromise(
      makeAppleFmClient({
        fetch: async () =>
          new Response(
            JSON.stringify([
              { sequence: 0, content: "alpha" },
              { sequence: 1, content: "alphabet" },
              { sequence: 2, content: "alphabet soup" },
            ]),
          ),
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );
    const result = await Effect.runPromise(client.streamPlainTextSnapshots([{ role: "user", content: "stream" }]));
    const rendered = result.events
      .filter((event) => event.kind === "assistant_snapshot")
      .reduce((_, event) => event.content ?? "", "");

    expect(rendered).toBe("alphabet soup");
    expect(rendered).not.toBe("alphaalphabetalphabet soup");
    expect(result.completion.text).toBe("alphabet soup");
  });

  test("snapshot stream failures emit typed receipts without final commit", async () => {
    const client = await Effect.runPromise(
      makeAppleFmClient({
        fetch: async () =>
          Response.json(
            {
              error: {
                message: "stream refused",
              },
            },
            { status: 503 },
          ),
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    await expect(
      Effect.runPromise(client.streamPlainTextSnapshots([{ role: "user", content: "stream" }])),
    ).rejects.toMatchObject({
      _tag: "AppleFmBackendError",
      failureClass: "stream_http_503",
      receipt: {
        kind: "probe_backend_failure",
        contentRedacted: true,
      },
    });
  });

  test("streams a bridge session with tool callbacks through Probe loopback", async () => {
    const tool: AppleFmToolDefinition = {
      name: "read_file",
      description: "Read README.md from the current Probe workspace.",
      inputSchema: {
        type: "object",
        title: "ReadFileArguments",
        properties: {
          path: {
            type: "string",
            title: "path",
            enum: ["README.md"],
          },
        },
        required: ["path"],
        "x-order": ["path"],
        additionalProperties: false,
      },
      policy: "allow",
      execute: (input) => Effect.succeed({ path: input.path, content: "# Probe\n\nhello from callback" }),
    };
    const toolSession = makeAppleFmToolCallbackSession({
      sessionId: "session_stream",
      token: "stream-secret",
      tools: [tool],
      now: new Date("2026-06-07T00:00:00.000Z"),
    });
    let callbackUrl = "";
    let callbackToken = "";
    const seenPaths: string[] = [];
    const client = await Effect.runPromise(
      makeAppleFmClient({
        explicitBaseUrl: "http://127.0.0.1:11439",
        fetch: async (input, init) => {
          const url = new URL(String(input));
          seenPaths.push(`${init?.method ?? "GET"} ${url.pathname}`);

          if (url.pathname === "/v1/sessions") {
            const body = JSON.parse(String(init?.body));
            callbackUrl = body.tool_callback.url;
            callbackToken = body.tool_callback.session_token;

            expect(body.tools[0].name).toBe("read_file");
            expect(body.tools[0].arguments_schema["x-order"]).toEqual(["path"]);
            expect(body.tools[0].arguments_schema.properties.path.enum).toEqual(["README.md"]);
            expect(JSON.stringify(body)).not.toContain("Bearer");

            return Response.json({
              session: {
                id: "sess-tool-stream",
              },
            });
          }

          expect(url.pathname).toBe("/v1/sessions/sess-tool-stream/responses/stream");
          const callbackResponse = await fetch(callbackUrl, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              session_token: callbackToken,
              tool_name: "read_file",
              arguments: {
                generation_id: "gen-stream-1",
                content: {
                  path: "README.md",
                },
                is_complete: true,
              },
            }),
          });
          const callbackBody = await callbackResponse.json() as { readonly output?: string };

          expect(callbackResponse.status).toBe(200);
          expect(callbackBody.output).toContain("hello from callback");

          return new Response(
            [
              "event: snapshot",
              "data: {\"kind\":\"snapshot\",\"model\":\"apple-foundation-model\",\"output\":\"Reading README.md\"}",
              "",
              "event: completed",
              "data: {\"kind\":\"completed\",\"model\":\"apple-foundation-model\",\"output\":\"README.md first heading is Probe.\",\"usage\":{\"total_tokens_detail\":{\"value\":12,\"truth\":\"estimated\"}}}",
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
    const result = await Effect.runPromise(
      client.streamSessionWithTools({
        prompt: "Use the read_file tool to inspect README.md.",
        instructions: "Use tools when asked to inspect files.",
        toolSession,
      }),
    );

    expect(seenPaths).toEqual([
      "POST /v1/sessions",
      "POST /v1/sessions/sess-tool-stream/responses/stream",
    ]);
    expect(result.events.map((event) => event.kind)).toEqual([
      "assistant_stream_started",
      "assistant_snapshot",
      "assistant_stream_finished",
      "assistant_final_commit",
    ]);
    expect(result.completion.text).toBe("README.md first heading is Probe.");
    expect(result.completion.usage).toEqual({ truth: "estimated", totalTokens: 12 });
    expect(result.toolTranscript[0]?.status).toBe("success");
    expect(result.toolTranscript[0]?.toolCallId).toBe("gen-stream-1");
    expect(JSON.stringify(result)).not.toContain("stream-secret");
  });
});
