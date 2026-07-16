import { createHash } from "node:crypto";
import { decodeStableAcpMethodPayload } from "@openagentsinc/agent-client-protocol/stable";
import {
  applyRuntimeInteractionDecision,
  decodeRuntimeInteraction,
  decodeRuntimeInteractionDecisionEnvelope,
  type KhalaRuntimeSource,
  type RuntimeInteraction,
} from "@openagentsinc/agent-runtime-schema";
import type {
  CreateTerminalRequest,
  CreateTerminalResponse,
  KillTerminalRequest,
  KillTerminalResponse,
  McpServer,
  NewSessionResponse,
  ReadTextFileRequest,
  ReadTextFileResponse,
  ReleaseTerminalRequest,
  ReleaseTerminalResponse,
  RequestPermissionRequest,
  RequestPermissionResponse,
  TerminalOutputRequest,
  TerminalOutputResponse,
  WaitForTerminalExitRequest,
  WaitForTerminalExitResponse,
  WriteTextFileRequest,
  WriteTextFileResponse,
} from "@openagentsinc/agent-client-protocol/stable";

export type AcpAuthorityHealth = "healthy" | "unhealthy";

export type AcpAuthorityCapabilitySnapshot = Readonly<{
  connectionRef: string;
  generation: number;
  permission: boolean;
  filesystem: Readonly<{ readTextFile: boolean; writeTextFile: boolean }>;
  terminal: boolean;
  mcpTransports: ReadonlyArray<"http" | "sse" | "stdio">;
}>;

export const projectAcpClientCapabilities = (
  snapshot: AcpAuthorityCapabilitySnapshot,
): Readonly<{
  fs: Readonly<{ readTextFile: boolean; writeTextFile: boolean }>;
  terminal: boolean;
}> => Object.freeze({ fs: Object.freeze({ ...snapshot.filesystem }), terminal: snapshot.terminal });

export type AcpCapabilityReadiness = Readonly<{
  handlerInstalled: boolean;
  authorized: boolean;
  tested: boolean;
  healthy: boolean;
}>;

export type AcpAuthorityReadiness = Readonly<{
  permission?: AcpCapabilityReadiness;
  filesystem?: Readonly<{
    readTextFile?: AcpCapabilityReadiness;
    writeTextFile?: AcpCapabilityReadiness;
  }>;
  terminal?: AcpCapabilityReadiness;
  mcp?: Readonly<Partial<Record<"http" | "sse" | "stdio", AcpCapabilityReadiness>>>;
}>;

export type AcpAuthorityContext = Readonly<{
  requestRef: string;
  connectionRef: string;
  generation: number;
  sessionId: string;
  scopeRef: string;
  signal?: AbortSignal;
}>;

export type AcpAuthoritySession = Readonly<{
  sessionId: string;
  connectionRef: string;
  generation: number;
  scopeRef: string;
  authenticated: boolean;
  health: AcpAuthorityHealth;
}>;

export type AcpAuthorityLease = Readonly<{
  requestRef: string;
  sessionId: string;
  connectionRef: string;
  generation: number;
  scopeRef: string;
  signal?: AbortSignal;
}>;

export type AcpBrokerResult<Value> = Readonly<{
  value: Value;
  evidenceRefs?: ReadonlyArray<string>;
}>;

type HealthPort = Readonly<{ health: () => Promise<AcpAuthorityHealth> }>;

export type AcpSessionAuthorityPort = Readonly<{
  inspect: (sessionId: string) => Promise<AcpAuthoritySession | undefined>;
}>;

export type AcpFilesystemBrokerPort = HealthPort &
  Readonly<{
    readTextFile?: (
      request: ReadTextFileRequest,
      lease: AcpAuthorityLease,
    ) => Promise<AcpBrokerResult<ReadTextFileResponse>>;
    writeTextFile?: (
      request: WriteTextFileRequest,
      lease: AcpAuthorityLease,
    ) => Promise<AcpBrokerResult<WriteTextFileResponse>>;
  }>;

