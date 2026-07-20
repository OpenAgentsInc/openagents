import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type KeyboardEvent,
  type ReactElement,
} from "react";

import { Button } from "../../components/ui/button.tsx";
import { Input } from "../../components/ui/input.tsx";

import "./react-debug.css";

type DebugActor = Readonly<{ readonly _tag: "Human"; readonly actorRef: string }>;
type DebugLifecycleTag =
  | "Validated"
  | "Starting"
  | "Running"
  | "Stopped"
  | "Restarting"
  | "Terminated"
  | "Disconnected"
  | "Failed";
type DebugCapabilityName =
  | "configuration_done"
  | "conditional_breakpoints"
  | "hit_conditional_breakpoints"
  | "log_points"
  | "function_breakpoints"
  | "data_breakpoints"
  | "set_variable"
  | "evaluate"
  | "pause"
  | "step_in"
  | "step_over"
  | "step_out"
  | "step_back"
  | "run_to_cursor"
  | "restart_frame"
  | "restart_session"
  | "continue"
  | "disconnect"
  | "terminate"
  | "modules"
  | "loaded_sources"
  | "source_request"
  | "cancel_request";
type DebugControlOperation =
  | "continue"
  | "pause"
  | "step_in"
  | "step_over"
  | "step_out"
  | "step_back"
  | "run_to_cursor"
  | "restart_frame"
  | "restart_session"
  | "disconnect"
  | "terminate";

type DebugBinding = Readonly<{
  readonly projectRef: string;
  readonly rootRef: string;
  readonly worktreeRef: string;
  readonly attachmentGeneration: number;
  readonly languageGeneration: number;
  readonly placementGeneration: number;
  readonly serviceGeneration: number;
  readonly placementRef: string;
  readonly language: string;
}>;

type DebugSource = Readonly<{
  readonly sourceRef: string;
  readonly pathRef: string;
  readonly label: string;
  readonly origin: "project" | "generated" | "remote" | "adapter";
  readonly availability: "available" | "loading" | "unavailable" | "stale";
  readonly sourceMapRef: string | null;
  readonly documentGeneration: number | null;
}>;

type DebugLocation = Readonly<{
  readonly source: DebugSource;
  readonly line: number;
  readonly column: number;
}>;

type DebugBreakpoint = Readonly<{
  readonly _tag: "Source" | "Function" | "Data";
  readonly breakpointRef: string;
  readonly enabled: boolean;
  readonly verified: boolean;
  readonly message: string | null;
  readonly condition: string | null;
  readonly hitCondition: string | null;
  readonly logMessage: string | null;
  readonly location?: DebugLocation;
  readonly requestedLine?: number;
  readonly functionName?: string;
  readonly dataId?: string;
  readonly accessType?: "read" | "write" | "read_write";
}>;

type DebugConfiguration = Readonly<{
  readonly configurationRef: string;
  readonly configurationGeneration: number;
  readonly label: string;
  readonly binding: DebugBinding;
  readonly intent: Readonly<{
    readonly _tag: "Launch" | "Attach";
    readonly executableLabel?: string;
    readonly argumentLabels?: ReadonlyArray<string>;
    readonly prelaunchTaskRef?: string | null;
    readonly postdebugTaskRef?: string | null;
    readonly transportRef?: string;
    readonly targetProcessLabel?: string;
    readonly authenticationRef?: string | null;
  }>;
  readonly placement: Readonly<{
    readonly _tag: "Local" | "Container" | "Remote";
    readonly hostLabel: string;
  }>;
  readonly adapter: Readonly<{
    readonly adapterType: string;
    readonly adapterVersion: string;
    readonly transport: "stdio" | "socket" | "pipe";
    readonly admitted: boolean;
    readonly capabilities: ReadonlyArray<
      Readonly<{
        readonly capability: DebugCapabilityName;
        readonly supported: boolean;
        readonly reason: string | null;
      }>
    >;
  }>;
  readonly targetRef: string;
  readonly cwdRef: string;
  readonly environment: Readonly<{
    readonly admittedKeys: ReadonlyArray<string>;
    readonly redactedKeys: ReadonlyArray<string>;
    readonly sourceRefs: ReadonlyArray<string>;
    readonly valuesExposedToRenderer: false;
    readonly digest: string;
  }>;
  readonly sourceMaps: Readonly<{
    readonly sourceRoots: ReadonlyArray<string>;
    readonly remoteRootRefs: ReadonlyArray<string>;
    readonly generatedSourcesExplicit: boolean;
    readonly guessPositions: false;
  }>;
  readonly admitted: boolean;
  readonly refusalReason: string | null;
}>;

type DebugThread = Readonly<{
  readonly threadRef: string;
  readonly name: string;
  readonly state: "running" | "stopped" | "exited" | "unavailable";
  readonly stopReason: string | null;
}>;
type DebugFrame = Readonly<{
  readonly frameRef: string;
  readonly threadRef: string;
  readonly name: string;
  readonly location: DebugLocation | null;
  readonly canRestart: boolean;
}>;
type DebugScope = Readonly<{
  readonly scopeRef: string;
  readonly frameRef: string;
  readonly name: string;
  readonly expensive: boolean;
  readonly variableCount: number | null;
  readonly state: "loading" | "ready" | "unavailable" | "truncated" | "stale";
}>;
type DebugVariable = Readonly<{
  readonly variableRef: string;
  readonly scopeRef: string | null;
  readonly name: string;
  readonly value: string;
  readonly type: string | null;
  readonly redacted: boolean;
  readonly truncated: boolean;
  readonly childCount: number | null;
}>;
type DebugWatch = Readonly<{
  readonly watchRef: string;
  readonly expression: string;
  readonly value: string;
  readonly type: string | null;
  readonly state: "pending" | "ready" | "failed" | "stale";
  readonly message: string | null;
  readonly redacted: boolean;
  readonly truncated: boolean;
}>;
type DebugModule = Readonly<{
  readonly moduleRef: string;
  readonly name: string;
  readonly pathRef: string | null;
  readonly version: string | null;
  readonly symbolStatus: "loaded" | "missing" | "loading" | "unavailable";
}>;
type DebugConsoleEntry = Readonly<{
  readonly sequence: number;
  readonly category: "console" | "stdout" | "stderr" | "telemetry" | "important";
  readonly text: string;
  readonly redacted: boolean;
  readonly truncated: boolean;
  readonly gapBefore: boolean;
  readonly observedAt: string;
}>;
type DebugReceipt = Readonly<{
  readonly receiptRef: string;
  readonly operationRef: string;
  readonly operation: string;
  readonly disposition: "succeeded" | "refused" | "failed" | "canceled";
  readonly outcome: string;
  readonly observedAt: string;
  readonly sessionGeneration: number | null;
  readonly targetRef: string;
  readonly placementRef: string;
}>;
type DebugCapabilityState =
  | Readonly<{ readonly _tag: "Unconfigured" }>
  | Readonly<{
      readonly _tag: "Starting";
      readonly serviceGeneration: number;
      readonly since: string;
    }>
  | Readonly<{
      readonly _tag: "Ready";
      readonly serviceGeneration: number;
      readonly placementRef: string;
      readonly evidenceTier: string;
      readonly observedAt: string;
    }>
  | Readonly<{
      readonly _tag: "Degraded";
      readonly serviceGeneration: number;
      readonly placementRef: string;
      readonly evidenceTier: string;
      readonly reason: string;
      readonly observedAt: string;
    }>
  | Readonly<{ readonly _tag: "Stopped"; readonly reason: string; readonly stoppedAt: string }>
  | Readonly<{
      readonly _tag: "Failed";
      readonly serviceGeneration: number;
      readonly reason: string;
      readonly retry: "none" | "manual" | "bounded_backoff";
      readonly observedAt: string;
    }>;
type DebugBreakpointSet = Readonly<{
  readonly configurationRef: string;
  readonly breakpoints: ReadonlyArray<DebugBreakpoint>;
  readonly updatedAt: string;
}>;

