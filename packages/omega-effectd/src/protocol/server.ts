/**
 * Framed protocol server for supervised omega-effectd.
 *
 * Owns durable Full Auto registries under the injected data root.
 * Mutation API remains full-auto-run-actions. GPUI is not a caller here.
 */

import { createInterface } from "node:readline";

import {
  resolveAgentComputerSessionsPath,
  resolveFullAutoNativeBindingsPath,
  resolveFullAutoProviderHandoffsPath,
  resolveFullAutoRegistryPath,
  resolveFullAutoRunReportsPath,
  resolveFullAutoRunsPath,
  type OmegaEffectdPaths,
} from "../paths.ts";
import { openFullAutoRegistry } from "../engine/full-auto-registry.ts";
import {
  FULL_AUTO_RUN_ACTIVE_LIMIT,
  openFullAutoRunRegistry,
} from "../engine/full-auto-run-registry.ts";
import { openFullAutoRunReportStore } from "../engine/full-auto-run-report.ts";
import {
  assessFullAutoNativeBoundary,
  buildFullAutoNativeBinding,
  openFullAutoNativeBindingStore,
  projectFullAutoNativeEvidence,
} from "../engine/full-auto-native-binding.ts";
import {
  openAgentComputerSessionStore,
  refreshAgentComputerSession,
  runAgentComputerTurn,
  startAgentComputerSession,
} from "../engine/agent-computer-sessions.ts";
import type { FetchLike } from "@openagentsinc/agent-harness-environment";
import {
  FULL_AUTO_CONTROL_CALLER_LABEL,
  getFullAutoRunAction,
  getFullAutoRunReceiptAction,
  getFullAutoRunReportAction,
  handoffFullAutoRunAction,
  listFullAutoRunsAction,
  pauseFullAutoRunAction,
  resumeFullAutoRunAction,
  retryFullAutoRunNowAction,
  startFullAutoRunAction,
  stopFullAutoRunAction,
  type FullAutoRunActionContext,
} from "../engine/full-auto-run-actions.ts";
import {
  applyFullAutoRunControlIntent,
  type FullAutoRunControlAction,
} from "../engine/full-auto-run-control-intent.ts";
import type { FullAutoControlCapabilities } from "../engine/full-auto-control-server.ts";
import {
  decodeFullAutoControlRunHandoffRequest,
  decodeFullAutoControlRunStartRequest,
} from "../engine/full-auto-control-contract.ts";
import { FULL_AUTO_DEFAULT_LANE, FULL_AUTO_LANE_POLICIES } from "../engine/full-auto-lane.ts";
import {
  FULL_AUTO_MAX_CONCURRENT_RUNS,
  projectFullAutoCapacityLedger,
} from "../engine/full-auto-capacity.ts";
import {
  FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS,
  makeSerialTaskQueue,
  reconcileFullAutoThreads,
  type FullAutoDispatchFailureCause,
} from "../engine/full-auto-reconcile.ts";
import {
  decideFullAutoLivenessNotification,
  type FullAutoStallCause,
} from "../engine/full-auto-liveness.ts";
import type { FullAutoRotationReason } from "../engine/full-auto-registry.ts";
import type { FullAutoRunState } from "../engine/full-auto-run-registry.ts";
import { openProviderHandoffRegistry } from "../engine/full-auto-provider-handoff.ts";
import type { DesktopThread } from "../support/chat-contract.ts";
import {
  compileFullAutoMissionPacket,
  renderFullAutoMissionPrompt,
} from "../engine/full-auto-mission.ts";
import type { OmegaEffectdService } from "../service.ts";
import { Schema } from "effect";
import { LocalTurnRecordSchema, type LocalTurnRecord } from "../support/local-turn-journal.ts";
import {
  isOmegaEffectdRequest,
  isOmegaEffectdHostResponse,
  OMEGA_EFFECTD_MAX_FRAME_BYTES,
  OMEGA_EFFECTD_PROTOCOL_SCHEMA,
  OMEGA_EFFECTD_PROTOCOL_VERSION,
  OMEGA_EFFECTD_SERVICE_VERSION,
  redactDiagnosticText,
  type OmegaEffectdAttentionResult,
  type OmegaEffectdCapacityResult,
  type OmegaEffectdHealthResult,
  type OmegaEffectdHostRequest,
  type OmegaEffectdInitializeResult,
  type OmegaEffectdNativeBinding,
  type OmegaEffectdNativeEvidence,
  type OmegaEffectdProtocolError,
  type OmegaEffectdPublishProjectionResult,
  type OmegaEffectdResponse,
  type OmegaEffectdRunDetail,
  type OmegaEffectdSyncStatus,
} from "./framed.ts";
import {
  OmegaEffectdHostBridge,
  OmegaEffectdHostBridgeError,
  type OmegaEffectdHostFrameEmitter,
} from "./host-bridge.ts";

const OWNER_CONFIGURABLE_GUARDRAILS = Object.freeze([
  "maxWallClockMs",
  "maxTurns",
  "maxPerTurnFailures",
  "tokenBudgetRef",
] as const);

const redactedError = (
  code: OmegaEffectdProtocolError["code"],
  message: string,
): OmegaEffectdProtocolError => ({
  code,
  message: redactDiagnosticText(message),
});

export type OmegaEffectdFramedServer = Readonly<{
  generation: () => number;
  handleLine: (line: string) => Promise<OmegaEffectdResponse | null>;
  serveStdio: () => Promise<void>;
}>;