export type AcpTerminalBrokerPort = HealthPort &
  Readonly<{
    create: (
      request: CreateTerminalRequest,
      lease: AcpAuthorityLease,
    ) => Promise<AcpBrokerResult<CreateTerminalResponse>>;
    output: (
      request: TerminalOutputRequest,
      lease: AcpAuthorityLease,
    ) => Promise<AcpBrokerResult<TerminalOutputResponse>>;
    waitForExit: (
      request: WaitForTerminalExitRequest,
      lease: AcpAuthorityLease,
    ) => Promise<AcpBrokerResult<WaitForTerminalExitResponse>>;
    kill: (
      request: KillTerminalRequest,
      lease: AcpAuthorityLease,
    ) => Promise<AcpBrokerResult<KillTerminalResponse>>;
    release: (
      request: ReleaseTerminalRequest,
      lease: AcpAuthorityLease,
    ) => Promise<AcpBrokerResult<ReleaseTerminalResponse>>;
  }>;

export type AcpMcpBrokerPort = HealthPort &
  Readonly<{
    supportedTransports: ReadonlyArray<"http" | "sse" | "stdio">;
    materializeForSessionNew: (
      request: Readonly<{
        cwdRef: string;
        servers: ReadonlyArray<
          Readonly<{ serverRef: string; transport: "http" | "sse" | "stdio"; expiresAt: string }>
        >;
      }>,
      lease: AcpAuthorityLease,
      launch: (material: ReadonlyArray<McpServer>) => Promise<NewSessionResponse>,
    ) => Promise<AcpBrokerResult<NewSessionResponse>>;
  }>;

export type AcpPendingInteractionDelivery = "queue" | "steer";
export type AcpPendingInteractionOrigin = "permission" | "provider_question" | "plan_review";

export const pendingInteractionDelivery = (
  origin: AcpPendingInteractionOrigin,
  promptActive: boolean,
): AcpPendingInteractionDelivery =>
  origin === "provider_question" && promptActive ? "steer" : "queue";

export const routeAcpUserInput = (
  hasPendingReverseInteraction: boolean,
): "queue-next-prompt" | "start-prompt" =>
  hasPendingReverseInteraction ? "queue-next-prompt" : "start-prompt";

export const routeAcpInteractionDecision = (): "steer-active-interaction" =>
  "steer-active-interaction";

export type AcpInteractionBrokerResult =
  | Readonly<{ kind: "cancelled"; evidenceRefs?: ReadonlyArray<string> }>
  | Readonly<{
      kind: "decision";
      envelope: unknown;
      evidenceRefs?: ReadonlyArray<string>;
    }>;

export type AcpInteractionBrokerPort = HealthPort &
  Readonly<{
    request: (
      interaction: RuntimeInteraction,
      input: Readonly<{ delivery: AcpPendingInteractionDelivery }>,
    ) => Promise<AcpInteractionBrokerResult>;
  }>;

export type AcpPermissionPolicyResult =
  | Readonly<{
      kind: "selected";
      optionId: string;
      allowed: boolean;
      authorityRef: string;
      policyRef: string;
      decisionRef: string;
      evidenceRefs?: ReadonlyArray<string>;
    }>
  | Readonly<{
      kind: "escalate";
      approveOptionId: string;
      denyOptionId: string;
      authorityRef: string;
      policyRef: string;
      decisionRef: string;
      evidenceRefs?: ReadonlyArray<string>;
    }>;

export type AcpPermissionPolicyPort = HealthPort &
  Readonly<{
    decide: (
      request: RequestPermissionRequest,
      lease: AcpAuthorityLease,
    ) => Promise<AcpPermissionPolicyResult>;
  }>;

export type AcpAuthorityFaultCode =
  | "unsupported"
  | "timed_out"
  | "overloaded"
  | "cancelled"
  | "capability_not_advertised"
  | "connection_mismatch"
  | "generation_mismatch"
  | "session_not_found"
  | "session_unauthenticated"
  | "scope_mismatch"
  | "authority_unhealthy"
  | "invalid_decision"
  | "decision_conflict"
  | "interaction_expired"
  | "interaction_revoked"
  | "broker_failure";

