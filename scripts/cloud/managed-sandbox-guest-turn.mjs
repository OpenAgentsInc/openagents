#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
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
const artifactPath = `${workspace}/forensic-artifact.tar.zst`;
mkdirSync(runtimeHome, { recursive: true, mode: 0o700 });
mkdirSync(workspace, { recursive: true, mode: 0o700 });

const writeState = () => {
  const temporary = `${statePath}.tmp-${process.pid}`;
  writeFileSync(temporary, JSON.stringify(state), { mode: 0o600 });
  renameSync(temporary, statePath);
};
const terminalEventTags = new Set(["RuntimeSettled", "RuntimeFailed", "RuntimeInterrupted"]);
let terminalEventTag = state.events.findLast((event) => terminalEventTags.has(event?._tag))?._tag;
const emit = (event) => {
  // Provider streams are not lifecycle authority. Some SDK transports can
  // yield a late error after a completed turn; the first terminal event is
  // authoritative and the guest must never persist a second terminal (or any
  // trailing event) for the same generation-fenced turn.
  if (terminalEventTag !== undefined) return false;
  const next = {
    ...event,
    turnRef: request.turnRef,
    resourceGeneration: request.expectedResourceGeneration,
    turnEventSequence: state.events.length + 1,
    observedAt: new Date().toISOString(),
  };
  state.events.push(next);
  if (terminalEventTags.has(next._tag)) terminalEventTag = next._tag;
  writeState();
  return true;
};
const usageRef = (usage) =>
  `provider.usage.sha256.${digest(`${request.turnRef}|${JSON.stringify(usage)}`)}`;
const runtimeUsage = (usage) => {
  const inputTokens = usage?.input_tokens ?? usage?.inputTokens;
  const outputTokens = usage?.output_tokens ?? usage?.outputTokens;
  if (
    typeof inputTokens !== "number" ||
    !Number.isSafeInteger(inputTokens) ||
    inputTokens < 0 ||
    typeof outputTokens !== "number" ||
    !Number.isSafeInteger(outputTokens) ||
    outputTokens < 0
  ) {
    throw new Error("provider_usage_unavailable");
  }
  return {
    inputTokens,
    outputTokens,
    ...(Number.isSafeInteger(Number(usage?.cached_input_tokens)) &&
    Number(usage.cached_input_tokens) >= 0
      ? { cachedInputTokens: Number(usage.cached_input_tokens) }
      : {}),
    providerUsageRef: usageRef(usage),
    exact: true,
  };
};

const networkBytes = () => {
  let bytes = 0;
  let observed = false;
  for (const interfaceName of readdirSync("/sys/class/net")) {
    if (interfaceName === "lo") continue;
    observed = true;
    for (const counter of ["rx_bytes", "tx_bytes"]) {
      const value = Number.parseInt(
        readFileSync(`/sys/class/net/${interfaceName}/statistics/${counter}`, "utf8"),
        10,
      );
      if (!Number.isSafeInteger(value) || value < 0) throw new Error("network_usage_invalid");
      bytes += value;
      if (!Number.isSafeInteger(bytes)) throw new Error("network_usage_invalid");
    }
  }
  if (!observed) throw new Error("network_usage_unavailable");
  return bytes;
};

const artifactBytes = () => {
  if (!existsSync(artifactPath)) return 0;
  const status = lstatSync(artifactPath);
  if (!status.isFile() || !Number.isSafeInteger(status.size)) {
    throw new Error("artifact_usage_invalid");
  }
  return status.size;
};

const guardrails = request.guardrails;
const guarded = request.runtime?.harnessRef === "driver.openagents.forensic-worker.v1";
if (
  guarded &&
  (guardrails?.sandboxRef !== request.sandboxRef ||
    guardrails?.resourceGeneration !== request.expectedResourceGeneration ||
    !Number.isSafeInteger(guardrails?.remainingTokens) ||
    guardrails.remainingTokens < 1 ||
    !Number.isSafeInteger(guardrails?.remainingCostMicros) ||
    guardrails.remainingCostMicros < 1 ||
    !Number.isSafeInteger(guardrails?.networkBytesObserved) ||
    !Number.isSafeInteger(guardrails?.remainingNetworkBytes) ||
    !Number.isSafeInteger(guardrails?.artifactBytesObserved) ||
    !Number.isSafeInteger(guardrails?.remainingArtifactBytes) ||
    Date.parse(guardrails?.deadlineAt) <= Date.now())
) {
  process.exit(2);
}
const abortController = new AbortController();
let budgetExceeded = false;
const enforceBudget = () => {
  if (!guarded) return;
  try {
    if (
      Date.now() >= Date.parse(guardrails.deadlineAt) ||
      networkBytes() > guardrails.networkBytesObserved + guardrails.remainingNetworkBytes ||
      artifactBytes() > guardrails.artifactBytesObserved + guardrails.remainingArtifactBytes
    ) {
      budgetExceeded = true;
      abortController.abort();
    }
  } catch {
    budgetExceeded = true;
    abortController.abort();
  }
};
const deadlineTimer = guarded
  ? setTimeout(enforceBudget, Math.max(0, Date.parse(guardrails.deadlineAt) - Date.now()))
  : undefined;
const budgetTimer = guarded ? setInterval(enforceBudget, 100) : undefined;
enforceBudget();

const runCodex = async () => {
  let settled = false;
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
  const streamed = await thread.runStreamed(request.prompt, { signal: abortController.signal });
  for await (const event of streamed.events) {
    if (settled) continue;
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
      settled = true;
    } else if (event.type === "turn.failed") {
      emit({
        _tag: "RuntimeFailed",
        errorRef: `provider.failure.sha256.${digest(JSON.stringify(event))}`,
        retryable: false,
      });
      settled = true;
    }
  }
  if (!settled) throw new Error("codex_stream_ended_without_result");
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
      abortController,
    },
  });
  for await (const message of session) {
    if (settled) continue;
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
  if (terminalEventTag === undefined) {
    if (budgetExceeded) emit({ _tag: "RuntimeSettled", finishReason: "budget_guardrail" });
    else
      emit({
        _tag: "RuntimeFailed",
        errorRef: `provider.failure.sha256.${digest(error instanceof Error ? error.name : "unknown")}`,
        retryable: true,
      });
  }
} finally {
  if (deadlineTimer !== undefined) clearTimeout(deadlineTimer);
  if (budgetTimer !== undefined) clearInterval(budgetTimer);
  rmSync(runtimeHome, { recursive: true, force: true });
}
