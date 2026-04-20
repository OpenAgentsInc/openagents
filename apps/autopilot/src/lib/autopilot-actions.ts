import type { ProviderMode, ProofLane } from "@/lib/autopilot-runtime";

export type ActiveView = "command" | "pylon" | "proof";
export type Theme = "light" | "dark";
export type ActionKind = "view" | "safe" | "mutating" | "destructive";
export type ActionScope = "global" | "active-view" | "selection";
export type ActionAuthority =
  | "local-tauri"
  | "pylon"
  | "proof"
  | "theme"
  | "network";

export type AutopilotAction = {
  id: string;
  label: string;
  menuPath: string[];
  paletteKeywords: string[];
  aliases?: string[];
  shortcut?: string;
  scope: ActionScope;
  kind: ActionKind;
  authority: ActionAuthority;
  effect: string;
  evidence: string[];
  active?: boolean;
  disabledReason?: string;
  separatorBefore?: boolean;
  run: () => Promise<void> | void;
};

export type BuildAutopilotActionsOptions = {
  activeView: ActiveView;
  busy: string | null;
  pylonInstalled: boolean;
  theme: Theme;
  setActiveView: (view: ActiveView) => void;
  setTheme: (theme: Theme) => void;
  showControlStatus: () => Promise<void> | void;
  refreshPylon: () => Promise<void> | void;
  openPylonLogs: () => Promise<void> | void;
  startPylon: () => Promise<void> | void;
  stopPylon: () => Promise<void> | void;
  restartPylon: () => Promise<void> | void;
  setProviderMode: (mode: ProviderMode) => Promise<void> | void;
  showProofFlow: () => void;
  refreshProof: () => Promise<void> | void;
  runProofLane: (lane: ProofLane) => Promise<void> | void;
  doctorProof: () => Promise<void> | void;
  stopProof: () => Promise<void> | void;
  resetProof: () => Promise<void> | void;
  openProofArtifacts: () => Promise<void> | void;
};

export const topLevelMenus = [
  "Autopilot",
  "View",
  "Pylon",
  "Proof",
  "Help",
] as const;

export const viewLabels: Record<ActiveView, string> = {
  command: "Command Console",
  pylon: "Pylon",
  proof: "Proof Flow",
};

export const proofLaneLabels: Record<ProofLane, string> = {
  "cs336-a1": "CS336 A1",
  "cs336-a1-stale-recovery": "CS336 Stale Recovery",
  "cs336-a1-replacement-attempt": "CS336 Replacement Attempt",
};

const providerModes: ProviderMode[] = ["online", "offline", "pause", "resume"];