const faultMessages: Readonly<Record<AcpAuthorityFaultCode, string>> = {
  unsupported: "The requested operation is unsupported.",
  timed_out: "The authority request timed out.",
  overloaded: "The authority request queue is full.",
  cancelled: "The authority request was cancelled.",
  capability_not_advertised: "The requested capability is not available.",
  connection_mismatch: "The request does not belong to the active connection.",
  generation_mismatch: "The request belongs to a stale connection or session generation.",
  session_not_found: "The requested session is unavailable.",
  session_unauthenticated: "The requested session is not authenticated.",
  scope_mismatch: "The request is outside the authorized scope.",
  authority_unhealthy: "The authority broker is unavailable.",
  invalid_decision: "The interaction decision is invalid.",
  decision_conflict: "The interaction was already resolved differently.",
  interaction_expired: "The interaction has expired.",
  interaction_revoked: "The interaction was revoked.",
  broker_failure: "The authority broker refused the request.",
};

export class AcpAuthorityFault extends Error {
  override readonly name = "AcpAuthorityFault";

  constructor(readonly code: AcpAuthorityFaultCode) {
    super(faultMessages[code]);
  }
}

export type AcpAuthorityProtocolError = Readonly<{
  code: number;
  message: string;
  data: Readonly<{ reason: AcpAuthorityFaultCode; retryable: boolean }>;
}>;

/** Bounded JSON-RPC error projection. It never includes raw request or broker data. */
export const toAcpAuthorityProtocolError = (
  fault: AcpAuthorityFault,
): AcpAuthorityProtocolError => {
  const code =
    fault.code === "unsupported" || fault.code === "capability_not_advertised"
      ? -32601
      : fault.code === "generation_mismatch" || fault.code === "session_not_found"
        ? -32002
        : fault.code === "session_unauthenticated"
          ? -32000
          : fault.code === "timed_out" || fault.code === "interaction_expired"
            ? -32001
            : fault.code === "overloaded"
              ? -32005
              : fault.code === "cancelled"
                ? -32800
                : fault.code === "connection_mismatch" ||
                    fault.code === "scope_mismatch" ||
                    fault.code === "invalid_decision" ||
                    fault.code === "decision_conflict" ||
                    fault.code === "interaction_revoked"
                  ? -32602
                  : -32603;
  return Object.freeze({
    code,
    message: fault.message,
    data: Object.freeze({
      reason: fault.code,
      retryable:
        fault.code === "timed_out" ||
        fault.code === "overloaded" ||
        fault.code === "authority_unhealthy",
    }),
  });
};

export type AcpAuthorityReceipt = Readonly<{
  receiptRef: string;
  requestRef: string;
  method: string;
  connectionRef: string;
  generation: number;
  sessionRef: string;
  scopeRef: string;
  outcome: "allowed" | "refused" | "cancelled";
  faultCode?: AcpAuthorityFaultCode;
  evidenceRefs: ReadonlyArray<string>;
  redaction: "safe-metadata-only";
  startedAt: string;
  endedAt: string;
}>;

export type AcpAuthorityReceiptPort = Readonly<{
  record: (receipt: AcpAuthorityReceipt) => void | Promise<void>;
}>;

export type AcpAuthorityBridgeOptions = Readonly<{
  connectionRef: string;
  generation: number;
  sessions: AcpSessionAuthorityPort;
  interactions?: AcpInteractionBrokerPort;
  permissionPolicy?: AcpPermissionPolicyPort;
  filesystem?: AcpFilesystemBrokerPort;
  terminal?: AcpTerminalBrokerPort;
  mcp?: AcpMcpBrokerPort;
  receipts?: AcpAuthorityReceiptPort;
  source: KhalaRuntimeSource;
  threadId: string;
  turnId: string;
  now: () => string;
  nextRef: (kind: "interaction" | "decision" | "receipt" | "question") => string;
  interactionTtlMs?: number;
  maxReplayEntries?: number;
  readiness?: AcpAuthorityReadiness;
}>;

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/;
const safeEvidenceRefs = (refs: ReadonlyArray<string> | undefined): ReadonlyArray<string> =>
  Object.freeze([
    ...new Set((refs ?? []).map((ref) => (safeRefPattern.test(ref) ? ref : "evidence.redacted"))),
  ]);

