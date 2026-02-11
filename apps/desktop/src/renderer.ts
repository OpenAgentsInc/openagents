import { Effect } from "effect";

import { DesktopAppService, type DesktopAppApi } from "./effect/app";
import { makeDesktopRuntime } from "./effect/runtime";

import type { DesktopRuntimeState, ExecutorTask } from "./effect/model";

import "./index.css";

const runtime = makeDesktopRuntime();

const appEffect = <A>(f: (app: DesktopAppApi) => Effect.Effect<A, unknown>) =>
  Effect.gen(function* () {
    const app = yield* DesktopAppService;
    return yield* f(app);
  });

const runApp = <A>(f: (app: DesktopAppApi) => Effect.Effect<A, unknown>): Promise<A> =>
  runtime.runPromise(appEffect(f));

const stateEls = {
  openAgents: document.querySelector<HTMLElement>("[data-status-openagents]"),
  convex: document.querySelector<HTMLElement>("[data-status-convex]"),
  checkedAt: document.querySelector<HTMLElement>("[data-status-checked-at]"),
  authState: document.querySelector<HTMLElement>("[data-auth-state]"),
  authUserId: document.querySelector<HTMLElement>("[data-auth-user-id]"),
  authEmail: document.querySelector<HTMLElement>("[data-auth-email]"),
  authToken: document.querySelector<HTMLElement>("[data-auth-token]"),
  authError: document.querySelector<HTMLElement>("[data-auth-error]"),
  loopState: document.querySelector<HTMLElement>("[data-loop-state]"),
  runState: document.querySelector<HTMLElement>("[data-run-state]"),
  ticks: document.querySelector<HTMLElement>("[data-loop-ticks]"),
  lastTask: document.querySelector<HTMLElement>("[data-loop-last-task]"),
  loopError: document.querySelector<HTMLElement>("[data-loop-error]"),
  tasks: document.querySelector<HTMLElement>("[data-task-list]"),
};

const emailInput = document.querySelector<HTMLInputElement>("[data-auth-email-input]");
const codeInput = document.querySelector<HTMLInputElement>("[data-auth-code-input]");
const taskInput = document.querySelector<HTMLInputElement>("[data-task-input]");

const requestCodeButton = document.querySelector<HTMLButtonElement>("[data-auth-request-code]");
const verifyCodeButton = document.querySelector<HTMLButtonElement>("[data-auth-verify-code]");
const signOutButton = document.querySelector<HTMLButtonElement>("[data-auth-sign-out]");
const startLoopButton = document.querySelector<HTMLButtonElement>("[data-loop-start]");
const stopLoopButton = document.querySelector<HTMLButtonElement>("[data-loop-stop]");
const tickLoopButton = document.querySelector<HTMLButtonElement>("[data-loop-tick]");
const enqueueTaskButton = document.querySelector<HTMLButtonElement>("[data-task-enqueue]");

const setText = (el: HTMLElement | null, value: string): void => {
  if (el) el.textContent = value;
};

const formatTs = (ts: number | null): string => (ts ? new Date(ts).toLocaleTimeString() : "n/a");

const renderTasks = (tasks: ReadonlyArray<ExecutorTask>): void => {
  const host = stateEls.tasks;
  if (!host) return;
  if (tasks.length === 0) {
    host.innerHTML = "<li>No queued tasks.</li>";
    return;
  }
  host.innerHTML = tasks
    .map((task) => {
      const meta =
        task.status === "failed" && task.failureReason
          ? ` (${task.failureReason})`
          : "";
      return `<li><code>${task.id.slice(0, 8)}</code> · <strong>${task.status}</strong> · ${task.payload}${meta}</li>`;
    })
    .join("");
};

const renderSnapshot = (snapshot: DesktopRuntimeState): void => {
  setText(stateEls.openAgents, snapshot.connectivity.openAgentsReachable ? "reachable" : "unreachable");
  setText(stateEls.convex, snapshot.connectivity.convexReachable ? "reachable" : "unreachable");
  setText(stateEls.checkedAt, formatTs(snapshot.connectivity.lastCheckedAtMs));

  setText(stateEls.authState, snapshot.auth.status);
  setText(stateEls.authUserId, snapshot.auth.userId ?? "n/a");
  setText(stateEls.authEmail, snapshot.auth.email ?? "n/a");
  setText(stateEls.authToken, snapshot.auth.tokenPresent ? "present (memory only)" : "none");
  setText(stateEls.authError, snapshot.auth.lastError ?? "none");

  setText(stateEls.loopState, snapshot.executor.loop);
  setText(stateEls.runState, snapshot.executor.status);
  setText(stateEls.ticks, String(snapshot.executor.ticks));
  setText(stateEls.lastTask, snapshot.executor.lastTaskId ?? "n/a");
  setText(stateEls.loopError, snapshot.executor.lastError ?? "none");
};

const refreshView = async (): Promise<void> => {
  const snapshot = await runApp((app) => app.snapshot());
  const tasks = await runApp((app) => app.listTasks());
  renderSnapshot(snapshot);
  renderTasks(tasks);
};

const bindActions = (): void => {
  requestCodeButton?.addEventListener("click", async () => {
    const email = emailInput?.value?.trim() ?? "";
    if (!email) return;
    await runApp((app) => app.requestMagicCode(email));
    await refreshView();
  });

  verifyCodeButton?.addEventListener("click", async () => {
    const email = emailInput?.value?.trim() ?? "";
    const code = codeInput?.value?.trim() ?? "";
    if (!email || !code) return;
    await runApp((app) => app.verifyMagicCode({ email, code }));
    await refreshView();
  });

  signOutButton?.addEventListener("click", async () => {
    await runApp((app) => app.signOut());
    await refreshView();
  });

  startLoopButton?.addEventListener("click", async () => {
    await runApp((app) => app.startExecutor());
    await refreshView();
  });

  stopLoopButton?.addEventListener("click", async () => {
    await runApp((app) => app.stopExecutor());
    await refreshView();
  });

  tickLoopButton?.addEventListener("click", async () => {
    await runApp((app) => app.tickExecutor());
    await refreshView();
  });

  enqueueTaskButton?.addEventListener("click", async () => {
    const payload = taskInput?.value?.trim() ?? "";
    await runApp((app) => app.enqueueDemoTask(payload));
    if (taskInput) taskInput.value = "";
    await refreshView();
  });
};

const mount = async (): Promise<void> => {
  try {
    bindActions();
    await runApp((app) => app.bootstrap());
    await refreshView();
    setInterval(() => {
      void refreshView();
    }, 1_500);
  } catch (error) {
    console.error("[desktop] boot_failed", error);
    setText(stateEls.authError, String(error));
  }
};

void mount();
