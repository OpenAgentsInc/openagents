import { describe, expect, test } from "vite-plus/test";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { ClaudeLocalEvent } from "./claude-local-contract";
import { createHeadlessHost } from "./desktop-headless-host";
import type { ProviderLane } from "./provider-lane";

const root = () => mkdtempSync(join(tmpdir(), "oa-headless-host-"));

/** A scripted lane behind the REAL ProviderLane SPI (identity-question shape). */
const identityLane = (answer: string): ProviderLane<null> => ({
  laneRef: "test-local",
  graphLaneRef: "test_local",
  eventChannel: "openagents:test-local:event",
  usageProvider: "test_provider",
  capabilities: () => ({
    laneRef: "test-local",
    provider: "test_provider",
    models: ["test-model-1"],
    features: {
      skills: false,
      planOnly: false,
      reasoningEffort: false,
      images: false,
      fullAuto: true,
      interrupt: false,
      queueFollowup: false,
      steerTurn: false,
      steerChild: false,
      answerQuestion: false,
    },
    composer: {
      displayName: "Test lane",
      reasoningEfforts: [],
      permissionModes: ["owner_full"],
      approvals: "none",
      extensions: [],
    },
    policy: {
      source: "native-static-declaration",
      profileRef: "native:test-local:v1",
      evidence: "conformant",
      allowedModels: ["test-model-1"],
      allowedFeatures: ["fullAuto"],
      allowedExtensions: [],
    },
    recovery: "interrupt_on_restart",
  }),
  admit: () => ({ ok: true, model: "test-model-1", context: null }),
  streamMeta: (ctx) => ({ lane: "test-local", turnRef: ctx.request.turnRef }),
  modelNoteText: (model) => `Test lane · ${model}`,
  runTurn: async ({ emit }) => {
    emit({ kind: "turn_started" });
    emit({ kind: "model_effective", model: "test-model-1" });
    emit({ kind: "text_delta", text: answer });
    emit({ kind: "turn_completed", totalTokens: 5 });
    return { ok: true, text: answer, totalTokens: 5 };
  },
  interrupt: () => false,
  finalMeta: (ctx) => ({ lane: "test-local", turnRef: ctx.request.turnRef }),
  failureMessage: (reason, detail) => `Test lane failed (${reason} · ${detail}).`,
});

describe("desktop headless host (#9161 host-context bootstrap)", () => {
  test("runs an ordinary turn through the production dispatcher without a renderer", async () => {
    const host = createHeadlessHost({ root: root() });
    const thread = host.createThread("identity check");
    const result = await host.submitOrdinaryTurn({
      lane: identityLane("I am the test lane."),
      threadRef: thread.id,
      turnRef: "turn-1",
      message: "who are you",
    });
    expect(result.dispatch.ok).toBe(true);
    const kinds = result.frames.map((frame) => frame.event.kind);
    expect(kinds).toEqual(["turn_started", "model_effective", "text_delta", "turn_completed"]);
    // Ordered typed frames all carry the turn ref.
    expect(result.frames.every((frame) => frame.turnRef === "turn-1")).toBe(true);
  });

  test("an ordinary turn creates NO Full Auto run record", async () => {
    const host = createHeadlessHost({ root: root() });
    const thread = host.createThread();
    expect(host.fullAutoRecordCount()).toBe(0);
    const result = await host.submitOrdinaryTurn({
      lane: identityLane("2 plus 2 is 4."),
      threadRef: thread.id,
      turnRef: "turn-1",
      message: "what is 2 + 2",
    });
    expect(result.fullAutoRecordCount).toBe(0);
    expect(host.fullAutoRecordCount()).toBe(0);
  });

  test("the turn persists to the durable thread (survives a fresh host over the same root)", async () => {
    const dir = root();
    const host = createHeadlessHost({ root: dir });
    const thread = host.createThread("persisted");
    await host.submitOrdinaryTurn({
      lane: identityLane("persisted answer"),
      threadRef: thread.id,
      turnRef: "turn-1",
      message: "remember this",
    });
    // A fresh host over the same root reads the durable thread — the #9161
    // "reopen" property at the host layer.
    const reopened = createHeadlessHost({ root: dir });
    const seen = reopened.listThreads().find((candidate) => candidate.id === thread.id);
    expect(seen).toBeDefined();
    expect(seen?.notes.length ?? 0).toBeGreaterThan(0);
  });

  test("a lane failure surfaces the typed dispatch result, no frames promoted as success", async () => {
    const host = createHeadlessHost({ root: root() });
    const thread = host.createThread();
    const failingLane: ProviderLane<null> = {
      ...identityLane("unused"),
      runTurn: async ({ emit }) => {
        emit({ kind: "turn_started" });
        emit({ kind: "turn_failed", reason: "session_failed", detail: "provider down" });
        return { ok: false, reason: "session_failed" as const, detail: "provider down" };
      },
    };
    const result = await host.submitOrdinaryTurn({
      lane: failingLane,
      threadRef: thread.id,
      turnRef: "turn-1",
      message: "hi",
    });
    expect(result.dispatch.ok).toBe(false);
    const kinds = result.frames.map((frame: { event: ClaudeLocalEvent }) => frame.event.kind);
    expect(kinds).toContain("turn_failed");
  });

  test("explicit Full Auto start creates exactly one run record with a stable ref", () => {
    const host = createHeadlessHost({ root: root() });
    expect(host.fullAutoRuns()).toHaveLength(0);
    const run = host.startFullAutoRun({
      title: "burn the backlog",
      objective: "Implement issue #1 and run the named verification.",
      doneCondition: "The verification passes or a concrete blocker is reported.",
    });
    expect(run.runRef).toBeTruthy();
    expect(run.state).toBe("running");
    const runs = host.fullAutoRuns();
    expect(runs).toHaveLength(1);
    expect(runs[0].runRef).toBe(run.runRef);
  });

  test("the Full Auto run ref survives reopen from a fresh host over the same root", () => {
    const dir = root();
    const host = createHeadlessHost({ root: dir });
    const run = host.startFullAutoRun({
      title: "durable run",
      objective: "Do the durable thing.",
      doneCondition: "It is done.",
    });
    const reopened = createHeadlessHost({ root: dir });
    const seen = reopened.fullAutoRuns().find((candidate) => candidate.runRef === run.runRef);
    expect(seen).toBeDefined();
    expect(seen?.objective).toBe("Do the durable thing.");
  });

  test("an ordinary turn beside a Full Auto run does not create a second run record", async () => {
    const host = createHeadlessHost({ root: root() });
    host.startFullAutoRun({ title: "t", objective: "o", doneCondition: "d" });
    const thread = host.createThread();
    const result = await host.submitOrdinaryTurn({
      lane: identityLane("beside"),
      threadRef: thread.id,
      turnRef: "turn-1",
      message: "hi",
    });
    // The ordinary turn created no NEW run record — still exactly one.
    expect(host.fullAutoRuns()).toHaveLength(1);
    expect(result.fullAutoRecordCount).toBe(0);
  });
});
