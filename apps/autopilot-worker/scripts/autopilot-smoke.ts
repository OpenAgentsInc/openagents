import WebSocket, { type RawData } from "ws";
import { randomUUID } from "node:crypto";

const BASE_URL =
  process.env.AUTOPILOT_SMOKE_BASE_URL ?? "http://127.0.0.1:8787";
const THREAD_ID =
  process.env.AUTOPILOT_SMOKE_THREAD_ID ?? `smoke-${Date.now()}`;
const WS_URL = `${BASE_URL.replace(/^http/, "ws")}/agents/chat/${THREAD_ID}`;
const MESSAGE =
  process.env.AUTOPILOT_SMOKE_MESSAGE ??
  "SmokeUser";
const TTFT_LIMIT_MS = Number(process.env.AUTOPILOT_SMOKE_TTFT_MS ?? 10_000);
const RESPONSE_TIMEOUT_MS = Number(
  process.env.AUTOPILOT_SMOKE_RESPONSE_TIMEOUT_MS ?? 60_000
);
const REQUIRE_TOOL = (process.env.AUTOPILOT_SMOKE_REQUIRE_TOOL ?? "1") === "1";

const MESSAGE_TYPES = {
  chatRequest: "cf_agent_use_chat_request",
  chatResponse: "cf_agent_use_chat_response",
  streamResuming: "cf_agent_stream_resuming",
  streamResumeAck: "cf_agent_stream_resume_ack"
} as const;

type ChatResult = {
  text: string;
  ttftMs: number | null;
  durationMs: number;
};

const log = (message: string) => {
  console.log(`[autopilot-smoke] ${message}`);
};

const assert: (condition: unknown, message: string) => asserts condition = (
  condition,
  message
) => {
  if (!condition) {
    throw new Error(message);
  }
};

const sendChatMessage = async (content: string): Promise<ChatResult> => {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(WS_URL);
    const requestId = randomUUID();
    const messageId = randomUUID();
    const startAt = Date.now();
    let firstTokenAt: number | null = null;
    let buffer = "";
    let finished = false;

    const ttftTimer = setTimeout(() => {
      if (finished) return;
      finished = true;
      socket.close();
      reject(
        new Error(
          `TTFT exceeded ${TTFT_LIMIT_MS}ms before first token was received.`
        )
      );
    }, TTFT_LIMIT_MS);

    const responseTimer = setTimeout(() => {
      if (finished) return;
      finished = true;
      socket.close();
      reject(
        new Error(
          `Response timeout after ${RESPONSE_TIMEOUT_MS}ms waiting for completion.`
        )
      );
    }, RESPONSE_TIMEOUT_MS);

    const finalize = () => {
      if (finished) return;
      finished = true;
      clearTimeout(ttftTimer);
      clearTimeout(responseTimer);
      socket.close();
      resolve({
        text: buffer,
        ttftMs: firstTokenAt ? firstTokenAt - startAt : null,
        durationMs: Date.now() - startAt
      });
    };

    socket.on("open", () => {
      socket.send(
        JSON.stringify({
          type: MESSAGE_TYPES.chatRequest,
          id: requestId,
          init: {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({
              messages: [
                {
                  id: messageId,
                  role: "user",
                  parts: [{ type: "text", text: content }],
                },
              ],
            })
          }
        })
      );
    });

    socket.on("message", (data: RawData) => {
      let parsed: unknown = null;
      try {
        parsed = JSON.parse(data.toString());
      } catch {
        return;
      }
      if (!parsed || typeof parsed !== "object") return;
      const record = parsed as Record<string, unknown>;
      const type = record.type;

      if (type === MESSAGE_TYPES.streamResuming) {
        const resumeId = record.id;
        if (typeof resumeId === "string") {
          socket.send(
            JSON.stringify({
              type: MESSAGE_TYPES.streamResumeAck,
              id: resumeId
            })
          );
        }
        return;
      }

      if (type !== MESSAGE_TYPES.chatResponse) return;

      if (!firstTokenAt) firstTokenAt = Date.now();

      if (typeof record.body === "string") {
        buffer += record.body;
      }

      if (record.done) finalize();
    });

    socket.on("error", (error: Error) => {
      if (finished) return;
      finished = true;
      clearTimeout(ttftTimer);
      clearTimeout(responseTimer);
      reject(error);
    });

    socket.on("close", () => {
      if (!finished) {
        clearTimeout(ttftTimer);
        clearTimeout(responseTimer);
        reject(new Error("WebSocket closed before completion."));
      }
    });
  });
};

const getMessages = async () => {
  const url = `${BASE_URL}/agents/chat/${THREAD_ID}/get-messages`;
  const response = await fetch(url);
  if (!response.ok) return { ok: false as const, status: response.status };
  const json: any = await response.json().catch(() => null);
  return { ok: true as const, status: response.status, json };
};

const main = async () => {
  log(`Thread: ${THREAD_ID}`);
  log(`WS: ${WS_URL}`);

  const result = await sendChatMessage(MESSAGE);
  log(
    `ttft_ms=${result.ttftMs ?? "null"} duration_ms=${result.durationMs} chars=${result.text.length}`
  );
  assert(result.text.trim().length > 0, "Empty assistant response.");

  const messages = await getMessages();
  if (messages.ok) {
    log(`get-messages OK (status=${messages.status})`);

    if (REQUIRE_TOOL) {
      const allMessages: ReadonlyArray<any> = Array.isArray(messages.json?.messages)
        ? messages.json.messages
        : [];
      const allParts = allMessages.flatMap((m) => (Array.isArray(m?.parts) ? m.parts : []));
      const toolInvocations = allParts.filter((p) => {
        const t = p?.type;
        return typeof t === "string" && (t === "dynamic-tool" || t.startsWith("tool-"));
      });
      const completedInvocations = toolInvocations.filter(
        (p) => typeof p?.state === "string" && p.state.startsWith("output-"),
      );

      assert(toolInvocations.length > 0, "No tool invocations recorded in get-messages.");
      assert(
        completedInvocations.length > 0,
        "No tool outputs recorded in get-messages (expected output-* state).",
      );

      log(`tool_invocations=${toolInvocations.length} tool_outputs=${completedInvocations.length}`);
    }
  } else {
    log(`get-messages failed (status=${messages.status})`);
  }
};

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
