import WebSocket, { type RawData } from "ws";
import { randomUUID } from "node:crypto";
import { createSkyValidators } from "../src/sky/contracts";

const BASE_URL =
  process.env.LITECLAW_SMOKE_BASE_URL ?? "http://127.0.0.1:8787";
const THREAD_ID =
  process.env.LITECLAW_SMOKE_THREAD_ID ?? `smoke-${Date.now()}`;
const WS_URL = `${BASE_URL.replace(/^http/, "ws")}/agents/chat/${THREAD_ID}`;
const TTFT_LIMIT_MS = Number(process.env.LITECLAW_SMOKE_TTFT_MS ?? 10_000);
const RESPONSE_TIMEOUT_MS = Number(
  process.env.LITECLAW_SMOKE_RESPONSE_TIMEOUT_MS ?? 60_000
);
const STRICT =
  process.env.LITECLAW_SMOKE_STRICT === "1" || process.env.CI === "1";
const ADMIN_SECRET =
  process.env.LITECLAW_SMOKE_ADMIN_SECRET ??
  process.env.LITECLAW_TOOL_ADMIN_SECRET ??
  process.env.LITECLAW_EXTENSION_ADMIN_SECRET ??
  "";
const EXTENSION_ALLOWLIST =
  process.env.LITECLAW_SMOKE_EXTENSION_ALLOWLIST ??
  process.env.LITECLAW_EXTENSION_ALLOWLIST ??
  "";
const EXPECT_EXECUTOR =
  process.env.LITECLAW_SMOKE_EXECUTOR_KIND ??
  process.env.LITECLAW_EXECUTOR_KIND ??
  "workers";
const REQUIRE_SKY =
  process.env.LITECLAW_SMOKE_REQUIRE_SKY !== "0" &&
  process.env.LITECLAW_SMOKE_REQUIRE_SKY !== "false";

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

type ExportSummary = {
  headerCount: number;
  memoryCount: number;
  messageCount: number;
  runCount: number;
  eventCount: number;
  receiptCount: number;
  toolReceipts: Array<{
    tool_name: string;
    local_receipt: unknown | null | undefined;
  }>;
};

const log = (message: string) => {
  console.log(`[liteclaw-smoke] ${message}`);
};

const warn = (message: string) => {
  console.warn(`[liteclaw-smoke] ${message}`);
};

const assert: (condition: unknown, message: string) => asserts condition = (
  condition,
  message
) => {
  if (!condition) {
    throw new Error(message);
  }
};

const requireStep = (condition: boolean, label: string) => {
  if (condition) return true;
  const message = `Skipping ${label}. Missing prerequisites.`;
  if (STRICT) {
    throw new Error(message);
  }
  warn(message);
  return false;
};

const fetchJson = async (url: string, options?: RequestInit) => {
  const response = await fetch(url, {
    ...options,
    headers: {
      "content-type": "application/json",
      ...(options?.headers ?? {})
    }
  });
  const text = await response.text();
  let json: unknown = null;
  if (text) {
    try {
      json = JSON.parse(text);
    } catch {
      json = null;
    }
  }
  return { response, json, text };
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
      const payload = {
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
                content
              }
            ]
          })
        }
      };
      socket.send(JSON.stringify(payload));
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

      if (!firstTokenAt) {
        firstTokenAt = Date.now();
      }

      if (typeof record.body === "string") {
        buffer += record.body;
      }

      if (record.done) {
        finalize();
      }
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

