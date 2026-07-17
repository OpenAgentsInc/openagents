/**
 * QA-3 (#8908): deterministic fixture shell states for the Desktop visual
 * baseline harness. Every state is a pure function of its name — frozen
 * display timestamps, frozen ISO instants, no clocks, no randomness — so two
 * probe runs on the same build render byte-identical frames.
 *
 * The states deliberately reuse the REAL shell-state constructors
 * (`initialDesktopShellState`, `withChatSelected`, `withFullAutoLiveState`)
 * so the captured pixels exercise the same projection path production uses,
 * not a parallel storybook. Consumed by `visual-baseline.ts` (renderer
 * mount) and unit-tested for determinism in
 * `visual-baseline-fixtures.test.ts`.
 */
import type { DesktopThread } from "../chat-contract.ts";
import {
  initialDesktopShellState,
  withChatSelected,
  withFullAutoLiveState,
  withHarnessLanes,
  type DesktopShellState,
} from "./shell.ts";

/** The one frozen instant every fixture clock reads (see the Date shim in
 * visual-baseline.ts). Rail relative timestamps derive from this. */
export const VISUAL_BASELINE_FROZEN_NOW_ISO = "2026-07-15T09:45:00.000Z";

export {
  VISUAL_BASELINE_SHELL_STATES,
  VISUAL_BASELINE_STATES,
  VISUAL_BASELINE_WORKBENCH_STATES,
  isVisualBaselineStateName,
  isVisualBaselineShellStateName,
  isVisualBaselineWorkbenchStateName,
  type VisualBaselineStateName,
} from "../visual-baseline-contract.ts";
import { type VisualBaselineShellStateName } from "../visual-baseline-contract.ts";

const FIXTURE_HOST = "visual-baseline/fixture";
/** Frozen display timestamp ("HH:MM", the shell's message format). */
const FIXTURE_CLOCK = "09:41";
const FIXTURE_UPDATED_AT = "2026-07-15T09:41:00.000Z";
const FIXTURE_CREATED_AT = "2026-07-15T09:40:00.000Z";

const fixtureBase = (): DesktopShellState =>
  withHarnessLanes(initialDesktopShellState(FIXTURE_HOST, FIXTURE_CLOCK), {
    fable: { available: true, reason: null },
    codex: { available: true, reason: null },
  });

const fixtureThread = (
  suffix: string,
  title: string,
  notes: DesktopThread["notes"],
): DesktopThread => ({
  id: `thread.visual.${suffix}`,
  title,
  createdAt: FIXTURE_CREATED_AT,
  updatedAt: FIXTURE_UPDATED_AT,
  notes,
});

const selected = (state: DesktopShellState, thread: DesktopThread): DesktopShellState =>
  withChatSelected({ ...state, threads: [thread] }, thread);

const planThread = (): DesktopThread =>
  fixtureThread("plan", "Wire the payout receipts", [
    {
      key: "note.visual.plan.user",
      role: "user",
      text: "Draft a plan to wire the payout receipt projection.",
      timestamp: FIXTURE_CLOCK,
    },
    {
      key: "note.visual.plan.card",
      role: "system",
      text: "Plan updated",
      timestamp: FIXTURE_CLOCK,
      runtime: {
        kind: "plan",
        entries: [
          { step: "Audit the receipt schema", status: "completed" },
          { step: "Wire the projection into the ledger view", status: "in_progress" },
          { step: "Prove the gate with a fixture receipt", status: "pending" },
        ],
      },
    },
    {
      key: "note.visual.plan.assistant",
      role: "assistant",
      text: "Plan staged — starting with the receipt schema audit.",
      timestamp: FIXTURE_CLOCK,
    },
  ]);

const approvalThread = (): DesktopThread =>
  fixtureThread("approval", "Run the verification sweep", [
    {
      key: "note.visual.approval.user",
      role: "user",
      text: "Run the verification sweep for the receipts lane.",
      timestamp: FIXTURE_CLOCK,
    },
    {
      key: "note.visual.approval.card",
      role: "system",
      text: "Approval · pnpm test",
      timestamp: FIXTURE_CLOCK,
      question: {
        turnRef: "turn.visual.approval",
        questionRef: "question.visual.approval",
        status: "pending",
        kind: "tool_approval",
        questions: [
          {
            questionRef: "question.visual.approval",
            question: "/bin/zsh -lc \"shasum -a 256 /tmp/commandcode-audit/package.tgz && unzip -q /tmp/commandcode-audit/package.tgz -d /tmp/commandcode-audit/unpacked\"",
            header: "Command approval",
            options: [
              { optionRef: "option.approve", label: "Approve" },
              { optionRef: "option.deny", label: "Deny" },
            ],
            multiSelect: false,
          },
        ],
      },
    },
  ]);

const reasoningThread = (): DesktopThread =>
  fixtureThread("reasoning", "Migrate the ledger projection", [
    {
      key: "note.visual.reasoning.user",
      role: "user",
      text: "Migrate the ledger projection to the new schema.",
      timestamp: FIXTURE_CLOCK,
    },
    {
      key: "note.visual.reasoning.card",
      role: "system",
      text: "Reasoning · Weighing the projection contract before writing the migration: the ledger rows must stay exact, so the projection change lands behind the existing gate.",
      timestamp: FIXTURE_CLOCK,
    },
    {
      key: "note.visual.reasoning.assistant",
      role: "assistant",
      text: "Migration plan settled — the projection keeps exact rows.",
      timestamp: FIXTURE_CLOCK,
    },
  ]);