const isReady = (readiness: AcpCapabilityReadiness | undefined): boolean =>
  readiness?.handlerInstalled === true &&
  readiness.authorized === true &&
  readiness.tested === true &&
  readiness.healthy === true;

const healthOf = (port: HealthPort | undefined): (() => Promise<AcpAuthorityHealth>) | undefined =>
  port === undefined ? undefined : () => port.health();

const immutableSnapshot = (options: AcpAuthorityBridgeOptions): AcpAuthorityCapabilitySnapshot =>
  Object.freeze({
    connectionRef: options.connectionRef,
    generation: options.generation,
    permission:
      options.permissionPolicy !== undefined &&
      options.interactions !== undefined &&
      isReady(options.readiness?.permission),
    filesystem: Object.freeze({
      readTextFile:
        options.filesystem?.readTextFile !== undefined &&
        isReady(options.readiness?.filesystem?.readTextFile),
      writeTextFile:
        options.filesystem?.writeTextFile !== undefined &&
        isReady(options.readiness?.filesystem?.writeTextFile),
    }),
    terminal: options.terminal !== undefined && isReady(options.readiness?.terminal),
    mcpTransports: Object.freeze(
      (options.mcp?.supportedTransports ?? []).filter((transport) =>
        isReady(options.readiness?.mcp?.[transport]),
      ),
    ),
  });

export class AcpAuthorityBridge {
  readonly capabilities: AcpAuthorityCapabilitySnapshot;
  readonly #options: AcpAuthorityBridgeOptions;
  readonly #responses = new Map<
    string,
    Readonly<{ fingerprint: string; promise: Promise<unknown> }>
  >();

  constructor(options: AcpAuthorityBridgeOptions) {
    this.#options = options;
    this.capabilities = immutableSnapshot(options);
  }