export function buildAutopilotActions({
  activeView,
  busy,
  pylonInstalled,
  theme,
  setActiveView,
  setTheme,
  showControlStatus,
  refreshPylon,
  openPylonLogs,
  startPylon,
  stopPylon,
  restartPylon,
  setProviderMode,
  showProofFlow,
  refreshProof,
  runProofLane,
  doctorProof,
  stopProof,
  resetProof,
  openProofArtifacts,
}: BuildAutopilotActionsOptions): AutopilotAction[] {
  const busyReason = busy ? `Runtime command in flight: ${busy}` : undefined;
  const pylonReason = busyReason ?? (!pylonInstalled ? "Pylon binary not found" : undefined);
  const proofReason = busyReason;

  return [
    {
      id: "autopilot.runtime.status",
      label: "Check Runtime Status",
      menuPath: ["Autopilot", "Check Runtime Status"],
      paletteKeywords: ["status", "runtime", "tauri", "control"],
      aliases: ["runtime status", "status"],
      shortcut: "status",
      scope: "global",
      kind: "safe",
      authority: "local-tauri",
      effect: "Reads the local Tauri shell authority status.",
      evidence: ["command console message"],
      run: showControlStatus,
    },
    {
      id: "autopilot.settings",
      label: "Settings",
      menuPath: ["Autopilot", "Settings"],
      paletteKeywords: ["settings", "preferences", "config"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Opens the Autopilot settings panel.",
      evidence: ["settings pane"],
      disabledReason: "Settings panel is not in this prototype yet.",
      separatorBefore: true,
      run: () => setActiveView("command"),
    },
    {
      id: "autopilot.quit",
      label: "Quit Autopilot",
      menuPath: ["Autopilot", "Quit Autopilot"],
      paletteKeywords: ["quit", "exit", "close"],
      shortcut: "cmd q",
      scope: "global",
      kind: "destructive",
      authority: "local-tauri",
      effect: "Exits the desktop application.",
      evidence: ["operating system process list"],
      disabledReason: "Use the operating system app menu to quit.",
      separatorBefore: true,
      run: () => undefined,
    },
    {
      id: "view.command",
      label: "Command Console",
      menuPath: ["View", "Command Console"],
      paletteKeywords: ["console", "command", "home"],
      aliases: ["command", "console"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the sparse command console landing view.",
      evidence: ["active view badge"],
      active: activeView === "command",
      run: () => setActiveView("command"),
    },
    {
      id: "view.pylon",
      label: "Pylon",
      menuPath: ["View", "Pylon"],
      paletteKeywords: ["provider", "process", "serve"],
      aliases: ["pylon"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the Pylon process and provider controls.",
      evidence: ["active view badge", "Pylon register grid"],
      active: activeView === "pylon",
      run: () => setActiveView("pylon"),
    },
    {
      id: "view.proof",
      label: "Proof Flow",
      menuPath: ["View", "Proof Flow"],
      paletteKeywords: ["proof", "nexus", "fleet"],
      aliases: ["proof", "nexus"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the proof flow and local lane controls.",
      evidence: ["active view badge", "proof stage grid"],
      active: activeView === "proof",
      run: showProofFlow,
    },
    {
      id: "view.activity",
      label: "Activity",
      menuPath: ["View", "Activity"],
      paletteKeywords: ["activity", "events", "tape"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Opens the activity event tape.",
      evidence: ["activity event tape"],
      disabledReason: "Activity tape is not wired in this Tauri shell yet.",
      separatorBefore: true,
      run: () => setActiveView("command"),
    },
    {
      id: "view.logs",
      label: "Logs",
      menuPath: ["View", "Logs"],
      paletteKeywords: ["logs", "pylon logs", "folder"],
      scope: "global",
      kind: "safe",
      authority: "pylon",
      effect: "Opens the Pylon log folder.",
      evidence: ["operating system file browser"],
      disabledReason: pylonReason,
      run: openPylonLogs,
    },
    {
      id: "view.artifacts",
      label: "Artifacts",
      menuPath: ["View", "Artifacts"],
      paletteKeywords: ["artifacts", "proof artifacts", "run report"],
      scope: "global",
      kind: "safe",
      authority: "proof",
      effect: "Opens the active proof namespace artifact folder.",
      evidence: ["run report", "authority trace", "proof summary"],
      disabledReason: proofReason,
      run: openProofArtifacts,
    },
    {
      id: "view.theme.dark",
      label: "Dark",
      menuPath: ["View", "Theme", "Dark"],
      paletteKeywords: ["dark", "theme", "appearance"],
      aliases: ["dark"],
      scope: "global",
      kind: "safe",
      authority: "theme",
      effect: "Switches the local shell to dark mode.",
      evidence: ["document theme class"],
      active: theme === "dark",
      separatorBefore: true,
      run: () => setTheme("dark"),
    },
    {
      id: "view.theme.light",
      label: "Light",
      menuPath: ["View", "Theme", "Light"],
      paletteKeywords: ["light", "theme", "appearance"],
      aliases: ["light"],
      scope: "global",
      kind: "safe",
      authority: "theme",
      effect: "Switches the local shell to light mode.",
      evidence: ["document theme class"],
      active: theme === "light",
      run: () => setTheme("light"),
    },
    {
      id: "pylon.status.show",
      label: "Show Status",
      menuPath: ["Pylon", "Show Status"],
      paletteKeywords: ["pylon", "status", "provider", "process"],
      aliases: ["pylon status"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the Pylon status panel.",
      evidence: ["Pylon register grid"],
      active: activeView === "pylon",
      run: () => setActiveView("pylon"),
    },
    {
      id: "pylon.refresh",
      label: "Refresh",
      menuPath: ["Pylon", "Refresh"],
      paletteKeywords: ["pylon", "refresh", "reload", "detect"],
      aliases: ["refresh pylon"],
      shortcut: "status",
      scope: "global",
      kind: "safe",
      authority: "pylon",
      effect: "Refreshes Pylon binary and runtime status through Tauri IPC.",
      evidence: ["binary source", "process state", "provider state", "updated"],
      disabledReason: busyReason,
      run: refreshPylon,
    },
    {
      id: "pylon.serve.start",
      label: "Start Serve",
      menuPath: ["Pylon", "Start Serve"],
      paletteKeywords: ["pylon", "serve", "start", "spawn"],
      aliases: ["start pylon", "pylon start"],
      shortcut: "spawn",
      scope: "global",
      kind: "mutating",
      authority: "pylon",
      effect: "Starts the detected Pylon serve process.",
      evidence: ["process state", "pid", "last action", "last error", "updated"],
      disabledReason: pylonReason,
      separatorBefore: true,
      run: startPylon,
    },
    {
      id: "pylon.serve.stop",
      label: "Stop Serve",
      menuPath: ["Pylon", "Stop Serve"],
      paletteKeywords: ["pylon", "serve", "stop", "process"],
      aliases: ["stop pylon", "pylon stop"],
      shortcut: "stop",
      scope: "global",
      kind: "mutating",
      authority: "pylon",
      effect: "Stops the active Pylon serve process.",
      evidence: ["process state", "pid", "last action", "last error", "updated"],
      disabledReason: pylonReason,
      run: stopPylon,
    },
    {
      id: "pylon.serve.restart",
      label: "Restart Serve",
      menuPath: ["Pylon", "Restart Serve"],
      paletteKeywords: ["pylon", "serve", "restart", "process"],
      aliases: ["restart pylon", "pylon restart"],
      shortcut: "restart",
      scope: "global",
      kind: "mutating",
      authority: "pylon",
      effect: "Stops and restarts the detected Pylon serve process.",
      evidence: ["process state", "pid", "last action", "last error", "updated"],
      disabledReason: pylonReason,
      run: restartPylon,
    },
    ...providerModes.map<AutopilotAction>((mode, index) => ({
      id: `pylon.provider.${mode}`,
      label: titleCase(mode),
      menuPath: ["Pylon", "Provider Mode", titleCase(mode)],
      paletteKeywords: ["pylon", "provider", "mode", mode],
      aliases: [`provider ${mode}`, `pylon ${mode}`],
      shortcut: mode,
      scope: "global",
      kind: "mutating",
      authority: "pylon",
      effect: `Requests provider ${mode} mode through local Tauri IPC.`,
      evidence: ["provider state", "desired mode", "last action", "last error", "updated"],
      disabledReason: pylonReason,
      separatorBefore: index === 0,
      run: () => setProviderMode(mode),
    })),
    {
      id: "pylon.logs.open",
      label: "Open Logs",
      menuPath: ["Pylon", "Open Logs"],
      paletteKeywords: ["pylon", "logs", "folder"],
      aliases: ["open pylon logs", "logs"],
      shortcut: "logs",
      scope: "global",
      kind: "safe",
      authority: "pylon",
      effect: "Opens the local Pylon logs folder.",
      evidence: ["operating system file browser"],
      disabledReason: pylonReason,
      separatorBefore: true,
      run: openPylonLogs,
    },
    {
      id: "proof.flow.show",
      label: "Show Flow",
      menuPath: ["Proof", "Show Flow"],
      paletteKeywords: ["proof", "flow", "nexus", "fleet"],
      aliases: ["proof flow"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the proof flow panel.",
      evidence: ["proof stage grid"],
      active: activeView === "proof",
      run: showProofFlow,
    },
    {
      id: "proof.namespace.refresh",
      label: "Refresh",
      menuPath: ["Proof", "Refresh"],
      paletteKeywords: ["proof", "refresh", "status", "namespace"],
      aliases: ["refresh proof", "proof refresh"],
      shortcut: "status",
      scope: "global",
      kind: "safe",
      authority: "proof",
      effect: "Refreshes the active proof namespace projection.",
      evidence: ["proof status", "transport state", "updated"],
      disabledReason: proofReason,
      run: refreshProof,
    },
    {
      id: "proof.run.cs336-a1",
      label: "CS336 A1",
      menuPath: ["Proof", "Run Lane", "CS336 A1"],
      paletteKeywords: ["proof", "run", "cs336", "a1"],
      aliases: ["run cs336 a1", "cs336 a1"],
      shortcut: "proof",
      scope: "global",
      kind: "mutating",
      authority: "proof",
      effect: "Starts the CS336 A1 local proof lane.",
      evidence: ["proof status", "run report", "authority trace", "summary"],
      disabledReason: proofReason,
      separatorBefore: true,
      run: () => runProofLane("cs336-a1"),
    },
    {
      id: "proof.run.cs336-a1-stale-recovery",
      label: "Stale Recovery",
      menuPath: ["Proof", "Run Lane", "Stale Recovery"],
      paletteKeywords: ["proof", "run", "cs336", "stale", "recovery"],
      aliases: ["run stale recovery", "stale recovery"],
      shortcut: "proof",
      scope: "global",
      kind: "mutating",
      authority: "proof",
      effect: "Starts the CS336 stale recovery local proof lane.",
      evidence: ["proof status", "run report", "authority trace", "summary"],
      disabledReason: proofReason,
      run: () => runProofLane("cs336-a1-stale-recovery"),
    },
    {
      id: "proof.run.cs336-a1-replacement-attempt",
      label: "Replacement Attempt",
      menuPath: ["Proof", "Run Lane", "Replacement Attempt"],
      paletteKeywords: ["proof", "run", "cs336", "replacement", "attempt"],
      aliases: ["run replacement attempt", "replacement attempt"],
      shortcut: "proof",
      scope: "global",
      kind: "mutating",
      authority: "proof",
      effect: "Starts the CS336 replacement-attempt local proof lane.",
      evidence: ["proof status", "run report", "authority trace", "summary"],
      disabledReason: proofReason,
      run: () => runProofLane("cs336-a1-replacement-attempt"),
    },
    {
      id: "proof.namespace.doctor",
      label: "Doctor Namespace",
      menuPath: ["Proof", "Doctor Namespace"],
      paletteKeywords: ["proof", "doctor", "diagnose", "transport"],
      aliases: ["doctor proof", "proof doctor"],
      shortcut: "doctor",
      scope: "global",
      kind: "safe",
      authority: "proof",
      effect: "Diagnoses the active proof namespace.",
      evidence: ["transport state", "first red stage", "detail", "updated"],
      disabledReason: proofReason,
      separatorBefore: true,
      run: doctorProof,
    },
    {
      id: "proof.namespace.stop",
      label: "Stop Namespace",
      menuPath: ["Proof", "Stop Namespace"],
      paletteKeywords: ["proof", "stop", "fleet", "down"],
      aliases: ["stop proof", "proof stop"],
      shortcut: "down",
      scope: "global",
      kind: "mutating",
      authority: "proof",
      effect: "Stops workers attached to the active proof namespace.",
      evidence: ["worker table", "validator table", "proof status", "updated"],
      disabledReason: proofReason,
      run: stopProof,
    },
    {
      id: "proof.namespace.reset",
      label: "Reset Namespace",
      menuPath: ["Proof", "Reset Namespace"],
      paletteKeywords: ["proof", "reset", "clean", "namespace"],
      aliases: ["reset proof", "proof reset"],
      shortcut: "reset",
      scope: "global",
      kind: "destructive",
      authority: "proof",
      effect: "Clears retained local proof namespace state.",
      evidence: ["proof status", "artifact root", "updated"],
      disabledReason: proofReason,
      run: resetProof,
    },
    {
      id: "proof.artifacts.open",
      label: "Open Artifacts",
      menuPath: ["Proof", "Open Artifacts"],
      paletteKeywords: ["proof", "artifacts", "files", "run-report", "summary", "trace"],
      aliases: ["open proof artifacts", "proof artifacts", "artifacts"],
      shortcut: "files",
      scope: "global",
      kind: "safe",
      authority: "proof",
      effect: "Opens active proof namespace artifacts.",
      evidence: ["run report", "authority trace", "summary", "artifact trace"],
      disabledReason: proofReason,
      separatorBefore: true,
      run: openProofArtifacts,
    },
    {
      id: "help.diagnostics",
      label: "Diagnostics",
      menuPath: ["Help", "Diagnostics"],
      paletteKeywords: ["help", "diagnostics", "doctor", "status"],
      aliases: ["diagnostics"],
      scope: "global",
      kind: "safe",
      authority: "proof",
      effect: "Runs proof namespace diagnostics.",
      evidence: ["transport state", "detail", "updated"],
      disabledReason: proofReason,
      run: doctorProof,
    },
    {
      id: "help.control.status",
      label: "Control Plane Status",
      menuPath: ["Help", "Control Plane Status"],
      paletteKeywords: ["help", "control", "tauri", "status"],
      aliases: ["control status"],
      scope: "global",
      kind: "safe",
      authority: "local-tauri",
      effect: "Reads the local shell control-plane status.",
      evidence: ["command console message"],
      run: showControlStatus,
    },
  ];
}

export function actionBreadcrumb(action: Pick<AutopilotAction, "menuPath">) {
  return action.menuPath.join(" > ");
}

export function normalizeActionQuery(value: string) {
  return value.trim().toLowerCase().replace(/\s*>\s*/g, " > ").replace(/\s+/g, " ");
}

export function resolveRegisteredAction(
  actions: AutopilotAction[],
  value: string,
) {
  const query = normalizeActionQuery(value);

  if (!query) {
    return null;
  }

  return (
    actions.find((action) => {
      const candidates = [
        action.id,
        actionBreadcrumb(action),
        ...(action.aliases ?? []),
      ];

      return candidates.some((candidate) => normalizeActionQuery(candidate) === query);
    }) ?? null
  );
}

export function validateAutopilotActions(actions: AutopilotAction[]) {
  const errors: string[] = [];
  const ids = new Set<string>();

  for (const action of actions) {
    if (ids.has(action.id)) {
      errors.push(`duplicate action id: ${action.id}`);
    }

    ids.add(action.id);

    if (action.menuPath.length < 2) {
      errors.push(`action ${action.id} must have a top-level and leaf menu path`);
    }

    if (!topLevelMenus.includes(action.menuPath[0] as (typeof topLevelMenus)[number])) {
      errors.push(`action ${action.id} has unknown top-level menu: ${action.menuPath[0]}`);
    }

    if ((action.kind === "mutating" || action.kind === "destructive") && action.evidence.length === 0) {
      errors.push(`action ${action.id} must declare evidence`);
    }

    if ((action.kind === "mutating" || action.kind === "destructive") && !action.effect) {
      errors.push(`action ${action.id} must declare effect`);
    }
  }

  return errors;
}

function titleCase(value: string) {
  return value.slice(0, 1).toUpperCase() + value.slice(1);
}
