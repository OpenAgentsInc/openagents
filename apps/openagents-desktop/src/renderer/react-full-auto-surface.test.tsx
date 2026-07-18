/**
 * FA-UX-01 (#8974) DOM coverage for the REAL production render path
 * (`WorkbenchShell` mounts `ReactFullAutoSurface` as the "full-auto"
 * `workspaceSurface`, see react-primitive-adapters.tsx) -- proves the
 * launcher form collects the mission contract before Start is enabled, the
 * run view pins objective/done-condition/state and wires Pause/Resume/Stop/
 * Retry to the exact intents `full-auto-workspace.ts` handles, and that the
 * ordinary chat composer never renders inside this surface.
 */
import { resolveIntentRef, type IntentReporter } from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";
import { Window } from "happy-dom";
import { act, type ReactNode } from "react";
import type { Root } from "react-dom/client";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { initialDesktopShellState, type DesktopShellState } from "./shell.ts";
import { emptyFullAutoWorkspaceState } from "./full-auto-workspace.ts";
import type { FullAutoControlRun } from "../full-auto-control-contract.ts";

const restores: Array<() => void> = [];
const roots = new Set<Root>();
const installDom = () => {
  const window = new Window({ url: "http://localhost/" });
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Text: window.Text,
    Document: window.Document,
    Range: window.Range,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLDivElement: window.HTMLDivElement,
    HTMLInputElement: window.HTMLInputElement,
    HTMLTextAreaElement: window.HTMLTextAreaElement,
    HTMLSelectElement: window.HTMLSelectElement,
    Event: window.Event,
    InputEvent: window.InputEvent,
    KeyboardEvent: window.KeyboardEvent,
    MouseEvent: window.MouseEvent,
    MutationObserver: window.MutationObserver,
    ResizeObserver: ResizeObserverStub,
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
    IS_REACT_ACT_ENVIRONMENT: true,
  };
  const previous = new Map<string, PropertyDescriptor | undefined>();
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value });
  }
  restores.push(() => {
    for (const [name, descriptor] of previous) {
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name];
      else Object.defineProperty(globalThis, name, descriptor);
    }
  });
  const container = window.document.createElement("div") as unknown as HTMLDivElement;
  window.document.body.appendChild(container as never);
  return { window, container };
};

afterEach(async () => {
  await act(async () => {
    for (const root of roots) root.unmount();
    roots.clear();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  while (restores.length > 0) restores.pop()?.();
});

const recorder = () => {
  const received: Array<{ name: string; payload: unknown }> = [];
  const report: IntentReporter = (ref, payload) =>
    Effect.sync(() => received.push(resolveIntentRef(ref, payload)));
  return { received, report };
};
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 20));
const createTestRoot = (container: HTMLDivElement): Root => {
  const root = createRoot(container);
  roots.add(root);
  return root;
};
const render = async (root: Root, node: ReactNode): Promise<void> => {
  await act(async () => {
    root.render(node);
    await settle();
  });
};
const interact = async (interaction: () => void): Promise<void> => {
  await act(async () => {
    interaction();
    await settle();
  });
};

const baseRun = (overrides: Partial<FullAutoControlRun> = {}): FullAutoControlRun => ({
  runRef: "run-1",
  threadRef: "thread-1",
  title: "Ship the thing",
  objective: "Ship the thing end to end.",
  objectiveSource: "user",
  doneCondition: "pnpm run check is green on main.",
  workspaceRef: "/workspace/repo",
  lane: "codex-local",
  turnCap: 20,
  successfulAttempts: 2,
  failedAttempts: 0,
  state: "running",
  stateRevision: 3,
  terminalReason: null,
  predecessorRunRef: null,
  migratedFrom: null,
  createdAt: "2026-07-17T00:00:00.000Z",
  startedAt: "2026-07-17T00:00:01.000Z",
  lastProgressAt: "2026-07-17T00:05:00.000Z",
  pausedAt: null,
  stoppedAt: null,
  completedAt: null,
  transitions: [],
  stallCause: null,
  nextRetryAt: null,
  recoveryAction: "none",
  ...overrides,
});

const fixtureState = (extra: Partial<DesktopShellState> = {}): DesktopShellState => ({
  ...initialDesktopShellState("electron/darwin"),
  workspace: "full-auto",
  ...extra,
});