const fullAutoThread = (): DesktopThread =>
  fixtureThread("full-auto", "Keep working the backlog", [
    {
      key: "note.visual.full-auto.user",
      role: "user",
      text: "Keep working the backlog until I turn Full Auto off.",
      timestamp: FIXTURE_CLOCK,
    },
    {
      key: "note.visual.full-auto.assistant",
      role: "assistant",
      text: "Full Auto engaged — continuing with the next backlog item.",
      timestamp: FIXTURE_CLOCK,
    },
  ]);

/**
 * The fixed capture set. Pure: same name -> deep-equal state, always.
 * `full-auto-running` layers the durable toggle AND the live turn_running
 * push so the composer renders the "Full Auto running…" badge (FA-H4).
 */
export const visualBaselineShellState = (name: VisualBaselineShellStateName): DesktopShellState => {
  switch (name) {
    case "composer-idle":
      return fixtureBase();
    case "thread-plan-card":
      return selected(fixtureBase(), planThread());
    case "approval-card":
      return {
        ...selected(fixtureBase(), approvalThread()),
        questionAnswerHostAvailable: true,
      };
    case "reasoning-disclosure":
      return selected(fixtureBase(), reasoningThread());
    case "full-auto-running": {
      const thread = fullAutoThread();
      const state = selected(fixtureBase(), thread);
      return withFullAutoLiveState(
        { ...state, fullAutoByThread: { [thread.id]: true } },
        thread.id,
        { state: "turn_running", turnRef: "turn.visual.full-auto" },
      );
    }
    case "surface-tabs": {
      const state = selected(fixtureBase(), planThread());
      const document = {
        grantRef: "grant.visual.surface",
        pathRef: "src/receipt.ts",
        content: "export const receipt = 'exact'\n",
        revisionRef: "revision.visual.receipt",
        languageMode: "typescript" as const,
        encoding: "utf-8" as const,
        lineEnding: "lf" as const,
        sizeBytes: 31,
      };
      return {
        ...state,
        workspace: "files",
        codingCatalog: {
          ...state.codingCatalog,
          selectedSessionRef: "session.visual.surface",
          sessions: [{
            sessionRef: "session.visual.surface",
            workContextRef: "context.visual.surface",
            grantRef: "grant.visual.surface",
            projectRef: "project.visual",
            repositoryRef: "repository.visual",
            worktreeRef: "worktree.visual",
            projectLabel: "OpenAgents",
            repositoryLabel: "openagents",
            worktreeLabel: "feature/t3-ui",
            state: "active",
            lastActiveAt: FIXTURE_UPDATED_AT,
            recoveryReason: null,
          }],
          totalSessions: 1,
          activeCount: 1,
        },
        workspaceBrowser: {
          ...state.workspaceBrowser,
          phase: "ready",
          grantRef: "grant.visual.surface",
          pages: {
            "": {
              state: "available",
              grantRef: "grant.visual.surface",
              directoryRef: "",
              entries: [
                { name: "src", pathRef: "src", kind: "directory", expandable: true, sizeBytes: null, revisionRef: "revision.visual.src" },
                { name: "README.md", pathRef: "README.md", kind: "file", expandable: false, sizeBytes: 2_048, revisionRef: "revision.visual.readme" },
              ],
              nextOffset: null,
              cache: { key: "cache.visual.root", epoch: 1, freshness: "current" },
            },
          },
        },
        workspaceEditor: {
          ...state.workspaceEditor,
          activePathRef: document.pathRef,
          tabs: [{ pathRef: document.pathRef, phase: "ready", document, externalDocument: null, draft: document.content, selection: { start: 0, end: 0 }, selectionVersion: 0, undo: [], redo: [], saveState: "idle", reason: null, findQuery: "", findMatches: [], findIndex: 0 }],
        },
      };
    }
    case "files-rich-diff": {
      const state = visualBaselineShellState("surface-tabs");
      return {
        ...state,
        workspace: "review",
        git: {
          ...state.git,
          phase: "ready",
          status: {
            ok: true,
            op: "status",
            branch: "feature/t3-ui",
            upstream: "origin/feature/t3-ui",
            detached: false,
            ahead: 1,
            behind: 0,
            staged: [{ path: "src/receipt.ts", status: "modified" }],
            unstaged: [{ path: "src/ledger.ts", status: "modified" }],
            untracked: [],
            truncated: false,
            repositoryRef: "repository.visual",
            statusRef: "status.visual",
            headRef: "head.visual",
          },
          diff: {
            ok: true,
            op: "diff",
            repositoryRef: "repository.visual",
            statusRef: "status.visual",
            path: "src/receipt.ts",
            source: "staged",
            causalItemRef: null,
            content: "@@ -1 +1 @@\n-export const receipt = 'draft'\n+export const receipt = 'exact'",
            hunks: [{ header: "@@ -1 +1 @@", oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, content: "-export const receipt = 'draft'\n+export const receipt = 'exact'" }],
            truncated: false,
          },
        },
      };
    }
    case "terminal-workbench": {
      const state = visualBaselineShellState("surface-tabs");
      return {
        ...state,
        workspace: "chat",
        terminal: {
          phase: "ready",
          activeRef: "terminal.visual",
          input: "pnpm test",
          notice: null,
          sessions: [{
            sessionRef: "terminal.visual",
            cwdLabel: "openagents · feature/t3-ui",
            shellLabel: "zsh",
            status: "running",
            exitCode: null,
            recovered: true,
            gap: false,
            output: "$ pnpm test\n✓ transcript message contracts\n✓ sticky scroll controller\n✓ terminal lifecycle\n\n209 files passed\n2025 tests passed\n",
            previews: [{ port: 5173, url: "http://localhost:5173", ready: true }],
          }],
        },
      };
    }
    case "browser-preview":
      return visualBaselineShellState("terminal-workbench");
  }
};