type DebugSession = Readonly<{
  readonly sessionRef: string;
  readonly sessionGeneration: number;
  readonly adapterGeneration: number;
  readonly targetGeneration: number;
  readonly configuration: DebugConfiguration;
  readonly lifecycle: Readonly<{
    readonly _tag: DebugLifecycleTag;
    readonly reason?: string;
    readonly threadRef?: string | null;
    readonly targetTerminated?: boolean;
  }>;
  readonly breakpoints: ReadonlyArray<DebugBreakpoint>;
  readonly threads: ReadonlyArray<DebugThread>;
  readonly frames: ReadonlyArray<DebugFrame>;
  readonly scopes: ReadonlyArray<DebugScope>;
  readonly variables: ReadonlyArray<DebugVariable>;
  readonly watches: ReadonlyArray<DebugWatch>;
  readonly modules: ReadonlyArray<DebugModule>;
  readonly loadedSources: ReadonlyArray<DebugSource>;
  readonly console: ReadonlyArray<DebugConsoleEntry>;
  readonly invalidatedAreas: ReadonlyArray<string>;
  readonly retainedConsoleBytes: number;
  readonly droppedConsoleBytes: number;
}>;

export type IdeDebugRendererSnapshot = Readonly<{
  readonly schemaVersion: "openagents.desktop.ide-debug.v1";
  readonly binding: DebugBinding;
  readonly capabilityState: DebugCapabilityState;
  readonly configurations: ReadonlyArray<DebugConfiguration>;
  readonly breakpointSets: ReadonlyArray<DebugBreakpointSet>;
  readonly sessions: ReadonlyArray<DebugSession>;
  readonly receipts: ReadonlyArray<DebugReceipt>;
  readonly stopped: boolean;
}>;

type DebugFence = Readonly<
  Pick<
    DebugSession,
    "sessionRef" | "sessionGeneration" | "adapterGeneration" | "targetGeneration"
  > & { readonly operationRef: string }
>;
export type IdeDebugRendererCommand =
  | Readonly<{
      readonly _tag: "Discover";
      readonly operationRef: string;
      readonly actor: DebugActor;
    }>
  | Readonly<{
      readonly _tag: "Validate" | "Start";
      readonly operationRef: string;
      readonly configurationRef: string;
      readonly actor: DebugActor;
    }>
  | Readonly<
      {
        readonly _tag: "Control";
        readonly operation: DebugControlOperation;
        readonly actor: DebugActor;
      } & DebugFence
    >
  | Readonly<
      {
        readonly _tag: "ReplaceBreakpoints";
        readonly breakpoints: ReadonlyArray<DebugBreakpoint>;
        readonly actor: DebugActor;
      } & DebugFence
    >
  | Readonly<
      {
        readonly _tag: "Evaluate";
        readonly expression: string;
        readonly frameRef: string | null;
        readonly actor: DebugActor;
      } & DebugFence
    >
  | Readonly<
      {
        readonly _tag: "SetVariable";
        readonly variableRef: string;
        readonly value: string;
        readonly actor: DebugActor;
      } & DebugFence
    >
  | Readonly<
      {
        readonly _tag: "NavigateSource";
        readonly source: DebugSource;
        readonly actor: DebugActor;
      } & DebugFence
    >
  | Readonly<{
      readonly _tag: "Cancel";
      readonly operationRef: string;
      readonly targetOperationRef: string;
      readonly reason: string;
      readonly actor: DebugActor;
    }>
  | Readonly<{
      readonly _tag: "DeleteRetainedData";
      readonly operationRef: string;
      readonly reason: string;
      readonly actor: DebugActor;
    }>;

export type IdeDebugRendererCommandResult =
  | Readonly<{
      readonly _tag: "Succeeded";
      readonly snapshot: IdeDebugRendererSnapshot;
      readonly payload: unknown;
    }>
  | Readonly<{
      readonly _tag: "Refused";
      readonly snapshot: IdeDebugRendererSnapshot | null;
      readonly reason:
        | "invalid_input"
        | "not_admitted"
        | "stale_generation"
        | "protocol"
        | "unavailable";
      readonly message: string;
    }>;

export type IdeDebugRendererEvent =
  | Readonly<{ readonly _tag: "Snapshot"; readonly snapshot: IdeDebugRendererSnapshot }>
  | Readonly<{
      readonly _tag: "StaleEventDropped";
      readonly sessionRef: string;
      readonly detail: string;
      readonly observedAt: string;
    }>;

export type IdeDebugRendererBridge = Readonly<{
  snapshot: () => Promise<unknown>;
  command: (command: IdeDebugRendererCommand) => Promise<unknown>;
  onEvent: (listener: (event: unknown) => void) => () => void;
}>;

const ownerActor: DebugActor = { _tag: "Human", actorRef: "owner.desktop" };
let operationSequence = 0;
const nextOperationRef = (): string =>
  `ide.debug-operation.renderer-${Date.now().toString(36)}-${++operationSequence}`;
const panelRefs = [
  "variables",
  "watch",
  "breakpoints",
  "console",
  "modules",
  "sources",
  "receipts",
] as const;
type DebugPanelRef = (typeof panelRefs)[number];

const record = (value: unknown): Readonly<Record<string, unknown>> | null =>
  typeof value === "object" && value !== null ? (value as Readonly<Record<string, unknown>>) : null;
const string = (value: unknown): value is string => typeof value === "string";
const number = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);
const array = (value: unknown): ReadonlyArray<unknown> | null =>
  Array.isArray(value) ? value : null;
const nullableString = (value: unknown): boolean => value === null || string(value);
const strings = (value: unknown): value is ReadonlyArray<string> =>
  Array.isArray(value) && value.every(string);

const isBinding = (value: unknown): value is DebugBinding => {
  const item = record(value);
  return (
    item !== null &&
    string(item.projectRef) &&
    string(item.rootRef) &&
    string(item.worktreeRef) &&
    number(item.attachmentGeneration) &&
    number(item.languageGeneration) &&
    number(item.placementGeneration) &&
    number(item.serviceGeneration) &&
    string(item.placementRef) &&
    string(item.language)
  );
};

const isSource = (value: unknown): value is DebugSource => {
  const item = record(value);
  return (
    item !== null &&
    string(item.sourceRef) &&
    string(item.pathRef) &&
    string(item.label) &&
    ["project", "generated", "remote", "adapter"].includes(String(item.origin)) &&
    ["available", "loading", "unavailable", "stale"].includes(String(item.availability)) &&
    nullableString(item.sourceMapRef) &&
    (item.documentGeneration === null || number(item.documentGeneration))
  );
};

const isLocation = (value: unknown): value is DebugLocation => {
  const item = record(value);
  return item !== null && isSource(item.source) && number(item.line) && number(item.column);
};

const isBreakpoint = (value: unknown): value is DebugBreakpoint => {
  const item = record(value);
  return (
    item !== null &&
    ["Source", "Function", "Data"].includes(String(item._tag)) &&
    string(item.breakpointRef) &&
    typeof item.enabled === "boolean" &&
    typeof item.verified === "boolean" &&
    nullableString(item.message) &&
    nullableString(item.condition) &&
    nullableString(item.hitCondition) &&
    nullableString(item.logMessage) &&
    (item._tag !== "Source" || (isLocation(item.location) && number(item.requestedLine))) &&
    (item._tag !== "Function" || string(item.functionName)) &&
    (item._tag !== "Data" || string(item.dataId))
  );
};

