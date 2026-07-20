import { Window } from "happy-dom";
import type { IntentReporter } from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, test } from "vite-plus/test";

import {
  ReactIdeDebugPanel,
  type IdeDebugRendererBridge,
  type IdeDebugRendererCommand,
  type IdeDebugRendererSnapshot,
} from "./react-debug.tsx";
import { ReactTerminalSurface } from "../react-workspace-surfaces.tsx";
import { initialDesktopShellState } from "../shell.ts";

const restores: Array<() => void> = [];

const installDom = () => {
  const window = new Window({ url: "http://localhost/" });
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLButtonElement: window.HTMLButtonElement,
    HTMLInputElement: window.HTMLInputElement,
    Event: window.Event,
    InputEvent: window.InputEvent,
    KeyboardEvent: window.KeyboardEvent,
    MouseEvent: window.MouseEvent,
    IS_REACT_ACT_ENVIRONMENT: true,
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
      if (descriptor === undefined) Reflect.deleteProperty(globalThis, name);
      else Object.defineProperty(globalThis, name, descriptor);
    }
  });
  const container = window.document.createElement("div") as unknown as HTMLDivElement;
  window.document.body.appendChild(container as never);
  return { container, window };
};

afterEach(async () => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  restores
    .splice(0)
    .toReversed()
    .forEach((restore) => restore());
});

const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 30));
const interact = async (interaction: () => void): Promise<void> => {
  await act(async () => {
    interaction();
    await settle();
  });
};