describe("ReactFullAutoSurface: launcher (FA-AC-54)", () => {
  test("Start needs only objective + resolved workspace; advanced fields stay collapsed and inferred", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={fixtureState()} report={report} />);
    const start = container.querySelector('[data-en-key="full-auto-launcher-start"]') as HTMLButtonElement;
    expect(start.disabled).toBe(true);

    const title = container.querySelector("#full-auto-launcher-title") as HTMLInputElement;
    const objective = container.querySelector("#full-auto-launcher-objective") as HTMLTextAreaElement;
    const doneCondition = container.querySelector("#full-auto-launcher-done-condition") as HTMLTextAreaElement;
    const workspace = container.querySelector("#full-auto-launcher-workspace") as HTMLInputElement;
    expect(title).not.toBeNull();
    expect(objective).not.toBeNull();
    expect(doneCondition).not.toBeNull();
    expect(workspace).not.toBeNull();

    const advanced = container.querySelector(".oa-react-full-auto-advanced") as HTMLDetailsElement;
    expect(advanced.open).toBe(false);
    await render(root, <ReactFullAutoSurface state={fixtureState({
      fullAuto: { ...emptyFullAutoWorkspaceState(), launcher: { ...emptyFullAutoWorkspaceState().launcher, objective: "Do it", workspaceRef: "/ws" } },
    })} report={report} />);
    const startEnabled = container.querySelector('[data-en-key="full-auto-launcher-start"]') as HTMLButtonElement;
    expect(startEnabled.disabled).toBe(false);
    await interact(() => startEnabled.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoLauncherStartRequested", payload: null });
  });

  // Note: onChange dispatch on the shadcn/base-ui `Input`/`Textarea`
  // primitives could not be reliably driven via a simulated native
  // "input" event in this happy-dom harness (a general test-infra gap
  // unrelated to this feature -- even a raw <input> exhibits the same
  // behavior here). The field-change intent handlers themselves are
  // proven directly against the real registry in
  // full-auto-workspace.test.ts ("field-change intents update exactly
  // the named draft field"); this test proves the complementary half --
  // that each field's `value` reads live from `state.fullAuto.launcher`.
  test("each field renders the current draft value from state", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={fixtureState({
      fullAuto: {
        ...emptyFullAutoWorkspaceState(),
        launcher: { ...emptyFullAutoWorkspaceState().launcher, title: "My run", objective: "My objective", doneCondition: "My done condition", workspaceRef: "/my/ws", turnCapText: "7" },
      },
    })} report={report} />);
    expect((container.querySelector("#full-auto-launcher-title") as HTMLInputElement).value).toBe("My run");
    expect((container.querySelector("#full-auto-launcher-objective") as HTMLTextAreaElement).value).toBe("My objective");
    expect((container.querySelector("#full-auto-launcher-done-condition") as HTMLTextAreaElement).value).toBe("My done condition");
    expect((container.querySelector("#full-auto-launcher-workspace") as HTMLInputElement).value).toBe("/my/ws");
    expect((container.querySelector("#full-auto-launcher-turn-cap") as HTMLInputElement).value).toBe("7");
  });

  test("Cancel dispatches DesktopFullAutoLauncherCancelled", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={fixtureState()} report={report} />);
    const cancel = container.querySelector('[data-en-key="full-auto-launcher-cancel"]') as HTMLButtonElement;
    await interact(() => cancel.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoLauncherCancelled", payload: null });
  });

  test("monitor shows every active run and can open or stop a run by runRef", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    const first = baseRun({ runRef: "run-1", title: "First active" });
    const second = baseRun({ runRef: "run-2", threadRef: "thread-2", title: "Second active", state: "paused" });
    await render(root, <ReactFullAutoSurface state={fixtureState({
      fullAuto: { ...emptyFullAutoWorkspaceState(), runs: [first, second] },
    })} report={report} />);
    expect(container.querySelector(".oa-react-full-auto-monitor")?.textContent).toContain("2 active");
    const open = container.querySelector('[aria-label="Open Second active"]') as HTMLButtonElement;
    await interact(() => open.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoRunOpened", payload: "run-2" });
    const stop = container.querySelector('[data-en-key="full-auto-run-stop-run-1"]') as HTMLButtonElement;
    await interact(() => stop.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoRunStopByRefRequested", payload: "run-1" });
  });
});