const isConfiguration = (value: unknown): value is DebugConfiguration => {
  const item = record(value);
  const intent = record(item?.intent);
  const placement = record(item?.placement);
  const adapter = record(item?.adapter);
  const environment = record(item?.environment);
  const sourceMaps = record(item?.sourceMaps);
  const capabilities = array(adapter?.capabilities);
  return (
    item !== null &&
    string(item.configurationRef) &&
    number(item.configurationGeneration) &&
    string(item.label) &&
    isBinding(item.binding) &&
    intent !== null &&
    ["Launch", "Attach"].includes(String(intent._tag)) &&
    placement !== null &&
    ["Local", "Container", "Remote"].includes(String(placement._tag)) &&
    string(placement.hostLabel) &&
    adapter !== null &&
    string(adapter.adapterType) &&
    string(adapter.adapterVersion) &&
    ["stdio", "socket", "pipe"].includes(String(adapter.transport)) &&
    typeof adapter.admitted === "boolean" &&
    capabilities !== null &&
    capabilities.every((capability) => {
      const candidate = record(capability);
      return (
        candidate !== null &&
        string(candidate.capability) &&
        typeof candidate.supported === "boolean" &&
        nullableString(candidate.reason)
      );
    }) &&
    string(item.targetRef) &&
    string(item.cwdRef) &&
    environment !== null &&
    strings(environment.admittedKeys) &&
    strings(environment.redactedKeys) &&
    strings(environment.sourceRefs) &&
    environment.valuesExposedToRenderer === false &&
    string(environment.digest) &&
    sourceMaps !== null &&
    strings(sourceMaps.sourceRoots) &&
    strings(sourceMaps.remoteRootRefs) &&
    typeof sourceMaps.generatedSourcesExplicit === "boolean" &&
    sourceMaps.guessPositions === false &&
    typeof item.admitted === "boolean" &&
    nullableString(item.refusalReason)
  );
};

const isThread = (value: unknown): value is DebugThread => {
  const item = record(value);
  return (
    item !== null &&
    string(item.threadRef) &&
    string(item.name) &&
    ["running", "stopped", "exited", "unavailable"].includes(String(item.state)) &&
    nullableString(item.stopReason)
  );
};
const isFrame = (value: unknown): value is DebugFrame => {
  const item = record(value);
  return (
    item !== null &&
    string(item.frameRef) &&
    string(item.threadRef) &&
    string(item.name) &&
    (item.location === null || isLocation(item.location)) &&
    typeof item.canRestart === "boolean"
  );
};
const isScope = (value: unknown): value is DebugScope => {
  const item = record(value);
  return (
    item !== null &&
    string(item.scopeRef) &&
    string(item.frameRef) &&
    string(item.name) &&
    typeof item.expensive === "boolean" &&
    (item.variableCount === null || number(item.variableCount)) &&
    ["loading", "ready", "unavailable", "truncated", "stale"].includes(String(item.state))
  );
};
const isVariable = (value: unknown): value is DebugVariable => {
  const item = record(value);
  return (
    item !== null &&
    string(item.variableRef) &&
    nullableString(item.scopeRef) &&
    string(item.name) &&
    string(item.value) &&
    nullableString(item.type) &&
    typeof item.redacted === "boolean" &&
    typeof item.truncated === "boolean" &&
    (item.childCount === null || number(item.childCount))
  );
};
const isWatch = (value: unknown): value is DebugWatch => {
  const item = record(value);
  return (
    item !== null &&
    string(item.watchRef) &&
    string(item.expression) &&
    string(item.value) &&
    nullableString(item.type) &&
    ["pending", "ready", "failed", "stale"].includes(String(item.state)) &&
    nullableString(item.message) &&
    typeof item.redacted === "boolean" &&
    typeof item.truncated === "boolean"
  );
};
const isModule = (value: unknown): value is DebugModule => {
  const item = record(value);
  return (
    item !== null &&
    string(item.moduleRef) &&
    string(item.name) &&
    nullableString(item.pathRef) &&
    nullableString(item.version) &&
    ["loaded", "missing", "loading", "unavailable"].includes(String(item.symbolStatus))
  );
};
const isConsoleEntry = (value: unknown): value is DebugConsoleEntry => {
  const item = record(value);
  return (
    item !== null &&
    number(item.sequence) &&
    ["console", "stdout", "stderr", "telemetry", "important"].includes(String(item.category)) &&
    string(item.text) &&
    typeof item.redacted === "boolean" &&
    typeof item.truncated === "boolean" &&
    typeof item.gapBefore === "boolean" &&
    string(item.observedAt)
  );
};
const isReceipt = (value: unknown): value is DebugReceipt => {
  const item = record(value);
  return (
    item !== null &&
    string(item.receiptRef) &&
    string(item.operationRef) &&
    string(item.operation) &&
    ["succeeded", "refused", "failed", "canceled"].includes(String(item.disposition)) &&
    string(item.outcome) &&
    string(item.observedAt) &&
    (item.sessionGeneration === null || number(item.sessionGeneration)) &&
    string(item.targetRef) &&
    string(item.placementRef)
  );
};

const isCapabilityState = (value: unknown): value is DebugCapabilityState => {
  const item = record(value);
  if (
    item === null ||
    !["Unconfigured", "Starting", "Ready", "Degraded", "Stopped", "Failed"].includes(
      String(item._tag),
    )
  )
    return false;
  if (item._tag === "Unconfigured") return true;
  if (item._tag === "Stopped") return string(item.reason) && string(item.stoppedAt);
  if (!number(item.serviceGeneration)) return false;
  if (item._tag === "Starting") return string(item.since);
  if (item._tag === "Failed")
    return (
      string(item.reason) &&
      ["none", "manual", "bounded_backoff"].includes(String(item.retry)) &&
      string(item.observedAt)
    );
  return (
    string(item.placementRef) &&
    string(item.evidenceTier) &&
    string(item.observedAt) &&
    (item._tag !== "Degraded" || string(item.reason))
  );
};

const isBreakpointSet = (value: unknown): value is DebugBreakpointSet => {
  const item = record(value);
  return (
    item !== null &&
    string(item.configurationRef) &&
    every(item.breakpoints, isBreakpoint) &&
    string(item.updatedAt)
  );
};

const every = <Item,>(
  value: unknown,
  guard: (candidate: unknown) => candidate is Item,
): value is ReadonlyArray<Item> => Array.isArray(value) && value.every(guard);

const isSession = (value: unknown): value is DebugSession => {
  const item = record(value);
  const lifecycle = record(item?.lifecycle);
  return (
    item !== null &&
    string(item.sessionRef) &&
    number(item.sessionGeneration) &&
    number(item.adapterGeneration) &&
    number(item.targetGeneration) &&
    isConfiguration(item.configuration) &&
    lifecycle !== null &&
    [
      "Validated",
      "Starting",
      "Running",
      "Stopped",
      "Restarting",
      "Terminated",
      "Disconnected",
      "Failed",
    ].includes(String(lifecycle._tag)) &&
    every(item.breakpoints, isBreakpoint) &&
    every(item.threads, isThread) &&
    every(item.frames, isFrame) &&
    every(item.scopes, isScope) &&
    every(item.variables, isVariable) &&
    every(item.watches, isWatch) &&
    every(item.modules, isModule) &&
    every(item.loadedSources, isSource) &&
    every(item.console, isConsoleEntry) &&
    strings(item.invalidatedAreas) &&
    number(item.retainedConsoleBytes) &&
    number(item.droppedConsoleBytes)
  );
};

export const decodeIdeDebugRendererSnapshot = (value: unknown): IdeDebugRendererSnapshot | null => {
  const item = record(value);
  if (
    item === null ||
    item.schemaVersion !== "openagents.desktop.ide-debug.v1" ||
    !isBinding(item.binding) ||
    !isCapabilityState(item.capabilityState) ||
    !every(item.configurations, isConfiguration) ||
    !every(item.breakpointSets, isBreakpointSet) ||
    !every(item.sessions, isSession) ||
    !every(item.receipts, isReceipt) ||
    typeof item.stopped !== "boolean"
  )
    return null;
  return {
    schemaVersion: item.schemaVersion,
    binding: item.binding,
    capabilityState: item.capabilityState,
    configurations: item.configurations,
    breakpointSets: item.breakpointSets,
    sessions: item.sessions,
    receipts: item.receipts,
    stopped: item.stopped,
  };
};

type DebugRefusalReason = Extract<
  IdeDebugRendererCommandResult,
  { readonly _tag: "Refused" }
>["reason"];
const isDebugRefusalReason = (value: unknown): value is DebugRefusalReason =>
  ["invalid_input", "not_admitted", "stale_generation", "protocol", "unavailable"].includes(
    String(value),
  );

