import { DomServiceTag, EffuseLive, html, type TemplateResult } from "@openagentsinc/effuse";
import {
  DEFAULT_PANE_SYSTEM_THEME,
  mountPaneSystemDom,
  type PaneSystemDom,
} from "@openagentsinc/effuse-panes";
import { Effect } from "effect";

import { DesktopAppService, type DesktopAppApi } from "./effect/app";
import {
  classifyFailure,
  deriveNodePaneModel,
  deriveTransactionHistoryModel,
  deriveWalletBalanceModel,
  latestExecutorFailure,
  type FailureCategory,
  type TransactionHistoryModel,
} from "./effect/paneModels";
import {
  DESKTOP_PANE_SPEC_BY_ID,
  addInitialDesktopPanes,
  focusOrOpenDesktopPane,
  isDesktopPaneId,
  toggleDesktopPane,
  type DesktopPaneId,
} from "./effect/paneLayout";
import { initialDesktopRuntimeState, type DesktopRuntimeState, type ExecutorTask } from "./effect/model";
import { makeDesktopRuntime } from "./effect/runtime";

import "./index.css";

const runtime = makeDesktopRuntime();

type PaneId = DesktopPaneId;

const drafts: {
  email: string;
  code: string;
  task: string;
  walletInitPassphrase: string;
  walletUnlockPassphrase: string;
  walletRestorePassphrase: string;
  walletRestoreSeed: string;
} = {
  email: "",
  code: "",
  task: "",
  walletInitPassphrase: "",
  walletUnlockPassphrase: "",
  walletRestorePassphrase: "",
  walletRestoreSeed: "",
};

let currentSnapshot: DesktopRuntimeState = initialDesktopRuntimeState();
let currentTasks: ReadonlyArray<ExecutorTask> = [];
let uiError: string | null = null;
let loadedSnapshot = false;

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
  if (status === "completed" || status === "paid" || status === "cached") return "oa-status-completed";
  if (status === "blocked" || status === "failed") return "oa-status-failed";
  return "";
};

const failureCategoryLabel = (category: FailureCategory): string => {
  if (category === "policy") return "policy";
  if (category === "node") return "node";
  if (category === "wallet") return "wallet";
  if (category === "transport") return "transport";
  if (category === "endpoint") return "endpoint";
  return "unknown";
};

const failureCategoryClass = (category: FailureCategory): string => `oa-failure-${category}`;

const paneSlot = (paneRoot: HTMLElement, paneId: PaneId): HTMLElement | null => {
  const el = paneRoot.querySelector(`[data-pane-id="${paneId}"] [data-oa-pane-content]`);
  return el instanceof HTMLElement ? el : null;
};

const paneScreen = (paneRoot: HTMLElement): { width: number; height: number } => ({
  width: Math.max(640, paneRoot.clientWidth || window.innerWidth || 1024),
  height: Math.max(480, paneRoot.clientHeight || window.innerHeight || 768),
});

const openPaneIds = (paneSystem: PaneSystemDom): ReadonlySet<PaneId> => {
  const ids = paneSystem.store.panes().map((pane) => pane.id).filter((id): id is PaneId => id in DESKTOP_PANE_SPEC_BY_ID);
  return new Set(ids);
};

const paneControlButton = (label: string, paneId: PaneId, isOpen: boolean, mode: "focus" | "toggle" = "focus"): TemplateResult =>
  html`<button
    class="oa-btn oa-pane-btn ${isOpen ? "active" : ""}"
    data-pane-control
    data-pane-id="${paneId}"
    data-pane-mode="${mode}"
    type="button"
  >
    ${label}
  </button>`;

