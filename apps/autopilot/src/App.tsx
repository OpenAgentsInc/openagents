import "./App.css";
import * as React from "react";
import { listen } from "@tauri-apps/api/event";
import {
  CheckCircle,
  Command as CommandIcon,
  Coins,
  FileCode,
  Moon,
  Pulse,
  ShieldCheck,
  Sun,
  TerminalWindow,
} from "@phosphor-icons/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "@/components/ui/command";
import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarShortcut,
  MenubarSub,
  MenubarSubContent,
  MenubarSubTrigger,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Separator } from "@/components/ui/separator";
import {
  actionBreadcrumb,
  type ActiveView,
  type AutopilotAction,
  buildAutopilotActions,
  proofLaneLabels,
  resolveRegisteredAction,
  type Theme,
  topLevelMenus,
  validateAutopilotActions,
  viewLabels,
} from "@/lib/autopilot-actions";
import {
  autopilotWorkbenchSnapshot,
  autopilotStatus,
  type AutopilotWorkbenchSnapshot,
  type ProviderMode,
  type ProofLane,
  type ProofNodeProjection,
  type ProofRunProjection,
  type PylonBinaryStatus,
  type PylonStatusProjection,
  proofDoctor,
  proofGet,
  proofOpenArtifacts,
  proofReset,
  proofRun,
  proofStop,
  pylonDetect,
  pylonGetStatus,
  pylonOpenLogs,
  pylonRestart,
  pylonSetMode,
  pylonStart,
  pylonStop,
} from "@/lib/autopilot-runtime";

type ActionRunner = (action: AutopilotAction) => Promise<void>;
type RegisterRow = [string, React.ReactNode];

type MenuBranch = {
  key: string;
  label: string;
  action?: AutopilotAction;
  children?: MenuBranch[];
  separatorBefore?: boolean;
};

