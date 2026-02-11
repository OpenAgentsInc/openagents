import { DomServiceTag, EffuseLive, html, type TemplateResult } from "@openagentsinc/effuse";
import {
  DEFAULT_PANE_SYSTEM_THEME,
  calculateNewPanePosition,
  mountPaneSystemDom,
  type PaneRect,
  type PaneSystemDom,
} from "@openagentsinc/effuse-panes";
import { Effect } from "effect";

import { DesktopAppService, type DesktopAppApi } from "./effect/app";
import { initialDesktopRuntimeState, type DesktopRuntimeState, type ExecutorTask } from "./effect/model";
import { makeDesktopRuntime } from "./effect/runtime";

import "./index.css";

const runtime = makeDesktopRuntime();

const PANE_SPECS = [
  { id: "desktop-overview", kind: "overview", title: "OpenAgents Desktop", width: 540, height: 250 },
  { id: "desktop-auth", kind: "auth", title: "Auth Session", width: 540, height: 360 },
  { id: "desktop-executor", kind: "executor", title: "Executor Loop", width: 540, height: 260 },
  { id: "desktop-queue", kind: "queue", title: "Task Queue", width: 540, height: 330 },
] as const;

type PaneId = (typeof PANE_SPECS)[number]["id"];

const drafts: { email: string; code: string; task: string } = {
  email: "",
  code: "",
  task: "",
};

let currentSnapshot: DesktopRuntimeState = initialDesktopRuntimeState();
let currentTasks: ReadonlyArray<ExecutorTask> = [];
let uiError: string | null = null;

const appEffect = <A>(f: (app: DesktopAppApi) => Effect.Effect<A, unknown>) =>
  Effect.gen(function* () {
    const app = yield* DesktopAppService;
    return yield* f(app);
  });

const runApp = <A>(f: (app: DesktopAppApi) => Effect.Effect<A, unknown>): Promise<A> => runtime.runPromise(appEffect(f));

const renderWithEffuse = (container: Element, content: TemplateResult): Promise<void> =>
  Effect.runPromise(
    Effect.gen(function* () {
      const dom = yield* DomServiceTag;
      yield* dom.render(container, content);
    }).pipe(Effect.provide(EffuseLive)),
  );

const formatTs = (ts: number | null): string => (ts ? new Date(ts).toLocaleTimeString() : "n/a");

const statusBadge = (ok: boolean): TemplateResult =>
  html`<span class="oa-badge ${ok ? "up" : "down"}">${ok ? "reachable" : "unreachable"}</span>`;

const queueStatusClass = (status: ExecutorTask["status"]): string => {
  if (status === "completed") return "oa-status-completed";
  if (status === "failed") return "oa-status-failed";
  return "";
};

const paneSlot = (paneRoot: HTMLElement, paneId: PaneId): HTMLElement | null => {
  const el = paneRoot.querySelector(`[data-pane-id="${paneId}"] [data-oa-pane-content]`);
  return el instanceof HTMLElement ? el : null;
};

const paneScreen = (paneRoot: HTMLElement): { width: number; height: number } => ({
  width: Math.max(640, paneRoot.clientWidth || window.innerWidth || 1024),
  height: Math.max(480, paneRoot.clientHeight || window.innerHeight || 768),
});

const overviewTemplate = (snapshot: DesktopRuntimeState): TemplateResult => html`
  <section class="oa-pane">
    <h2>OpenAgents Desktop Lightning Executor</h2>
    <p class="oa-muted">EP212 shell: desktop executes payment flows while openagents.com stays orchestration + chat.</p>
    ${uiError
      ? html`<div class="oa-warning">UI error: ${uiError}</div>`
      : null}
    <dl class="oa-grid">
      <dt>OpenAgents API</dt>
      <dd>${statusBadge(snapshot.connectivity.openAgentsReachable)}</dd>
      <dt>Convex</dt>
      <dd>${statusBadge(snapshot.connectivity.convexReachable)}</dd>
      <dt>Last check</dt>
      <dd>${formatTs(snapshot.connectivity.lastCheckedAtMs)}</dd>
      <dt>Identity link</dt>
      <dd>Sign in here with the same email used on <code>openagents.com</code>.</dd>
    </dl>
  </section>
`;