const overviewTemplate = (input: {
  readonly snapshot: DesktopRuntimeState;
  readonly openPanes: ReadonlySet<PaneId>;
}): TemplateResult => {
  const node = deriveNodePaneModel({
    snapshot: input.snapshot,
    loaded: loadedSnapshot,
    uiError,
  });
  const wallet = deriveWalletBalanceModel({
    snapshot: input.snapshot,
    tasks: currentTasks,
    loaded: loadedSnapshot,
    uiError,
  });
  const history = deriveTransactionHistoryModel({
    tasks: currentTasks,
    loaded: loadedSnapshot,
    uiError,
  });
  const lastFailure = latestExecutorFailure(currentTasks);

  return html`
    <section class="oa-pane">
      <h2>OpenAgents Desktop Lightning Executor</h2>
      <p class="oa-muted">Local node + wallet execution for L402 tasks queued from <code>openagents.com</code>.</p>
      ${uiError
        ? html`<div class="oa-warning">UI error: ${uiError}</div>`
        : null}

      <dl class="oa-grid">
        <dt>OpenAgents API</dt>
        <dd>${statusBadge(input.snapshot.connectivity.openAgentsReachable)}</dd>
        <dt>Convex</dt>
        <dd>${statusBadge(input.snapshot.connectivity.convexReachable)}</dd>
        <dt>Last check</dt>
        <dd>${formatTs(input.snapshot.connectivity.lastCheckedAtMs)}</dd>
        <dt>Node sync</dt>
        <dd>
          <span class="oa-chip oa-sync-${node.syncStage}">${node.syncLabel}</span>
          ${node.diagnostic ? html`<span class="oa-muted"> · ${node.diagnostic}</span>` : null}
        </dd>
        <dt>Wallet availability</dt>
        <dd><span class="oa-chip">${wallet.availability}</span></dd>
        <dt>History</dt>
        <dd>${history.payments.length} payments / ${history.invoices.length} invoices</dd>
        <dt>Last failure</dt>
        <dd>
          ${lastFailure
            ? html`<span class="oa-chip ${failureCategoryClass(lastFailure.category)}">${failureCategoryLabel(lastFailure.category)}</span>
                <span class="oa-muted">${lastFailure.reason ?? "n/a"}</span>`
            : "none"}
        </dd>
      </dl>

      <div class="oa-row">
        ${paneControlButton("Node", "desktop-node", input.openPanes.has("desktop-node"))}
        ${paneControlButton("Wallet", "desktop-wallet", input.openPanes.has("desktop-wallet"))}
        ${paneControlButton("Executor", "desktop-executor", input.openPanes.has("desktop-executor"))}
        ${paneControlButton("Transactions", "desktop-transactions", input.openPanes.has("desktop-transactions"), "toggle")}
      </div>
    </section>
  `;
};

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

const nodeTemplate = (snapshot: DesktopRuntimeState): TemplateResult => {
  const model = deriveNodePaneModel({
    snapshot,
    loaded: loadedSnapshot,
    uiError,
  });

  if (model.paneState === "loading") {
    return html`<section class="oa-pane"><div class="oa-info">Loading node runtime state...</div></section>`;
  }

  if (model.paneState === "error") {
    return html`
      <section class="oa-pane">
        <div class="oa-warning">${model.syncLabel}${model.diagnostic ? `: ${model.diagnostic}` : ""}</div>
      </section>
    `;
  }

  return html`
    <section class="oa-pane">
      ${model.paneState === "empty" ? html`<div class="oa-info">Node is offline. Start LND to process L402 tasks.</div>` : null}
      <dl class="oa-grid">
        <dt>Lifecycle</dt>
        <dd>${snapshot.lnd.lifecycle}</dd>
        <dt>Health</dt>
        <dd>${snapshot.lnd.health}</dd>
        <dt>Sync stage</dt>
        <dd><span class="oa-chip oa-sync-${model.syncStage}">${model.syncLabel}</span></dd>
        <dt>Target / PID</dt>
        <dd><code>${snapshot.lnd.target ?? "n/a"}</code> / <code>${snapshot.lnd.pid ?? "n/a"}</code></dd>
        <dt>Crashes / Restarts</dt>
        <dd>${snapshot.lnd.crashCount} / ${snapshot.lnd.restartCount}</dd>
        <dt>Next restart</dt>
        <dd>${formatTs(snapshot.lnd.nextRestartAtMs)}</dd>
        <dt>Last health check</dt>
        <dd>${formatTs(snapshot.lnd.lastHealthCheckAtMs)}</dd>
        <dt>Last error</dt>
        <dd>${snapshot.lnd.lastError ?? "none"}</dd>
      </dl>
      <div class="oa-row">
        <button class="oa-btn" data-lnd-start type="button">Start LND</button>
        <button class="oa-btn muted" data-lnd-stop type="button">Stop LND</button>
        <button class="oa-btn muted" data-lnd-restart type="button">Restart LND</button>
      </div>
    </section>
  `;
};