function App() {
  const [activeView, setActiveView] = React.useState<ActiveView>("workbench");
  const [commandOpen, setCommandOpen] = React.useState(false);
  const [commandText, setCommandText] = React.useState("");
  const [consoleMessage, setConsoleMessage] = React.useState(
    "Open the command palette or enter an exact action ID.",
  );
  const [theme, setTheme] = useTheme();
  const [workbenchSnapshot, setWorkbenchSnapshot] =
    React.useState<AutopilotWorkbenchSnapshot | null>(null);
  const [pylonBinary, setPylonBinary] =
    React.useState<PylonBinaryStatus | null>(null);
  const [pylonStatus, setPylonStatus] =
    React.useState<PylonStatusProjection | null>(null);
  const [proofStatus, setProofStatus] =
    React.useState<ProofRunProjection | null>(null);
  const [namespaceDraft, setNamespaceDraft] = React.useState(
    makeProofNamespace("cs336-a1"),
  );
  const [busy, setBusy] = React.useState<string | null>(null);
  const [actionError, setActionError] = React.useState<string | null>(null);

  const refreshWorkbench = React.useCallback(async () => {
    setBusy("autopilot.workbench");
    try {
      const snapshot = await autopilotWorkbenchSnapshot();
      setWorkbenchSnapshot(snapshot);
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    } finally {
      setBusy(null);
    }
  }, []);

  const refreshPylon = React.useCallback(async () => {
    setBusy("pylon.refresh");
    try {
      const [binary, status] = await Promise.all([
        pylonDetect(),
        pylonGetStatus(),
      ]);
      setPylonBinary(binary);
      setPylonStatus(status);
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    } finally {
      setBusy(null);
    }
  }, []);

  const runPylonControl = React.useCallback(
    async (
      busyKey: string,
      control: () => Promise<PylonStatusProjection>,
      view: ActiveView = "pylon",
    ) => {
      setBusy(busyKey);
      try {
        const status = await control();
        setPylonStatus(status);
        setActiveView(view);
        setActionError(null);
      } catch (error) {
        setActionError(formatError(error));
      } finally {
        setBusy(null);
      }
    },
    [],
  );

  const setProviderMode = React.useCallback(
    (mode: ProviderMode) => {
      void runPylonControl(`pylon.${mode}`, () => pylonSetMode(mode));
    },
    [runPylonControl],
  );

  const runProofLane = React.useCallback(async (lane: ProofLane) => {
    const namespace = makeProofNamespace(lane);
    setNamespaceDraft(namespace);
    setActiveView("proof");
    setBusy(`proof.${lane}`);
    setProofStatus({
      namespace,
      lane,
      status: "running",
      firstRedStage: null,
      firstRedSubject: null,
      blockerId: null,
      detail: null,
      runId: null,
      windowId: null,
      assignmentId: null,
      leaseId: null,
      membershipRevision: null,
      closeoutStage: null,
      closeoutNextAction: null,
      closeoutLastError: null,
      workers: [],
      validators: [],
      transport: {
        authority: "running",
        relay: "running",
        artifactStore: "running",
        nodeSurfaces: "running",
      },
      artifacts: {
        root: "",
        runReportPath: null,
        authorityTracePath: null,
        summaryPath: null,
        artifactTracePath: null,
      },
      firstFailedAuthorityWrite: null,
      localSimulation: true,
      simulatedTreasury: true,
      updatedAt: String(Date.now()),
    });

    try {
      const status = await proofRun({ lane, namespace });
      setProofStatus(status);
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    } finally {
      setBusy(null);
    }
  }, []);

  const activeNamespace = proofStatus?.namespace || namespaceDraft;

  const runProofCommand = React.useCallback(
    async (
      busyKey: string,
      command: () => Promise<ProofRunProjection | unknown>,
    ) => {
      setBusy(busyKey);
      setActiveView("proof");
      try {
        const result = await command();
        if (isProofRunProjection(result)) {
          setProofStatus(result);
        } else {
          const latest = await proofGet(activeNamespace);
          setProofStatus(latest);
        }
        setActionError(null);
      } catch (error) {
        setActionError(formatError(error));
      } finally {
        setBusy(null);
      }
    },
    [activeNamespace],
  );

  const showControlStatus = React.useCallback(async () => {
    setBusy("autopilot.status");
    try {
      const status = await autopilotStatus();
      setActiveView("command");
      setConsoleMessage(
        `${status.product} ${status.shell}: authority ${status.rustAuthority}; lane ${status.runtimeLane}.`,
      );
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    } finally {
      setBusy(null);
    }
  }, []);

  const openPylonLogs = React.useCallback(async () => {
    try {
      const path = await pylonOpenLogs();
      setConsoleMessage(`Opened earn-runtime logs: ${path}`);
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    }
  }, []);

  const openProofArtifacts = React.useCallback(async () => {
    try {
      const path = await proofOpenArtifacts(activeNamespace);
      setConsoleMessage(`Opened diagnostic artifacts: ${path}`);
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    }
  }, [activeNamespace]);

  const pylonInstalled =
    pylonBinary?.installed ?? pylonStatus?.installed ?? true;

  const actions = React.useMemo(
    () =>
      buildAutopilotActions({
        activeView,
        busy,
        pylonInstalled,
        theme,
        setActiveView,
        setTheme,
        showControlStatus,
        refreshWorkbench,
        refreshPylon,
        openPylonLogs,
        startPylon: () => runPylonControl("pylon.start", pylonStart),
        stopPylon: () => runPylonControl("pylon.stop", pylonStop),
        restartPylon: () => runPylonControl("pylon.restart", pylonRestart),
        setProviderMode,
        showProofFlow: () => setActiveView("proof"),
        refreshProof: () =>
          runProofCommand("proof.refresh", () => proofGet(activeNamespace)),
        runProofLane,
        doctorProof: () =>
          runProofCommand("proof.doctor", () => proofDoctor(activeNamespace)),
        stopProof: () =>
          runProofCommand("proof.stop", () => proofStop(activeNamespace)),
        resetProof: () =>
          runProofCommand("proof.reset", () => proofReset(activeNamespace)),
        openProofArtifacts,
      }),
    [
      activeNamespace,
      activeView,
      busy,
      openProofArtifacts,
      openPylonLogs,
      pylonInstalled,
      refreshWorkbench,
      refreshPylon,
      runProofCommand,
      runProofLane,
      runPylonControl,
      setProviderMode,
      setTheme,
      showControlStatus,
      theme,
    ],
  );

  const actionById = React.useMemo(
    () => new Map(actions.map((action) => [action.id, action])),
    [actions],
  );

  const actionRegistryErrors = React.useMemo(
    () => validateAutopilotActions(actions),
    [actions],
  );

  React.useEffect(() => {
    if (actionRegistryErrors.length > 0) {
      console.error("Autopilot action registry errors", actionRegistryErrors);
      setActionError(actionRegistryErrors.join("; "));
    }
  }, [actionRegistryErrors]);

  const executeAction = React.useCallback<ActionRunner>(
    async (action) => {
      if (action.disabledReason) {
        setActionError(`${actionBreadcrumb(action)}: ${action.disabledReason}`);
        return;
      }

      if (
        action.kind === "destructive" &&
        !window.confirm(`${actionBreadcrumb(action)}\n\n${action.effect}`)
      ) {
        return;
      }

      await action.run();
      setCommandOpen(false);

      if (
        action.id !== "autopilot.runtime.status" &&
        action.id !== "help.control.status"
      ) {
        setConsoleMessage(`Ran ${actionBreadcrumb(action)}.`);
      }
    },
    [],
  );

  const submitCommand = React.useCallback(
    (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      const action = resolveRegisteredAction(actions, commandText);

      if (!commandText.trim()) {
        setCommandOpen(true);
        return;
      }

      if (!action) {
        setActionError(`No exact action matched: ${commandText}`);
        setConsoleMessage("Use Command-K for search, or enter an exact action ID.");
        return;
      }

      setCommandText("");
      void executeAction(action);
    },
    [actions, commandText, executeAction],
  );

  React.useEffect(() => {
    void refreshWorkbench();
  }, [refreshWorkbench]);

  React.useEffect(() => {
    void refreshPylon();
  }, [refreshPylon]);

  React.useEffect(() => {
    const unlisten: Array<() => void> = [];
    let mounted = true;

    void listen<PylonStatusProjection>("pylon://status", (event) => {
      if (mounted) {
        setPylonStatus(event.payload);
      }
    }).then((off) => unlisten.push(off));

    void listen<ProofRunProjection>("proof://status", (event) => {
      if (mounted) {
        setProofStatus(event.payload);
      }
    }).then((off) => unlisten.push(off));

    void listen<ProofRunProjection>("proof://summary", (event) => {
      if (mounted) {
        setProofStatus(event.payload);
      }
    }).then((off) => unlisten.push(off));

    void listen<ProofRunProjection>("proof://error", (event) => {
      if (mounted) {
        setProofStatus(event.payload);
        setActionError(event.payload.detail);
      }
    }).then((off) => unlisten.push(off));

    return () => {
      mounted = false;
      unlisten.forEach((off) => off());
    };
  }, []);

  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        setCommandOpen((open) => !open);
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const pylonCardActions = {
    refresh: requireAction(actionById, "pylon.refresh"),
    logs: requireAction(actionById, "pylon.logs.open"),
    start: requireAction(actionById, "pylon.serve.start"),
    stop: requireAction(actionById, "pylon.serve.stop"),
    restart: requireAction(actionById, "pylon.serve.restart"),
    modes: (["online", "offline", "pause", "resume"] as ProviderMode[]).map(
      (mode) => requireAction(actionById, `pylon.provider.${mode}`),
    ),
  };

  const proofCardActions = {
    refresh: requireAction(actionById, "proof.namespace.refresh"),
    artifacts: requireAction(actionById, "proof.artifacts.open"),
    lanes: (Object.keys(proofLaneLabels) as ProofLane[]).map((lane) =>
      requireAction(actionById, `proof.run.${lane}`),
    ),
    doctor: requireAction(actionById, "proof.namespace.doctor"),
    stop: requireAction(actionById, "proof.namespace.stop"),
    reset: requireAction(actionById, "proof.namespace.reset"),
  };

  return (
    <main className="shell">
      <section className="operator-stage">
        <div className="system-menu-strip">
          <div className="system-title">Autopilot</div>
          <AutopilotMenuBar actions={actions} onAction={executeAction} />
          <Button
            className="command-launcher"
            type="button"
            variant="outline"
            onClick={() => setCommandOpen(true)}
          >
            <CommandIcon aria-hidden="true" data-icon="inline-start" />
            Command
            <KbdGroup className="ml-1">
              <Kbd>cmd</Kbd>
              <Kbd>K</Kbd>
            </KbdGroup>
          </Button>
        </div>

        <div className="system-status-row">
          <div className="status-strip">
            <Badge variant="outline">{viewLabels[activeView]}</Badge>
            {activeView === "pylon" ? (
              <>
                <StateBadge value={pylonStatus?.processState ?? "unknown"} />
                <StateBadge value={pylonStatus?.providerState ?? "unknown"} />
              </>
            ) : null}
            {activeView === "proof" ? (
              <StateBadge value={proofStatus?.status ?? "no diagnostics"} />
            ) : null}
          </div>
          <div className="system-status-copy">
            {busy ? `busy ${busy}` : "ready"}
          </div>
        </div>

        <Separator />

        {actionError ? (
          <div className="operator-error" role="status">
            {actionError}
          </div>
        ) : null}

        <div className="operator-stage__content" aria-live="polite">
          {activeView === "workbench" ? (
            <AutopilotWorkbench
              busy={busy}
              onRefresh={refreshWorkbench}
              snapshot={workbenchSnapshot}
            />
          ) : activeView === "command" ? (
            <CommandEntry
              message={consoleMessage}
              value={commandText}
              onChange={setCommandText}
              onSubmit={submitCommand}
            />
          ) : activeView === "pylon" ? (
            <PylonStatusCard
              actions={pylonCardActions}
              binary={pylonBinary}
              onAction={executeAction}
              status={pylonStatus}
            />
          ) : (
            <ProofRunCard
              actions={proofCardActions}
              namespace={activeNamespace}
              onAction={executeAction}
              proof={proofStatus}
            />
          )}
        </div>
      </section>

      <AutopilotCommandPalette
        actions={actions}
        activeView={activeView}
        onAction={executeAction}
        onOpenChange={setCommandOpen}
        open={commandOpen}
      />
    </main>
  );
}

