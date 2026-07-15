import { resolveIntentRef, type IntentReporter } from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";
import { Window } from "happy-dom";
import { act, type ReactNode } from "react";
import type { Root } from "react-dom/client";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { desktopCommandRegistry } from "./command-registry.ts";
import { initialDesktopShellState, type DesktopShellState } from "./shell.ts";

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
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLInputElement: window.HTMLInputElement,
    HTMLTextAreaElement: window.HTMLTextAreaElement,
    Event: window.Event,
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

const fixtureState = (extra: Partial<DesktopShellState> = {}): DesktopShellState => {
  const base = initialDesktopShellState("electron/darwin");
  return {
    ...base,
    activeThreadId: "thread-1",
    threads: [{ id: "thread-1", title: "Test", updatedAt: "2026-07-14T12:00:00.000Z", notes: [] }],
    selectedHarness: "codex",
    harnessLanes: { ...base.harnessLanes, codex: { available: true, reason: null } },
    ...extra,
  };
};

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

describe("React Codex composer", () => {
  test("focuses on entry, grows within bounds, and sends one exact intent", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(
      root,
      <ReactComposer state={fixtureState({ input: "Ship it" })} report={report} />,
    );
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;
    expect(container.querySelector('[data-en-key="shell-input"] textarea')).toBe(textarea);
    Object.defineProperty(textarea, "scrollHeight", { configurable: true, value: 240 });
    await render(
      root,
      <ReactComposer state={fixtureState({ input: "Ship it now" })} report={report} />,
    );
    expect(window.document.activeElement).toBe(textarea);
    expect(textarea.style.height).toBe("180px");
    expect(textarea.style.overflowY).toBe("auto");
    await interact(() => {
      textarea.dispatchEvent(
        new window.KeyboardEvent("keydown", { key: "Enter", bubbles: true }) as unknown as Event,
      );
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "Send")
        ?.click();
    });
    expect(received.filter((value) => value.name === "DesktopNoteSubmitted")).toEqual([
      { name: "DesktopNoteSubmitted", payload: "Ship it now" },
    ]);
  });

  test("focuses the composer after a new-session transition even when the trigger owns focus", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactComposer state={fixtureState()} report={report} />);
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;
    const trigger = window.document.createElement("button");
    window.document.body.appendChild(trigger);
    trigger.focus();
    expect(window.document.activeElement).toBe(trigger);

    await render(
      root,
      <ReactComposer
        state={fixtureState({ activeThreadId: null, threads: [], history: {
          ...fixtureState().history,
          page: null,
        } })}
        report={report}
      />,
    );

    expect(window.document.activeElement).toBe(textarea);
  });

  test("does not submit during IME composition and preserves Shift+Enter", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactComposer state={fixtureState({ input: "入力" })} report={report} />);
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;
    const composingEnter = new window.KeyboardEvent("keydown", { key: "Enter", bubbles: true });
    Object.defineProperty(composingEnter, "isComposing", { configurable: true, value: true });
    await interact(() => {
      textarea.dispatchEvent(composingEnter as unknown as Event);
      textarea.dispatchEvent(
        new window.KeyboardEvent("keydown", {
          key: "Enter",
          shiftKey: true,
          bubbles: true,
        }) as unknown as Event,
      );
    });
    expect(received.some((value) => value.name.includes("Submitted"))).toBe(false);
  });

  test("maps streaming controls to stop, steer, queue, and mode intents", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(
      root,
      <ReactComposer
        state={fixtureState({ input: "Continue", pending: true, pendingSubmitMode: "steer" })}
        report={report}
      />,
    );
    const click = (label: string, last = false) => {
      const buttons = [...container.querySelectorAll("button")].filter(
        (button) => button.textContent === label,
      );
      (last ? buttons.at(-1) : buttons[0])?.click();
    };
    await interact(() => {
      click("Stop");
      click("Queue");
      click("Steer", true);
    });
    expect(received).toEqual(
      expect.arrayContaining([
        { name: "DesktopTurnInterrupted", payload: null },
        { name: "DesktopPendingSubmitModeSelected", payload: "queue" },
        { name: "DesktopSteerCurrentRequested", payload: "Continue" },
      ]),
    );
  });
});

describe("React command and decision surfaces", () => {
  test("renders the canonical registry and dispatches the exact command identity", async () => {
    const { window, container } = installDom();
    const { ReactCommandPalette } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(
      root,
      <ReactCommandPalette state={fixtureState({ commandPaletteOpen: true })} report={report} />,
    );
    expect(window.document.querySelectorAll("[data-slot=command-item]").length).toBe(
      desktopCommandRegistry.length,
    );
    const commandItems = [
      ...window.document.querySelectorAll("[data-slot=command-item]"),
    ] as unknown as Array<HTMLElement>;
    const newChat = commandItems.find((item) => item.textContent?.includes("New chat"));
    await interact(() => newChat?.click());
    expect(received).toEqual(
      expect.arrayContaining([
        { name: "DesktopNewChat", payload: null },
        { name: "DesktopCommandPaletteDismissed", payload: null },
      ]),
    );
  });

  test("keeps approval explicit and presents failed bridge attempts without inventing resolution", async () => {
    const { window, container } = installDom();
    const { DecisionSurface } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const question = {
      turnRef: "turn-1",
      questionRef: "decision-1",
      status: "pending" as const,
      kind: "tool_approval" as const,
      questions: [
        {
          question: "Allow the tool?",
          header: "Approval",
          multiSelect: false,
          options: [
            { label: "Approve", description: "Run the requested tool." },
            { label: "Deny", description: "Do not run it." },
          ],
        },
      ],
    };
    const state = fixtureState({
      questionAnswerHostAvailable: true,
      notes: [{ key: "note-1", role: "system", text: "", timestamp: "now", question }],
      questionCards: {
        "decision-1": {
          selections: [[]],
          answered: false,
          submitting: false,
          failure: "answer_refused",
          answers: null,
        },
      },
    });
    const root = createTestRoot(container);
    await render(root, <DecisionSurface state={state} report={report} />);
    expect(window.document.body.textContent).toContain("did not accept");
    const approve = [...window.document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Approve"),
    );
    await interact(() => approve?.click());
    expect(received).toContainEqual({ name: "DesktopApprovalApproved", payload: "decision-1" });
  });
});