const executorTemplate = (snapshot: DesktopRuntimeState, tasks: ReadonlyArray<ExecutorTask>): TemplateResult => {
  const loading = !loadedSnapshot;
  const lastFailure = latestExecutorFailure(tasks);
  const pending = tasks.filter((task) => task.status === "queued" || task.status === "approved" || task.status === "running");

  return html`
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
        <dt>Queue depth</dt>
        <dd>${pending.length}</dd>
        <dt>Last failure class</dt>
        <dd>
          ${lastFailure
            ? html`<span class="oa-chip ${failureCategoryClass(lastFailure.category)}">${failureCategoryLabel(lastFailure.category)}</span>
                <span class="oa-muted">${lastFailure.reason ?? "n/a"}</span>`
            : "none"}
        </dd>
      </dl>

      <div class="oa-row">
        <button class="oa-btn" data-loop-start type="button">Start Loop</button>
        <button class="oa-btn muted" data-loop-stop type="button">Stop Loop</button>
        <button class="oa-btn muted" data-loop-tick type="button">Tick Once</button>
      </div>

      <div class="oa-row">
        <input
          id="desktop-task-input"
          name="desktop-task-input"
          class="oa-input"
          data-task-input
          type="text"
          placeholder="https://api.example.com/premium-data"
          value="${drafts.task}"
        />
        <button class="oa-btn" data-task-enqueue type="button">Enqueue Task</button>
      </div>

      ${loading
        ? html`<div class="oa-info">Loading queue state...</div>`
        : pending.length === 0
          ? html`<div class="oa-info">Queue empty.</div>`
          : html`
              <ul class="oa-list">
                ${pending.map((task) => {
                  const reason = task.failureReason ?? task.lastErrorMessage;
                  const category = classifyFailure({
                    code: task.lastErrorCode,
                    message: task.lastErrorMessage,
                    reason: task.failureReason,
                  });
                  return html`
                    <li class="oa-list-item">
                      <div><code>${task.id.slice(0, 8)}</code> · <strong class="${queueStatusClass(task.status)}">${task.status}</strong></div>
                      <div>${task.request.method ?? "GET"} ${task.request.url}</div>
                      <div class="oa-muted">max spend: ${task.request.maxSpendMsats} msats · attempts: ${task.attemptCount}</div>
                      ${reason
                        ? html`<div class="${failureCategoryClass(category)}">${failureCategoryLabel(category)}: ${reason}</div>`
                        : null}
                    </li>
                  `;
                })}
              </ul>
            `}
    </section>
  `;
};

