/**
 * QA-3 (#8908): determinism and content oracles for the visual-baseline
 * fixture shell states. The pixel gate is only as honest as its inputs:
 * every state must be a pure function of its name (deep-equal across
 * constructions, no live clocks) and must actually carry the surface it
 * claims to capture (plan card, pending approval, reasoning row, Full Auto
 * running badge state).
 */
import { describe, expect, test } from "vite-plus/test";
import {
  VISUAL_BASELINE_SHELL_STATES,
  VISUAL_BASELINE_STATES,
  isVisualBaselineStateName,
  visualBaselineShellState,
} from "./visual-baseline-fixtures.ts";
import { activeFullAutoEnabled, activeFullAutoTurnRunning } from "./shell.ts";

describe("visual-baseline fixture states", () => {
  test("the capture set is the issue's fixed shell-state list", () => {
    expect([...VISUAL_BASELINE_STATES]).toEqual([
      "composer-idle",
      "thread-plan-card",
      "approval-card",
      "reasoning-disclosure",
      "full-auto-running",
      "surface-tabs",
      "files-rich-diff",
      "terminal-workbench",
      "browser-preview",
      "settings-routed",
      "remote-connect",
      "workbench-messages-reasoning",
      "workbench-commands",
      "workbench-files",
      "workbench-tools-mcp-dynamic",
      "workbench-tools-web-image",
      "workbench-plans-approvals",
      "workbench-agents",
      "workbench-context",
      "workbench-notices-long-tail",
      "workbench-shell",
      "workbench-frame",
    ]);
    expect(isVisualBaselineStateName("composer-idle")).toBe(true);
    expect(isVisualBaselineStateName("not-a-state")).toBe(false);
  });

  test("every state is deterministic: same name, deep-equal state, twice", () => {
    for (const name of VISUAL_BASELINE_SHELL_STATES) {
      expect(visualBaselineShellState(name)).toEqual(visualBaselineShellState(name));
    }
  });

  test("no fixture note carries a live-clock timestamp shape", () => {
    for (const name of VISUAL_BASELINE_SHELL_STATES) {
      const state = visualBaselineShellState(name);
      for (const note of state.notes) {
        // Frozen display timestamps only — the fixture clock, never Date.now.
        expect(note.timestamp).toBe("09:41");
      }
      for (const thread of state.threads) {
        expect(thread.updatedAt).toBe("2026-07-15T09:41:00.000Z");
      }
    }
  });

  test("composer-idle renders an empty transcript with both lanes available", () => {
    const state = visualBaselineShellState("composer-idle");
    expect(state.notes).toHaveLength(0);
    expect(state.activeThreadId).toBeNull();
    expect(state.harnessLanes.codex.available).toBe(true);
    expect(state.harnessLanes.fable.available).toBe(true);
  });

  test("thread-plan-card carries a typed runtime plan card", () => {
    const state = visualBaselineShellState("thread-plan-card");
    const plan = state.notes.find((note) => note.runtime?.kind === "plan");
    expect(plan).toBeDefined();
    expect(
      plan!.runtime!.kind === "plan" && plan!.runtime!.entries.map((entry) => entry.status),
    ).toEqual(["completed", "in_progress", "pending"]);
  });

  test("approval-card carries a pending tool_approval question card", () => {
    const state = visualBaselineShellState("approval-card");
    const approval = state.notes.find((note) => note.question !== undefined);
    expect(approval).toBeDefined();
    expect(approval!.question!.status).toBe("pending");
    expect(approval!.question!.kind).toBe("tool_approval");
    expect(state.questionAnswerHostAvailable).toBe(true);
    expect(approval!.question!.questions[0]!.options.map((option) => option.label)).toEqual([
      "Approve",
      "Deny",
    ]);
  });

  test("reasoning-disclosure carries a Reasoning system row", () => {
    const state = visualBaselineShellState("reasoning-disclosure");
    expect(
      state.notes.some((note) => note.role === "system" && /^Reasoning\s*·/.test(note.text)),
    ).toBe(true);
  });

  test("full-auto-running projects the enabled toggle AND the running badge", () => {
    const state = visualBaselineShellState("full-auto-running");
    expect(activeFullAutoEnabled(state)).toBe(true);
    expect(activeFullAutoTurnRunning(state)).toBe(true);
    expect(state.pending).toBe(false);
  });
});
