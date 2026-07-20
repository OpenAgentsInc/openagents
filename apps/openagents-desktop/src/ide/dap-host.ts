import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { Context, Effect, Exit, Layer, Schema, Scope, Stream } from "effect";

import {
  IdeDebugAdmissionFailure,
  IdeDebugCancellation,
  IdeDebugCommandSchema,
  IdeDebugCommandResultSchema,
  IdeDebugConfigurationRefSchema,
  IdeDebugConfigurationSchema,
  IdeDebugFrameRefSchema,
  IdeDebugModuleRefSchema,
  IdeDebugOperationRefSchema,
  IdeDebugProtocolFailure,
  IdeDebugStaleEvent,
  IdeDebugPersistenceSchema,
  IdeDebugSnapshotSchema,
  IdeDebugSourceRefSchema,
  IdeDebugThreadRefSchema,
  IdeDebugScopeRefSchema,
  IdeDebugVariableRefSchema,
  type IdeDebugAdapterEvent,
  type IdeDebugBreakpoint,
  type IdeDebugCapability,
  type IdeDebugCommand,
  type IdeDebugCommandResult,
  type IdeDebugConfiguration,
  type IdeDebugControlOperation,
  type IdeDebugEvent,
  type IdeDebugSession,
  type IdeDebugSnapshot,
  type IdeDebugSource,
  type IdeDebugThread,
  type IdeDebugFrame,
  type IdeDebugScope,
  type IdeDebugVariable,
  type IdeDebugModule,
} from "./debug-contract.ts";
import {
  IdeDebugService,
  makeIdeDebugServiceLayer,
  type IdeDebugServiceShape,
} from "./debug-service.ts";
import { openDapClient, type DapClient, type DapClientOptions } from "./dap-client.ts";
import type { DapEvent, DapResponse } from "./dap-transport.ts";
import { IdeLanguageGenerationSchema, IdeServiceGenerationSchema } from "./project-contract.ts";
import { ideRunBindingFor } from "./run-host.ts";
import type { IdeRunActor } from "./run-contract.ts";
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./portable-mutation-authority.ts";

const JsonObjectSchema = Schema.Record(Schema.String, Schema.Json);
const DapSourceSchema = Schema.Struct({
  name: Schema.optionalKey(Schema.String),
  path: Schema.optionalKey(Schema.String),
  sourceReference: Schema.optionalKey(Schema.Number),
});
const InitializeBodySchema = Schema.Struct({
  supportsConfigurationDoneRequest: Schema.optionalKey(Schema.Boolean),
  supportsConditionalBreakpoints: Schema.optionalKey(Schema.Boolean),
  supportsHitConditionalBreakpoints: Schema.optionalKey(Schema.Boolean),
  supportsLogPoints: Schema.optionalKey(Schema.Boolean),
  supportsFunctionBreakpoints: Schema.optionalKey(Schema.Boolean),
  supportsDataBreakpoints: Schema.optionalKey(Schema.Boolean),
  supportsSetVariable: Schema.optionalKey(Schema.Boolean),
  supportsEvaluateForHovers: Schema.optionalKey(Schema.Boolean),
  supportsStepBack: Schema.optionalKey(Schema.Boolean),
  supportsRestartFrame: Schema.optionalKey(Schema.Boolean),
  supportsRestartRequest: Schema.optionalKey(Schema.Boolean),
  supportsTerminateRequest: Schema.optionalKey(Schema.Boolean),
  supportsModulesRequest: Schema.optionalKey(Schema.Boolean),
  supportsLoadedSourcesRequest: Schema.optionalKey(Schema.Boolean),
});
const ThreadsBodySchema = Schema.Struct({
  threads: Schema.Array(Schema.Struct({ id: Schema.Number, name: Schema.String })),
});
const StackBodySchema = Schema.Struct({
  stackFrames: Schema.Array(
    Schema.Struct({
      id: Schema.Number,
      name: Schema.String,
      source: Schema.optionalKey(DapSourceSchema),
      line: Schema.Number,
      column: Schema.Number,
      endLine: Schema.optionalKey(Schema.Number),
      endColumn: Schema.optionalKey(Schema.Number),
      moduleId: Schema.optionalKey(Schema.Union([Schema.String, Schema.Number])),
    }),
  ),
});
const ScopesBodySchema = Schema.Struct({
  scopes: Schema.Array(
    Schema.Struct({
      name: Schema.String,
      variablesReference: Schema.Number,
      expensive: Schema.Boolean,
      namedVariables: Schema.optionalKey(Schema.Number),
      indexedVariables: Schema.optionalKey(Schema.Number),
    }),
  ),
});
const VariablesBodySchema = Schema.Struct({
  variables: Schema.Array(
    Schema.Struct({
      name: Schema.String,
      value: Schema.String,
      type: Schema.optionalKey(Schema.String),
      evaluateName: Schema.optionalKey(Schema.String),
      variablesReference: Schema.Number,
      namedVariables: Schema.optionalKey(Schema.Number),
      indexedVariables: Schema.optionalKey(Schema.Number),
    }),
  ),
});
const ModulesBodySchema = Schema.Struct({
  modules: Schema.Array(
    Schema.Struct({
      id: Schema.Union([Schema.String, Schema.Number]),
      name: Schema.String,
      path: Schema.optionalKey(Schema.String),
      version: Schema.optionalKey(Schema.String),
    }),
  ),
});
const SourcesBodySchema = Schema.Struct({ sources: Schema.Array(DapSourceSchema) });
const EvaluateBodySchema = Schema.Struct({
  result: Schema.String,
  type: Schema.optionalKey(Schema.String),
  variablesReference: Schema.Number,
});
const SetVariableBodySchema = Schema.Struct({
  value: Schema.String,
  type: Schema.optionalKey(Schema.String),
  variablesReference: Schema.optionalKey(Schema.Number),
});
const SourceBodySchema = Schema.Struct({
  content: Schema.String,
  mimeType: Schema.optionalKey(Schema.String),
});
const BreakpointsBodySchema = Schema.Struct({
  breakpoints: Schema.Array(
    Schema.Struct({
      verified: Schema.Boolean,
      message: Schema.optionalKey(Schema.String),
      line: Schema.optionalKey(Schema.Number),
    }),
  ),
});

export const IdeDapAdapterResolutionSchema = Schema.Struct({
  configurationRef: IdeDebugConfigurationRefSchema,
  configurationDigest: Schema.String.check(Schema.isMinLength(64), Schema.isMaxLength(64)),
  executable: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(4_096)),
  argv: Schema.Array(Schema.String.check(Schema.isMaxLength(16_384))).check(
    Schema.isMaxLength(256),
  ),
  cwd: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(4_096)),
  environment: Schema.Record(Schema.String, Schema.String.check(Schema.isMaxLength(65_536))),
  adapterId: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  startCommand: Schema.Literals(["launch", "attach"]),
  startArguments: JsonObjectSchema,
}).annotate({ identifier: "IdeDapAdapterResolution" });
export interface IdeDapAdapterResolution extends Schema.Schema.Type<
  typeof IdeDapAdapterResolutionSchema
