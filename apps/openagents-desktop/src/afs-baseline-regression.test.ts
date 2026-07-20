/**
 * AFS-00 baseline regression capture (using existing code).
 *
 * This suite freezes two current behaviors as the regression baseline for the
 * Apple FM router to full-agent-system program:
 *
 * 1. Current local chat does NOT dispatch a provider. The Apple FM local answer
 *    path runs through the host supervisor and its local helper session. It does
 *    not go through the provider-lane dispatcher.
 * 2. The explicit provider path still works. A typed request runs through the
 *    real `makeProviderLaneDispatcher` shared engine to a terminal completed
 *    journal disposition.
 *
 * It also anchors the AFS-00 frozen input bounds to the current code so a later
 * packet cannot silently move them.
 */
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { MAX_RENDERER_PREPARED_INPUT_CHARS, MAX_TURN_INPUT_CHARS } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import {
  createAppleFmHost,
  type AppleFmLaunchOutcome,
  type AppleFmLauncher,
  type AppleFmLauncherSession,
  type AppleFmProbe,
} from "./apple-fm-host.ts";
import { openLocalTurnJournal } from "./local-turn-journal.ts";
import {
  makeProviderLaneDispatcher,
  type ProviderLane,
  type ProviderLaneDispatcherDeps,
} from "./provider-lane.ts";
import type { ClaudeLocalEvent, ClaudeLocalStartRequest } from "./claude-local-contract.ts";
import { makeThreadStore } from "./thread-store.ts";

const appRoot = path.resolve(import.meta.dirname, "..");

const readySession = (record: (call: string) => void): AppleFmLauncherSession => ({
  mode: "launched",
  probe: async (): Promise<AppleFmProbe> => ({
    status: "ready",
    ready: true,
    model: "apple-foundation-model",
    profileId: "apple-fm-local",
    usageTruth: "estimated",
  }),
  complete: async () => {
    record("local_helper_complete");
    return { outcome: "completed", text: "local answer", usageTruth: "estimated", totalTokens: 5 };
  },
  stop: () => {},
});

const launcher = (session: AppleFmLauncherSession): AppleFmLauncher => ({
  supported: () => true,
  launch: async (): Promise<AppleFmLaunchOutcome> => ({ kind: "session", session }),
});

describe("AFS-00 baseline: local chat does not dispatch a provider", () => {
  test("Apple FM local answer completes through its local helper, not a dispatcher", async () => {
    const calls: string[] = [];
    const host = createAppleFmHost(launcher(readySession((call) => calls.push(call))));
    await host.ensureStarted();
    const turn = await host.runTurn("hi");
    expect(turn).toMatchObject({ ok: true, outcome: "completed", usageTruth: "estimated" });
    // The only inference call was the local helper session; no provider dispatch.
    expect(calls).toEqual(["local_helper_complete"]);
  });

  test("the Apple FM host module is structurally dispatcher-free", () => {
    const source = readFileSync(path.join(appRoot, "src", "apple-fm-host.ts"), "utf8");
    for (const dispatcherSymbol of [
      "provider-lane",
      "makeProviderLaneDispatcher",
      "dispatchTurn",
      "codex-local-runtime",
      "claude-local",
    ]) {
      expect(source.includes(dispatcherSymbol)).toBe(false);
    }
  });

  test("the current renderer standby path uses the direct Apple FM bridge flattener", () => {
    const source = readFileSync(path.join(appRoot, "src", "renderer", "shell.ts"), "utf8");
    expect(source.includes("openAgentsStandby")).toBe(true);
    expect(source.includes("buildOpenAgentsAppleFmPrompt")).toBe(true);
    // The renderer standby answer path does not call the provider-lane dispatcher.
    expect(source.includes("dispatchTurn")).toBe(false);
  });
});

