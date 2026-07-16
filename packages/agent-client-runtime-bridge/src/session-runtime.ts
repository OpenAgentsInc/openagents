import { createHash, randomUUID } from "node:crypto";

import {
  decodeStableAcpDefinition,
  decodeStableAcpMethodPayload,
  type ContentBlock,
  type McpServer,
  type SessionConfigOption,
  type SessionModeState,
} from "@openagentsinc/agent-client-protocol/stable";
import { decodeUnstableAcpDefinition } from "@openagentsinc/agent-client-protocol/unstable";
import {
  buildAdmittedLaunchEnvironment,
  type AcpAdmittedSessionLaunch,
} from "@openagentsinc/agent-client-protocol/profiles";
import {
  AgentStdioTransport,
  type AgentStdioTransportLimits,
} from "@openagentsinc/agent-stdio-transport";

import type { AcpPeerProfile } from "./native-envelope.ts";

type JsonObject = Record<string, unknown>;
const object = (value: unknown): JsonObject =>
  value !== null && typeof value === "object" && !Array.isArray(value) ? (value as JsonObject) : {};
const present = (value: unknown): boolean => value !== undefined && value !== null;
const hashRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`;

export type AcpSessionRuntimeState =
  | "idle"
  | "starting"
  | "ready"
  | "recovering"
  | "stopping"
  | "stopped"
  | "failed";

export type AcpSessionPhase = "replay" | "live" | "closed";
export type AcpTurnTerminal =
  | "completed"
  | "cancelled"
  | "refused"
  | "timed_out"
  | "process_exit"
  | "protocol_failure";

export type AcpLifecycleFailure =
  | "unsupported"
  | "refused"
  | "timed_out"
  | "cancelled"
  | "process_exit"
  | "protocol_failure"
  | "invalid_state"
  | "invalid_value"
  | "missing_session"
  | "auth_required"
  | "auth_lost"
  | "missing_binary"
  | "protocol_drift"
  | "incompatible_version"
  | "restart_budget_exhausted";

export type AcpLifecycleOutcome<Value = undefined> =
  | Readonly<{ ok: true; value: Value; receipt: AcpLifecycleReceipt }>
  | Readonly<{
      ok: false;
      reason: AcpLifecycleFailure;
      safeDetail: string;
      receipt: AcpLifecycleReceipt;
    }>;

export type AcpLifecycleReceipt = Readonly<{
  at: string;
  runtimeGeneration: number;
  sessionGeneration?: number;
  turnGeneration?: number;
  method: string;
  outcome: "started" | "succeeded" | AcpLifecycleFailure | AcpTurnTerminal;
  latencyMs?: number;
  phase?: AcpSessionPhase;
  stopReasonRef?: string;
  cancelSource?: AcpCancelSource;
  recoveryDecision?: AcpRecoveryDecision;
  evidenceRefs: ReadonlyArray<string>;
}>;

export type AcpRuntimeEvidence = Readonly<{
  schemaRelease: "schema-v1.19.0";
  wireVersion: 1;
  runtimeGeneration: number;
  connectionRef: string;
  profile: AcpPeerProfile;
  peer: Readonly<{ name: string; version: string }>;
  capabilities: Readonly<{
    load: boolean;
    list: boolean;
    delete: boolean;
    resume: boolean;
    close: boolean;
    logout: boolean;
    fork: boolean;
  }>;
  authMethodIds: ReadonlyArray<string>;
  extensionMethods: ReadonlyArray<string>;
}>;

export type AcpSessionUpdateRecord = Readonly<{
  runtimeGeneration: number;
  sessionGeneration: number;
  turnGeneration?: number;
  sessionId: string;
  phase: AcpSessionPhase;
  sequence: number;
  disposition: "applied" | "quarantined";
  safeReason?: "late-after-turn" | "stale-generation";
  update: unknown;
}>;

export type AcpSessionSnapshot = Readonly<{
  threadId: string;
  peerSessionId: string;
  runtimeGeneration: number;
  sessionGeneration: number;
  phase: AcpSessionPhase;
  modes?: SessionModeState;
  configOptions: ReadonlyArray<SessionConfigOption>;
  promptActive: boolean;
}>;

export type AcpCancelSource = "user" | "transport" | "protocol" | "shutdown" | "restart";

export type AcpRecoveryDecision =
  | "reattached"
  | "new-session-required"
  | "missing-binary"
  | "auth-lost"
  | "incompatible-version"
  | "missing-session"
  | "protocol-drift"
  | "crash-loop"
  | "cancelled";

export interface AcpSessionTransportPort {
  readonly generation: number;
  readonly state: string;
  request(
    method: string,
    params?: unknown,
    options?: Readonly<{ timeoutMs?: number; signal?: AbortSignal }>,
  ): Promise<unknown>;
  notify(method: string, params?: unknown): void;
  onNotification(method: string, handler: (params: unknown) => void): () => void;
  cancelReverseRequests(sessionId?: string): number;
  drainAcceptedInbound(maxTurns?: number): Promise<void>;
  waitForExit(): Promise<unknown>;
  shutdown(sessionIds?: ReadonlyArray<string>): Promise<void>;
  dispose(): Promise<void>;
}

export type AcpMcpMaterialization = Readonly<{
  servers: ReadonlyArray<McpServer>;
  resolvedRefs?: ReadonlyArray<
    Readonly<{ serverRef: string; transport: "http" | "sse" | "stdio" }>
  >;
  receiptRefs: ReadonlyArray<string>;
  dispose(): void | Promise<void>;
}>;

export type AcpMcpReference = Readonly<{
  serverRef: string;
  transport: "http" | "sse" | "stdio";
  expiresAt: string;
  scopeRef?: string;
}>;

export type AcpSessionRuntimeOptions = Readonly<{
  profile: AcpPeerProfile;
  extensionMethods?: ReadonlyArray<string>;
  createTransport: (signal?: AbortSignal) => Promise<AcpSessionTransportPort>;
  clientCapabilities: Readonly<{
    fs: Readonly<{ readTextFile: boolean; writeTextFile: boolean }>;
    terminal: boolean;
  }>;
  clientInfo?: Readonly<{ name: string; version: string; title?: string }>;
  peerIdentityFallback?: Readonly<{ name: string; version: string }>;
  expectedPeerIdentity?: Readonly<{ namePrefix: string; version: string }>;
  selectAuthMethod?: (
    advertised: ReadonlyArray<Readonly<{ id: string; name?: string; description?: string }>>,
  ) => Promise<string | undefined>;
  authenticateMeta?: Readonly<Record<string, unknown>>;
  bootstrap?: AcpNewSessionInput;
  materializeMcp?: (
    refs: ReadonlyArray<AcpMcpReference>,
    context: Readonly<{
      runtimeGeneration: number;
      sessionGeneration: number;
      method: "session/new" | "session/load" | "session/resume";
      cwd: string;
      scopeRef?: string;
    }>,
  ) => Promise<AcpMcpMaterialization>;
  now?: () => Date;
  maxReceipts?: number;
  requestTimeoutMs?: number;
  restart?: Readonly<{ maxAttempts: number; baseBackoffMs: number; maxBackoffMs: number }>;
  unstableFork?: Readonly<{ enabled: true; peerVersion: string }>;
  onUpdate?: (record: AcpSessionUpdateRecord) => void | Promise<void>;
  settleTurn?: (
    input: Readonly<{
      threadId: string;
      peerSessionId: string;
      runtimeGeneration: number;
      sessionGeneration: number;
      turnGeneration: number;
      terminal: AcpTurnTerminal;
      stopReason: string;
    }>,
  ) => void | Promise<void>;
}>;

/**
 * The production transport constructor accepts only an admission receipt. It
 * launches the admitted real path and re-verifies both path and digest at the
 * final spawn boundary; caller-supplied argv never enters this seam.
 */
export const createAdmittedAcpSessionTransport = async (
  admission: AcpAdmittedSessionLaunch,
  input: Readonly<{
    cwd?: string;
    environment?: Readonly<Record<string, string | undefined>>;
    limits?: Partial<AgentStdioTransportLimits>;
    methodKinds?: ReadonlyArray<Readonly<{ method: string; kind: "request" | "notification" }>>;
  }> = {},
): Promise<AgentStdioTransport> => {
  const environment = buildAdmittedLaunchEnvironment(admission.launchPlan, input.environment ?? {});
  if (environment._tag === "LaunchEnvironmentRejected") throw new Error(environment.detail);
  return AgentStdioTransport.start({
    executable: admission.identityPin.realPath,
    args: admission.launchPlan.args,
    versionProbeArgs: admission.launchPlan.versionProbeArgs,
    env: environment.env,
    identityPin: admission.identityPin,
    ...(input.methodKinds === undefined ? {} : { methodKinds: input.methodKinds }),
    ...(input.cwd === undefined ? {} : { cwd: input.cwd }),
    ...(input.limits === undefined ? {} : { limits: input.limits }),
  });
};

export type AcpNewSessionInput = Readonly<{
  cwd: string;
  additionalDirectories?: ReadonlyArray<string>;
  mcpRefs?: ReadonlyArray<AcpMcpReference>;
  canonicalThreadSeed: string;
  scopeRef?: string;
}>;

export type AcpRestoreSessionInput = AcpNewSessionInput & Readonly<{ peerSessionId: string }>;

type SessionRecord = {
  threadId: string;
  peerSessionId: string;
  runtimeGeneration: number;
  sessionGeneration: number;
  phase: AcpSessionPhase;
  modes: SessionModeState | undefined;
  pendingModeId: string | undefined;
  configOptions: SessionConfigOption[];
  sequence: number;
  liveGate: Promise<void>;
  openLiveGate: () => void;
  promptTail: Promise<void>;
  activeTurn: TurnRecord | undefined;
  queuedTurns: TurnRecord[];
  closed: boolean;
};

type TurnRecord = {
  generation: number;
  controller: AbortController;
  terminal?: AcpTurnTerminal;
  cancelSource?: AcpCancelSource;
  settlement?: Promise<TurnSettlement>;
};

type TurnSettlement = Readonly<{ terminal: AcpTurnTerminal; stopReason: string }>;

type CapabilitySnapshot = AcpRuntimeEvidence["capabilities"];

const defaultRestart = { maxAttempts: 3, baseBackoffMs: 100, maxBackoffMs: 2_000 } as const;

export class AcpSessionRuntime {
  readonly #options: AcpSessionRuntimeOptions;
  readonly #connectionRef = hashRef("acp_connection", randomUUID());
  readonly #receipts: AcpLifecycleReceipt[] = [];
  readonly #sessions = new Map<string, SessionRecord>();
  readonly #updateTasks = new Set<Promise<unknown>>();
  readonly #updateTaskFailures: unknown[] = [];
  #state: AcpSessionRuntimeState = "idle";
  #startFlight: Promise<AcpLifecycleOutcome<AcpRuntimeEvidence>> | undefined;
  #transport: AcpSessionTransportPort | undefined;
  #unsubscribeUpdate: (() => void) | undefined;
  #evidence: AcpRuntimeEvidence | undefined;
  #sessionGeneration = 0;
  #turnGeneration = 0;
  #restartAttempts = 0;
  #lifecycleGeneration = 0;
  #attachFlight: Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> | undefined;
  #recoverFlight: Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> | undefined;

  constructor(options: AcpSessionRuntimeOptions) {
    this.#options = options;
  }

  get state(): AcpSessionRuntimeState {
    return this.#state;
  }

  get evidence(): AcpRuntimeEvidence | undefined {
    return this.#evidence === undefined ? undefined : structuredClone(this.#evidence);
  }

  receipts(): ReadonlyArray<AcpLifecycleReceipt> {
    return structuredClone(this.#receipts);
  }

  sessions(): ReadonlyArray<AcpSessionSnapshot> {
    return [...this.#sessions.values()].map((session) => this.#snapshot(session));
  }

  start(): Promise<AcpLifecycleOutcome<AcpRuntimeEvidence>> {
    if (this.#state === "stopping" || this.#state === "stopped")
      return Promise.resolve(
        this.#failure("start", "invalid_state", "stopped runtime cannot be restarted"),
      );
    return this.#beginStart(false);
  }

  #beginStart(
    skipBootstrap: boolean,
    signal?: AbortSignal,
  ): Promise<AcpLifecycleOutcome<AcpRuntimeEvidence>> {
    if (this.#state === "ready" && this.#evidence !== undefined)
      return Promise.resolve(this.#success("start", this.#evidence, 0));
    if (this.#startFlight !== undefined) return this.#startFlight;
    const lifecycleGeneration = ++this.#lifecycleGeneration;
    this.#startFlight = this.#start(lifecycleGeneration, skipBootstrap, signal).finally(() => {
      this.#startFlight = undefined;
    });
    return this.#startFlight;
  }

  async #start(
    lifecycleGeneration: number,
    skipBootstrap: boolean,
    signal?: AbortSignal,
  ): Promise<AcpLifecycleOutcome<AcpRuntimeEvidence>> {
    const started = Date.now();
    this.#state = "starting";
    this.#record({ method: "start", outcome: "started", evidenceRefs: [] });
    let transport: AcpSessionTransportPort;
    try {
      transport = await this.#options.createTransport(signal);
      if (
        signal?.aborted ||
        lifecycleGeneration !== this.#lifecycleGeneration ||
        this.#state !== "starting"
      ) {
        await transport.dispose();
        return this.#failure("start", "cancelled", "runtime start was cancelled");
      }
      this.#transport = transport;
      this.#unsubscribeUpdate = transport.onNotification("session/update", (params) => {
        const task = this.#acceptUpdate(params, transport.generation);
        this.#updateTasks.add(task);
        void task
          .catch((error: unknown) => this.#updateTaskFailures.push(error))
          .finally(() => this.#updateTasks.delete(task));
      });
      void transport.waitForExit().then(() => this.#handleTransportExit(transport.generation));
      const init = await this.#validatedRequest(
        "initialize",
        {
          protocolVersion: 1,
          clientCapabilities: this.#options.clientCapabilities,
          clientInfo: this.#options.clientInfo ?? {
            name: "openagents",
            title: "OpenAgents",
            version: "0.1.0",
          },
        },
        transport,
        signal,
      );
      const decodedInit = decodeStableAcpDefinition("InitializeResponse", init);
      if (decodedInit._tag === "DecodeFailure") {
        return await this.#startFailure("incompatible_version", decodedInit.detail, started);
      }
      const value = object(decodedInit.value);
      if (value.protocolVersion !== 1)
        return await this.#startFailure(
          "incompatible_version",
          "peer negotiated an unsupported protocol version",
          started,
        );
      const authMethods = Array.isArray(value.authMethods)
        ? value.authMethods
            .map(object)
            .filter((method) => typeof method.id === "string")
            .map((method) => ({
              id: String(method.id),
              ...(typeof method.name === "string" ? { name: method.name } : {}),
              ...(typeof method.description === "string"
                ? { description: method.description }
                : {}),
            }))
        : [];
      if (authMethods.length > 0) {
        const methodId = await this.#options.selectAuthMethod?.(authMethods);
        if (
          signal?.aborted ||
          lifecycleGeneration !== this.#lifecycleGeneration ||
          this.#state !== "starting"
        ) {
          await this.#disposeTransport();
          return this.#failure("start", "cancelled", "runtime start was cancelled");
        }
        if (methodId === undefined || !authMethods.some((method) => method.id === methodId))
          return await this.#startFailure(
            "auth_required",
            "peer authentication is required but no advertised method was authorized",
            started,
          );
        try {
          await this.#validatedRequest(
            "authenticate",
            {
              methodId,
              ...(this.#options.authenticateMeta ? { _meta: this.#options.authenticateMeta } : {}),
            },
            transport,
            signal,
          );
        } catch {
          if (
            signal?.aborted ||
            lifecycleGeneration !== this.#lifecycleGeneration ||
            this.#state !== "starting"
          ) {
            await this.#disposeTransport();
            return this.#failure("start", "cancelled", "runtime start was cancelled");
          }
          return await this.#startFailure("auth_lost", "peer authentication failed", started);
        }
      }
      if (lifecycleGeneration !== this.#lifecycleGeneration || this.#state !== "starting") {
        await this.#disposeTransport();
        return this.#failure("start", "cancelled", "runtime start was cancelled");
      }
      const capabilities = this.#capabilities(value.agentCapabilities);
      const info = object(value.agentInfo);
      const metadata = object(value._meta);
      const peerName =
        typeof info.name === "string"
          ? info.name
          : (this.#options.peerIdentityFallback?.name ?? "unknown-agent");
      const peerVersion =
        typeof info.version === "string"
          ? info.version
          : typeof metadata.agentVersion === "string"
            ? metadata.agentVersion
            : (this.#options.peerIdentityFallback?.version ?? "unknown");
      const expected = this.#options.expectedPeerIdentity;
      if (
        expected !== undefined &&
        (!peerName.toLowerCase().startsWith(expected.namePrefix.toLowerCase()) ||
          peerVersion !== expected.version)
      )
        return await this.#startFailure(
          "incompatible_version",
          "peer initialize identity does not match executable admission",
          started,
        );
      const evidence: AcpRuntimeEvidence = Object.freeze({
        schemaRelease: "schema-v1.19.0",
        wireVersion: 1,
        runtimeGeneration: transport.generation,
        connectionRef: this.#connectionRef,
        profile: this.#options.profile,
        peer: Object.freeze({
          name: peerName,
          version: peerVersion,
        }),
        capabilities,
        authMethodIds: Object.freeze(authMethods.map((method) => method.id)),
        extensionMethods: Object.freeze([...(this.#options.extensionMethods ?? [])]),
      });
      this.#evidence = evidence;
      this.#state = "ready";
      if (!skipBootstrap && this.#options.bootstrap !== undefined) {
        const session = await this.newSession(this.#options.bootstrap);
        if (!session.ok)
          return await this.#startFailure(session.reason, session.safeDetail, started);
      }
      return this.#success("start", evidence, Date.now() - started);
    } catch (error) {
      return await this.#startFailure(this.#failureOf(error), this.#safeError(error), started);
    }
  }

  async newSession(input: AcpNewSessionInput): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    return this.#attachSingleFlight("session/new", input);
  }

  async loadSession(
    input: AcpRestoreSessionInput,
  ): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    if (!this.#requireCapability("load")) return this.#unsupported("session/load");
    return this.#attachSingleFlight("session/load", input);
  }

  async resumeSession(
    input: AcpRestoreSessionInput,
  ): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    if (!this.#requireCapability("resume")) return this.#unsupported("session/resume");
    return this.#attachSingleFlight("session/resume", input);
  }

  #attachSingleFlight(
    method: "session/new" | "session/load" | "session/resume",
    input: AcpNewSessionInput | AcpRestoreSessionInput,
  ): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    if (this.#attachFlight !== undefined)
      return Promise.resolve(
        this.#failure(method, "invalid_state", "another session attach is already active"),
      );
    const flight = this.#attach(method, input);
    this.#attachFlight = flight;
    void flight.then(
      () => {
        if (this.#attachFlight === flight) this.#attachFlight = undefined;
      },
      () => {
        if (this.#attachFlight === flight) this.#attachFlight = undefined;
      },
    );
    return flight;
  }

  async #attach(
    method: "session/new" | "session/load" | "session/resume",
    input: AcpNewSessionInput | AcpRestoreSessionInput,
  ): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    const started = Date.now();
    this.#record({ method, outcome: "started", evidenceRefs: [] });
    const transport = this.#readyTransport(method);
    if (!transport.ok) return transport.outcome;
    const lifecycleGeneration = this.#lifecycleGeneration;
    if (this.#sessions.size > 0)
      return this.#failure(method, "invalid_state", "runtime profile owns one session per process");
    const sessionGeneration = ++this.#sessionGeneration;
    const createSession = (peerSessionId: string, phase: AcpSessionPhase): SessionRecord => {
      let openLiveGate!: () => void;
      const liveGate = new Promise<void>((resolve) => {
        openLiveGate = resolve;
      });
      return {
        threadId: hashRef("thread", `${this.#options.profile}:${input.canonicalThreadSeed}`),
        peerSessionId,
        runtimeGeneration: transport.value.generation,
        sessionGeneration,
        phase,
        modes: undefined,
        pendingModeId: undefined,
        configOptions: [],
        sequence: 0,
        liveGate,
        openLiveGate,
        promptTail: Promise.resolve(),
        activeTurn: undefined,
        queuedTurns: [],
        closed: false,
      };
    };
    let session: SessionRecord | undefined;
    if (method !== "session/new") {
      const peerSessionId = (input as AcpRestoreSessionInput).peerSessionId;
      session = createSession(peerSessionId, "replay");
      // The binding must exist before the request: Grok legitimately emits
      // replay notifications while session/load is still outstanding.
      this.#sessions.set(peerSessionId, session);
      this.#record({
        method: "session/replay",
        outcome: "started",
        sessionGeneration,
        phase: "replay",
        evidenceRefs: [],
      });
    }
    let material: AcpMcpMaterialization | undefined;
    let peerAttached = false;
    try {
      material = await this.#materialize(input, sessionGeneration, method);
      const params = {
        cwd: input.cwd,
        ...(input.additionalDirectories === undefined
          ? {}
          : { additionalDirectories: [...input.additionalDirectories] }),
        mcpServers: [...material.servers],
        ...(method === "session/new"
          ? {}
          : { sessionId: (input as AcpRestoreSessionInput).peerSessionId }),
      };
      const response = object(await this.#validatedRequest(method, params, transport.value));
      peerAttached = true;
      if (
        lifecycleGeneration !== this.#lifecycleGeneration ||
        this.#state !== "ready" ||
        transport.value !== this.#transport
      )
        throw Object.assign(new Error("session attach was cancelled"), { kind: "cancelled" });
      const peerSessionId =
        method === "session/new"
          ? typeof response.sessionId === "string"
            ? response.sessionId
            : undefined
          : (input as AcpRestoreSessionInput).peerSessionId;
      if (peerSessionId === undefined)
        return this.#failure(method, "protocol_failure", "peer omitted the session identifier");
      session ??= createSession(peerSessionId, "live");
      session.modes = this.#modeState(response.modes) ?? session.modes;
      if (session.pendingModeId !== undefined) {
        session.modes = {
          currentModeId: session.pendingModeId,
          availableModes: session.modes?.availableModes ?? [],
        };
        session.pendingModeId = undefined;
      }
      const returnedConfig = this.#configOptions(response.configOptions);
      if (returnedConfig.length > 0 || Object.hasOwn(response, "configOptions"))
        session.configOptions = returnedConfig;
      this.#sessions.set(peerSessionId, session);
      await transport.value.drainAcceptedInbound();
      await this.#drainUpdateTasks();
      if (method !== "session/new") {
        session.phase = "live";
        session.openLiveGate();
        this.#record({
          method: "session/live",
          outcome: "succeeded",
          sessionGeneration,
          phase: "live",
          evidenceRefs: [],
        });
      } else session.openLiveGate();
      const receiptRefs = [...material.receiptRefs];
      await material.dispose();
      material = undefined;
      return this.#success(
        method,
        this.#snapshot(session),
        Date.now() - started,
        session,
        undefined,
        receiptRefs,
      );
    } catch (error) {
      if (session !== undefined) {
        session.closed = true;
        session.phase = "closed";
        this.#sessions.delete(session.peerSessionId);
        if (peerAttached && this.#requireCapability("close")) {
          await this.#validatedRequest(
            "session/close",
            { sessionId: session.peerSessionId },
            transport.value,
          ).catch(() => undefined);
        }
      }
      return this.#failure(
        method,
        this.#failureOf(error),
        this.#safeError(error),
        Date.now() - started,
      );
    } finally {
      try {
        await material?.dispose();
      } catch {
        this.#record({
          method: "mcp/dispose",
          outcome: "protocol_failure",
          evidenceRefs: material?.receiptRefs ?? [],
        });
      }
      material = undefined;
    }
  }

  async listSessions(
    input: Readonly<{ cwd?: string; cursor?: string }> = {},
  ): Promise<AcpLifecycleOutcome<unknown>> {
    if (!this.#requireCapability("list")) return this.#unsupported("session/list");
    return this.#simpleRequest("session/list", input);
  }

  async deleteSession(peerSessionId: string): Promise<AcpLifecycleOutcome<undefined>> {
    if (!this.#requireCapability("delete")) return this.#unsupported("session/delete");
    const outcome = await this.#simpleRequest("session/delete", { sessionId: peerSessionId });
    if (outcome.ok) this.#sessions.delete(peerSessionId);
    return outcome.ok ? { ...outcome, value: undefined } : outcome;
  }

  async closeSession(peerSessionId: string): Promise<AcpLifecycleOutcome<undefined>> {
    if (!this.#requireCapability("close")) return this.#unsupported("session/close");
    const session = this.#sessions.get(peerSessionId);
    if (session?.activeTurn !== undefined) await this.cancel(peerSessionId, "shutdown");
    const outcome = await this.#simpleRequest("session/close", { sessionId: peerSessionId });
    if (outcome.ok && session !== undefined) {
      session.closed = true;
      session.phase = "closed";
      this.#sessions.delete(peerSessionId);
    }
    return outcome.ok ? { ...outcome, value: undefined } : outcome;
  }

  async logout(): Promise<AcpLifecycleOutcome<undefined>> {
    if (!this.#requireCapability("logout")) return this.#unsupported("logout");
    const outcome = await this.#simpleRequest("logout", {});
    return outcome.ok ? { ...outcome, value: undefined } : outcome;
  }

  async forkSession(
    input: Readonly<{
      peerSessionId: string;
      cwd: string;
      additionalDirectories?: ReadonlyArray<string>;
    }>,
  ): Promise<AcpLifecycleOutcome<unknown>> {
    const gate = this.#options.unstableFork;
    if (
      gate?.enabled !== true ||
      this.#evidence === undefined ||
      this.#evidence.peer.version !== gate.peerVersion ||
      !this.#evidence.capabilities.fork
    )
      return this.#unsupported("session/fork");
    const started = Date.now();
    this.#record({ method: "session/fork", outcome: "started", evidenceRefs: [] });
    const transport = this.#readyTransport("session/fork");
    if (!transport.ok) return transport.outcome;
    try {
      const params = {
        sessionId: input.peerSessionId,
        cwd: input.cwd,
        ...(input.additionalDirectories === undefined
          ? {}
          : { additionalDirectories: [...input.additionalDirectories] }),
        mcpServers: [],
      };
      const request = decodeUnstableAcpDefinition("ForkSessionRequest", params);
      if (request._tag === "DecodeFailure") throw new Error(request.detail);
      const response = await transport.value.request("session/fork", request.value, {
        ...(this.#options.requestTimeoutMs === undefined
          ? {}
          : { timeoutMs: this.#options.requestTimeoutMs }),
      });
      const decoded = decodeUnstableAcpDefinition("ForkSessionResponse", response);
      if (decoded._tag === "DecodeFailure") throw new Error(decoded.detail);
      return this.#success("session/fork", decoded.value, Date.now() - started);
    } catch (error) {
      return this.#failure(
        "session/fork",
        this.#failureOf(error),
        this.#safeError(error),
        Date.now() - started,
      );
    }
  }

  async setMode(
    peerSessionId: string,
    modeId: string,
  ): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    const session = this.#sessions.get(peerSessionId);
    if (session === undefined)
      return this.#failure("session/set_mode", "missing_session", "session is unavailable");
    if (session.modes === undefined) return this.#unsupported("session/set_mode");
    const available = session.modes?.availableModes ?? [];
    if (!available.some((mode) => mode.id === modeId))
      return this.#failure(
        "session/set_mode",
        "invalid_value",
        "mode is not advertised by this session",
      );
    if (session.modes?.currentModeId === modeId)
      return this.#success("session/set_mode", this.#snapshot(session), 0, session);
    const outcome = await this.#simpleRequest("session/set_mode", {
      sessionId: peerSessionId,
      modeId,
    });
    if (!outcome.ok) return outcome;
    session.modes = { ...session.modes!, currentModeId: modeId };
    return this.#success(
      "session/set_mode",
      this.#snapshot(session),
      outcome.receipt.latencyMs ?? 0,
      session,
    );
  }

  async setConfigOption(
    peerSessionId: string,
    configId: string,
    value: string | boolean,
  ): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    const session = this.#sessions.get(peerSessionId);
    if (session === undefined)
      return this.#failure(
        "session/set_config_option",
        "missing_session",
        "session is unavailable",
      );
    if (session.configOptions.length === 0) return this.#unsupported("session/set_config_option");
    const option = session.configOptions.find((candidate) => candidate.id === configId);
    if (option === undefined || !this.#validConfigValue(option, value))
      return this.#failure(
        "session/set_config_option",
        "invalid_value",
        "configuration value is not advertised by this session",
      );
    if (option.currentValue === value)
      return this.#success("session/set_config_option", this.#snapshot(session), 0, session);
    const outcome = await this.#simpleRequest("session/set_config_option", {
      sessionId: peerSessionId,
      configId,
      ...(typeof value === "boolean" ? { type: "boolean", value } : { value }),
    });
    if (!outcome.ok) return outcome;
    const response = object(outcome.value);
    session.configOptions = this.#configOptions(response.configOptions);
    return this.#success(
      "session/set_config_option",
      this.#snapshot(session),
      outcome.receipt.latencyMs ?? 0,
      session,
    );
  }

  prompt(
    peerSessionId: string,
    prompt: ReadonlyArray<ContentBlock>,
  ): Promise<AcpLifecycleOutcome<Readonly<{ stopReason: string; terminal: AcpTurnTerminal }>>> {
    const session = this.#sessions.get(peerSessionId);
    if (session === undefined)
      return Promise.resolve(
        this.#failure("session/prompt", "missing_session", "session is unavailable"),
      );
    const turn: TurnRecord = {
      generation: ++this.#turnGeneration,
      controller: new AbortController(),
    };
    session.queuedTurns.push(turn);
    const run = session.promptTail.then(() => this.#runPrompt(session, prompt, turn));
    session.promptTail = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  }

  async #runPrompt(
    session: SessionRecord,
    prompt: ReadonlyArray<ContentBlock>,
    turn: TurnRecord,
  ): Promise<AcpLifecycleOutcome<Readonly<{ stopReason: string; terminal: AcpTurnTerminal }>>> {
    const started = Date.now();
    await session.liveGate;
    session.queuedTurns = session.queuedTurns.filter((candidate) => candidate !== turn);
    if (turn.controller.signal.aborted) {
      await this.#settleTurn(session, turn, "cancelled", "cancelled");
      return this.#failure(
        "session/prompt",
        "cancelled",
        "queued prompt was cancelled before start",
        Date.now() - started,
        session,
        turn,
      );
    }
    const transport = this.#readyTransport("session/prompt");
    if (!transport.ok) {
      const terminal: AcpTurnTerminal =
        this.#state === "failed" ? "process_exit" : "protocol_failure";
      await this.#settleTurn(session, turn, terminal, terminal);
      return this.#failure(
        "session/prompt",
        terminal === "process_exit" ? "process_exit" : "invalid_state",
        "runtime is unavailable for the queued prompt",
        Date.now() - started,
        session,
        turn,
      );
    }
    if (session.closed || session.runtimeGeneration !== transport.value.generation) {
      await this.#settleTurn(session, turn, "process_exit", "stale_generation");
      return this.#failure(
        "session/prompt",
        "invalid_state",
        "session belongs to a stale runtime generation",
      );
    }
    session.activeTurn = turn;
    this.#record({
      method: "session/prompt",
      outcome: "started",
      sessionGeneration: session.sessionGeneration,
      turnGeneration: turn.generation,
      phase: session.phase,
      evidenceRefs: [],
    });
    try {
      const response = object(
        await this.#validatedRequest(
          "session/prompt",
          { sessionId: session.peerSessionId, prompt: [...prompt] },
          transport.value,
          turn.controller.signal,
        ),
      );
      await transport.value.drainAcceptedInbound();
      await this.#drainUpdateTasks();
      const stopReason = typeof response.stopReason === "string" ? response.stopReason : "unknown";
      const terminal: AcpTurnTerminal =
        turn.cancelSource !== undefined || stopReason === "cancelled"
          ? "cancelled"
          : stopReason === "refusal"
            ? "refused"
            : "completed";
      const winner = await this.#settleTurn(session, turn, terminal, stopReason);
      if (winner.terminal === "process_exit")
        return this.#failure(
          "session/prompt",
          "process_exit",
          "peer process exited before turn settlement",
          Date.now() - started,
          session,
          turn,
        );
      const outcome = this.#success("session/prompt", winner, Date.now() - started, session, turn);
      this.#replaceLastReceipt({
        ...outcome.receipt,
        stopReasonRef: hashRef("stop_reason", winner.stopReason),
      });
      return { ...outcome, receipt: this.#receipts.at(-1)! };
    } catch (error) {
      await transport.value.drainAcceptedInbound().catch(() => undefined);
      await this.#drainUpdateTasks().catch(() => undefined);
      const reason = this.#failureOf(error);
      const terminal: AcpTurnTerminal =
        reason === "cancelled"
          ? "cancelled"
          : reason === "timed_out"
            ? "timed_out"
            : reason === "process_exit"
              ? "process_exit"
              : reason === "refused"
                ? "refused"
                : "protocol_failure";
      await this.#settleTurn(session, turn, terminal, reason).catch(() => undefined);
      return this.#failure(
        "session/prompt",
        reason,
        this.#safeError(error),
        Date.now() - started,
        session,
        turn,
      );
    }
  }

  async cancel(
    peerSessionId: string,
    source: AcpCancelSource = "user",
    options: Readonly<{ abortLocal?: boolean }> = {},
  ): Promise<AcpLifecycleOutcome<undefined>> {
    const session = this.#sessions.get(peerSessionId);
    if (session === undefined)
      return this.#failure("session/cancel", "missing_session", "session is unavailable");
    const turn = session.activeTurn ?? session.queuedTurns[0];
    if (turn === undefined || turn.terminal !== undefined)
      return this.#success("session/cancel", undefined, 0, session, turn);
    if (turn.cancelSource !== undefined) {
      if (options.abortLocal !== false) turn.controller.abort();
      return this.#success("session/cancel", undefined, 0, session, turn);
    }
    if (turn !== undefined) {
      turn.cancelSource = source;
      if (options.abortLocal !== false) turn.controller.abort();
    }
    try {
      if (session.activeTurn === turn) {
        this.#transport?.cancelReverseRequests(peerSessionId);
        this.#transport?.notify("session/cancel", { sessionId: peerSessionId });
      }
      const result = this.#success("session/cancel", undefined, 0, session, turn);
      this.#replaceLastReceipt({ ...result.receipt, cancelSource: source });
      return { ...result, receipt: this.#receipts.at(-1)! };
    } catch (error) {
      return this.#failure(
        "session/cancel",
        this.#failureOf(error),
        this.#safeError(error),
        0,
        session,
        turn,
      );
    }
  }

  recover(
    input: AcpRestoreSessionInput,
    options: Readonly<{ signal?: AbortSignal }> = {},
  ): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    if (this.#recoverFlight !== undefined) return this.#recoverFlight;
    const flight = this.#recover(input, options.signal);
    this.#recoverFlight = flight;
    void flight.then(
      () => {
        if (this.#recoverFlight === flight) this.#recoverFlight = undefined;
      },
      () => {
        if (this.#recoverFlight === flight) this.#recoverFlight = undefined;
      },
    );
    return flight;
  }

  async #recover(
    input: AcpRestoreSessionInput,
    signal?: AbortSignal,
  ): Promise<AcpLifecycleOutcome<AcpSessionSnapshot>> {
    this.#record({ method: "recover", outcome: "started", evidenceRefs: [] });
    const config = { ...defaultRestart, ...this.#options.restart };
    if (signal?.aborted || this.#isStopping()) {
      const cancelled = this.#failure("recover", "cancelled", "runtime recovery was cancelled");
      this.#replaceLastReceipt({ ...cancelled.receipt, recoveryDecision: "cancelled" });
      return { ...cancelled, receipt: this.#receipts.at(-1)! };
    }
    if (this.#restartAttempts >= config.maxAttempts) {
      const failed = this.#failure(
        "recover",
        "restart_budget_exhausted",
        "runtime restart budget exhausted",
      );
      this.#replaceLastReceipt({ ...failed.receipt, recoveryDecision: "crash-loop" });
      return { ...failed, receipt: this.#receipts.at(-1)! };
    }
    this.#restartAttempts += 1;
    this.#state = "recovering";
    for (const session of this.#sessions.values()) {
      await this.cancel(session.peerSessionId, "restart").catch(() => undefined);
    }
    await this.#drainUpdateTasks().catch(() => undefined);
    this.#sessions.clear();
    await this.#disposeTransport();
    if (this.#restartAttempts > 1) {
      const delay = Math.min(
        config.maxBackoffMs,
        config.baseBackoffMs * 2 ** (this.#restartAttempts - 2),
      );
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(resolve, delay);
        signal?.addEventListener(
          "abort",
          () => {
            clearTimeout(timer);
            reject(Object.assign(new Error("cancelled"), { kind: "cancelled" }));
          },
          { once: true },
        );
      }).catch(() => undefined);
    }
    if (signal?.aborted || this.#isStopping()) {
      const cancelled = this.#failure("recover", "cancelled", "runtime recovery was cancelled");
      this.#replaceLastReceipt({ ...cancelled.receipt, recoveryDecision: "cancelled" });
      return { ...cancelled, receipt: this.#receipts.at(-1)! };
    }
    const start = await this.#beginStart(true, signal);
    if (!start.ok) {
      const decision: AcpRecoveryDecision =
        start.reason === "cancelled"
          ? "cancelled"
          : start.reason === "auth_required" || start.reason === "auth_lost"
            ? "auth-lost"
            : start.reason === "incompatible_version"
              ? "incompatible-version"
              : start.reason === "process_exit" || start.reason === "missing_binary"
                ? "missing-binary"
                : "protocol-drift";
      this.#replaceLastReceipt({ ...start.receipt, recoveryDecision: decision });
      return { ...start, receipt: this.#receipts.at(-1)! };
    }
    if (signal?.aborted || this.#isStopping()) {
      const cancelled = this.#failure("recover", "cancelled", "runtime recovery was cancelled");
      this.#replaceLastReceipt({ ...cancelled.receipt, recoveryDecision: "cancelled" });
      return { ...cancelled, receipt: this.#receipts.at(-1)! };
    }
    const restored = this.#requireCapability("resume")
      ? await this.resumeSession(input)
      : this.#requireCapability("load")
        ? await this.loadSession(input)
        : this.#failure("recover", "unsupported", "peer does not advertise a recovery method");
    const decision: AcpRecoveryDecision = restored.ok
      ? "reattached"
      : restored.reason === "missing_session"
        ? "missing-session"
        : restored.reason === "cancelled"
          ? "cancelled"
          : "new-session-required";
    this.#replaceLastReceipt({ ...restored.receipt, recoveryDecision: decision });
    if (restored.ok) this.#restartAttempts = 0;
    return { ...restored, receipt: this.#receipts.at(-1)! };
  }

  async shutdown(): Promise<void> {
    if (this.#state === "stopped") return;
    this.#lifecycleGeneration += 1;
    this.#state = "stopping";
    for (const session of this.#sessions.values()) {
      await this.cancel(session.peerSessionId, "shutdown").catch(() => undefined);
      for (const turn of session.queuedTurns) {
        turn.cancelSource = "shutdown";
        turn.controller.abort();
      }
    }
    await this.#transport?.shutdown([...this.#sessions.keys()]);
    await this.#startFlight;
    await this.#attachFlight;
    await this.#recoverFlight;
    this.#sessions.clear();
    await this.#disposeTransport();
    this.#state = "stopped";
  }

  async #acceptUpdate(params: unknown, runtimeGeneration: number): Promise<void> {
    if (this.#transport?.generation !== runtimeGeneration) return;
    const decoded = decodeStableAcpMethodPayload({
      direction: "agent-to-client",
      method: "session/update",
      phase: "params",
      payload: params,
    });
    if (decoded._tag === "DecodeFailure") return;
    const notification = object(decoded.value);
    const sessionId =
      typeof notification.sessionId === "string" ? notification.sessionId : undefined;
    if (sessionId === undefined) return;
    const session = this.#sessions.get(sessionId);
    if (session === undefined || session.closed || session.runtimeGeneration !== runtimeGeneration)
      return;
    const update = object(notification.update);
    const contentLike = [
      "agent_message_chunk",
      "agent_thought_chunk",
      "tool_call",
      "tool_call_update",
    ].includes(String(update.sessionUpdate));
    const lateAfterTurn =
      session.phase === "live" && session.activeTurn === undefined && contentLike;
    if (
      !lateAfterTurn &&
      update.sessionUpdate === "current_mode_update" &&
      typeof update.currentModeId === "string"
    ) {
      if (session.modes === undefined) session.pendingModeId = update.currentModeId;
      else session.modes = { ...session.modes, currentModeId: update.currentModeId };
    }
    if (!lateAfterTurn && update.sessionUpdate === "config_option_update")
      session.configOptions = this.#configOptions(update.configOptions);
    const record: AcpSessionUpdateRecord = Object.freeze({
      runtimeGeneration,
      sessionGeneration: session.sessionGeneration,
      ...(session.activeTurn === undefined
        ? {}
        : { turnGeneration: session.activeTurn.generation }),
      sessionId,
      phase: session.phase,
      sequence: ++session.sequence,
      disposition: lateAfterTurn ? "quarantined" : "applied",
      ...(lateAfterTurn ? { safeReason: "late-after-turn" as const } : {}),
      update: structuredClone(notification.update),
    });
    await this.#options.onUpdate?.(record);
  }

  async #drainUpdateTasks(maxTurns = 16): Promise<void> {
    for (let turn = 0; turn < maxTurns; turn += 1) {
      const tasks = [...this.#updateTasks];
      if (tasks.length === 0) {
        const failure = this.#updateTaskFailures.shift();
        if (failure !== undefined) throw failure;
        return;
      }
      await Promise.allSettled(tasks);
    }
    throw new Error("ACP update projection barrier did not quiesce");
  }

  #handleTransportExit(runtimeGeneration: number): void {
    if (
      this.#transport?.generation !== runtimeGeneration ||
      this.#state === "stopping" ||
      this.#state === "stopped"
    )
      return;
    this.#state = "failed";
    for (const session of this.#sessions.values()) {
      if (session.runtimeGeneration !== runtimeGeneration) continue;
      const turns = [session.activeTurn, ...session.queuedTurns].filter(
        (turn): turn is TurnRecord => turn !== undefined,
      );
      for (const turn of turns) {
        const task = this.#settleTurn(session, turn, "process_exit", "process_exit");
        this.#updateTasks.add(task);
        void task
          .catch((error: unknown) => this.#updateTaskFailures.push(error))
          .finally(() => this.#updateTasks.delete(task));
      }
      session.closed = true;
      session.phase = "closed";
    }
    this.#record({ method: "process/exit", outcome: "process_exit", evidenceRefs: [] });
  }

  async #validatedRequest(
    method: string,
    params: unknown,
    transport: AcpSessionTransportPort,
    signal?: AbortSignal,
  ): Promise<unknown> {
    const request = decodeStableAcpMethodPayload({
      direction: "client-to-agent",
      method,
      phase: "params",
      payload: params,
    });
    if (request._tag === "DecodeFailure")
      throw new Error(`invalid ${method} request: ${request.detail}`);
    const response = await transport.request(method, request.value, {
      ...(this.#options.requestTimeoutMs === undefined
        ? {}
        : { timeoutMs: this.#options.requestTimeoutMs }),
      ...(signal === undefined ? {} : { signal }),
    });
    const decoded = decodeStableAcpMethodPayload({
      direction: "client-to-agent",
      method,
      phase: "result",
      payload: response,
    });
    if (decoded._tag === "DecodeFailure")
      throw new Error(`invalid ${method} response: ${decoded.detail}`);
    return decoded.value;
  }

  async #simpleRequest(method: string, params: unknown): Promise<AcpLifecycleOutcome<unknown>> {
    const started = Date.now();
    this.#record({ method, outcome: "started", evidenceRefs: [] });
    const transport = this.#readyTransport(method);
    if (!transport.ok) return transport.outcome;
    try {
      const value = await this.#validatedRequest(method, params, transport.value);
      return this.#success(method, value, Date.now() - started);
    } catch (error) {
      return this.#failure(
        method,
        this.#failureOf(error),
        this.#safeError(error),
        Date.now() - started,
      );
    }
  }

  async #materialize(
    input: AcpNewSessionInput | AcpRestoreSessionInput,
    sessionGeneration: number,
    method: "session/new" | "session/load" | "session/resume",
  ): Promise<AcpMcpMaterialization> {
    const refs = input.mcpRefs ?? [];
    if (refs.length === 0) return { servers: [], receiptRefs: [], dispose: () => undefined };
    if (refs.length > 32) throw new Error("MCP capability reference limit exceeded");
    const seen = new Set<string>();
    for (const ref of refs) {
      if (
        !/^[A-Za-z0-9._:-]{1,256}$/.test(ref.serverRef) ||
        !Number.isFinite(Date.parse(ref.expiresAt)) ||
        Date.parse(ref.expiresAt) <= this.#now().getTime() ||
        seen.has(ref.serverRef)
      )
        throw new Error("MCP capability reference is invalid or expired");
      if (ref.scopeRef !== undefined && ref.scopeRef !== input.scopeRef)
        throw new Error("MCP capability reference is outside the requested scope");
      seen.add(ref.serverRef);
    }
    if (this.#options.materializeMcp === undefined)
      throw new Error("MCP capability references cannot be resolved by this runtime");
    const material = await this.#options.materializeMcp(refs, {
      runtimeGeneration: this.#transport?.generation ?? 0,
      sessionGeneration,
      method,
      cwd: input.cwd,
      ...(input.scopeRef === undefined ? {} : { scopeRef: input.scopeRef }),
    });
    try {
      if (
        material.resolvedRefs === undefined ||
        material.resolvedRefs.length !== refs.length ||
        refs.some((ref, index) => {
          const resolved = material.resolvedRefs?.[index];
          return resolved?.serverRef !== ref.serverRef || resolved.transport !== ref.transport;
        })
      )
        throw new Error("MCP materialization does not match the authorized references");
      return material;
    } catch (error) {
      await Promise.resolve(material.dispose()).catch(() => undefined);
      throw error;
    }
  }

  #readyTransport(
    method: string,
  ):
    | Readonly<{ ok: true; value: AcpSessionTransportPort }>
    | Readonly<{ ok: false; outcome: AcpLifecycleOutcome<never> }> {
    if (
      this.#state !== "ready" ||
      this.#transport === undefined ||
      this.#transport.state !== "running"
    )
      return { ok: false, outcome: this.#failure(method, "invalid_state", "runtime is not ready") };
    return { ok: true, value: this.#transport };
  }

  #capabilities(value: unknown): CapabilitySnapshot {
    const capabilities = object(value);
    const sessions = object(capabilities.sessionCapabilities);
    return Object.freeze({
      load: capabilities.loadSession === true,
      list: present(sessions.list),
      delete: present(sessions.delete),
      resume: present(sessions.resume),
      close: present(sessions.close),
      logout: present(object(capabilities.auth).logout),
      fork: present(sessions.fork),
    });
  }

  #requireCapability(key: keyof CapabilitySnapshot): boolean {
    return this.#evidence?.capabilities[key] === true;
  }

  #modeState(value: unknown): SessionModeState | undefined {
    const mode = object(value);
    return typeof mode.currentModeId === "string" && Array.isArray(mode.availableModes)
      ? (structuredClone(mode) as SessionModeState)
      : undefined;
  }

  #configOptions(value: unknown): SessionConfigOption[] {
    return Array.isArray(value) ? (structuredClone(value) as SessionConfigOption[]) : [];
  }

  #validConfigValue(option: SessionConfigOption, value: string | boolean): boolean {
    if (option.type === "boolean") return typeof value === "boolean";
    if (typeof value !== "string") return false;
    const entries = Array.isArray(option.options)
      ? option.options.flatMap((entry) => {
          const candidate = object(entry);
          return Array.isArray(candidate.options) ? candidate.options : [entry];
        })
      : [];
    return entries.some((entry) => object(entry).value === value);
  }

  #snapshot(session: SessionRecord): AcpSessionSnapshot {
    return Object.freeze({
      threadId: session.threadId,
      peerSessionId: session.peerSessionId,
      runtimeGeneration: session.runtimeGeneration,
      sessionGeneration: session.sessionGeneration,
      phase: session.phase,
      ...(session.modes === undefined ? {} : { modes: structuredClone(session.modes) }),
      configOptions: structuredClone(session.configOptions),
      promptActive:
        (session.activeTurn !== undefined && session.activeTurn.terminal === undefined) ||
        session.queuedTurns.some((turn) => turn.terminal === undefined),
    });
  }

  async #settleTurn(
    session: SessionRecord,
    turn: TurnRecord,
    terminal: AcpTurnTerminal,
    stopReason: string,
  ): Promise<TurnSettlement> {
    if (turn.settlement !== undefined) return turn.settlement;
    turn.terminal = terminal;
    const winner = Object.freeze({ terminal, stopReason });
    const settlement = Promise.resolve()
      .then(() =>
        this.#options.settleTurn?.({
          threadId: session.threadId,
          peerSessionId: session.peerSessionId,
          runtimeGeneration: session.runtimeGeneration,
          sessionGeneration: session.sessionGeneration,
          turnGeneration: turn.generation,
          terminal,
          stopReason,
        }),
      )
      .then(() => winner)
      .finally(() => {
        if (session.activeTurn === turn) session.activeTurn = undefined;
        session.queuedTurns = session.queuedTurns.filter((candidate) => candidate !== turn);
      });
    turn.settlement = settlement;
    return settlement;
  }

  #success<Value>(
    method: string,
    value: Value,
    latencyMs: number,
    session?: SessionRecord,
    turn?: TurnRecord,
    evidenceRefs: ReadonlyArray<string> = [],
  ): AcpLifecycleOutcome<Value> {
    const receipt = this.#record({
      method,
      outcome: "succeeded",
      latencyMs,
      ...(session === undefined
        ? {}
        : { sessionGeneration: session.sessionGeneration, phase: session.phase }),
      ...(turn === undefined
        ? {}
        : { turnGeneration: turn.generation, cancelSource: turn.cancelSource }),
      evidenceRefs,
    });
    return { ok: true, value, receipt };
  }

  #failure<Value = never>(
    method: string,
    reason: AcpLifecycleFailure,
    safeDetail: string,
    latencyMs = 0,
    session?: SessionRecord,
    turn?: TurnRecord,
  ): AcpLifecycleOutcome<Value> {
    const receipt = this.#record({
      method,
      outcome: reason,
      latencyMs,
      ...(session === undefined
        ? {}
        : { sessionGeneration: session.sessionGeneration, phase: session.phase }),
      ...(turn === undefined
        ? {}
        : { turnGeneration: turn.generation, cancelSource: turn.cancelSource }),
      evidenceRefs: [],
    });
    return { ok: false, reason, safeDetail: safeDetail.slice(0, 500), receipt };
  }

  #unsupported(method: string): AcpLifecycleOutcome<never> {
    return this.#failure(method, "unsupported", "peer did not advertise this method");
  }

  #record(input: Omit<AcpLifecycleReceipt, "at" | "runtimeGeneration">): AcpLifecycleReceipt {
    const receipt = Object.freeze({
      at: this.#now().toISOString(),
      runtimeGeneration: this.#transport?.generation ?? 0,
      ...input,
      evidenceRefs: Object.freeze([...input.evidenceRefs]),
    });
    this.#receipts.push(receipt);
    const max = this.#options.maxReceipts ?? 256;
    if (this.#receipts.length > max) this.#receipts.splice(0, this.#receipts.length - max);
    return receipt;
  }

  #replaceLastReceipt(receipt: AcpLifecycleReceipt): void {
    if (this.#receipts.length > 0)
      this.#receipts[this.#receipts.length - 1] = Object.freeze(receipt);
  }

  #failureOf(error: unknown): AcpLifecycleFailure {
    const details = object(error);
    const kind = details.kind;
    if (kind === "missing_executable") return "missing_binary";
    if (details.code === "ENOENT") return "missing_binary";
    if (kind === "remote_error" && details.code === -32002) return "missing_session";
    if (kind === "remote_error") return "refused";
    if (kind === "timeout") return "timed_out";
    if (kind === "cancelled") return "cancelled";
    if (kind === "process_exit" || kind === "not_running") return "process_exit";
    const text = error instanceof Error ? error.message : String(error);
    if (/initialize.+(?:protocolVersion|protocol version)/i.test(text))
      return "incompatible_version";
    if (/timed out/i.test(text)) return "timed_out";
    if (/cancel/i.test(text)) return "cancelled";
    if (/session.+(?:missing|not found)|missing session/i.test(text)) return "missing_session";
    return "protocol_failure";
  }

  #safeError(error: unknown): string {
    const summaries: Readonly<Record<AcpLifecycleFailure, string>> = {
      unsupported: "peer does not support this operation",
      refused: "peer refused this operation",
      timed_out: "operation timed out",
      cancelled: "operation was cancelled",
      process_exit: "peer process exited",
      protocol_failure: "peer protocol operation failed",
      protocol_drift: "peer protocol behavior is incompatible",
      invalid_state: "runtime state does not allow this operation",
      invalid_value: "operation value is not advertised",
      missing_session: "peer session is unavailable",
      auth_required: "peer authentication requires owner action",
      auth_lost: "peer authentication failed",
      missing_binary: "peer executable is unavailable",
      incompatible_version: "peer protocol version is incompatible",
      restart_budget_exhausted: "runtime restart budget is exhausted",
    };
    return summaries[this.#failureOf(error)];
  }

  async #startFailure(
    reason: AcpLifecycleFailure,
    detail: string,
    started: number,
  ): Promise<AcpLifecycleOutcome<AcpRuntimeEvidence>> {
    if (this.#state !== "stopping" && this.#state !== "stopped") this.#state = "failed";
    const outcome = this.#failure<AcpRuntimeEvidence>(
      "start",
      reason,
      detail,
      Date.now() - started,
    );
    await this.#disposeTransport();
    return outcome;
  }

  async #disposeTransport(): Promise<void> {
    this.#unsubscribeUpdate?.();
    this.#unsubscribeUpdate = undefined;
    const transport = this.#transport;
    this.#transport = undefined;
    await transport?.dispose().catch(() => undefined);
  }

  #now(): Date {
    return this.#options.now?.() ?? new Date();
  }

  #isStopping(): boolean {
    return this.#state === "stopping" || this.#state === "stopped";
  }
}
