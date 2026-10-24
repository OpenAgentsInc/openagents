import type { EnemySnapshot } from "./enemies.ts";
import type {
  EditorCommand,
  EngineStatus,
  InputSample,
} from "../editor-services/protocol.ts";
import type { SourceVignetteOverride } from "../assets/src/scene.ts";
import {
  validateCaptureActorReferenceMetadata,
  type ActorCaptureReferenceMetadata,
} from "./capture-actors.ts";

export const AGENT_PROTOCOL = "aethyrforge.agent.v1" as const;
export const AGENT_CAPTURE_SCHEMA = "aethyrforge.agent.capture.v1" as const;
export const AGENT_LOG_SCHEMA = "aethyrforge.agent.run.v1" as const;
export const AGENT_LIMITS = Object.freeze({
  inspectObjects: 64,
  logEntries: 64,
  logBytes: 1024 * 1024,
  shaderBytes: 65_536,
  inputDurationMs: 1000,
  captureBytes: 32 * 1024 * 1024,
});
export type AgentVector = [number, number, number];
export type SpellSwarmCount = 0 | 10000 | 100000 | 200000;
export function validSpellSwarmCount(value: unknown): value is SpellSwarmCount {
  return value === 0 || value === 10000 || value === 100000 || value === 200000;
}
export interface AgentContext {
  worldId: string;
  sourceIdentity: string;
  revision: number;
  generation: number;
  tick: number;
  shaderIdentity: string;
}
export type AgentExpectation = Pick<
  AgentContext,
  "sourceIdentity" | "revision" | "generation" | "shaderIdentity"
>;
export interface AgentCamera {
  eye: AgentVector;
  target: AgentVector;
  up?: AgentVector;
  fov: number;
}
export interface AgentCaptureCamera extends AgentCamera {
  /** Omitted and null both disable vignette for reproducible agent captures. */
  vignette?: SourceVignetteOverride | null;
}
export interface AgentInspectOptions {
  cursor?: number;
  limit?: number;
  expectedRevision?: number;
}
export interface AgentObject {
  id: string;
  kind: string;
  position: AgentVector;
  yaw: number;
  scale: number;
  collidable: boolean;
  halfExtents?: AgentVector | null;
  collisionOffset?: AgentVector | null;
  handle?: { slot: number; generation: number };
}
export interface AgentReadOnlyObject {
  id: string;
  kind: string;
  position: AgentVector;
  editable: false;
  sourceIndex?: number;
  bounds?: { min: AgentVector; max: AgentVector };
}
export interface AgentInspection {
  schema: typeof AGENT_PROTOCOL;
  requestId: string;
  sessionId: string;
  context: AgentContext;
  /** Stable semantic document content, excluding revision and transient runtime handles. */
  authoredIdentity: string;
  objects: AgentObject[];
  totalObjects: number;
  nextCursor?: number;
  truncated: boolean;
  readOnlyObjects?: AgentReadOnlyObject[];
  readOnlyTotal?: number;
  player: {
    position: AgentVector;
    yaw: number;
    pitch: number;
    velocity?: AgentVector;
    grounded?: boolean;
    perspective?: "first" | "third";
  };
  game?: EngineStatus["game"];
  /** On-demand runtime poses; excluded from authored identity and UI subscriptions. */
  enemies?: EnemySnapshot;
  view: AgentCamera & { mode: "walk" | "fly" };
  input: InputSample;
  render: { frame: number; instances: number; triangles: number };
  capabilities: string[];
  metrics?: Record<string, number>;
  lastWorkingCode?: { identity: string; source?: string };
}
export interface AgentDiscovery {
  schema: typeof AGENT_PROTOCOL;
  sessionId: string;
  worldId: string;
  sourceIdentity: string;
  capabilities: string[];
  limits: typeof AGENT_LIMITS;
  readOnly?: boolean;
}
export type AgentCommand = Extract<
  EditorCommand,
  {
    kind:
      | "prop.add"
      | "entity.transform"
      | "entity.remove"
      | "history.undo"
      | "history.redo";
  }
>;
export type AgentAction =
  | { type: "command"; command: AgentCommand }
  | { type: "shader"; source: string }
  | ({ type: "camera"; mode: "walk" | "fly" } & AgentCamera)
  | { type: "input"; input: InputSample; durationMs: number }
  | { type: "spellSwarm"; count: SpellSwarmCount }
  | { type: "game"; action: "start" | "interact" | "reset" };
