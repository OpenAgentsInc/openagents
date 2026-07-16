import { resolveIntentRef, type IntentReporter } from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";
import { Window } from "happy-dom";
import { act, type ReactNode } from "react";
import type { Root } from "react-dom/client";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { initialDesktopShellState, type DesktopShellState } from "./shell.ts";
import { CODEX_CHIP_REASON_VERIFYING } from "../codex-local-contract.ts";

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
    Event: window.Event,
    InputEvent: window.InputEvent,
    CompositionEvent: window.CompositionEvent,
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
  test("keeps initial Codex probing compact and local to the composer", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    const base = fixtureState();
    await render(root, <ReactComposer state={{
      ...base,
      harnessLanes: { ...base.harnessLanes, codex: { available: false, reason: CODEX_CHIP_REASON_VERIFYING } },
    }} report={report} />);
    const status = container.querySelector('[data-codex-status="checking"]');
    expect(status?.getAttribute("data-slot")).toBe("badge");
    expect(status?.textContent).toBe("Checking Codex…");
    expect(status?.querySelector(".oa-react-composer-status-dot")).not.toBeNull();
  });

  test("keeps first-submit enabled while startup thread admission is still pending", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactComposer state={fixtureState({ activeThreadId: null, threads: [], input: "Start now" })} report={report} />);
    const send = container.querySelector('[aria-label="Send"]') as HTMLButtonElement;
    expect(send.disabled).toBe(false);
    await interact(() => send.click());
    expect(received).toContainEqual({ name: "DesktopNoteSubmitted", payload: "Start now" });
  });

  test("attaches, previews, removes, reports rejection, and sends an image-only turn", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    const image = { id: "image-1", mediaType: "image/png" as const, data: "aGVsbG8=", name: "screen.png", sizeBytes: 5 };
    await render(root, <ReactComposer state={fixtureState({ composerImages: [image], composerImageNotice: "That image is too large." })} report={report} />);
    expect(container.querySelector('form[data-chat-composer-form="true"]')).not.toBeNull();
    expect(container.querySelector('[data-chat-composer-footer="true"]')).not.toBeNull();
    expect(container.querySelector('[data-en-key="shell-composer"] [data-en-key="shell-input"] [contenteditable="true"]')).not.toBeNull();
    expect(container.querySelector('img[src="data:image/png;base64,aGVsbG8="]')).not.toBeNull();
    expect(container.textContent).toContain("screen.png");
    expect(container.textContent).not.toContain("aGVsbG8=");
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("too large");
    const attach = container.querySelector('[aria-label="Attach images"]') as HTMLButtonElement;
    const remove = container.querySelector('[aria-label="Remove screen.png"]') as HTMLButtonElement;
    const send = container.querySelector('[aria-label="Send"]') as HTMLButtonElement;
    expect(send.disabled).toBe(false);
    await interact(() => { attach.click(); remove.click(); send.click(); });
    expect(received).toEqual(expect.arrayContaining([
      { name: "DesktopComposerImagePickRequested", payload: null },
      { name: "DesktopComposerImageRemoved", payload: "image-1" },
      { name: "DesktopNoteSubmitted", payload: null },
    ]));
  });

  test("disables acquisition while pending and at the limit, and exposes drag-over state", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactComposer state={fixtureState({ pending: true })} report={report} />);
    expect((container.querySelector('[aria-label*="current turn"]') as HTMLButtonElement).disabled).toBe(true);
    const images = Array.from({ length: 8 }, (_, index) => ({ id: `i${index}`, mediaType: "image/png" as const, data: "YQ==", name: `${index}.png`, sizeBytes: 1 }));
    await render(root, <ReactComposer state={fixtureState({ composerImages: images })} report={report} />);
    expect((container.querySelector('[aria-label="Image limit reached (8 max)"]') as HTMLButtonElement).disabled).toBe(true);
    await render(root, <ReactComposer state={fixtureState()} report={report} />);
    const composer = container.querySelector('[data-en-key="shell-composer"]') as HTMLElement;
    const drag = new window.Event("dragenter", { bubbles: true });
    Object.defineProperty(drag, "dataTransfer", { configurable: true, value: { types: ["Files"] } });
    await interact(() => composer.dispatchEvent(drag as unknown as Event));
    expect(composer.dataset.dragActive).toBe("true");
    expect(container.textContent).toContain("Drop images to attach");
  });

  test("focuses the Lexical editor, preserves its bounded shell, and sends one exact intent", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(
      root,
      <ReactComposer state={fixtureState({ input: "Ship it" })} report={report} />,
    );
    const editor = container.querySelector('[data-lexical-composer="true"]') as HTMLElement;
    expect(container.querySelector('[data-en-key="shell-input"] [contenteditable="true"]')).toBe(editor);
    expect(container.querySelector('[data-icon-name="Command"]')).not.toBeNull();
    expect(container.querySelector('[data-icon-name="ArrowUp"]')).not.toBeNull();
    expect(container.querySelectorAll('[data-composer-button-kind="action"]')).toHaveLength(2);
    expect(container.querySelector('[data-composer-button-kind="toggle"]')).not.toBeNull();
    expect(container.querySelector('[data-composer-button-kind="submit"]')).not.toBeNull();
    await render(
      root,
      <ReactComposer state={fixtureState({ input: "Ship it now" })} report={report} />,
    );
    expect(window.document.activeElement).toBe(editor);
    expect(editor.classList.contains("oa-lexical-composer-content")).toBe(true);
    expect(editor.textContent).toBe("Ship it now");
    await interact(() => {
      editor.dispatchEvent(
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
    const editor = container.querySelector('[data-lexical-composer="true"]') as HTMLElement;
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
    expect(window.document.activeElement).toBe(editor);
  });

  test("does not submit during IME composition and preserves Shift+Enter", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactComposer state={fixtureState({ input: "入力" })} report={report} />);
    const editor = container.querySelector('[data-lexical-composer="true"]') as HTMLElement;
    const composingEnter = new window.KeyboardEvent("keydown", { key: "Enter", bubbles: true });
    Object.defineProperty(composingEnter, "isComposing", { configurable: true, value: true });
    await interact(() => {
      editor.dispatchEvent(composingEnter as unknown as Event);
      editor.dispatchEvent(
        new window.KeyboardEvent("keydown", {
          key: "Enter",
          shiftKey: true,
          bubbles: true,
        }) as unknown as Event,
      );
    });
    expect(received.some((value) => value.name.includes("Submitted"))).toBe(false);
  });

  test("keeps Lexical controlled with caret-safe hydration and keyboard history", async () => {
    const { window, container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactComposer state={fixtureState({ input: "Initial" })} report={report} />);
    const editor = container.querySelector('[data-lexical-composer="true"]') as HTMLElement & { value: string };
    expect(editor.getAttribute("role")).toBe("textbox");
    expect(editor.getAttribute("aria-multiline")).toBe("true");
    expect(editor.textContent).toBe("Initial");

    await interact(() => {
      editor.value = "Local edit";
    });
    expect(received).toContainEqual({ name: "DesktopInputChanged", payload: "Local edit" });

    await interact(() => {
      editor.dispatchEvent(new window.KeyboardEvent("keydown", {
        key: "z",
        ctrlKey: true,
        bubbles: true,
      }) as unknown as Event);
    });
    expect(editor.textContent).toBe("Initial");
    await interact(() => {
      editor.dispatchEvent(new window.KeyboardEvent("keydown", {
        key: "z",
        ctrlKey: true,
        shiftKey: true,
        bubbles: true,
      }) as unknown as Event);
    });
    expect(editor.textContent).toBe("Local edit");

    const text = editor.querySelector("p span")?.firstChild;
    expect(text).not.toBeNull();
    const selection = window.getSelection();
    const range = window.document.createRange();
    range.setStart(text as never, 3);
    range.collapse(true);
    selection?.removeAllRanges();
    selection?.addRange(range);
    editor.dispatchEvent(new window.Event("selectionchange", { bubbles: true }) as unknown as Event);

    const changeCount = received.filter((value) => value.name === "DesktopInputChanged").length;
    await render(root, <ReactComposer state={fixtureState({ input: "Server hydration" })} report={report} />);
    expect(container.querySelector('[data-lexical-composer="true"]')).toBe(editor);
    expect(editor.textContent).toBe("Server hydration");
    expect(window.getSelection()?.anchorOffset).toBe(3);
    expect(received.filter((value) => value.name === "DesktopInputChanged")).toHaveLength(changeCount);
  });

  test("maps streaming controls to stop, steer, queue, and mode intents", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(
      root,
      <ReactComposer
        state={fixtureState({ input: "Continue", pending: true, pendingSubmitMode: "steer", composerAdmission: { state: "active_steerable", activeTurnId: "turn-provider-7", reason: null, queuedCount: 0 } })}
        report={report}
      />,
    );
    expect(container.querySelector('[data-icon-name="Stop"]')).not.toBeNull();
    const click = (label: string, last = false) => {
      const buttons = [...container.querySelectorAll("button")].filter(
        (button) => button.textContent === label,
      );
      (last ? buttons.at(-1) : buttons[0])?.click();
    };
    await interact(() => {
      click("Stop");
      click("Queue next");
      click("Steer now", true);
      click("Steer", true);
    });
    expect(received).toEqual(
      expect.arrayContaining([
        { name: "DesktopTurnInterrupted", payload: null },
        { name: "DesktopPendingSubmitModeSelected", payload: "queue" },
        { name: "DesktopSteerCurrentRequested", payload: "Continue" },
      ]),
    );
    expect(container.textContent).toContain("Sends into active turn turn-provider-7")
  });

  test("projects durable queue order and disables mutation after dispatch ownership transfers", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    const entry = (queueRef: string, position: number, status: "queued" | "promoting") => ({
      queueRef, intentRef: `intent-${queueRef}`, clientUserMessageId: `user-${queueRef}`,
      threadRef: "thread-1", message: `Message ${queueRef}`, position, status, revision: 0,
      quiescenceRef: null, providerTurnId: null, failure: null,
      createdAt: "2026-07-15T00:00:00.000Z", updatedAt: "2026-07-15T00:00:00.000Z",
    });
    await render(root, <ReactComposer state={fixtureState({ composerQueue: [entry("one", 1, "queued"), entry("two", 0, "promoting")] })} report={report} />);
    expect(container.textContent).toContain("#1Message onepending");
    expect(container.textContent).toContain("Message twodispatching");
    const edit = [...container.querySelectorAll("button")].find(button => button.textContent === "Edit" && !button.disabled);
    await interact(() => edit?.click());
    expect(received).toContainEqual({ name: "DesktopQueuedIntentEditRequested", payload: "one" });
    expect([...container.querySelectorAll('button[title="This turn is already dispatching"]')]).toHaveLength(2);
  });

  test("Full Auto (#8852, FA-H1 #8874): renders the ACTIVE thread's per-thread toggle state and reports DesktopFullAutoToggled", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(root, <ReactComposer state={fixtureState({ fullAutoByThread: {} })} report={report} />);
    const toggle = container.querySelector('[data-en-key="shell-full-auto-toggle"]');
    expect(toggle).not.toBeNull();
    // An absent entry is honestly off.
    expect(toggle?.getAttribute("aria-pressed")).toBe("false");
    expect(toggle?.getAttribute("aria-label")).toBe("Turn on Full Auto");
    await interact(() => {
      (toggle as HTMLButtonElement).click();
    });
    expect(received).toEqual(
      expect.arrayContaining([{ name: "DesktopFullAutoToggled", payload: null }]),
    );
    // The hydrated durable entry for the active thread drives the pressed
    // state and the label — one click on this honestly means "turn it off".
    await render(root, <ReactComposer state={fixtureState({ fullAutoByThread: { "thread-1": true } })} report={report} />);
    const pressed = container.querySelector('[data-en-key="shell-full-auto-toggle"]');
    expect(pressed?.getAttribute("aria-pressed")).toBe("true");
    expect(pressed?.getAttribute("aria-label")).toBe("Turn off Full Auto");
    // ANOTHER thread's enabled entry never leaks into this thread's toggle.
    await render(root, <ReactComposer state={fixtureState({ fullAutoByThread: { "thread-2": true } })} report={report} />);
    expect(
      container.querySelector('[data-en-key="shell-full-auto-toggle"]')?.getAttribute("aria-pressed"),
    ).toBe("false");
  });

  test("FA-H4 (#8877): a running background Full Auto turn renders the status badge and the Stop affordance; idle renders neither", async () => {
    const { container } = installDom();
    const { ReactComposer } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    // Idle (no live entry): no badge, no Stop.
    await render(root, <ReactComposer state={fixtureState()} report={report} />);
    expect(container.querySelector('[data-full-auto-status="running"]')).toBeNull();
    expect(container.querySelector('[aria-label="Stop current turn"]')).toBeNull();
    // Background turn running (renderer NOT pending): badge + Stop render,
    // and Stop reports the same DesktopTurnInterrupted intent whose handler
    // targets the actual background turn.
    await render(root, <ReactComposer state={fixtureState({
      pending: false,
      fullAutoLiveByThread: { "thread-1": { state: "turn_running", turnRef: "turn.full-auto.bg-1" } },
    })} report={report} />);
    const badge = container.querySelector('[data-full-auto-status="running"]');
    expect(badge?.getAttribute("data-slot")).toBe("badge");
    expect(badge?.textContent).toBe("Full Auto running…");
    const stop = container.querySelector('[aria-label="Stop current turn"]') as HTMLButtonElement;
    expect(stop).not.toBeNull();
    await interact(() => stop.click());
    expect(received).toContainEqual({ name: "DesktopTurnInterrupted", payload: null });
    // A terminal live state clears both again.
    await render(root, <ReactComposer state={fixtureState({
      fullAutoLiveByThread: { "thread-1": { state: "turn_completed", turnRef: null } },
    })} report={report} />);
    expect(container.querySelector('[data-full-auto-status="running"]')).toBeNull();
    expect(container.querySelector('[aria-label="Stop current turn"]')).toBeNull();
    // ANOTHER thread's running turn never leaks into this thread's composer.
    await render(root, <ReactComposer state={fixtureState({
      fullAutoLiveByThread: { "thread-2": { state: "turn_running", turnRef: "turn.full-auto.bg-2" } },
    })} report={report} />);
    expect(container.querySelector('[data-full-auto-status="running"]')).toBeNull();
  });
});

