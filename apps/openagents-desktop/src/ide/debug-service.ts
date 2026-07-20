import { createHash, randomUUID } from "node:crypto";

import { Context, Effect, Layer, PubSub, Schema, Stream, SubscriptionRef } from "effect";

import {
  IdeDebugAdapterEventSchema,
  IdeDebugAdapterGenerationSchema,
  IdeDebugBreakpointSchema,
  IdeDebugCapabilityFailure,
  IdeDebugConsoleEntrySchema,
  IdeDebugConfigurationSchema,
  IdeDebugConfigurationFailure,
  IdeDebugEventSchema,
  IdeDebugReceiptRefSchema,
  IdeDebugReceiptSchema,
  IdeDebugSessionGenerationSchema,
  IdeDebugSessionNotFound,
  IdeDebugSessionSchema,
  IdeDebugSequenceSchema,
  IdeDebugStaleEvent,
  IdeDebugStopped,
  IdeDebugTargetGenerationSchema,
  IdeDebugWatchRefSchema,
  IdeDebugWatchSchema,
  IdeDebugAdmissionFailure,
  IdeDebugSnapshotSchema,
  type IdeDebugAdapterEvent,
  type IdeDebugBreakpoint,
  type IdeDebugCapabilityName,
  type IdeDebugConfiguration,
  type IdeDebugControlOperation,
  type IdeDebugEvent,
  type IdeDebugOperationRef,
  type IdeDebugReceipt,
  type IdeDebugServiceError,
  type IdeDebugSession,
  type IdeDebugSnapshot,
  type IdeDebugSource,
  type IdeDebugVariable,
  type IdeDebugWatch,
} from "./debug-contract.ts";
import { IdeDebugSessionRefSchema, IdeTimestampSchema } from "./project-contract.ts";
import type { IdeRunActor } from "./run-contract.ts";

const MAX_CONFIGURATIONS = 1_000;
const MAX_SESSIONS = 128;
const MAX_RECEIPTS = 2_000;
const MAX_BREAKPOINTS = 10_000;
const MAX_CONSOLE_ENTRIES = 2_048;
const DEFAULT_CONSOLE_BYTE_LIMIT = 262_144;
const decodeDebugSnapshot = Schema.decodeUnknownEffect(IdeDebugSnapshotSchema);

const trim = <A>(values: ReadonlyArray<A>, limit: number): ReadonlyArray<A> =>
  values.length <= limit ? values : values.slice(values.length - limit);

const nowTimestamp = (now: () => string) => IdeTimestampSchema.make(now());
const digest = (value: unknown): string =>
  createHash("sha256").update(JSON.stringify(value)).digest("hex");

const retainUtf8Prefix = (
  text: string,
  byteLimit: number,
): { readonly text: string; readonly truncated: boolean } => {
  const bytes = Buffer.from(text);
  if (bytes.length <= byteLimit) return { text, truncated: false };
  let end = byteLimit;
  while (end > 0 && (bytes[end] ?? 0) >= 0x80 && (bytes[end] ?? 0) < 0xc0) end -= 1;
  return { text: bytes.subarray(0, end).toString("utf8"), truncated: true };
};

const makeRedactor = (secretValues: ReadonlyArray<string>) => {
  const values = secretValues
    .filter((value) => value.length > 0)
    .toSorted((left, right) => right.length - left.length);
  return (source: string): { readonly text: string; readonly redacted: boolean } => {
    let text = source;
    let redacted = false;
    for (const value of values) {
      if (!text.includes(value)) continue;
      text = text.replaceAll(value, "[REDACTED]");
      redacted = true;
    }
    const secretAssignment =
      /\b(token|password|secret|authorization|api[_-]?key)\s*[:=]\s*([^\s,;]+)/giu;
    text = text.replace(secretAssignment, (_match, label: string) => {
      redacted = true;
      return `${label}=[REDACTED]`;
    });
    return { text, redacted };
  };
};

const bindingMatches = (
  configuration: IdeDebugConfiguration,
  snapshot: IdeDebugSnapshot,
): boolean => {
  const left = configuration.binding;
  const right = snapshot.binding;
  return (
    left.projectRef === right.projectRef &&
    left.rootRef === right.rootRef &&
    left.worktreeRef === right.worktreeRef &&
    left.attachmentGeneration === right.attachmentGeneration &&
    left.languageGeneration === right.languageGeneration &&
    left.placementGeneration === right.placementGeneration &&
    left.serviceGeneration === right.serviceGeneration &&
    left.placementRef === right.placementRef &&
    left.language === right.language
  );
};

const capabilityForControl = (operation: IdeDebugControlOperation): IdeDebugCapabilityName =>
  operation;

const hasCapability = (session: IdeDebugSession, capability: IdeDebugCapabilityName): boolean =>
  session.configuration.adapter.capabilities.some(
    (entry) => entry.capability === capability && entry.supported,
  );

const sanitizeBreakpoint = (
  breakpoint: IdeDebugBreakpoint,
  redact: ReturnType<typeof makeRedactor>,
): IdeDebugBreakpoint => {
  const common = {
    ...breakpoint,
    condition: breakpoint.condition === null ? null : redact(breakpoint.condition).text,
    hitCondition: breakpoint.hitCondition === null ? null : redact(breakpoint.hitCondition).text,
    logMessage: breakpoint.logMessage === null ? null : redact(breakpoint.logMessage).text,
    message: breakpoint.message === null ? null : redact(breakpoint.message).text,
  };
  return IdeDebugBreakpointSchema.make(common);
};

