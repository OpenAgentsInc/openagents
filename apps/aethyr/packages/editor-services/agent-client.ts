import {
  AGENT_PROTOCOL,
  AGENT_CAPTURE_SCHEMA,
  AGENT_LOG_SCHEMA,
  AGENT_LIMITS,
  agentExpectation,
  validateAgentAction,
  validateAgentCaptureRequest,
  validateAgentCaptureCamera,
  validateAgentContext,
  validateAgentExpectation,
  validateAgentInspectOptions,
  type AgentAction,
  type AgentCaptureEnvelope,
  type AgentCaptureRequest,
  type AgentContext,
  type AgentDiscovery,
  type AgentExpectation,
  type AgentInspection,
  type AgentInspectOptions,
  type AgentReceipt,
  type AgentRunLog,
  type AgentStep,
  type AgentTransport,
} from "../runtime/agent-contract.ts";
import { sameCaptureActorReference } from "../runtime/capture-actors.ts";

const utf8 = new TextEncoder();
const hash = (value: unknown): value is string =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const integer = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
const text = (value: unknown, limit = 96): value is string =>
  typeof value === "string" &&
  value.length > 0 &&
  utf8.encode(value).byteLength <= limit;
const vector = (value: unknown): value is [number, number, number] =>
  Array.isArray(value) &&
  value.length === 3 &&
  value.every(
    (n) =>
      typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= 100_000,
  );
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
function sameContext(actual: AgentContext, expected: AgentExpectation) {
  return (
    actual.sourceIdentity === expected.sourceIdentity &&
    actual.revision === expected.revision &&
    actual.generation === expected.generation &&
    actual.shaderIdentity === expected.shaderIdentity
  );
}
function receipt(value: unknown, requestId?: string): AgentReceipt {
  if (
    !record(value) ||
    !text(value.requestId) ||
    (requestId !== undefined && value.requestId !== requestId) ||
    typeof value.ok !== "boolean" ||
    typeof value.message !== "string" ||
    utf8.encode(value.message).byteLength > 4096 ||
    !integer(value.revision) ||
    (value.stage !== undefined &&
      !["rejected", "activated", "rendered", "saved"].includes(
        value.stage as string,
      )) ||
    (value.saved !== undefined && typeof value.saved !== "boolean")
  )
    throw new Error(
      "The agent received an invalid or mismatched command result.",
    );
  if (
    value.diagnostics !== undefined &&
    (!Array.isArray(value.diagnostics) ||
      value.diagnostics.length > 8 ||
      value.diagnostics.some(
        (item) =>
          typeof item !== "string" || utf8.encode(item).byteLength > 512,
      ))
  )
    throw new Error("Agent diagnostics exceed their bounded result size.");
  if (
    value.timings !== undefined &&
    (!record(value.timings) ||
      Object.keys(value.timings).length > 32 ||
      Object.entries(value.timings).some(
        ([key, n]) =>
          !text(key, 64) ||
          typeof n !== "number" ||
          !Number.isFinite(n) ||
          n < 0,
      ))
  )
    throw new Error("The agent received invalid timing measurements.");
  return {
    requestId: value.requestId,
    ok: value.ok,
    message: value.message,
    revision: value.revision,
    ...(value.stage === undefined
      ? {}
      : { stage: value.stage as AgentReceipt["stage"] }),
    ...(value.saved === undefined ? {} : { saved: value.saved }),
    ...(value.diagnostics === undefined
      ? {}
      : { diagnostics: structuredClone(value.diagnostics) as string[] }),
    ...(value.timings === undefined
      ? {}
      : { timings: structuredClone(value.timings) as Record<string, number> }),
  };
}
function inspection(value: unknown): AgentInspection {
  if (
    !record(value) ||
    value.schema !== AGENT_PROTOCOL ||
    !text(value.requestId) ||
    !text(value.sessionId) ||
    !hash(value.authoredIdentity) ||
    !Array.isArray(value.objects) ||
    value.objects.length > AGENT_LIMITS.inspectObjects ||
    !integer(value.totalObjects) ||
    value.totalObjects < value.objects.length ||
    value.totalObjects > 100_000 ||
    typeof value.truncated !== "boolean" ||
    (value.nextCursor !== undefined &&
      (!integer(value.nextCursor) || value.nextCursor > value.totalObjects)) ||
    !record(value.player) ||
    !vector(value.player.position) ||
    typeof value.player.yaw !== "number" ||
    !Number.isFinite(value.player.yaw) ||
    typeof value.player.pitch !== "number" ||
    !Number.isFinite(value.player.pitch) ||
    !record(value.view) ||
    !record(value.render) ||
    !["frame", "instances", "triangles"].every((key) =>
      integer((value.render as Record<string, unknown>)[key]),
    ) ||
    !Array.isArray(value.capabilities) ||
    value.capabilities.length > 64 ||
    value.capabilities.some((item) => !text(item))
  )
    throw new Error(
      "The agent received an incomplete or oversized world inspection.",
    );
  validateAgentContext(value.context);
  if (utf8.encode(JSON.stringify(value)).byteLength > 256 * 1024)
    throw new Error("The agent inspection exceeds its bounded response size.");
  if (
    value.metrics !== undefined &&
    (!record(value.metrics) ||
      Object.keys(value.metrics).length > 64 ||
      Object.entries(value.metrics).some(
        ([key, n]) =>
          !text(key, 64) || typeof n !== "number" || !Number.isFinite(n),
      ))
  )
    throw new Error("The inspection contains invalid runtime metrics.");
  const seenIds = new Set<string>();
  for (const object of value.objects) {
    if (
      !record(object) ||
      !text(object.id) ||
      !text(object.kind, 128) ||
      !vector(object.position) ||
      typeof object.yaw !== "number" ||
      !Number.isFinite(object.yaw) ||
      typeof object.scale !== "number" ||
      !Number.isFinite(object.scale) ||
      object.scale < 0.05 ||
      object.scale > 20 ||
      typeof object.collidable !== "boolean" ||
      (object.halfExtents != null && !vector(object.halfExtents)) ||
      (object.collisionOffset != null && !vector(object.collisionOffset))
    )
      throw new Error("The agent received invalid authored object data.");
    if (seenIds.has(object.id))
      throw new Error(
        "The inspection lists an authored object more than once.",
      );
    seenIds.add(object.id);
  }
  if (
    value.readOnlyObjects !== undefined &&
    (!Array.isArray(value.readOnlyObjects) ||
      value.readOnlyObjects.length > AGENT_LIMITS.inspectObjects ||
      value.readOnlyObjects.some(
        (object) =>
          !record(object) ||
          !text(object.id) ||
          !text(object.kind, 128) ||
          !vector(object.position) ||
          object.editable !== false,
      ))
  )
    throw new Error(
      "The agent received an oversized source-object inspection.",
    );
  const view = value.view;
  validateAgentAction({
    type: "camera",
    eye: view.eye,
    target: view.target,
    ...(view.up === undefined ? {} : { up: view.up }),
    fov: view.fov,
    mode: view.mode,
  });
  validateAgentAction({ type: "input", input: value.input, durationMs: 1 });
  if (
    value.lastWorkingCode !== undefined &&
    (!record(value.lastWorkingCode) ||
      !hash(value.lastWorkingCode.identity) ||
      value.lastWorkingCode.identity !==
        (value.context as AgentContext).shaderIdentity ||
      (value.lastWorkingCode.source !== undefined &&
        (typeof value.lastWorkingCode.source !== "string" ||
          utf8.encode(value.lastWorkingCode.source).byteLength >
            AGENT_LIMITS.shaderBytes)))
  )
    throw new Error(
      "The last working code does not match the inspected shader.",
    );
  return structuredClone(value) as unknown as AgentInspection;
}
function runLog(value: unknown): AgentRunLog {
  let size: number;
  try {
    size = utf8.encode(JSON.stringify(value)).byteLength;
  } catch {
    throw new Error("Open a serializable agent run log.");
  }
  if (
    size > AGENT_LIMITS.logBytes ||
    !record(value) ||
    value.schema !== AGENT_LOG_SCHEMA ||
    !text(value.worldId) ||
    !hash(value.sourceIdentity) ||
    !hash(value.initialAuthoredIdentity) ||
    !Array.isArray(value.entries) ||
    value.entries.length < 1 ||
    value.entries.length > AGENT_LIMITS.logEntries
  )
    throw new Error(
      "Open a compatible agent run log of at most 64 entries and 1 MiB.",
    );
  const initial = validateAgentContext(value.initialContext);
  if (
    initial.worldId !== value.worldId ||
    initial.sourceIdentity !== value.sourceIdentity
  )
    throw new Error("The agent log contains conflicting initial identities.");
  let expectedAuthored = value.initialAuthoredIdentity,
    expectedShader = initial.shaderIdentity;
  for (let i = 0; i < value.entries.length; i++) {
    const step = value.entries[i];
    if (
      !record(step) ||
      step.sequence !== i + 1 ||
      !hash(step.beforeAuthoredIdentity) ||
      !hash(step.afterAuthoredIdentity) ||
      typeof step.elapsedMs !== "number" ||
      !Number.isFinite(step.elapsedMs) ||
      step.elapsedMs < 0
    )
      throw new Error("The agent log contains an invalid step or sequence.");
    validateAgentAction(step.action);
    receipt(step.receipt);
    for (const context of [
      validateAgentContext(step.before),
      validateAgentContext(step.after),
    ])
      if (
        context.worldId !== value.worldId ||
        context.sourceIdentity !== value.sourceIdentity
      )
        throw new Error("The agent log crosses a world or source boundary.");
    const before = step.before as AgentContext,
      after = step.after as AgentContext,
      result = step.receipt as unknown as AgentReceipt;
    if (
      step.beforeAuthoredIdentity !== expectedAuthored ||
      before.shaderIdentity !== expectedShader
    )
      throw new Error(
        "The run contains an unrecorded document or working-shader change.",
      );
    if (
      !result.ok &&
      (step.afterAuthoredIdentity !== expectedAuthored ||
        after.shaderIdentity !== expectedShader)
    )
      throw new Error(
        "A rejected action changed the document or working shader in this run.",
      );
    expectedAuthored = step.afterAuthoredIdentity;
    expectedShader = after.shaderIdentity;
  }
  return structuredClone(value) as unknown as AgentRunLog;
}
async function digest(bytes: ArrayBuffer) {
  return Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (n) => n.toString(16).padStart(2, "0"),
  ).join("");
}
function normalized(point: number[]) {
  const length = Math.hypot(...point);
  return point.map((n) => n / length);
}
function closeVector(a: number[], b: number[], tolerance = 0.00001) {
  return (
    a.length === b.length && a.every((n, i) => Math.abs(n - b[i]) <= tolerance)
  );
}
interface AgentFlight {
  requestId: string;
  action: AgentAction;
  before: AgentInspection;
  started: number;
}