const authTemplate = (snapshot: DesktopRuntimeState): TemplateResult => html`
  <section class="oa-pane">
    <dl class="oa-grid">
      <dt>Status</dt>
      <dd>${snapshot.auth.status}</dd>
      <dt>User ID</dt>
      <dd><code>${snapshot.auth.userId ?? "n/a"}</code></dd>
      <dt>Email</dt>
      <dd>${snapshot.auth.email ?? "n/a"}</dd>
      <dt>Token</dt>
      <dd>${snapshot.auth.tokenPresent ? "present (memory only)" : "none"}</dd>
      <dt>Last error</dt>
      <dd>${snapshot.auth.lastError ?? "none"}</dd>
    </dl>
    <div class="oa-field">
      <label for="desktop-auth-email">Email</label>
      <input
        id="desktop-auth-email"
        name="desktop-auth-email"
        class="oa-input"
        data-auth-email-input
        type="email"
        autocomplete="email"
        placeholder="you@openagents.com"
        value="${drafts.email || snapshot.auth.email || ""}"
      />
    </div>
    <div class="oa-row">
      <button class="oa-btn" data-auth-request-code type="button">Request Magic Code</button>
    </div>
    <div class="oa-field">
      <label for="desktop-auth-code">Code</label>
      <input
        id="desktop-auth-code"
        name="desktop-auth-code"
        class="oa-input"
        data-auth-code-input
        type="text"
        autocomplete="one-time-code"
        placeholder="6-digit code"
        value="${drafts.code}"
      />
    </div>
    <div class="oa-row">
      <button class="oa-btn" data-auth-verify-code type="button">Verify Code</button>
      <button class="oa-btn muted" data-auth-sign-out type="button">Sign Out</button>
    </div>
  </section>
`;

const executorTemplate = (snapshot: DesktopRuntimeState): TemplateResult => html`
  <section class="oa-pane">
    <dl class="oa-grid">
      <dt>Loop</dt>
      <dd>${snapshot.executor.loop}</dd>
      <dt>Run status</dt>
      <dd>${snapshot.executor.status}</dd>
      <dt>Ticks</dt>
      <dd>${snapshot.executor.ticks}</dd>
      <dt>Last task</dt>
      <dd><code>${snapshot.executor.lastTaskId ?? "n/a"}</code></dd>
      <dt>Last transition</dt>
      <dd>${formatTs(snapshot.executor.lastTransitionAtMs)}</dd>
      <dt>Last error</dt>
      <dd>${snapshot.executor.lastError ?? "none"}</dd>
    </dl>
    <div class="oa-row">
      <button class="oa-btn" data-loop-start type="button">Start Loop</button>
      <button class="oa-btn muted" data-loop-stop type="button">Stop Loop</button>
      <button class="oa-btn muted" data-loop-tick type="button">Tick Once</button>
    </div>
  </section>
`;

const queueTemplate = (tasks: ReadonlyArray<ExecutorTask>): TemplateResult => html`
  <section class="oa-pane">
    <h3>Demo Provider Queue</h3>
    <p class="oa-muted">Use payload containing <code>fail</code> to simulate a failure path.</p>
    <div class="oa-row">
      <input
        id="desktop-task-input"
        name="desktop-task-input"
        class="oa-input"
        data-task-input
        type="text"
        placeholder="task payload"
        value="${drafts.task}"
      />
      <button class="oa-btn" data-task-enqueue type="button">Enqueue Task</button>
    </div>
    <ul class="oa-list">
      ${tasks.length === 0
        ? html`<li class="oa-list-item">No queued tasks.</li>`
        : tasks.map((task) => html`
            <li class="oa-list-item">
              <div><code>${task.id.slice(0, 8)}</code> · <strong class="${queueStatusClass(task.status)}">${task.status}</strong></div>
              <div>${task.payload}</div>
              ${task.status === "failed" && task.failureReason
                ? html`<div class="oa-status-failed">reason: ${task.failureReason}</div>`
                : null}
            </li>
          `)}
    </ul>
  </section>
`;

const runUiAction = (
  label: string,
  action: () => Promise<void>,
  refresh: () => Promise<void>,
): void => {
  void action().catch((error) => {
    uiError = String(error);
    console.error(`[desktop] ${label}_failed`, error);
  }).finally(() => {
    void refresh();
  });
};

const bindAuthActions = (slot: HTMLElement, refresh: () => Promise<void>): void => {
  const emailInput = slot.querySelector<HTMLInputElement>("[data-auth-email-input]");
  const codeInput = slot.querySelector<HTMLInputElement>("[data-auth-code-input]");
  const requestButton = slot.querySelector<HTMLButtonElement>("[data-auth-request-code]");
  const verifyButton = slot.querySelector<HTMLButtonElement>("[data-auth-verify-code]");
  const signOutButton = slot.querySelector<HTMLButtonElement>("[data-auth-sign-out]");

  emailInput?.addEventListener("input", () => {
    drafts.email = emailInput.value;
  });

  codeInput?.addEventListener("input", () => {
    drafts.code = codeInput.value;
  });

  requestButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "request_magic_code",
      async () => {
        const email = (emailInput?.value ?? drafts.email).trim();
        if (!email) return;
        drafts.email = email;
        await runApp((app) => app.requestMagicCode(email));
      },
      refresh,
    );
  });

  verifyButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "verify_magic_code",
      async () => {
        const email = (emailInput?.value ?? drafts.email).trim();
        const code = (codeInput?.value ?? drafts.code).trim();
        if (!email || !code) return;
        drafts.email = email;
        drafts.code = code;
        await runApp((app) => app.verifyMagicCode({ email, code }));
      },
      refresh,
    );
  });

  signOutButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "sign_out",
      async () => {
        drafts.code = "";
        await runApp((app) => app.signOut());
      },
      refresh,
    );
  });
};

