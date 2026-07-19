#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

import { query } from "@anthropic-ai/claude-agent-sdk";
import { Codex } from "@openai/codex-sdk";

const [requestPath, statePath] = process.argv.slice(2);
if (!requestPath || !statePath) process.exit(2);

let request;
let state;
try {
  request = JSON.parse(readFileSync(requestPath, "utf8"));
  state = JSON.parse(readFileSync(statePath, "utf8"));
  rmSync(requestPath, { force: true });
} catch {
  process.exit(2);
}

const digest = (value) => createHash("sha256").update(value).digest("hex");
if (`sha256:${digest(request.prompt)}` !== request.promptDigest) process.exit(2);
const turnKey = digest(request.turnRef).slice(0, 24);
const runtimeHome = `/run/openagents-managed-sandbox/${turnKey}`;
const workspace = "/workspace";
mkdirSync(runtimeHome, { recursive: true, mode: 0o700 });
mkdirSync(workspace, { recursive: true, mode: 0o700 });

const writeState = () => {
  const temporary = `${statePath}.tmp-${process.pid}`;
  writeFileSync(temporary, JSON.stringify(state), { mode: 0o600 });
  renameSync(temporary, statePath);
};
const emit = (event) => {
  const next = {
    ...event,
    turnRef: request.turnRef,
    resourceGeneration: request.expectedResourceGeneration,
    turnEventSequence: state.events.length + 1,
    observedAt: new Date().toISOString(),
  };
  state.events.push(next);
  writeState();
};
const usageRef = (usage) =>
  `provider.usage.sha256.${digest(`${request.turnRef}|${JSON.stringify(usage)}`)}`;
const runtimeUsage = (usage) => ({
  inputTokens: Number(usage?.input_tokens ?? usage?.inputTokens ?? 0),
  outputTokens: Number(usage?.output_tokens ?? usage?.outputTokens ?? 0),
  ...(Number.isFinite(Number(usage?.cached_input_tokens))
    ? { cachedInputTokens: Number(usage.cached_input_tokens) }
    : {}),
  providerUsageRef: usageRef(usage),
  exact: true,
});

const runCodex = async () => {
  const codex = new Codex({
    apiKey: request.providerCapabilityToken,
    baseUrl: `${request.providerBaseUrl}/openai/v1`,
    env: {
      HOME: runtimeHome,
      PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin",
    },
  });
  const thread = codex.startThread({
    model: request.providerModel,
    workingDirectory: workspace,
    skipGitRepoCheck: true,
    sandboxMode: "workspace-write",
    approvalPolicy: "never",
    networkAccessEnabled: false,
  });
  const streamed = await thread.runStreamed(request.prompt);
  for await (const event of streamed.events) {
    if (event.type === "item.completed" && event.item.type === "agent_message" && event.item.text) {
      emit({ _tag: "RuntimeTextDelta", content: event.item.text });
    } else if (event.type === "item.started" && event.item.type === "command_execution") {
      emit({
        _tag: "RuntimeToolStarted",
        toolCallRef: `tool.${event.item.id}`,
        toolName: "command_execution",
      });
    } else if (event.type === "item.completed" && event.item.type === "command_execution") {
      emit({
        _tag: "RuntimeToolCompleted",
        toolCallRef: `tool.${event.item.id}`,
        toolName: "command_execution",
        outcome: event.item.status === "completed" ? "succeeded" : "failed",
        evidenceRefs: [],
      });
    } else if (event.type === "turn.completed") {
      const usage = runtimeUsage(event.usage);
      emit({ _tag: "RuntimeUsageRecorded", usage });
      emit({ _tag: "RuntimeSettled", finishReason: "structural_completion", usage });
    } else if (event.type === "turn.failed" || event.type === "error") {
      emit({
        _tag: "RuntimeFailed",
        errorRef: `provider.failure.sha256.${digest(JSON.stringify(event))}`,
        retryable: false,
      });
    }
  }
};

const runClaude = async () => {
  let settled = false;
  const session = query({
    prompt: request.prompt,
    options: {
      cwd: workspace,
      model: request.providerModel,
      maxTurns: 1,
      tools: [],
      env: {
        HOME: runtimeHome,
        PATH: process.env.PATH ?? "/usr/local/bin:/usr/bin:/bin",
        ANTHROPIC_BASE_URL: `${request.providerBaseUrl}/anthropic`,
        ANTHROPIC_API_KEY: request.providerCapabilityToken,
        CLAUDE_AGENT_SDK_CLIENT_APP: "openagents-managed-sandbox/1",
      },
    },
  });
  for await (const message of session) {
    if (message.type === "assistant") {
      const content = Array.isArray(message.message?.content)
        ? message.message.content
            .filter((block) => block?.type === "text" && typeof block.text === "string")
            .map((block) => block.text)
            .join("")
        : "";
      if (content) emit({ _tag: "RuntimeTextDelta", content });
    } else if (message.type === "result") {
      if (message.subtype === "success") {
        const usage = runtimeUsage(message.usage);
        emit({ _tag: "RuntimeUsageRecorded", usage });
        emit({ _tag: "RuntimeSettled", finishReason: "structural_completion", usage });
      } else {
        emit({
          _tag: "RuntimeFailed",
          errorRef: `provider.failure.sha256.${digest(JSON.stringify(message.subtype))}`,
          retryable: false,
        });
      }
      settled = true;
    }
  }
  if (!settled) throw new Error("claude_stream_ended_without_result");
};

try {
  if (request.runtime.provider === "codex") await runCodex();
  else if (request.runtime.provider === "claude") await runClaude();
  else throw new Error("provider_not_admitted");
} catch (error) {
  const last = state.events.at(-1);
  if (!["RuntimeSettled", "RuntimeFailed", "RuntimeInterrupted"].includes(last?._tag)) {
    emit({
      _tag: "RuntimeFailed",
      errorRef: `provider.failure.sha256.${digest(error instanceof Error ? error.name : "unknown")}`,
      retryable: true,
    });
  }
} finally {
  rmSync(runtimeHome, { recursive: true, force: true });
}
