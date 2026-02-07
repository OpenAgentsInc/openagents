import {
  env,
  createExecutionContext,
  waitOnExecutionContext
} from "cloudflare:test";
import { describe, expect, it } from "vitest";

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

const makeAiStreamWithDelayedDone = (linesBeforeDone: ReadonlyArray<string>, delayMs: number): ReadableStream => {
  const encoder = new TextEncoder();
  return new ReadableStream({
    start(controller) {
      for (const line of linesBeforeDone) {
        controller.enqueue(encoder.encode(`${line}\n`));
      }

      setTimeout(() => {
        try {
          controller.enqueue(encoder.encode("data: [DONE]\n"));
          controller.close();
        } catch {
          // Stream was cancelled/closed.
        }
      }, Math.max(0, Math.floor(delayMs)));
    }
  });
};

const connectWs = async (threadId: string) => {
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

  return ws!;
};

const closeWs = async (ws: WebSocket) => {
  const closePromise = new Promise<void>((resolve) => {
    ws.addEventListener("close", () => resolve(), { once: true } as any);
  });
  try {
    ws.close(1000, "done");
  } catch {
    // ignore
  }
  await Promise.race([closePromise, new Promise<void>((r) => setTimeout(r, 250))]);
};

const getModelReceipts = async (threadId: string) => {
  const ctx = createExecutionContext();
  const response = await worker.fetch(
    new Request(`http://example.com/agents/chat/${threadId}/ai/receipts?limit=25`),
    env,
    ctx
  );
  await waitOnExecutionContext(ctx);
  expect(response.status).toBe(200);
  return (await response.json()) as any[];
};

describe("Autopilot chat protocol", () => {
  it("propagates provider finish_reason into wire finish parts and model receipts", async () => {
    const threadId = `finish-reason-${Date.now()}`;

    const stubAi = {
      run: async (_model: any, input: any) => {
        const wantsStream = Boolean(input && typeof input === "object" && (input as any).stream);
        if (!wantsStream) {
          return {
            choices: [{ message: { content: "ok" }, finish_reason: "stop" }],
            usage: { prompt_tokens: 1, completion_tokens: 1 }
          };
        }

        return makeAiStream([
          'data: {"response":"hi","usage":{"prompt_tokens":2,"completion_tokens":1}}',
          'data: {"choices":[{"finish_reason":"length","delta":{}}]}',
          "data: [DONE]"
        ]);
      }
    };

    const envAny = env as any;
    const aiBinding = (envAny.AI ??= {}) as any;
    const originalRun = aiBinding.run;
    aiBinding.run = stubAi.run;

    const ws = await connectWs(threadId);
    try {
      const requestId = `req-${Date.now()}`;
      const userMsgId = "user_1";
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
                // ignore non-json bodies
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
                    parts: [{ type: "text", text: "hello" }]
                  }
                ],
                trigger: "submit-message",
                messageId: userMsgId
              })
            }
          })
        );
      });

      const finish = wireParts.find((p) => p?.type === "finish") as any;
      expect(finish?.reason).toBe("length");

      const receipts = await getModelReceipts(threadId);
      const match = receipts.find((r) => r?.correlation?.requestId === requestId && r?.finish?.reason === "length") as any;
      expect(match).toBeTruthy();
      expect(match?.result?._tag).toBe("Ok");
    } finally {
      await closeWs(ws);
      if (typeof originalRun === "function") {
        aiBinding.run = originalRun;
      } else {
        delete aiBinding.run;
      }
    }
  });

  it("supports cancel: emits a terminal finish part (pause) and records a receipt with a cancellation reason", async () => {
    const threadId = `cancel-${Date.now()}`;

    const stubAi = {
      run: async (_model: any, input: any) => {
        const wantsStream = Boolean(input && typeof input === "object" && (input as any).stream);
        if (!wantsStream) {
          return {
            choices: [{ message: { content: "ok" }, finish_reason: "stop" }],
            usage: { prompt_tokens: 1, completion_tokens: 1 }
          };
        }

        // Stream a first chunk immediately, but delay DONE long enough for a cancel request to arrive.
        return makeAiStreamWithDelayedDone(
          ['data: {"response":"streaming...","usage":{"prompt_tokens":3,"completion_tokens":1}}'],
          500
        );
      }
    };

    const envAny = env as any;
    const aiBinding = (envAny.AI ??= {}) as any;
    const originalRun = aiBinding.run;
    aiBinding.run = stubAi.run;

    const ws = await connectWs(threadId);
    try {
      const requestId = `req-${Date.now()}`;
      const userMsgId = "user_1";

      const doneMessage = await new Promise<{ readonly donePart: any; readonly ms: number }>(
        (resolve, reject) => {
          const startedAt = Date.now();
          const timeoutId = setTimeout(() => reject(new Error("timeout waiting for cancel finish")), 2_000);

          let sentCancel = false;

          const onMessage = (event: MessageEvent) => {
            if (typeof (event as any).data !== "string") return;
            let parsed: any;
            try {
              parsed = JSON.parse((event as any).data);
            } catch {
              return;
            }

            if (parsed?.type !== MessageType.CF_AGENT_USE_CHAT_RESPONSE || parsed?.id !== requestId) {
              return;
            }

            const bodyText = typeof parsed?.body === "string" ? parsed.body : "";
            let part: any = null;
            if (bodyText.trim()) {
              try {
                part = JSON.parse(bodyText);
              } catch {
                part = null;
              }
            }

            // Cancel as soon as we see the first stream part.
            if (!sentCancel && part && typeof part === "object" && typeof part.type === "string") {
              sentCancel = true;
              ws.send(
                JSON.stringify({
                  type: MessageType.CF_AGENT_CHAT_REQUEST_CANCEL,
                  id: requestId
                })
              );
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
              resolve({ donePart: part, ms: Date.now() - startedAt });
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
                      parts: [{ type: "text", text: "hello" }]
                    }
                  ],
                  trigger: "submit-message",
                  messageId: userMsgId
                })
              }
            })
          );
        }
      );

      // Cancel should produce a schema-valid finish part.
      expect(doneMessage.donePart?.type).toBe("finish");
      expect(doneMessage.donePart?.reason).toBe("pause");
      // Ensure the cancel completion doesn't wait for the underlying AI stream to finish.
      expect(doneMessage.ms).toBeLessThan(300);

      // The model receipt is written after the step finalizer runs; poll briefly to avoid flakes.
      const deadline = Date.now() + 2_000;
      let match: any = null;

      while (Date.now() < deadline) {
        const receipts = await getModelReceipts(threadId);
        match = receipts.find((r) => r?.correlation?.requestId === requestId) as any;
        if (match) break;
        await new Promise((r) => setTimeout(r, 50));
      }

      expect(match).toBeTruthy();
      expect(match?.finish?.reason).toBe("pause");
      expect(match?.result?._tag).toBe("Error");
      expect(String(match?.result?.message ?? "")).toContain("Cancelled");
    } finally {
      await closeWs(ws);
      if (typeof originalRun === "function") {
        aiBinding.run = originalRun;
      } else {
        delete aiBinding.run;
      }
    }
  });
});

