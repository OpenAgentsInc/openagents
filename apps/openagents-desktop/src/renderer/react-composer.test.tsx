import { resolveIntentRef, type IntentReporter } from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";
import { Window } from "happy-dom";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { desktopCommandRegistry } from "./command-registry.ts";
import { initialDesktopShellState, type DesktopShellState } from "./shell.ts";

const restores: Array<() => void> = [];
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
  await new Promise((resolve) => setTimeout(resolve, 0));
  restores
    .splice(0)
    .reverse()
    .forEach((restore) => restore());
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

describe("React Codex composer", () => {
  test("focuses on entry, grows within bounds, and sends one exact intent", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createRoot(container);
    root.render(<ReactComposer state={fixtureState({ input: "Ship it" })} report={report} />);
    await settle();
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;
    Object.defineProperty(textarea, "scrollHeight", { configurable: true, value: 240 });
    root.render(<ReactComposer state={fixtureState({ input: "Ship it now" })} report={report} />);
    await settle();
    expect(window.document.activeElement).toBe(textarea);
    expect(textarea.style.height).toBe("180px");
    expect(textarea.style.overflowY).toBe("auto");
    textarea.dispatchEvent(
      new window.KeyboardEvent("keydown", { key: "Enter", bubbles: true }) as unknown as Event,
    );
    [...container.querySelectorAll("button")]
      .find((button) => button.textContent === "Send")
      ?.click();
    await settle();
    expect(received.filter((value) => value.name === "DesktopNoteSubmitted")).toEqual([
      { name: "DesktopNoteSubmitted", payload: "Ship it now" },
    ]);
    root.unmount();
  });

  test("does not submit during IME composition and preserves Shift+Enter", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createRoot(container);
    root.render(<ReactComposer state={fixtureState({ input: "入力" })} report={report} />);
    await settle();
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;
    const composingEnter = new window.KeyboardEvent("keydown", { key: "Enter", bubbles: true });
    Object.defineProperty(composingEnter, "isComposing", { configurable: true, value: true });
    textarea.dispatchEvent(composingEnter as unknown as Event);
    textarea.dispatchEvent(
      new window.KeyboardEvent("keydown", {
        key: "Enter",
        shiftKey: true,
        bubbles: true,
      }) as unknown as Event,
    );
    await settle();
    expect(received.some((value) => value.name.includes("Submitted"))).toBe(false);
    root.unmount();
  });

  test("maps streaming controls to stop, steer, queue, and mode intents", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createRoot(container);
    root.render(
      <ReactComposer
        state={fixtureState({ input: "Continue", pending: true, pendingSubmitMode: "steer" })}
        report={report}
      />,
    );
    await settle();
    const click = (label: string, last = false) => {
      const buttons = [...container.querySelectorAll("button")].filter(
        (button) => button.textContent === label,
      );
      (last ? buttons.at(-1) : buttons[0])?.click();
    };
    click("Stop");
    click("Queue");
    click("Steer", true);
    await settle();
    expect(received).toEqual(
      expect.arrayContaining([
        { name: "DesktopTurnInterrupted", payload: null },
        { name: "DesktopPendingSubmitModeSelected", payload: "queue" },
        { name: "DesktopSteerCurrentRequested", payload: "Continue" },
      ]),
    );
    root.unmount();
  });
});

describe("React command and decision surfaces", () => {
  test("renders the canonical registry and dispatches the exact command identity", async () => {
    const { window, container } = installDom();
    const { ReactCommandPalette } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createRoot(container);
    root.render(
      <ReactCommandPalette state={fixtureState({ commandPaletteOpen: true })} report={report} />,
    );
    await settle();
    expect(window.document.querySelectorAll("[data-slot=command-item]").length).toBe(
      desktopCommandRegistry.length,
    );
    const commandItems = [
      ...window.document.querySelectorAll("[data-slot=command-item]"),
    ] as unknown as Array<HTMLElement>;
    const newChat = commandItems.find((item) => item.textContent?.includes("New chat"));
    newChat?.click();
    await settle();
    expect(received).toEqual(
      expect.arrayContaining([
        { name: "DesktopNewChat", payload: null },
        { name: "DesktopCommandPaletteDismissed", payload: null },
      ]),
    );
    root.unmount();
    container.remove();
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
    const root = createRoot(container);
    root.render(<DecisionSurface state={state} report={report} />);
    await settle();
    expect(window.document.body.textContent).toContain("did not accept");
    const approve = [...window.document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Approve"),
    );
    approve?.click();
    await settle();
    expect(received).toContainEqual({ name: "DesktopApprovalApproved", payload: "decision-1" });
    root.unmount();
    container.remove();
  });
});