const sanitizeConfiguration = (
  configuration: IdeDebugConfiguration,
  redact: ReturnType<typeof makeRedactor>,
): IdeDebugConfiguration =>
  IdeDebugConfigurationSchema.make({
    ...configuration,
    label: redact(configuration.label).text,
    cwdRef: redact(configuration.cwdRef).text,
    refusalReason:
      configuration.refusalReason === null ? null : redact(configuration.refusalReason).text,
    placement:
      configuration.placement._tag === "Local"
        ? { ...configuration.placement, hostLabel: redact(configuration.placement.hostLabel).text }
        : { ...configuration.placement, hostLabel: redact(configuration.placement.hostLabel).text },
    intent:
      configuration.intent._tag === "Launch"
        ? {
            ...configuration.intent,
            executableLabel: redact(configuration.intent.executableLabel).text,
            argumentLabels: configuration.intent.argumentLabels.map(
              (argument) => redact(argument).text,
            ),
          }
        : {
            ...configuration.intent,
            targetProcessLabel: redact(configuration.intent.targetProcessLabel).text,
          },
    adapter: {
      ...configuration.adapter,
      capabilities: configuration.adapter.capabilities.map((capability) => ({
        ...capability,
        reason: capability.reason === null ? null : redact(capability.reason).text,
      })),
    },
    sourceMaps: {
      ...configuration.sourceMaps,
      sourceRoots: configuration.sourceMaps.sourceRoots.map((root) => redact(root).text),
      remoteRootRefs: configuration.sourceMaps.remoteRootRefs.map((root) => redact(root).text),
    },
  });

type GenerationFence = Readonly<{
  sessionRef: IdeDebugSession["sessionRef"];
  sessionGeneration: IdeDebugSession["sessionGeneration"];
  adapterGeneration: IdeDebugSession["adapterGeneration"];
  targetGeneration: IdeDebugSession["targetGeneration"];
}>;

type StartInput = Readonly<{
  operationRef: IdeDebugOperationRef;
  configuration: IdeDebugConfiguration;
  actor: IdeRunActor;
}>;
type BreakpointInput = GenerationFence &
  Readonly<{
    operationRef: IdeDebugOperationRef;
    breakpoints: ReadonlyArray<IdeDebugBreakpoint>;
    actor: IdeRunActor;
  }>;
type ControlInput = GenerationFence &
  Readonly<{
    operationRef: IdeDebugOperationRef;
    operation: IdeDebugControlOperation;
    actor: IdeRunActor;
  }>;
type AdapterEventInput = GenerationFence & Readonly<{ event: IdeDebugAdapterEvent }>;
type EvaluationInput = GenerationFence &
  Readonly<{
    operationRef: IdeDebugOperationRef;
    actor: IdeRunActor;
    expression: string;
    value: string;
    type: string | null;
    failedMessage: string | null;
  }>;
type SetVariableInput = GenerationFence &
  Readonly<{
    operationRef: IdeDebugOperationRef;
    actor: IdeRunActor;
    variableRef: IdeDebugVariable["variableRef"];
    value: string;
    type: string | null;
  }>;
type NavigateSourceInput = GenerationFence &
  Readonly<{ operationRef: IdeDebugOperationRef; actor: IdeRunActor; source: IdeDebugSource }>;

export interface IdeDebugServiceShape {
  readonly snapshot: Effect.Effect<IdeDebugSnapshot>;
  readonly events: Stream.Stream<IdeDebugEvent>;
  readonly registerSecretValues: (values: ReadonlyArray<string>) => Effect.Effect<void>;
  readonly replaceConfigurations: (
    configurations: ReadonlyArray<IdeDebugConfiguration>,
  ) => Effect.Effect<IdeDebugSnapshot, IdeDebugServiceError>;
  readonly validate: (
    configuration: IdeDebugConfiguration,
    actor: IdeRunActor,
    operationRef: IdeDebugOperationRef,
  ) => Effect.Effect<IdeDebugConfiguration, IdeDebugServiceError>;
  readonly start: (input: StartInput) => Effect.Effect<IdeDebugSession, IdeDebugServiceError>;
  readonly preflightBreakpoints: (
    input: BreakpointInput,
  ) => Effect.Effect<IdeDebugSession, IdeDebugServiceError>;
  readonly replaceBreakpoints: (
    input: BreakpointInput,
  ) => Effect.Effect<IdeDebugSession, IdeDebugServiceError>;
  readonly preflightControl: (
    input: ControlInput,
  ) => Effect.Effect<IdeDebugSession, IdeDebugServiceError>;
  readonly control: (input: ControlInput) => Effect.Effect<IdeDebugSession, IdeDebugServiceError>;
  readonly preflightEvaluation: (
    input: Omit<EvaluationInput, "value" | "type" | "failedMessage">,
  ) => Effect.Effect<IdeDebugSession, IdeDebugServiceError>;
  readonly recordEvaluation: (
    input: EvaluationInput,
  ) => Effect.Effect<IdeDebugWatch, IdeDebugServiceError>;
  readonly preflightSetVariable: (
    input: Omit<SetVariableInput, "value" | "type">,
  ) => Effect.Effect<IdeDebugVariable, IdeDebugServiceError>;
  readonly recordSetVariable: (
    input: SetVariableInput,
  ) => Effect.Effect<IdeDebugVariable, IdeDebugServiceError>;
  readonly preflightSource: (
    input: NavigateSourceInput,
  ) => Effect.Effect<IdeDebugSource, IdeDebugServiceError>;
  readonly navigateSource: (
    input: NavigateSourceInput,
  ) => Effect.Effect<IdeDebugSource, IdeDebugServiceError>;
  readonly applyAdapterEvent: (
    input: AdapterEventInput,
  ) => Effect.Effect<IdeDebugSession, IdeDebugServiceError>;
  readonly recordCancellation: (
    operationRef: IdeDebugOperationRef,
    targetOperationRef: IdeDebugOperationRef,
    reason: string,
    actor: IdeRunActor,
  ) => Effect.Effect<IdeDebugSnapshot, IdeDebugServiceError>;
  readonly deleteRetainedData: (
    operationRef: IdeDebugOperationRef,
    reason: string,
    actor: IdeRunActor,
  ) => Effect.Effect<IdeDebugSnapshot, IdeDebugServiceError>;
  readonly cleanup: (
    operationRef: IdeDebugOperationRef,
    reason: string,
    actor: IdeRunActor,
  ) => Effect.Effect<IdeDebugSnapshot, IdeDebugServiceError>;
}