const walletTemplate = (snapshot: DesktopRuntimeState): TemplateResult => {
  const balance = deriveWalletBalanceModel({
    snapshot,
    tasks: currentTasks,
    loaded: loadedSnapshot,
    uiError,
  });

  return html`
    <section class="oa-pane">
      ${balance.paneState === "loading"
        ? html`<div class="oa-info">Loading wallet state...</div>`
        : null}
      ${balance.paneState === "error"
        ? html`<div class="oa-warning">Wallet view degraded due to desktop refresh errors.</div>`
        : null}

      <dl class="oa-grid">
        <dt>Wallet state</dt>
        <dd>${snapshot.wallet.walletState}</dd>
        <dt>Recovery state</dt>
        <dd>${snapshot.wallet.recoveryState}</dd>
        <dt>Balance visibility</dt>
        <dd><span class="oa-chip">${balance.availability}</span></dd>
        <dt>Session spend (msats)</dt>
        <dd>${balance.estimatedSpendMsats}</dd>
        <dt>Settled / Failed / Blocked</dt>
        <dd>${balance.settledCount} / ${balance.failedCount} / ${balance.blockedCount}</dd>
        <dt>Cache hits</dt>
        <dd>${balance.cacheHits}</dd>
        <dt>Seed backup acknowledged</dt>
        <dd>${snapshot.wallet.seedBackupAcknowledged ? "yes" : "no"}</dd>
        <dt>Passphrase stored</dt>
        <dd>${snapshot.wallet.passphraseStored ? "yes (secure storage)" : "no"}</dd>
        <dt>Restore prepared</dt>
        <dd>${snapshot.wallet.restorePrepared ? "yes" : "no"}</dd>
        <dt>Last wallet operation</dt>
        <dd>${snapshot.wallet.lastOperation ?? "n/a"}</dd>
        <dt>Wallet error code</dt>
        <dd><code>${snapshot.wallet.lastErrorCode ?? "none"}</code></dd>
        <dt>Wallet error</dt>
        <dd>${snapshot.wallet.lastErrorMessage ?? "none"}</dd>
        <dt>Updated</dt>
        <dd>${formatTs(snapshot.wallet.updatedAtMs)}</dd>
      </dl>

      <div class="oa-field">
        <label for="desktop-wallet-init-passphrase">Init Passphrase</label>
        <input
          id="desktop-wallet-init-passphrase"
          name="desktop-wallet-init-passphrase"
          class="oa-input"
          data-wallet-init-passphrase
          type="password"
          autocomplete="new-password"
          placeholder="enter passphrase"
          value="${drafts.walletInitPassphrase}"
        />
      </div>

      <div class="oa-row">
        <button class="oa-btn" data-wallet-init type="button">Initialize Wallet</button>
        <button class="oa-btn muted" data-wallet-ack type="button">Acknowledge Seed Backup</button>
      </div>

      <div class="oa-field">
        <label for="desktop-wallet-unlock-passphrase">Unlock Passphrase (optional)</label>
        <input
          id="desktop-wallet-unlock-passphrase"
          name="desktop-wallet-unlock-passphrase"
          class="oa-input"
          data-wallet-unlock-passphrase
          type="password"
          autocomplete="current-password"
          placeholder="leave empty to use stored passphrase"
          value="${drafts.walletUnlockPassphrase}"
        />
      </div>

      <div class="oa-row">
        <button class="oa-btn" data-wallet-unlock type="button">Unlock Wallet</button>
        <button class="oa-btn muted" data-wallet-lock type="button">Lock Wallet</button>
        <button class="oa-btn muted" data-wallet-prepare-restore type="button">Prepare Restore</button>
      </div>

      <div class="oa-field">
        <label for="desktop-wallet-restore-passphrase">Restore Passphrase</label>
        <input
          id="desktop-wallet-restore-passphrase"
          name="desktop-wallet-restore-passphrase"
          class="oa-input"
          data-wallet-restore-passphrase
          type="password"
          autocomplete="new-password"
          placeholder="restore passphrase"
          value="${drafts.walletRestorePassphrase}"
        />
      </div>
      <div class="oa-field">
        <label for="desktop-wallet-restore-seed">Restore Seed Phrase</label>
        <textarea
          id="desktop-wallet-restore-seed"
          name="desktop-wallet-restore-seed"
          class="oa-input"
          data-wallet-restore-seed
          rows="2"
          placeholder="space-separated 12-24 word seed phrase"
        >${drafts.walletRestoreSeed}</textarea>
      </div>
      <div class="oa-row">
        <button class="oa-btn muted" data-wallet-restore type="button">Restore Wallet</button>
      </div>
    </section>
  `;
};

const transactionsTemplate = (history: TransactionHistoryModel): TemplateResult => {
  if (history.paneState === "loading") {
    return html`<section class="oa-pane"><div class="oa-info">Loading payment and invoice history...</div></section>`;
  }
  if (history.paneState === "error") {
    return html`<section class="oa-pane"><div class="oa-warning">Unable to render payment history from current desktop state.</div></section>`;
  }
  if (history.paneState === "empty") {
    return html`<section class="oa-pane"><div class="oa-info">No payment/invoice history yet. Enqueue an L402 task to populate this pane.</div></section>`;
  }

  return html`
    <section class="oa-pane">
      <h3>Recent Payments</h3>
      <ul class="oa-list">
        ${history.payments.slice(0, 12).map((entry) => html`
          <li class="oa-list-item">
            <div><code>${entry.id.slice(0, 8)}</code> · <strong class="${queueStatusClass(entry.status)}">${entry.status}</strong></div>
            <div>${entry.method} ${entry.urlLabel}</div>
            <div class="oa-muted">updated: ${formatTs(entry.updatedAtMs)} · amount: ${entry.amountMsats ?? "n/a"} msats</div>
            ${entry.category
              ? html`<div class="${failureCategoryClass(entry.category)}">${failureCategoryLabel(entry.category)}${entry.reason ? `: ${entry.reason}` : ""}</div>`
              : null}
          </li>
        `)}
      </ul>

      <h3>Recent Invoices</h3>
      <ul class="oa-list">
        ${history.invoices.slice(0, 12).map((entry) => html`
          <li class="oa-list-item">
            <div><code>${entry.id.slice(0, 8)}</code> · <strong>${entry.state}</strong></div>
            <div>${entry.urlLabel}</div>
            <div class="oa-muted">updated: ${formatTs(entry.updatedAtMs)}</div>
            ${entry.reason ? html`<div class="oa-status-failed">${entry.reason}</div>` : null}
          </li>
        `)}
      </ul>
    </section>
  `;
};

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