export interface AgentReceipt {
  requestId: string;
  ok: boolean;
  message: string;
  revision: number;
  stage?: "rejected" | "activated" | "rendered" | "saved";
  saved?: boolean;
  diagnostics?: string[];
  timings?: Record<string, number>;
}
export interface AgentCaptureRequest {
  actorReference?: ActorCaptureReferenceMetadata;
  expect: AgentExpectation;
  camera: AgentCaptureCamera;
  width: number;
  height: number;
  time: number;
  exposure: number;
}
export interface AgentCaptureEnvelope {
  actors?: ActorCaptureReferenceMetadata;
  schema: typeof AGENT_CAPTURE_SCHEMA;
  requestId: string;
  sessionId: string;
  sequence: number;
  context: AgentContext;
  frameBefore: number;
  frameAfter: number;
  camera: AgentCaptureCamera;
  width: number;
  height: number;
  time: number;
  exposure: number;
  pixelSha256: string;
  pixels: ArrayBuffer;
}
export interface AgentStep {
  sequence: number;
  action: AgentAction;
  before: AgentContext;
  after: AgentContext;
  beforeAuthoredIdentity: string;
  afterAuthoredIdentity: string;
  receipt: AgentReceipt;
  elapsedMs: number;
}
export interface AgentRunLog {
  schema: typeof AGENT_LOG_SCHEMA;
  worldId: string;
  sourceIdentity: string;
  initialContext: AgentContext;
  initialAuthoredIdentity: string;
  entries: AgentStep[];
}
export interface AgentTransport {
  discover(): Promise<AgentDiscovery>;
  inspect(options?: AgentInspectOptions): Promise<AgentInspection>;
  execute(
    action: AgentAction,
    expectation: AgentExpectation,
  ): string | Promise<string>;
  result(requestId: string): Promise<AgentReceipt>;
  capture(options: AgentCaptureRequest): Promise<AgentCaptureEnvelope>;
  /** Release this adapter's resources; do not close a shared editor session accidentally. */
  close(): void | Promise<void>;
}

const utf8 = new TextEncoder();
export function truncateAgentText(value: string, maxBytes: number): string {
  if (!Number.isSafeInteger(maxBytes) || maxBytes < 0)
    throw new Error("Choose a nonnegative text byte limit.");
  const bytes = new Uint8Array(Math.min(maxBytes, value.length * 3));
  const { written } = utf8.encodeInto(value, bytes);
  return new TextDecoder("utf-8", { ignoreBOM: true }).decode(
    bytes.subarray(0, written),
  );
}
const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const integer = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
const hash = (value: unknown): value is string =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const id = (value: unknown): value is string =>
  typeof value === "string" && /^[a-zA-Z0-9][a-zA-Z0-9_.:-]{0,95}$/.test(value);
