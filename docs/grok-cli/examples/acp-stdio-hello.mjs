#!/usr/bin/env node
/**
 * Minimal Grok ACP (stdio) client.
 *
 * Upstream pattern: https://docs.x.ai/build/cli/headless-scripting#acp
 *
 * Requires either:
 *   - prior `grok login` (cached_token), or
 *   - XAI_API_KEY set (xai.api_key)
 *
 * Usage:
 *   node docs/grok-cli/examples/acp-stdio-hello.mjs
 *   node docs/grok-cli/examples/acp-stdio-hello.mjs "Say hi in five words"
 */

import { spawn } from "node:child_process";
import readline from "node:readline";
import process from "node:process";

const userText = process.argv.slice(2).join(" ") || "Say hello in one short sentence.";

const proc = spawn("grok", ["agent", "stdio"], { stdio: ["pipe", "pipe", "pipe"] });
const rl = readline.createInterface({ input: proc.stdout });
const pending = new Map();
let nextId = 1;
let text = "";

proc.stderr.on("data", (chunk) => process.stderr.write(chunk));

rl.on("line", (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return;
  }

  if (message.method === "session/update") {
    const update = message.params?.update;
    if (update?.sessionUpdate === "agent_message_chunk" && update.content?.text) {
      text += update.content.text;
    }
    return;
  }

  const pendingRequest = pending.get(message.id);
  if (!pendingRequest) return;

  pending.delete(message.id);
  if (message.error) {
    pendingRequest.reject(
      new Error(message.error.message ?? JSON.stringify(message.error)),
    );
  } else {
    pendingRequest.resolve(message.result ?? {});
  }
});

function request(method, params, timeoutMs = 60_000) {
  const id = nextId++;

  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`${method} timed out`));
    }, timeoutMs);

    pending.set(id, {
      resolve(result) {
        clearTimeout(timer);
        resolve(result);
      },
      reject(error) {
        clearTimeout(timer);
        reject(error);
      },
    });

    proc.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

try {
  const init = await request("initialize", {
    protocolVersion: 1,
    clientCapabilities: {
      fs: { readTextFile: true, writeTextFile: true },
      terminal: true,
    },
  });

  const authMethods = new Set((init.authMethods ?? []).map((method) => method.id));
  const methodId =
    process.env.XAI_API_KEY && authMethods.has("xai.api_key")
      ? "xai.api_key"
      : authMethods.has("cached_token")
        ? "cached_token"
        : null;

  if (!methodId) {
    throw new Error("Run `grok login` first, or set XAI_API_KEY.");
  }

  await request("authenticate", { methodId, _meta: { headless: true } });

  const { sessionId } = await request("session/new", {
    cwd: process.cwd(),
    mcpServers: [],
  });

  const prompt = await request("session/prompt", {
    sessionId,
    prompt: [{ type: "text", text: userText }],
  });

  // Text streams via session/update; wait until it stabilizes briefly.
  let lastLength = -1;
  let stableChecks = 0;
  while (stableChecks < 2) {
    await sleep(150);
    if (text.length === lastLength) {
      stableChecks += 1;
    } else {
      lastLength = text.length;
      stableChecks = 0;
    }
  }

  console.log(text.trim() || `No text returned (stopReason=${prompt.stopReason})`);
} finally {
  rl.close();
  proc.kill();
}