function AutopilotWorkbench({
  busy,
  onRefresh,
  snapshot,
}: {
  busy: string | null;
  onRefresh: () => Promise<void> | void;
  snapshot: AutopilotWorkbenchSnapshot | null;
}) {
  if (!snapshot) {
    return (
      <Card className="operator-card">
        <CardHeader className="operator-card__header">
          <CardTitle>Workbench</CardTitle>
          <Button disabled size="sm" type="button" variant="outline">
            Loading snapshot
          </Button>
        </CardHeader>
        <CardContent className="operator-card__content">
          <div className="workbench-empty">
            Waiting for Rust-owned Autopilot workbench state.
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <section className="workbench" aria-label="Autopilot workbench">
      <aside className="workbench-rail">
        <Card className="workbench-panel">
          <CardHeader className="workbench-panel__header">
            <CardTitle>Workspace</CardTitle>
            <Button
              disabled={busy === "autopilot.workbench"}
              size="sm"
              type="button"
              variant="ghost"
              onClick={() => void onRefresh()}
            >
              Refresh
            </Button>
          </CardHeader>
          <CardContent className="workbench-panel__content">
            <div className="workbench-object-title">{snapshot.workspace.name}</div>
            <RegisterGrid
              rows={[
                ["path", snapshot.workspace.path],
                ["branch", snapshot.workspace.branch],
                ["trust", snapshot.workspace.trust],
                ["policy", snapshot.workspace.policy],
                ["updated", formatTimestamp(String(snapshot.generatedAtUnixMs))],
              ]}
            />
          </CardContent>
        </Card>

        <Card className="workbench-panel">
          <CardHeader className="workbench-panel__header">
            <CardTitle>Scorecard</CardTitle>
            <Coins aria-hidden="true" />
          </CardHeader>
          <CardContent className="workbench-panel__content">
            <div className="score-grid">
              <MetricCell
                label="first tool"
                value={`${snapshot.scorecard.firstToolEventSeconds}s`}
              />
              <MetricCell
                label="verified diff"
                value={`${snapshot.scorecard.verifiedDiffMinutes}m`}
              />
              <MetricCell
                label="interventions"
                value={String(snapshot.scorecard.humanInterventions)}
              />
              <MetricCell
                label="earned today"
                value={`${snapshot.scorecard.satsEarnedToday} sats`}
              />
            </div>
            <div className="workbench-note">
              {snapshot.scorecard.recoveryState}
            </div>
          </CardContent>
        </Card>
      </aside>

      <section className="workbench-main">
        <Card className="workbench-panel workbench-session">
          <CardHeader className="workbench-panel__header">
            <div>
              <CardTitle>{snapshot.session.title}</CardTitle>
              <p>{snapshot.session.goal}</p>
            </div>
            <StateBadge value={snapshot.session.state} />
          </CardHeader>
          <CardContent className="workbench-panel__content">
            <div className="timeline" aria-label="Run timeline">
              {snapshot.timeline.map((event) => (
                <article className="timeline-event" data-state={stateTone(event.state)} key={event.id}>
                  <div className="timeline-event__time">{event.time}</div>
                  <div className="timeline-event__body">
                    <div className="timeline-event__heading">
                      <strong>{event.label}</strong>
                      <StateBadge value={event.state} />
                    </div>
                    <p>{event.detail}</p>
                    <div className="timeline-event__meta">
                      <span>{event.owner}</span>
                      <span>{event.evidence}</span>
                    </div>
                  </div>
                </article>
              ))}
            </div>
          </CardContent>
        </Card>
      </section>

      <aside className="workbench-inspector">
        <WorkbenchList
          icon={<ShieldCheck aria-hidden="true" />}
          items={snapshot.approvals.map((approval) => ({
            id: approval.id,
            state: approval.state,
            title: approval.request,
            detail: `${approval.risk} risk / ${approval.policy}`,
            meta: approval.paths.join(", "),
          }))}
          title="Approvals"
        />
        <WorkbenchList
          icon={<FileCode aria-hidden="true" />}
          items={snapshot.diffs.map((diff) => ({
            id: diff.id,
            state: diff.state,
            title: diff.file,
            detail: diff.summary,
            meta: `+${diff.additions} / -${diff.deletions}`,
          }))}
          title="Diffs"
        />
        <WorkbenchList
          icon={<TerminalWindow aria-hidden="true" />}
          items={snapshot.verification.map((verification) => ({
            id: verification.id,
            state: verification.state,
            title: verification.command,
            detail: verification.detail,
            meta: `${verification.elapsedMs}ms`,
          }))}
          title="Verification"
        />
        <WorkbenchList
          icon={<CheckCircle aria-hidden="true" />}
          items={snapshot.evidence.map((evidence) => ({
            id: evidence.id,
            state: evidence.state,
            title: evidence.location,
            detail: `${evidence.kind} / ${evidence.owner}`,
            meta: evidence.id,
          }))}
          title="Evidence"
        />
      </aside>
    </section>
  );
}

function MetricCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric-cell">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function WorkbenchList({
  icon,
  items,
  title,
}: {
  icon: React.ReactNode;
  items: Array<{
    id: string;
    state: string;
    title: string;
    detail: string;
    meta: string;
  }>;
  title: string;
}) {
  return (
    <Card className="workbench-panel">
      <CardHeader className="workbench-panel__header">
        <CardTitle>{title}</CardTitle>
        {icon}
      </CardHeader>
      <CardContent className="workbench-panel__content">
        <div className="workbench-list">
          {items.map((item) => (
            <article className="workbench-list-item" key={item.id}>
              <div className="workbench-list-item__head">
                <strong>{item.title}</strong>
                <StateBadge value={item.state} />
              </div>
              <p>{item.detail}</p>
              <span>{item.meta}</span>
            </article>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function AutopilotMenuBar({
  actions,
  onAction,
}: {
  actions: AutopilotAction[];
  onAction: ActionRunner;
}) {
  return (
    <Menubar className="system-menubar">
      {topLevelMenus.map((menu) => {
        const branches = buildMenuBranches(
          actions.filter((action) => action.menuPath[0] === menu),
        );

        return (
          <MenubarMenu key={menu}>
            <MenubarTrigger>{menu}</MenubarTrigger>
            <MenubarContent className="system-menu-content">
              {branches.map((branch) => (
                <MenuBranchItem
                  branch={branch}
                  key={branch.key}
                  onAction={onAction}
                />
              ))}
            </MenubarContent>
          </MenubarMenu>
        );
      })}
    </Menubar>
  );
}

function MenuBranchItem({
  branch,
  onAction,
}: {
  branch: MenuBranch;
  onAction: ActionRunner;
}) {
  return (
    <React.Fragment>
      {branch.separatorBefore ? <MenubarSeparator /> : null}
      {branch.children ? (
        <MenubarSub>
          <MenubarSubTrigger>{branch.label}</MenubarSubTrigger>
          <MenubarSubContent className="system-menu-content">
            {branch.children.map((child) => (
              <MenuBranchItem
                branch={child}
                key={child.key}
                onAction={onAction}
              />
            ))}
          </MenubarSubContent>
        </MenubarSub>
      ) : branch.action ? (
        <MenubarItem
          data-checked={branch.action.active}
          disabled={Boolean(branch.action.disabledReason)}
          title={branch.action.disabledReason ?? branch.action.effect}
          variant={branch.action.kind === "destructive" ? "destructive" : "default"}
          onClick={() => void onAction(branch.action as AutopilotAction)}
        >
          <ActionGlyph action={branch.action} />
          <span>{branch.label}</span>
          {branch.action.disabledReason ? (
            <span className="menu-disabled-reason">
              {branch.action.disabledReason}
            </span>
          ) : null}
          <MenubarShortcut>
            {branch.action.shortcut ?? branch.action.kind}
          </MenubarShortcut>
        </MenubarItem>
      ) : null}
    </React.Fragment>
  );
}

function AutopilotCommandPalette({
  actions,
  activeView,
  onAction,
  onOpenChange,
  open,
}: {
  actions: AutopilotAction[];
  activeView: ActiveView;
  onAction: ActionRunner;
  onOpenChange: (open: boolean) => void;
  open: boolean;
}) {
  const orderedMenus = orderCommandMenus(activeView);

  return (
    <CommandDialog
      className="sm:max-w-3xl"
      description="Search registered Autopilot commands."
      onOpenChange={onOpenChange}
      open={open}
      title="Autopilot Command Menu"
    >
      <Command label="Autopilot commands" loop>
        <CommandInput placeholder="Search actions by workspace, earn state, diagnostics, or exact ID..." />
        <CommandList className="max-h-[30rem]">
          <CommandEmpty>No registered command matched.</CommandEmpty>
          {orderedMenus.map((menu) => {
            const menuActions = actions.filter(
              (action) => action.menuPath[0] === menu,
            );

            if (menuActions.length === 0) {
              return null;
            }

            return (
              <CommandGroup heading={menu} key={menu}>
                {menuActions.map((action) => (
                  <CommandItem
                    data-checked={action.active}
                    disabled={Boolean(action.disabledReason)}
                    key={action.id}
                    keywords={[
                      action.id,
                      actionBreadcrumb(action),
                      action.authority,
                      action.kind,
                      ...action.paletteKeywords,
                    ]}
                    value={`${action.id} ${actionBreadcrumb(action)}`}
                    onSelect={() => void onAction(action)}
                  >
                    <ActionGlyph action={action} />
                    <span className="command-row-copy">
                      <span className="command-row-label">{action.label}</span>
                      <span className="command-row-path">
                        {actionBreadcrumb(action)}
                      </span>
                      <span className="command-row-meta">
                        {action.kind} / {action.authority}
                        {action.disabledReason ? ` / ${action.disabledReason}` : ""}
                      </span>
                    </span>
                    <CommandShortcut>
                      {action.shortcut ?? action.id}
                    </CommandShortcut>
                  </CommandItem>
                ))}
              </CommandGroup>
            );
          })}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}

function CommandEntry({
  message,
  value,
  onChange,
  onSubmit,
}: {
  message: string;
  value: string;
  onChange: (value: string) => void;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <section className="command-home" aria-label="Command console">
      <p>{message}</p>
      <form className="command-entry" onSubmit={onSubmit}>
        <FieldGroup>
          <Field>
            <FieldLabel className="sr-only" htmlFor="autopilot-command">
              Command
            </FieldLabel>
            <Input
              autoComplete="off"
              id="autopilot-command"
              value={value}
              onChange={(event) => onChange(event.target.value)}
              placeholder="action id, for example view.workbench"
            />
          </Field>
        </FieldGroup>
        <Button type="submit" variant="outline">
          Run
        </Button>
      </form>
    </section>
  );
}

function PylonStatusCard({
  actions,
  binary,
  onAction,
  status,
}: {
  actions: {
    refresh: AutopilotAction;
    logs: AutopilotAction;
    start: AutopilotAction;
    stop: AutopilotAction;
    restart: AutopilotAction;
    modes: AutopilotAction[];
  };
  binary: PylonBinaryStatus | null;
  onAction: ActionRunner;
  status: PylonStatusProjection | null;
}) {
  const rows: RegisterRow[] = [
    ["binary", binary?.binaryPath ?? status?.binaryPath ?? "not found"],
    ["binary source", binary?.source ?? "unknown"],
    ["config", status?.configPath ?? "not loaded"],
    ["runtime home", status?.pylonHome ?? "not loaded"],
    ["process", status?.processState ?? "unknown"],
    ["pid", status?.pid ?? "none"],
    ["provider", status?.providerState ?? "unknown"],
    ["desired mode", status?.desiredMode ?? "unknown"],
    ["listen", status?.listenAddr ?? "none"],
    ["backend", status?.executionBackend ?? "unknown"],
    ["ready model", status?.readyModel ?? "none"],
    ["products", formatCountPair(status?.productsEligible, status?.productsVisible)],
    ["queue", status?.queueDepth ?? "unknown"],
    ["uptime", formatDuration(status?.uptimeSeconds)],
    ["blockers", status?.blockerCodes.length ? status.blockerCodes.join(", ") : "none"],
    ["last action", status?.lastAction ?? "none"],
    ["last error", status?.lastError ?? "none"],
    ["updated", status ? formatTimestamp(status.lastUpdatedAt) : "not loaded"],
  ];

  return (
    <Card className="operator-card">
      <CardHeader className="operator-card__header">
        <CardTitle>Earn Runtime</CardTitle>
        <div className="button-row">
          <ActionButton action={actions.refresh} onAction={onAction} />
          <ActionButton action={actions.logs} onAction={onAction} />
        </div>
      </CardHeader>
      <CardContent className="operator-card__content">
        <div className="button-row button-row--wrap">
          <ActionButton action={actions.start} onAction={onAction} variant="default" />
          <ActionButton action={actions.stop} onAction={onAction} />
          <ActionButton action={actions.restart} onAction={onAction} />
          {actions.modes.map((action) => (
            <ActionButton action={action} key={action.id} onAction={onAction} />
          ))}
        </div>
        <RegisterGrid rows={rows} />
      </CardContent>
    </Card>
  );
}

function ProofRunCard({
  actions,
  namespace,
  onAction,
  proof,
}: {
  actions: {
    refresh: AutopilotAction;
    artifacts: AutopilotAction;
    lanes: AutopilotAction[];
    doctor: AutopilotAction;
    stop: AutopilotAction;
    reset: AutopilotAction;
  };
  namespace: string;
  onAction: ActionRunner;
  proof: ProofRunProjection | null;
}) {
  const rows: RegisterRow[] = [
    ["namespace", proof?.namespace ?? namespace],
    ["lane", proof?.lane ?? "none"],
    ["status", proof?.status ?? "idle"],
    ["run", proof?.runId ?? "none"],
    ["window", proof?.windowId ?? "none"],
    ["assignment", proof?.assignmentId ?? "none"],
    ["lease", proof?.leaseId ?? "none"],
    ["membership", proof?.membershipRevision ?? "none"],
    ["first red stage", proof?.firstRedStage ?? "none"],
    ["first red subject", proof?.firstRedSubject ?? "none"],
    ["blocker", proof?.blockerId ?? "none"],
    ["detail", proof?.detail ?? "none"],
    ["closeout", proof?.closeoutStage ?? "none"],
    ["next action", proof?.closeoutNextAction ?? "none"],
    ["closeout error", proof?.closeoutLastError ?? "none"],
    ["authority write", proof?.firstFailedAuthorityWrite?.detail ?? "none"],
    ["artifacts", proof?.artifacts.root || "none"],
    ["updated", proof ? formatTimestamp(proof.updatedAt) : "not loaded"],
  ];

  return (
    <Card className="operator-card">
      <CardHeader className="operator-card__header">
        <CardTitle>Diagnostics Flow</CardTitle>
        <div className="button-row">
          <ActionButton action={actions.refresh} onAction={onAction} />
          <ActionButton action={actions.artifacts} onAction={onAction} />
        </div>
      </CardHeader>
      <CardContent className="operator-card__content">
        <div className="button-row button-row--wrap">
          {actions.lanes.map((action) => (
            <ActionButton
              action={action}
              key={action.id}
              onAction={onAction}
              variant="default"
            />
          ))}
          <ActionButton action={actions.doctor} onAction={onAction} />
          <ActionButton action={actions.stop} onAction={onAction} />
          <ActionButton action={actions.reset} onAction={onAction} />
        </div>
        <ProofStageGrid proof={proof} />
        <RegisterGrid rows={rows} />
        <ProofNodeTable label="Workers" nodes={proof?.workers ?? []} />
        <ProofNodeTable label="Validators" nodes={proof?.validators ?? []} />
      </CardContent>
    </Card>
  );
}

function ActionButton({
  action,
  onAction,
  variant,
}: {
  action: AutopilotAction;
  onAction: ActionRunner;
  variant?: "default" | "outline";
}) {
  const resolvedVariant =
    variant ?? (action.kind === "destructive" ? "destructive" : "outline");

  return (
    <Button
      disabled={Boolean(action.disabledReason)}
      size="sm"
      title={action.disabledReason ?? action.effect}
      type="button"
      variant={resolvedVariant}
      onClick={() => void onAction(action)}
    >
      <ActionGlyph action={action} />
      {action.label}
    </Button>
  );
}

function ActionGlyph({ action }: { action: AutopilotAction }) {
  if (action.authority === "pylon") {
    return <Pulse aria-hidden="true" data-icon="inline-start" />;
  }

  if (action.authority === "proof") {
    return <CheckCircle aria-hidden="true" data-icon="inline-start" />;
  }

  if (action.authority === "theme") {
    return action.id.endsWith(".light") ? (
      <Sun aria-hidden="true" data-icon="inline-start" />
    ) : (
      <Moon aria-hidden="true" data-icon="inline-start" />
    );
  }

  return <CommandIcon aria-hidden="true" data-icon="inline-start" />;
}

function ProofStageGrid({ proof }: { proof: ProofRunProjection | null }) {
  const stages = [
    ["authority", proof?.transport.authority ?? "unknown"],
    ["relay", proof?.transport.relay ?? "unknown"],
    ["artifact store", proof?.transport.artifactStore ?? "unknown"],
    ["node surfaces", proof?.transport.nodeSurfaces ?? "unknown"],
    ["workers", nodeGroupState(proof?.workers ?? [])],
    ["validators", nodeGroupState(proof?.validators ?? [])],
    ["treasury", proof?.simulatedTreasury ? "simulated" : "unknown"],
    ["closeout", proof?.closeoutStage ?? proof?.status ?? "idle"],
  ] as const;

  return (
    <div className="proof-stage-grid" aria-label="Diagnostics stages">
      {stages.map(([label, state]) => (
        <div className="proof-stage" data-state={stateTone(state)} key={label}>
          <span>{label}</span>
          <strong>{state}</strong>
        </div>
      ))}
    </div>
  );
}

function ProofNodeTable({
  label,
  nodes,
}: {
  label: string;
  nodes: ProofNodeProjection[];
}) {
  return (
    <div className="node-table">
      <div className="node-table__title">{label}</div>
      <div className="node-table__grid" role="table" aria-label={label}>
        <div role="row">
          <span role="columnheader">node</span>
          <span role="columnheader">running</span>
          <span role="columnheader">eligibility</span>
          <span role="columnheader">fixture</span>
          <span role="columnheader">status</span>
        </div>
        {nodes.length === 0 ? (
          <div role="row">
            <span role="cell">none</span>
            <span role="cell">false</span>
            <span role="cell">none</span>
            <span role="cell">none</span>
            <span role="cell">idle</span>
          </div>
        ) : (
          nodes.map((node) => (
            <div role="row" key={`${node.role}-${node.index}`}>
              <span role="cell">{node.label}</span>
              <span role="cell">{String(node.running)}</span>
              <span role="cell">
                {node.eligibility ?? (node.hardGateReasons.join(", ") || "none")}
              </span>
              <span role="cell">{node.retainedStateFixtureId ?? "none"}</span>
              <span role="cell">
                {node.trainingError ?? node.trainingStatus ?? "idle"}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function RegisterGrid({ rows }: { rows: RegisterRow[] }) {
  return (
    <dl className="register-grid">
      {rows.map(([label, value], index) => (
        <div key={`${label}-${index}`}>
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function StateBadge({ value }: { value: string }) {
  return (
    <Badge className="state-badge" data-state={stateTone(value)} variant="outline">
      {value}
    </Badge>
  );
}

function useTheme(): [Theme, React.Dispatch<React.SetStateAction<Theme>>] {
  const [theme, setTheme] = React.useState<Theme>(() => {
    const storedTheme = window.localStorage.getItem("autopilot.theme");

    return storedTheme === "light" ? "light" : "dark";
  });

  React.useLayoutEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", theme === "dark");
    root.classList.toggle("light", theme === "light");
    root.style.colorScheme = theme;
    window.localStorage.setItem("autopilot.theme", theme);
  }, [theme]);

  return [theme, setTheme];
}

function buildMenuBranches(actions: AutopilotAction[], depth = 1): MenuBranch[] {
  const branches: MenuBranch[] = [];
  const nested = new Map<string, { branch: MenuBranch; actions: AutopilotAction[] }>();

  for (const action of actions) {
    const segment = action.menuPath[depth];

    if (!segment) {
      continue;
    }

    if (action.menuPath.length === depth + 1) {
      branches.push({
        action,
        key: action.id,
        label: segment,
        separatorBefore: action.separatorBefore,
      });
      continue;
    }

    let entry = nested.get(segment);

    if (!entry) {
      entry = {
        actions: [],
        branch: {
          key: `${depth}-${segment}`,
          label: segment,
          separatorBefore: action.separatorBefore,
        },
      };
      nested.set(segment, entry);
      branches.push(entry.branch);
    }

    entry.actions.push(action);
  }

  for (const entry of nested.values()) {
    entry.branch.children = buildMenuBranches(entry.actions, depth + 1);
  }

  return branches;
}

function orderCommandMenus(activeView: ActiveView) {
  const activeMenu =
    activeView === "pylon"
      ? "Earn"
      : activeView === "proof"
        ? "Diagnostics"
        : "View";

  return [
    activeMenu,
    ...topLevelMenus.filter((menu) => menu !== activeMenu),
  ];
}

function requireAction(
  actions: Map<string, AutopilotAction>,
  id: string,
): AutopilotAction {
  const action = actions.get(id);

  if (!action) {
    throw new Error(`missing registered action: ${id}`);
  }

  return action;
}

function makeProofNamespace(lane: ProofLane) {
  return `proof.autopilot.${lane.replace(/-/g, ".")}.${Date.now()}`;
}

function isProofRunProjection(value: unknown): value is ProofRunProjection {
  return Boolean(
    value &&
      typeof value === "object" &&
      "namespace" in value &&
      "status" in value &&
      "transport" in value,
  );
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function formatTimestamp(value: string) {
  const numeric = Number(value);

  if (Number.isFinite(numeric) && numeric > 0) {
    return new Date(numeric).toLocaleString();
  }

  return value;
}

function formatDuration(seconds: number | null | undefined) {
  if (seconds === null || seconds === undefined) {
    return "unknown";
  }

  return `${seconds}s`;
}

function formatCountPair(eligible: number | null | undefined, visible: number | null | undefined) {
  if (eligible === null || eligible === undefined || visible === null || visible === undefined) {
    return "unknown";
  }

  return `${eligible}/${visible}`;
}

function nodeGroupState(nodes: ProofNodeProjection[]) {
  if (nodes.length === 0) {
    return "none";
  }

  if (nodes.some((node) => node.trainingError || node.hardGateReasons.length > 0)) {
    return "failed";
  }

  if (nodes.every((node) => node.trainingStatus === "completed")) {
    return "ok";
  }

  if (nodes.some((node) => node.running || node.trainingStatus)) {
    return "running";
  }

  return "ready";
}

function stateTone(value: string) {
  const normalized = value.toLowerCase();

  if (
    normalized.includes("ok") ||
    normalized.includes("online") ||
    normalized.includes("ready") ||
    normalized.includes("completed") ||
    normalized.includes("accepted")
  ) {
    return "ok";
  }

  if (
    normalized.includes("fail") ||
    normalized.includes("error") ||
    normalized.includes("down") ||
    normalized.includes("red")
  ) {
    return "bad";
  }

  if (
    normalized.includes("warn") ||
    normalized.includes("degraded") ||
    normalized.includes("simulated") ||
    normalized.includes("unconfigured")
  ) {
    return "warn";
  }

  if (normalized.includes("running") || normalized.includes("starting")) {
    return "active";
  }

  return "neutral";
}

export default App;