describe("ReactFullAutoSurface: read-only run view (FA-AC-55, FA-AC-56)", () => {
  const runState = (run: FullAutoControlRun): DesktopShellState => fixtureState({
    fullAuto: { ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: run.runRef, runs: [run] },
  });

  test("pins title, objective, done condition, workspace, provider, and explicit lifecycle state", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={runState(baseRun())} report={report} />);
    expect(container.textContent).toContain("Ship the thing");
    expect(container.textContent).toContain("Ship the thing end to end.");
    expect(container.textContent).toContain("pnpm run check is green on main.");
    expect(container.textContent).toContain("/workspace/repo");
    expect(container.textContent).toContain("codex-local");
    const badge = container.querySelector('[data-en-key="full-auto-run-state"]');
    expect(badge?.textContent).toBe("Running");
  });

  test("the ordinary chat composer is genuinely absent from the run view", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={runState(baseRun())} report={report} />);
    expect(container.querySelector('[data-en-key="shell-composer"]')).toBeNull();
    expect(container.querySelector('form[data-chat-composer-form="true"]')).toBeNull();
    expect(container.querySelector('[data-en-key="shell-full-auto-toggle"]')).toBeNull();
  });

  test("Pause is primary while Running and dispatches DesktopFullAutoRunPauseRequested; Resume appears only once Paused", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={runState(baseRun({ state: "running" }))} report={report} />);
    expect(container.querySelector('[data-en-key="full-auto-run-resume"]')).toBeNull();
    const pause = container.querySelector('[data-en-key="full-auto-run-pause"]') as HTMLButtonElement;
    expect(pause).not.toBeNull();
    await interact(() => pause.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoRunPauseRequested", payload: null });

    await render(root, <ReactFullAutoSurface state={runState(baseRun({ state: "paused" }))} report={report} />);
    expect(container.querySelector('[data-en-key="full-auto-run-pause"]')).toBeNull();
    const resume = container.querySelector('[data-en-key="full-auto-run-resume"]') as HTMLButtonElement;
    await interact(() => resume.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoRunResumeRequested", payload: null });
    const handoff = container.querySelector('[data-en-key="full-auto-run-handoff"]') as HTMLButtonElement;
    expect(handoff.textContent).toContain("Switch to Claude");
    await interact(() => handoff.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoRunHandoffRequested", payload: "fable-local" });
  });

  test("Stop is visibly distinct (destructive) and separate from Pause, dispatches DesktopFullAutoRunStopRequested", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={runState(baseRun({ state: "running" }))} report={report} />);
    const stop = container.querySelector('[data-en-key="full-auto-run-stop"]') as HTMLButtonElement;
    expect(stop).not.toBeNull();
    expect(stop.getAttribute("aria-label")).toContain("cannot be undone");
    await interact(() => stop.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoRunStopRequested", payload: null });
  });

  test("Stop is absent once the run is terminal (completed/failed/stopped/cap_reached)", async () => {
    for (const state of ["completed", "failed", "stopped", "cap_reached"] as const) {
      const { container } = installDom();
      const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
      const { report } = recorder();
      const root = createTestRoot(container);
      await render(root, <ReactFullAutoSurface state={runState(baseRun({ state }))} report={report} />);
      expect(container.querySelector('[data-en-key="full-auto-run-stop"]')).toBeNull();
      expect(container.querySelector('[data-en-key="full-auto-run-pause"]')).toBeNull();
      expect(container.querySelector('[data-en-key="full-auto-run-resume"]')).toBeNull();
    }
  });

  test("a stalled run explains cause and next retry, and offers Retry now only when recoverable", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={runState(baseRun({
      state: "stalled",
      stallCause: "dispatch_overdue",
      nextRetryAt: "2026-07-17T01:00:00.000Z",
      recoveryAction: "retry_now",
    }))} report={report} />);
    expect(container.textContent).toContain("dispatch overdue");
    expect(container.textContent).toContain("2026-07-17T01:00:00.000Z");
    const retry = container.querySelector('[data-en-key="full-auto-run-retry-now"]') as HTMLButtonElement;
    expect(retry).not.toBeNull();
    await interact(() => retry.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoRunRetryNowRequested", payload: null });
  });

  // FA-UX-02 (#8997) oracle -- behavior contract
  // openagents_desktop.full_auto_run_view_canonical_timeline.v1: the run view
  // composes the SAME canonical ConversationTimeline component ordinary chats
  // render (proven by the message-scroller element it alone mounts), fed by
  // the bound thread's notes, with no composer.
  test("FA-UX-02: the run view mounts the canonical thread timeline for the bound thread's conversation", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    const run = baseRun({ state: "running", threadRef: "thread-1" });
    await render(root, <ReactFullAutoSurface state={fixtureState({
      fullAuto: { ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: run.runRef, runs: [run] },
      activeThreadId: "thread-1",
      notes: [
        { key: "n1", role: "user", text: "Continue Full Auto: do the next thing.", timestamp: "02:18" },
        { key: "n2", role: "assistant", text: "Shipped the next packet.", timestamp: "02:23" },
      ],
    })} report={report} />);
    // The canonical timeline component (not a parallel mini-renderer).
    expect(container.querySelector('[data-slot="message-scroller"]')).not.toBeNull();
    expect(container.textContent).toContain("Shipped the next packet.");
    // Read-only: still no composer (CUT-DSK-06).
    expect(container.querySelector('form[data-chat-composer-form="true"]')).toBeNull();
  });

  test("FA-UX-02: with no conversation yet, the run view says so instead of mounting an empty-chat CTA", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={runState(baseRun({ state: "running" }))} report={report} />);
    expect(container.textContent).toContain("No conversation yet.");
    expect(container.querySelector('[data-slot="message-scroller"]')).toBeNull();
  });

  test("FA-UX-02: turn rows render provider chip + disposition + relative time/duration, never raw ISO concatenation", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    const run = baseRun({ state: "running" });
    await render(root, <ReactFullAutoSurface state={fixtureState({
      fullAuto: {
        ...emptyFullAutoWorkspaceState(),
        mode: "run",
        activeRunRef: run.runRef,
        runs: [run],
        activeReport: {
          turns: [{
            turnRef: "turn.full-auto.1",
            lane: "codex-local",
            outcomeSummary: "turn completed",
            createdAt: new Date(Date.now() - 10 * 60_000).toISOString(),
            updatedAt: new Date(Date.now() - 5 * 60_000).toISOString(),
          }],
          providerTransitions: [],
        },
      },
    })} report={report} />);
    const row = container.querySelector(".oa-react-full-auto-turn");
    expect(row).not.toBeNull();
    expect(row!.querySelector('[data-slot="badge"]')?.textContent ?? row!.textContent).toContain("codex-local");
    expect(row!.textContent).toContain("turn completed");
    const time = row!.querySelector(".oa-react-full-auto-turn-time");
    expect(time?.textContent).toContain("ago");
    expect(time?.textContent).not.toContain("T");
    expect(row!.textContent).not.toContain("→");
  });
});