const bindExecutorActions = (slot: HTMLElement, refresh: () => Promise<void>): void => {
  const startLoopButton = slot.querySelector<HTMLButtonElement>("[data-loop-start]");
  const stopLoopButton = slot.querySelector<HTMLButtonElement>("[data-loop-stop]");
  const tickLoopButton = slot.querySelector<HTMLButtonElement>("[data-loop-tick]");

  startLoopButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction("start_loop", () => runApp((app) => app.startExecutor()).then(() => undefined), refresh);
  });

  stopLoopButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction("stop_loop", () => runApp((app) => app.stopExecutor()).then(() => undefined), refresh);
  });

  tickLoopButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction("tick_loop", () => runApp((app) => app.tickExecutor()).then(() => undefined), refresh);
  });
};

const bindQueueActions = (slot: HTMLElement, refresh: () => Promise<void>): void => {
  const taskInput = slot.querySelector<HTMLInputElement>("[data-task-input]");
  const enqueueTaskButton = slot.querySelector<HTMLButtonElement>("[data-task-enqueue]");

  taskInput?.addEventListener("input", () => {
    drafts.task = taskInput.value;
  });

  enqueueTaskButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "enqueue_task",
      async () => {
        const payload = (taskInput?.value ?? drafts.task).trim();
        if (!payload) return;
        drafts.task = "";
        await runApp((app) => app.enqueueDemoTask(payload));
      },
      refresh,
    );
  });
};

const addInitialPanes = (paneSystem: PaneSystemDom, paneRoot: HTMLElement): void => {
  const screen = paneScreen(paneRoot);
  let lastRect: PaneRect | undefined;

  for (const pane of PANE_SPECS) {
    const rect = calculateNewPanePosition(lastRect, screen, pane.width, pane.height);
    lastRect = rect;
    paneSystem.store.addPane({
      id: pane.id,
      kind: pane.kind,
      title: pane.title,
      rect,
      dismissable: false,
    });
  }
  paneSystem.render();
};

const mount = async (): Promise<void> => {
  const paneRoot = document.querySelector<HTMLElement>("[data-desktop-pane-root]");
  if (!(paneRoot instanceof HTMLElement)) {
    throw new Error("desktop_pane_root_missing");
  }

  const paneSystem = mountPaneSystemDom(paneRoot, {
    enableDotsBackground: true,
    enableCanvasPan: true,
    enablePaneDrag: true,
    enablePaneResize: true,
    enableKeyboardShortcuts: true,
    enableHotbar: false,
    theme: {
      ...DEFAULT_PANE_SYSTEM_THEME,
      background: "rgba(3, 7, 14, 0.9)",
      surface: "rgba(15, 23, 42, 0.6)",
      border: "rgba(148, 163, 184, 0.35)",
      accent: "#93c5fd",
      text: "#e5edf5",
      mutedText: "#9fb0c7",
    },
  });

  addInitialPanes(paneSystem, paneRoot);

  let refreshInFlight: Promise<void> | null = null;
  let refreshQueued = false;

  const renderAllPanes = async (): Promise<void> => {
    const overviewSlot = paneSlot(paneRoot, "desktop-overview");
    const authSlot = paneSlot(paneRoot, "desktop-auth");
    const executorSlot = paneSlot(paneRoot, "desktop-executor");
    const queueSlot = paneSlot(paneRoot, "desktop-queue");

    if (overviewSlot) {
      await renderWithEffuse(overviewSlot, overviewTemplate(currentSnapshot));
    }
    if (authSlot) {
      await renderWithEffuse(authSlot, authTemplate(currentSnapshot));
      bindAuthActions(authSlot, refreshView);
    }
    if (executorSlot) {
      await renderWithEffuse(executorSlot, executorTemplate(currentSnapshot));
      bindExecutorActions(executorSlot, refreshView);
    }
    if (queueSlot) {
      await renderWithEffuse(queueSlot, queueTemplate(currentTasks));
      bindQueueActions(queueSlot, refreshView);
    }
  };

  const refreshView = async (): Promise<void> => {
    if (refreshInFlight) {
      refreshQueued = true;
      await refreshInFlight;
      return;
    }

    refreshInFlight = (async () => {
      try {
        const [snapshot, tasks] = await Promise.all([
          runApp((app) => app.snapshot()),
          runApp((app) => app.listTasks()),
        ]);

        currentSnapshot = snapshot;
        currentTasks = tasks;
        uiError = null;
      } catch (error) {
        uiError = String(error);
        console.error("[desktop] refresh_failed", error);
      }

      await renderAllPanes();
    })();

    try {
      await refreshInFlight;
    } finally {
      refreshInFlight = null;
      if (refreshQueued) {
        refreshQueued = false;
        await refreshView();
      }
    }
  };

  await renderAllPanes();
  await runApp((app) => app.bootstrap());
  await refreshView();

  setInterval(() => {
    void refreshView();
  }, 1_500);
};

void mount().catch((error) => {
  console.error("[desktop] boot_failed", error);
});