const parseSeedWords = (raw: string): ReadonlyArray<string> =>
  raw
    .split(/\s+/)
    .map((word) => word.trim())
    .filter((word) => word.length > 0);

const bindOverviewActions = (
  slot: HTMLElement,
  paneSystem: PaneSystemDom,
  paneRoot: HTMLElement,
  refresh: () => Promise<void>,
): void => {
  slot.querySelectorAll<HTMLButtonElement>("[data-pane-control]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.preventDefault();
      const paneId = button.dataset.paneId as PaneId | undefined;
      const mode = button.dataset.paneMode;
      if (!paneId || !isDesktopPaneId(paneId)) return;
      const screen = paneScreen(paneRoot);
      if (mode === "toggle") {
        toggleDesktopPane(paneSystem.store, screen, paneId);
      } else {
        focusOrOpenDesktopPane(paneSystem.store, screen, paneId);
      }
      paneSystem.render();
      void refresh();
    });
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

const bindNodeActions = (slot: HTMLElement, refresh: () => Promise<void>): void => {
  const startLndButton = slot.querySelector<HTMLButtonElement>("[data-lnd-start]");
  const stopLndButton = slot.querySelector<HTMLButtonElement>("[data-lnd-stop]");
  const restartLndButton = slot.querySelector<HTMLButtonElement>("[data-lnd-restart]");

  startLndButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction("start_lnd", () => runApp((app) => app.startLndRuntime()).then(() => undefined), refresh);
  });

  stopLndButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction("stop_lnd", () => runApp((app) => app.stopLndRuntime()).then(() => undefined), refresh);
  });

  restartLndButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "restart_lnd",
      () => runApp((app) => app.restartLndRuntime()).then(() => undefined),
      refresh,
    );
  });
};

