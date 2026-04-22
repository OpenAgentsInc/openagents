import type { ProviderMode, ProofLane } from "@/lib/autopilot-runtime";

export type ActiveView = "workbench" | "command" | "pylon" | "proof";
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
  refreshWorkbench: () => Promise<void> | void;
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
  "Earn",
  "Diagnostics",
  "Help",
] as const;

export const viewLabels: Record<ActiveView, string> = {
  workbench: "Workbench",
  command: "Command Console",
  pylon: "Earn Runtime",
  proof: "Diagnostics",
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
  refreshWorkbench,
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
  const pylonReason = busyReason ?? (!pylonInstalled ? "Earn runtime binary not found" : undefined);
  const proofReason = busyReason;

  return [
    {
      id: "autopilot.workbench.refresh",
      label: "Refresh Workbench",
      menuPath: ["Autopilot", "Refresh Workbench"],
      paletteKeywords: ["workbench", "refresh", "snapshot", "session"],
      aliases: ["refresh workbench", "workbench refresh"],
      shortcut: "refresh",
      scope: "global",
      kind: "safe",
      authority: "local-tauri",
      effect: "Refreshes the Rust-owned Autopilot workbench snapshot.",
      evidence: ["workbench generated timestamp", "session timeline"],
      disabledReason: busyReason,
      run: refreshWorkbench,
    },
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
      id: "view.workbench",
      label: "Workbench",
      menuPath: ["View", "Workbench"],
      paletteKeywords: ["workbench", "session", "timeline", "approvals", "diffs"],
      aliases: ["workbench", "home"],
      shortcut: "home",
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the clean-room Autopilot workbench.",
      evidence: ["active view badge", "workbench object model"],
      active: activeView === "workbench",
      run: () => setActiveView("workbench"),
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
      label: "Earn Runtime",
      menuPath: ["View", "Earn Runtime"],
      paletteKeywords: ["earn", "provider", "process", "serve"],
      aliases: ["earn", "provider runtime"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the earn-runtime process and provider controls.",
      evidence: ["active view badge", "earn runtime register grid"],
      active: activeView === "pylon",
      run: () => setActiveView("pylon"),
    },
    {
      id: "view.proof",
      label: "Diagnostics",
      menuPath: ["View", "Diagnostics"],
      paletteKeywords: ["diagnostics", "proof", "fleet"],
      aliases: ["diagnostics", "proof"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows local proof and diagnostic lane controls.",
      evidence: ["active view badge", "diagnostics stage grid"],
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
      effect: "Opens the earn-runtime log folder.",
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
      menuPath: ["Earn", "Show Runtime Status"],
      paletteKeywords: ["earn", "status", "provider", "process"],
      aliases: ["earn status", "provider status"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the earn-runtime status panel.",
      evidence: ["earn runtime register grid"],
      active: activeView === "pylon",
      run: () => setActiveView("pylon"),
    },
    {
      id: "pylon.refresh",
      label: "Refresh",
      menuPath: ["Earn", "Refresh"],
      paletteKeywords: ["earn", "refresh", "reload", "detect"],
      aliases: ["refresh earn runtime"],
      shortcut: "status",
      scope: "global",
      kind: "safe",
      authority: "pylon",
      effect: "Refreshes earn-runtime binary and status through Tauri IPC.",
      evidence: ["binary source", "process state", "provider state", "updated"],
      disabledReason: busyReason,
      run: refreshPylon,
    },
    {
      id: "pylon.serve.start",
      label: "Start Earn Runtime",
      menuPath: ["Earn", "Start Runtime"],
      paletteKeywords: ["earn", "serve", "start", "spawn"],
      aliases: ["start earn runtime", "provider start"],
      shortcut: "spawn",
      scope: "global",
      kind: "mutating",
      authority: "pylon",
      effect: "Starts the detected earn-runtime process.",
      evidence: ["process state", "pid", "last action", "last error", "updated"],
      disabledReason: pylonReason,
      separatorBefore: true,
      run: startPylon,
    },
    {
      id: "pylon.serve.stop",
      label: "Stop Earn Runtime",
      menuPath: ["Earn", "Stop Runtime"],
      paletteKeywords: ["earn", "serve", "stop", "process"],
      aliases: ["stop earn runtime", "provider stop"],
      shortcut: "stop",
      scope: "global",
      kind: "mutating",
      authority: "pylon",
      effect: "Stops the active earn-runtime process.",
      evidence: ["process state", "pid", "last action", "last error", "updated"],
      disabledReason: pylonReason,
      run: stopPylon,
    },
    {
      id: "pylon.serve.restart",
      label: "Restart Earn Runtime",
      menuPath: ["Earn", "Restart Runtime"],
      paletteKeywords: ["earn", "serve", "restart", "process"],
      aliases: ["restart earn runtime", "provider restart"],
      shortcut: "restart",
      scope: "global",
      kind: "mutating",
      authority: "pylon",
      effect: "Stops and restarts the detected earn-runtime process.",
      evidence: ["process state", "pid", "last action", "last error", "updated"],
      disabledReason: pylonReason,
      run: restartPylon,
    },
    ...providerModes.map<AutopilotAction>((mode, index) => ({
      id: `pylon.provider.${mode}`,
      label: titleCase(mode),
      menuPath: ["Earn", "Provider Mode", titleCase(mode)],
      paletteKeywords: ["earn", "provider", "mode", mode],
      aliases: [`provider ${mode}`, `earn ${mode}`],
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
      label: "Open Runtime Logs",
      menuPath: ["Earn", "Open Runtime Logs"],
      paletteKeywords: ["earn", "logs", "folder"],
      aliases: ["open earn logs", "logs"],
      shortcut: "logs",
      scope: "global",
      kind: "safe",
      authority: "pylon",
      effect: "Opens the local earn-runtime logs folder.",
      evidence: ["operating system file browser"],
      disabledReason: pylonReason,
      separatorBefore: true,
      run: openPylonLogs,
    },
    {
      id: "proof.flow.show",
      label: "Show Flow",
      menuPath: ["Diagnostics", "Show Flow"],
      paletteKeywords: ["diagnostics", "proof", "flow", "fleet"],
      aliases: ["diagnostics flow", "proof flow"],
      scope: "global",
      kind: "view",
      authority: "local-tauri",
      effect: "Shows the local diagnostics flow panel.",
      evidence: ["diagnostics stage grid"],
      active: activeView === "proof",
      run: showProofFlow,
    },
    {
      id: "proof.namespace.refresh",
      label: "Refresh",
      menuPath: ["Diagnostics", "Refresh"],
      paletteKeywords: ["diagnostics", "proof", "refresh", "status", "namespace"],
      aliases: ["refresh diagnostics", "refresh proof", "proof refresh"],
      shortcut: "status",
      scope: "global",
      kind: "safe",
      authority: "proof",
      effect: "Refreshes the active diagnostics namespace projection.",
      evidence: ["proof status", "transport state", "updated"],
      disabledReason: proofReason,
      run: refreshProof,
    },
    {
      id: "proof.run.cs336-a1",
      label: "CS336 A1",
      menuPath: ["Diagnostics", "Run Lane", "CS336 A1"],
      paletteKeywords: ["diagnostics", "proof", "run", "cs336", "a1"],
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
      menuPath: ["Diagnostics", "Run Lane", "Stale Recovery"],
      paletteKeywords: ["diagnostics", "proof", "run", "cs336", "stale", "recovery"],
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
      menuPath: ["Diagnostics", "Run Lane", "Replacement Attempt"],
      paletteKeywords: ["diagnostics", "proof", "run", "cs336", "replacement", "attempt"],
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
      menuPath: ["Diagnostics", "Doctor Namespace"],
      paletteKeywords: ["diagnostics", "proof", "doctor", "diagnose", "transport"],
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
      menuPath: ["Diagnostics", "Stop Namespace"],
      paletteKeywords: ["diagnostics", "proof", "stop", "fleet", "down"],
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
      menuPath: ["Diagnostics", "Reset Namespace"],
      paletteKeywords: ["diagnostics", "proof", "reset", "clean", "namespace"],
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
      menuPath: ["Diagnostics", "Open Artifacts"],
      paletteKeywords: ["diagnostics", "proof", "artifacts", "files", "run-report", "summary", "trace"],
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