const validateExport = async (): Promise<ExportSummary> => {
  const exportUrl = `${BASE_URL}/agents/chat/${THREAD_ID}/export`;
  const response = await fetch(exportUrl);
  assert(response.ok, `Export failed: ${response.status}`);
  const text = await response.text();
  const lines = text.split("\n").filter((line) => line.trim().length > 0);

  const validators = createSkyValidators();
  let headerCount = 0;
  let memoryCount = 0;
  let messageCount = 0;
  let runCount = 0;
  let eventCount = 0;
  let receiptCount = 0;
  const toolReceipts: ExportSummary["toolReceipts"] = [];

  for (const line of lines) {
    const entry = JSON.parse(line) as Record<string, unknown>;
    const type = entry.type;

    if (type === "liteclaw.export") {
      headerCount += 1;
      assert(entry.thread_id === THREAD_ID, "Export header thread_id mismatch.");
      continue;
    }

    if (type === "memory") {
      const payload = entry.payload;
      assert(
        validators.validateMemory(payload),
        "Memory payload failed schema validation."
      );
      memoryCount += 1;
      continue;
    }

    if (type === "message") {
      const payload = entry.payload;
      assert(
        validators.validateMessage(payload),
        "Message payload failed schema validation."
      );
      messageCount += 1;
      continue;
    }

    if (type === "run") {
      const payload = entry.payload;
      assert(
        validators.validateRun(payload),
        "Run payload failed schema validation."
      );
      runCount += 1;
      continue;
    }

    if (type === "event") {
      const payload = entry.payload as Record<string, unknown>;
      const eventType = payload.type;
      assert(
        typeof eventType === "string",
        "Event payload missing type."
      );
      const ok = validators.validateEventPayload(
        eventType as Parameters<typeof validators.validateEventPayload>[0],
        payload.payload
      );
      assert(ok, `Event payload failed schema validation: ${eventType}`);
      eventCount += 1;
      continue;
    }

    if (type === "receipt") {
      const payload = entry.payload as Record<string, unknown>;
      const receipt = payload.receipt;
      assert(
        validators.validateReceipt(receipt),
        "Receipt payload failed schema validation."
      );
      receiptCount += 1;
      if (
        receipt &&
        typeof receipt === "object" &&
        (receipt as { type?: unknown }).type === "tool"
      ) {
        toolReceipts.push({
          tool_name: (receipt as { tool_name?: string }).tool_name ?? "",
          local_receipt: (receipt as { local_receipt?: unknown }).local_receipt
        });
      }
      continue;
    }

    throw new Error(`Unknown export line type: ${String(type)}`);
  }

  return {
    headerCount,
    memoryCount,
    messageCount,
    runCount,
    eventCount,
    receiptCount,
    toolReceipts
  };
};

const getToolPolicy = async () => {
  const url = `${BASE_URL}/agents/chat/${THREAD_ID}/tool-policy`;
  const { response, json, text } = await fetchJson(url, {
    headers: {
      "x-liteclaw-admin-secret": ADMIN_SECRET
    }
  });
  assert(response.ok, `Tool policy GET failed: ${response.status} ${text}`);
  return (json as { policy?: string } | null)?.policy ?? null;
};

const setToolPolicy = async (policy: string) => {
  const url = `${BASE_URL}/agents/chat/${THREAD_ID}/tool-policy`;
  const { response, json, text } = await fetchJson(url, {
    method: "POST",
    headers: {
      "x-liteclaw-admin-secret": ADMIN_SECRET
    },
    body: JSON.stringify({ policy })
  });
  assert(response.ok, `Tool policy POST failed: ${response.status} ${text}`);
  return (json as { policy?: string } | null)?.policy ?? null;
};