describe("React command and decision surfaces", () => {
  test("does no catalog work while closed and bounds recent-session projection when open", async () => {
    const { container } = installDom();
    const { ReactCommandPalette, projectRecentCommandSessions } = await import("./react-composer.tsx");
    const { report } = recorder();
    let rootReads = 0;
    const roots = new Proxy(Array.from({ length: 10_000 }, (_, index) => ({
      threadRef: `history-${index}`,
      parentThreadRef: null,
      title: `History ${index}`,
      status: "completed" as const,
      createdAt: new Date(10_000 - index).toISOString(),
      updatedAt: new Date(10_000 - index).toISOString(),
      depth: 0,
      descendantCount: 0,
      model: null,
      role: null,
      nickname: null,
      agentPath: null,
      sourceVersion: null,
      reasoning: null,
      source: "codex" as const,
    })), {
      get(target, property, receiver) {
        if (typeof property === "string" && /^\d+$/u.test(property)) rootReads += 1;
        return Reflect.get(target, property, receiver);
      },
    });
    const base = fixtureState();
    const state = { ...base, commandPaletteOpen: false, history: {
      ...base.history,
      catalog: { roots, agents: [] },
    } };
    const root = createTestRoot(container);
    await render(root, <ReactCommandPalette state={state} report={report} />);
    expect(rootReads).toBe(0);
    expect(container.textContent).toBe("");

    const projected = projectRecentCommandSessions(state, "");
    expect(projected).toHaveLength(6);
    expect(rootReads).toBeLessThanOrEqual(24);
  });

  test("groups only available OpenAgents actions and real recent sessions in the T3-shaped palette", async () => {
    const { window, container } = installDom();
    const { ReactCommandPalette } = await import("./react-composer.tsx");
    const { received, report } = recorder();
    const root = createTestRoot(container);
    await render(
      root,
      <ReactCommandPalette state={fixtureState({ commandPaletteOpen: true })} report={report} />,
    );
    expect(window.document.querySelector('[data-command-palette="true"]')).not.toBeNull();
    const paletteInput = window.document.querySelector('[data-slot="command-input"]') as unknown as HTMLInputElement | null;
    expect(paletteInput?.placeholder).toBe(
      "Search commands and sessions…",
    );
    const headings = [...window.document.querySelectorAll("[cmdk-group-heading]")].map(
      (heading) => heading.textContent,
    );
    expect(headings).toEqual(["Actions", "Recent Sessions"]);
    expect(window.document.querySelector('[data-slot="command-footer"]')?.textContent).toContain(
      "NavigateEnterSelectEscClose",
    );
    const commandItems = [
      ...window.document.querySelectorAll("[data-slot=command-item]"),
    ] as unknown as Array<HTMLElement>;
    const newChat = commandItems.find((item) => item.textContent?.includes("New chat"));
    expect(newChat?.querySelector("svg")).not.toBeNull();
    expect(container.textContent).not.toContain("Add project");
    expect(container.textContent).not.toContain("New thread in");
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