describe("AFS-00 baseline: the explicit provider path still works", () => {
  const fixtureLane = (record: (message: string) => void): ProviderLane<null> => ({
    laneRef: "afs-baseline-fixture",
    graphLaneRef: "afs_baseline_fixture",
    eventChannel: "openagents:afs-baseline:event",
    usageProvider: "afs_baseline_provider",
    capabilities: () => ({
      laneRef: "afs-baseline-fixture",
      provider: "afs_baseline_provider",
      models: ["fixture-model-1"],
      features: {
        skills: false,
        planOnly: false,
        reasoningEffort: false,
        images: false,
        fullAuto: false,
        interrupt: true,
        queueFollowup: false,
        steerTurn: false,
        steerChild: false,
        answerQuestion: false,
      },
      composer: {
        displayName: "AFS Baseline",
        reasoningEfforts: [],
        permissionModes: ["owner_full"],
        approvals: "none",
        extensions: [],
      },
      policy: {
        source: "native-static-declaration",
        profileRef: "native:afs-baseline:v1",
        evidence: "conformant",
        allowedModels: ["fixture-model-1"],
        allowedFeatures: ["interrupt"],
        allowedExtensions: [],
      },
      recovery: "interrupt_on_restart",
    }),
    admit: () => ({ ok: true, model: "fixture-model-1", context: null }),
    streamMeta: (ctx) => ({ lane: "afs-baseline-fixture", turnRef: ctx.request.turnRef, model: "fixture-model-1" }),
    modelNoteText: (model) => `AFS Baseline · ${model}`,
    runTurn: async ({ message, emit }) => {
      record(message);
      const events: ReadonlyArray<ClaudeLocalEvent> = [
        { kind: "turn_started" },
        { kind: "model_effective", model: "fixture-model-1" },
        { kind: "text_delta", text: "dispatched." },
        {
          kind: "turn_completed",
          totalTokens: 3,
          accountRef: "afs-baseline-account-1",
          usage: { inputTokens: 2, cachedInputTokens: 0, outputTokens: 1, reasoningTokens: 0, totalTokens: 3 },
        },
      ];
      for (const event of events) emit(event);
      return {
        ok: true,
        text: "dispatched.",
        totalTokens: 3,
        accountRef: "afs-baseline-account-1",
        providerSessionRef: "afs-baseline-session-1",
      };
    },
    interrupt: () => false,
    finalMeta: (ctx) => ({
      lane: "afs-baseline-fixture",
      turnRef: ctx.request.turnRef,
      model: "fixture-model-1",
      totalTokens: ctx.result.totalTokens,
      durationMs: ctx.durationMs,
    }),
    failureMessage: (reason, detail) => `The AFS baseline lane failed (${reason}${detail === "" ? "" : ` · ${detail}`}).`,
  });

  test("a typed request dispatches through the shared engine to a completed journal disposition", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "afs-baseline-explicit-"));
    try {
      const store = makeThreadStore(path.join(root, "threads.json"));
      const journal = openLocalTurnJournal(path.join(root, "turns.json"));
      const dispatched: string[] = [];
      const deps: ProviderLaneDispatcherDeps = {
        threads: () => store,
        journal,
        liveAgentGraph: { beginTurn: () => {}, applyEvent: () => {} },
        usageLedger: { record: () => {} },
        captureTurnCheckpoint: async () => {},
        localTurnFlushers: new Set(),
        isQuitting: () => false,
      };
      const thread = store.newThread("AFS baseline explicit");
      const request: ClaudeLocalStartRequest = {
        turnRef: "afs-baseline-turn-1",
        threadRef: thread.id,
        message: "run the explicit provider path",
      };
      const result = await makeProviderLaneDispatcher(deps).dispatchTurn(
        fixtureLane((message) => dispatched.push(message)),
        request,
        null,
      );
      // The explicit provider path dispatched (the lane ran) and completed.
      expect(result.ok).toBe(true);
      expect(dispatched).toEqual(["run the explicit provider path"]);
      const record = journal.get({ threadRef: thread.id, turnRef: "afs-baseline-turn-1", lane: "afs-baseline-fixture" });
      expect(record?.disposition).toBe("completed");
      expect(record?.phase).toBe("completed");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("AFS-00 baseline: frozen input bounds match current code", () => {
  test("the frozen turn input bound equals the current Apple FM IPC bound", () => {
    const source = readFileSync(path.join(appRoot, "src", "apple-fm-contract.ts"), "utf8");
    expect(source.includes(`isMaxLength(${MAX_TURN_INPUT_CHARS})`)).toBe(true);
    expect(MAX_TURN_INPUT_CHARS).toBe(4000);
  });

  test("the frozen renderer-prepared bound equals the current renderer cap", () => {
    const source = readFileSync(path.join(appRoot, "src", "renderer", "shell.ts"), "utf8");
    expect(source.includes(`APPLE_FM_PROMPT_MAX_CHARS = ${MAX_RENDERER_PREPARED_INPUT_CHARS}`)).toBe(true);
    expect(MAX_RENDERER_PREPARED_INPUT_CHARS).toBe(3900);
  });
});