const run = async () => {
  log(`Base URL: ${BASE_URL}`);
  log(`Thread ID: ${THREAD_ID}`);
  let ranToolSteps = false;

  log("Sending baseline chat message...");
  const baseline = await sendChatMessage("Reply with OK.");
  assert(
    baseline.ttftMs !== null && baseline.ttftMs <= TTFT_LIMIT_MS,
    `TTFT exceeded ${TTFT_LIMIT_MS}ms (got ${baseline.ttftMs ?? "none"}ms).`
  );
  assert(baseline.text.trim().length > 0, "No assistant text returned.");
  log(`Baseline TTFT: ${baseline.ttftMs}ms`);

  const messagesUrl = `${BASE_URL}/agents/chat/${THREAD_ID}/get-messages`;
  const messagesResponse = await fetch(messagesUrl);
  assert(messagesResponse.ok, `get-messages failed: ${messagesResponse.status}`);
  const messages = (await messagesResponse.json()) as unknown;
  assert(Array.isArray(messages), "get-messages did not return an array.");
  assert(messages.length >= 2, "Transcript missing user+assistant messages.");
  log(`Transcript messages: ${messages.length}`);

  if (requireStep(Boolean(ADMIN_SECRET), "tool policy toggle")) {
    const originalPolicy = await getToolPolicy();
    log(`Original tool policy: ${originalPolicy}`);
    await setToolPolicy("read-write");
    log("Tool policy set to read-write for smoke tools.");

    log("Sending workspace tool prompt...");
    await sendChatMessage(
      "You are running a tool smoke test. Use workspace.write to write a file named smoke.txt with content 'smoke-ok'. Then use workspace.read to read it. Then use workspace.edit to replace 'smoke-ok' with 'smoke-ok-2'. Reply with 'DONE:' followed by the final file content only."
    );

    log("Sending http.fetch tool prompt...");
    await sendChatMessage(
      "Use http.fetch to GET https://example.com and reply with 'STATUS:' followed by the numeric status code only."
    );
    ranToolSteps = true;

    if (originalPolicy) {
      await setToolPolicy(originalPolicy);
      log(`Restored tool policy to ${originalPolicy}.`);
    }
  }

  if (requireStep(Boolean(ADMIN_SECRET), "extensions admin")) {
    const catalogUrl = `${BASE_URL}/agents/chat/${THREAD_ID}/extensions/catalog`;
    const policyUrl = `${BASE_URL}/agents/chat/${THREAD_ID}/extensions`;

    const catalog = await fetchJson(catalogUrl, {
      headers: { "x-liteclaw-admin-secret": ADMIN_SECRET }
    });
    assert(catalog.response.ok, "extensions/catalog GET failed.");

    await fetchJson(catalogUrl, {
      method: "POST",
      headers: { "x-liteclaw-admin-secret": ADMIN_SECRET },
      body: JSON.stringify({
        extensions: [
          {
            id: "sky.echo",
            name: "Sky Echo",
            version: "0.1.0",
            description: "Adds a simple echo tool for extension wiring.",
            tools: ["extension.echo"],
            system_prompt:
              "Extension sky.echo is enabled. Use extension.echo to repeat text when debugging."
          }
        ]
      })
    });

    if (EXTENSION_ALLOWLIST && !EXTENSION_ALLOWLIST.includes("*")) {
      const bad = await fetchJson(policyUrl, {
        method: "POST",
        headers: { "x-liteclaw-admin-secret": ADMIN_SECRET },
        body: JSON.stringify({ enabled: ["not.allowed@0.1.0"] })
      });
      assert(
        bad.response.status === 400,
        "Expected allowlist rejection for invalid extension."
      );
    }

    if (EXTENSION_ALLOWLIST) {
      const good = await fetchJson(policyUrl, {
        method: "POST",
        headers: { "x-liteclaw-admin-secret": ADMIN_SECRET },
        body: JSON.stringify({ enabled: ["sky.echo@0.1.0"] })
      });
      assert(good.response.ok, "Failed to enable sky.echo extension.");
    } else {
      requireStep(false, "extension allowlist configured");
    }
  }

  log("Validating export JSONL...");
  const exportSummary = await validateExport();
  assert(exportSummary.headerCount === 1, "Export header missing.");

  if (REQUIRE_SKY) {
    assert(exportSummary.runCount > 0, "No Sky runs found in export.");
    assert(exportSummary.eventCount > 0, "No Sky events found in export.");
    assert(exportSummary.receiptCount > 0, "No Sky receipts found in export.");
  }

  if (exportSummary.toolReceipts.length > 0) {
    const toolNames = exportSummary.toolReceipts.map((entry) => entry.tool_name);
    assert(
      toolNames.includes("workspace.write"),
      "workspace.write receipt missing."
    );
    assert(
      toolNames.includes("workspace.read"),
      "workspace.read receipt missing."
    );
    assert(
      toolNames.includes("workspace.edit"),
      "workspace.edit receipt missing."
    );
    assert(
      toolNames.includes("http.fetch"),
      "http.fetch receipt missing."
    );

    if (EXPECT_EXECUTOR === "tunnel") {
      const hasLocal = exportSummary.toolReceipts.some(
        (entry) => entry.local_receipt && typeof entry.local_receipt === "object"
      );
      assert(hasLocal, "Expected tunnel tool receipts with local_receipt.");
    }
  } else if (ranToolSteps) {
    throw new Error(
      "No tool receipts found. Ensure tool policy is read-write and model supports tool calls."
    );
  }

  log("Smoke test completed successfully.");
};

run().catch((error) => {
  console.error("[liteclaw-smoke] Failed:", error);
  process.exit(1);
});