export type OmegaEffectdFramedServerOptions = Readonly<{
  /** Test-only fetch injection for Agent Computer Worker calls. */
  agentComputerFetch?: FetchLike;
  /** Test-only sleep injection for Agent Computer turn polling. */
  agentComputerSleep?: (durationMs: number) => import("effect").Effect.Effect<void>;
  emitHostRequest?: OmegaEffectdHostFrameEmitter;
  /** In-process host adapter used by protocol tests. Production uses framed stdio. */
  hostRequestHandler?: (request: OmegaEffectdHostRequest) => Promise<unknown>;
  /** Test-only override for the bounded host response deadline. */
  hostRequestTimeoutMs?: number;
}>;

type HostLiveState = Readonly<{
  state: "turn_running" | "turn_completed" | "blocked";
  turnRef: string | null;
  reason?: string;
}>;

type HostThreadEvidence = Readonly<{
  present: boolean;
  revision: number;
  live: HostLiveState | null;
  turns: ReadonlyArray<LocalTurnRecord>;
}>;

const objectResult = (value: unknown): Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new OmegaEffectdHostBridgeError(
      "invalid_response",
      "The Omega host returned an invalid result.",
    );
  }
  return value as Record<string, unknown>;
};

const requiredString = (value: unknown, field: string): string => {
  if (typeof value !== "string" || value.length === 0 || value.length > 180) {
    throw new OmegaEffectdHostBridgeError(
      "invalid_response",
      `The Omega host returned an invalid ${field}.`,
    );
  }
  return value;
};

const decodeLocalTurnRecord = Schema.decodeUnknownSync(LocalTurnRecordSchema);

