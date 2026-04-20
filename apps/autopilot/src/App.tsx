import "./App.css";
import * as React from "react";
import { listen } from "@tauri-apps/api/event";
import {
  CheckCircle,
  Command as CommandIcon,
  Moon,
  Pulse,
  Sun,
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
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Separator } from "@/components/ui/separator";
import { Textarea } from "@/components/ui/textarea";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
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

type ActiveView = "command" | "pylon" | "proof";
type Theme = "light" | "dark";

const viewLabels: Record<ActiveView, string> = {
  command: "Command",
  pylon: "Pylon",
  proof: "Proof",
};

const proofLaneLabels: Record<ProofLane, string> = {
  "cs336-a1": "CS336 A1",
  "cs336-a1-stale-recovery": "CS336 Stale Recovery",
  "cs336-a1-replacement-attempt": "CS336 Replacement Attempt",
};

function App() {
  const [activeView, setActiveView] = React.useState<ActiveView>("command");
  const [commandOpen, setCommandOpen] = React.useState(false);
  const [commandText, setCommandText] = React.useState("");
  const [theme, setTheme] = useTheme();
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

  const toggleTheme = () => {
    setTheme((currentTheme) => (currentTheme === "dark" ? "light" : "dark"));
    setCommandOpen(false);
  };

  const submitCommand = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const command = commandText.trim().toLowerCase();

    if (command.includes("pylon")) {
      setActiveView("pylon");
    } else if (command.includes("proof") || command.includes("nexus")) {
      setActiveView("proof");
    }

    setCommandText("");
  };

  const selectCommand = (action: () => void) => {
    action();
    setCommandOpen(false);
  };

  return (
    <main className="shell grid place-items-center p-4">
      <section className="operator-stage">
        <div className="operator-stage__bar">
          <div className="status-strip">
            <Badge variant="outline">ACTIVE: {viewLabels[activeView]}</Badge>
            <StateBadge value={pylonStatus?.processState ?? "unknown"} />
            <StateBadge value={pylonStatus?.providerState ?? "unknown"} />
            <StateBadge value={proofStatus?.status ?? "no proof"} />
          </div>

          <div className="operator-stage__actions">
            <ToggleGroup
              aria-label="Theme"
              value={[theme]}
              variant="outline"
              size="sm"
              onValueChange={(value) => {
                const nextTheme = value[0];

                if (nextTheme === "light" || nextTheme === "dark") {
                  setTheme(nextTheme);
                }
              }}
            >
              <ToggleGroupItem value="dark" aria-label="Use dark theme">
                <Moon aria-hidden="true" data-icon="inline-start" />
                Dark
              </ToggleGroupItem>
              <ToggleGroupItem value="light" aria-label="Use light theme">
                <Sun aria-hidden="true" data-icon="inline-start" />
                Light
              </ToggleGroupItem>
            </ToggleGroup>

            <Button
              type="button"
              variant="outline"
              onClick={() => setCommandOpen(true)}
            >
              <CommandIcon aria-hidden="true" data-icon="inline-start" />
              Command
              <KbdGroup className="ml-1">
                <Kbd>⌘</Kbd>
                <Kbd>K</Kbd>
              </KbdGroup>
            </Button>
          </div>
        </div>

        <Separator />

        {actionError ? (
          <div className="operator-error" role="status">
            {actionError}
          </div>
        ) : null}

        <div className="operator-stage__content" aria-live="polite">
          {activeView === "command" ? (
            <CommandEntry
              value={commandText}
              onChange={setCommandText}
              onSubmit={submitCommand}
            />
          ) : activeView === "pylon" ? (
            <PylonStatusCard
              binary={pylonBinary}
              busy={busy}
              status={pylonStatus}
              onOpenLogs={() => {
                void pylonOpenLogs().catch((error) =>
                  setActionError(formatError(error)),
                );
              }}
              onRefresh={refreshPylon}
              onRestart={() =>
                void runPylonControl("pylon.restart", pylonRestart)
              }
              onSetMode={setProviderMode}
              onStart={() => void runPylonControl("pylon.start", pylonStart)}
              onStop={() => void runPylonControl("pylon.stop", pylonStop)}
            />
          ) : (
            <ProofRunCard
              busy={busy}
              namespace={activeNamespace}
              proof={proofStatus}
              onDoctor={() =>
                void runProofCommand("proof.doctor", () =>
                  proofDoctor(activeNamespace),
                )
              }
              onOpenArtifacts={() => {
                void proofOpenArtifacts(activeNamespace).catch((error) =>
                  setActionError(formatError(error)),
                );
              }}
              onRefresh={() =>
                void runProofCommand("proof.refresh", () =>
                  proofGet(activeNamespace),
                )
              }
              onReset={() =>
                void runProofCommand("proof.reset", () =>
                  proofReset(activeNamespace),
                )
              }
              onRunLane={runProofLane}
              onStop={() =>
                void runProofCommand("proof.stop", () =>
                  proofStop(activeNamespace),
                )
              }
            />
          )}
        </div>
      </section>

      <CommandDialog
        open={commandOpen}
        onOpenChange={setCommandOpen}
        title="Autopilot Command Menu"
        description="Run local Autopilot, Pylon, and proof commands."
        className="sm:max-w-2xl"
      >
        <Command label="Autopilot commands" loop>
          <CommandInput placeholder="Filter commands by pylon, proof, theme..." />
          <CommandList className="max-h-[26rem]">
            <CommandEmpty>No command matched.</CommandEmpty>
            <CommandGroup heading="Pylon">
              <CommandItem
                value="show pylon status"
                keywords={["status", "provider", "process"]}
                data-checked={activeView === "pylon"}
                onSelect={() => selectCommand(() => setActiveView("pylon"))}
              >
                <Pulse aria-hidden="true" data-icon="inline-start" />
                <span>Show Pylon Status</span>
                <CommandShortcut>view</CommandShortcut>
              </CommandItem>
              <CommandItem
                value="refresh pylon status"
                keywords={["reload", "detect", "binary"]}
                onSelect={() => selectCommand(() => void refreshPylon())}
              >
                <Pulse aria-hidden="true" data-icon="inline-start" />
                <span>Refresh Pylon</span>
                <CommandShortcut>status</CommandShortcut>
              </CommandItem>
              <CommandSeparator />
              <CommandItem
                value="start pylon serve"
                keywords={["serve", "spawn", "process"]}
                onSelect={() =>
                  selectCommand(
                    () => void runPylonControl("pylon.start", pylonStart),
                  )
                }
              >
                <Pulse aria-hidden="true" data-icon="inline-start" />
                <span>Start Pylon Serve</span>
                <CommandShortcut>spawn</CommandShortcut>
              </CommandItem>
              <CommandItem
                value="stop pylon serve"
                keywords={["kill", "process"]}
                onSelect={() =>
                  selectCommand(
                    () => void runPylonControl("pylon.stop", pylonStop),
                  )
                }
              >
                <Pulse aria-hidden="true" data-icon="inline-start" />
                <span>Stop Pylon Serve</span>
                <CommandShortcut>stop</CommandShortcut>
              </CommandItem>
              <CommandItem
                value="restart pylon serve"
                keywords={["process", "serve"]}
                onSelect={() =>
                  selectCommand(
                    () => void runPylonControl("pylon.restart", pylonRestart),
                  )
                }
              >
                <Pulse aria-hidden="true" data-icon="inline-start" />
                <span>Restart Pylon Serve</span>
                <CommandShortcut>restart</CommandShortcut>
              </CommandItem>
              <CommandSeparator />
              {(["online", "offline", "pause", "resume"] as ProviderMode[]).map(
                (mode) => (
                  <CommandItem
                    key={mode}
                    value={`set pylon ${mode}`}
                    keywords={["provider", "mode"]}
                    onSelect={() => selectCommand(() => setProviderMode(mode))}
                  >
                    <Pulse aria-hidden="true" data-icon="inline-start" />
                    <span>Set Provider {mode}</span>
                    <CommandShortcut>{mode}</CommandShortcut>
                  </CommandItem>
                ),
              )}
              <CommandItem
                value="open pylon logs"
                keywords={["logs", "folder"]}
                onSelect={() =>
                  selectCommand(
                    () =>
                      void pylonOpenLogs().catch((error) =>
                        setActionError(formatError(error)),
                      ),
                  )
                }
              >
                <CommandIcon aria-hidden="true" data-icon="inline-start" />
                <span>Open Pylon Logs</span>
                <CommandShortcut>logs</CommandShortcut>
              </CommandItem>
            </CommandGroup>
            <CommandSeparator />
            <CommandGroup heading="Proof">
              <CommandItem
                value="show proof flow"
                keywords={["nexus", "fleet", "simulation"]}
                data-checked={activeView === "proof"}
                onSelect={() => selectCommand(() => setActiveView("proof"))}
              >
                <CheckCircle aria-hidden="true" data-icon="inline-start" />
                <span>Show Proof Flow</span>
                <CommandShortcut>view</CommandShortcut>
              </CommandItem>
              {(Object.keys(proofLaneLabels) as ProofLane[]).map((lane) => (
                <CommandItem
                  key={lane}
                  value={`run proof ${lane}`}
                  keywords={["cs336", "nexus", "pylon", "fleet"]}
                  onSelect={() => selectCommand(() => void runProofLane(lane))}
                >
                  <CheckCircle aria-hidden="true" data-icon="inline-start" />
                  <span>Run {proofLaneLabels[lane]}</span>
                  <CommandShortcut>proof</CommandShortcut>
                </CommandItem>
              ))}
              <CommandSeparator />
              <CommandItem
                value="doctor proof namespace"
                keywords={["diagnose", "transport", "split"]}
                onSelect={() =>
                  selectCommand(
                    () =>
                      void runProofCommand("proof.doctor", () =>
                        proofDoctor(activeNamespace),
                      ),
                  )
                }
              >
                <CheckCircle aria-hidden="true" data-icon="inline-start" />
                <span>Doctor Proof Namespace</span>
                <CommandShortcut>doctor</CommandShortcut>
              </CommandItem>
              <CommandItem
                value="stop proof namespace"
                keywords={["fleet", "down"]}
                onSelect={() =>
                  selectCommand(
                    () =>
                      void runProofCommand("proof.stop", () =>
                        proofStop(activeNamespace),
                      ),
                  )
                }
              >
                <Pulse aria-hidden="true" data-icon="inline-start" />
                <span>Stop Proof Namespace</span>
                <CommandShortcut>down</CommandShortcut>
              </CommandItem>
              <CommandItem
                value="reset proof namespace"
                keywords={["fleet", "clean"]}
                onSelect={() =>
                  selectCommand(
                    () =>
                      void runProofCommand("proof.reset", () =>
                        proofReset(activeNamespace),
                      ),
                  )
                }
              >
                <Pulse aria-hidden="true" data-icon="inline-start" />
                <span>Reset Proof Namespace</span>
                <CommandShortcut>reset</CommandShortcut>
              </CommandItem>
              <CommandItem
                value="open proof artifacts"
                keywords={["files", "run-report", "summary", "trace"]}
                onSelect={() =>
                  selectCommand(
                    () =>
                      void proofOpenArtifacts(activeNamespace).catch((error) =>
                        setActionError(formatError(error)),
                      ),
                  )
                }
              >
                <CommandIcon aria-hidden="true" data-icon="inline-start" />
                <span>Open Proof Artifacts</span>
                <CommandShortcut>files</CommandShortcut>
              </CommandItem>
            </CommandGroup>
            <CommandSeparator />
            <CommandGroup heading="Theme">
              <CommandItem
                value="toggle theme"
                keywords={["light", "dark", "mode", "appearance"]}
                onSelect={toggleTheme}
              >
                {theme === "dark" ? (
                  <Sun aria-hidden="true" data-icon="inline-start" />
                ) : (
                  <Moon aria-hidden="true" data-icon="inline-start" />
                )}
                <span>
                  Switch to {theme === "dark" ? "Light" : "Dark"} Theme
                </span>
                <CommandShortcut>
                  {theme === "dark" ? "light" : "dark"}
                </CommandShortcut>
              </CommandItem>
            </CommandGroup>
          </CommandList>
        </Command>
      </CommandDialog>
    </main>
  );
}