> {}

export const IdeDapDiscoveredConfigurationSchema = Schema.Struct({
  configuration: IdeDebugConfigurationSchema,
  resolution: IdeDapAdapterResolutionSchema,
}).annotate({ identifier: "IdeDapDiscoveredConfiguration" });
export interface IdeDapDiscoveredConfiguration extends Schema.Schema.Type<
  typeof IdeDapDiscoveredConfigurationSchema
> {}

export type IdeDapWorkspaceBinding = Readonly<{ root: string; grantRef: string }>;
export type IdeDapDiscoveryInput = Readonly<{
  root: string;
  grantRef: string;
  binding: IdeDebugConfiguration["binding"];
}>;
export type IdeDapSourceResolverInput = Readonly<{
  root: string;
  binding: IdeDebugConfiguration["binding"];
  source: typeof DapSourceSchema.Type;
}>;

export type IdeDapHostOptions = Readonly<{
  workspace: () => IdeDapWorkspaceBinding | null;
  mutationAuthority: IdePortableMutationAuthority;
  discoverConfigurations: (
    input: IdeDapDiscoveryInput,
  ) =>
    | ReadonlyArray<IdeDapDiscoveredConfiguration>
    | Promise<ReadonlyArray<IdeDapDiscoveredConfiguration>>;
  resolveSource?: (input: IdeDapSourceResolverInput) => IdeDebugSource | null;
  runTask?: (taskRef: string, actor: IdeRunActor) => Promise<boolean>;
  emit?: (event: IdeDebugEvent) => void;
  now?: () => string;
  openClient?: (options: DapClientOptions) => DapClient;
  persistenceRoot?: string;
}>;

export type IdeDapHost = Readonly<{
  snapshot: () => Promise<IdeDebugSnapshot | null>;
  command: (value: unknown) => Promise<IdeDebugCommandResult | null>;
  pendingRequestCount: () => number;
  dispose: () => Promise<void>;
}>;

type Fence = Pick<
  IdeDebugSession,
  "sessionRef" | "sessionGeneration" | "adapterGeneration" | "targetGeneration"
>;
type VariableTarget = Readonly<{ variablesReference: number; name: string }>;
type MutableRuntime = {
  root: string;
  grantRef: string;
  scope: Scope.Scope;
  service: IdeDebugServiceShape;
  client: DapClient | null;
  session: IdeDebugSession | null;
  resolution: IdeDapAdapterResolution | null;
  secretValues: ReadonlyArray<string>;
  sourceByRef: Map<string, typeof DapSourceSchema.Type>;
  frameIdByRef: Map<string, number>;
  variableTargetByRef: Map<string, VariableTarget>;
  sourceKeys: Set<string>;
  threadIdByRef: Map<string, number>;
  configurations: Map<string, IdeDapDiscoveredConfiguration>;
  operations: Map<string, AbortController>;
  resolveSource: IdeDapHostOptions["resolveSource"];
  eventTail: Promise<void>;
  mutationAuthority: IdePortableMutationAuthority;
  permit: IdePortableMutationPermit | null;
  revocationScheduled: boolean;
};

const fullDigest = (value: string): string => createHash("sha256").update(value).digest("hex");
const digest = (value: string): string => fullDigest(value).slice(0, 32);
const configurationDigest = (configuration: IdeDebugConfiguration): string =>
  fullDigest(JSON.stringify(configuration));
const clean = (value: string, maximum: number): string => value.slice(0, maximum);
const idFor = (kind: string, value: string): string => `ide.${kind}.${digest(value)}`;
const internalOperationRef = () =>
  IdeDebugOperationRefSchema.make(`ide.debug-operation.internal-${randomUUID()}`);
const fenceOf = (session: IdeDebugSession): Fence => ({
  sessionRef: session.sessionRef,
  sessionGeneration: session.sessionGeneration,
  adapterGeneration: session.adapterGeneration,
  targetGeneration: session.targetGeneration,
});

const assertPortablePermit = (runtime: MutableRuntime, operation: string): void => {
  if (runtime.permit === null || !runtime.mutationAuthority.reauthorize(runtime.permit)) {
    throw new IdeDebugStaleEvent({
      operation,
      detail: "The portable workspace attachment changed before the DAP operation completed.",
    });
  }
};

export const ideDebugBindingFor = (workspace: IdeDapWorkspaceBinding) => {
  const run = ideRunBindingFor({
    root: path.resolve(workspace.root),
    grantRef: workspace.grantRef,
  });
  return {
    projectRef: run.projectRef,
    rootRef: run.rootRef,
    worktreeRef: run.worktreeRef,
    attachmentGeneration: run.attachmentGeneration,
    languageGeneration: IdeLanguageGenerationSchema.make(1),
    placementGeneration: run.placementGeneration,
    serviceGeneration: IdeServiceGenerationSchema.make(1),
    placementRef: run.placementRef,
    language: "typescript",
  };
};

const emptySnapshot = (workspace: IdeDapWorkspaceBinding): IdeDebugSnapshot =>
  IdeDebugSnapshotSchema.make({
    schemaVersion: "openagents.desktop.ide-debug.v1",
    binding: ideDebugBindingFor(workspace),
    capabilityState: { _tag: "Unconfigured" },
    configurations: [],
    breakpointSets: [],
    sessions: [],
    receipts: [],
    stopped: false,
  });

const persistenceFile = (root: string, projectRef: string): string =>
  path.join(root, `${projectRef.replaceAll(/[^A-Za-z0-9._-]/gu, "_")}.json`);

const decodeBody = <S extends Schema.ConstraintDecoder<unknown, never>>(
  schema: S,
  response: DapResponse,
  operation: string,
): S["Type"] => {
  const decoded = Schema.decodeUnknownExit(schema)(response.body);
  if (Exit.isFailure(decoded))
    throw new IdeDebugProtocolFailure({
      operation,
      detail: `DAP ${operation} returned an invalid body.`,
    });
  return decoded.value;
};