  async #lease(
    context: AcpAuthorityContext,
    capability: boolean,
    brokerHealth: (() => Promise<AcpAuthorityHealth>) | undefined,
  ): Promise<AcpAuthorityLease> {
    if (!capability || brokerHealth === undefined)
      throw new AcpAuthorityFault("capability_not_advertised");
    if (
      context.connectionRef !== this.capabilities.connectionRef ||
      context.connectionRef !== this.#options.connectionRef
    )
      throw new AcpAuthorityFault("connection_mismatch");
    if (context.generation !== this.capabilities.generation)
      throw new AcpAuthorityFault("generation_mismatch");
    const session = await this.#options.sessions.inspect(context.sessionId);
    if (session === undefined) throw new AcpAuthorityFault("session_not_found");
    if (session.connectionRef !== context.connectionRef)
      throw new AcpAuthorityFault("connection_mismatch");
    if (session.generation !== context.generation)
      throw new AcpAuthorityFault("generation_mismatch");
    if (!session.authenticated) throw new AcpAuthorityFault("session_unauthenticated");
    if (session.scopeRef !== context.scopeRef) throw new AcpAuthorityFault("scope_mismatch");
    if (session.health !== "healthy" || (await brokerHealth()) !== "healthy")
      throw new AcpAuthorityFault("authority_unhealthy");
    return Object.freeze({
      requestRef: context.requestRef,
      sessionId: context.sessionId,
      connectionRef: context.connectionRef,
      generation: context.generation,
      scopeRef: context.scopeRef,
      ...(context.signal === undefined ? {} : { signal: context.signal }),
    });
  }

  #assertSession(request: Readonly<{ sessionId: string }>, context: AcpAuthorityContext): void {
    if (request.sessionId !== context.sessionId) throw new AcpAuthorityFault("session_not_found");
    if (context.signal?.aborted === true) throw new AcpAuthorityFault("cancelled");
  }

  async #record(
    method: string,
    context: AcpAuthorityContext,
    startedAt: string,
    outcome: AcpAuthorityReceipt["outcome"],
    evidenceRefs?: ReadonlyArray<string>,
    faultCode?: AcpAuthorityFaultCode,
  ): Promise<void> {
    const receipt: AcpAuthorityReceipt = Object.freeze({
      receiptRef: this.#options.nextRef("receipt"),
      requestRef: context.requestRef,
      method,
      connectionRef: context.connectionRef,
      generation: context.generation,
      sessionRef: context.sessionId,
      scopeRef: context.scopeRef,
      outcome,
      ...(faultCode === undefined ? {} : { faultCode }),
      evidenceRefs: safeEvidenceRefs(evidenceRefs),
      redaction: "safe-metadata-only",
      startedAt,
      endedAt: this.#options.now(),
    });
    try {
      await this.#options.receipts?.record(receipt);
    } catch {
      // Evidence persistence must not change an already decided authority outcome.
    }
  }

  async #run<Value>(
    method: string,
    context: AcpAuthorityContext,
    payload: unknown,
    operation: () => Promise<AcpBrokerResult<Value>>,
  ): Promise<Value> {
    return this.#memo(method, context, payload, async () => {
      const startedAt = this.#options.now();
      try {
        const result = await operation();
        if (
          (method.startsWith("fs/") || method.startsWith("terminal/")) &&
          decodeStableAcpMethodPayload({
            direction: "agent-to-client",
            method,
            phase: "result",
            payload: result.value,
          })._tag !== "Decoded"
        ) {
          throw new AcpAuthorityFault("broker_failure");
        }
        await this.#record(method, context, startedAt, "allowed", result.evidenceRefs);
        return result.value;
      } catch (error) {
        const fault =
          error instanceof AcpAuthorityFault
            ? error
            : error instanceof Error &&
                error.name === "NodeBrokerFault" &&
                (error as Error & { code?: string }).code === "aborted"
              ? new AcpAuthorityFault("cancelled")
              : new AcpAuthorityFault("broker_failure");
        await this.#record(method, context, startedAt, "refused", undefined, fault.code);
        throw fault;
      }
    });
  }

  #memo<Value>(
    method: string,
    context: AcpAuthorityContext,
    payload: unknown,
    operation: () => Promise<Value>,
  ): Promise<Value> {
    const key = `${context.connectionRef}:${context.generation}:${context.sessionId}:${context.scopeRef}:${method}:${context.requestRef}`;
    const fingerprint = createHash("sha256")
      .update(JSON.stringify([method, payload]))
      .digest("hex");
    const existing = this.#responses.get(key);
    if (existing !== undefined) {
      if (existing.fingerprint !== fingerprint)
        return Promise.reject(new AcpAuthorityFault("decision_conflict"));
      return existing.promise as Promise<Value>;
    }
    if (this.#responses.size >= (this.#options.maxReplayEntries ?? 1_024))
      return Promise.reject(new AcpAuthorityFault("overloaded"));
    const promise = operation();
    this.#responses.set(key, { fingerprint, promise });
    return promise;
  }

  close(): void {
    this.#responses.clear();
  }

  requestPermission(
    request: RequestPermissionRequest,
    context: AcpAuthorityContext,
  ): Promise<RequestPermissionResponse> {
    return this.#memo("session/request_permission", context, request, () =>
      this.#requestPermissionOnce(request, context),
    );
  }

  async #requestPermissionOnce(
    request: RequestPermissionRequest,
    context: AcpAuthorityContext,
  ): Promise<RequestPermissionResponse> {
    const startedAt = this.#options.now();
    try {
      this.#assertSession(request, context);
      const policy = this.#options.permissionPolicy;
      const lease = await this.#lease(context, this.capabilities.permission, healthOf(policy));
      if (request.options.length === 0) throw new AcpAuthorityFault("invalid_decision");
      const policyResult = await policy!.decide(request, lease);
      const offered = new Map(request.options.map((option) => [option.optionId, option]));
      if (policyResult.kind === "selected") {
        const selected = offered.get(policyResult.optionId);
        if (selected === undefined || selected.kind.startsWith("allow_") !== policyResult.allowed)
          throw new AcpAuthorityFault("invalid_decision");
        await this.#record(
          "session/request_permission",
          context,
          startedAt,
          policyResult.allowed ? "allowed" : "refused",
          policyResult.evidenceRefs,
        );
        return { outcome: { outcome: "selected", optionId: policyResult.optionId } };
      }
      const broker = this.#options.interactions;
      if (broker === undefined || (await broker.health()) !== "healthy")
        throw new AcpAuthorityFault("authority_unhealthy");
      const approveOption = offered.get(policyResult.approveOptionId);
      const denyOption = offered.get(policyResult.denyOptionId);
      if (
        approveOption === undefined ||
        denyOption === undefined ||
        !approveOption.kind.startsWith("allow_") ||
        !denyOption.kind.startsWith("reject_")
      )
        throw new AcpAuthorityFault("invalid_decision");
      const requestedAt = this.#options.now();
      const expiresAt = new Date(
        Date.parse(requestedAt) + (this.#options.interactionTtlMs ?? 300_000),
      ).toISOString();
      const interaction = decodeRuntimeInteraction({
        schema: "openagents.runtime_interaction.v1",
        interactionRef: this.#options.nextRef("interaction"),
        threadId: this.#options.threadId,
        turnId: this.#options.turnId,
        requestedSequence: context.generation,
        requestedAt,
        expiresAt,
        source: this.#options.source,
        visibility: "private",
        redactionClass: "private_ref",
        causalityRefs: [context.requestRef],
        payload: {
          kind: "tool_approval",
          displayText: "The agent requested permission for a provider tool call.",
          toolCallId: this.#options.nextRef("decision"),
          toolName: "ACP provider tool",
          authority: {
            authorityRef: policyResult.authorityRef,
            policyRef: policyResult.policyRef,
            decisionRef: policyResult.decisionRef,
            toolRef: this.#options.nextRef("decision"),
            status: "operator_escalation_required",
            allowed: false,
            blockerRefs: ["blocker.operator_decision_required"],
          },
        },
        lifecycle: { status: "pending" },
      });
      const brokerResult = await broker!.request(interaction, {
        delivery: pendingInteractionDelivery("permission", true),
      });
      if (brokerResult.kind === "cancelled") {
        await this.#record(
          "session/request_permission",
          context,
          startedAt,
          "cancelled",
          brokerResult.evidenceRefs,
        );
        return { outcome: { outcome: "cancelled" } };
      }
      let envelope;
      try {
        envelope = decodeRuntimeInteractionDecisionEnvelope(brokerResult.envelope);
      } catch {
        throw new AcpAuthorityFault("invalid_decision");
      }
      const decision = applyRuntimeInteractionDecision(interaction, envelope, this.#options.now());
      if (decision.state !== "applied" && decision.state !== "duplicate") {
        const code: AcpAuthorityFaultCode =
          decision.state === "expired"
            ? "interaction_expired"
            : decision.state === "revoked"
              ? "interaction_revoked"
              : decision.state === "conflict"
                ? "decision_conflict"
                : "invalid_decision";
        throw new AcpAuthorityFault(code);
      }
      if (envelope.decision.kind !== "tool_approval")
        throw new AcpAuthorityFault("invalid_decision");
      const selected =
        envelope.decision.outcome === "approve"
          ? policyResult.approveOptionId
          : policyResult.denyOptionId;
      const selectedOption = offered.get(selected)!;
      await this.#record(
        "session/request_permission",
        context,
        startedAt,
        selectedOption.kind.startsWith("allow_") ? "allowed" : "refused",
        brokerResult.evidenceRefs,
      );
      return { outcome: { outcome: "selected", optionId: selected } };
    } catch (error) {
      const fault =
        error instanceof AcpAuthorityFault ? error : new AcpAuthorityFault("broker_failure");
      await this.#record(
        "session/request_permission",
        context,
        startedAt,
        "refused",
        undefined,
        fault.code,
      );
      throw fault;
    }
  }

  async readTextFile(request: ReadTextFileRequest, context: AcpAuthorityContext) {
    this.#assertSession(request, context);
    return await this.#run("fs/read_text_file", context, request, async () => {
      const broker = this.#options.filesystem;
      const lease = await this.#lease(
        context,
        this.capabilities.filesystem.readTextFile,
        healthOf(broker),
      );
      return broker!.readTextFile!(request, lease);
    });
  }

  async writeTextFile(request: WriteTextFileRequest, context: AcpAuthorityContext) {
    this.#assertSession(request, context);
    return await this.#run("fs/write_text_file", context, request, async () => {
      const broker = this.#options.filesystem;
      const lease = await this.#lease(
        context,
        this.capabilities.filesystem.writeTextFile,
        healthOf(broker),
      );
      return broker!.writeTextFile!(request, lease);
    });
  }

  async createTerminal(request: CreateTerminalRequest, context: AcpAuthorityContext) {
    this.#assertSession(request, context);
    return await this.#terminal("terminal/create", request, context, (broker, lease) =>
      broker.create(request, lease),
    );
  }

  async terminalOutput(request: TerminalOutputRequest, context: AcpAuthorityContext) {
    this.#assertSession(request, context);
    return await this.#terminal("terminal/output", request, context, (broker, lease) =>
      broker.output(request, lease),
    );
  }

  async waitForTerminalExit(request: WaitForTerminalExitRequest, context: AcpAuthorityContext) {
    this.#assertSession(request, context);
    return await this.#terminal("terminal/wait_for_exit", request, context, (broker, lease) =>
      broker.waitForExit(request, lease),
    );
  }

  async killTerminal(request: KillTerminalRequest, context: AcpAuthorityContext) {
    this.#assertSession(request, context);
    return await this.#terminal("terminal/kill", request, context, (broker, lease) =>
      broker.kill(request, lease),
    );
  }

  async releaseTerminal(request: ReleaseTerminalRequest, context: AcpAuthorityContext) {
    this.#assertSession(request, context);
    return await this.#terminal("terminal/release", request, context, (broker, lease) =>
      broker.release(request, lease),
    );
  }

  #terminal<Value>(
    wireMethod: string,
    request: unknown,
    context: AcpAuthorityContext,
    invoke: (
      broker: AcpTerminalBrokerPort,
      lease: AcpAuthorityLease,
    ) => Promise<AcpBrokerResult<Value>>,
  ): Promise<Value> {
    return this.#run(wireMethod, context, request, async () => {
      const broker = this.#options.terminal;
      const lease = await this.#lease(context, this.capabilities.terminal, healthOf(broker));
      return invoke(broker!, lease);
    });
  }

  createSessionWithMcp(
    request: Readonly<{
      cwdRef: string;
      servers: ReadonlyArray<
        Readonly<{ serverRef: string; transport: "http" | "sse" | "stdio"; expiresAt: string }>
      >;
    }>,
    context: AcpAuthorityContext,
    launch: (material: ReadonlyArray<McpServer>) => Promise<NewSessionResponse>,
  ): Promise<NewSessionResponse> {
    return this.#run("session/mcp_materialize", context, request, async () => {
      const broker = this.#options.mcp;
      const now = Date.parse(this.#options.now());
      if (
        !safeRefPattern.test(request.cwdRef) ||
        !Number.isFinite(now) ||
        request.servers.some((server) => {
          const expiry = Date.parse(server.expiresAt);
          return (
            !safeRefPattern.test(server.serverRef) || !Number.isFinite(expiry) || expiry <= now
          );
        })
      )
        throw new AcpAuthorityFault("interaction_expired");
      const requested = new Set(request.servers.map((server) => server.transport));
      const supported = new Set(this.capabilities.mcpTransports);
      if ([...requested].some((type) => !supported.has(type as "http" | "sse" | "stdio")))
        throw new AcpAuthorityFault("capability_not_advertised");
      const lease = await this.#lease(
        context,
        this.capabilities.mcpTransports.length > 0,
        healthOf(broker),
      );
      return broker!.materializeForSessionNew(request, lease, launch);
    });
  }
}
