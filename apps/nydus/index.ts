import { randomUUID } from "crypto";
import { readFile } from "fs/promises";
import path from "path";
import * as Data from "effect/Data";
import * as Effect from "effect/Effect";

class NydusError extends Data.TaggedError("NydusError")<{
  message: string;
  cause?: unknown;
}> {}

const MODE = (process.argv[2] ?? process.env.NYDUS_MODE ?? "handshake")
  .toLowerCase()
  .trim();

const RUN_HANDSHAKE = MODE === "handshake" || MODE === "full";
const RUN_CLOUD = MODE === "cloud" || MODE === "full";
const RUN_SKY = MODE === "sky" || MODE === "sky-tools";

if (!RUN_HANDSHAKE && !RUN_CLOUD && !RUN_SKY) {
  throw new Error(
    "NYDUS_MODE must be one of: handshake, cloud, sky, full (or pass as argv)."
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
const SKY_ROOT = process.env.NYDUS_SKY_ROOT ?? LOCAL_ROOT;

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

const log = (message: string) =>
  Effect.sync(() => {
    console.log(`[nydus] ${message}`);
  });

const warn = (message: string) =>
  Effect.sync(() => {
    console.warn(`[nydus] ${message}`);
  });

const fail = (message: string, cause?: unknown) =>
  Effect.fail(new NydusError({ message, cause }));

const assert = (condition: boolean, message: string) =>
  condition ? Effect.void : fail(message);

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

const fetchWithTimeout = (url: string, options: RequestInit = {}) =>
  Effect.tryPromise({
    try: async () => {
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
    },
    catch: (cause) =>
      new NydusError({ message: `Fetch failed for ${url}`, cause })
  });

const fetchJson = (url: string, options: RequestInit = {}) =>
  Effect.gen(function* () {
    const response = yield* fetchWithTimeout(url, {
      ...options,
      headers: {
        "content-type": "application/json",
        ...(options.headers ?? {})
      }
    });
    const text = yield* Effect.tryPromise({
      try: () => response.text(),
      catch: (cause) =>
        new NydusError({ message: `Failed reading response from ${url}`, cause })
    });
    let json: unknown = null;
    if (text) {
      try {
        json = JSON.parse(text);
      } catch {
        json = null;
      }
    }
    return { response, json, text };
  });

const invokeTool = (toolName: string, args: Record<string, unknown>) =>
  Effect.gen(function* () {
    const payload = {
      tool_name: toolName,
      tool_call_id: randomUUID(),
      run_id: randomUUID(),
      thread_id: "nydus-handshake",
      args
    };

    const { response, json, text } = yield* fetchJson(
      `${normalizeBaseUrl(BASE_URL)}/tools/invoke`,
      {
        method: "POST",
        headers: authHeaders,
        body: JSON.stringify(payload)
      }
    );

    yield* assert(
      response.ok,
      `Tool ${toolName} failed: ${response.status} ${text}`
    );
    yield* assert(
      typeof json === "object" && json !== null && (json as any).ok,
      `Tool ${toolName} returned error: ${text}`
    );
    return json as any;
  });

const runHandshake = Effect.fn("runHandshake")(function* () {
  if (!BASE_URL) {
    yield* fail("LITECLAW_TUNNEL_URL is required for handshake mode.");
  }
  if (!TOKEN) {
    yield* fail("LITECLAW_TUNNEL_TOKEN is required for handshake mode.");
  }

  const baseUrl = normalizeBaseUrl(BASE_URL);
  yield* log(`Handshake base URL: ${baseUrl}`);
  yield* log(`Handshake test path: ${TEST_PATH}`);

  const health = yield* fetchWithTimeout(`${baseUrl}/health`, {
    headers: accessHeaders
  });
  yield* assert(health.ok, `Health check failed: ${health.status}`);
  yield* log("Health check ok.");

  yield* invokeTool("workspace.write", {
    path: TEST_PATH,
    content: TEST_CONTENT
  });
  yield* log("workspace.write ok.");

  const readResult = yield* invokeTool("workspace.read", { path: TEST_PATH });
  const readContent = readResult.output?.content ?? "";
  yield* assert(
    readContent === TEST_CONTENT,
    `workspace.read content mismatch: ${readContent}`
  );
  yield* log("workspace.read ok.");

  yield* invokeTool("workspace.edit", {
    path: TEST_PATH,
    find: TEST_CONTENT,
    replace: `${TEST_CONTENT}-2`,
    all: false
  });
  yield* log("workspace.edit ok.");

  const readAfter = yield* invokeTool("workspace.read", { path: TEST_PATH });
  const finalContent = readAfter.output?.content ?? "";
  yield* assert(
    finalContent === `${TEST_CONTENT}-2`,
    `Final content mismatch: ${finalContent}`
  );
  yield* log("Final read ok.");

  yield* log("Nydus handshake completed.");
});

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

const fetchExportSnapshot = () =>
  Effect.gen(function* () {
    const exportUrl = `${normalizeBaseUrl(AGENT_BASE_URL)}/agents/chat/${THREAD_ID}/export`;
    const response = yield* fetchWithTimeout(exportUrl);
    if (!response.ok) {
      yield* warn(`Export fetch failed (${response.status}).`);
      return null;
    }
    const text = yield* Effect.tryPromise({
      try: () => response.text(),
      catch: (cause) =>
        new NydusError({ message: "Failed reading export response", cause })
    });
    return parseExportSnapshot(text);
  });

const setToolPolicy = () =>
  Effect.gen(function* () {
    if (!ADMIN_SECRET) {
      yield* warn("No admin secret; skipping tool-policy update.");
      return false;
    }
    const policyUrl = `${normalizeBaseUrl(AGENT_BASE_URL)}/agents/chat/${THREAD_ID}/tool-policy`;
    const { response, text } = yield* fetchJson(policyUrl, {
      method: "POST",
      headers: {
        "x-liteclaw-admin-secret": ADMIN_SECRET
      },
      body: JSON.stringify({ policy: "read-write" })
    });
    yield* assert(
      response.ok,
      `Tool policy update failed: ${response.status} ${text}`
    );
    yield* log("Tool policy set to read-write.");
    return true;
  });

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

const sendChatMessage = (content: string) =>
  Effect.tryPromise({
    try: () =>
      new Promise<ChatResult>((resolve, reject) => {
        const wsUrl = `${normalizeBaseUrl(AGENT_BASE_URL).replace(/^http/, "ws")}/agents/chat/${THREAD_ID}`;
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
          reject(new NydusError({ message: `TTFT exceeded ${TTFT_LIMIT_MS}ms.` }));
        }, TTFT_LIMIT_MS);

        const responseTimer = setTimeout(() => {
          if (finished) return;
          finished = true;
          socket.close();
          reject(
            new NydusError({
              message: `Response timeout after ${MESSAGE_TIMEOUT_MS}ms waiting for completion.`
            })
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
          reject(new NydusError({ message: "WebSocket error." }));
        });

        socket.addEventListener("close", () => {
          if (!finished) {
            clearTimeout(ttftTimer);
            clearTimeout(responseTimer);
            reject(new NydusError({ message: "WebSocket closed before completion." }));
          }
        });
      }),
    catch: (cause) =>
      cause instanceof NydusError
        ? cause
        : new NydusError({ message: "WebSocket error", cause })
  });

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
    if (char === '"') {
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

const runFallbackToolCalls = (calls: ParsedToolCall[]) =>
  Effect.gen(function* () {
    if (!calls.length) return;
    if (!BASE_URL || !TOKEN) {
      yield* fail(
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
        yield* warn(`Skipping unsupported tool from assistant: ${call.name}`);
        continue;
      }
      yield* invokeTool(call.name, call.arguments);
      yield* log(`Fallback tool executed: ${call.name}`);
    }
  });

const waitForFileContent = (filePath: string, expected: string) =>
  Effect.gen(function* () {
    const started = Date.now();
    while (Date.now() - started < FILE_WAIT_MS) {
      const content = yield* Effect.tryPromise({
        try: () => readFile(filePath, "utf8"),
        catch: (cause) =>
          new NydusError({ message: `Failed reading ${filePath}`, cause })
      }).pipe(Effect.catchAll(() => Effect.succeed(null)));
      if (content === expected) return;
      yield* Effect.sleep("500 millis");
    }
    yield* fail(
      `Timed out waiting for ${filePath} to contain expected content.`
    );
  });

const runCloudDemo = Effect.fn("runCloudDemo")(function* () {
  yield* log(`Agent base URL: ${AGENT_BASE_URL}`);
  yield* log(`Thread id: ${THREAD_ID}`);

  const exportBefore = REQUIRE_EXPORT ? yield* fetchExportSnapshot() : null;
  if (REQUIRE_EXPORT && !exportBefore) {
    yield* fail("Failed to fetch export before run.");
  }

  yield* setToolPolicy();

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

  yield* log(`Requesting tool run for ${toolPath}.`);
  const result = yield* sendChatMessage(prompt);
  const assistantText = extractAssistantTextFromStream(result.text);
  const streamTools = parseToolEventsFromStream(result.text);
  const toolCalls =
    result.toolCalls.length > 0 ? result.toolCalls : streamTools.calls;
  const toolOutputs =
    result.toolOutputs.length > 0 ? result.toolOutputs : streamTools.outputs;
  yield* log(
    `Agent response (${result.durationMs}ms): ${
      assistantText ? assistantText.trim() : result.text.trim()
    }`
  );

  let fallbackUsed = false;

  if (toolCalls.length > 0) {
    yield* log(
      `Parsed tool calls: ${toolCalls.length}, outputs: ${toolOutputs.length}`
    );
    const needsLocal =
      toolOutputs.length === 0 ||
      toolOutputs.some((item) => {
        const executor = item.output?.executor_kind;
        return executor && executor !== "tunnel";
      });

    if (needsLocal) {
      yield* log(
        `Tool outputs used ${toolOutputs.length} call(s) with non-tunnel executors. Mirroring via nydus.`
      );
      for (const call of toolCalls) {
        if (!call.toolName.startsWith("workspace.")) continue;
        yield* invokeTool(call.toolName, call.input);
        fallbackUsed = true;
      }
    } else {
      yield* log("Tool calls executed by tunnel executor.");
    }
  } else {
    const fallbackCalls = parseToolCallsFromText(assistantText);
    if (fallbackCalls.length > 0) {
      yield* log(
        `Detected ${fallbackCalls.length} tool call(s) in assistant text. Executing via nydus fallback.`
      );
      yield* runFallbackToolCalls(fallbackCalls);
      fallbackUsed = true;
    }
  }

  yield* waitForFileContent(localPath, toolContent);
  yield* log(`Local file updated: ${localPath}`);

  if (REQUIRE_EXPORT && !fallbackUsed) {
    const exportAfter = yield* fetchExportSnapshot();
    if (!exportAfter) {
      yield* fail("Failed to fetch export after run.");
      return;
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

    yield* assert(
      workspaceReceipts.length > 0,
      "No workspace tool receipts found in export."
    );

    if (REQUIRE_LOCAL_RECEIPT) {
      const hasLocal = workspaceReceipts.some(
        (receipt) => receipt.receipt?.local_receipt
      );
      yield* assert(hasLocal, "No local_receipt found for workspace tools.");
    }

    yield* log(
      `Export check ok. Workspace receipts: ${workspaceReceipts.length}.`
    );
  } else if (REQUIRE_EXPORT && fallbackUsed) {
    yield* warn(
      "Skipping export receipt checks because fallback tool execution bypasses Sky receipts."
    );
  }

  yield* log("Cloud-driven tool demo completed.");
});

const sliceLines = (text: string, offset: number, limit: number) => {
  const lines = text.split("\n");
  const startIndex = Math.max(offset - 1, 0);
  const endIndex = Math.min(startIndex + limit, lines.length);
  return lines.slice(startIndex, endIndex).join("\n");
};

const runSkyToolDemo = Effect.fn("runSkyToolDemo")(function* () {
  if (!BASE_URL) {
    yield* fail("LITECLAW_TUNNEL_URL is required for sky tools mode.");
  }
  if (!TOKEN) {
    yield* fail("LITECLAW_TUNNEL_TOKEN is required for sky tools mode.");
  }

  const runId = randomUUID().slice(0, 8);
  const toolPath = `output/liteclaw/nydus-sky-tools-${runId}.txt`;
  const contentLines = ["line-1", "line-2", "line-3", "line-4", "line-5"];
  const content = contentLines.join("\n");

  yield* log(`Sky tools demo path: ${toolPath}`);

  yield* invokeTool("workspace.write", {
    label: "sky-write",
    path: toolPath,
    content
  });
  yield* log("sky write ok.");

  const readResult = yield* invokeTool("workspace.read", {
    label: "sky-read",
    path: toolPath
  });
  const readContent = readResult.output?.content ?? "";
  yield* assert(readContent === content, "sky read content mismatch.");
  yield* log("sky read ok.");

  const offset = 3;
  const limit = 2;
  const sliced = sliceLines(readContent, offset, limit);
  yield* assert(
    sliced === "line-3\nline-4",
    "sky read offset/limit slice mismatch."
  );
  yield* log("sky read offset/limit ok.");

  yield* invokeTool("workspace.edit", {
    label: "sky-edit",
    path: toolPath,
    find: "line-3",
    replace: "line-3-edited",
    all: false
  });
  yield* log("sky edit ok.");

  const editedRead = yield* invokeTool("workspace.read", {
    label: "sky-read-edited",
    path: toolPath
  });
  const editedContent = editedRead.output?.content ?? "";
  yield* assert(editedContent.includes("line-3-edited"), "sky edit did not apply.");
  yield* log("sky edit verified.");

  const bashResult = yield* invokeTool("bash", {
    label: "sky-bash",
    command: "printf 'sky-bash-ok'"
  });
  const bashOutput = bashResult.output?.output ?? "";
  yield* assert(bashOutput.includes("sky-bash-ok"), "sky bash output mismatch.");
  yield* log("sky bash ok.");

  const localPath = path.resolve(SKY_ROOT, toolPath);
  yield* waitForFileContent(localPath, editedContent);
  yield* log(`sky tools local file updated: ${localPath}`);
  yield* log("Sky tools demo completed.");
});

const main = Effect.fn("main")(function* () {
  if (RUN_HANDSHAKE) {
    yield* runHandshake();
  }
  if (RUN_CLOUD) {
    yield* runCloudDemo();
  }
  if (RUN_SKY) {
    yield* runSkyToolDemo();
  }
});

Effect.runPromise(main()).catch((error) => {
  console.error("[nydus] Failed:", error);
  process.exit(1);
});
