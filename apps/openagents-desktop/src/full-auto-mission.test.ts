import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import { openFullAutoRegistry, type FullAutoRecord } from "./full-auto-registry.ts";
import {
  appendFullAutoQueuedInstruction,
  compileFullAutoMissionPacket,
  FULL_AUTO_MISSION_SCHEMA,
  renderFullAutoMissionPrompt,
} from "./full-auto-mission.ts";
import { reconcileFullAutoThreads } from "./full-auto-reconcile.ts";
import {
  FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION,
  FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE,
  openFullAutoRunRegistry,
} from "./full-auto-run-registry.ts";

const lowLevelRecord = (continuationCount = 2): FullAutoRecord => ({
  threadRef: "thread.mission",
  enabled: true,
  continuationCount,
  updatedAt: "2026-07-18T00:00:00.000Z",
  enabledAt: "2026-07-18T00:00:00.000Z",
  workspaceRef: "/workspace",
  profile: { lane: "codex-local", accountRef: "codex-account" },
});

describe("Full Auto mission packet", () => {
  test("preserves the exact durable objective/done condition and changes prompt bytes when the objective changes", () => {
    const root = mkdtempSync(path.join(tmpdir(), "full-auto-mission-"));
    try {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"));
      const started = registry.startNew({
        title: "Ship the focused fix",
        objective: "Fix issue #9000 exactly; keep punctuation & casing.",
        doneCondition: "Tests pass and the real provider receipt names #9000.",
        objectiveSource: "user",
        workspaceRef: "/workspace",
        profile: { lane: "codex-local", accountRef: "codex-account" },
        turnCap: 7,
        threadRef: "thread.mission",
        actor: "owner_ui",
        reason: "test start",
      });
      expect(started.ok).toBe(true);
      if (!started.ok) return;
      const packet = compileFullAutoMissionPacket({
        run: started.run,
        record: lowLevelRecord(),
        threadRef: "thread.mission",
        profile: { lane: "claude-local", accountRef: "claude-account" },
        turnCap: 7,
        priorAcceptedOutcome: null,
        previousHandoff: null,
      });
      expect(packet).toMatchObject({
        schema: FULL_AUTO_MISSION_SCHEMA,
        runRef: started.run.runRef,
        objective: "Fix issue #9000 exactly; keep punctuation & casing.",
        doneCondition: "Tests pass and the real provider receipt names #9000.",
        currentLane: "claude-local",
        continuationOrdinal: 3,
        turnCap: 7,
        remainingTurnsIncludingThisOne: 5,
      });
      const prompt = renderFullAutoMissionPrompt(packet);
      expect(prompt).toContain(packet.objective);
      expect(prompt).toContain(packet.doneCondition);
      expect(
        renderFullAutoMissionPrompt({ ...packet, objective: `${packet.objective} revised` }),
      ).not.toBe(prompt);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("uses an explicitly attributed legacy mission instead of inventing an objective", () => {
    const packet = compileFullAutoMissionPacket({
      run: null,
      record: lowLevelRecord(0),
      threadRef: "thread.mission",
      profile: { lane: "codex-local" },
      turnCap: 20,
      priorAcceptedOutcome: null,
      previousHandoff: null,
    });
    expect(packet.runRef).toBeNull();
    expect(packet.objectiveSource).toBe("legacy_migration");
    expect(packet.objective).toBe(FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE);
    expect(packet.doneCondition).toBe(FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION);
  });

  test("carries only the bounded accepted-outcome and handoff facts", () => {
    const packet = compileFullAutoMissionPacket({
      run: null,
      record: lowLevelRecord(),
      threadRef: "thread.mission",
      profile: { lane: "claude-local" },
      turnCap: 20,
      priorAcceptedOutcome: {
        schema: "openagents.desktop.local_turn_record.v1",
        threadRef: "thread.mission",
        turnRef: "turn.codex.1",
        lane: "codex-local",
        userMessageKey: "user.1",
        assistantMessageKey: "assistant.1",
        accountRef: "codex-account",
        providerSessionRef: "provider-private-session",
        model: "model-private",
        phase: "completed",
        persistedCursor: 1,
        assistantText: "raw assistant text must not be copied into the packet",
        assistantSegments: [],
        recoveryGeneration: 0,
        disposition: "completed",
        createdAt: "2026-07-18T00:00:00.000Z",
        updatedAt: "2026-07-18T00:01:00.000Z",
      },
      previousHandoff: {
        handoffRef: "handoff.provider.1",
        runRef: "run.1",
        threadRef: "thread.mission",
        from: "codex-local",
        to: "claude-local",
        actor: "turn_resolution",
        at: "2026-07-18T00:01:30.000Z",
        reason: "private detail is deliberately not copied",
        disposition: "complete_within_bounds",
        truncated: false,
      },
    });
    const rendered = renderFullAutoMissionPrompt(packet);
    expect(packet.priorAcceptedOutcome).toEqual({
      turnRef: "turn.codex.1",
      lane: "codex-local",
      disposition: "completed",
      updatedAt: "2026-07-18T00:01:00.000Z",
    });
    expect(packet.previousHandoff).toMatchObject({ from: "codex-local", to: "claude-local" });
    expect(rendered).not.toContain("provider-private-session");
    expect(rendered).not.toContain("raw assistant text");
    expect(rendered).not.toContain("private detail");
  });

  test("recompiles after a same-pass rotation and queued instructions augment rather than replace it", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "full-auto-mission-rotation-"));
    try {
      const registry = openFullAutoRegistry(path.join(root, "registry.json"));
      registry.set("thread.mission", true, {
        workspaceRef: "/workspace",
        profile: { lane: "codex-local" },
      });
      registry.bindRoutingPolicy("thread.mission", [
        { lane: "codex-local" },
        { lane: "claude-local" },
      ]);
      let handoffRecorded = false;
      const compiled: string[] = [];
      await reconcileFullAutoThreads({
        registry,
        nonterminalThreadRefs: () => new Set(),
        resolveWorkspaceRef: () => "/workspace",
        journalHasNonterminalTurn: () => false,
        compileDispatchMessage: ({ profile }) => {
          const message = `mission:${profile?.lane}:handoff=${handoffRecorded}`;
          compiled.push(message);
          return message;
        },
        dispatch: async ({ profile, message }) =>
          profile?.lane === "codex-local"
            ? { ok: false, reason: "provider failed", failureClass: "provider_error" }
            : { ok: message === "mission:claude-local:handoff=true" },
        onRotated: () => {
          handoffRecorded = true;
        },
      });
      expect(compiled).toEqual([
        "mission:codex-local:handoff=false",
        "mission:claude-local:handoff=true",
      ]);
      expect(appendFullAutoQueuedInstruction(compiled[1]!, "also inspect issue #9001")).toContain(
        compiled[1]!,
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