function CommandEntry({
  value,
  onChange,
  onSubmit,
}: {
  value: string;
  onChange: (value: string) => void;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <form className="command-entry" onSubmit={onSubmit}>
      <FieldGroup>
        <Field>
          <FieldLabel className="sr-only" htmlFor="autopilot-command">
            Command
          </FieldLabel>
          <Textarea
            id="autopilot-command"
            value={value}
            onChange={(event) => onChange(event.target.value)}
            placeholder="Enter command"
          />
        </Field>
      </FieldGroup>
      <Button type="submit">Submit</Button>
    </form>
  );
}

function PylonStatusCard({
  binary,
  busy,
  status,
  onOpenLogs,
  onRefresh,
  onRestart,
  onSetMode,
  onStart,
  onStop,
}: {
  binary: PylonBinaryStatus | null;
  busy: string | null;
  status: PylonStatusProjection | null;
  onOpenLogs: () => void;
  onRefresh: () => void;
  onRestart: () => void;
  onSetMode: (mode: ProviderMode) => void;
  onStart: () => void;
  onStop: () => void;
}) {
  const disabled = busy !== null;
  const rows: RegisterRow[] = [
    ["binary", binary?.binaryPath ?? status?.binaryPath ?? "not found"],
    ["binary source", binary?.source ?? "unknown"],
    ["config", status?.configPath ?? "not loaded"],
    ["pylon home", status?.pylonHome ?? "not loaded"],
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
        <CardTitle>Pylon</CardTitle>
        <div className="button-row">
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onRefresh}>
            <Pulse aria-hidden="true" data-icon="inline-start" />
            Refresh
          </Button>
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onOpenLogs}>
            <CommandIcon aria-hidden="true" data-icon="inline-start" />
            Logs
          </Button>
        </div>
      </CardHeader>
      <CardContent className="operator-card__content">
        <div className="button-row button-row--wrap">
          <Button type="button" variant="default" size="sm" disabled={disabled} onClick={onStart}>
            <Pulse aria-hidden="true" data-icon="inline-start" />
            Start
          </Button>
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onStop}>
            <Pulse aria-hidden="true" data-icon="inline-start" />
            Stop
          </Button>
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onRestart}>
            <Pulse aria-hidden="true" data-icon="inline-start" />
            Restart
          </Button>
          {(["online", "offline", "pause", "resume"] as ProviderMode[]).map((mode) => (
            <Button
              key={mode}
              type="button"
              variant="outline"
              size="sm"
              disabled={disabled}
              onClick={() => onSetMode(mode)}
            >
              {mode}
            </Button>
          ))}
        </div>
        <RegisterGrid rows={rows} />
      </CardContent>
    </Card>
  );
}