describe("ReactFullAutoSurface: FA-WIRE-01 (#8996) ordered fallback lanes + guardrails in the launcher", () => {
  test("adding a fallback lane dispatches the intent; the rotation order list renders with a working Remove", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={fixtureState()} report={report} />);
    const select = container.querySelector('[data-en-key="full-auto-launcher-fallback-add"]') as HTMLSelectElement;
    expect(select).not.toBeNull();
    await interact(() => {
      select.value = "acp:grok-cli";
      select.dispatchEvent(new (globalThis as unknown as { Event: typeof Event }).Event("change", { bubbles: true }));
    });
    expect(received).toContainEqual({ name: "DesktopFullAutoLauncherFallbackLaneAdded", payload: "acp:grok-cli" });

    await render(root, <ReactFullAutoSurface state={fixtureState({
      fullAuto: {
        ...emptyFullAutoWorkspaceState(),
        launcher: { ...emptyFullAutoWorkspaceState().launcher, fallbackLanes: ["acp:grok-cli"] },
      },
    })} report={report} />);
    const order = container.querySelector('[data-en-key="full-auto-launcher-rotation-order"]');
    expect(order).not.toBeNull();
    expect(order!.textContent).toContain("Codex");
    expect(order!.textContent).toContain("Grok CLI");
    const remove = container.querySelector('[data-en-key="full-auto-launcher-fallback-remove-acp:grok-cli"]') as HTMLButtonElement;
    expect(remove).not.toBeNull();
    await interact(() => remove.click());
    expect(received).toContainEqual({ name: "DesktopFullAutoLauncherFallbackLaneRemoved", payload: "acp:grok-cli" });
  });

  test("the max wall clock field renders the draft value", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={fixtureState({
      fullAuto: {
        ...emptyFullAutoWorkspaceState(),
        launcher: { ...emptyFullAutoWorkspaceState().launcher, maxWallClockMinutesText: "120" },
      },
    })} report={report} />);
    expect((container.querySelector("#full-auto-launcher-max-wall-clock") as HTMLInputElement).value).toBe("120");
  });

  test("an action error is surfaced without hiding the pinned mission contract", async () => {
    const { container } = installDom();
    const { ReactFullAutoSurface } = await import("./react-full-auto-surface.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactFullAutoSurface state={fixtureState({
      fullAuto: { ...emptyFullAutoWorkspaceState(), mode: "run", activeRunRef: "run-1", runs: [baseRun()], actionError: "Pause is not legal from state stopped." },
    })} report={report} />);
    expect(container.textContent).toContain("Pause is not legal from state stopped.");
    expect(container.textContent).toContain("Ship the thing end to end.");
  });
});