const capabilitiesOf = (
  body: typeof InitializeBodySchema.Type,
): ReadonlyArray<IdeDebugCapability> => {
  const value = (
    capability: IdeDebugCapability["capability"],
    supported: boolean,
  ): IdeDebugCapability => ({
    capability,
    supported,
    reason: supported ? null : "The adapter did not report this capability.",
  });
  return [
    value("configuration_done", body.supportsConfigurationDoneRequest === true),
    value("conditional_breakpoints", body.supportsConditionalBreakpoints === true),
    value("hit_conditional_breakpoints", body.supportsHitConditionalBreakpoints === true),
    value("log_points", body.supportsLogPoints === true),
    value("function_breakpoints", body.supportsFunctionBreakpoints === true),
    value("data_breakpoints", body.supportsDataBreakpoints === true),
    value("set_variable", body.supportsSetVariable === true),
    value("evaluate", body.supportsEvaluateForHovers === true),
    value("pause", true),
    value("step_in", true),
    value("step_over", true),
    value("step_out", true),
    value("step_back", body.supportsStepBack === true),
    value("restart_frame", body.supportsRestartFrame === true),
    value("restart_session", body.supportsRestartRequest === true),
    value("continue", true),
    value("disconnect", true),
    value("terminate", body.supportsTerminateRequest === true),
    value("modules", body.supportsModulesRequest === true),
    value("loaded_sources", body.supportsLoadedSourcesRequest === true),
    value("source_request", true),
    value("cancel_request", true),
    value("run_to_cursor", false),
  ];
};

const redactWith = (
  runtime: MutableRuntime,
  source: string,
): Readonly<{ text: string; redacted: boolean }> => {
  let text = source;
  for (const value of runtime.secretValues)
    if (value.length > 0) text = text.replaceAll(value, "[REDACTED]");
  const next = text.replace(
    /\b(token|password|authorization|secret|cookie)\s*[:=]\s*([^\s,;]+)/giu,
    "$1=[REDACTED]",
  );
  return { text: next, redacted: next !== source };
};

const sourceOf = (runtime: MutableRuntime, source: typeof DapSourceSchema.Type): IdeDebugSource => {
  const key = `${source.path ?? ""}\0${source.sourceReference ?? 0}\0${source.name ?? ""}`;
  const resolved =
    runtime.resolveSource?.({
      root: runtime.root,
      binding: ideDebugBindingFor({ root: runtime.root, grantRef: runtime.grantRef }),
      source,
    }) ?? null;
  if (resolved !== null) {
    runtime.sourceByRef.set(resolved.sourceRef, source);
    return resolved;
  }
  const sourceRef = IdeDebugSourceRefSchema.make(idFor("debug-source", key));
  runtime.sourceByRef.set(sourceRef, source);
  const rawPath =
    source.path ??
    (source.sourceReference === undefined
      ? (source.name ?? "unavailable")
      : `dap-source:${source.sourceReference}`);
  const rawLabel = source.name ?? (path.basename(rawPath) || "DAP source");
  return {
    sourceRef,
    fileRef: null,
    documentRef: null,
    documentGeneration: null,
    pathRef: clean(redactWith(runtime, rawPath).text, 512),
    label: clean(redactWith(runtime, rawLabel).text, 240),
    origin: "adapter",
    availability:
      source.path !== undefined || (source.sourceReference ?? 0) > 0 ? "available" : "unavailable",
    sourceMapRef: null,
  };
};

const applyEvent = async (
  runtime: MutableRuntime,
  fence: Fence,
  event: IdeDebugAdapterEvent,
): Promise<void> => {
  assertPortablePermit(runtime, "IdeDap.adapterEvent");
  await Effect.runPromise(runtime.service.applyAdapterEvent({ ...fence, event }));
  assertPortablePermit(runtime, "IdeDap.adapterEvent.result");
};

const requestWithPermit = async (
  runtime: MutableRuntime,
  client: DapClient,
  ...request: Parameters<DapClient["request"]>
): Promise<DapResponse> => {
  assertPortablePermit(runtime, `IdeDap.request.${request[0]}`);
  const response = await client.request(...request);
  assertPortablePermit(runtime, `IdeDap.request.${request[0]}.result`);
  return response;
};

const refreshGraph = async (runtime: MutableRuntime, fence: Fence): Promise<void> => {
  const client = runtime.client;
  if (client === null || client.isExited()) return;
  const threadsBody = decodeBody(
    ThreadsBodySchema,
    await requestWithPermit(runtime, client, "threads"),
    "threads",
  );
  const threads: Array<IdeDebugThread> = [];
  const frames: Array<IdeDebugFrame> = [];
  const scopes: Array<IdeDebugScope> = [];
  const variables: Array<IdeDebugVariable> = [];
  const modules: Array<IdeDebugModule> = [];
  const loadedSources: Array<IdeDebugSource> = [];
  runtime.frameIdByRef.clear();
  runtime.threadIdByRef.clear();
  runtime.variableTargetByRef.clear();

  const loadVariables = async (
    reference: number,
    scopeRef: typeof IdeDebugScopeRefSchema.Type,
  ): Promise<void> => {
    if (reference <= 0 || variables.length >= 50_000) return;
    const body = decodeBody(
      VariablesBodySchema,
      await requestWithPermit(runtime, client, "variables", { variablesReference: reference }),
      "variables",
    );
    for (const [index, candidate] of body.variables.entries()) {
      const variableRef = IdeDebugVariableRefSchema.make(
        idFor("debug-variable", `${fence.sessionRef}:${reference}:${index}:${candidate.name}`),
      );
      const secured = redactWith(runtime, candidate.value);
      variables.push({
        variableRef,
        parentRef: null,
        scopeRef,
        name: clean(redactWith(runtime, candidate.name).text, 500),
        value: clean(secured.text, 16_384),
        type:
          candidate.type === undefined
            ? null
            : clean(redactWith(runtime, candidate.type).text, 500),
        evaluateName:
          candidate.evaluateName === undefined
            ? null
            : clean(redactWith(runtime, candidate.evaluateName).text, 1_000),
        childCount: candidate.namedVariables ?? candidate.indexedVariables ?? null,
        redacted: secured.redacted,
        truncated: candidate.value.length > 16_384,
      });
      runtime.variableTargetByRef.set(variableRef, {
        variablesReference: reference,
        name: candidate.name,
      });
    }
  };

  for (const thread of threadsBody.threads) {
    const threadRef = IdeDebugThreadRefSchema.make(
      idFor("debug-thread", `${fence.sessionRef}:${thread.id}`),
    );
    runtime.threadIdByRef.set(threadRef, thread.id);
    threads.push({
      threadRef,
      name: clean(redactWith(runtime, thread.name).text, 240),
      state: "stopped",
      stopReason: null,
    });
    const stackBody = decodeBody(
      StackBodySchema,
      await requestWithPermit(runtime, client, "stackTrace", { threadId: thread.id }),
      "stackTrace",
    );
    for (const frame of stackBody.stackFrames) {
      const frameRef = IdeDebugFrameRefSchema.make(
        idFor("debug-frame", `${fence.sessionRef}:${frame.id}`),
      );
      runtime.frameIdByRef.set(frameRef, frame.id);
      const dapSource = frame.source;
      const source = dapSource === undefined ? null : sourceOf(runtime, dapSource);
      frames.push({
        frameRef,
        threadRef,
        name: clean(redactWith(runtime, frame.name).text, 500),
        location:
          source === null
            ? null
            : {
                source,
                line: Math.max(1, frame.line),
                column: Math.max(1, frame.column),
                endLine: frame.endLine === undefined ? null : Math.max(1, frame.endLine),
                endColumn: frame.endColumn === undefined ? null : Math.max(1, frame.endColumn),
              },
        moduleRef:
          frame.moduleId === undefined
            ? null
            : IdeDebugModuleRefSchema.make(idFor("debug-module", String(frame.moduleId))),
        canRestart: true,
      });
      const scopeBody = decodeBody(
        ScopesBodySchema,
        await requestWithPermit(runtime, client, "scopes", { frameId: frame.id }),
        "scopes",
      );
      for (const [scopeIndex, scope] of scopeBody.scopes.entries()) {
        const scopeRef = IdeDebugScopeRefSchema.make(
          idFor("debug-scope", `${frameRef}:${scopeIndex}:${scope.variablesReference}`),
        );
        scopes.push({
          scopeRef,
          frameRef,
          name: clean(redactWith(runtime, scope.name).text, 240),
          expensive: scope.expensive,
          variableCount: scope.namedVariables ?? scope.indexedVariables ?? null,
          state: "ready",
        });
        await loadVariables(scope.variablesReference, scopeRef);
      }
    }
  }

  const session = runtime.session;
  const supports = (name: string) =>
    session?.configuration.adapter.capabilities.some(
      (entry) => entry.capability === name && entry.supported,
    ) === true;
  if (supports("modules")) {
    const body = decodeBody(
      ModulesBodySchema,
      await requestWithPermit(runtime, client, "modules", { startModule: 0, moduleCount: 10_000 }),
      "modules",
    );
    for (const module of body.modules)
      modules.push({
        moduleRef: IdeDebugModuleRefSchema.make(idFor("debug-module", String(module.id))),
        name: clean(redactWith(runtime, module.name).text, 500),
        pathRef:
          module.path === undefined ? null : clean(redactWith(runtime, module.path).text, 512),
        version:
          module.version === undefined
            ? null
            : clean(redactWith(runtime, module.version).text, 500),
        symbolStatus: "unavailable",
      });
  }
  if (supports("loaded_sources")) {
    const body = decodeBody(
      SourcesBodySchema,
      await requestWithPermit(runtime, client, "loadedSources"),
      "loadedSources",
    );
    for (const source of body.sources) loadedSources.push(sourceOf(runtime, source));
  }
  await applyEvent(runtime, fence, {
    _tag: "Projection",
    threads,
    frames,
    scopes,
    variables,
    modules,
    loadedSources,
  });
};