const decodeCommandResult = (value: unknown): IdeDebugRendererCommandResult | null => {
  const item = record(value);
  if (item?._tag === "Succeeded") {
    const snapshot = decodeIdeDebugRendererSnapshot(item.snapshot);
    return snapshot === null ? null : { _tag: "Succeeded", snapshot, payload: item.payload };
  }
  if (item?._tag === "Refused") {
    const reason = item.reason;
    const snapshot = item.snapshot === null ? null : decodeIdeDebugRendererSnapshot(item.snapshot);
    if (
      !isDebugRefusalReason(reason) ||
      !string(item.message) ||
      (item.snapshot !== null && snapshot === null)
    )
      return null;
    return { _tag: "Refused", snapshot, reason, message: item.message };
  }
  return null;
};

const decodeEvent = (value: unknown): IdeDebugRendererEvent | null => {
  const item = record(value);
  if (item?._tag === "Snapshot") {
    const snapshot = decodeIdeDebugRendererSnapshot(item.snapshot);
    return snapshot === null ? null : { _tag: "Snapshot", snapshot };
  }
  return item?._tag === "StaleEventDropped" &&
    string(item.sessionRef) &&
    string(item.detail) &&
    string(item.observedAt)
    ? {
        _tag: "StaleEventDropped",
        sessionRef: item.sessionRef,
        detail: item.detail,
        observedAt: item.observedAt,
      }
    : null;
};

const method = (
  value: unknown,
  name: string,
): ((...args: ReadonlyArray<unknown>) => unknown) | null => {
  const owner = record(value);
  const candidate = owner?.[name];
  return typeof candidate === "function" ? candidate.bind(value) : null;
};

export const readIdeDebugRendererBridge = (): IdeDebugRendererBridge | null => {
  const desktop = record(Reflect.get(globalThis, "openagentsDesktop"));
  const value = desktop?.ideDebug;
  const snapshot = method(value, "snapshot");
  const command = method(value, "command");
  const onEvent = method(value, "onEvent");
  if (snapshot === null || command === null || onEvent === null) return null;
  return {
    snapshot: () => Promise.resolve(snapshot()),
    command: (input) => Promise.resolve(command(input)),
    onEvent: (listener) => {
      const unsubscribe = onEvent(listener);
      return typeof unsubscribe === "function"
        ? () => {
            Reflect.apply(unsubscribe, value, []);
          }
        : () => undefined;
    },
  };
};

export const acceptsDebugSnapshotEvent = (
  current: IdeDebugRendererSnapshot,
  incoming: IdeDebugRendererSnapshot,
): boolean => {
  if (
    current.binding.projectRef !== incoming.binding.projectRef ||
    current.binding.rootRef !== incoming.binding.rootRef
  )
    return true;
  if (incoming.binding.serviceGeneration < current.binding.serviceGeneration) return false;
  if (incoming.binding.serviceGeneration > current.binding.serviceGeneration) return true;
  return current.sessions.every((currentSession) => {
    const next = incoming.sessions.find(
      (candidate) => candidate.sessionRef === currentSession.sessionRef,
    );
    return next === undefined || next.sessionGeneration >= currentSession.sessionGeneration;
  });
};

const fence = (session: DebugSession): DebugFence => ({
  operationRef: nextOperationRef(),
  sessionRef: session.sessionRef,
  sessionGeneration: session.sessionGeneration,
  adapterGeneration: session.adapterGeneration,
  targetGeneration: session.targetGeneration,
});

const sentence = (value: string): string =>
  value.replaceAll("_", " ").replace(/^./u, (letter) => letter.toLocaleUpperCase());
const lifecycleReason = (session: DebugSession): string | null =>
  session.lifecycle.reason ??
  session.threads.find((thread) => thread.stopReason !== null)?.stopReason ??
  null;
const lifecycleTone = (tag: DebugLifecycleTag): "neutral" | "success" | "warning" | "danger" =>
  tag === "Running"
    ? "success"
    : tag === "Stopped" || tag === "Restarting"
      ? "warning"
      : tag === "Failed"
        ? "danger"
        : "neutral";
const locationLabel = (location: DebugLocation | null): string =>
  location === null
    ? "Source unavailable"
    : `${location.source.label}:${location.line}:${location.column}`;
const breakpointLabel = (breakpoint: DebugBreakpoint): string =>
  breakpoint._tag === "Source"
    ? locationLabel(breakpoint.location ?? null)
    : breakpoint._tag === "Function"
      ? (breakpoint.functionName ?? "Unnamed function")
      : `${breakpoint.dataId ?? "Unknown data"} · ${(breakpoint.accessType ?? "read_write").replaceAll("_", "/")}`;

const capabilityFor = (
  session: DebugSession,
  capability: DebugCapabilityName,
): Readonly<{ supported: boolean; reason: string }> => {
  const declared = session.configuration.adapter.capabilities.find(
    (candidate) => candidate.capability === capability,
  );
  return declared?.supported === true
    ? { supported: true, reason: "Supported by the active adapter." }
    : {
        supported: false,
        reason:
          declared?.reason ??
          `The active adapter does not support ${capability.replaceAll("_", " ")}.`,
      };
};

const controlAvailability = (
  session: DebugSession,
  operation: DebugControlOperation,
): Readonly<{ enabled: boolean; reason: string }> => {
  const capability = capabilityFor(
    session,
    operation === "restart_frame"
      ? "restart_frame"
      : operation === "restart_session"
        ? "restart_session"
        : operation,
  );
  if (!capability.supported) return { enabled: false, reason: capability.reason };
  if (["Terminated", "Disconnected", "Failed"].includes(session.lifecycle._tag))
    return {
      enabled: false,
      reason: `The session is ${session.lifecycle._tag.toLocaleLowerCase()}.`,
    };
  const stopped = session.lifecycle._tag === "Stopped";
  if (
    [
      "continue",
      "step_in",
      "step_over",
      "step_out",
      "step_back",
      "run_to_cursor",
      "restart_frame",
    ].includes(operation) &&
    !stopped
  ) {
    return { enabled: false, reason: "Stop the target before using this control." };
  }
  if (operation === "pause" && session.lifecycle._tag !== "Running")
    return { enabled: false, reason: "Pause is available while the target is running." };
  return { enabled: true, reason: capability.reason };
};

const debugPanelLabel: Readonly<Record<DebugPanelRef, string>> = {
  variables: "Variables",
  watch: "Watch",
  breakpoints: "Breakpoints",
  console: "Console",
  modules: "Modules",
  sources: "Sources",
  receipts: "Receipts",
};

const DebugStatus = ({
  label,
  tone = "neutral",
}: {
  readonly label: string;
  readonly tone?: "neutral" | "success" | "warning" | "danger";
}): ReactElement => (
  <span className="oa-debug-status" data-tone={tone}>
    <span aria-hidden="true" className="oa-debug-status-mark" />
    {label}
  </span>
);

const EmptyState = ({
  title,
  detail,
}: {
  readonly title: string;
  readonly detail: string;
}): ReactElement => (
  <div className="oa-debug-empty">
    <strong>{title}</strong>
    <p>{detail}</p>
  </div>
);

type ReactIdeDebugPanelProps = Readonly<{ readonly bridge?: IdeDebugRendererBridge | null }>;