const bindExecutorActions = (slot: HTMLElement, refresh: () => Promise<void>): void => {
  const startLoopButton = slot.querySelector<HTMLButtonElement>("[data-loop-start]");
  const stopLoopButton = slot.querySelector<HTMLButtonElement>("[data-loop-stop]");
  const tickLoopButton = slot.querySelector<HTMLButtonElement>("[data-loop-tick]");
  const taskInput = slot.querySelector<HTMLInputElement>("[data-task-input]");
  const enqueueTaskButton = slot.querySelector<HTMLButtonElement>("[data-task-enqueue]");

  taskInput?.addEventListener("input", () => {
    drafts.task = taskInput.value;
  });

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

const bindWalletActions = (slot: HTMLElement, refresh: () => Promise<void>): void => {
  const initPassphraseInput = slot.querySelector<HTMLInputElement>("[data-wallet-init-passphrase]");
  const unlockPassphraseInput = slot.querySelector<HTMLInputElement>("[data-wallet-unlock-passphrase]");
  const restorePassphraseInput = slot.querySelector<HTMLInputElement>("[data-wallet-restore-passphrase]");
  const restoreSeedInput = slot.querySelector<HTMLTextAreaElement>("[data-wallet-restore-seed]");

  const initButton = slot.querySelector<HTMLButtonElement>("[data-wallet-init]");
  const unlockButton = slot.querySelector<HTMLButtonElement>("[data-wallet-unlock]");
  const lockButton = slot.querySelector<HTMLButtonElement>("[data-wallet-lock]");
  const ackButton = slot.querySelector<HTMLButtonElement>("[data-wallet-ack]");
  const prepareRestoreButton = slot.querySelector<HTMLButtonElement>("[data-wallet-prepare-restore]");
  const restoreButton = slot.querySelector<HTMLButtonElement>("[data-wallet-restore]");

  initPassphraseInput?.addEventListener("input", () => {
    drafts.walletInitPassphrase = initPassphraseInput.value;
  });
  unlockPassphraseInput?.addEventListener("input", () => {
    drafts.walletUnlockPassphrase = unlockPassphraseInput.value;
  });
  restorePassphraseInput?.addEventListener("input", () => {
    drafts.walletRestorePassphrase = restorePassphraseInput.value;
  });
  restoreSeedInput?.addEventListener("input", () => {
    drafts.walletRestoreSeed = restoreSeedInput.value;
  });

  initButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "wallet_initialize",
      async () => {
        const passphrase = (initPassphraseInput?.value ?? drafts.walletInitPassphrase).trim();
        if (!passphrase) return;
        drafts.walletInitPassphrase = "";
        await runApp((app) => app.initializeWallet({ passphrase }));
      },
      refresh,
    );
  });

  unlockButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "wallet_unlock",
      async () => {
        const passphrase = (unlockPassphraseInput?.value ?? drafts.walletUnlockPassphrase).trim();
        drafts.walletUnlockPassphrase = "";
        await runApp((app) =>
          app.unlockWallet(passphrase.length > 0 ? { passphrase } : undefined));
      },
      refresh,
    );
  });

  lockButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction("wallet_lock", () => runApp((app) => app.lockWallet()).then(() => undefined), refresh);
  });

  ackButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "wallet_ack_backup",
      () => runApp((app) => app.acknowledgeSeedBackup()).then(() => undefined),
      refresh,
    );
  });

  prepareRestoreButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "wallet_prepare_restore",
      () => runApp((app) => app.prepareWalletRestore()).then(() => undefined),
      refresh,
    );
  });

  restoreButton?.addEventListener("click", (event) => {
    event.preventDefault();
    runUiAction(
      "wallet_restore",
      async () => {
        const passphrase = (restorePassphraseInput?.value ?? drafts.walletRestorePassphrase).trim();
        const seed = parseSeedWords(restoreSeedInput?.value ?? drafts.walletRestoreSeed);
        if (!passphrase || seed.length === 0) return;
        drafts.walletRestorePassphrase = "";
        drafts.walletRestoreSeed = "";
        await runApp((app) =>
          app.restoreWallet({
            passphrase,
            seedMnemonic: seed,
          }));
      },
      refresh,
    );
  });
};

const addInitialPanes = (paneSystem: PaneSystemDom, paneRoot: HTMLElement): void => {
  addInitialDesktopPanes(paneSystem.store, paneScreen(paneRoot));
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
    const openPanes = openPaneIds(paneSystem);
    const history = deriveTransactionHistoryModel({
      tasks: currentTasks,
      loaded: loadedSnapshot,
      uiError,
    });

    const overviewSlot = paneSlot(paneRoot, "desktop-overview");
    const authSlot = paneSlot(paneRoot, "desktop-auth");
    const nodeSlot = paneSlot(paneRoot, "desktop-node");
    const walletSlot = paneSlot(paneRoot, "desktop-wallet");
    const executorSlot = paneSlot(paneRoot, "desktop-executor");
    const transactionsSlot = paneSlot(paneRoot, "desktop-transactions");

    if (overviewSlot) {
      await renderWithEffuse(overviewSlot, overviewTemplate({ snapshot: currentSnapshot, openPanes }));
      bindOverviewActions(overviewSlot, paneSystem, paneRoot, refreshView);
    }
    if (authSlot) {
      await renderWithEffuse(authSlot, authTemplate(currentSnapshot));
      bindAuthActions(authSlot, refreshView);
    }
    if (nodeSlot) {
      await renderWithEffuse(nodeSlot, nodeTemplate(currentSnapshot));
      bindNodeActions(nodeSlot, refreshView);
    }
    if (walletSlot) {
      await renderWithEffuse(walletSlot, walletTemplate(currentSnapshot));
      bindWalletActions(walletSlot, refreshView);
    }
    if (executorSlot) {
      await renderWithEffuse(executorSlot, executorTemplate(currentSnapshot, currentTasks));
      bindExecutorActions(executorSlot, refreshView);
    }
    if (transactionsSlot) {
      await renderWithEffuse(transactionsSlot, transactionsTemplate(history));
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
        loadedSnapshot = true;
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