const responsePayload = (response: DapResponse): typeof Schema.Json.Type | null =>
  response.body ?? null;

const commandName = (operation: IdeDebugControlOperation): string =>
  ({
    continue: "continue",
    pause: "pause",
    step_in: "stepIn",
    step_over: "next",
    step_out: "stepOut",
    step_back: "stepBack",
    run_to_cursor: "goto",
    restart_frame: "restartFrame",
    restart_session: "restart",
    disconnect: "disconnect",
    terminate: "terminate",
  })[operation];

export const openIdeDapHost = async (options: IdeDapHostOptions): Promise<IdeDapHost> => {
  let runtime: MutableRuntime | null = null;
  let disposed = false;
  let commandTail = Promise.resolve();

  const loadSeed = async (binding: IdeDapWorkspaceBinding): Promise<IdeDebugSnapshot> => {
    const seed = emptySnapshot(binding);
    if (options.persistenceRoot === undefined) return seed;
    try {
      const decoded = Schema.decodeUnknownSync(IdeDebugPersistenceSchema)(
        JSON.parse(
          await readFile(persistenceFile(options.persistenceRoot, seed.binding.projectRef), "utf8"),
        ),
      );
      if (
        decoded.projectRef !== seed.binding.projectRef ||
        decoded.rootRef !== seed.binding.rootRef ||
        decoded.worktreeRef !== seed.binding.worktreeRef
      )
        return seed;
      return IdeDebugSnapshotSchema.make({
        ...seed,
        configurations: decoded.configurations,
        breakpointSets: decoded.breakpointSets,
      });
    } catch {
      return seed;
    }
  };

  const persist = async (current: MutableRuntime): Promise<void> => {
    if (options.persistenceRoot === undefined) return;
    const snapshot = await Effect.runPromise(current.service.snapshot);
    const target = persistenceFile(options.persistenceRoot, snapshot.binding.projectRef);
    const temporary = `${target}.${process.pid}.${randomUUID()}.tmp`;
    const value = IdeDebugPersistenceSchema.make({
      schemaVersion: "openagents.desktop.ide-debug-persistence.v1",
      projectRef: snapshot.binding.projectRef,
      rootRef: snapshot.binding.rootRef,
      worktreeRef: snapshot.binding.worktreeRef,
      configurations: snapshot.configurations,
      breakpointSets: snapshot.breakpointSets,
      updatedAt: (options.now ?? (() => new Date().toISOString()))(),
    });
    await mkdir(options.persistenceRoot, { recursive: true, mode: 0o700 });
    try {
      await writeFile(temporary, `${JSON.stringify(value)}\n`, {
        encoding: "utf8",
        mode: 0o600,
        flag: "wx",
      });
      await rename(temporary, target);
    } catch (cause) {
      await rm(temporary, { force: true }).catch(() => undefined);
      throw cause;
    }
  };

  const closeRuntime = async (
    reason: string,
    operationRef = internalOperationRef(),
    actor: IdeDebugCommand["actor"] = {
      _tag: "Agent",
      actorRef: "ide.dap-host",
      turnRef: "ide.dap-host.cleanup",
    },
  ): Promise<void> => {
    const current = runtime;
    runtime = null;
    if (current === null) return;
    await current.eventTail.catch(() => undefined);
    if (current.client !== null) await current.client.dispose(reason).catch(() => undefined);
    for (const controller of current.operations.values()) controller.abort(reason);
    await Effect.runPromise(current.service.cleanup(operationRef, reason, actor)).catch(
      () => undefined,
    );
    await Effect.runPromise(Scope.close(current.scope, Exit.void)).catch(() => undefined);
  };

  const portablePermitIsLive = (current: MutableRuntime): boolean =>
    current.permit !== null && options.mutationAuthority.reauthorize(current.permit);

  const schedulePortableRevocation = (current: MutableRuntime): void => {
    if (current.revocationScheduled) return;
    current.revocationScheduled = true;
    queueMicrotask(() => {
      if (runtime === current) void closeRuntime("portable workspace attachment changed");
    });
  };

  const ensureRuntime = async (): Promise<MutableRuntime | null> => {
    if (disposed) return null;
    const workspace = options.workspace();
    if (workspace === null) {
      await closeRuntime("workspace unavailable");
      return null;
    }
    const root = path.resolve(workspace.root);
    if (runtime !== null && runtime.root === root && runtime.grantRef === workspace.grantRef)
      return runtime;
    await closeRuntime("workspace authority changed");
    const binding = { root, grantRef: workspace.grantRef };
    const scope = await Effect.runPromise(Scope.make());
    const context = await Effect.runPromise(
      Layer.buildWithScope(
        makeIdeDebugServiceLayer(await loadSeed(binding), { now: options.now }),
        scope,
      ),
    );
    const service = Context.get(context, IdeDebugService);
    const opened: MutableRuntime = {
      root,
      grantRef: workspace.grantRef,
      scope,
      service,
      client: null,
      session: null,
      resolution: null,
      secretValues: [],
      sourceByRef: new Map(),
      frameIdByRef: new Map(),
      variableTargetByRef: new Map(),
      sourceKeys: new Set(),
      threadIdByRef: new Map(),
      configurations: new Map(),
      operations: new Map(),
      resolveSource: options.resolveSource,
      eventTail: Promise.resolve(),
      mutationAuthority: options.mutationAuthority,
      permit: null,
      revocationScheduled: false,
    };
    runtime = opened;
    if (options.emit !== undefined)
      await Effect.runPromise(
        service.events.pipe(
          Stream.runForEach((event) => Effect.sync(() => options.emit?.(event))),
          Effect.forkIn(scope),
        ),
      );
    return opened;
  };

  const snapshot = async (): Promise<IdeDebugSnapshot | null> => {
    let current = await ensureRuntime();
    if (current !== null && current.client !== null && !portablePermitIsLive(current)) {
      await closeRuntime("portable workspace attachment changed");
      current = await ensureRuntime();
    }
    return current === null ? null : Effect.runPromise(current.service.snapshot);
  };

  const discover = async (
    current: MutableRuntime,
  ): Promise<ReadonlyArray<IdeDebugConfiguration>> => {
    const binding = ideDebugBindingFor({ root: current.root, grantRef: current.grantRef });
    const discovered = Schema.decodeUnknownSync(Schema.Array(IdeDapDiscoveredConfigurationSchema))(
      await options.discoverConfigurations({
        root: current.root,
        grantRef: current.grantRef,
        binding,
      }),
    );
    const registry = new Map<string, IdeDapDiscoveredConfiguration>();
    for (const entry of discovered) {
      if (JSON.stringify(entry.configuration.binding) !== JSON.stringify(binding)) {
        throw new IdeDebugAdmissionFailure({
          operation: "IdeDap.discover",
          detail: `Configuration ${entry.configuration.configurationRef} does not use the active project binding.`,
        });
      }
      if (
        entry.resolution.configurationRef !== entry.configuration.configurationRef ||
        entry.resolution.configurationDigest !== configurationDigest(entry.configuration)
      ) {
        throw new IdeDebugAdmissionFailure({
          operation: "IdeDap.discover",
          detail: `Configuration ${entry.configuration.configurationRef} does not match its main-process adapter resolution.`,
        });
      }
      registry.set(entry.configuration.configurationRef, entry);
    }
    current.configurations = registry;
    const configurations = [...registry.values()].map((entry) => entry.configuration);
    await Effect.runPromise(current.service.replaceConfigurations(configurations));
    await persist(current);
    return configurations;
  };

  const admittedEntry = async (
    current: MutableRuntime,
    configurationRef: IdeDebugConfiguration["configurationRef"],
  ): Promise<IdeDapDiscoveredConfiguration> => {
    if (current.configurations.size === 0) await discover(current);
    const entry = current.configurations.get(configurationRef);
    if (entry === undefined) {
      throw new IdeDebugAdmissionFailure({
        operation: "IdeDap.configuration",
        detail: `Configuration ${configurationRef} is not in the active main-process discovery registry.`,
      });
    }
    return entry;
  };

  const refused = async (
    current: MutableRuntime | null,
    reason: "invalid_input" | "not_admitted" | "stale_generation" | "protocol" | "unavailable",
    message: string,
  ): Promise<IdeDebugCommandResult> =>
    IdeDebugCommandResultSchema.cases.Refused.make({
      snapshot:
        current === null
          ? null
          : await Effect.runPromise(current.service.snapshot).catch(() => null),
      reason,
      message: clean(message || "The DAP operation was refused.", 1_024),
    });

  const handleEvent = async (
    current: MutableRuntime,
    fence: Fence,
    event: DapEvent,
  ): Promise<void> => {
    const body =
      event.body !== undefined &&
      typeof event.body === "object" &&
      event.body !== null &&
      !Array.isArray(event.body)
        ? (event.body as Record<string, unknown>)
        : {};
    switch (event.event) {
      case "stopped": {
        const threadId = typeof body.threadId === "number" ? body.threadId : null;
        await applyEvent(current, fence, {
          _tag: "Stopped",
          reason: clean(
            redactWith(
              current,
              typeof body.description === "string"
                ? body.description
                : String(body.reason ?? "stopped"),
            ).text,
            500,
          ),
          threadRef:
            threadId === null
              ? null
              : IdeDebugThreadRefSchema.make(
                  idFor("debug-thread", `${fence.sessionRef}:${threadId}`),
                ),
          allThreadsStopped: body.allThreadsStopped === true,
        });
        await refreshGraph(current, fence);
        break;
      }
      case "continued": {
        const threadId = typeof body.threadId === "number" ? body.threadId : null;
        await applyEvent(current, fence, {
          _tag: "Continued",
          threadRef:
            threadId === null
              ? null
              : IdeDebugThreadRefSchema.make(
                  idFor("debug-thread", `${fence.sessionRef}:${threadId}`),
                ),
          allThreadsContinued: body.allThreadsContinued === true,
        });
        break;
      }
      case "output":
        await applyEvent(current, fence, {
          _tag: "Output",
          category: ["console", "stdout", "stderr", "important"].includes(String(body.category))
            ? (body.category as "console")
            : "console",
          text: clean(redactWith(current, String(body.output ?? "")).text, 262_144),
        });
        break;
      case "invalidated":
        await applyEvent(current, fence, {
          _tag: "Invalidated",
          areas: ["threads", "stacks", "scopes", "variables", "watches", "modules", "sources"],
        });
        await refreshGraph(current, fence);
        break;
      case "terminated":
        await applyEvent(current, fence, {
          _tag: "Terminated",
          reason: "The debug adapter reported termination.",
        });
        break;
      case "exited":
        await applyEvent(current, fence, {
          _tag: "TargetLost",
          reason: "The debug target exited.",
        });
        break;
    }
  };

  const start = async (
    current: MutableRuntime,
    command: Extract<IdeDebugCommand, { _tag: "Start" }>,
  ): Promise<typeof Schema.Json.Type | null> => {
    if (current.client !== null && !current.client.isExited())
      throw new Error("A DAP session is already active.");
    const authorization = options.mutationAuthority.authorize(current.grantRef);
    if (authorization._tag === "Refused") {
      throw new IdeDebugStaleEvent({
        operation: "IdeDap.start",
        detail: "The current portable workspace attachment cannot start a debug adapter.",
      });
    }
    current.permit = authorization.permit;
    assertPortablePermit(current, "IdeDap.start");
    const admitted = await admittedEntry(current, command.configurationRef);
    current.sourceByRef.clear();
    current.sourceKeys.clear();
    current.frameIdByRef.clear();
    current.variableTargetByRef.clear();
    current.threadIdByRef.clear();
    const prelaunchTaskRef =
      admitted.configuration.intent._tag === "Launch"
        ? admitted.configuration.intent.prelaunchTaskRef
        : null;
    if (prelaunchTaskRef !== null) {
      assertPortablePermit(current, "IdeDap.prelaunch");
      if (
        options.runTask === undefined ||
        !(await options.runTask(prelaunchTaskRef, command.actor))
      ) {
        throw new IdeDebugAdmissionFailure({
          operation: "IdeDap.prelaunch",
          detail: `Prelaunch task ${prelaunchTaskRef} did not complete successfully.`,
        });
      }
      assertPortablePermit(current, "IdeDap.prelaunch.result");
    }
    current.secretValues = Object.values(admitted.resolution.environment).filter(
      (value) => value.length >= 4,
    );
    await Effect.runPromise(current.service.registerSecretValues(current.secretValues));
    const session = await Effect.runPromise(
      current.service.start({
        operationRef: command.operationRef,
        configuration: admitted.configuration,
        actor: command.actor,
      }),
    );
    assertPortablePermit(current, "IdeDap.start.service-result");
    return activateSession(
      current,
      session,
      admitted.resolution,
      command.operationRef,
      command.actor,
    );
  };

  const replaceBreakpoints = async (
    current: MutableRuntime,
    command: Extract<IdeDebugCommand, { _tag: "ReplaceBreakpoints" }>,
  ): Promise<void> => {
    const client = current.client;
    if (client === null) throw new Error("No DAP adapter is active.");
    await Effect.runPromise(current.service.preflightBreakpoints(command));
    const requestOptions = { signal: current.operations.get(command.operationRef)?.signal };
    const next: IdeDebugBreakpoint[] = [];
    const sources = new Map<string, Extract<IdeDebugBreakpoint, { _tag: "Source" }>[]>();
    for (const breakpoint of command.breakpoints) {
      if (breakpoint._tag === "Source") {
        const key = breakpoint.location.source.sourceRef;
        sources.set(key, [...(sources.get(key) ?? []), breakpoint]);
      }
    }
    for (const key of new Set([...current.sourceKeys, ...sources.keys()])) {
      const entries = sources.get(key) ?? [];
      const dapSource = current.sourceByRef.get(key);
      const fallback = entries[0]?.location.source;
      const source = dapSource ?? { path: fallback?.pathRef };
      const response = decodeBody(
        BreakpointsBodySchema,
        await requestWithPermit(
          current,
          client,
          "setBreakpoints",
          {
            source,
            breakpoints: entries.map((entry) => ({
              line: entry.requestedLine,
              column: entry.location.column,
              ...(entry.condition === null ? {} : { condition: entry.condition }),
              ...(entry.hitCondition === null ? {} : { hitCondition: entry.hitCondition }),
              ...(entry.logMessage === null ? {} : { logMessage: entry.logMessage }),
            })),
          },
          requestOptions,
        ),
        "setBreakpoints",
      );
      for (const [index, entry] of entries.entries()) {
        const result = response.breakpoints[index];
        next.push({
          ...entry,
          verified: result?.verified ?? false,
          message: result?.message ?? null,
        });
      }
    }
    current.sourceKeys = new Set(sources.keys());
    const functions = command.breakpoints.filter(
      (entry): entry is Extract<IdeDebugBreakpoint, { _tag: "Function" }> =>
        entry._tag === "Function",
    );
    if (functions.length > 0) {
      const body = decodeBody(
        BreakpointsBodySchema,
        await requestWithPermit(
          current,
          client,
          "setFunctionBreakpoints",
          {
            breakpoints: functions.map((entry) => ({
              name: entry.functionName,
              ...(entry.condition === null ? {} : { condition: entry.condition }),
              ...(entry.hitCondition === null ? {} : { hitCondition: entry.hitCondition }),
            })),
          },
          requestOptions,
        ),
        "setFunctionBreakpoints",
      );
      next.push(
        ...functions.map((entry, index) => ({
          ...entry,
          verified: body.breakpoints[index]?.verified ?? false,
          message: body.breakpoints[index]?.message ?? null,
        })),
      );
    }
    const data = command.breakpoints.filter(
      (entry): entry is Extract<IdeDebugBreakpoint, { _tag: "Data" }> => entry._tag === "Data",
    );
    if (data.length > 0) {
      const body = decodeBody(
        BreakpointsBodySchema,
        await requestWithPermit(
          current,
          client,
          "setDataBreakpoints",
          {
            breakpoints: data.map((entry) => ({
              dataId: entry.dataId,
              accessType: entry.accessType === "read_write" ? "readWrite" : entry.accessType,
            })),
          },
          requestOptions,
        ),
        "setDataBreakpoints",
      );
      next.push(
        ...data.map((entry, index) => ({
          ...entry,
          verified: body.breakpoints[index]?.verified ?? false,
          message: body.breakpoints[index]?.message ?? null,
        })),
      );
    }
    current.session = await Effect.runPromise(
      current.service.replaceBreakpoints({ ...command, breakpoints: next }),
    );
    await persist(current);
  };

  const activateSession = async (
    current: MutableRuntime,
    startingSession: IdeDebugSession,
    resolution: IdeDapAdapterResolution,
    operationRef: typeof IdeDebugOperationRefSchema.Type,
    actor: IdeRunActor,
  ): Promise<typeof Schema.Json.Type> => {
    const expectedCommand =
      startingSession.configuration.intent._tag === "Launch" ? "launch" : "attach";
    if (resolution.startCommand !== expectedCommand)
      throw new IdeDebugAdmissionFailure({
        operation: "IdeDap.activateSession",
        detail: "The resolved DAP start command does not match the admitted intent.",
      });
    current.session = startingSession;
    current.resolution = resolution;
    const immutableFence = fenceOf(startingSession);
    let signalInitialized: () => void = () => undefined;
    const initializedEvent = new Promise<void>((resolve) => {
      signalInitialized = resolve;
    });
    assertPortablePermit(current, "IdeDap.adapter.spawn");
    const client = (options.openClient ?? openDapClient)({
      launch: {
        executable: resolution.executable,
        argv: resolution.argv,
        cwd: resolution.cwd,
        environment: resolution.environment,
        timeoutMs: startingSession.configuration.timeoutMs,
      },
      onEvent: (event) => {
        if (!portablePermitIsLive(current)) {
          schedulePortableRevocation(current);
          return;
        }
        if (event.event === "initialized") signalInitialized();
        const handled = handleEvent(current, immutableFence, event);
        current.eventTail = handled.catch(() => undefined);
        return handled;
      },
      onExit: (exit) => {
        if (!portablePermitIsLive(current)) {
          schedulePortableRevocation(current);
          return;
        }
        if (exit.stderr === "") return;
        return applyEvent(current, immutableFence, {
          _tag: "AdapterFailed",
          reason: clean(redactWith(current, exit.stderr).text || "The DAP adapter exited.", 500),
        });
      },
    });
    current.client = client;
    if (!portablePermitIsLive(current)) {
      await client.dispose("portable workspace attachment changed").catch(() => undefined);
      current.client = null;
      throw new IdeDebugStaleEvent({
        operation: "IdeDap.adapter.spawn",
        detail: "The portable workspace attachment changed while the debug adapter started.",
      });
    }
    const signal = current.operations.get(operationRef)?.signal;
    const requestOptions = signal === undefined ? undefined : { signal };
    const initialized = decodeBody(
      InitializeBodySchema,
      await requestWithPermit(
        current,
        client,
        "initialize",
        {
          clientID: "openagents",
          clientName: "OpenAgents",
          adapterID: resolution.adapterId,
          pathFormat: "path",
          linesStartAt1: true,
          columnsStartAt1: true,
        },
        requestOptions,
      ),
      "initialize",
    );
    const capabilities = capabilitiesOf(initialized);
    let session = await Effect.runPromise(
      current.service.applyAdapterEvent({
        ...immutableFence,
        event: { _tag: "Initialized", capabilities },
      }),
    );
    current.session = session;
    const startOutcome = requestWithPermit(
      current,
      client,
      resolution.startCommand,
      resolution.startArguments,
      requestOptions,
    ).then(
      (response) => ({ _tag: "Succeeded" as const, response }),
      (error: unknown) => ({ _tag: "Failed" as const, error }),
    );
    let timeout: ReturnType<typeof setTimeout> | undefined;
    try {
      await Promise.race([
        initializedEvent,
        new Promise<never>((_resolve, reject) => {
          timeout = setTimeout(
            () =>
              reject(
                new IdeDebugProtocolFailure({
                  operation: "IdeDap.activateSession",
                  detail: "The adapter did not emit initialized before the configuration timeout.",
                }),
              ),
            Math.min(startingSession.configuration.timeoutMs, 10_000),
          );
          timeout.unref?.();
        }),
      ]);
    } finally {
      if (timeout !== undefined) clearTimeout(timeout);
    }
    if (session.breakpoints.length > 0) {
      await replaceBreakpoints(current, {
        _tag: "ReplaceBreakpoints",
        ...immutableFence,
        operationRef,
        breakpoints: session.breakpoints,
        actor,
      });
      session = current.session ?? session;
    }
    if (
      capabilities.some((entry) => entry.capability === "configuration_done" && entry.supported)
    ) {
      await requestWithPermit(current, client, "configurationDone", undefined, requestOptions);
    }
    const started = await startOutcome;
    if (started._tag === "Failed") throw started.error;
    await client.drainEvents();
    assertPortablePermit(current, "IdeDap.adapter.events-drained");
    await current.eventTail;
    await refreshGraph(current, immutableFence);
    return { sessionRef: session.sessionRef, pid: client.pid };
  };

  const runCommand = async (
    current: MutableRuntime,
    command: IdeDebugCommand,
  ): Promise<typeof Schema.Json.Type | null> => {
    if (
      command._tag !== "Discover" &&
      command._tag !== "Validate" &&
      command._tag !== "Start" &&
      command._tag !== "Cancel" &&
      command._tag !== "DeleteRetainedData" &&
      command._tag !== "Cleanup"
    ) {
      assertPortablePermit(current, `IdeDap.${command._tag}`);
    }
    switch (command._tag) {
      case "Discover": {
        const configurations = await discover(current);
        return {
          configurationRefs: configurations.map((configuration) => configuration.configurationRef),
        };
      }
      case "Validate": {
        const entry = await admittedEntry(current, command.configurationRef);
        await Effect.runPromise(
          current.service.validate(entry.configuration, command.actor, command.operationRef),
        );
        return null;
      }
      case "Start":
        return start(current, command);
      case "ReplaceBreakpoints":
        await replaceBreakpoints(current, command);
        return null;
      case "Control": {
        const client = current.client;
        if (client === null) throw new Error("No DAP adapter is active.");
        await Effect.runPromise(current.service.preflightControl(command));
        if (command.operation === "restart_session") {
          const resolution = current.resolution;
          if (resolution === null)
            throw new Error("The active DAP adapter resolution is unavailable.");
          const changed = await Effect.runPromise(current.service.control(command));
          current.session = changed;
          await client.dispose("The debug session is restarting with a new adapter generation.");
          current.client = null;
          current.sourceByRef.clear();
          current.sourceKeys.clear();
          current.threadIdByRef.clear();
          current.frameIdByRef.clear();
          current.variableTargetByRef.clear();
          return activateSession(current, changed, resolution, command.operationRef, command.actor);
        }
        const terminalOperation =
          command.operation === "terminate" || command.operation === "disconnect";
        const argumentsValue: Record<string, typeof Schema.Json.Type> =
          command.operation === "disconnect" ? { restart: false, terminateDebuggee: false } : {};
        const thread = current.session?.threads[0];
        const threadId =
          thread === undefined ? undefined : current.threadIdByRef.get(thread.threadRef);
        if (!terminalOperation && threadId !== undefined) argumentsValue.threadId = threadId;
        if (command.operation === "restart_frame") {
          const frame = current.session?.frames[0];
          const frameId =
            frame === undefined ? undefined : current.frameIdByRef.get(frame.frameRef);
          if (frameId !== undefined) argumentsValue.frameId = frameId;
        }
        let response: Awaited<ReturnType<DapClient["request"]>> | null = null;
        let adapterFailure: string | null = null;
        try {
          response = await requestWithPermit(
            current,
            client,
            commandName(command.operation),
            argumentsValue,
            { signal: current.operations.get(command.operationRef)?.signal },
          );
        } catch (cause) {
          if (!terminalOperation) throw cause;
          adapterFailure = clean(
            redactWith(
              current,
              cause instanceof Error ? cause.message : "The adapter did not acknowledge teardown.",
            ).text,
            500,
          );
        }
        const changed = await Effect.runPromise(current.service.control(command));
        current.session = changed;
        if (terminalOperation) {
          await client.dispose(`The debug session completed through ${command.operation}.`);
          current.client = null;
          current.resolution = null;
          const postdebugTaskRef =
            changed.configuration.intent._tag === "Launch"
              ? changed.configuration.intent.postdebugTaskRef
              : null;
          if (postdebugTaskRef !== null) {
            assertPortablePermit(current, "IdeDap.postdebug");
            if (
              options.runTask === undefined ||
              !(await options.runTask(postdebugTaskRef, command.actor))
            ) {
              throw new IdeDebugProtocolFailure({
                operation: "IdeDap.postdebug",
                detail: `Postdebug task ${postdebugTaskRef} did not complete successfully.`,
              });
            }
            assertPortablePermit(current, "IdeDap.postdebug.result");
          }
        }
        return adapterFailure === null
          ? {
              adapterAcknowledged: true,
              response: response === null ? null : responsePayload(response),
            }
          : { adapterAcknowledged: false, adapterFailure };
      }
      case "Evaluate": {
        const client = current.client;
        if (client === null) throw new Error("No DAP adapter is active.");
        await Effect.runPromise(current.service.preflightEvaluation(command));
        const frameId =
          command.frameRef === null ? undefined : current.frameIdByRef.get(command.frameRef);
        const body = decodeBody(
          EvaluateBodySchema,
          await requestWithPermit(
            current,
            client,
            "evaluate",
            {
              expression: command.expression,
              context: "watch",
              ...(frameId === undefined ? {} : { frameId }),
            },
            { signal: current.operations.get(command.operationRef)?.signal },
          ),
          "evaluate",
        );
        const secured = redactWith(current, body.result);
        await Effect.runPromise(
          current.service.recordEvaluation({
            ...command,
            value: secured.text,
            type: body.type ?? null,
            failedMessage: null,
          }),
        );
        return {
          value: secured.text,
          type: body.type ?? null,
          variablesReference: body.variablesReference,
        };
      }
      case "SetVariable": {
        const client = current.client;
        const target = current.variableTargetByRef.get(command.variableRef);
        if (client === null || target === undefined)
          throw new Error("The DAP variable target is unavailable.");
        await Effect.runPromise(current.service.preflightSetVariable(command));
        const body = decodeBody(
          SetVariableBodySchema,
          await requestWithPermit(
            current,
            client,
            "setVariable",
            {
              variablesReference: target.variablesReference,
              name: target.name,
              value: command.value,
            },
            { signal: current.operations.get(command.operationRef)?.signal },
          ),
          "setVariable",
        );
        const secured = redactWith(current, body.value);
        await Effect.runPromise(
          current.service.recordSetVariable({
            ...command,
            value: secured.text,
            type: body.type ?? null,
          }),
        );
        return { value: secured.text, type: body.type ?? null };
      }
      case "NavigateSource": {
        const client = current.client;
        if (client === null) throw new Error("No DAP adapter is active.");
        await Effect.runPromise(current.service.preflightSource(command));
        const dapSource = current.sourceByRef.get(command.source.sourceRef);
        if (dapSource === undefined) throw new Error("The DAP source identity is unavailable.");
        const body = decodeBody(
          SourceBodySchema,
          await requestWithPermit(
            current,
            client,
            "source",
            { source: dapSource, sourceReference: dapSource.sourceReference ?? 0 },
            { signal: current.operations.get(command.operationRef)?.signal },
          ),
          "source",
        );
        await Effect.runPromise(current.service.navigateSource(command));
        return { content: redactWith(current, body.content).text, mimeType: body.mimeType ?? null };
      }
      case "Cancel":
        return null;
      case "DeleteRetainedData": {
        await Effect.runPromise(
          current.service.deleteRetainedData(command.operationRef, command.reason, command.actor),
        );
        current.configurations.clear();
        current.sourceByRef.clear();
        current.variableTargetByRef.clear();
        if (options.persistenceRoot !== undefined) {
          const snapshot = await Effect.runPromise(current.service.snapshot);
          await rm(persistenceFile(options.persistenceRoot, snapshot.binding.projectRef), {
            force: true,
          });
        }
        return null;
      }
      case "Cleanup":
        await closeRuntime(command.reason, command.operationRef, command.actor);
        return null;
    }
  };

  const command = async (value: unknown): Promise<IdeDebugCommandResult | null> => {
    const current = await ensureRuntime();
    if (current === null) return null;
    const decoded = Schema.decodeUnknownExit(IdeDebugCommandSchema)(value);
    if (Exit.isFailure(decoded))
      return refused(current, "invalid_input", "The IDE debug command is invalid.");
    const requested = decoded.value;
    if (requested._tag === "Cancel") {
      const controller = current.operations.get(requested.targetOperationRef);
      if (controller === undefined)
        return refused(
          current,
          "unavailable",
          `Operation ${requested.targetOperationRef} is not pending.`,
        );
      controller.abort(requested.reason);
      await Effect.runPromise(
        current.service.recordCancellation(
          requested.operationRef,
          requested.targetOperationRef,
          requested.reason,
          requested.actor,
        ),
      );
      return IdeDebugCommandResultSchema.cases.Succeeded.make({
        snapshot: await Effect.runPromise(current.service.snapshot),
        payload: { canceledOperationRef: requested.targetOperationRef },
      });
    }
    const controller = new AbortController();
    current.operations.set(requested.operationRef, controller);
    const execute = async (): Promise<IdeDebugCommandResult | null> => {
      try {
        const active = await ensureRuntime();
        if (active === null) return null;
        if (controller.signal.aborted)
          throw new IdeDebugCancellation({
            operation: requested._tag,
            detail: "The debug operation was canceled before it started.",
          });
        const payload = await runCommand(active, requested);
        return IdeDebugCommandResultSchema.cases.Succeeded.make({
          snapshot: await Effect.runPromise((runtime ?? active).service.snapshot),
          payload,
        });
      } catch (cause) {
        const tagged =
          cause !== null && typeof cause === "object" && "_tag" in cause ? String(cause._tag) : "";
        const message =
          cause !== null &&
          typeof cause === "object" &&
          "detail" in cause &&
          typeof cause.detail === "string"
            ? cause.detail
            : cause instanceof Error
              ? cause.message
              : "The DAP operation failed.";
        const reason = tagged.includes("StaleEvent")
          ? "stale_generation"
          : tagged.includes("Protocol")
            ? "protocol"
            : tagged.includes("Admission")
              ? "not_admitted"
              : "unavailable";
        if (reason === "stale_generation" && runtime !== null) {
          schedulePortableRevocation(runtime);
        }
        return refused(runtime ?? current, reason, message);
      } finally {
        current.operations.delete(requested.operationRef);
      }
    };
    const result = commandTail.then(execute, execute);
    commandTail = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  };

  return {
    snapshot,
    command,
    pendingRequestCount: () => runtime?.client?.pendingRequestCount() ?? 0,
    dispose: async () => {
      if (disposed) return;
      disposed = true;
      await commandTail;
      await closeRuntime("IDE DAP host disposed");
    },
  };
};
