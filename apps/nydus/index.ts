import { randomUUID } from "crypto";

const BASE_URL = process.env.LITECLAW_TUNNEL_URL;
const TOKEN = process.env.LITECLAW_TUNNEL_TOKEN;
const ACCESS_CLIENT_ID =
  process.env.LITECLAW_TUNNEL_ACCESS_CLIENT_ID ??
  process.env.CF_ACCESS_CLIENT_ID ??
  "";
const ACCESS_CLIENT_SECRET =
  process.env.LITECLAW_TUNNEL_ACCESS_CLIENT_SECRET ??
  process.env.CF_ACCESS_CLIENT_SECRET ??
  "";
const TEST_PATH =
  process.env.LITECLAW_TUNNEL_TEST_PATH ??
  "output/liteclaw/nydus-handshake.txt";
const TEST_CONTENT =
  process.env.LITECLAW_TUNNEL_TEST_CONTENT ?? "nydus-handshake-ok";
const RAW_TIMEOUT = process.env.LITECLAW_TUNNEL_TIMEOUT_MS ?? "8000";
const TIMEOUT_MS = Number.isFinite(Number(RAW_TIMEOUT))
  ? Number(RAW_TIMEOUT)
  : 8000;

const log = (message: string) => {
  console.log(`[nydus] ${message}`);
};

const assert = (condition: boolean, message: string) => {
  if (!condition) {
    throw new Error(message);
  }
};

if (!BASE_URL) {
  throw new Error("LITECLAW_TUNNEL_URL is required.");
}
if (!TOKEN) {
  throw new Error("LITECLAW_TUNNEL_TOKEN is required.");
}
if ((ACCESS_CLIENT_ID && !ACCESS_CLIENT_SECRET) ||
    (!ACCESS_CLIENT_ID && ACCESS_CLIENT_SECRET)) {
  throw new Error(
    "Both CF_ACCESS_CLIENT_ID and CF_ACCESS_CLIENT_SECRET must be set together."
  );
}

const normalizeBaseUrl = (value: string) =>
  value.endsWith("/") ? value.slice(0, -1) : value;
const baseUrl = normalizeBaseUrl(BASE_URL);

const accessHeaders =
  ACCESS_CLIENT_ID && ACCESS_CLIENT_SECRET
    ? {
        "cf-access-client-id": ACCESS_CLIENT_ID,
        "cf-access-client-secret": ACCESS_CLIENT_SECRET
      }
    : {};

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
    `${baseUrl}/tools/invoke`,
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

const run = async () => {
  log(`Base URL: ${baseUrl}`);
  log(`Test path: ${TEST_PATH}`);

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

run().catch((error) => {
  console.error("[nydus] Failed:", error);
  process.exit(1);
});