export class IdeDebugService extends Context.Service<IdeDebugService, IdeDebugServiceShape>()(
  "@openagents/desktop/IdeDebugService",
) {}

export interface IdeDebugServiceOptions {
  readonly now?: () => string;
  readonly consoleByteLimit?: number;
  readonly secretValues?: ReadonlyArray<string>;
}

export const makeIdeDebugServiceLayer = (
  seed: IdeDebugSnapshot,
  options: IdeDebugServiceOptions = {},
): Layer.Layer<IdeDebugService, IdeDebugConfigurationFailure> =>
  Layer.effect(
    IdeDebugService,
    Effect.gen(function* () {
      const decoded = yield* decodeDebugSnapshot(seed).pipe(
        Effect.mapError(
          (cause) =>
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.acquire",
              detail: `The initial debug graph is invalid: ${String(cause)}`,
            }),
        ),
      );
      const state = yield* SubscriptionRef.make(decoded);
      const eventBus = yield* PubSub.unbounded<IdeDebugEvent>();
      const now = options.now ?? (() => new Date().toISOString());
      const consoleByteLimit = options.consoleByteLimit ?? DEFAULT_CONSOLE_BYTE_LIMIT;
      let secretValues = [...(options.secretValues ?? [])];
      let redact = makeRedactor(secretValues);

      const registerSecretValues = Effect.fn("IdeDebug.registerSecretValues")(
        (values: ReadonlyArray<string>) =>
          Effect.sync(() => {
            secretValues = [
              ...new Set([...secretValues, ...values.filter((value) => value.length > 0)]),
            ];
            redact = makeRedactor(secretValues);
          }),
      );

      const publishSnapshot = Effect.fn("IdeDebug.publishSnapshot")(function* (
        snapshot: IdeDebugSnapshot,
      ) {
        yield* PubSub.publish(eventBus, IdeDebugEventSchema.cases.Snapshot.make({ snapshot }));
      });

      const update = Effect.fn("IdeDebug.update")(function* (
        operation: string,
        change: (current: IdeDebugSnapshot) => IdeDebugSnapshot,
      ) {
        const next = yield* SubscriptionRef.modify(state, (current) => {
          const changed = IdeDebugSnapshotSchema.make(change(current));
          return [changed, changed] as const;
        });
        yield* publishSnapshot(next);
        return next;
      });

      const ensureActive = Effect.fn("IdeDebug.ensureActive")(function* (operation: string) {
        const current = yield* SubscriptionRef.get(state);
        if (current.stopped) {
          return yield* Effect.fail(
            new IdeDebugStopped({ operation, detail: "The debug graph is stopped." }),
          );
        }
        return current;
      });

      const projectConfiguration = Effect.fn("IdeDebug.projectConfiguration")(function* (
        operation: string,
        configuration: IdeDebugConfiguration,
      ) {
        const current = yield* ensureActive(operation);
        if (!bindingMatches(configuration, current)) {
          return yield* Effect.fail(
            new IdeDebugStaleEvent({
              operation,
              detail:
                "The configuration does not match the active project, language, placement, and service generations.",
            }),
          );
        }
        return sanitizeConfiguration(configuration, redact);
      });

      const validateConfiguration = Effect.fn("IdeDebug.validateConfiguration")(function* (
        operation: string,
        configuration: IdeDebugConfiguration,
      ) {
        const projected = yield* projectConfiguration(operation, configuration);
        if (!projected.admitted || !projected.adapter.admitted) {
          return yield* Effect.fail(
            new IdeDebugAdmissionFailure({
              operation,
              detail:
                projected.refusalReason ?? "The debug configuration or adapter is not admitted.",
            }),
          );
        }
        if (
          projected.intent._tag === "Attach" &&
          projected.placement._tag === "Remote" &&
          projected.intent.authenticationRef === null
        ) {
          return yield* Effect.fail(
            new IdeDebugAdmissionFailure({
              operation,
              detail: "A remote attach requires an admitted authentication reference.",
            }),
          );
        }
        return projected;
      });

      const findSession = Effect.fn("IdeDebug.findSession")(function* (
        operation: string,
        sessionRef: IdeDebugSession["sessionRef"],
      ) {
        const current = yield* ensureActive(operation);
        const session = current.sessions.find((candidate) => candidate.sessionRef === sessionRef);
        if (session === undefined) {
          return yield* Effect.fail(
            new IdeDebugSessionNotFound({
              operation,
              detail: `Debug session ${sessionRef} was not found.`,
            }),
          );
        }
        return session;
      });

      const fencedSession = Effect.fn("IdeDebug.fencedSession")(function* (
        operation: string,
        fence: GenerationFence,
      ) {
        const session = yield* findSession(operation, fence.sessionRef);
        if (
          session.sessionGeneration !== fence.sessionGeneration ||
          session.adapterGeneration !== fence.adapterGeneration ||
          session.targetGeneration !== fence.targetGeneration
        ) {
          yield* PubSub.publish(
            eventBus,
            IdeDebugEventSchema.cases.StaleEventDropped.make({
              sessionRef: fence.sessionRef,
              detail: "The event generations do not match the active session generations.",
              observedAt: nowTimestamp(now),
            }),
          );
          return yield* Effect.fail(
            new IdeDebugStaleEvent({
              operation,
              detail: "The event generations do not match the active session generations.",
            }),
          );
        }
        return session;
      });

      const makeReceipt = (
        session: IdeDebugSession,
        operationRef: IdeDebugOperationRef,
        actor: IdeRunActor,
        operation: IdeDebugReceipt["operation"],
        outcome: string,
      ): IdeDebugReceipt =>
        IdeDebugReceiptSchema.make({
          receiptRef: IdeDebugReceiptRefSchema.make(`ide.debug-receipt.${randomUUID()}`),
          operationRef,
          configurationRef: session.configuration.configurationRef,
          sessionRef: session.sessionRef,
          sessionGeneration: session.sessionGeneration,
          actor,
          operation,
          disposition: "succeeded",
          outcome,
          targetRef: session.configuration.targetRef,
          placementRef: session.configuration.binding.placementRef,
          environmentDigest: session.configuration.environment.digest,
          configurationDigest: digest(session.configuration),
          observedAt: nowTimestamp(now),
        });

      const replaceSession = (
        current: IdeDebugSnapshot,
        session: IdeDebugSession,
        receipt: IdeDebugReceipt | null = null,
      ): IdeDebugSnapshot => ({
        ...current,
        sessions: current.sessions.map((candidate) =>
          candidate.sessionRef === session.sessionRef ? session : candidate,
        ),
        receipts:
          receipt === null ? current.receipts : trim([...current.receipts, receipt], MAX_RECEIPTS),
      });

      const validate = Effect.fn("IdeDebug.validate")(function* (
        configuration: IdeDebugConfiguration,
        actor: IdeRunActor,
        operationRef: IdeDebugOperationRef,
      ) {
        const admitted = yield* validateConfiguration("IdeDebug.validate", configuration);
        const receipt = IdeDebugReceiptSchema.make({
          receiptRef: IdeDebugReceiptRefSchema.make(`ide.debug-receipt.${randomUUID()}`),
          operationRef,
          configurationRef: admitted.configurationRef,
          sessionRef: null,
          sessionGeneration: null,
          actor,
          operation: "validate",
          disposition: "succeeded",
          outcome: "admitted",
          targetRef: admitted.targetRef,
          placementRef: admitted.binding.placementRef,
          environmentDigest: admitted.environment.digest,
          configurationDigest: digest(admitted),
          observedAt: nowTimestamp(now),
        });
        yield* update("IdeDebug.validate", (current) => ({
          ...current,
          configurations: trim(
            [
              ...current.configurations.filter(
                (candidate) => candidate.configurationRef !== admitted.configurationRef,
              ),
              admitted,
            ],
            MAX_CONFIGURATIONS,
          ),
          receipts: trim([...current.receipts, receipt], MAX_RECEIPTS),
        }));
        return admitted;
      });

      const replaceConfigurations = Effect.fn("IdeDebug.replaceConfigurations")(function* (
        configurations: ReadonlyArray<IdeDebugConfiguration>,
      ) {
        if (configurations.length > MAX_CONFIGURATIONS) {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.replaceConfigurations",
              detail: "The debug configuration limit was exceeded.",
            }),
          );
        }
        const admitted: Array<IdeDebugConfiguration> = [];
        for (const configuration of configurations) {
          admitted.push(
            yield* projectConfiguration("IdeDebug.replaceConfigurations", configuration),
          );
        }
        if (
          new Set(admitted.map((configuration) => configuration.configurationRef)).size !==
          admitted.length
        ) {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.replaceConfigurations",
              detail: "Debug configuration identities must be unique.",
            }),
          );
        }
        return yield* update("IdeDebug.replaceConfigurations", (current) => ({
          ...current,
          configurations: admitted,
        }));
      });

      const start = Effect.fn("IdeDebug.start")(function* ({
        operationRef,
        configuration,
        actor,
      }: StartInput) {
        const admitted = yield* validateConfiguration("IdeDebug.start", configuration);
        const current = yield* SubscriptionRef.get(state);
        const active = current.sessions.find(
          (session) =>
            session.configuration.configurationRef === admitted.configurationRef &&
            session.lifecycle._tag !== "Terminated" &&
            session.lifecycle._tag !== "Disconnected" &&
            session.lifecycle._tag !== "Failed",
        );
        if (active !== undefined) {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.start",
              detail: `Configuration ${admitted.configurationRef} already has an active session.`,
            }),
          );
        }
        const savedBreakpoints =
          current.breakpointSets.find(
            (entry) => entry.configurationRef === admitted.configurationRef,
          )?.breakpoints ?? [];
        const session = IdeDebugSessionSchema.make({
          sessionRef: IdeDebugSessionRefSchema.make(`ide.debug-session.${randomUUID()}`),
          sessionGeneration: IdeDebugSessionGenerationSchema.make(1),
          adapterGeneration: IdeDebugAdapterGenerationSchema.make(1),
          targetGeneration: IdeDebugTargetGenerationSchema.make(1),
          configuration: admitted,
          actor,
          lifecycle: { _tag: "Starting", startedAt: nowTimestamp(now) },
          breakpoints: savedBreakpoints,
          threads: [],
          frames: [],
          scopes: [],
          variables: [],
          watches: [],
          modules: [],
          loadedSources: [],
          console: [],
          invalidatedAreas: [],
          retainedConsoleBytes: 0,
          droppedConsoleBytes: 0,
        });
        const operation = admitted.intent._tag === "Launch" ? "launch" : "attach";
        const receipt = makeReceipt(session, operationRef, actor, operation, "starting");
        yield* update("IdeDebug.start", (snapshot) => ({
          ...snapshot,
          capabilityState: {
            _tag: "Starting",
            since: nowTimestamp(now),
            serviceGeneration: admitted.binding.serviceGeneration,
          },
          configurations: trim(
            [
              ...snapshot.configurations.filter(
                (candidate) => candidate.configurationRef !== admitted.configurationRef,
              ),
              admitted,
            ],
            MAX_CONFIGURATIONS,
          ),
          sessions: trim([...snapshot.sessions, session], MAX_SESSIONS),
          receipts: trim([...snapshot.receipts, receipt], MAX_RECEIPTS),
        }));
        return session;
      });

      const preflightBreakpoints = Effect.fn("IdeDebug.preflightBreakpoints")(function* (
        input: BreakpointInput,
      ) {
        const session = yield* fencedSession("IdeDebug.replaceBreakpoints", input);
        if (input.breakpoints.length > MAX_BREAKPOINTS) {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.replaceBreakpoints",
              detail: "The breakpoint limit was exceeded.",
            }),
          );
        }
        if (
          new Set(input.breakpoints.map((breakpoint) => breakpoint.breakpointRef)).size !==
          input.breakpoints.length
        ) {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.replaceBreakpoints",
              detail: "Breakpoint identities must be unique.",
            }),
          );
        }
        for (const breakpoint of input.breakpoints) {
          const required: ReadonlyArray<IdeDebugCapabilityName> = [
            ...(breakpoint._tag === "Function" ? ["function_breakpoints" as const] : []),
            ...(breakpoint._tag === "Data" ? ["data_breakpoints" as const] : []),
            ...(breakpoint.condition !== null ? ["conditional_breakpoints" as const] : []),
            ...(breakpoint.hitCondition !== null ? ["hit_conditional_breakpoints" as const] : []),
            ...(breakpoint.logMessage !== null ? ["log_points" as const] : []),
          ];
          const unsupported = required.find((capability) => !hasCapability(session, capability));
          if (unsupported !== undefined) {
            return yield* Effect.fail(
              new IdeDebugCapabilityFailure({
                operation: "IdeDebug.replaceBreakpoints",
                capability: unsupported,
                detail: `The adapter did not negotiate ${unsupported}.`,
              }),
            );
          }
        }
        return session;
      });

      const replaceBreakpoints = Effect.fn("IdeDebug.replaceBreakpoints")(function* (
        input: BreakpointInput,
      ) {
        const session = yield* preflightBreakpoints(input);
        const changed = IdeDebugSessionSchema.make({
          ...session,
          breakpoints: input.breakpoints.map((breakpoint) =>
            sanitizeBreakpoint(breakpoint, redact),
          ),
        });
        const receipt = makeReceipt(
          changed,
          input.operationRef,
          input.actor,
          "breakpoints",
          `replaced:${changed.breakpoints.length}`,
        );
        yield* update("IdeDebug.replaceBreakpoints", (current) => ({
          ...replaceSession(current, changed, receipt),
          breakpointSets: [
            ...current.breakpointSets.filter(
              (entry) => entry.configurationRef !== changed.configuration.configurationRef,
            ),
            {
              configurationRef: changed.configuration.configurationRef,
              breakpoints: changed.breakpoints,
              updatedAt: nowTimestamp(now),
            },
          ],
        }));
        return changed;
      });

      const preflightControl = Effect.fn("IdeDebug.preflightControl")(function* (
        input: ControlInput,
      ) {
        const session = yield* fencedSession("IdeDebug.control", input);
        const capability = capabilityForControl(input.operation);
        if (!hasCapability(session, capability)) {
          return yield* Effect.fail(
            new IdeDebugCapabilityFailure({
              operation: "IdeDebug.control",
              capability,
              detail: `The adapter did not negotiate ${capability}.`,
            }),
          );
        }
        return session;
      });

      const control = Effect.fn("IdeDebug.control")(function* (input: ControlInput) {
        const session = yield* preflightControl(input);
        const observedAt = nowTimestamp(now);
        const lifecycle =
          input.operation === "terminate"
            ? {
                _tag: "Terminated" as const,
                terminatedAt: observedAt,
                reason: "Termination was requested.",
              }
            : input.operation === "disconnect"
              ? {
                  _tag: "Disconnected" as const,
                  disconnectedAt: observedAt,
                  targetTerminated: false,
                }
              : input.operation === "restart_session"
                ? { _tag: "Restarting" as const, requestedAt: observedAt }
                : input.operation === "continue" ||
                    input.operation.startsWith("step_") ||
                    input.operation === "run_to_cursor"
                  ? { _tag: "Running" as const, startedAt: observedAt }
                  : session.lifecycle;
        const restarting = input.operation === "restart_session";
        const changed = IdeDebugSessionSchema.make({
          ...session,
          sessionGeneration: restarting
            ? IdeDebugSessionGenerationSchema.make(session.sessionGeneration + 1)
            : session.sessionGeneration,
          adapterGeneration: restarting
            ? IdeDebugAdapterGenerationSchema.make(session.adapterGeneration + 1)
            : session.adapterGeneration,
          targetGeneration: restarting
            ? IdeDebugTargetGenerationSchema.make(session.targetGeneration + 1)
            : session.targetGeneration,
          lifecycle,
          threads: restarting || input.operation === "terminate" ? [] : session.threads,
          frames: restarting || input.operation === "terminate" ? [] : session.frames,
          scopes: restarting || input.operation === "terminate" ? [] : session.scopes,
          variables: restarting || input.operation === "terminate" ? [] : session.variables,
          watches: restarting
            ? session.watches.map((watch) => ({ ...watch, state: "stale" as const }))
            : session.watches,
          modules: restarting || input.operation === "terminate" ? [] : session.modules,
          loadedSources: restarting || input.operation === "terminate" ? [] : session.loadedSources,
          invalidatedAreas:
            restarting || input.operation === "terminate"
              ? ["threads", "stacks", "scopes", "variables", "watches", "modules", "sources"]
              : session.invalidatedAreas,
        });
        const receiptOperation =
          input.operation === "restart_session"
            ? "restart"
            : input.operation === "disconnect"
              ? "disconnect"
              : input.operation === "terminate"
                ? "terminate"
                : "control";
        const receipt = makeReceipt(
          changed,
          input.operationRef,
          input.actor,
          receiptOperation,
          input.operation,
        );
        yield* update("IdeDebug.control", (current) => replaceSession(current, changed, receipt));
        return changed;
      });

      const preflightEvaluation = Effect.fn("IdeDebug.preflightEvaluation")(function* (
        input: Omit<EvaluationInput, "value" | "type" | "failedMessage">,
      ) {
        const session = yield* fencedSession("IdeDebug.recordEvaluation", input);
        if (!hasCapability(session, "evaluate")) {
          return yield* Effect.fail(
            new IdeDebugCapabilityFailure({
              operation: "IdeDebug.recordEvaluation",
              capability: "evaluate",
              detail: "The adapter did not negotiate evaluate.",
            }),
          );
        }
        return session;
      });

      const recordEvaluation = Effect.fn("IdeDebug.recordEvaluation")(function* (
        input: EvaluationInput,
      ) {
        const session = yield* preflightEvaluation(input);
        const expression = retainUtf8Prefix(redact(input.expression).text, 4_096);
        const valueRedaction = redact(input.value);
        const value = retainUtf8Prefix(valueRedaction.text, 16_384);
        const message =
          input.failedMessage === null
            ? null
            : retainUtf8Prefix(redact(input.failedMessage).text, 1_000).text;
        const watch = IdeDebugWatchSchema.make({
          watchRef: IdeDebugWatchRefSchema.make(`ide.debug-watch.${randomUUID()}`),
          expression: expression.text,
          value: value.text,
          type: input.type === null ? null : retainUtf8Prefix(redact(input.type).text, 500).text,
          state: input.failedMessage === null ? "ready" : "failed",
          message,
          redacted: valueRedaction.redacted || expression.text !== input.expression,
          truncated: value.truncated || expression.truncated,
        });
        const changed = IdeDebugSessionSchema.make({
          ...session,
          watches: trim([...session.watches, watch], 1_000),
        });
        const receipt = makeReceipt(
          changed,
          input.operationRef,
          input.actor,
          "evaluate",
          watch.state,
        );
        yield* update("IdeDebug.recordEvaluation", (current) =>
          replaceSession(current, changed, receipt),
        );
        return watch;
      });

      const preflightSetVariable = Effect.fn("IdeDebug.preflightSetVariable")(function* (
        input: Omit<SetVariableInput, "value" | "type">,
      ) {
        const session = yield* fencedSession("IdeDebug.recordSetVariable", input);
        if (!hasCapability(session, "set_variable")) {
          return yield* Effect.fail(
            new IdeDebugCapabilityFailure({
              operation: "IdeDebug.recordSetVariable",
              capability: "set_variable",
              detail: "The adapter did not negotiate set_variable.",
            }),
          );
        }
        const variable = session.variables.find(
          (candidate) => candidate.variableRef === input.variableRef,
        );
        if (variable === undefined) {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.recordSetVariable",
              detail: `Variable ${input.variableRef} is absent or stale.`,
            }),
          );
        }
        return variable;
      });

      const recordSetVariable = Effect.fn("IdeDebug.recordSetVariable")(function* (
        input: SetVariableInput,
      ) {
        const session = yield* fencedSession("IdeDebug.recordSetVariable", input);
        const variable = yield* preflightSetVariable(input);
        const sanitized = redact(input.value);
        const retained = retainUtf8Prefix(sanitized.text, 16_384);
        const changedVariable = {
          ...variable,
          value: retained.text,
          type:
            input.type === null
              ? variable.type
              : retainUtf8Prefix(redact(input.type).text, 500).text,
          redacted: variable.redacted || sanitized.redacted,
          truncated: variable.truncated || retained.truncated,
        } satisfies IdeDebugVariable;
        const changed = IdeDebugSessionSchema.make({
          ...session,
          variables: session.variables.map((candidate) =>
            candidate.variableRef === input.variableRef ? changedVariable : candidate,
          ),
        });
        const receipt = makeReceipt(
          changed,
          input.operationRef,
          input.actor,
          "set_variable",
          "updated",
        );
        yield* update("IdeDebug.recordSetVariable", (current) =>
          replaceSession(current, changed, receipt),
        );
        return changedVariable;
      });

      const preflightSource = Effect.fn("IdeDebug.preflightSource")(function* (
        input: NavigateSourceInput,
      ) {
        const session = yield* fencedSession("IdeDebug.navigateSource", input);
        if (input.source.availability === "stale") {
          return yield* Effect.fail(
            new IdeDebugStaleEvent({
              operation: "IdeDebug.navigateSource",
              detail: `Source ${input.source.sourceRef} is stale.`,
            }),
          );
        }
        if (input.source.availability !== "available") {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.navigateSource",
              detail: `Source ${input.source.sourceRef} is not available.`,
            }),
          );
        }
        if (
          input.source.origin === "project" &&
          (input.source.fileRef === null ||
            input.source.documentRef === null ||
            input.source.documentGeneration === null)
        ) {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.navigateSource",
              detail:
                "A project source requires canonical file, document, and document-generation identities.",
            }),
          );
        }
        if (
          (input.source.origin === "generated" || input.source.origin === "remote") &&
          input.source.sourceMapRef === null
        ) {
          return yield* Effect.fail(
            new IdeDebugConfigurationFailure({
              operation: "IdeDebug.navigateSource",
              detail: "A generated or remote source requires an explicit source-map reference.",
            }),
          );
        }
        if (input.source.origin === "adapter" && !hasCapability(session, "source_request")) {
          return yield* Effect.fail(
            new IdeDebugCapabilityFailure({
              operation: "IdeDebug.navigateSource",
              capability: "source_request",
              detail: "The adapter did not negotiate source_request.",
            }),
          );
        }
        const navigable = {
          ...input.source,
          pathRef: redact(input.source.pathRef).text,
          label: redact(input.source.label).text,
        } satisfies IdeDebugSource;
        return navigable;
      });

      const navigateSource = Effect.fn("IdeDebug.navigateSource")(function* (
        input: NavigateSourceInput,
      ) {
        const session = yield* fencedSession("IdeDebug.navigateSource", input);
        const navigable = yield* preflightSource(input);
        const receipt = makeReceipt(
          session,
          input.operationRef,
          input.actor,
          "source_navigation",
          input.source.origin,
        );
        yield* update("IdeDebug.navigateSource", (current) => ({
          ...current,
          receipts: trim([...current.receipts, receipt], MAX_RECEIPTS),
        }));
        return navigable;
      });

      const applyAdapterEvent = Effect.fn("IdeDebug.applyAdapterEvent")(function* (
        input: AdapterEventInput,
      ) {
        const session = yield* fencedSession("IdeDebug.applyAdapterEvent", input);
        if (
          session.lifecycle._tag === "Terminated" ||
          session.lifecycle._tag === "Disconnected" ||
          session.lifecycle._tag === "Failed"
        ) {
          return yield* Effect.fail(
            new IdeDebugStopped({
              operation: "IdeDebug.applyAdapterEvent",
              detail: `Debug session ${session.sessionRef} no longer accepts adapter events.`,
            }),
          );
        }
        const event = IdeDebugAdapterEventSchema.make(input.event);
        const observedAt = nowTimestamp(now);
        let changed: IdeDebugSession;
        switch (event._tag) {
          case "Initialized":
            changed = IdeDebugSessionSchema.make({
              ...session,
              configuration: {
                ...session.configuration,
                adapter: { ...session.configuration.adapter, capabilities: event.capabilities },
              },
              lifecycle: { _tag: "Running", startedAt: observedAt },
            });
            break;
          case "Stopped":
            changed = IdeDebugSessionSchema.make({
              ...session,
              lifecycle: {
                _tag: "Stopped",
                reason: redact(event.reason).text,
                stoppedAt: observedAt,
                threadRef: event.threadRef,
              },
              threads: session.threads.map((thread) =>
                event.allThreadsStopped || event.threadRef === thread.threadRef
                  ? { ...thread, state: "stopped" as const, stopReason: redact(event.reason).text }
                  : thread,
              ),
            });
            break;
          case "Continued":
            changed = IdeDebugSessionSchema.make({
              ...session,
              lifecycle: { _tag: "Running", startedAt: observedAt },
              threads: session.threads.map((thread) =>
                event.allThreadsContinued || event.threadRef === thread.threadRef
                  ? { ...thread, state: "running" as const, stopReason: null }
                  : thread,
              ),
              frames: [],
              scopes: [],
              variables: [],
              invalidatedAreas: ["stacks", "scopes", "variables", "watches"],
            });
            break;
          case "Projection":
            changed = IdeDebugSessionSchema.make({
              ...session,
              threads: event.threads,
              frames: event.frames,
              scopes: event.scopes,
              variables: event.variables.map((variable) => {
                const value = retainUtf8Prefix(redact(variable.value).text, 16_384);
                return {
                  ...variable,
                  value: value.text,
                  redacted: variable.redacted || redact(variable.value).redacted,
                  truncated: variable.truncated || value.truncated,
                };
              }),
              modules: event.modules,
              loadedSources: event.loadedSources,
              invalidatedAreas: [],
            });
            break;
          case "Output": {
            const sanitized = redact(event.text);
            const retained = retainUtf8Prefix(sanitized.text, consoleByteLimit);
            const entryBytes = Buffer.byteLength(retained.text);
            const sourceBytes = Buffer.byteLength(sanitized.text);
            let entries = [
              ...session.console,
              IdeDebugConsoleEntrySchema.make({
                sequence: IdeDebugSequenceSchema.make(
                  session.console.length === 0
                    ? 1
                    : (session.console[session.console.length - 1]?.sequence ?? 0) + 1,
                ),
                category: event.category,
                text: retained.text,
                redacted: sanitized.redacted,
                truncated: retained.truncated,
                gapBefore: retained.truncated,
                observedAt,
              }),
            ];
            let retainedBytes = session.retainedConsoleBytes + entryBytes;
            let droppedBytes = session.droppedConsoleBytes + Math.max(0, sourceBytes - entryBytes);
            while (
              (retainedBytes > consoleByteLimit || entries.length > MAX_CONSOLE_ENTRIES) &&
              entries.length > 1
            ) {
              const removed = entries.shift();
              if (removed === undefined) break;
              const removedBytes = Buffer.byteLength(removed.text);
              retainedBytes -= removedBytes;
              droppedBytes += removedBytes;
            }
            const first = entries[0];
            if (first !== undefined && droppedBytes > session.droppedConsoleBytes)
              entries = [{ ...first, gapBefore: true }, ...entries.slice(1)];
            changed = IdeDebugSessionSchema.make({
              ...session,
              console: entries,
              retainedConsoleBytes: retainedBytes,
              droppedConsoleBytes: droppedBytes,
            });
            break;
          }
          case "Invalidated":
            changed = IdeDebugSessionSchema.make({ ...session, invalidatedAreas: event.areas });
            break;
          case "Terminated":
            changed = IdeDebugSessionSchema.make({
              ...session,
              lifecycle: {
                _tag: "Terminated",
                terminatedAt: observedAt,
                reason: redact(event.reason).text,
              },
              threads: [],
              frames: [],
              scopes: [],
              variables: [],
              modules: [],
              invalidatedAreas: ["threads", "stacks", "scopes", "variables", "modules"],
            });
            break;
          case "AdapterFailed":
          case "TargetLost":
            changed = IdeDebugSessionSchema.make({
              ...session,
              lifecycle: {
                _tag: "Failed",
                failedAt: observedAt,
                reason: redact(event.reason).text,
              },
              threads: [],
              frames: [],
              scopes: [],
              variables: [],
              invalidatedAreas: ["threads", "stacks", "scopes", "variables", "watches"],
            });
            break;
        }
        yield* update("IdeDebug.applyAdapterEvent", (current) => ({
          ...replaceSession(current, changed),
          capabilityState:
            event._tag === "Initialized"
              ? {
                  _tag: "Ready",
                  serviceGeneration: changed.configuration.binding.serviceGeneration,
                  placementRef: changed.configuration.binding.placementRef,
                  evidenceTier:
                    changed.configuration.placement._tag === "Local"
                      ? "project_local"
                      : "owner_managed_remote",
                  observedAt,
                }
              : event._tag === "AdapterFailed" || event._tag === "TargetLost"
                ? {
                    _tag: "Failed",
                    serviceGeneration: changed.configuration.binding.serviceGeneration,
                    reason: redact(event.reason).text,
                    retry: "manual",
                    observedAt,
                  }
                : event._tag === "Terminated"
                  ? { _tag: "Stopped", reason: redact(event.reason).text, stoppedAt: observedAt }
                  : current.capabilityState,
        }));
        return changed;
      });

      const deleteRetainedData = Effect.fn("IdeDebug.deleteRetainedData")(function* (
        operationRef: IdeDebugOperationRef,
        reason: string,
        actor: IdeRunActor,
      ) {
        const current = yield* ensureActive("IdeDebug.deleteRetainedData");
        const observedAt = nowTimestamp(now);
        const sanitizedReason = redact(reason).text || "The retained debug data was deleted.";
        const sessions = current.sessions.map((session) =>
          IdeDebugSessionSchema.make({
            ...session,
            lifecycle: { _tag: "Terminated", terminatedAt: observedAt, reason: sanitizedReason },
            breakpoints: [],
            threads: [],
            frames: [],
            scopes: [],
            variables: [],
            watches: [],
            modules: [],
            loadedSources: [],
            console: [],
            invalidatedAreas: [
              "threads",
              "stacks",
              "scopes",
              "variables",
              "watches",
              "modules",
              "sources",
              "console",
            ],
            retainedConsoleBytes: 0,
            droppedConsoleBytes: 0,
          }),
        );
        const anchor = sessions.at(-1);
        const receipt =
          anchor === undefined
            ? null
            : makeReceipt(anchor, operationRef, actor, "delete_retained_data", "deleted");
        return yield* update("IdeDebug.deleteRetainedData", (snapshot) => ({
          ...snapshot,
          configurations: [],
          breakpointSets: [],
          sessions,
          receipts: receipt === null ? [] : [receipt],
          capabilityState: { _tag: "Stopped", reason: sanitizedReason, stoppedAt: observedAt },
        }));
      });

      const recordCancellation = Effect.fn("IdeDebug.recordCancellation")(function* (
        operationRef: IdeDebugOperationRef,
        targetOperationRef: IdeDebugOperationRef,
        reason: string,
        actor: IdeRunActor,
      ) {
        const current = yield* ensureActive("IdeDebug.recordCancellation");
        const session = current.sessions.at(-1);
        if (session === undefined) return current;
        const receipt = IdeDebugReceiptSchema.make({
          ...makeReceipt(session, operationRef, actor, "cancel", "canceled"),
          disposition: "canceled",
          outcome: `canceled:${targetOperationRef}:${redact(reason).text}`.slice(0, 160),
        });
        return yield* update("IdeDebug.recordCancellation", (snapshot) => ({
          ...snapshot,
          receipts: trim([...snapshot.receipts, receipt], MAX_RECEIPTS),
        }));
      });

      const cleanup = Effect.fn("IdeDebug.cleanup")(function* (
        operationRef: IdeDebugOperationRef,
        reason: string,
        actor: IdeRunActor,
      ) {
        const current = yield* SubscriptionRef.get(state);
        if (current.stopped) return current;
        const stoppedAt = nowTimestamp(now);
        const sanitizedReason = redact(reason).text || "Debug graph cleanup.";
        const sessions = current.sessions.map((session) =>
          IdeDebugSessionSchema.make({
            ...session,
            lifecycle: { _tag: "Terminated", terminatedAt: stoppedAt, reason: sanitizedReason },
            threads: [],
            frames: [],
            scopes: [],
            variables: [],
            watches: [],
            modules: [],
            loadedSources: [],
            console: [],
            invalidatedAreas: [
              "threads",
              "stacks",
              "scopes",
              "variables",
              "watches",
              "modules",
              "sources",
              "console",
            ],
            retainedConsoleBytes: 0,
          }),
        );
        const cleanupReceipts = sessions.map((session) =>
          makeReceipt(session, operationRef, actor, "cleanup", "terminated"),
        );
        return yield* update("IdeDebug.cleanup", (snapshot) => ({
          ...snapshot,
          sessions,
          receipts: trim([...snapshot.receipts, ...cleanupReceipts], MAX_RECEIPTS),
          capabilityState: { _tag: "Stopped", reason: sanitizedReason, stoppedAt },
          stopped: true,
        }));
      });

      const service = IdeDebugService.of({
        snapshot: SubscriptionRef.get(state),
        events: Stream.fromPubSub(eventBus),
        registerSecretValues,
        replaceConfigurations,
        validate,
        start,
        preflightBreakpoints,
        replaceBreakpoints,
        preflightControl,
        control,
        preflightEvaluation,
        recordEvaluation,
        preflightSetVariable,
        recordSetVariable,
        preflightSource,
        navigateSource,
        applyAdapterEvent,
        recordCancellation,
        deleteRetainedData,
        cleanup,
      });

      yield* Effect.addFinalizer(() =>
        Effect.gen(function* () {
          const terminatedAt = nowTimestamp(now);
          yield* SubscriptionRef.modify(state, (current) => {
            const closed = IdeDebugSnapshotSchema.make({
              ...current,
              sessions: current.sessions.map((session) =>
                IdeDebugSessionSchema.make({
                  ...session,
                  lifecycle:
                    session.lifecycle._tag === "Terminated" ||
                    session.lifecycle._tag === "Disconnected"
                      ? session.lifecycle
                      : {
                          _tag: "Terminated",
                          terminatedAt,
                          reason: "The debug service scope closed.",
                        },
                  threads: [],
                  frames: [],
                  scopes: [],
                  variables: [],
                  watches: [],
                  modules: [],
                  loadedSources: [],
                  console: [],
                  invalidatedAreas: [
                    "threads",
                    "stacks",
                    "scopes",
                    "variables",
                    "watches",
                    "modules",
                    "sources",
                    "console",
                  ],
                  retainedConsoleBytes: 0,
                }),
              ),
              capabilityState: {
                _tag: "Stopped",
                reason: "The debug service scope closed.",
                stoppedAt: terminatedAt,
              },
              stopped: true,
            });
            return [undefined, closed] as const;
          });
          yield* PubSub.shutdown(eventBus);
        }),
      );

      return service;
    }),
  );
