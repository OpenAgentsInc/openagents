import { randomUUID } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { homedir } from "node:os";
import { join } from "node:path";
import { HarnessAgent } from "@ai-sdk/harness/agent";
import { createCodex } from "@ai-sdk/harness-codex";
import { LocalAiSdkSandboxProvider } from "@openagentsinc/ai-sdk-sandbox-local";

const port = parsePort(process.env.PORT);
const workspaceRoot =
  process.env.HARNESS_WORKSPACE_ROOT ?? join(process.cwd(), ".harness-workspaces");

const sandbox = new LocalAiSdkSandboxProvider({
  accountHomes: {
    // Use the normal local Codex login rather than inheriting another
    // process's CODEX_HOME selection.
    codexHome: join(homedir(), ".codex"),
  },
  defaultPorts: [4310],
  env: {
    OPENAGENTS_CODEX_BIN: process.env.CODEX_BIN ?? "codex",
  },
  rootDirectory: workspaceRoot,
});

const agent = new HarnessAgent({
  harness: createLocalCodexHarness(),
  id: "openagents-local-codex-poc",
  instructions: "Work only in the sandbox workspace and explain the result briefly.",
  sandbox,
});

const sessions = new Map<string, Awaited<ReturnType<typeof agent.createSession>>>();

const server = createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/health") {
      return sendJson(response, 200, { status: "ok" });
    }

    if (request.method === "POST" && request.url === "/api/run") {
      const body = await readJsonBody(request);
      const prompt = requirePrompt(body.prompt);
      const sessionId = requireSessionId(body.sessionId) ?? randomUUID();
      const session = await getSession(sessionId);
      const result = await agent.generate({ prompt, session });
      return sendJson(response, 200, { sessionId, text: result.text });
    }

    return sendJson(response, 404, { error: "not_found" });
  } catch (error) {
    console.error("Codex harness request failed:", error);
    const status = error instanceof RequestError ? 400 : 502;
    const message = error instanceof RequestError ? error.message : "Codex harness failed.";
    return sendJson(response, status, { error: message });
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`AI SDK Harness POC listening on http://127.0.0.1:${port}`);
});

let stopping = false;
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

function shutdown(): void {
  if (stopping) return;
  stopping = true;
  console.log("Stopping AI SDK Harness POC...");
  void Promise.all([...sessions.values()].map((session) => session.destroy()))
    .finally(() => sandbox.destroyAllSessions())
    .catch((error: unknown) => console.error("Failed to close a Codex session:", error))
    .finally(() => server.close(() => process.exit(0)));
}

function createLocalCodexHarness() {
  const harness = createCodex({
    model: process.env.CODEX_MODEL ?? "gpt-5.6-sol",
  });
  const getBootstrap = harness.getBootstrap;
  if (getBootstrap === undefined) {
    throw new Error("The installed Codex harness does not expose a bootstrap recipe.");
  }
  return {
    ...harness,
    getBootstrap: async () => {
      const bootstrap = await getBootstrap();
      return {
        ...bootstrap,
        files: bootstrap.files.map((file) =>
          file.path.endsWith("/bridge.mjs")
            ? { ...file, content: patchBridgeForLocalCodex(file.content) }
            : file,
        ),
      };
    },
  };
}

function patchBridgeForLocalCodex(bridge: string): string {
  const needle = "const codex = new codexSdk.Codex({";
  if (!bridge.includes(needle)) {
    throw new Error("The installed Codex harness bridge no longer supports this POC patch.");
  }
  return bridge.replace(
    needle,
    'const codex = new codexSdk.Codex({ codexPathOverride: process.env.OPENAGENTS_CODEX_BIN ?? "codex",',
  );
}

async function getSession(sessionId: string) {
  const existing = sessions.get(sessionId);
  if (existing !== undefined) return existing;

  const created = await agent.createSession({ sessionId });
  sessions.set(sessionId, created);
  return created;
}

async function readJsonBody(request: IncomingMessage): Promise<Record<string, unknown>> {
  const chunks: Buffer[] = [];
  let length = 0;
  for await (const chunk of request) {
    const bytes = Buffer.from(chunk);
    length += bytes.length;
    if (length > 32 * 1024) throw new RequestError("Request body is too large.");
    chunks.push(bytes);
  }

  try {
    const value: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      throw new RequestError("Request body must be a JSON object.");
    }
    return value as Record<string, unknown>;
  } catch (error) {
    if (error instanceof RequestError) throw error;
    throw new RequestError("Request body must be valid JSON.");
  }
}

function requirePrompt(value: unknown): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new RequestError("prompt must be a non-empty string.");
  }
  if (value.length > 8_000) throw new RequestError("prompt is too long.");
  return value;
}

function requireSessionId(value: unknown): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== "string" || !/^[a-zA-Z0-9._-]{1,64}$/.test(value)) {
    throw new RequestError(
      "sessionId must use only letters, numbers, dots, underscores, or hyphens.",
    );
  }
  return value;
}

function parsePort(value: string | undefined): number {
  if (value === undefined) return 8787;
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > 65_535) {
    throw new Error("PORT must be an integer between 1 and 65535.");
  }
  return parsed;
}

function sendJson(response: ServerResponse, status: number, body: unknown): void {
  response.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  response.end(JSON.stringify(body));
}

class RequestError extends Error {}