const fixtureSnapshot = (
  serviceGeneration = 7,
  sessionGeneration = 3,
): IdeDebugRendererSnapshot => {
  const binding = {
    projectRef: "ide.project.fixture",
    rootRef: "ide.root.fixture",
    worktreeRef: "ide.worktree.fixture",
    attachmentGeneration: 2,
    languageGeneration: 4,
    placementGeneration: 5,
    serviceGeneration,
    placementRef: "ide.placement.desktop-local",
    language: "typescript",
  } as const;
  const source = {
    sourceRef: "ide.debug-source.app",
    pathRef: "src/app.ts",
    label: "app.ts",
    origin: "project",
    availability: "available",
    sourceMapRef: "source-map.fixture",
    documentGeneration: 11,
  } as const;
  const configuration = {
    configurationRef: "ide.debug-config.fixture",
    configurationGeneration: 6,
    label: "Launch fixture",
    binding,
    intent: {
      _tag: "Launch",
      executableLabel: "Node fixture",
      argumentLabels: ["--inspect", "dist/app.js"],
      prelaunchTaskRef: "task.build",
      postdebugTaskRef: null,
    },
    placement: { _tag: "Local", hostLabel: "Desktop host" },
    adapter: {
      adapterType: "node",
      adapterVersion: "1.2.3",
      transport: "stdio",
      admitted: true,
      capabilities: [
        { capability: "continue", supported: true, reason: null },
        { capability: "pause", supported: true, reason: null },
        { capability: "step_over", supported: true, reason: null },
        { capability: "step_in", supported: true, reason: null },
        { capability: "step_out", supported: true, reason: null },
        { capability: "step_back", supported: false, reason: "Reverse execution is unavailable." },
        { capability: "restart_session", supported: true, reason: null },
        { capability: "disconnect", supported: true, reason: null },
        { capability: "terminate", supported: true, reason: null },
        { capability: "set_variable", supported: true, reason: null },
        { capability: "evaluate", supported: true, reason: null },
        { capability: "modules", supported: true, reason: null },
        { capability: "loaded_sources", supported: true, reason: null },
        { capability: "source_request", supported: true, reason: null },
        { capability: "cancel_request", supported: true, reason: null },
      ],
    },
    targetRef: "ide.debug-target.fixture",
    cwdRef: "project://fixture",
    environment: {
      admittedKeys: ["NODE_ENV"],
      redactedKeys: ["SECRET_TOKEN"],
      sourceRefs: ["secret-ref.fixture"],
      valuesExposedToRenderer: false,
      digest: "environment-digest",
    },
    sourceMaps: {
      sourceRoots: ["src"],
      remoteRootRefs: [],
      generatedSourcesExplicit: true,
      guessPositions: false,
    },
    admitted: true,
    refusalReason: null,
  } as const;
  return {
    schemaVersion: "openagents.desktop.ide-debug.v1",
    binding,
    capabilityState: {
      _tag: "Ready",
      serviceGeneration,
      placementRef: binding.placementRef,
      evidenceTier: "project_local",
      observedAt: "2026-07-20T02:00:00.000Z",
    },
    configurations: [configuration],
    breakpointSets: [
      {
        configurationRef: configuration.configurationRef,
        breakpoints: [],
        updatedAt: "2026-07-20T02:00:00.000Z",
      },
    ],
    sessions: [
      {
        sessionRef: "ide.debug-session.fixture",
        sessionGeneration,
        adapterGeneration: 8,
        targetGeneration: 9,
        configuration,
        lifecycle: { _tag: "Stopped", reason: "breakpoint", threadRef: "ide.debug-thread.main" },
        breakpoints: [
          {
            _tag: "Source",
            breakpointRef: "ide.debug-breakpoint.app-12",
            enabled: true,
            verified: true,
            message: null,
            condition: "count > 2",
            hitCondition: null,
            logMessage: null,
            location: { source, line: 12, column: 1 },
            requestedLine: 12,
          },
        ],
        threads: [
          {
            threadRef: "ide.debug-thread.main",
            name: "Main thread",
            state: "stopped",
            stopReason: "breakpoint",
          },
        ],
        frames: [
          {
            frameRef: "ide.debug-frame.main",
            threadRef: "ide.debug-thread.main",
            name: "main",
            location: { source, line: 12, column: 3 },
            canRestart: true,
          },
        ],
        scopes: [
          {
            scopeRef: "ide.debug-scope.local",
            frameRef: "ide.debug-frame.main",
            name: "Local",
            expensive: false,
            variableCount: 1,
            state: "ready",
          },
        ],
        variables: [
          {
            variableRef: "ide.debug-variable.count",
            scopeRef: "ide.debug-scope.local",
            name: "count",
            value: "3",
            type: "number",
            redacted: false,
            truncated: false,
            childCount: 0,
          },
        ],
        watches: [
          {
            watchRef: "ide.debug-watch.count",
            expression: "count + 1",
            value: "4",
            type: "number",
            state: "ready",
            message: null,
            redacted: false,
            truncated: false,
          },
        ],
        modules: [
          {
            moduleRef: "ide.debug-module.app",
            name: "app",
            pathRef: "dist/app.js",
            version: "1.0.0",
            symbolStatus: "loaded",
          },
        ],
        loadedSources: [
          source,
          {
            ...source,
            sourceRef: "ide.debug-source.remote",
            pathRef: "remote://worker.js",
            label: "worker.js",
            origin: "remote",
            availability: "stale",
            sourceMapRef: null,
            documentGeneration: null,
          },
        ],
        console: [
          {
            sequence: 1,
            category: "stdout",
            text: "fixture ready\n",
            redacted: false,
            truncated: false,
            gapBefore: false,
            observedAt: "2026-07-20T02:00:00.000Z",
          },
        ],
        invalidatedAreas: [],
        retainedConsoleBytes: 14,
        droppedConsoleBytes: 0,
      },
    ],
    receipts: [
      {
        receiptRef: "ide.debug-receipt.launch",
        operationRef: "ide.debug-operation.launch",
        operation: "launch",
        disposition: "succeeded",
        outcome: "started",
        observedAt: "2026-07-20T02:00:00.000Z",
        sessionGeneration,
        targetRef: "ide.debug-target.fixture",
        placementRef: "ide.placement.desktop-local",
      },
    ],
    stopped: false,
  };
};

const bridgeFor = (snapshot: IdeDebugRendererSnapshot) => {
  const commands: Array<IdeDebugRendererCommand> = [];
  let listener: ((event: unknown) => void) | null = null;
  const bridge: IdeDebugRendererBridge = {
    snapshot: async () => snapshot,
    command: async (command) => {
      commands.push(command);
      return { _tag: "Succeeded", snapshot, payload: null };
    },
    onEvent: (candidate) => {
      listener = candidate;
      return () => {
        listener = null;
      };
    },
  };
  return { bridge, commands, emit: (event: unknown): void => listener?.(event) };
};