export const ReactIdeDebugPanel = ({
  bridge: injectedBridge,
}: ReactIdeDebugPanelProps): ReactElement => {
  const bridge = useMemo(
    () => (injectedBridge === undefined ? readIdeDebugRendererBridge() : injectedBridge),
    [injectedBridge],
  );
  const [snapshot, setSnapshot] = useState<IdeDebugRendererSnapshot | null>(null);
  const [phase, setPhase] = useState<"loading" | "ready" | "unavailable">("loading");
  const [notice, setNotice] = useState<Readonly<{
    kind: "status" | "warning" | "error";
    text: string;
  }> | null>(null);
  const [pending, setPending] = useState<Readonly<{
    readonly operationRef: string | null;
    readonly label: string;
  }> | null>(null);
  const [selectedConfigurationRef, setSelectedConfigurationRef] = useState<string | null>(null);
  const [selectedSessionRef, setSelectedSessionRef] = useState<string | null>(null);
  const [selectedThreadRef, setSelectedThreadRef] = useState<string | null>(null);
  const [selectedFrameRef, setSelectedFrameRef] = useState<string | null>(null);
  const [panel, setPanel] = useState<DebugPanelRef>("variables");
  const [editingVariableRef, setEditingVariableRef] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const adoptSnapshot = useCallback((next: IdeDebugRendererSnapshot): void => {
    setSnapshot((current) =>
      current !== null && !acceptsDebugSnapshotEvent(current, next) ? current : next,
    );
    setPhase("ready");
    setSelectedConfigurationRef((current) =>
      next.configurations.some((candidate) => candidate.configurationRef === current)
        ? current
        : (next.configurations[0]?.configurationRef ?? null),
    );
    setSelectedSessionRef((current) =>
      next.sessions.some((candidate) => candidate.sessionRef === current)
        ? current
        : (next.sessions.at(-1)?.sessionRef ?? null),
    );
  }, []);

  const refresh = useCallback(async (): Promise<void> => {
    if (bridge === null) {
      setPhase("unavailable");
      setNotice({ kind: "warning", text: "Debug services are unavailable in this Desktop host." });
      return;
    }
    setPending({ operationRef: null, label: "Refresh debug graph" });
    const decoded = decodeIdeDebugRendererSnapshot(await bridge.snapshot().catch(() => null));
    setPending(null);
    if (decoded === null) {
      setPhase("unavailable");
      setNotice({
        kind: "warning",
        text: "Choose an admitted project with a valid debug configuration.",
      });
      return;
    }
    adoptSnapshot(decoded);
    setNotice(null);
  }, [adoptSnapshot, bridge]);

  useEffect(() => {
    void refresh();
    if (bridge === null) return;
    return bridge.onEvent((raw) => {
      const event = decodeEvent(raw);
      if (event === null) {
        setNotice({
          kind: "error",
          text: "The debug bridge emitted an invalid event. The current projection was kept.",
        });
      } else if (event._tag === "StaleEventDropped") {
        setNotice({ kind: "warning", text: `A stale debug event was dropped: ${event.detail}` });
      } else {
        setSnapshot((current) => {
          if (current !== null && !acceptsDebugSnapshotEvent(current, event.snapshot)) {
            setNotice({
              kind: "warning",
              text: "A stale debug snapshot was dropped. The current generation was kept.",
            });
            return current;
          }
          return event.snapshot;
        });
        setPhase("ready");
      }
    });
  }, [bridge, refresh]);

  const run = useCallback(
    async (command: IdeDebugRendererCommand, label: string): Promise<void> => {
      if (bridge === null) return;
      setPending({ operationRef: command.operationRef, label });
      setNotice({ kind: "status", text: `${label} is in progress.` });
      const decoded = decodeCommandResult(await bridge.command(command).catch(() => null));
      setPending(null);
      if (decoded === null) {
        setNotice({
          kind: "error",
          text: "The debug command returned an invalid response. No local state was assumed.",
        });
        return;
      }
      if (decoded.snapshot !== null) adoptSnapshot(decoded.snapshot);
      if (decoded._tag === "Refused") {
        setNotice({
          kind: decoded.reason === "stale_generation" ? "warning" : "error",
          text: `${sentence(decoded.reason)}: ${decoded.message}`,
        });
      } else {
        setNotice({ kind: "status", text: `${label} completed with a debug receipt.` });
      }
    },
    [adoptSnapshot, bridge],
  );

  const configuration =
    snapshot?.configurations.find(
      (candidate) => candidate.configurationRef === selectedConfigurationRef,
    ) ??
    snapshot?.configurations[0] ??
    null;
  const session =
    snapshot?.sessions.find((candidate) => candidate.sessionRef === selectedSessionRef) ??
    snapshot?.sessions.at(-1) ??
    null;
  const thread =
    session?.threads.find((candidate) => candidate.threadRef === selectedThreadRef) ??
    session?.threads.find((candidate) => candidate.state === "stopped") ??
    session?.threads[0] ??
    null;
  const frames =
    session === null || thread === null
      ? []
      : session.frames.filter((candidate) => candidate.threadRef === thread.threadRef);
  const frame =
    frames.find((candidate) => candidate.frameRef === selectedFrameRef) ?? frames[0] ?? null;
  const scopes =
    session === null || frame === null
      ? []
      : session.scopes.filter((candidate) => candidate.frameRef === frame.frameRef);
  const cancelTargetOperationRef = pending?.operationRef ?? null;

  useEffect(() => {
    setSelectedThreadRef((current) =>
      session?.threads.some((candidate) => candidate.threadRef === current) === true
        ? current
        : (session?.threads.find((candidate) => candidate.state === "stopped")?.threadRef ??
          session?.threads[0]?.threadRef ??
          null),
    );
  }, [session]);
  useEffect(() => {
    setSelectedFrameRef((current) =>
      frames.some((candidate) => candidate.frameRef === current)
        ? current
        : (frames[0]?.frameRef ?? null),
    );
  }, [frames]);

  const control = (operation: DebugControlOperation): void => {
    if (session === null) return;
    void run(
      { _tag: "Control", ...fence(session), operation, actor: ownerActor },
      sentence(operation),
    );
  };
  const navigate = (source: DebugSource): void => {
    if (session === null || source.availability !== "available") return;
    void run(
      { _tag: "NavigateSource", ...fence(session), source, actor: ownerActor },
      `Open ${source.label}`,
    );
  };

  const onPanelKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const focused = [
      ...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]'),
    ].findIndex((candidate) => candidate === event.currentTarget.ownerDocument.activeElement);
    const current = focused >= 0 ? focused : panelRefs.indexOf(panel);
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? panelRefs.length - 1
          : (current + (event.key === "ArrowRight" ? 1 : -1) + panelRefs.length) % panelRefs.length;
    setPanel(panelRefs[next]!);
    event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
  };

  if (phase !== "ready" || snapshot === null) {
    return (
      <div className="oa-debug-loading" role={phase === "unavailable" ? "alert" : "status"}>
        <strong>{phase === "loading" ? "Loading debug graph…" : "Debug unavailable"}</strong>
        <p>{notice?.text ?? "Reading the current project and debug generations."}</p>
        <Button
          disabled={pending !== null}
          size="sm"
          variant="ghost"
          onClick={() => void refresh()}
        >
          Refresh
        </Button>
      </div>
    );
  }

  return (
    <div className="oa-debug-workbench" data-debug-stopped={snapshot.stopped ? "true" : "false"}>
      <header className="oa-debug-header">
        <div className="oa-debug-title">
          <strong>Debugger</strong>
          <small>
            {snapshot.binding.language} · project generation {snapshot.binding.attachmentGeneration}{" "}
            · service generation {snapshot.binding.serviceGeneration}
          </small>
        </div>
        <label className="oa-debug-configuration-select">
          <span>Configuration</span>
          <select
            disabled={snapshot.configurations.length === 0 || pending !== null}
            value={configuration?.configurationRef ?? ""}
            onChange={(event) => setSelectedConfigurationRef(event.currentTarget.value)}
          >
            {snapshot.configurations.map((candidate) => (
              <option key={candidate.configurationRef} value={candidate.configurationRef}>
                {candidate.label}
              </option>
            ))}
          </select>
        </label>
        <Button
          disabled={pending !== null || snapshot.stopped}
          size="sm"
          variant="ghost"
          onClick={() =>
            void run(
              { _tag: "Discover", operationRef: nextOperationRef(), actor: ownerActor },
              "Discover configurations",
            )
          }
        >
          Discover
        </Button>
        {configuration === null ? null : (
          <>
            <Button
              disabled={pending !== null}
              size="sm"
              variant="ghost"
              onClick={() =>
                void run(
                  {
                    _tag: "Validate",
                    operationRef: nextOperationRef(),
                    configurationRef: configuration.configurationRef,
                    actor: ownerActor,
                  },
                  "Validate configuration",
                )
              }
            >
              Validate
            </Button>
            <Button
              disabled={pending !== null || !configuration.admitted || snapshot.stopped}
              size="sm"
              title={
                configuration.admitted
                  ? "Start the selected configuration."
                  : (configuration.refusalReason ?? "Configuration not admitted.")
              }
              onClick={() =>
                void run(
                  {
                    _tag: "Start",
                    operationRef: nextOperationRef(),
                    configurationRef: configuration.configurationRef,
                    actor: ownerActor,
                  },
                  `${configuration.intent._tag} session`,
                )
              }
            >
              {configuration.intent._tag}
            </Button>
          </>
        )}
        <Button
          aria-label="Refresh debug graph"
          disabled={pending !== null}
          size="sm"
          variant="ghost"
          onClick={() => void refresh()}
        >
          Refresh
        </Button>
      </header>

      {notice === null ? null : (
        <p
          className="oa-debug-notice"
          data-kind={notice.kind}
          role={notice.kind === "error" ? "alert" : "status"}
        >
          {notice.text}
        </p>
      )}
      {snapshot.capabilityState._tag === "Degraded" ||
      snapshot.capabilityState._tag === "Failed" ||
      snapshot.capabilityState._tag === "Stopped" ? (
        <p
          className="oa-debug-notice"
          data-kind={snapshot.capabilityState._tag === "Failed" ? "error" : "warning"}
          role="alert"
        >
          Debug capability {snapshot.capabilityState._tag.toLocaleLowerCase()}:{" "}
          {snapshot.capabilityState.reason}
        </p>
      ) : null}
      {snapshot.stopped ? (
        <p className="oa-debug-notice" data-kind="warning" role="alert">
          The debug graph is stopped. Controls are unavailable until the host creates a new
          generation.
        </p>
      ) : null}

      {configuration === null ? (
        <EmptyState
          title="No debug configuration"
          detail="Add an admitted project debug configuration. The renderer cannot invent an adapter or target."
        />
      ) : (
        <details className="oa-debug-disclosure">
          <summary>
            <span>
              {configuration.intent._tag} · {configuration.label}
            </span>
            <DebugStatus
              label={configuration.admitted ? "Admitted" : "Refused"}
              tone={configuration.admitted ? "success" : "danger"}
            />
          </summary>
          <dl>
            <div>
              <dt>Effective target</dt>
              <dd>
                {configuration.intent._tag === "Launch"
                  ? (configuration.intent.executableLabel ?? configuration.targetRef)
                  : (configuration.intent.targetProcessLabel ?? configuration.targetRef)}
              </dd>
            </div>
            <div>
              <dt>Adapter</dt>
              <dd>
                {configuration.adapter.adapterType} {configuration.adapter.adapterVersion} ·{" "}
                {configuration.adapter.transport}
              </dd>
            </div>
            <div>
              <dt>Placement</dt>
              <dd>
                {configuration.placement._tag} · {configuration.placement.hostLabel}
              </dd>
            </div>
            <div>
              <dt>Working directory</dt>
              <dd>{configuration.cwdRef}</dd>
            </div>
            <div>
              <dt>Generation</dt>
              <dd>
                configuration {configuration.configurationGeneration} · placement{" "}
                {configuration.binding.placementGeneration} · language{" "}
                {configuration.binding.languageGeneration}
              </dd>
            </div>
            <div>
              <dt>Arguments</dt>
              <dd>{configuration.intent.argumentLabels?.join(" ") || "None disclosed"}</dd>
            </div>
            <div>
              <dt>Environment</dt>
              <dd>
                {configuration.environment.admittedKeys.length} admitted key names ·{" "}
                {configuration.environment.redactedKeys.length} redacted key names · values withheld
              </dd>
            </div>
            <div>
              <dt>Source mapping</dt>
              <dd>
                {configuration.sourceMaps.sourceRoots.length} roots ·{" "}
                {configuration.sourceMaps.remoteRootRefs.length} remote roots · generated sources{" "}
                {configuration.sourceMaps.generatedSourcesExplicit ? "explicit" : "not declared"} ·
                position guessing off
              </dd>
            </div>
            <div>
              <dt>Persisted breakpoints</dt>
              <dd>
                {snapshot.breakpointSets.find(
                  (candidate) => candidate.configurationRef === configuration.configurationRef,
                )?.breakpoints.length ?? 0}{" "}
                retained for this configuration
              </dd>
            </div>
          </dl>
          {configuration.refusalReason === null ? null : (
            <p role="alert">Refusal: {configuration.refusalReason}</p>
          )}
        </details>
      )}

      {session === null ? (
        <EmptyState
          title="No active debug session"
          detail="Validate and start an admitted launch or attach configuration. All target work remains in the supervised host."
        />
      ) : (
        <>
          <section className="oa-debug-session" aria-label="Active debug session">
            <div className="oa-debug-session-summary">
              <label>
                <span>Session</span>
                <select
                  value={session.sessionRef}
                  onChange={(event) => setSelectedSessionRef(event.currentTarget.value)}
                >
                  {snapshot.sessions.map((candidate) => (
                    <option key={candidate.sessionRef} value={candidate.sessionRef}>
                      {candidate.configuration.label} · generation {candidate.sessionGeneration}
                    </option>
                  ))}
                </select>
              </label>
              <DebugStatus
                label={session.lifecycle._tag}
                tone={lifecycleTone(session.lifecycle._tag)}
              />
              <span className="oa-debug-generations">
                session {session.sessionGeneration} · adapter {session.adapterGeneration} · target{" "}
                {session.targetGeneration}
              </span>
              {lifecycleReason(session) === null ? null : (
                <span className="oa-debug-lifecycle-reason">{lifecycleReason(session)}</span>
              )}
            </div>
            <div className="oa-debug-controls" role="toolbar" aria-label="Debug controls">
              {(
                [
                  ["continue", "Continue"],
                  ["pause", "Pause"],
                  ["step_over", "Step over"],
                  ["step_in", "Step in"],
                  ["step_out", "Step out"],
                  ["step_back", "Step back"],
                  ["run_to_cursor", "Run to cursor"],
                  ["restart_frame", "Restart frame"],
                  ["restart_session", "Restart"],
                  ["disconnect", "Disconnect"],
                  ["terminate", "Terminate"],
                ] as const
              ).map(([operation, label]) => {
                const availability = controlAvailability(session, operation);
                return (
                  <Button
                    key={operation}
                    size="sm"
                    variant={operation === "terminate" ? "destructive" : "ghost"}
                    disabled={!availability.enabled || pending !== null || snapshot.stopped}
                    title={availability.reason}
                    onClick={() => control(operation)}
                  >
                    {label}
                  </Button>
                );
              })}
              {cancelTargetOperationRef === null ||
              pending === null ||
              !capabilityFor(session, "cancel_request").supported ? null : (
                <Button
                  size="sm"
                  variant="destructive"
                  title={capabilityFor(session, "cancel_request").reason}
                  onClick={() =>
                    void run(
                      {
                        _tag: "Cancel",
                        operationRef: nextOperationRef(),
                        targetOperationRef: cancelTargetOperationRef,
                        reason: `Owner canceled ${pending.label}.`,
                        actor: ownerActor,
                      },
                      "Cancel request",
                    )
                  }
                >
                  Cancel request
                </Button>
              )}
            </div>
          </section>

          {session.invalidatedAreas.length === 0 ? null : (
            <p className="oa-debug-notice" data-kind="warning" role="status">
              Refreshing invalidated data: {session.invalidatedAreas.join(", ")}.
            </p>
          )}

          <div className="oa-debug-grid">
            <aside className="oa-debug-execution" aria-label="Threads and call stack">
              <section>
                <header>
                  <strong>Threads</strong>
                  <span>{session.threads.length}</span>
                </header>
                {session.threads.length === 0 ? (
                  <p>No threads are available.</p>
                ) : (
                  <ul>
                    {session.threads.map((candidate) => (
                      <li key={candidate.threadRef}>
                        <button
                          aria-current={
                            candidate.threadRef === thread?.threadRef ? "true" : undefined
                          }
                          onClick={() => setSelectedThreadRef(candidate.threadRef)}
                          type="button"
                        >
                          <span>{candidate.name}</span>
                          <DebugStatus
                            label={candidate.state}
                            tone={
                              candidate.state === "stopped"
                                ? "warning"
                                : candidate.state === "running"
                                  ? "success"
                                  : "neutral"
                            }
                          />
                        </button>
                        {candidate.stopReason === null ? null : (
                          <small>{candidate.stopReason}</small>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </section>
              <section>
                <header>
                  <strong>Call stack</strong>
                  <span>{frames.length}</span>
                </header>
                {frames.length === 0 ? (
                  <p>No frames are available for this thread.</p>
                ) : (
                  <ol>
                    {frames.map((candidate) => (
                      <li key={candidate.frameRef}>
                        <button
                          aria-current={candidate.frameRef === frame?.frameRef ? "true" : undefined}
                          onClick={() => {
                            setSelectedFrameRef(candidate.frameRef);
                            if (candidate.location?.source.availability === "available")
                              navigate(candidate.location.source);
                          }}
                          type="button"
                        >
                          <span>{candidate.name}</span>
                          <small>{locationLabel(candidate.location)}</small>
                        </button>
                      </li>
                    ))}
                  </ol>
                )}
              </section>
            </aside>

            <section className="oa-debug-inspector" aria-label="Debug inspector">
              <nav
                className="oa-debug-panel-tabs"
                role="tablist"
                aria-label="Debug data"
                onKeyDown={onPanelKeyDown}
              >
                {panelRefs.map((candidate) => (
                  <button
                    aria-controls={`oa-debug-panel-${candidate}`}
                    aria-selected={panel === candidate}
                    id={`oa-debug-tab-${candidate}`}
                    key={candidate}
                    onClick={() => setPanel(candidate)}
                    role="tab"
                    tabIndex={panel === candidate ? 0 : -1}
                    type="button"
                  >
                    {debugPanelLabel[candidate]}
                  </button>
                ))}
              </nav>

              <div
                aria-labelledby={`oa-debug-tab-${panel}`}
                className="oa-debug-panel"
                id={`oa-debug-panel-${panel}`}
                role="tabpanel"
                tabIndex={0}
              >
                {panel === "variables" ? (
                  <div className="oa-debug-variables">
                    {frame === null ? (
                      <EmptyState
                        title="No selected frame"
                        detail="Select a stopped thread and frame to inspect scopes and variables."
                      />
                    ) : scopes.length === 0 ? (
                      <EmptyState
                        title="No scopes"
                        detail="The adapter did not project scopes for this frame."
                      />
                    ) : (
                      scopes.map((scope) => (
                        <section key={scope.scopeRef}>
                          <header>
                            <strong>{scope.name}</strong>
                            <DebugStatus
                              label={scope.state}
                              tone={
                                scope.state === "ready"
                                  ? "success"
                                  : scope.state === "unavailable" || scope.state === "stale"
                                    ? "warning"
                                    : "neutral"
                              }
                            />
                            <span>
                              {scope.variableCount ?? "unknown"} values
                              {scope.expensive ? " · expensive" : ""}
                            </span>
                          </header>
                          {session.variables.filter(
                            (variable) => variable.scopeRef === scope.scopeRef,
                          ).length === 0 ? (
                            <p>No retained variables in this scope.</p>
                          ) : (
                            <dl>
                              {session.variables
                                .filter((variable) => variable.scopeRef === scope.scopeRef)
                                .map((variable) => (
                                  <div key={variable.variableRef}>
                                    <dt>
                                      {variable.name}
                                      <small>
                                        {variable.type ?? "unknown type"}
                                        {variable.childCount === null
                                          ? ""
                                          : ` · ${variable.childCount} children`}
                                      </small>
                                    </dt>
                                    <dd>
                                      <code>{variable.value}</code>
                                      {variable.redacted ? (
                                        <DebugStatus label="Redacted" tone="warning" />
                                      ) : null}
                                      {variable.truncated ? (
                                        <DebugStatus label="Truncated" tone="warning" />
                                      ) : null}
                                      {editingVariableRef === variable.variableRef ? (
                                        <form
                                          onSubmit={(event) => {
                                            event.preventDefault();
                                            const field =
                                              event.currentTarget.elements.namedItem("value");
                                            const value =
                                              field instanceof HTMLInputElement ? field.value : "";
                                            if (value === "") return;
                                            void run(
                                              {
                                                _tag: "SetVariable",
                                                ...fence(session),
                                                variableRef: variable.variableRef,
                                                value,
                                                actor: ownerActor,
                                              },
                                              `Set ${variable.name}`,
                                            );
                                            setEditingVariableRef(null);
                                          }}
                                        >
                                          <Input
                                            autoFocus
                                            aria-label={`New value for ${variable.name}`}
                                            defaultValue={variable.value}
                                            maxLength={16_384}
                                            name="value"
                                            required
                                          />
                                          <Button size="sm" type="submit">
                                            Set
                                          </Button>
                                          <Button
                                            size="sm"
                                            type="button"
                                            variant="ghost"
                                            onClick={() => setEditingVariableRef(null)}
                                          >
                                            Cancel
                                          </Button>
                                        </form>
                                      ) : capabilityFor(session, "set_variable").supported ? (
                                        <Button
                                          size="sm"
                                          variant="ghost"
                                          onClick={() =>
                                            setEditingVariableRef(variable.variableRef)
                                          }
                                        >
                                          Edit
                                        </Button>
                                      ) : null}
                                    </dd>
                                  </div>
                                ))}
                            </dl>
                          )}
                        </section>
                      ))
                    )}
                  </div>
                ) : null}

                {panel === "watch" ? (
                  <div className="oa-debug-watch">
                    <form
                      onSubmit={(event) => {
                        event.preventDefault();
                        const field = event.currentTarget.elements.namedItem("expression");
                        const expression =
                          field instanceof HTMLInputElement ? field.value.trim() : "";
                        if (
                          expression === "" ||
                          frame === null ||
                          !capabilityFor(session, "evaluate").supported
                        )
                          return;
                        void run(
                          {
                            _tag: "Evaluate",
                            ...fence(session),
                            expression,
                            frameRef: frame.frameRef,
                            actor: ownerActor,
                          },
                          "Evaluate watch",
                        );
                        event.currentTarget.reset();
                      }}
                    >
                      <Input
                        aria-label="Watch expression"
                        disabled={
                          frame === null ||
                          !capabilityFor(session, "evaluate").supported ||
                          pending !== null
                        }
                        maxLength={4_096}
                        name="expression"
                        placeholder="Expression"
                        required
                      />
                      <Button
                        disabled={
                          frame === null ||
                          !capabilityFor(session, "evaluate").supported ||
                          pending !== null
                        }
                        size="sm"
                        type="submit"
                        title={capabilityFor(session, "evaluate").reason}
                      >
                        Evaluate
                      </Button>
                    </form>
                    {session.watches.length === 0 ? (
                      <EmptyState
                        title="No watches"
                        detail="Evaluate an expression in the selected stopped frame. Values remain bounded and redacted by the host."
                      />
                    ) : (
                      <dl>
                        {session.watches.map((watch) => (
                          <div key={watch.watchRef}>
                            <dt>{watch.expression}</dt>
                            <dd>
                              <code>{watch.value}</code>
                              <DebugStatus
                                label={watch.state}
                                tone={
                                  watch.state === "ready"
                                    ? "success"
                                    : watch.state === "failed" || watch.state === "stale"
                                      ? "danger"
                                      : "neutral"
                                }
                              />
                              {watch.type === null ? null : <small>{watch.type}</small>}
                              {watch.redacted ? <small>redacted</small> : null}
                              {watch.truncated ? <small>truncated</small> : null}
                              {watch.message === null ? null : <small>{watch.message}</small>}
                            </dd>
                          </div>
                        ))}
                      </dl>
                    )}
                  </div>
                ) : null}

                {panel === "breakpoints" ? (
                  <div className="oa-debug-breakpoints">
                    {session.breakpoints.length === 0 ? (
                      <EmptyState
                        title="No breakpoints"
                        detail="Add source, function, or data breakpoints through an admitted editor command."
                      />
                    ) : (
                      <ul>
                        {session.breakpoints.map((breakpoint) => (
                          <li key={breakpoint.breakpointRef}>
                            <label>
                              <input
                                checked={breakpoint.enabled}
                                onChange={() =>
                                  void run(
                                    {
                                      _tag: "ReplaceBreakpoints",
                                      ...fence(session),
                                      breakpoints: session.breakpoints.map((candidate) =>
                                        candidate.breakpointRef === breakpoint.breakpointRef
                                          ? { ...candidate, enabled: !candidate.enabled }
                                          : candidate,
                                      ),
                                      actor: ownerActor,
                                    },
                                    `${breakpoint.enabled ? "Disable" : "Enable"} breakpoint`,
                                  )
                                }
                                type="checkbox"
                              />
                              <span>{breakpointLabel(breakpoint)}</span>
                            </label>
                            <DebugStatus
                              label={breakpoint.verified ? "Verified" : "Unverified"}
                              tone={breakpoint.verified ? "success" : "warning"}
                            />
                            <small>
                              {breakpoint._tag}
                              {breakpoint.condition === null
                                ? ""
                                : ` · condition ${breakpoint.condition}`}
                              {breakpoint.hitCondition === null
                                ? ""
                                : ` · hit ${breakpoint.hitCondition}`}
                              {breakpoint.logMessage === null
                                ? ""
                                : ` · log ${breakpoint.logMessage}`}
                            </small>
                            {breakpoint.message === null ? null : <p>{breakpoint.message}</p>}
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                ) : null}

                {panel === "console" ? (
                  <div className="oa-debug-console">
                    <header>
                      <span>
                        {session.console.length} entries · {session.retainedConsoleBytes} retained
                        bytes
                        {session.droppedConsoleBytes === 0
                          ? ""
                          : ` · ${session.droppedConsoleBytes} dropped bytes`}
                      </span>
                    </header>
                    {session.console.length === 0 ? (
                      <EmptyState
                        title="Console is empty"
                        detail="Adapter output and evaluated results appear here after host redaction and bounded retention."
                      />
                    ) : (
                      <ol aria-label="Debug console output">
                        {session.console.map((entry) => (
                          <li data-category={entry.category} key={entry.sequence}>
                            {entry.gapBefore ? (
                              <strong>Output gap before this entry. </strong>
                            ) : null}
                            <span>{entry.text}</span>
                            {entry.redacted ? <small>redacted</small> : null}
                            {entry.truncated ? <small>truncated</small> : null}
                          </li>
                        ))}
                      </ol>
                    )}
                  </div>
                ) : null}

                {panel === "modules" ? (
                  <div className="oa-debug-modules">
                    {!capabilityFor(session, "modules").supported ? (
                      <EmptyState
                        title="Modules unavailable"
                        detail={capabilityFor(session, "modules").reason}
                      />
                    ) : session.modules.length === 0 ? (
                      <EmptyState
                        title="No modules"
                        detail="The adapter supports modules but has not projected any for this generation."
                      />
                    ) : (
                      <table>
                        <caption>Modules for target generation {session.targetGeneration}</caption>
                        <thead>
                          <tr>
                            <th scope="col">Module</th>
                            <th scope="col">Version</th>
                            <th scope="col">Symbols</th>
                            <th scope="col">Path</th>
                          </tr>
                        </thead>
                        <tbody>
                          {session.modules.map((item) => (
                            <tr key={item.moduleRef}>
                              <th scope="row">{item.name}</th>
                              <td>{item.version ?? "Unknown"}</td>
                              <td>
                                <DebugStatus
                                  label={item.symbolStatus}
                                  tone={
                                    item.symbolStatus === "loaded"
                                      ? "success"
                                      : item.symbolStatus === "missing"
                                        ? "warning"
                                        : "neutral"
                                  }
                                />
                              </td>
                              <td>{item.pathRef ?? "Unavailable"}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                  </div>
                ) : null}

                {panel === "sources" ? (
                  <div className="oa-debug-sources">
                    {!capabilityFor(session, "loaded_sources").supported ? (
                      <EmptyState
                        title="Loaded sources unavailable"
                        detail={capabilityFor(session, "loaded_sources").reason}
                      />
                    ) : session.loadedSources.length === 0 ? (
                      <EmptyState
                        title="No loaded sources"
                        detail="The adapter supports loaded sources but has not projected any for this generation."
                      />
                    ) : (
                      <ul>
                        {session.loadedSources.map((source) => (
                          <li key={source.sourceRef}>
                            <button
                              disabled={source.availability !== "available" || pending !== null}
                              title={
                                source.availability === "available"
                                  ? `Open ${source.pathRef}`
                                  : `${sentence(source.availability)} source cannot be opened.`
                              }
                              onClick={() => navigate(source)}
                              type="button"
                            >
                              <strong>{source.label}</strong>
                              <small>{source.pathRef}</small>
                            </button>
                            <DebugStatus
                              label={`${source.origin} · ${source.availability}`}
                              tone={
                                source.availability === "available"
                                  ? "success"
                                  : source.availability === "loading"
                                    ? "neutral"
                                    : "warning"
                              }
                            />
                            <span>
                              {source.sourceMapRef === null ? "No source map" : "Source mapped"}
                              {source.documentGeneration === null
                                ? ""
                                : ` · document generation ${source.documentGeneration}`}
                            </span>
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                ) : null}

                {panel === "receipts" ? (
                  <div className="oa-debug-receipts">
                    {snapshot.receipts.length === 0 ? (
                      <EmptyState
                        title="No receipts"
                        detail="Validation, target controls, navigation, and cleanup write bounded receipts in the authority graph."
                      />
                    ) : (
                      <ol>
                        {snapshot.receipts.toReversed().map((receipt) => (
                          <li key={receipt.receiptRef}>
                            <strong>{sentence(receipt.operation)}</strong>
                            <DebugStatus
                              label={`${receipt.disposition} · ${receipt.outcome}`}
                              tone={
                                receipt.disposition === "failed" ||
                                receipt.disposition === "refused"
                                  ? "danger"
                                  : receipt.disposition === "canceled"
                                    ? "warning"
                                    : "success"
                              }
                            />
                            <span>{receipt.observedAt}</span>
                            <small>
                              {receipt.operationRef} · session generation{" "}
                              {receipt.sessionGeneration ?? "none"} · {receipt.targetRef} ·{" "}
                              {receipt.placementRef}
                            </small>
                          </li>
                        ))}
                      </ol>
                    )}
                    <div className="oa-debug-retention-controls">
                      {confirmDelete ? (
                        <div role="alert">
                          <p>
                            This removes retained console, watch, variable, and receipt data from
                            the debug graph. It does not terminate the target.
                          </p>
                          <Button
                            size="sm"
                            variant="destructive"
                            disabled={pending !== null}
                            onClick={() => {
                              setConfirmDelete(false);
                              void run(
                                {
                                  _tag: "DeleteRetainedData",
                                  operationRef: nextOperationRef(),
                                  reason: "The owner requested deletion of retained debug data.",
                                  actor: ownerActor,
                                },
                                "Delete retained debug data",
                              );
                            }}
                          >
                            Confirm deletion
                          </Button>
                          <Button size="sm" variant="ghost" onClick={() => setConfirmDelete(false)}>
                            Keep retained data
                          </Button>
                        </div>
                      ) : (
                        <Button
                          size="sm"
                          variant="destructive"
                          disabled={pending !== null}
                          onClick={() => setConfirmDelete(true)}
                        >
                          Delete retained data
                        </Button>
                      )}
                    </div>
                  </div>
                ) : null}
              </div>
            </section>
          </div>
        </>
      )}
    </div>
  );
};