export const createOmegaEffectdFramedServer = (
  service: OmegaEffectdService,
  paths: OmegaEffectdPaths,
  options: OmegaEffectdFramedServerOptions = {},
): OmegaEffectdFramedServer => {
  let generation = 0;
  let initialized = false;

  const runRegistry = openFullAutoRunRegistry(resolveFullAutoRunsPath(paths));
  const registry = openFullAutoRegistry(resolveFullAutoRegistryPath(paths));
  for (const run of runRegistry.list()) {
    if (run.threadRef === undefined || registry.record(run.threadRef) !== null) continue;
    const binding = run.executionHistory?.at(-1);
    if (binding === undefined || binding.targetThreadRef !== run.threadRef) continue;
    registry.transferThread(binding.sourceThreadRef, binding.targetThreadRef, binding.targetProfile);
  }
  const reportStore = openFullAutoRunReportStore(resolveFullAutoRunReportsPath(paths));
  const nativeBindings = openFullAutoNativeBindingStore(resolveFullAutoNativeBindingsPath(paths));
  const providerHandoffs = openProviderHandoffRegistry(resolveFullAutoProviderHandoffsPath(paths));
  const agentComputerSessions = openAgentComputerSessionStore(
    resolveAgentComputerSessionsPath(paths),
  );
  let hostBridge: OmegaEffectdHostBridge;
  const emitHostRequest =
    options.emitHostRequest ??
    (options.hostRequestHandler === undefined
      ? undefined
      : async (request: OmegaEffectdHostRequest) => {
          try {
            const result = await options.hostRequestHandler?.(request);
            hostBridge.accept({
              schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
              kind: "host_response",
              id: request.id,
              generation: request.generation,
              ok: true,
              result,
            });
          } catch (error) {
            hostBridge.accept({
              schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
              kind: "host_response",
              id: request.id,
              generation: request.generation,
              ok: false,
              error: {
                code: "internal",
                message: error instanceof Error ? error.message : "Host request failed.",
              },
            });
          }
        });
  hostBridge = new OmegaEffectdHostBridge(emitHostRequest, {
    requestTimeoutMs: options.hostRequestTimeoutMs,
  });
  const serializeReconciliation = makeSerialTaskQueue();
  const evidenceByThread = new Map<string, HostThreadEvidence>();
  const laneReadiness = new Map<string, boolean>();
  let resolvedWorkspaceRef: string | null = null;
  let preparedThreadRef: string | null = null;
  let preparedInterruptResult = false;
  let lastReconciliation: Promise<void> = Promise.resolve();
  const pendingNotes: Array<Readonly<{ threadRef: string; text: string }>> = [];

  const hostRequest = async (
    method: Parameters<OmegaEffectdHostBridge["request"]>[0],
    params: unknown,
  ) => hostBridge.request(method, params);

  const resolveWorkspace = async (expectedWorkspaceRef?: string): Promise<string> => {
    const result = objectResult(
      await hostRequest(
        "resolve_workspace",
        expectedWorkspaceRef === undefined ? {} : { expectedWorkspaceRef },
      ),
    );
    resolvedWorkspaceRef = requiredString(result.workspaceRef, "workspaceRef");
    return resolvedWorkspaceRef;
  };

  const refreshThreadEvidence = async (
    runRef: string,
    threadRef: string,
  ): Promise<HostThreadEvidence> => {
    const result = objectResult(await hostRequest("refresh_evidence", { runRef, threadRef }));
    const revision =
      typeof result.revision === "number" &&
      Number.isSafeInteger(result.revision) &&
      result.revision >= 0
        ? result.revision
        : 0;
    const previous = evidenceByThread.get(threadRef);
    if (previous !== undefined && revision < previous.revision) return previous;
    const liveValue = result.live;
    const liveRecord =
      liveValue !== null && typeof liveValue === "object"
        ? (liveValue as Record<string, unknown>)
        : null;
    const live =
      liveRecord !== null &&
      (liveRecord.state === "turn_running" ||
        liveRecord.state === "turn_completed" ||
        liveRecord.state === "blocked") &&
      (liveRecord.turnRef === null || typeof liveRecord.turnRef === "string")
        ? ({
            state: liveRecord.state,
            turnRef: liveRecord.turnRef,
            ...(typeof liveRecord.reason === "string" ? { reason: liveRecord.reason } : {}),
          } satisfies HostLiveState)
        : null;
    let turns: ReadonlyArray<LocalTurnRecord> = [];
    if (Array.isArray(result.turns)) {
      try {
        turns = result.turns.slice(0, 128).map((turn) => decodeLocalTurnRecord(turn));
      } catch {
        throw new OmegaEffectdHostBridgeError(
          "invalid_response",
          "The Omega host returned invalid turn evidence.",
        );
      }
    }
    const evidence: HostThreadEvidence = {
      present: result.present === true,
      revision,
      live,
      turns,
    };
    evidenceByThread.set(threadRef, evidence);
    return evidence;
  };

  const probeLane = async (lane: string, threadRef?: string): Promise<boolean> => {
    const result = objectResult(
      await hostRequest("lane_readiness", {
        lane,
        ...(threadRef === undefined ? {} : { excludingThreadRef: threadRef }),
      }),
    );
    const ready =
      result.known === true &&
      result.admitted === true &&
      result.fullAuto === true &&
      result.state === "available";
    return ready;
  };

  const refreshLane = async (lane: string, threadRef?: string): Promise<boolean> => {
    const expectedGeneration = generation;
    const ready = await probeLane(lane, threadRef);
    if (generation !== expectedGeneration) {
      throw new OmegaEffectdHostBridgeError(
        "stale_generation",
        "The supervisor generation changed before lane readiness was recorded.",
      );
    }
    laneReadiness.set(lane, ready);
    return ready;
  };

  const refreshCapacityReadiness = async (expectedGeneration: number): Promise<void> => {
    const readiness = await Promise.all(
      Object.keys(FULL_AUTO_LANE_POLICIES)
        .sort()
        .map(async (lane) => [lane, await probeLane(lane)] as const),
    );
    if (generation !== expectedGeneration) {
      throw new OmegaEffectdHostBridgeError(
        "stale_generation",
        "The supervisor generation changed before capacity readiness was recorded.",
      );
    }
    for (const [lane, ready] of readiness) laneReadiness.set(lane, ready);
  };

  const flushNotes = async (): Promise<void> => {
    const notes = pendingNotes.splice(0, pendingNotes.length);
    await Promise.all(
      notes.map((note) =>
        hostRequest("append_system_note", {
          threadRef: note.threadRef,
          noteRef: `note.${generation}.${Date.now().toString(36)}`,
          text: note.text,
        }),
      ),
    );
  };

  const projectDetail = (
    ctx: FullAutoRunActionContext,
    runRef: string,
  ): OmegaEffectdRunDetail | null => {
    const outcome = getFullAutoRunAction(ctx, runRef);
    if (!outcome.ok) return null;
    const run = outcome.value;
    const report = getFullAutoRunReportAction(ctx, runRef);
    const turns = report.ok
      ? report.value.turns.map((turn) => ({
          turnRef: turn.turnRef,
          lane: turn.lane,
          outcomeSummary: turn.outcomeSummary,
          createdAt: turn.createdAt,
        }))
      : [];
    const binding = nativeBindings.get(runRef);
    const nativeEvidence: OmegaEffectdNativeEvidence | null = binding
      ? projectFullAutoNativeEvidence(binding)
      : null;
    return {
      runRef: run.runRef,
      threadRef: run.threadRef ?? null,
      state: run.state,
      title: run.title,
      objective: run.objective,
      doneCondition: run.doneCondition,
      workspaceRef: run.workspaceRef ?? null,
      lane: run.lane ?? null,
      turnCap: run.turnCap,
      successfulAttempts: run.successfulAttempts,
      failedAttempts: run.failedAttempts,
      stallCause: run.stallCause ?? null,
      recoveryAction: run.recoveryAction,
      terminalReason: run.terminalReason ?? null,
      updatedAt: run.lastProgressAt ?? run.createdAt,
      nativeEvidence,
      turns,
    };
  };
  const runReconciliation = (): Promise<void> =>
    serializeReconciliation(async () => {
      const workspaceRef = await resolveWorkspace();
      const enabledThreads = registry.enabledThreads();
      await Promise.all(
        enabledThreads.map(async (threadRef) => {
          const run = runRegistry.findByThreadRef(threadRef);
          if (run !== null) {
            const evidence = await refreshThreadEvidence(run.runRef, threadRef);
            if (!evidence.present) {
              registry.recordFailure(threadRef, "host_thread_missing");
              registry.set(threadRef, false, {
                blockedReason: "host_thread_missing",
                disabledBy: "dispatch_failure_limit",
              });
              runRegistry.transition(run.runRef, {
                to: "stalled",
                actor: "liveness_monitor",
                reason: "host_thread_missing",
              });
            }
          }
          const lane = registry.record(threadRef)?.profile?.lane ?? FULL_AUTO_DEFAULT_LANE;
          await refreshLane(lane, threadRef);
        }),
      );
      await reconcileFullAutoThreads({
        registry,
        nonterminalThreadRefs: () =>
          new Set(
            [...evidenceByThread.entries()]
              .filter(([, evidence]) => evidence.turns.some((turn) => turn.disposition === null))
              .map(([threadRef]) => threadRef),
          ),
        resolveWorkspaceRef: () => workspaceRef,
        journalHasNonterminalTurn: (turnRef) =>
          [...evidenceByThread.values()].some((evidence) =>
            evidence.turns.some((turn) => turn.turnRef === turnRef && turn.disposition === null),
          ),
        laneReady: ({ lane }) => laneReadiness.get(lane ?? FULL_AUTO_DEFAULT_LANE) === true,
        turnEvidence: (threadRef) =>
          (evidenceByThread.get(threadRef)?.turns ?? []).map((turn) => ({
            disposition: turn.disposition,
            updatedAt: turn.updatedAt,
          })),
        compileDispatchMessage: (input) => {
          const run = runRegistry.findByThreadRef(input.threadRef);
          const priorAcceptedOutcome = [...(evidenceByThread.get(input.threadRef)?.turns ?? [])]
            .reverse()
            .find((turn) => turn.disposition === "completed") ?? null;
          const previousHandoff = run === null
            ? null
            : [...providerHandoffs.list({ runRef: run.runRef })]
                .reverse()
                .find((handoff) => handoff.disposition !== "refused") ?? null;
          return renderFullAutoMissionPrompt(
            compileFullAutoMissionPacket({
              run,
              record: input.record,
              threadRef: input.threadRef,
              profile: input.profile,
              turnCap: input.turnCap,
              priorAcceptedOutcome,
              previousHandoff,
            }),
          );
        },
        dispatch: async (input) => {
          const run = runRegistry.findByThreadRef(input.threadRef);
          if (run === null)
            return {
              ok: false,
              reason: "host_thread_missing",
              failureCause: "host_thread_missing",
            };
          const result = objectResult(
            await hostRequest("dispatch_turn", {
              runRef: run.runRef,
              workspaceRef,
              ...input,
            }),
          );
          const failureCause =
            typeof result.failureCause === "string"
              ? (result.failureCause as FullAutoDispatchFailureCause)
              : undefined;
          return result.accepted === true
            ? { ok: true }
            : {
                ok: false,
                reason: typeof result.reason === "string" ? result.reason : "dispatch_rejected",
                ...(failureCause === undefined ? {} : { failureCause }),
              };
        },
        onDispatched: (threadRef, result) => {
          const run = runRegistry.findByThreadRef(threadRef);
          if (run !== null) runRegistry.recordAttempt(run.runRef, "success", result);
        },
        onDispatchFailed: (threadRef, failure) => {
          const run = runRegistry.findByThreadRef(threadRef);
          if (run !== null)
            runRegistry.recordAttempt(run.runRef, "failure", { reason: failure.reason });
        },
      });
      await flushNotes();
    });

  const capabilities: FullAutoControlCapabilities = {
    registry,
    runRegistry,
    reportStore,
    resolveWorkspaceRef: () => resolvedWorkspaceRef ?? "",
    triggerReconciliation: () => {
      lastReconciliation = runReconciliation();
      return lastReconciliation;
    },
    liveState: (threadRef) => evidenceByThread.get(threadRef)?.live ?? null,
    listTurns: (threadRef) => evidenceByThread.get(threadRef)?.turns ?? [],
    appendSystemNote: () => {},
    createThread: () => {
      if (preparedThreadRef === null)
        throw new OmegaEffectdHostBridgeError("host_unavailable", "No host thread was prepared.");
      const threadRef = preparedThreadRef;
      preparedThreadRef = null;
      return threadRef;
    },
    prepareHandoffThread: async (input) => {
      const result = objectResult(
        await hostRequest("create_thread", {
          title: `${input.title} — ${input.targetLaneRef}`,
          lane: input.targetLaneRef,
          workspaceRef: input.workspaceRef,
          operationRef: `${input.runRef}.handoff.${input.targetLaneRef}`,
        }),
      );
      return requiredString(result.threadRef, "threadRef");
    },
    isLaneEligible: (laneRef) => laneReadiness.get(laneRef) === true,
    listLanes: async () => [],
    providerLaneRegistry: {
      switchThread: (request) => {
        if (!(request.laneRef in FULL_AUTO_LANE_POLICIES)) {
          return {
            ok: false,
            reason: "unknown_lane",
            message: "That provider lane is not registered.",
            missingCapabilities: [],
          };
        }
        if (laneReadiness.get(request.laneRef) !== true) {
          return {
            ok: false,
            reason: "missing_auth",
            message: "That provider lane has no verified authentication.",
            missingCapabilities: [],
          };
        }
        if (request.thread === null) {
          return {
            ok: false,
            reason: "thread_not_found",
            message: "That thread does not exist.",
            missingCapabilities: [],
          };
        }
        return {
          ok: true,
          threadRef: request.threadRef,
          laneRef: request.laneRef,
          previousLaneRef:
            registry.record(request.threadRef)?.profile?.lane ?? FULL_AUTO_DEFAULT_LANE,
          history: [],
          truncated: false,
        };
      },
    },
    getThread: (threadRef): DesktopThread | null => {
      const evidence = evidenceByThread.get(threadRef);
      if (evidence?.present !== true) return null;
      const run = runRegistry.findByThreadRef(threadRef);
      if (run === null) return null;
      return {
        id: threadRef,
        title: run.title,
        updatedAt: run.lastProgressAt ?? run.createdAt,
        notes: evidence.turns
          .filter((turn) => turn.assistantText.trim() !== "")
          .map((turn) => ({
            key: turn.assistantMessageKey,
            role: "assistant" as const,
            text: turn.assistantText,
            timestamp: turn.updatedAt,
          })),
      };
    },
    providerHandoffRegistry: providerHandoffs,
    interruptLiveTurn: () => {
      const interrupted = preparedInterruptResult;
      preparedInterruptResult = false;
      return interrupted;
    },
  };

  const actionContext = (): FullAutoRunActionContext => ({
    capabilities,
    now: () => new Date(),
    actor: "control_api",
    callerLabel: FULL_AUTO_CONTROL_CALLER_LABEL,
  });

  const mobileActionContext = (): FullAutoRunActionContext => ({
    capabilities,
    now: () => new Date(),
    actor: "mobile",
    callerLabel: "mobile control intent",
  });

  const syncStatus = (): OmegaEffectdSyncStatus => ({
    available: false,
    publishBlocksDispatch: false,
    reason: "omega_khala_sync_session_unavailable",
  });

  const projectCapacity = (): OmegaEffectdCapacityResult => {
    const active = runRegistry.activeRuns();
    const coolingByLane = new Map<string, FullAutoRotationReason>();
    for (const record of registry.list()) {
      const history = record.rotationHistory ?? [];
      const last = history[history.length - 1];
      if (last === undefined) continue;
      if (
        last.reason === "account_exhausted" ||
        last.reason === "rate_limited" ||
        last.reason === "provider_error"
      ) {
        coolingByLane.set(last.toLane, last.reason);
      }
    }
    const lanes = projectFullAutoCapacityLedger({
      laneGate: (laneRef) => {
        if (!(laneRef in FULL_AUTO_LANE_POLICIES)) return null;
        const eligible =
          capabilities.isLaneEligible?.(laneRef) ?? laneRef === FULL_AUTO_DEFAULT_LANE;
        return eligible ? { admitted: true, fullAuto: true } : { admitted: false, fullAuto: false };
      },
      activeRunsByLane: (lane) =>
        active.filter((run) => (run.profile?.lane ?? null) === lane).length,
      coolingReasonByLane: (lane) => coolingByLane.get(lane) ?? null,
    });
    return {
      activeRunLimit: FULL_AUTO_MAX_CONCURRENT_RUNS,
      activeRunCount: active.length,
      lanes,
      nonOverridableGuardrails: [...FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS],
      ownerConfigurableGuardrails: [...OWNER_CONFIGURABLE_GUARDRAILS],
      enabledThreadsNeverEvicted: true,
    };
  };

  const respond = (
    id: string,
    ok: boolean,
    result?: unknown,
    error?: OmegaEffectdProtocolError,
  ): OmegaEffectdResponse => ({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "response",
    id,
    generation,
    ok,
    ...(result === undefined ? {} : { result }),
    ...(error === undefined ? {} : { error }),
  });

  const requireGeneration = (
    requestGeneration: number,
    id: string,
  ): OmegaEffectdResponse | null => {
    if (!initialized) {
      return respond(
        id,
        false,
        undefined,
        redactedError("invalid_request", "Call initialize first."),
      );
    }
    if (requestGeneration !== generation) {
      return respond(
        id,
        false,
        undefined,
        redactedError(
          "stale_generation",
          `Expected generation ${generation}, got ${requestGeneration}.`,
        ),
      );
    }
    return null;
  };

  const handle = async (request: {
    id: string;
    generation: number;
    method: string;
    params?: unknown;
  }): Promise<OmegaEffectdResponse> => {
    if (request.method === "initialize") {
      const params = (request.params ?? {}) as { generation?: number };
      if (
        typeof params.generation !== "number" ||
        !Number.isInteger(params.generation) ||
        params.generation < 1
      ) {
        return respond(
          request.id,
          false,
          undefined,
          redactedError("invalid_request", "initialize requires integer generation >= 1."),
        );
      }
      if (initialized && params.generation <= generation) {
        return respond(
          request.id,
          false,
          undefined,
          redactedError(
            "stale_generation",
            "initialize requires a generation newer than the active generation.",
          ),
        );
      }
      generation = params.generation;
      hostBridge.beginGeneration(generation);
      initialized = true;
      await service.start();
      const health = service.health();
      const result: OmegaEffectdInitializeResult = {
        schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
        protocolVersion: OMEGA_EFFECTD_PROTOCOL_VERSION,
        serviceVersion: OMEGA_EFFECTD_SERVICE_VERSION,
        generation,
        capabilities: [
          "health",
          "list_runs",
          "get_run",
          "start",
          "pause",
          "resume",
          "handoff",
          "stop",
          "retry",
          "get_capacity",
          "decide_attention",
          "get_report",
          "get_receipt",
          "apply_control_intent",
          "get_sync_status",
          "publish_projection",
          "get_native_binding",
          "assess_native_boundary",
          "start_agent_computer_session",
          "refresh_agent_computer_session",
          "run_agent_computer_turn",
          "get_agent_computer_session",
          "list_agent_computer_sessions",
          "host_bridge",
        ],
        dataRoot: health.dataRoot,
        activeRunLimit: FULL_AUTO_RUN_ACTIVE_LIMIT,
      };
      return respond(request.id, true, result);
    }

    const fence = requireGeneration(request.generation, request.id);
    if (fence) return fence;

    if (service.health().status !== "running") {
      return respond(
        request.id,
        false,
        undefined,
        redactedError("not_running", "omega-effectd is stopped."),
      );
    }

    switch (request.method) {
      case "health": {
        const health = service.health();
        const result: OmegaEffectdHealthResult = {
          ok: true,
          status: health.status,
          generation,
          dataRoot: health.dataRoot,
          activeRunCount: runRegistry.activeRuns().length,
        };
        return respond(request.id, true, result);
      }
      case "list_runs": {
        const runs = listFullAutoRunsAction(actionContext()).map((run) => ({
          runRef: run.runRef,
          threadRef: run.threadRef,
          state: run.state,
          title: run.title,
          updatedAt: run.lastProgressAt ?? run.createdAt,
        }));
        return respond(request.id, true, { runs });
      }
      case "get_run": {
        const params = (request.params ?? {}) as { runRef?: string };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_run requires runRef."),
          );
        }
        const detail = projectDetail(actionContext(), params.runRef);
        if (detail === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", "No Full Auto run exists for that runRef."),
          );
        }
        return respond(request.id, true, { run: detail });
      }
      case "start": {
        const raw = (request.params ?? {}) as {
          projectRef?: string;
          worktreeRef?: string;
          worktreeAbsolutePath?: string;
          gitHead?: string;
          rebaseUnsafe?: boolean;
        };
        const body = decodeFullAutoControlRunStartRequest(request.params ?? {});
        if (body === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "start requires workspaceRef, title, objective, and doneCondition.",
            ),
          );
        }
        if (raw.rebaseUnsafe === true) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "rebase_unsafe: refusing to start Full Auto on a rebase-unsafe worktree.",
            ),
          );
        }
        const workspaceRef = await resolveWorkspace(body.workspaceRef);
        if (workspaceRef !== body.workspaceRef) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "The requested workspace does not match the Omega host workspace.",
            ),
          );
        }
        const lane = body.lane ?? FULL_AUTO_DEFAULT_LANE;
        if (!(await refreshLane(lane))) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", `Provider lane ${lane} is not ready for Full Auto.`),
          );
        }
        const created = objectResult(
          await hostRequest("create_thread", {
            title: body.title,
            lane,
            workspaceRef,
            operationRef: `start.${generation}.${request.id}`,
          }),
        );
        preparedThreadRef = requiredString(created.threadRef, "threadRef");
        const outcome = startFullAutoRunAction(actionContext(), body);
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.error.message),
          );
        }
        await lastReconciliation;
        await flushNotes();
        if (
          typeof raw.projectRef === "string" &&
          raw.projectRef.length > 0 &&
          typeof raw.worktreeRef === "string" &&
          raw.worktreeRef.length > 0
        ) {
          nativeBindings.put(
            buildFullAutoNativeBinding({
              runRef: outcome.value.runRef,
              workspaceRef: body.workspaceRef,
              projectRef: raw.projectRef,
              worktreeRef: raw.worktreeRef,
              worktreeAbsolutePath: raw.worktreeAbsolutePath,
              gitHead: raw.gitHead,
              rebaseUnsafe: false,
            }),
          );
        }
        const detail = projectDetail(actionContext(), outcome.value.runRef);
        return respond(request.id, true, { run: detail });
      }
      case "get_capacity": {
        await refreshCapacityReadiness(request.generation);
        return respond(request.id, true, projectCapacity());
      }
      case "decide_attention": {
        const params = (request.params ?? {}) as {
          runRef?: string;
          permissionGranted?: boolean;
          previousDedupKey?: string | null;
        };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "decide_attention requires runRef."),
          );
        }
        const detail = projectDetail(actionContext(), params.runRef);
        if (detail === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", "No Full Auto run exists for that runRef."),
          );
        }
        const decision = decideFullAutoLivenessNotification({
          runRef: detail.runRef,
          runTitle: detail.title,
          projectedState: detail.state as FullAutoRunState,
          cause: (detail.stallCause as FullAutoStallCause | null) ?? null,
          previousDedupKey:
            typeof params.previousDedupKey === "string" ? params.previousDedupKey : null,
          permissionGranted: params.permissionGranted === true,
        });
        const result: OmegaEffectdAttentionResult = decision;
        return respond(request.id, true, { attention: result });
      }
      case "get_report": {
        const params = (request.params ?? {}) as { runRef?: string };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_report requires runRef."),
          );
        }
        const outcome = getFullAutoRunReportAction(actionContext(), params.runRef);
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", outcome.error.message),
          );
        }
        return respond(request.id, true, { report: outcome.value });
      }
      case "get_receipt": {
        const params = (request.params ?? {}) as { runRef?: string };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_receipt requires runRef."),
          );
        }
        const outcome = getFullAutoRunReceiptAction(actionContext(), params.runRef);
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", outcome.error.message),
          );
        }
        return respond(request.id, true, { receipt: outcome.value });
      }
      case "apply_control_intent": {
        const params = (request.params ?? {}) as {
          intentId?: string;
          runRef?: string;
          action?: string;
        };
        if (
          typeof params.intentId !== "string" ||
          params.intentId.length === 0 ||
          typeof params.runRef !== "string" ||
          params.runRef.length === 0 ||
          (params.action !== "pause" && params.action !== "resume" && params.action !== "stop")
        ) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "apply_control_intent requires intentId, runRef, and action pause|resume|stop.",
            ),
          );
        }
        const outcome = applyFullAutoRunControlIntent(mobileActionContext(), {
          intentId: params.intentId,
          runRef: params.runRef,
          action: params.action as FullAutoRunControlAction,
        });
        return respond(request.id, true, { outcome });
      }
      case "get_sync_status": {
        return respond(request.id, true, syncStatus());
      }
      case "publish_projection": {
        const params = (request.params ?? {}) as { runRef?: string };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "publish_projection requires runRef."),
          );
        }
        const run = getFullAutoRunAction(actionContext(), params.runRef);
        if (!run.ok) {
          const result: OmegaEffectdPublishProjectionResult = {
            ok: false,
            status: "run_not_found",
            reason: "No Full Auto run exists for that runRef.",
          };
          return respond(request.id, true, result);
        }
        // Honest stub: Sync publish never blocks local dispatch (exit criterion).
        const result: OmegaEffectdPublishProjectionResult = {
          ok: false,
          status: "sync_unavailable",
          reason: syncStatus().reason,
        };
        return respond(request.id, true, result);
      }
      case "get_native_binding": {
        const params = (request.params ?? {}) as { runRef?: string };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_native_binding requires runRef."),
          );
        }
        const run = getFullAutoRunAction(actionContext(), params.runRef);
        if (!run.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", "No Full Auto run exists for that runRef."),
          );
        }
        const binding = nativeBindings.get(params.runRef);
        return respond(request.id, true, {
          binding: (binding as OmegaEffectdNativeBinding | null) ?? null,
        });
      }
      case "assess_native_boundary": {
        const params = (request.params ?? {}) as {
          runRef?: string;
          currentWorktreePathDigest?: string;
        };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "assess_native_boundary requires runRef."),
          );
        }
        const run = getFullAutoRunAction(actionContext(), params.runRef);
        if (!run.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("run_not_found", "No Full Auto run exists for that runRef."),
          );
        }
        await resolveWorkspace(run.value.workspaceRef ?? undefined);
        const assessment = assessFullAutoNativeBoundary({
          binding: nativeBindings.get(params.runRef),
          expectedWorkspaceRef: capabilities.resolveWorkspaceRef(),
          currentWorktreePathDigest: params.currentWorktreePathDigest,
        });
        return respond(request.id, true, { assessment });
      }
      case "start_agent_computer_session": {
        const params = (request.params ?? {}) as {
          bearerToken?: string;
          controlPlaneBaseUrl?: string;
          repoRef?: string;
          objective?: string;
          adapter?: "codex" | "claude_agent";
          lane?: "cloud-gcp";
          verify?: ReadonlyArray<string>;
        };
        if (
          typeof params.bearerToken !== "string" ||
          typeof params.controlPlaneBaseUrl !== "string" ||
          typeof params.repoRef !== "string" ||
          typeof params.objective !== "string"
        ) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "start_agent_computer_session requires bearerToken, controlPlaneBaseUrl, repoRef, and objective.",
            ),
          );
        }
        const outcome = await startAgentComputerSession(agentComputerSessions, {
          bearerToken: params.bearerToken,
          controlPlaneBaseUrl: params.controlPlaneBaseUrl,
          repoRef: params.repoRef,
          objective: params.objective,
          ...(params.adapter === undefined ? {} : { adapter: params.adapter }),
          ...(params.lane === undefined ? {} : { lane: params.lane }),
          ...(params.verify === undefined ? {} : { verify: params.verify }),
          ...(options.agentComputerFetch === undefined
            ? {}
            : { fetch: options.agentComputerFetch }),
        });
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.message),
          );
        }
        return respond(request.id, true, { session: outcome.session });
      }
      case "refresh_agent_computer_session": {
        const params = (request.params ?? {}) as {
          bearerToken?: string;
          sessionRef?: string;
        };
        if (typeof params.bearerToken !== "string" || typeof params.sessionRef !== "string") {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "refresh_agent_computer_session requires bearerToken and sessionRef.",
            ),
          );
        }
        const existing = agentComputerSessions.get(params.sessionRef);
        if (existing === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "No Agent Computer session exists for that sessionRef.",
            ),
          );
        }
        const outcome = await refreshAgentComputerSession(agentComputerSessions, {
          bearerToken: params.bearerToken,
          session: existing,
          ...(options.agentComputerFetch === undefined
            ? {}
            : { fetch: options.agentComputerFetch }),
        });
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.message),
          );
        }
        return respond(request.id, true, { session: outcome.session });
      }
      case "run_agent_computer_turn": {
        const params = (request.params ?? {}) as {
          bearerToken?: string;
          controlPlaneBaseUrl?: string;
          repoRef?: string;
          objective?: string;
          adapter?: "codex" | "claude_agent";
          lane?: "cloud-gcp";
          verify?: ReadonlyArray<string>;
        };
        if (
          typeof params.bearerToken !== "string" ||
          typeof params.controlPlaneBaseUrl !== "string" ||
          typeof params.repoRef !== "string" ||
          typeof params.objective !== "string"
        ) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError(
              "invalid_request",
              "run_agent_computer_turn requires bearerToken, controlPlaneBaseUrl, repoRef, and objective.",
            ),
          );
        }
        const outcome = await runAgentComputerTurn(agentComputerSessions, {
          bearerToken: params.bearerToken,
          controlPlaneBaseUrl: params.controlPlaneBaseUrl,
          repoRef: params.repoRef,
          objective: params.objective,
          ...(params.adapter === undefined ? {} : { adapter: params.adapter }),
          ...(params.lane === undefined ? {} : { lane: params.lane }),
          ...(params.verify === undefined ? {} : { verify: params.verify }),
          ...(options.agentComputerFetch === undefined
            ? {}
            : { fetch: options.agentComputerFetch }),
          ...(options.agentComputerSleep === undefined
            ? {}
            : { sleep: options.agentComputerSleep }),
          ...(options.agentComputerFetch === undefined
            ? {}
            : { pollIntervalMs: 1, maxPollAttempts: 120 }),
        });
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.message),
          );
        }
        return respond(request.id, true, {
          session: outcome.session,
          finishReason: outcome.finishReason,
          eventKinds: outcome.eventKinds,
        });
      }
      case "get_agent_computer_session": {
        const params = (request.params ?? {}) as { sessionRef?: string };
        if (typeof params.sessionRef !== "string" || params.sessionRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "get_agent_computer_session requires sessionRef."),
          );
        }
        return respond(request.id, true, {
          session: agentComputerSessions.get(params.sessionRef),
        });
      }
      case "list_agent_computer_sessions": {
        return respond(request.id, true, {
          sessions: agentComputerSessions.list(),
        });
      }
      case "retry": {
        const params = (request.params ?? {}) as { runRef?: string };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "retry requires runRef."),
          );
        }
        const retryRun = runRegistry.get(params.runRef);
        if (retryRun?.threadRef !== undefined) {
          await resolveWorkspace(retryRun.workspaceRef);
          await refreshThreadEvidence(retryRun.runRef, retryRun.threadRef);
          await refreshLane(retryRun.profile?.lane ?? FULL_AUTO_DEFAULT_LANE, retryRun.threadRef);
        }
        const outcome = retryFullAutoRunNowAction(actionContext(), params.runRef);
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.error.message),
          );
        }
        await lastReconciliation;
        await flushNotes();
        const detail = projectDetail(actionContext(), params.runRef);
        return respond(request.id, true, { run: detail });
      }
      case "pause":
      case "resume":
      case "stop": {
        const params = (request.params ?? {}) as { runRef?: string };
        if (typeof params.runRef !== "string" || params.runRef.length === 0) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", `${request.method} requires runRef.`),
          );
        }
        const targetRun = runRegistry.get(params.runRef);
        if (targetRun?.threadRef !== undefined) {
          const evidence = await refreshThreadEvidence(targetRun.runRef, targetRun.threadRef);
          if (request.method === "resume") {
            await resolveWorkspace(targetRun.workspaceRef);
            await refreshLane(
              targetRun.profile?.lane ?? FULL_AUTO_DEFAULT_LANE,
              targetRun.threadRef,
            );
          }
          if (request.method === "stop" && evidence.live?.state === "turn_running") {
            const interrupted = objectResult(
              await hostRequest("interrupt_turn", {
                threadRef: targetRun.threadRef,
                ...(evidence.live.turnRef === null ? {} : { turnRef: evidence.live.turnRef }),
              }),
            );
            preparedInterruptResult = interrupted.interrupted === true;
          }
        }
        const outcome =
          request.method === "pause"
            ? pauseFullAutoRunAction(actionContext(), params.runRef)
            : request.method === "resume"
              ? resumeFullAutoRunAction(actionContext(), params.runRef)
              : stopFullAutoRunAction(actionContext(), params.runRef);
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.error.message),
          );
        }
        if (request.method === "resume") await lastReconciliation;
        await flushNotes();
        const detail = projectDetail(actionContext(), params.runRef);
        return respond(request.id, true, {
          run: detail,
        });
      }
      case "handoff": {
        const params = (request.params ?? {}) as {
          runRef?: string;
          targetLaneRef?: string;
        };
        const decodedBody = decodeFullAutoControlRunHandoffRequest(request.params ?? {});
        const body =
          decodedBody === null
            ? null
            : {
                ...decodedBody,
                ...(decodedBody.reason === undefined
                  ? {}
                  : { reason: redactDiagnosticText(decodedBody.reason) }),
              };
        if (typeof params.runRef !== "string" || params.runRef.length === 0 || body === null) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", "handoff requires runRef and targetLaneRef."),
          );
        }
        const targetRun = runRegistry.get(params.runRef);
        if (targetRun?.threadRef !== undefined) {
          await refreshThreadEvidence(targetRun.runRef, targetRun.threadRef);
          await refreshLane(body.targetLaneRef, targetRun.threadRef);
        }
        const outcome = await handoffFullAutoRunAction(actionContext(), params.runRef, body);
        if (!outcome.ok) {
          return respond(
            request.id,
            false,
            undefined,
            redactedError("invalid_request", outcome.error.message),
          );
        }
        const detail = projectDetail(actionContext(), params.runRef);
        return respond(request.id, true, {
          run: detail,
          transition: outcome.value.transition,
        });
      }
      default:
        return respond(
          request.id,
          false,
          undefined,
          redactedError("unknown_method", `Unknown method ${request.method}.`),
        );
    }
  };

  const handleLine = async (line: string): Promise<OmegaEffectdResponse | null> => {
    if (Buffer.byteLength(line, "utf8") > OMEGA_EFFECTD_MAX_FRAME_BYTES) {
      hostBridge.rejectPending(
        "frame_too_large",
        "An inbound Omega host frame exceeded the protocol limit.",
      );
      return respond(
        "invalid",
        false,
        undefined,
        redactedError("frame_too_large", "Frame exceeded the 64 KiB protocol limit."),
      );
    }
    const trimmed = line.trim();
    if (trimmed.length === 0) return null;
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      return respond(
        "invalid",
        false,
        undefined,
        redactedError("invalid_request", "Frame was not valid JSON."),
      );
    }
    if (isOmegaEffectdHostResponse(parsed)) {
      hostBridge.accept(parsed);
      return null;
    }
    if (!isOmegaEffectdRequest(parsed)) {
      return respond(
        typeof (parsed as { id?: unknown })?.id === "string"
          ? (parsed as { id: string }).id
          : "invalid",
        false,
        undefined,
        redactedError("invalid_request", "Frame was not an omega-effectd request."),
      );
    }
    try {
      return await handle(parsed);
    } catch (error) {
      if (error instanceof OmegaEffectdHostBridgeError) {
        const code =
          error.reason === "stale_generation"
            ? "stale_generation"
            : error.reason === "timeout"
              ? "host_timeout"
              : error.reason === "frame_too_large"
                ? "frame_too_large"
                : "host_unavailable";
        return respond(parsed.id, false, undefined, redactedError(code, error.message));
      }
      const message = error instanceof Error ? error.message : "internal error";
      return respond(parsed.id, false, undefined, redactedError("internal", message));
    }
  };

  return {
    generation: () => generation,
    handleLine,
    serveStdio: async () => {
      let writeTail = Promise.resolve();
      const writeFrame = (frame: OmegaEffectdHostRequest | OmegaEffectdResponse): Promise<void> => {
        writeTail = writeTail.then(
          () =>
            new Promise<void>((resolve, reject) => {
              process.stdout.write(`${JSON.stringify(frame)}\n`, (error) =>
                error ? reject(error) : resolve(),
              );
            }),
        );
        return writeTail;
      };
      hostBridge.setEmitter(writeFrame);
      const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
      const active = new Set<Promise<void>>();
      for await (const line of rl) {
        const task = handleLine(line)
          .then((response) => (response === null ? undefined : writeFrame(response)))
          .then(() => undefined);
        active.add(task);
        void task.finally(() => active.delete(task));
      }
      await Promise.all(active);
      await writeTail;
    },
  };
};
