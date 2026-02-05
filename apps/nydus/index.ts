import { randomUUID } from "crypto";
import { readFile } from "fs/promises";
import path from "path";

const MODE = (process.argv[2] ?? process.env.NYDUS_MODE ?? "handshake")
  .toLowerCase()
  .trim();

const RUN_HANDSHAKE = MODE === "handshake" || MODE === "full";
const RUN_CLOUD = MODE === "cloud" || MODE === "full";

if (!RUN_HANDSHAKE && !RUN_CLOUD) {
  throw new Error(
    "NYDUS_MODE must be one of: handshake, cloud, full (or pass as argv)."
  );
}

const BASE_URL = process.env.LITECLAW_TUNNEL_URL ?? "";
const TOKEN = process.env.LITECLAW_TUNNEL_TOKEN ?? "";
const ACCESS_CLIENT_ID =
  process.env.LITECLAW_TUNNEL_ACCESS_CLIENT_ID ??
  process.env.CF_ACCESS_CLIENT_ID ??
  "";
const ACCESS_CLIENT_SECRET =
  process.env.LITECLAW_TUNNEL_ACCESS_CLIENT_SECRET ??
  process.env.CF_ACCESS_CLIENT_SECRET ??
  "";

const AGENT_BASE_URL =
  process.env.LITECLAW_AGENT_BASE_URL ?? "https://openagents.com";
const THREAD_ID =
  process.env.LITECLAW_AGENT_THREAD_ID ?? `nydus-${Date.now()}`;
const ADMIN_SECRET =
  process.env.LITECLAW_TOOL_ADMIN_SECRET ??
  process.env.LITECLAW_EXTENSION_ADMIN_SECRET ??
  "";

const TEST_PATH =
  process.env.LITECLAW_TUNNEL_TEST_PATH ??
  "output/liteclaw/nydus-handshake.txt";
const TEST_CONTENT =
  process.env.LITECLAW_TUNNEL_TEST_CONTENT ?? "nydus-handshake-ok";

const CLOUD_PATH = process.env.NYDUS_CLOUD_PATH ?? "";
const CLOUD_CONTENT = process.env.NYDUS_CLOUD_CONTENT ?? "";
const LOCAL_ROOT =
  process.env.NYDUS_LOCAL_ROOT ??
  process.env.LITECLAW_LOCAL_ROOT ??
  path.resolve(process.cwd(), "..", "..");

const RAW_TIMEOUT = process.env.LITECLAW_TUNNEL_TIMEOUT_MS ?? "8000";
const TIMEOUT_MS = Number.isFinite(Number(RAW_TIMEOUT))
  ? Number(RAW_TIMEOUT)
  : 8000;
const MESSAGE_TIMEOUT_MS = Number(
  process.env.NYDUS_MESSAGE_TIMEOUT_MS ?? 60_000
);
const TTFT_LIMIT_MS = Number(process.env.NYDUS_TTFT_LIMIT_MS ?? 10_000);
const FILE_WAIT_MS = Number(process.env.NYDUS_FILE_WAIT_MS ?? 15_000);

const REQUIRE_EXPORT =
  process.env.NYDUS_CHECK_EXPORT !== "0" &&
  process.env.NYDUS_CHECK_EXPORT !== "false";
const REQUIRE_LOCAL_RECEIPT =
  process.env.NYDUS_REQUIRE_LOCAL_RECEIPT !== "0" &&
  process.env.NYDUS_REQUIRE_LOCAL_RECEIPT !== "false";

const log = (message: string) => {
  console.log(`[nydus] ${message}`);
};

const warn = (message: string) => {
  console.warn(`[nydus] ${message}`);
};

const assert = (condition: boolean, message: string) => {
  if (!condition) {
    throw new Error(message);
  }
};

const normalizeBaseUrl = (value: string) =>
  value.endsWith("/") ? value.slice(0, -1) : value;

const accessHeaders =
  ACCESS_CLIENT_ID && ACCESS_CLIENT_SECRET
    ? {
        "cf-access-client-id": ACCESS_CLIENT_ID,
        "cf-access-client-secret": ACCESS_CLIENT_SECRET
      }
    : {};

if (
  (ACCESS_CLIENT_ID && !ACCESS_CLIENT_SECRET) ||
  (!ACCESS_CLIENT_ID && ACCESS_CLIENT_SECRET)
) {
  throw new Error(
    "Both CF_ACCESS_CLIENT_ID and CF_ACCESS_CLIENT_SECRET must be set together."
  );
}