function keys(
  value: unknown,
  allowed: string[],
): asserts value is Record<string, unknown> {
  if (
    !isRecord(value) ||
    Object.keys(value).some((key) => !allowed.includes(key))
  )
    throw new Error("Choose supported agent tool fields.");
}
function vector(value: unknown, limit = 100_000): value is AgentVector {
  return (
    Array.isArray(value) &&
    value.length === 3 &&
    value.every(
      (n) =>
        typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= limit,
    )
  );
}
export function validateAgentContext(value: unknown): AgentContext {
  keys(value, [
    "worldId",
    "sourceIdentity",
    "revision",
    "generation",
    "tick",
    "shaderIdentity",
  ]);
  if (
    !id(value.worldId) ||
    !hash(value.sourceIdentity) ||
    !hash(value.shaderIdentity) ||
    !integer(value.revision) ||
    !integer(value.generation) ||
    !integer(value.tick)
  )
    throw new Error("Use a complete agent world context.");
  return structuredClone(value) as unknown as AgentContext;
}
export function validateAgentExpectation(value: unknown): AgentExpectation {
  keys(value, ["sourceIdentity", "revision", "generation", "shaderIdentity"]);
  if (
    !hash(value.sourceIdentity) ||
    !hash(value.shaderIdentity) ||
    !integer(value.revision) ||
    !integer(value.generation)
  )
    throw new Error(
      "Use the expected source, revision, generation, and shader identities.",
    );
  return structuredClone(value) as unknown as AgentExpectation;
}
export function agentExpectation(context: AgentContext): AgentExpectation {
  const checked = validateAgentContext(context);
  return {
    sourceIdentity: checked.sourceIdentity,
    revision: checked.revision,
    generation: checked.generation,
    shaderIdentity: checked.shaderIdentity,
  };
}
export function validateAgentInspectOptions(
  value: unknown = {},
): AgentInspectOptions {
  keys(value, ["cursor", "limit", "expectedRevision"]);
  if (
    (value.cursor !== undefined &&
      (!integer(value.cursor) || value.cursor > 100_000)) ||
    (value.limit !== undefined &&
      (!integer(value.limit) ||
        value.limit < 1 ||
        value.limit > AGENT_LIMITS.inspectObjects)) ||
    (value.expectedRevision !== undefined && !integer(value.expectedRevision))
  )
    throw new Error(
      "Inspect at most 64 objects from a valid cursor and revision.",
    );
  return structuredClone(value) as AgentInspectOptions;
}
function validateCamera(value: Record<string, unknown>): void {
  if (
    !vector(value.eye, 20_000) ||
    !vector(value.target, 20_000) ||
    Math.hypot(
      ...value.eye.map((n, i) => n - (value.target as AgentVector)[i]),
    ) < 0.01 ||
    typeof value.fov !== "number" ||
    !Number.isFinite(value.fov) ||
    value.fov < 0.15 ||
    value.fov > 2.5
  )
    throw new Error("Choose a complete bounded camera pose and field of view.");
  if (value.up !== undefined) {
    if (!vector(value.up, 20_000))
      throw new Error("Choose a finite camera up direction.");
    const d = value.target.map((n, i) => n - (value.eye as AgentVector)[i]),
      u = value.up;
    if (
      Math.hypot(...u) < 1e-6 ||
      Math.hypot(
        d[1] * u[2] - d[2] * u[1],
        d[2] * u[0] - d[0] * u[2],
        d[0] * u[1] - d[1] * u[0],
      ) /
        (Math.hypot(...u) * Math.hypot(...d)) <
        1e-4
    )
      throw new Error("Choose an up direction across the camera view.");
  }
}
export function validateAgentAction(value: unknown): AgentAction {
  if (!isRecord(value)) throw new Error("Choose a supported agent action.");
  if (value.type === "command") {
    keys(value, ["type", "command"]);
    const command = value.command;
    if (!isRecord(command))
      throw new Error("Choose a supported typed editor command.");
    if (command.kind === "prop.add") {
      keys(command, ["kind", "assetId", "id", "position", "scale"]);
      if (!id(command.assetId)) throw new Error("Choose a valid prop ID.");
      if (!id(command.id)) throw new Error("Choose a valid object ID.");
      if (!vector(command.position))
        throw new Error(
          "Choose finite position coordinates within the supported world bounds.",
        );
    } else if (command.kind === "entity.transform") {
      keys(command, ["kind", "id", "position", "scale"]);
      if (!id(command.id)) throw new Error("Choose a valid object ID.");
      if (command.position === undefined && command.scale === undefined)
        throw new Error("Choose an explicit position or size.");
      if (command.position !== undefined && !vector(command.position))
        throw new Error(
          "Choose finite position coordinates within the supported world bounds.",
        );
    } else if (command.kind === "entity.remove") {
      keys(command, ["kind", "id"]);
      if (!id(command.id)) throw new Error("Choose a valid object ID.");
    } else if (
      command.kind === "history.undo" ||
      command.kind === "history.redo"
    )
      keys(command, ["kind"]);
    else
      throw new Error(
        "Use explicit stable prop commands; random placement and free text are unavailable here.",
      );
    if (
      command.scale !== undefined &&
      (typeof command.scale !== "number" ||
        !Number.isFinite(command.scale) ||
        command.scale < 0.05 ||
        command.scale > 20)
    )
      throw new Error("Choose a prop size from 0.05 to 20.");
  } else if (value.type === "shader") {
    keys(value, ["type", "source"]);
    if (
      typeof value.source !== "string" ||
      !value.source.trim() ||
      utf8.encode(value.source).byteLength > AGENT_LIMITS.shaderBytes
    )
      throw new Error("Use shader source up to 64 KiB.");
  } else if (value.type === "camera") {
    keys(value, ["type", "eye", "target", "up", "fov", "mode"]);
    validateCamera(value);
    if (value.mode !== "walk" && value.mode !== "fly")
      throw new Error("Choose Walk or Fly.");
  } else if (value.type === "input") {
    keys(value, ["type", "input", "durationMs"]);
    keys(value.input, [
      "forward",
      "right",
      "up",
      "lookX",
      "lookY",
      "sprint",
      "jump",
      "attack",
      "dance",
    ]);
    const input = value.input;
    if (
      !integer(value.durationMs) ||
      value.durationMs < 1 ||
      value.durationMs > AGENT_LIMITS.inputDurationMs ||
      !["forward", "right", "up"].every(
        (key) =>
          typeof input[key] === "number" &&
          Number.isFinite(input[key]) &&
          Math.abs(input[key]) <= 1,
      ) ||
      !["lookX", "lookY"].every(
        (key) =>
          typeof input[key] === "number" &&
          Number.isFinite(input[key]) &&
          Math.abs(input[key]) <= Math.PI * 2,
      ) ||
      typeof input.sprint !== "boolean" ||
      ["jump", "attack", "dance"].some(
        (key) => input[key] !== undefined && typeof input[key] !== "boolean",
      )
    )
      throw new Error(
        "Use bounded movement input lasting from 1 to 1,000 milliseconds.",
      );
  } else if (value.type === "spellSwarm") {
    keys(value, ["type", "count"]);
    if (!validSpellSwarmCount(value.count))
      throw new Error("Choose 0, 10,000, 100,000, or 200,000 spell creatures.");
  } else if (value.type === "game") {
    keys(value, ["type", "action"]);
    if (!["start", "interact", "reset"].includes(value.action as string))
      throw new Error("Choose start, interact, or reset.");
  } else throw new Error("Choose a supported agent action.");
  return structuredClone(value) as unknown as AgentAction;
}
export function validateAgentCaptureRequest(
  value: unknown,
): AgentCaptureRequest {
  keys(value, [
    "expect",
    "camera",
    "width",
    "height",
    "time",
    "exposure",
    "actorReference",
  ]);
  if (value.actorReference !== undefined) {
    const reference = validateCaptureActorReferenceMetadata(
      value.actorReference,
    );
    if (
      reference.sourceIdentity !==
      (value.expect as AgentExpectation)?.sourceIdentity
    )
      throw new Error("Capture actors from the inspected world source.");
  }
  validateAgentExpectation(value.expect);
  validateAgentCaptureCamera(value.camera);
  if (
    !integer(value.width) ||
    !integer(value.height) ||
    value.width < 1 ||
    value.height < 1 ||
    value.width > 1920 ||
    value.height > 1080 ||
    value.width * value.height * 4 > AGENT_LIMITS.captureBytes ||
    typeof value.time !== "number" ||
    !Number.isFinite(value.time) ||
    value.time < 0 ||
    value.time > 86400 ||
    typeof value.exposure !== "number" ||
    !Number.isFinite(value.exposure) ||
    value.exposure < 0.01 ||
    value.exposure > 64
  )
    throw new Error(
      "Choose explicit capture dimensions, time, and exposure within the supported limits.",
    );
  return structuredClone(value) as unknown as AgentCaptureRequest;
}
export function validateAgentCaptureCamera(value: unknown): AgentCaptureCamera {
  keys(value, ["eye", "target", "up", "fov", "vignette"]);
  validateCamera(value);
  if (value.vignette !== undefined && value.vignette !== null) {
    keys(value.vignette, ["shape", "intensity", "blendWeight"]);
    if (
      value.vignette.shape !== "cosine-fourth" ||
      ![value.vignette.intensity, value.vignette.blendWeight].every(
        (n) => typeof n === "number" && Number.isFinite(n) && n >= 0 && n <= 1,
      )
    )
      throw new Error("Choose a supported camera vignette.");
  }
  return structuredClone(value) as unknown as AgentCaptureCamera;
}
