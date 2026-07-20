import { describe, expect, test } from "vite-plus/test";
import { Schema as S } from "effect";

import {
  SAFE_TURN_PROJECTION_SCHEMA_LITERAL,
  SafeTurnProjection,
  type AgentCardState,
} from "@openagentsinc/agent-runtime-schema";

import {
  buildSafeMessageChainEntry,
  isLiveCardState,
  isTerminalCardState,
  makeSafeCardProjector,
  projectSafeAgentCard,
  projectSafeMessageChain,
  safeMessageChainOf,
  type ObservedAgentActivity,
} from "./index.js";

const decodeProjection = S.decodeUnknownSync(SafeTurnProjection);

const baseProjection = (cardState: AgentCardState) =>
  decodeProjection({
    schema: SAFE_TURN_PROJECTION_SCHEMA_LITERAL,
    threadRef: "thread.1",
    requestRef: "request.codex.1",
    providerTurnRef: "providerturn.codex.1",
    cardState,
    candidate: "codex",
    dataDestination: "on_device_local",
    usageTruth: "unknown",
    localOnly: true,
    updatedAt: "2026-07-20T00:00:00Z",
    messageChain: [
      { entryRef: "request.codex.1.chain.0", role: "assistant", text: "Working on it." },
      { entryRef: "request.codex.1.chain.1", role: "tool", text: "", toolLabel: "shell", commandOutputByteCount: 42 },
    ],
    evidenceRefs: [],
  });

describe("SafeCardProjector", () => {
  test("derives the bounded card facts and never invents a running state", () => {
    const projector = makeSafeCardProjector();
    const states: ReadonlyArray<AgentCardState> = [
      "queued",
      "running",
      "done",
      "refused",
      "failed",
      "cancelled",
    ];
    for (const state of states) {
      const projected = projector.project(baseProjection(state));
      expect(projected.cardState).toBe(state);
      expect(projected.stage).toBe("card");
    }
  });

  test("the card is only ever a `card` stage, never an action or a release", () => {
    const projector = makeSafeCardProjector();
    expect(projector.project(baseProjection("running")).stage).toBe("card");
  });
});

describe("projectSafeAgentCard", () => {
  test("carries the safe route disclosure and the message count only", () => {
    const card = projectSafeAgentCard(baseProjection("running"));
    expect(card).toEqual({
      requestRef: "request.codex.1",
      threadRef: "thread.1",
      providerTurnRef: "providerturn.codex.1",
      cardState: "running",
      stage: "card",
      provider: "codex",
      dataDestination: "on_device_local",
      usageTruth: "unknown",
      localOnly: true,
      updatedAt: "2026-07-20T00:00:00Z",
      messageCount: 2,
    });
  });
});

describe("card-state helpers", () => {
  test("terminal and live states partition the vocabulary", () => {
    expect(isLiveCardState("queued")).toBe(true);
    expect(isLiveCardState("running")).toBe(true);
    expect(isTerminalCardState("done")).toBe(true);
    expect(isTerminalCardState("refused")).toBe(true);
    expect(isTerminalCardState("failed")).toBe(true);
    expect(isTerminalCardState("cancelled")).toBe(true);
    expect(isLiveCardState("done")).toBe(false);
    expect(isTerminalCardState("running")).toBe(false);
  });
});

describe("safe message-chain redaction boundary", () => {
  test("reads ONLY the named safe fields; raw args/output/paths/tokens never appear", () => {
    // Every field in `unsafeExtras` is UNSAFE and must never reach the
    // projection. A spread is exempt from excess-property checks, so no cast is
    // needed to prove the projector structurally ignores unnamed fields.
    const unsafeExtras: Record<string, unknown> = {
      rawCommand: "git push --force origin main",
      rawOutput: "fatal: secret token sk-live-DEADBEEF leaked to /Users/owner/.ssh/id_rsa",
      localPath: "/Users/owner/work/openagents/.secrets/tailnet.env",
      token: "sk-live-DEADBEEF",
      apiKey: "AKIAIOSFODNN7EXAMPLE",
    };
    const raw: ObservedAgentActivity = {
      role: "tool",
      text: "Ran the build.",
      toolLabel: "shell",
      fileChangeCount: 3,
      commandOutputByteCount: 1024,
      ...unsafeExtras,
    };

    const entry = buildSafeMessageChainEntry("request.codex.1", 0, raw);
    const serialized = JSON.stringify(entry);

    // The safe fields survive.
    expect(entry.role).toBe("tool");
    expect(entry.text).toBe("Ran the build.");
    expect(entry.toolLabel).toBe("shell");
    expect(entry.fileChangeCount).toBe(3);
    expect(entry.commandOutputByteCount).toBe(1024);

    // No raw command, output, local path, token, or secret leaked anywhere.
    for (const secret of [
      "git push --force",
      "sk-live-DEADBEEF",
      "/Users/owner",
      ".secrets",
      "id_rsa",
      "AKIAIOSFODNN7EXAMPLE",
      "rawCommand",
      "rawOutput",
      "localPath",
      "apiKey",
    ]) {
      expect(serialized.includes(secret)).toBe(false);
    }
  });

  test("bounds oversized text and clamps negative or non-finite counts", () => {
    const entry = buildSafeMessageChainEntry("request.codex.1", 1, {
      role: "assistant",
      text: "x".repeat(20_000),
      fileChangeCount: -5,
      commandOutputByteCount: Number.NaN,
    });
    expect(entry.text.length).toBe(8192);
    expect(entry.fileChangeCount).toBe(0);
    expect(entry.commandOutputByteCount).toBe(0);
  });

  test("projects a bounded chain with stable, deterministic entry refs", () => {
    const activities: ReadonlyArray<ObservedAgentActivity> = [
      { role: "user", text: "delegate this" },
      { role: "assistant", text: "on it" },
      { role: "tool", toolLabel: "apply_patch", fileChangeCount: 2 },
    ];
    const chain = projectSafeMessageChain("request.codex.9", activities);
    expect(chain.map((entry) => entry.entryRef)).toEqual([
      "request.codex.9.chain.0",
      "request.codex.9.chain.1",
      "request.codex.9.chain.2",
    ]);
  });

  test("caps the chain at the frozen retained-segment bound", () => {
    const many: ReadonlyArray<ObservedAgentActivity> = Array.from({ length: 300 }, () => ({
      role: "assistant" as const,
      text: "step",
    }));
    expect(projectSafeMessageChain("request.codex.1", many).length).toBe(256);
  });

  test("safeMessageChainOf reads the already-safe chain from a projection", () => {
    expect(safeMessageChainOf(baseProjection("running")).length).toBe(2);
  });
});
