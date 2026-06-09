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
  type AutopilotNexusHealthProjection,
  autopilotWorkbenchSnapshot,
  autopilotStatus,
  type AutopilotWorkbenchSnapshot,
  type HomeworkAssignmentProjection,
  type HomeworkRuntimeProjection,
  type HomeworkSnapshotProjection,
  type ProviderMode,
  type ProofLane,
  type ProofNodeProjection,
  type ProofRunProjection,
  type PylonBinaryStatus,
  type PylonStatusProjection,
  nexusHealthStatus,
  pylonHomeworkGet,
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
  const [activeView, setActiveView] = React.useState<ActiveView>("homework");
  const [commandOpen, setCommandOpen] = React.useState(false);
  const [commandText, setCommandText] = React.useState("");
  const [consoleMessage, setConsoleMessage] = React.useState(
    "Open the command palette or enter an exact action ID.",
  );
  const [theme, setTheme] = useTheme();
  const [workbenchSnapshot, setWorkbenchSnapshot] =
    React.useState<AutopilotWorkbenchSnapshot | null>(null);
  const [homeworkSnapshot, setHomeworkSnapshot] =
    React.useState<HomeworkSnapshotProjection | null>(null);
  const [nexusHealth, setNexusHealth] =
    React.useState<AutopilotNexusHealthProjection | null>(null);
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

  const refreshHomework = React.useCallback(async () => {
    setBusy("homework.refresh");
    try {
      const snapshot = await pylonHomeworkGet();
      setHomeworkSnapshot(snapshot);
      setPylonStatus(snapshot.pylon);
      if (snapshot.proof) {
        setProofStatus(snapshot.proof);
      }
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    } finally {
      setBusy(null);
    }
  }, []);

  const refreshHealth = React.useCallback(async () => {
    setBusy("health.nexus");
    try {
      const snapshot = await nexusHealthStatus();
      setNexusHealth(snapshot);
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
      void runPylonControl(
        `pylon.${mode}`,
        () => pylonSetMode(mode),
        activeView === "homework" ? "homework" : "pylon",
      );
    },
    [activeView, runPylonControl],
  );

  const refreshHomeworkProjection = React.useCallback(async () => {
    const snapshot = await pylonHomeworkGet();
    setHomeworkSnapshot(snapshot);
    setPylonStatus(snapshot.pylon);
    if (snapshot.proof) {
      setProofStatus(snapshot.proof);
    }
    return snapshot;
  }, []);

  const goOnline = React.useCallback(async () => {
    setBusy("pylon.go-online");
    setActiveView("homework");
    try {
      if (pylonStatus?.processState !== "running") {
        const starting = await pylonStart();
        setPylonStatus(starting);
      }
      const online = await pylonSetMode("online");
      setPylonStatus(online);
      await refreshHomeworkProjection();
      setConsoleMessage("Pylon is online and eligible for homework jobs.");
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    } finally {
      setBusy(null);
    }
  }, [pylonStatus?.processState, refreshHomeworkProjection]);

  const goOffline = React.useCallback(async () => {
    setBusy("pylon.go-offline");
    setActiveView("homework");
    try {
      const offline = await pylonSetMode("offline");
      setPylonStatus(offline);
      await refreshHomeworkProjection();
      setConsoleMessage("Pylon is offline and no longer eligible for jobs.");
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    } finally {
      setBusy(null);
    }
  }, [refreshHomeworkProjection]);

  const runProofLane = React.useCallback(async (lane: ProofLane) => {
    const namespace = makeProofNamespace(lane);
    setNamespaceDraft(namespace);
    setActiveView(activeView === "homework" ? "homework" : "proof");
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
      if (activeView === "homework") {
        setHomeworkSnapshot((current) =>
          current
            ? {
                ...current,
                proof: status,
                status: status.status === "accepted" ? "Homework paid" : current.status,
                detail: status.detail ?? current.detail,
                updatedAt: String(Date.now()),
              }
            : current,
        );
      }
      setActionError(null);
    } catch (error) {
      setActionError(formatError(error));
    } finally {
      setBusy(null);
    }
  }, [activeView]);

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
        refreshHomework,
        refreshHealth,
        refreshPylon,
        openPylonLogs,
        startPylon: () =>
          runPylonControl(
            "pylon.start",
            pylonStart,
            activeView === "homework" ? "homework" : "pylon",
          ),
        stopPylon: () =>
          runPylonControl(
            "pylon.stop",
            pylonStop,
            activeView === "homework" ? "homework" : "pylon",
          ),
        restartPylon: () =>
          runPylonControl(
            "pylon.restart",
            pylonRestart,
            activeView === "homework" ? "homework" : "pylon",
          ),
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
      refreshHomework,
      refreshHealth,
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
    void refreshHomework();
  }, [refreshHomework]);

  React.useEffect(() => {
    void refreshHealth();
  }, [refreshHealth]);

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

  const homeworkActions = {
    refresh: requireAction(actionById, "homework.refresh"),
    start: requireAction(actionById, "pylon.serve.start"),
    online: requireAction(actionById, "pylon.provider.online"),
    offline: requireAction(actionById, "pylon.provider.offline"),
    pause: requireAction(actionById, "pylon.provider.pause"),
    stop: requireAction(actionById, "pylon.serve.stop"),
    runProof: requireAction(actionById, "proof.run.cs336-a1"),
    doctor: requireAction(actionById, "proof.namespace.doctor"),
    artifacts: requireAction(actionById, "proof.artifacts.open"),
  };

  const healthActions = {
    refresh: requireAction(actionById, "health.nexus.refresh"),
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
            {activeView === "homework" ? (
              <StateBadge value={homeworkSnapshot?.status ?? "not loaded"} />
            ) : null}
            {activeView === "health" ? (
              <>
                <StateBadge value={nexusHealth?.state ?? "not loaded"} />
                <StateBadge value={nexusHealth?.severity ?? "unknown"} />
              </>
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
          ) : activeView === "homework" ? (
            <HomeworkPylonScreen
              actions={homeworkActions}
              busy={busy}
              onGoOffline={goOffline}
              onGoOnline={goOnline}
              onAction={executeAction}
              snapshot={homeworkSnapshot}
            />
          ) : activeView === "health" ? (
            <NexusHealthPanel
              actions={healthActions}
              busy={busy}
              onAction={executeAction}
              snapshot={nexusHealth}
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

function HomeworkPylonScreen({
  actions,
  busy,
  onGoOffline,
  onGoOnline,
  onAction,
  snapshot,
}: {
  actions: {
    refresh: AutopilotAction;
    start: AutopilotAction;
    online: AutopilotAction;
    offline: AutopilotAction;
    pause: AutopilotAction;
    stop: AutopilotAction;
    runProof: AutopilotAction;
    doctor: AutopilotAction;
    artifacts: AutopilotAction;
  };
  busy: string | null;
  onGoOffline: () => Promise<void>;
  onGoOnline: () => Promise<void>;
  onAction: ActionRunner;
  snapshot: HomeworkSnapshotProjection | null;
}) {
  if (!snapshot) {
    return (
      <Card className="operator-card">
        <CardHeader className="operator-card__header">
          <CardTitle>Homework</CardTitle>
          <ActionButton action={actions.refresh} onAction={onAction} />
        </CardHeader>
        <CardContent className="operator-card__content">
          <div className="workbench-empty">
            Loading current Pylon homework state.
          </div>
        </CardContent>
      </Card>
    );
  }

  const training = snapshot.training;
  const activeRuntime = training?.activeRuntime ?? null;
  const activeAssignment = activeRuntime
    ? runtimeAsAssignment(activeRuntime)
    : training?.leasedAssignment ?? training?.recentWorkOffers[0] ?? null;
  const closeout = training?.recentCloseoutProgress[0] ?? null;
  const pylonRows: RegisterRow[] = [
    ["process", snapshot.pylon.processState],
    ["provider", snapshot.pylon.providerState],
    ["mode", snapshot.pylon.desiredMode ?? "unknown"],
    ["pid", snapshot.pylon.pid ?? "none"],
    ["eligible products", formatCountPair(snapshot.pylon.productsEligible, snapshot.pylon.productsVisible)],
    ["queue", snapshot.pylon.queueDepth ?? "unknown"],
    ["model", snapshot.pylon.readyModel ?? "none"],
    ["backend", snapshot.pylon.executionBackend ?? "unknown"],
    ["config", snapshot.pylon.configPath ?? "not loaded"],
  ];
  const trainingRows: RegisterRow[] = [
    ["node", training?.nodeLabel ?? "not loaded"],
    ["run", training?.currentRunId ?? snapshot.proof?.runId ?? "waiting"],
    ["window", training?.activeWindowId ?? snapshot.proof?.windowId ?? "waiting"],
    ["checkpoint", training?.checkpointServeUrl || "not serving"],
    ["runtime surface", String(training?.runtimeSurfaceDetected ?? false)],
    ["contributor", String(training?.contributorSupported ?? false)],
    ["manifests", training?.manifestCount ?? 0],
    ["work offers", training?.workOfferCount ?? 0],
    ["closeouts", training?.closeoutCount ?? 0],
    ["TRN events", training?.recentTrnEventCount ?? 0],
  ];
  const assignmentRows = activeAssignment
    ? homeworkAssignmentRows(activeAssignment)
    : [["assignment", "waiting for admin-launched homework"]] satisfies RegisterRow[];
  const isOnline = snapshot.pylon.providerState === "online";
  const isGoOnlineBusy = busy === "pylon.go-online";
  const isGoOfflineBusy = busy === "pylon.go-offline";
  const goOnlineDisabledReason = isOnline
    ? actions.offline.disabledReason
    : actions.start.disabledReason ?? actions.online.disabledReason;
  const goOnlineButtonDisabled = Boolean(
    goOnlineDisabledReason || isGoOnlineBusy || isGoOfflineBusy,
  );
  const goOnlineButtonLabel = isOnline
    ? isGoOfflineBusy
      ? "GOING OFFLINE"
      : "GO OFFLINE"
    : isGoOnlineBusy || snapshot.pylon.processState === "starting"
      ? "STARTING"
      : "GO ONLINE";
  const goOnlineButtonDetail = isOnline
    ? "Eligible for admin-launched homework jobs now."
    : snapshot.pylon.processState === "running"
      ? "Runtime is running. Click to become eligible for jobs."
      : "Starts Pylon and marks this machine eligible for jobs.";
  const missionStats: RegisterRow[] = [
    ["status", snapshot.status],
    ["process", snapshot.pylon.processState],
    ["provider", snapshot.pylon.providerState],
    ["offers", training?.workOfferCount ?? 0],
    ["assignment", activeAssignment?.assignmentId ?? "waiting"],
    ["payout", closeout?.payoutState ?? snapshot.proof?.status ?? "waiting"],
  ];
  const payoutRows: RegisterRow[] = [
    ["policy", snapshot.payoutPolicy],
    ["stage", closeout?.stage ?? snapshot.proof?.closeoutStage ?? "waiting"],
    ["acceptance", closeout?.acceptanceState ?? "none"],
    ["accepted outcome", closeout?.acceptedOutcomeId ?? "none"],
    ["payout", closeout?.payoutState ?? snapshot.proof?.status ?? "none"],
    ["payout id", closeout?.payoutId ?? "none"],
    ["payment", closeout?.payoutReceiptId ?? "none"],
    ["reconciliation", closeout?.payoutReconciliationStatus ?? "none"],
  ];

  return (
    <section className="homework-screen" aria-label="Pylon homework control">
      <Card className="homework-hero">
        <CardContent className="homework-hero__content">
          <div className="mission-control-hero">
            <div className="mission-control-copy">
              <span className="mission-control-kicker">Autopilot - Mission Control</span>
              <h1>{isOnline ? "Online for jobs" : "Ready to earn"}</h1>
              <span className="mission-control-assignment">
                {snapshot.assignmentLabel}
              </span>
              <p>{snapshot.detail}</p>
              <div className="status-strip">
                <StateBadge value={snapshot.status} />
                <StateBadge value={snapshot.pylon.providerState} />
                <StateBadge value={snapshot.pylon.processState} />
              </div>
            </div>
            <button
              className="go-online-button"
              data-state={isOnline ? "online" : "offline"}
              disabled={goOnlineButtonDisabled}
              title={goOnlineDisabledReason ?? goOnlineButtonDetail}
              type="button"
              onClick={() => void (isOnline ? onGoOffline() : onGoOnline())}
            >
              <span className="go-online-button__label">{goOnlineButtonLabel}</span>
              <span className="go-online-button__detail">{goOnlineButtonDetail}</span>
            </button>
          </div>

          <div className="mission-control-register">
            <RegisterGrid rows={missionStats} />
          </div>

          <div className="homework-actions" aria-label="Secondary homework controls">
            <ActionButton action={actions.refresh} onAction={onAction} />
            <ActionButton action={actions.start} onAction={onAction} />
            <ActionButton action={actions.online} onAction={onAction} />
            <ActionButton action={actions.offline} onAction={onAction} />
            <ActionButton action={actions.pause} onAction={onAction} />
            <ActionButton action={actions.stop} onAction={onAction} />
            <ActionButton action={actions.runProof} onAction={onAction} variant="default" />
            <ActionButton action={actions.doctor} onAction={onAction} />
            <ActionButton action={actions.artifacts} onAction={onAction} />
          </div>
          <div className="homework-stage-grid">
            {snapshot.stages.map((stage) => (
              <article className="homework-stage" data-state={stateTone(stage.state)} key={stage.id}>
                <div className="homework-stage__head">
                  <span>{stage.label}</span>
                  <StateBadge value={stage.state} />
                </div>
                <p>{stage.detail}</p>
              </article>
            ))}
          </div>
        </CardContent>
      </Card>

      <div className="homework-grid">
        <Card className="workbench-panel">
          <CardHeader className="workbench-panel__header">
            <CardTitle>Current Pylon</CardTitle>
          </CardHeader>
          <CardContent className="workbench-panel__content">
            <RegisterGrid rows={pylonRows} />
          </CardContent>
        </Card>

        <Card className="workbench-panel">
          <CardHeader className="workbench-panel__header">
            <CardTitle>Homework Training</CardTitle>
          </CardHeader>
          <CardContent className="workbench-panel__content">
            <RegisterGrid rows={trainingRows} />
            {snapshot.trainingError ? (
              <div className="workbench-note">{snapshot.trainingError}</div>
            ) : null}
            {training?.runtimeSurfaceError ? (
              <div className="workbench-note">{training.runtimeSurfaceError}</div>
            ) : null}
          </CardContent>
        </Card>

        <Card className="workbench-panel">
          <CardHeader className="workbench-panel__header">
            <CardTitle>Assignment</CardTitle>
          </CardHeader>
          <CardContent className="workbench-panel__content">
            <RegisterGrid rows={assignmentRows} />
          </CardContent>
        </Card>

        <Card className="workbench-panel">
          <CardHeader className="workbench-panel__header">
            <CardTitle>Closeout + Payout</CardTitle>
          </CardHeader>
          <CardContent className="workbench-panel__content">
            <RegisterGrid rows={payoutRows} />
          </CardContent>
        </Card>

        <HomeworkListPanel
          empty="No current homework blockers."
          items={(training?.recentIssues ?? []).map((issue) => ({
            id: `${issue.kind}-${issue.subjectId}`,
            state: issue.blockingClass ?? (issue.retryable ? "retryable" : "issue"),
            title: `${issue.kind} / ${issue.subjectId}`,
            detail: issue.reason,
            meta: issue.owner ?? "homework",
          }))}
          title="Issues"
        />

        <HomeworkListPanel
          empty="No retained work offers."
          items={(training?.recentWorkOffers ?? []).map((offer) => ({
            id: `${offer.kind}-${offer.assignmentId ?? offer.leaseId ?? offer.state}`,
            state: offer.state,
            title: offer.assignmentId ?? "assignment pending",
            detail: `${offer.role ?? "worker"} ${offer.runtimeOperation ?? "homework"}`,
            meta: offer.networkId ?? offer.trainingRunId ?? "network pending",
          }))}
          title="Work Offers"
        />
      </div>
    </section>
  );
}

function HomeworkListPanel({
  empty,
  items,
  title,
}: {
  empty: string;
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
      </CardHeader>
      <CardContent className="workbench-panel__content">
        {items.length === 0 ? (
          <div className="workbench-note">{empty}</div>
        ) : (
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
        )}
      </CardContent>
    </Card>
  );
}

function NexusHealthPanel({
  actions,
  busy,
  onAction,
  snapshot,
}: {
  actions: {
    refresh: AutopilotAction;
  };
  busy: string | null;
  onAction: ActionRunner;
  snapshot: AutopilotNexusHealthProjection | null;
}) {
  if (!snapshot) {
    return (
      <Card className="operator-card">
        <CardHeader className="operator-card__header">
          <CardTitle>Nexus Health</CardTitle>
          <ActionButton action={actions.refresh} onAction={onAction} />
        </CardHeader>
        <CardContent className="operator-card__content">
          <div className="workbench-empty">Loading Nexus health projection.</div>
        </CardContent>
      </Card>
    );
  }

  const headlineRows: RegisterRow[] = [
    ["state", snapshot.state],
    ["severity", snapshot.severity],
    ["source", snapshot.source],
    ["base", snapshot.baseUrl],
    ["updated", formatTimestamp(String(snapshot.generatedAtUnixMs))],
    ["exact cause", snapshot.exactCause],
  ];
  const activeRunRows: RegisterRow[] = [
    ["run", snapshot.activeRun.runId ?? "none"],
    ["window", snapshot.activeRun.windowId ?? "none"],
    ["status", snapshot.activeRun.status],
    ["detail", snapshot.activeRun.detail],
  ];
  const stopRows: RegisterRow[] = [
    ["state", snapshot.stopState.state],
    ["can cancel", String(snapshot.stopState.canCancel)],
    ["reason", snapshot.stopState.reason],
  ];
  const gateRows: RegisterRow[] = snapshot.verificationGates.map((gate) => [
    gate.gateId,
    `${gate.status}${gate.passed ? "" : " failed"}`,
  ]);

  return (
    <section className="health-screen" aria-label="Autopilot Nexus health">
      <Card className="health-hero">
        <CardContent className="health-hero__content">
          <div className="health-hero__copy">
            <span className="mission-control-kicker">Autopilot Health</span>
            <h1>Nexus {snapshot.state}</h1>
            <p>{snapshot.summary}</p>
            <div className="status-strip">
              <StateBadge value={snapshot.state} />
              <StateBadge value={snapshot.severity} />
              <StateBadge value={`${snapshot.failedPredicates.length} failed predicates`} />
            </div>
          </div>
          <div className="health-hero__actions">
            <ActionButton
              action={actions.refresh}
              disabled={busy === "health.nexus"}
              onAction={onAction}
              variant="default"
            />
          </div>
        </CardContent>
      </Card>

      <div className="health-layout">
        <div className="health-main">
          <Card className="workbench-panel">
            <CardHeader className="workbench-panel__header">
              <CardTitle>Current State</CardTitle>
              <Pulse aria-hidden="true" />
            </CardHeader>
            <CardContent className="workbench-panel__content">
              <RegisterGrid rows={headlineRows} />
            </CardContent>
          </Card>

          <div className="health-subsystem-grid">
            {snapshot.subsystems.map((subsystem) => (
              <article
                className="health-subsystem"
                data-state={stateTone(subsystem.state)}
                key={subsystem.id}
              >
                <div className="health-subsystem__head">
                  <strong>{subsystem.label}</strong>
                  <StateBadge value={subsystem.state} />
                </div>
                <p>{subsystem.summary}</p>
                <span>{subsystem.detail}</span>
                <RegisterGrid
                  rows={subsystem.metrics.map((metric) => [
                    metric.label,
                    metric.value,
                  ])}
                />
              </article>
            ))}
          </div>
        </div>

        <aside className="health-side">
          <Card className="workbench-panel">
            <CardHeader className="workbench-panel__header">
              <CardTitle>Active Training Run</CardTitle>
            </CardHeader>
            <CardContent className="workbench-panel__content">
              <RegisterGrid rows={activeRunRows} />
            </CardContent>
          </Card>

          <Card className="workbench-panel">
            <CardHeader className="workbench-panel__header">
              <CardTitle>Queued Follow-Ups</CardTitle>
            </CardHeader>
            <CardContent className="workbench-panel__content">
              {snapshot.queuedFollowups.length === 0 ? (
                <div className="workbench-note">No follow-up actions are queued.</div>
              ) : (
                <div className="workbench-list">
                  {snapshot.queuedFollowups.map((item) => (
                    <article className="workbench-list-item" key={item.id}>
                      <div className="workbench-list-item__head">
                        <strong>{item.id}</strong>
                        <StateBadge value={item.severity} />
                      </div>
                      <p>{item.detail}</p>
                      <span>{item.owner}: {item.action}</span>
                    </article>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <Card className="workbench-panel">
            <CardHeader className="workbench-panel__header">
              <CardTitle>Stop / Cancel State</CardTitle>
            </CardHeader>
            <CardContent className="workbench-panel__content">
              <RegisterGrid rows={stopRows} />
            </CardContent>
          </Card>

          <Card className="workbench-panel">
            <CardHeader className="workbench-panel__header">
              <CardTitle>Verification Gates</CardTitle>
            </CardHeader>
            <CardContent className="workbench-panel__content">
              <RegisterGrid rows={gateRows.length > 0 ? gateRows : [["gates", "none"]]} />
            </CardContent>
          </Card>
        </aside>
      </div>

      <Card className="workbench-panel health-events-panel">
        <CardHeader className="workbench-panel__header">
          <CardTitle>Health Event Timeline</CardTitle>
        </CardHeader>
        <CardContent className="workbench-panel__content">
          <div className="timeline" aria-label="Nexus health event timeline">
            {snapshot.eventTimeline.map((event) => (
              <article
                className="timeline-event"
                data-state={stateTone(event.state)}
                key={event.id}
              >
                <div className="timeline-event__time">
                  {formatTimestamp(String(event.atUnixMs))}
                </div>
                <div className="timeline-event__body">
                  <div className="timeline-event__heading">
                    <strong>{event.title}</strong>
                    <StateBadge value={event.state} />
                  </div>
                  <p>{event.detail}</p>
                  <div className="timeline-event__meta">
                    <span>{event.id}</span>
                    <span>{event.evidence}</span>
                  </div>
                </div>
              </article>
            ))}
          </div>
        </CardContent>
      </Card>
    </section>
  );
}

function runtimeAsAssignment(
  runtime: HomeworkRuntimeProjection,
): HomeworkAssignmentProjection {
  return {
    kind: "active",
    state: runtime.processState,
    trainingRunId: runtime.trainingRunId,
    windowId: runtime.windowId,
    assignmentId: runtime.assignmentId,
    leaseId: runtime.leaseId,
    membershipRevision: null,
    role: runtime.role,
    networkId: null,
    runtimeLaneId: null,
    runtimeOperation: "train",
    runtimeWorkClass: "homework",
    runtimeManifestPath: runtime.manifestPath,
    updatedAtMs: runtime.updatedAtMs,
  };
}

function homeworkAssignmentRows(
  assignment: HomeworkAssignmentProjection | null,
): RegisterRow[] {
  if (!assignment) {
    return [["assignment", "waiting"]];
  }

  return [
    ["kind", assignment.kind],
    ["state", assignment.state],
    ["role", assignment.role ?? "unknown"],
    ["run", assignment.trainingRunId ?? "unknown"],
    ["window", assignment.windowId ?? "unknown"],
    ["assignment", assignment.assignmentId ?? "unknown"],
    ["lease", assignment.leaseId ?? "unknown"],
    ["network", assignment.networkId ?? "unknown"],
    ["runtime", assignment.runtimeLaneId ?? assignment.runtimeOperation ?? "unknown"],
    ["manifest", assignment.runtimeManifestPath ?? "not materialized"],
  ];
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
  disabled,
  onAction,
  variant,
}: {
  action: AutopilotAction;
  disabled?: boolean;
  onAction: ActionRunner;
  variant?: "default" | "outline";
}) {
  const resolvedVariant =
    variant ?? (action.kind === "destructive" ? "destructive" : "outline");

  return (
    <Button
      disabled={disabled ?? Boolean(action.disabledReason)}
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
      : activeView === "homework"
        ? "Earn"
      : activeView === "proof" || activeView === "health"
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
    normalized.includes("accepted") ||
    normalized.includes("paid")
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
    normalized.includes("unconfigured") ||
    normalized.includes("attention") ||
    normalized.includes("blocked")
  ) {
    return "warn";
  }

  if (
    normalized.includes("running") ||
    normalized.includes("starting") ||
    normalized.includes("active") ||
    normalized.includes("training") ||
    normalized.includes("closing")
  ) {
    return "active";
  }

  return "neutral";
}

export default App;