function ProofRunCard({
  busy,
  namespace,
  proof,
  onDoctor,
  onOpenArtifacts,
  onRefresh,
  onReset,
  onRunLane,
  onStop,
}: {
  busy: string | null;
  namespace: string;
  proof: ProofRunProjection | null;
  onDoctor: () => void;
  onOpenArtifacts: () => void;
  onRefresh: () => void;
  onReset: () => void;
  onRunLane: (lane: ProofLane) => void;
  onStop: () => void;
}) {
  const disabled = busy !== null;
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
        <CardTitle>Proof Flow</CardTitle>
        <div className="button-row">
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onRefresh}>
            <Pulse aria-hidden="true" data-icon="inline-start" />
            Refresh
          </Button>
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onOpenArtifacts}>
            <CommandIcon aria-hidden="true" data-icon="inline-start" />
            Artifacts
          </Button>
        </div>
      </CardHeader>
      <CardContent className="operator-card__content">
        <div className="button-row button-row--wrap">
          {(Object.keys(proofLaneLabels) as ProofLane[]).map((lane) => (
            <Button
              key={lane}
              type="button"
              variant="default"
              size="sm"
              disabled={disabled}
              onClick={() => onRunLane(lane)}
            >
              <CheckCircle aria-hidden="true" data-icon="inline-start" />
              {proofLaneLabels[lane]}
            </Button>
          ))}
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onDoctor}>
            Doctor
          </Button>
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onStop}>
            Stop
          </Button>
          <Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onReset}>
            Reset
          </Button>
        </div>
        <ProofStageGrid proof={proof} />
        <RegisterGrid rows={rows} />
        <ProofNodeTable label="Workers" nodes={proof?.workers ?? []} />
        <ProofNodeTable label="Validators" nodes={proof?.validators ?? []} />
      </CardContent>
    </Card>
  );
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
    <div className="proof-stage-grid" aria-label="Proof stages">
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

type RegisterRow = [string, React.ReactNode];

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