/** Bounded client tools; every mutation still crosses the existing trusted editor transport. */
export class AgentClient {
  readonly #transport: AgentTransport;
  readonly #now: () => number;
  readonly #pending = new Map<symbol, (error: Error) => void>();
  #sessionId?: string;
  #identity?: { worldId: string; sourceIdentity: string };
  #initial?: { context: AgentContext; authoredIdentity: string };
  #entries: AgentStep[] = [];
  #busy = false;
  #closed = false;
  #closePromise?: Promise<void>;
  #unresolved?: string;
  #flight?: AgentFlight;
  #completion?: Promise<AgentStep>;
  #captureSequence = 0;
  constructor(transport: AgentTransport, options: { now?: () => number } = {}) {
    this.#transport = transport;
    this.#now = options.now ?? (() => performance.now());
  }
  async discover(): Promise<AgentDiscovery> {
    this.#open();
    const value = await this.#call(() => this.#transport.discover());
    this.#open();
    if (
      !value ||
      value.schema !== AGENT_PROTOCOL ||
      !text(value.sessionId) ||
      !text(value.worldId) ||
      !hash(value.sourceIdentity) ||
      !Array.isArray(value.capabilities) ||
      value.capabilities.length > 64 ||
      value.capabilities.some((capability) => !text(capability)) ||
      !value.limits ||
      Object.entries(AGENT_LIMITS).some(
        ([key, limit]) =>
          value.limits[key as keyof typeof AGENT_LIMITS] !== limit,
      )
    )
      throw new Error(
        "This agent transport uses an incompatible discovery contract.",
      );
    this.#bind(value.sessionId, value);
    return structuredClone(value);
  }
  async inspect(options: AgentInspectOptions = {}): Promise<AgentInspection> {
    this.#open();
    const checked = validateAgentInspectOptions(options);
    const value = inspection(
      await this.#call(() => this.#transport.inspect(checked)),
    );
    this.#open();
    this.#bind(value.sessionId, value.context);
    if (
      checked.expectedRevision !== undefined &&
      checked.expectedRevision !== value.context.revision
    )
      throw new Error(
        "The inspected world changed from the expected revision.",
      );
    return value;
  }
  async execute(
    action: AgentAction,
    expectationOverride?: AgentExpectation,
  ): Promise<AgentStep> {
    this.#begin();
    try {
      return await this.#perform(action, expectationOverride);
    } finally {
      this.#busy = false;
    }
  }
  async result(requestId: string): Promise<AgentReceipt> {
    this.#open();
    if (!text(requestId))
      throw new Error("Choose a valid agent command request ID.");
    if (this.#flight?.requestId === requestId && !this.#busy)
      return (await this.#completeFlight()).receipt;
    return this.#readReceipt(requestId);
  }
  async capture(request: AgentCaptureRequest): Promise<AgentCaptureEnvelope> {
    this.#open();
    if (this.#busy)
      throw new Error(
        "Finish the current agent action before capturing its result.",
      );
    const checked = validateAgentCaptureRequest(request);
    const value = await this.#call(() => this.#transport.capture(checked));
    if (
      !value ||
      value.schema !== AGENT_CAPTURE_SCHEMA ||
      !text(value.requestId) ||
      !text(value.sessionId) ||
      !integer(value.sequence) ||
      value.sequence <= this.#captureSequence ||
      !integer(value.frameBefore) ||
      !integer(value.frameAfter) ||
      value.frameAfter <= value.frameBefore ||
      value.width !== checked.width ||
      value.height !== checked.height ||
      value.time !== checked.time ||
      value.exposure !== checked.exposure ||
      (checked.actorReference
        ? !sameCaptureActorReference(value.actors, checked.actorReference)
        : value.actors !== undefined) ||
      !(value.pixels instanceof ArrayBuffer) ||
      value.pixels.byteLength !== checked.width * checked.height * 4 ||
      !hash(value.pixelSha256)
    )
      throw new Error(
        "The agent received a stale or incomplete frame capture.",
      );
    const context = validateAgentContext(value.context);
    this.#bind(value.sessionId, context);
    if (!sameContext(context, checked.expect))
      throw new Error("The captured frame belongs to another world state.");
    const actual = validateAgentCaptureCamera(value.camera);
    if (
      !closeVector(actual.eye, checked.camera.eye) ||
      Math.abs(actual.fov - checked.camera.fov) > 0.00001 ||
      (actual.vignette ?? null)?.shape !==
        (checked.camera.vignette ?? null)?.shape ||
      (actual.vignette ?? null)?.intensity !==
        (checked.camera.vignette ?? null)?.intensity ||
      (actual.vignette ?? null)?.blendWeight !==
        (checked.camera.vignette ?? null)?.blendWeight ||
      !closeVector(
        normalized(actual.target.map((n, i) => n - actual.eye[i])),
        normalized(
          checked.camera.target.map((n, i) => n - checked.camera.eye[i]),
        ),
      ) ||
      (checked.camera.up !== undefined &&
        (!actual.up ||
          !closeVector(normalized(actual.up), normalized(checked.camera.up))))
    )
      throw new Error("The captured frame uses a different camera.");
    if ((await digest(value.pixels)) !== value.pixelSha256)
      throw new Error("The captured pixels do not match their hash.");
    this.#open();
    this.#captureSequence = value.sequence;
    return value;
  }
  exportLog(): AgentRunLog {
    this.#open();
    if (this.#unresolved)
      throw new Error(
        "Resolve the pending action result before exporting its run.",
      );
    if (!this.#initial || !this.#entries.length)
      throw new Error("Run an agent action before exporting its log.");
    return structuredClone(this.#log());
  }
  async replay(candidate: unknown): Promise<AgentStep[]> {
    this.#begin();
    try {
      if (this.#entries.length)
        throw new Error(
          "Replay into a new agent client and a matching initial world.",
        );
      const log = runLog(candidate),
        current = await this.inspect();
      if (
        current.context.worldId !== log.worldId ||
        current.context.sourceIdentity !== log.sourceIdentity ||
        current.authoredIdentity !== log.initialAuthoredIdentity ||
        current.context.shaderIdentity !== log.initialContext.shaderIdentity
      )
        throw new Error(
          "The replay target does not match the run's initial world, document, or shader.",
        );
      const replayed: AgentStep[] = [];
      let expectedAuthored = log.initialAuthoredIdentity;
      for (const old of log.entries) {
        if (old.beforeAuthoredIdentity !== expectedAuthored)
          throw new Error(
            "The run contains an unrecorded authored change and cannot replay safely.",
          );
        if (!old.receipt.ok) {
          if (old.afterAuthoredIdentity !== expectedAuthored)
            throw new Error(
              "A rejected action changed the authored document in this run.",
            );
          continue;
        }
        const next = await this.#perform(old.action);
        replayed.push(next);
        if (
          !next.receipt.ok ||
          next.afterAuthoredIdentity !== old.afterAuthoredIdentity ||
          next.after.shaderIdentity !== old.after.shaderIdentity
        )
          throw new Error(
            "The replay did not reproduce the accepted action's document and working shader.",
          );
        expectedAuthored = old.afterAuthoredIdentity;
      }
      return structuredClone(replayed);
    } finally {
      this.#busy = false;
    }
  }
  async close(): Promise<void> {
    if (this.#closePromise) return this.#closePromise;
    this.#closed = true;
    for (const reject of this.#pending.values())
      reject(
        new Error("The agent client closed before the operation finished."),
      );
    this.#pending.clear();
    this.#closePromise = (async () => {
      await this.#transport.close();
    })();
    return this.#closePromise;
  }
  #open() {
    if (this.#closed)
      throw new Error("Open a new agent client after closing this one.");
  }
  #begin() {
    this.#open();
    if (this.#busy)
      throw new Error(
        "Finish the current agent action before starting another.",
      );
    if (this.#unresolved)
      throw new Error(
        "Resolve the previous action result before submitting another action.",
      );
    this.#busy = true;
  }
  #bind(
    sessionId: string,
    context: { worldId: string; sourceIdentity: string },
  ) {
    if (
      (this.#sessionId && this.#sessionId !== sessionId) ||
      (this.#identity &&
        (this.#identity.worldId !== context.worldId ||
          this.#identity.sourceIdentity !== context.sourceIdentity))
    )
      throw new Error("The agent world or session changed. Open a new client.");
    this.#sessionId = sessionId;
    this.#identity = {
      worldId: context.worldId,
      sourceIdentity: context.sourceIdentity,
    };
  }
  async #call<Result>(
    operation: () => Promise<Result> | Result,
  ): Promise<Result> {
    this.#open();
    if (this.#pending.size >= 16)
      throw new Error("Let the current agent requests finish first.");
    const key = Symbol(),
      closed = new Promise<never>((_, reject) =>
        this.#pending.set(key, reject),
      );
    try {
      return await Promise.race([Promise.resolve(operation()), closed]);
    } finally {
      this.#pending.delete(key);
    }
  }
  #log(): AgentRunLog {
    return {
      schema: AGENT_LOG_SCHEMA,
      worldId: this.#initial!.context.worldId,
      sourceIdentity: this.#initial!.context.sourceIdentity,
      initialContext: this.#initial!.context,
      initialAuthoredIdentity: this.#initial!.authoredIdentity,
      entries: this.#entries,
    };
  }
  async #perform(
    action: AgentAction,
    expectationOverride?: AgentExpectation,
  ): Promise<AgentStep> {
    const checked = validateAgentAction(action),
      before = await this.inspect();
    const expect =
      expectationOverride === undefined
        ? agentExpectation(before.context)
        : validateAgentExpectation(expectationOverride);
    this.#initial ??= {
      context: structuredClone(before.context),
      authoredIdentity: before.authoredIdentity,
    };
    // Reserve the complete bounded receipt/context overhead before sending a mutation.
    if (
      this.#entries.length >= AGENT_LIMITS.logEntries ||
      utf8.encode(JSON.stringify(this.#log())).byteLength +
        utf8.encode(JSON.stringify(checked)).byteLength +
        16_384 >
        AGENT_LIMITS.logBytes
    )
      throw new Error(
        "This agent run reached its 64-entry or 1 MiB log limit. Start a new run.",
      );
    const started = this.#now();
    const requestId = await this.#call(() =>
      this.#transport.execute(checked, expect),
    );
    if (!text(requestId))
      throw new Error("The agent action did not return a request ID.");
    this.#unresolved = requestId;
    this.#flight = { requestId, action: checked, before, started };
    return this.#completeFlight();
  }
  async #readReceipt(requestId: string): Promise<AgentReceipt> {
    return receipt(
      await this.#call(() => this.#transport.result(requestId)),
      requestId,
    );
  }
  async #completeFlight(): Promise<AgentStep> {
    if (this.#completion) return this.#completion;
    const flight = this.#flight;
    if (!flight)
      throw new Error("There is no unresolved agent action to recover.");
    const completion = this.#finishFlight(flight);
    this.#completion = completion;
    try {
      return await completion;
    } finally {
      if (this.#completion === completion) this.#completion = undefined;
    }
  }
  async #finishFlight(flight: AgentFlight): Promise<AgentStep> {
    const { requestId, before, action: checked, started } = flight;
    const outcome = await this.#readReceipt(requestId);
    const after = await this.inspect();
    this.#open();
    const entry = {
      sequence: this.#entries.length + 1,
      action: checked,
      before: before.context,
      after: after.context,
      beforeAuthoredIdentity: before.authoredIdentity,
      afterAuthoredIdentity: after.authoredIdentity,
      receipt: outcome,
      elapsedMs: Math.max(0, this.#now() - started),
    };
    const projected = { ...this.#log(), entries: [...this.#entries, entry] };
    if (
      utf8.encode(JSON.stringify(projected)).byteLength > AGENT_LIMITS.logBytes
    )
      throw new Error("The agent result exceeded the reserved run-log size.");
    this.#entries.push(structuredClone(entry));
    this.#unresolved = undefined;
    this.#flight = undefined;
    return structuredClone(entry);
  }
}