const clickButton = (container: HTMLElement, label: string): void => {
  const button = [...container.querySelectorAll("button")].find(
    (candidate) => candidate.textContent?.trim() === label,
  );
  if (button === undefined) throw new Error(`Button not found: ${label}`);
  button.click();
};

const changeInput = (input: HTMLInputElement, value: string): void => {
  const setter =
    Object.getOwnPropertyDescriptor(input, "value")?.set ??
    Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), "value")?.set ??
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
};

const installDesktopBridge = (bridge: IdeDebugRendererBridge): void => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, "openagentsDesktop");
  Object.defineProperty(globalThis, "openagentsDesktop", {
    configurable: true,
    writable: true,
    value: { ideDebug: bridge },
  });
  restores.push(() => {
    if (previous === undefined) Reflect.deleteProperty(globalThis, "openagentsDesktop");
    else Object.defineProperty(globalThis, "openagentsDesktop", previous);
  });
};

describe("IDE-11 debugger renderer projection", () => {
  test("mounts Debug in the terminal bottom-panel tab sequence", async () => {
    const { container } = installDom();
    const fixture = bridgeFor(fixtureSnapshot());
    installDesktopBridge(fixture.bridge);
    const report: IntentReporter = () => Effect.void;
    const root = createRoot(container);
    await interact(() =>
      root.render(
        <ReactTerminalSurface
          state={initialDesktopShellState("electron/darwin")}
          report={report}
        />,
      ),
    );
    const tablist = container.querySelector(
      "nav[aria-label='Terminal debug tasks tests and Output']",
    );
    expect(tablist?.textContent).toBe("TerminalDebugTasksTestsOutput");
    await interact(() => clickButton(container, "Debug"));
    expect(container.querySelector(".oa-debug-workbench")?.textContent).toContain("Debugger");
    root.unmount();
  });

  test("discloses effective launch, lifecycle, generations, and secret-safe environment facts", async () => {
    const { container } = installDom();
    const fixture = bridgeFor(fixtureSnapshot());
    const root = createRoot(container);
    root.render(<ReactIdeDebugPanel bridge={fixture.bridge} />);
    await settle();

    expect(container.textContent).toContain("Debugger");
    expect(container.textContent).toContain("project generation 2 · service generation 7");
    expect(container.textContent).toContain("Stopped");
    expect(container.textContent).toContain("session 3 · adapter 8 · target 9");
    expect(container.textContent).toContain("Launch · Launch fixture");
    const disclosure = container.querySelector("details");
    disclosure?.setAttribute("open", "");
    expect(disclosure?.textContent).toContain("Node fixture");
    expect(disclosure?.textContent).toContain(
      "1 admitted key names · 1 redacted key names · values withheld",
    );
    expect(disclosure?.textContent).not.toContain("SECRET_TOKEN");
    expect(disclosure?.textContent).not.toContain("secret-ref.fixture");
    expect(disclosure?.textContent).not.toContain("environment-digest");

    const stepBack = [...container.querySelectorAll("button")].find(
      (button) => button.textContent === "Step back",
    );
    expect(stepBack?.disabled).toBe(true);
    expect(stepBack?.title).toBe("Reverse execution is unavailable.");
    expect(
      [...container.querySelectorAll("button")].find(
        (button) => button.textContent === "Run to cursor",
      )?.disabled,
    ).toBe(true);
    expect(
      [...container.querySelectorAll("button")].find(
        (button) => button.textContent === "Restart frame",
      )?.disabled,
    ).toBe(true);
    expect(container.querySelector(".oa-debug-status[data-tone='warning']")?.textContent).toContain(
      "Stopped",
    );
    root.unmount();
  });

  test("sends generation-fenced control, breakpoint, source, watch, and variable commands", async () => {
    const { container } = installDom();
    const fixture = bridgeFor(fixtureSnapshot());
    const root = createRoot(container);
    root.render(<ReactIdeDebugPanel bridge={fixture.bridge} />);
    await settle();

    clickButton(container, "Continue");
    await settle();
    expect(fixture.commands[0]).toMatchObject({
      _tag: "Control",
      operationRef: expect.stringMatching(/^ide\.debug-operation\.renderer-/u),
      operation: "continue",
      sessionRef: "ide.debug-session.fixture",
      sessionGeneration: 3,
      adapterGeneration: 8,
      targetGeneration: 9,
      actor: { _tag: "Human", actorRef: "owner.desktop" },
    });

    clickButton(container, "Breakpoints");
    await settle();
    const breakpoint = container.querySelector<HTMLInputElement>("input[type='checkbox']");
    breakpoint?.click();
    await settle();
    expect(fixture.commands.at(-1)).toMatchObject({
      _tag: "ReplaceBreakpoints",
      breakpoints: [{ enabled: false }],
    });

    clickButton(container, "Sources");
    await settle();
    const availableSource = [...container.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("app.ts"),
    );
    if (availableSource === undefined) throw new Error("Available source button not found");
    availableSource.click();
    await settle();
    expect(fixture.commands.at(-1)).toMatchObject({
      _tag: "NavigateSource",
      source: { sourceRef: "ide.debug-source.app" },
      sessionGeneration: 3,
    });
    const staleSource = [...container.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("worker.js"),
    );
    expect(staleSource?.disabled).toBe(true);
    expect(staleSource?.title).toContain("Stale source");

    clickButton(container, "Watch");
    await settle();
    const watch = container.querySelector<HTMLInputElement>("input[aria-label='Watch expression']");
    if (watch === null) throw new Error("Watch input not found");
    await interact(() => changeInput(watch, "count * 2"));
    const watchForm = watch.closest("form");
    await interact(() => {
      watchForm?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });
    expect(fixture.commands.at(-1)).toMatchObject({
      _tag: "Evaluate",
      expression: "count * 2",
      frameRef: "ide.debug-frame.main",
    });

    clickButton(container, "Variables");
    await settle();
    clickButton(container, "Edit");
    await settle();
    const variable = container.querySelector<HTMLInputElement>(
      "input[aria-label='New value for count']",
    );
    if (variable === null) throw new Error("Variable input not found");
    await interact(() => changeInput(variable, "5"));
    await interact(() => {
      variable
        .closest("form")
        ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });
    expect(fixture.commands.at(-1)).toMatchObject({
      _tag: "SetVariable",
      variableRef: "ide.debug-variable.count",
      value: "5",
    });
    await interact(() => clickButton(container, "Receipts"));
    await interact(() => clickButton(container, "Delete retained data"));
    expect(container.textContent).toContain("It does not terminate the target.");
    await interact(() => clickButton(container, "Confirm deletion"));
    expect(fixture.commands.at(-1)).toMatchObject({
      _tag: "DeleteRetainedData",
      operationRef: expect.stringMatching(/^ide\.debug-operation\.renderer-/u),
      actor: { _tag: "Human", actorRef: "owner.desktop" },
    });
    root.unmount();
  });

  test("sends only operation and configuration references for discovery, validation, and start", async () => {
    const { container } = installDom();
    const fixture = bridgeFor(fixtureSnapshot());
    const root = createRoot(container);
    await interact(() => root.render(<ReactIdeDebugPanel bridge={fixture.bridge} />));

    await interact(() => clickButton(container, "Discover"));
    await interact(() => clickButton(container, "Validate"));
    await interact(() => clickButton(container, "Launch"));

    expect(fixture.commands.map((command) => command._tag)).toEqual([
      "Discover",
      "Validate",
      "Start",
    ]);
    expect(fixture.commands[0]).toMatchObject({
      _tag: "Discover",
      operationRef: expect.stringMatching(/^ide\.debug-operation\.renderer-/u),
    });
    expect(fixture.commands[1]).toMatchObject({
      _tag: "Validate",
      configurationRef: "ide.debug-config.fixture",
      operationRef: expect.stringMatching(/^ide\.debug-operation\.renderer-/u),
    });
    expect(fixture.commands[2]).toMatchObject({
      _tag: "Start",
      configurationRef: "ide.debug-config.fixture",
      operationRef: expect.stringMatching(/^ide\.debug-operation\.renderer-/u),
    });
    expect(fixture.commands.some((command) => Reflect.has(command, "configuration"))).toBe(false);
    expect(JSON.stringify(fixture.commands)).not.toContain("Node fixture");
    expect(JSON.stringify(fixture.commands)).not.toContain("SECRET_TOKEN");
    root.unmount();
  });

  test("exposes capability-gated cancellation with a distinct target operation reference", async () => {
    const { container } = installDom();
    const snapshot = fixtureSnapshot();
    const commands: Array<IdeDebugRendererCommand> = [];
    const bridge: IdeDebugRendererBridge = {
      snapshot: async () => snapshot,
      command: (command) => {
        commands.push(command);
        if (command._tag === "Control") return new Promise(() => undefined);
        return Promise.resolve({ _tag: "Succeeded", snapshot, payload: null });
      },
      onEvent: () => () => undefined,
    };
    const root = createRoot(container);
    await interact(() => root.render(<ReactIdeDebugPanel bridge={bridge} />));
    await interact(() => clickButton(container, "Continue"));
    const control = commands[0];
    if (control?._tag !== "Control") throw new Error("Control command not captured");
    expect(container.textContent).toContain("Cancel request");
    await interact(() => clickButton(container, "Cancel request"));
    expect(commands[1]).toMatchObject({
      _tag: "Cancel",
      operationRef: expect.stringMatching(/^ide\.debug-operation\.renderer-/u),
      targetOperationRef: control.operationRef,
      actor: { _tag: "Human", actorRef: "owner.desktop" },
    });
    root.unmount();
  });

  test("keeps degraded capability and stale-generation refusal states explicit", async () => {
    const { container } = installDom();
    const ready = fixtureSnapshot();
    const degraded: IdeDebugRendererSnapshot = {
      ...ready,
      capabilityState: {
        _tag: "Degraded",
        serviceGeneration: 7,
        placementRef: ready.binding.placementRef,
        evidenceTier: "project_local",
        reason: "Adapter restart budget is active.",
        observedAt: "2026-07-20T02:01:00.000Z",
      },
    };
    const bridge: IdeDebugRendererBridge = {
      snapshot: async () => degraded,
      command: async () => ({
        _tag: "Refused",
        snapshot: degraded,
        reason: "stale_generation",
        message: "The target generation changed.",
      }),
      onEvent: () => () => undefined,
    };
    const root = createRoot(container);
    await interact(() => root.render(<ReactIdeDebugPanel bridge={bridge} />));
    expect(container.textContent).toContain(
      "Debug capability degraded: Adapter restart budget is active.",
    );
    await interact(() => clickButton(container, "Validate"));
    expect(container.textContent).toContain("Stale generation: The target generation changed.");
    root.unmount();
  });

  test("keeps the current projection when an old service generation arrives", async () => {
    const { container } = installDom();
    const fixture = bridgeFor(fixtureSnapshot(9, 6));
    const root = createRoot(container);
    root.render(<ReactIdeDebugPanel bridge={fixture.bridge} />);
    await settle();
    fixture.emit({ _tag: "Snapshot", snapshot: fixtureSnapshot(8, 2) });
    await settle();
    expect(container.textContent).toContain("service generation 9");
    expect(container.textContent).toContain("session 6 · adapter 8 · target 9");
    expect(container.textContent).toContain("A stale debug snapshot was dropped");
    root.unmount();
  });

  test("supports roving keyboard navigation for every debug data panel", async () => {
    const { container, window } = installDom();
    const fixture = bridgeFor(fixtureSnapshot());
    const root = createRoot(container);
    root.render(<ReactIdeDebugPanel bridge={fixture.bridge} />);
    await settle();
    const tabs = container.querySelector<HTMLElement>("nav[aria-label='Debug data']");
    const variables = container.querySelector<HTMLButtonElement>("#oa-debug-tab-variables");
    variables?.focus();
    tabs?.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
    await settle();
    expect(container.querySelector("#oa-debug-tab-receipts")?.getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(window.document.activeElement?.textContent).toBe("Receipts");
    root.unmount();
  });

  test("reports a missing bridge and does not invent renderer authority", async () => {
    const { container } = installDom();
    const root = createRoot(container);
    root.render(<ReactIdeDebugPanel bridge={null} />);
    await settle();
    expect(container.querySelector("[role='alert']")?.textContent).toContain("Debug unavailable");
    expect(container.textContent).toContain("Debug services are unavailable in this Desktop host.");
    expect(container.textContent).not.toContain("Launch fixture");
    root.unmount();
  });
});