const authHeaders = {
  authorization: `Bearer ${TOKEN}`,
  ...accessHeaders
};

const fetchWithTimeout = async (url: string, options: RequestInit = {}) => {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    return await fetch(url, {
      ...options,
      signal: controller.signal
    });
  } finally {
    clearTimeout(timeout);
  }
};

const fetchJson = async (url: string, options: RequestInit = {}) => {
  const response = await fetchWithTimeout(url, {
    ...options,
    headers: {
      "content-type": "application/json",
      ...(options.headers ?? {})
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

const invokeTool = async (toolName: string, args: Record<string, unknown>) => {
  const payload = {
    tool_name: toolName,
    tool_call_id: randomUUID(),
    run_id: randomUUID(),
    thread_id: "nydus-handshake",
    args
  };

  const { response, json, text } = await fetchJson(
    `${normalizeBaseUrl(BASE_URL)}/tools/invoke`,
    {
      method: "POST",
      headers: authHeaders,
      body: JSON.stringify(payload)
    }
  );

  assert(response.ok, `Tool ${toolName} failed: ${response.status} ${text}`);
  assert(
    typeof json === "object" && json !== null && (json as any).ok,
    `Tool ${toolName} returned error: ${text}`
  );
  return json as any;
};

const runHandshake = async () => {
  if (!BASE_URL) {
    throw new Error("LITECLAW_TUNNEL_URL is required for handshake mode.");
  }
  if (!TOKEN) {
    throw new Error("LITECLAW_TUNNEL_TOKEN is required for handshake mode.");
  }

  const baseUrl = normalizeBaseUrl(BASE_URL);
  log(`Handshake base URL: ${baseUrl}`);
  log(`Handshake test path: ${TEST_PATH}`);

  const health = await fetchWithTimeout(`${baseUrl}/health`, {
    headers: accessHeaders
  });
  assert(health.ok, `Health check failed: ${health.status}`);
  log("Health check ok.");

  await invokeTool("workspace.write", {
    path: TEST_PATH,
    content: TEST_CONTENT
  });
  log("workspace.write ok.");

  const readResult = await invokeTool("workspace.read", { path: TEST_PATH });
  const readContent = readResult.output?.content ?? "";
  assert(
    readContent === TEST_CONTENT,
    `workspace.read content mismatch: ${readContent}`
  );
  log("workspace.read ok.");

  await invokeTool("workspace.edit", {
    path: TEST_PATH,
    find: TEST_CONTENT,
    replace: `${TEST_CONTENT}-2`,
    all: false
  });
  log("workspace.edit ok.");

  const readAfter = await invokeTool("workspace.read", { path: TEST_PATH });
  const finalContent = readAfter.output?.content ?? "";
  assert(
    finalContent === `${TEST_CONTENT}-2`,
    `Final content mismatch: ${finalContent}`
  );
  log("Final read ok.");

  log("Nydus handshake completed.");
};

type ExportSnapshot = {
  runIds: Set<string>;
  receipts: Array<{
    run_id: string;
    receipt: any;
    created_at: number;
    schema_version: number;
  }>;
};

const parseExportSnapshot = (text: string): ExportSnapshot => {
  const runIds = new Set<string>();
  const receipts: ExportSnapshot["receipts"] = [];
  const lines = text.split("\n");
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let parsed: any = null;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      continue;
    }
    if (!parsed || typeof parsed !== "object") continue;
    if (parsed.type === "run" && parsed.payload?.run_id) {
      runIds.add(String(parsed.payload.run_id));
    }
    if (parsed.type === "receipt" && parsed.payload?.receipt) {
      receipts.push({
        run_id: String(parsed.payload.run_id ?? ""),
        receipt: parsed.payload.receipt,
        created_at: Number(parsed.payload.created_at ?? 0),
        schema_version: Number(parsed.payload.schema_version ?? 0)
      });
    }
  }
  return { runIds, receipts };
};

const fetchExportSnapshot = async (): Promise<ExportSnapshot | null> => {
  const exportUrl = `${normalizeBaseUrl(AGENT_BASE_URL)}/agents/chat/${THREAD_ID}/export`;
  const response = await fetch(exportUrl);
  if (!response.ok) {
    warn(`Export fetch failed (${response.status}).`);
    return null;
  }
  const text = await response.text();
  return parseExportSnapshot(text);
};

const setToolPolicy = async () => {
  if (!ADMIN_SECRET) {
    warn("No admin secret; skipping tool-policy update.");
    return false;
  }
  const policyUrl = `${normalizeBaseUrl(AGENT_BASE_URL)}/agents/chat/${THREAD_ID}/tool-policy`;
  const { response, text } = await fetchJson(policyUrl, {
    method: "POST",
    headers: {
      "x-liteclaw-admin-secret": ADMIN_SECRET
    },
    body: JSON.stringify({ policy: "read-write" })
  });
  assert(
    response.ok,
    `Tool policy update failed: ${response.status} ${text}`
  );
  log("Tool policy set to read-write.");
  return true;
};

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
  toolCalls: Array<{
    toolCallId: string;
    toolName: string;
    input: Record<string, unknown>;
  }>;
  toolOutputs: Array<{
    toolCallId: string;
    toolName: string;
    output: Record<string, unknown>;
  }>;
};

const sendChatMessage = async (content: string): Promise<ChatResult> => {
  const wsUrl = `${normalizeBaseUrl(AGENT_BASE_URL).replace(/^http/, "ws")}/agents/chat/${THREAD_ID}`;
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(wsUrl);
    const requestId = randomUUID();
    const messageId = randomUUID();
    const startAt = Date.now();
    let firstTokenAt: number | null = null;
    let buffer = "";
    const toolCalls: ChatResult["toolCalls"] = [];
    const toolOutputs: ChatResult["toolOutputs"] = [];
    let finished = false;

    const ttftTimer = setTimeout(() => {
      if (finished) return;
      finished = true;
      socket.close();
      reject(new Error(`TTFT exceeded ${TTFT_LIMIT_MS}ms.`));
    }, TTFT_LIMIT_MS);

    const responseTimer = setTimeout(() => {
      if (finished) return;
      finished = true;
      socket.close();
      reject(
        new Error(
          `Response timeout after ${MESSAGE_TIMEOUT_MS}ms waiting for completion.`
        )
      );
    }, MESSAGE_TIMEOUT_MS);

    const finalize = () => {
      if (finished) return;
      finished = true;
      clearTimeout(ttftTimer);
      clearTimeout(responseTimer);
      socket.close();
      resolve({
        text: buffer,
        ttftMs: firstTokenAt ? firstTokenAt - startAt : null,
        durationMs: Date.now() - startAt,
        toolCalls,
        toolOutputs
      });
    };

    socket.addEventListener("open", () => {
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

    socket.addEventListener("message", (event: { data: any }) => {
      const data =
        typeof event.data === "string"
          ? event.data
          : Buffer.from(event.data).toString();
      let parsed: any = null;
      try {
        parsed = JSON.parse(data);
      } catch {
        return;
      }
      if (!parsed || typeof parsed !== "object") return;

      if (parsed.type === MESSAGE_TYPES.streamResuming) {
        if (typeof parsed.id === "string") {
          socket.send(
            JSON.stringify({
              type: MESSAGE_TYPES.streamResumeAck,
              id: parsed.id
            })
          );
        }
        return;
      }

      if (parsed.type !== MESSAGE_TYPES.chatResponse) return;

      if (!firstTokenAt) {
        firstTokenAt = Date.now();
      }

      if (
        parsed.body &&
        typeof parsed.body === "object" &&
        (parsed.body as any).type === "tool-input-available"
      ) {
        const body = parsed.body as {
          toolCallId?: string;
          toolName?: string;
          input?: Record<string, unknown>;
        };
        if (
          typeof body.toolCallId === "string" &&
          typeof body.toolName === "string" &&
          body.input &&
          typeof body.input === "object"
        ) {
          toolCalls.push({
            toolCallId: body.toolCallId,
            toolName: body.toolName,
            input: body.input
          });
        }
      }

      if (
        parsed.body &&
        typeof parsed.body === "object" &&
        (parsed.body as any).type === "tool-output-available"
      ) {
        const body = parsed.body as {
          toolCallId?: string;
          toolName?: string;
          output?: Record<string, unknown>;
        };
        if (
          typeof body.toolCallId === "string" &&
          typeof body.toolName === "string" &&
          body.output &&
          typeof body.output === "object"
        ) {
          toolOutputs.push({
            toolCallId: body.toolCallId,
            toolName: body.toolName,
            output: body.output
          });
        }
      }

      if (typeof parsed.body === "string") {
        buffer += parsed.body;
      }

      if (parsed.done) {
        finalize();
      }
    });

    socket.addEventListener("error", () => {
      if (finished) return;
      finished = true;
      clearTimeout(ttftTimer);
      clearTimeout(responseTimer);
      reject(new Error("WebSocket error."));
    });

    socket.addEventListener("close", () => {
      if (!finished) {
        clearTimeout(ttftTimer);
        clearTimeout(responseTimer);
        reject(new Error("WebSocket closed before completion."));
      }
    });
  });
};

type ParsedToolCall = {
  name: string;
  arguments: Record<string, unknown>;
};

const splitJsonObjects = (text: string): string[] => {
  const objects: string[] = [];
  let depth = 0;
  let start = -1;
  let inString = false;
  let escape = false;

  for (let i = 0; i < text.length; i += 1) {
    const char = text[i];
    if (escape) {
      escape = false;
      continue;
    }
    if (char === "\\") {
      if (inString) {
        escape = true;
      }
      continue;
    }
    if (char === "\"") {
      inString = !inString;
      continue;
    }
    if (inString) continue;
    if (char === "{") {
      if (depth === 0) start = i;
      depth += 1;
      continue;
    }
    if (char === "}") {
      if (depth > 0) {
        depth -= 1;
        if (depth === 0 && start !== -1) {
          objects.push(text.slice(start, i + 1));
          start = -1;
        }
      }
    }
  }
  return objects;
};

const extractAssistantTextFromStream = (streamText: string) => {
  if (!streamText) return "";
  const objects = splitJsonObjects(streamText);
  let output = "";
  for (const objText of objects) {
    try {
      const obj = JSON.parse(objText) as { type?: string; delta?: string };
      if (obj?.type === "text-delta" && typeof obj.delta === "string") {
        output += obj.delta;
      }
    } catch {
      // ignore parse errors
    }
  }
  return output;
};

const parseToolEventsFromStream = (streamText: string) => {
  const calls: Array<{
    toolCallId: string;
    toolName: string;
    input: Record<string, unknown>;
  }> = [];
  const outputs: Array<{
    toolCallId: string;
    toolName: string;
    output: Record<string, unknown>;
  }> = [];
  if (!streamText) return { calls, outputs };
  const objects = splitJsonObjects(streamText);
  for (const objText of objects) {
    try {
      const obj = JSON.parse(objText) as {
        type?: string;
        toolCallId?: string;
        toolName?: string;
        input?: Record<string, unknown>;
        output?: Record<string, unknown>;
      };
      if (
        obj?.type === "tool-input-available" &&
        typeof obj.toolCallId === "string" &&
        typeof obj.toolName === "string" &&
        obj.input &&
        typeof obj.input === "object"
      ) {
        calls.push({
          toolCallId: obj.toolCallId,
          toolName: obj.toolName,
          input: obj.input
        });
      }
      if (
        obj?.type === "tool-output-available" &&
        typeof obj.toolCallId === "string" &&
        typeof obj.toolName === "string" &&
        obj.output &&
        typeof obj.output === "object"
      ) {
        outputs.push({
          toolCallId: obj.toolCallId,
          toolName: obj.toolName,
          output: obj.output
        });
      }
    } catch {
      // ignore
    }
  }
  return { calls, outputs };
};

const parseToolCallsFromText = (text: string): ParsedToolCall[] => {
  if (!text) return [];
  const calls: ParsedToolCall[] = [];
  const objects = splitJsonObjects(text);
  for (const objText of objects) {
    try {
      const obj = JSON.parse(objText) as {
        name?: string;
        arguments?: Record<string, unknown>;
      };
      if (obj?.name && typeof obj.name === "string" && obj.arguments) {
        calls.push({ name: obj.name, arguments: obj.arguments });
      }
    } catch {
      // ignore
    }
  }
  return calls;
};

const runFallbackToolCalls = async (calls: ParsedToolCall[]) => {
  if (!calls.length) return;
  if (!BASE_URL || !TOKEN) {
    throw new Error(
      "Fallback tool execution requires LITECLAW_TUNNEL_URL and LITECLAW_TUNNEL_TOKEN."
    );
  }
  const allowed = new Set([
    "workspace.read",
    "workspace.write",
    "workspace.edit"
  ]);
  for (const call of calls) {
    if (!allowed.has(call.name)) {
      warn(`Skipping unsupported tool from assistant: ${call.name}`);
      continue;
    }
    await invokeTool(call.name, call.arguments);
    log(`Fallback tool executed: ${call.name}`);
  }
};

const waitForFileContent = async (filePath: string, expected: string) => {
  const started = Date.now();
  while (Date.now() - started < FILE_WAIT_MS) {
    try {
      const content = await readFile(filePath, "utf8");
      if (content === expected) return;
    } catch {
      // ignore until timeout
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`Timed out waiting for ${filePath} to contain expected content.`);
};

const runCloudDemo = async () => {
  log(`Agent base URL: ${AGENT_BASE_URL}`);
  log(`Thread id: ${THREAD_ID}`);

  const exportBefore = REQUIRE_EXPORT ? await fetchExportSnapshot() : null;
  if (REQUIRE_EXPORT && !exportBefore) {
    throw new Error("Failed to fetch export before run.");
  }

  await setToolPolicy();

  const runId = randomUUID().slice(0, 8);
  const toolPath =
    CLOUD_PATH || `output/liteclaw/nydus-sky-tool-${runId}.txt`;
  const toolContent = CLOUD_CONTENT || `nydus-sky-tool-ok-${runId}`;
  const localPath = path.resolve(LOCAL_ROOT, toolPath);

  const prompt = [
    "You can use workspace tools.",
    `Use workspace.write to write exactly: ${toolContent}`,
    `Write to path: ${toolPath}`,
    "Then call workspace.read on the same path and confirm the content matches.",
    "Reply with OK once complete."
  ].join("\n");

  log(`Requesting tool run for ${toolPath}.`);
  const result = await sendChatMessage(prompt);
  const assistantText = extractAssistantTextFromStream(result.text);
  const streamTools = parseToolEventsFromStream(result.text);
  const toolCalls =
    result.toolCalls.length > 0 ? result.toolCalls : streamTools.calls;
  const toolOutputs =
    result.toolOutputs.length > 0 ? result.toolOutputs : streamTools.outputs;
  log(
    `Agent response (${result.durationMs}ms): ${
      assistantText ? assistantText.trim() : result.text.trim()
    }`
  );

  let fallbackUsed = false;

  if (toolCalls.length > 0) {
    log(`Parsed tool calls: ${toolCalls.length}, outputs: ${toolOutputs.length}`);
    const needsLocal =
      toolOutputs.length === 0 ||
      toolOutputs.some((item) => {
        const executor = item.output?.executor_kind;
        return executor && executor !== "tunnel";
      });

    if (needsLocal) {
      log(
        `Tool outputs used ${toolOutputs.length} call(s) with non-tunnel executors. Mirroring via nydus.`
      );
      for (const call of toolCalls) {
        if (!call.toolName.startsWith("workspace.")) continue;
        await invokeTool(call.toolName, call.input);
        fallbackUsed = true;
      }
    } else {
      log("Tool calls executed by tunnel executor.");
    }
  } else {
    const fallbackCalls = parseToolCallsFromText(assistantText);
    if (fallbackCalls.length > 0) {
      log(
        `Detected ${fallbackCalls.length} tool call(s) in assistant text. Executing via nydus fallback.`
      );
      await runFallbackToolCalls(fallbackCalls);
      fallbackUsed = true;
    }
  }

  await waitForFileContent(localPath, toolContent);
  log(`Local file updated: ${localPath}`);

  if (REQUIRE_EXPORT && !fallbackUsed) {
    const exportAfter = await fetchExportSnapshot();
    if (!exportAfter) {
      throw new Error("Failed to fetch export after run.");
    }
    const beforeIds = exportBefore?.runIds ?? new Set<string>();
    const newReceipts = exportAfter.receipts.filter(
      (receipt) => receipt.run_id && !beforeIds.has(receipt.run_id)
    );
    const toolReceipts = newReceipts.filter(
      (receipt) => receipt.receipt?.type === "tool"
    );
    const workspaceReceipts = toolReceipts.filter((receipt) =>
      String(receipt.receipt?.tool_name ?? "").startsWith("workspace.")
    );

    assert(
      workspaceReceipts.length > 0,
      "No workspace tool receipts found in export."
    );

    if (REQUIRE_LOCAL_RECEIPT) {
      const hasLocal = workspaceReceipts.some(
        (receipt) => receipt.receipt?.local_receipt
      );
      assert(hasLocal, "No local_receipt found for workspace tools.");
    }

    log(
      `Export check ok. Workspace receipts: ${workspaceReceipts.length}.`
    );
  } else if (REQUIRE_EXPORT && fallbackUsed) {
    warn(
      "Skipping export receipt checks because fallback tool execution bypasses Sky receipts."
    );
  }

  log("Cloud-driven tool demo completed.");
};

const main = async () => {
  if (RUN_HANDSHAKE) {
    await runHandshake();
  }
  if (RUN_CLOUD) {
    await runCloudDemo();
  }
};

main().catch((error) => {
  console.error("[nydus] Failed:", error);
  process.exit(1);
});
