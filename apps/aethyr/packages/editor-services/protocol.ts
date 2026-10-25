import {
  validMmoUiState,
  type MmoUiAction,
  type MmoUiState,
} from "../runtime/mmorpg-protocol.ts";
import type {
  FrameCaptureOptions,
  FramePixelOptions,
  FilmCaptureSize,
} from "../renderer/film-frame.ts";
import { sameCaptureActorReference } from "../runtime/capture-actors.ts";
import type {
  CTownBenchmarkRequest,
  CTownBenchmarkResult,
} from "../../experiments/c-town/metrics.ts";
import type {
  MaterialViewRequest,
  MaterialViewResult,
} from "./creator-material.ts";
import type { AuthorAction, AuthorView } from "./authoring.ts";
import type { ProjectDocument } from "./project-document.ts";
import type {
  AgentAction,
  AgentExpectation,
  AgentInspectOptions,
  AgentCaptureRequest,
  AgentInspection,
  AgentDiscovery,
  AgentCaptureEnvelope,
  AgentReceipt,
  SpellSwarmCount,
} from "../runtime/agent-contract.ts";
import { validSpellSwarmCount } from "../runtime/agent-contract.ts";
import {
  validSceneEffects,
  type SceneEffectsSettings,
} from "../runtime/scene-effects.ts";
/** Versioned local-editor bridge. Game source never receives this port. */
import { validWorldAnimationTime } from "../runtime/world-clock.ts";
import {
  validQuickCombat,
  validCombatRendering,
  type QuickCombat,
  type CombatRendering,
} from "../runtime/quick-combat.ts";
import {
  validDruidStatus,
  type DruidAbility,
  type DruidStatus,
} from "../runtime/druid-abilities.ts";
export const PREVIEW_PROTOCOL = "aethyrforge.preview.v1" as const;
export type Tool =
  "select" | "assets" | "build" | "command" | "settings" | "shader";
export type CameraMode = "walk" | "fly";
export type CharacterPerspective = "first" | "third";
export const CHARACTER_ORBIT_RANGE = [2.5, 40] as const;
export const DEFAULT_CHARACTER_ORBIT_DISTANCE = 6;
export interface CharacterStatus {
  perspective: CharacterPerspective;
  position: [number, number, number];
  velocity: [number, number, number];
  grounded: boolean;
  animation:
    "idle" | "walk" | "run" | "jump" | "fall" | "attack" | "dance" | "death";
  orbitDistance: number;
  modelLoaded?: boolean;
  controller?: "quick-mmorpg-v1";
  animationTime?: number;
}
export function hasCharacterView(
  character?: Pick<CharacterStatus, "modelLoaded">,
): boolean {
  return character?.modelLoaded === true;
}
export function validCharacterPerspective(
  value: unknown,
): value is CharacterPerspective {
  return value === "first" || value === "third";
}
export function validOrbitDistance(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isFinite(value) &&
    value >= CHARACTER_ORBIT_RANGE[0] &&
    value <= CHARACTER_ORBIT_RANGE[1]
  );
}
import {
  DEFAULT_ENVIRONMENT,
  validateEnvironment,
  type EnvironmentSettings,
} from "../runtime/environment.ts";
export {
  DEFAULT_ENVIRONMENT,
  ENVIRONMENT_RANGES,
  validateEnvironment,
  type EnvironmentSettings,
  type NumericEnvironmentKey,
} from "../runtime/environment.ts";
export interface AssetSummary {
  id: string;
  name: string;
  category: string;
}
export interface SelectionSummary {
  id: string;
  name: string;
  kind: string;
  position: [number, number, number];
  scale: number;
}
export type BattleAttackKind = "melee" | "ranged" | "meteor_shower";
export type BattleAction =
  | { op: "start"; count: number; seed?: number }
  | { op: "reset" }
  | { op: "stop" }
  | { op: "camera"; view: BattleView }
  /** Battle Lab fixed-tick capture: restart from the seed, run to `tick`, freeze camera and clock. */
  | { op: "hold"; tick: number; cameraSeconds?: number; time?: number }
  | { op: "release" }
  | { op: "join"; at?: [number, number]; team?: 0 | 1; view?: "first" | "third"; class?: "paladin" | "sorceror" }
  | { op: "leave" }
  | { op: "respawn" }
  | { op: "weapon"; attackKind: BattleAttackKind }
  | { op: "input"; forward: number; turn: number; sprint: boolean; attack: boolean; attackKind?: BattleAttackKind; lookX?: number; lookY?: number };
export type BattleView = "overview" | "orbit" | "player" | "first" | "third" | "dolly";
export interface BattleParticipantStatus {
  active: boolean;
  ready: boolean;
  class: "paladin" | "sorceror";
  team: 0 | 1;
  position: [number, number, number];
  yaw: number;
  pitch: number;
  health: number;
  maxHealth: number;
  alive: boolean;
  attackKind: BattleAttackKind;
  target?: { point: [number, number, number]; normal: [number, number, number]; distance: number };
  attackSequence: number;
  tick: number;
  droppedSeconds: number;
}
/** Ticks a Battle Lab hold may run to; matches `BATTLE_HOLD_LIMITS.maxTick`. */
export const BATTLE_HOLD_MAX_TICK = 36_000;
export interface BattleHoldStatus {
  tick: number;
  /** Fixed steps run since the hold restarted the battle; equals `tick` once reached. */
  steps: number;
  reached: boolean;
  cameraSeconds: number;
  time: number;
}
export interface BattleStatus {
  enabled: boolean;
  hold?: BattleHoldStatus;
  count: number;
  camera: BattleView;
  center?: [number, number, number];
  radius?: number;
  participant?: BattleParticipantStatus;
  stats: Record<string, number>;
  message?: string;
}
export function validBattleAction(value: unknown): value is BattleAction {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const action = value as Record<string, unknown>;
  const fields = (allowed: string[]) =>
    Object.keys(action).every((key) => allowed.includes(key));
  const finite = (value: unknown, max: number) => typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= max;
  const weapon = (value: unknown) => value === "melee" || value === "ranged" || value === "meteor_shower";
  if (action.op === "start")
    return (
      fields(["op", "count", "seed"]) &&
      Number.isSafeInteger(action.count) &&
      (action.count as number) >= 100 &&
      (action.count as number) <= 1_000_000 &&
      (action.seed === undefined ||
        (Number.isSafeInteger(action.seed) &&
          (action.seed as number) >= 0 &&
          (action.seed as number) <= 0xffffffff))
    );
  if (action.op === "camera")
    return (
      fields(["op", "view"]) &&
      ["overview", "orbit", "player", "first", "third", "dolly"].includes(String(action.view))
    );
  if (action.op === "hold")
    return (
      fields(["op", "tick", "cameraSeconds", "time"]) &&
      Number.isSafeInteger(action.tick) &&
      (action.tick as number) >= 0 &&
      (action.tick as number) <= BATTLE_HOLD_MAX_TICK &&
      (action.cameraSeconds === undefined || (finite(action.cameraSeconds, 86400) && (action.cameraSeconds as number) >= 0)) &&
      (action.time === undefined || (finite(action.time, 86400) && (action.time as number) >= 0))
    );
  if (action.op === "join") return fields(["op", "at", "team", "view", "class"]) &&
    (action.at === undefined || (Array.isArray(action.at) && action.at.length === 2 && action.at.every((v) => finite(v, 20000)))) &&
    (action.team === undefined || action.team === 0 || action.team === 1) &&
    (action.view === undefined || action.view === "first" || action.view === "third") &&
    (action.class === undefined || action.class === "paladin" || action.class === "sorceror");
  if (action.op === "weapon") return fields(["op", "attackKind"]) && weapon(action.attackKind);
  if (action.op === "input") return fields(["op", "forward", "turn", "sprint", "attack", "attackKind", "lookX", "lookY"]) &&
    finite(action.forward, 1) && finite(action.turn, 1) && typeof action.sprint === "boolean" && typeof action.attack === "boolean" &&
    (action.attackKind === undefined || weapon(action.attackKind)) &&
    (action.lookX === undefined || finite(action.lookX, .5)) && (action.lookY === undefined || finite(action.lookY, .5));
  return ["stop", "reset", "leave", "respawn", "release"].includes(String(action.op)) && fields(["op"]);
}
export function validBattleRequest(
  value: unknown,
): value is Extract<PreviewRequest, { type: "battle" }> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const request = value as Record<string, unknown>;
  return (
    request.type === "battle" &&
    validBattleAction(request.action) &&
    Object.keys(request).every((key) =>
      ["type", "action", "protocol", "sessionId", "requestId"].includes(key),
    ) &&
    (request.protocol === undefined || request.protocol === PREVIEW_PROTOCOL) &&
    [request.sessionId, request.requestId].every(
      (id) =>
        id === undefined ||
        (typeof id === "string" && id.length > 0 && id.length <= 128),
    )
  );
}
export function validBattleStatus(value: unknown): value is BattleStatus {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const battle = value as Record<string, unknown>;
  return (
    Object.keys(battle).every((key) =>
      ["enabled", "count", "camera", "stats", "message", "center", "radius", "participant", "hold"].includes(key),
    ) &&
    typeof battle.enabled === "boolean" &&
    Number.isSafeInteger(battle.count) &&
    (battle.count as number) >= (battle.enabled ? 100 : 0) &&
    (battle.count as number) <= 1_000_000 &&
    ["overview", "orbit", "player", "first", "third", "dolly"].includes(String(battle.camera)) &&
    (battle.center === undefined || (Array.isArray(battle.center) && battle.center.length === 3 && battle.center.every((v) => typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= 20000))) &&
    (battle.radius === undefined || (typeof battle.radius === "number" && Number.isFinite(battle.radius) && battle.radius >= 0 && battle.radius <= 1000)) &&
    (battle.participant === undefined || validBattleParticipantStatus(battle.participant)) &&
    (battle.hold === undefined || validBattleHoldStatus(battle.hold)) &&
    validNumericSummary(battle.stats) &&
    Object.values(battle.stats as Record<string, number>).every(
      (n) => n <= Number.MAX_SAFE_INTEGER,
    ) &&
    (battle.message === undefined ||
      (typeof battle.message === "string" && battle.message.length <= 256))
  );
}
function validBattleHoldStatus(value: unknown): value is BattleHoldStatus {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const h = value as BattleHoldStatus;
  const tick = (v: unknown) => Number.isSafeInteger(v) && (v as number) >= 0 && (v as number) <= BATTLE_HOLD_MAX_TICK;
  const seconds = (v: unknown) => typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= 86400;
  return Object.keys(h).every((k) => ["tick", "steps", "reached", "cameraSeconds", "time"].includes(k)) &&
    tick(h.tick) && tick(h.steps) && h.steps <= h.tick && typeof h.reached === "boolean" &&
    h.reached === (h.steps === h.tick) && seconds(h.cameraSeconds) && seconds(h.time);
}
function validBattleParticipantStatus(value: unknown): value is BattleParticipantStatus {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const p = value as BattleParticipantStatus;
  return Object.keys(p).every((k) => ["active", "ready", "class", "team", "position", "yaw", "pitch", "health", "maxHealth", "alive", "attackKind", "attackSequence", "tick", "droppedSeconds", "target"].includes(k)) &&
    typeof p.active === "boolean" && typeof p.ready === "boolean" && ["paladin", "sorceror"].includes(p.class) && (p.team === 0 || p.team === 1) &&
    Array.isArray(p.position) && p.position.length === 3 && p.position.every((v) => Number.isFinite(v) && Math.abs(v) <= 20000) &&
    [p.yaw, p.pitch].every((v) => typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= 100000) &&
    [p.health, p.maxHealth, p.droppedSeconds].every((v) => typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= Number.MAX_SAFE_INTEGER) &&
    p.maxHealth > 0 && p.health <= p.maxHealth && typeof p.alive === "boolean" && p.alive === (p.health > 0) &&
    ["melee", "ranged", "meteor_shower"].includes(p.attackKind) &&
    (p.target === undefined || validBattleTarget(p.target)) &&
    [p.attackSequence, p.tick].every((v) => Number.isSafeInteger(v) && v >= 0 && v <= Number.MAX_SAFE_INTEGER);
}
function validBattleTarget(value: unknown): value is NonNullable<BattleParticipantStatus["target"]> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const target = value as NonNullable<BattleParticipantStatus["target"]>;
  const vector = (v: unknown, bound: number) => Array.isArray(v) && v.length === 3 &&
    v.every((n) => typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= bound);
  return Object.keys(target).every((key) => ["point", "normal", "distance"].includes(key)) &&
    vector(target.point, 20000) && vector(target.normal, 1) &&
    Math.abs(Math.hypot(...target.normal) - 1) <= .02 &&
    typeof target.distance === "number" && Number.isFinite(target.distance) && target.distance >= 0 && target.distance <= 80;
}
export type EditorCommand =
  | {
      kind: "prop.add";
      assetId: string;
      id: string;
      /** Mesh pivot in world meters, matching entity.transform. */
      position: [number, number, number];
      scale?: number;
    }
  | {
      kind: "asset.add";
      assetId: string;
      count: number;
      placement: "ahead" | "left" | "right";
    }
  | { kind: "entity.select"; id: string | null }
  | { kind: "entity.remove"; id: string }
  | {
      kind: "entity.transform";
      id: string;
      position?: [number, number, number];
      scale?: number;
    }
  | { kind: "room.add"; side: "ahead" | "left" | "right"; size: number }
  | { kind: "wall.remove" }
  | { kind: "history.undo" }
  | { kind: "history.redo" }
  | { kind: "text"; text: string };
export interface InputSample {
  forward: number;
  right: number;
  up: number;
  lookX: number;
  lookY: number;
  sprint: boolean;
  /** Held state; the simulation accepts one jump on each press. */
  jump?: boolean;
  attack?: boolean;
  dance?: boolean;
}
export type PreviewRequest =
  | { type: "agentDiscover" }
  | { type: "agentInspect"; options?: AgentInspectOptions }
  | { type: "agentAction"; action: AgentAction; expect: AgentExpectation }
  | { type: "agentCapture"; options: AgentCaptureRequest }
  | { type: "agentCancel" }
  | {
      type: "cpuCullProbe";
      workers: 0 | 1 | 2;
      iterations: number;
      timeoutMs?: number;
    }
  | { type: "cancelCpuCullProbe" }
  | {
      type: "stream";
      action: "load" | "cancel" | "unload";
      cell?: "entry" | "town";
    }
  | { type: "game"; action: "start" | "interact" | "reset" }
  | { type: "spellSwarm"; count: SpellSwarmCount }
  | { type: "setSceneEffects"; settings: SceneEffectsSettings }
  | { type: "previewImpact" }
  | { type: "cTownBenchmark"; options: CTownBenchmarkRequest }
  | { type: "cTownReload"; candidate: "default" | "changed" | "invalid" }
  | { type: "materialView"; view: MaterialViewRequest; base: number }
  | { type: "exportProject" }
  | { type: "initialize"; animationTime?: number; player?: boolean }
  | {
      type: "loadPlayable";
      id: string;
      action: AuthorAction;
      base: number;
      ticket?: number;
    }
  | { type: "command"; command: EditorCommand }
  | { type: "author"; action: AuthorAction; base: number; commandId: string }
  | { type: "input"; input: InputSample }
  | {
      type: "setEnvironment";
      settings: EnvironmentSettings;
      restoreSourceFill?: boolean;
    }
  | { type: "setShader"; source: string }
  | { type: "setCamera"; shotId: string }
  | {
      type: "setCameraPose";
      eye: [number, number, number];
      target: [number, number, number];
      up?: [number, number, number];
      fov?: number;
      mode?: CameraMode;
    }
  | { type: "mode"; mode: CameraMode }
  | { type: "perspective"; perspective: CharacterPerspective }
  | { type: "orbitDistance"; distance: number }
  | { type: "selectAbility"; ability: DruidAbility | null }
  | { type: "castAbility" }
  | { type: "cancelAbility" }
  | { type: "resetGameplay" }
  | { type: "restartCombat" }
  | { type: "mmorpg"; action: MmoUiAction }
  | { type: "mmorpgEditing"; editing: boolean }
  | { type: "battle"; action: BattleAction }
  | ({ type: "capture" } & FrameCaptureOptions)
  | ({ type: "capturePixels" } & FramePixelOptions)
  | ({ type: "beginFilmCapture" } & FilmCaptureSize)
  | { type: "endFilmCapture" }
  | { type: "mesh"; enabled: boolean }
  | { type: "cancelAuthoring" }
  | { type: "status" };
export type PreviewMessage = PreviewRequest & {
  protocol: typeof PREVIEW_PROTOCOL;
  sessionId: string;
  requestId: string;
};
/** Editing is a local creator control; a playable page cannot enable it. */
export function canEditMmorpg(
  publicDemo: boolean,
  pathname: string,
  readOnly = false,
): boolean {
  return !publicDemo && !readOnly && !pathname.endsWith("/player.html");
}
export function validMmorpgEditingRequest(
  value: unknown,
): value is Extract<PreviewRequest, { type: "mmorpgEditing" }> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const request = value as Record<string, unknown>;
  return (
    request.type === "mmorpgEditing" &&
    typeof request.editing === "boolean" &&
    Object.keys(request).every((key) =>
      ["type", "editing", "protocol", "sessionId", "requestId"].includes(key),
    ) &&
    (request.protocol === undefined || request.protocol === PREVIEW_PROTOCOL) &&
    [request.sessionId, request.requestId].every(
      (id) =>
        id === undefined ||
        (typeof id === "string" && id.length > 0 && id.length <= 128),
    )
  );
}
export interface EngineStatus {
  battle?: BattleStatus;
  mmorpg?: MmoUiState;
  mmorpgEditing?: boolean;
  combat?: QuickCombat;
  combatRendering?: CombatRendering;
  sceneEffects?: SceneEffectsSettings;
  effectPreview?: Record<string, number>;
  spellSwarm?: { count: SpellSwarmCount; enabled: boolean };
  /** Geometry and animation counters from the last procedural grass draw. */
  groundGrass?: Record<string, number>;
  /** Last accepted authored edit; byte counters cover typed buffers only. */
  runtimeTables?: Record<string, number>;
  kernelWork?: Record<string, number>;
  runtimeEditMetrics?: Record<string, number>;
  streaming?: {
    activeCell: string | null;
    loading: boolean;
    quality: string;
    firstControlMs: number;
    stats: Record<string, number>;
  };
  runtimeMetrics?: {
    firstControlMs: number;
    preparationMs: number;
    gpuAccountedBytes: number;
    frameAttempts?: number;
    submittedFrames?: number;
    skippedFrames?: number;
    kernelDroppedSeconds?: number;
    hostClampedSeconds?: number;
    mmorpgActors?: number;
    mmorpgClasses?: number;
    mmorpgTriangles?: number;
    mmorpgParticles?: number;
    mmorpgParticleDraws?: number;
    mmorpgTerrainUpdates?: number;
    mmorpgNameLabels?: number;
    mmorpgNameTriangles?: number;
    mmorpgNameDrawCalls?: number;
    mmorpgNameTextureBytes?: number;
    mmorpgNameTextureLayers?: number;
    mmorpgNameUploads?: number;
  };
  game?: {
    phase: "find-key" | "find-exit" | "complete";
    message: string;
    keyId: string;
    exitId: string;
    keyPosition: [number, number, number];
    exitPosition: [number, number, number];
    hasKey: boolean;
  };
  /** Bounded applied content for private inspection, separate from base-scene statistics. */
  personalWorld?: {
    recipeSha256: string;
    entities: ProjectDocument["entities"];
  };
  /** Last submitted personal-world lighting; private diagnostics, never public host status. */
  renderedEnvironment?: {
    frame: number;
    atUnixMs: number;
    settings: EnvironmentSettings;
  };
  mesh?: boolean;
  authoring?: AuthorView;
  phase: "loading" | "ready" | "error";
  message: string;
  fps: number;
  frame: number;
  revision: number;
  triangles: number;
  instances: number;
  camera: [number, number, number];
  character?: CharacterStatus;
  gameplay?: DruidStatus;
  view?: {
    eye: [number, number, number];
    target: [number, number, number];
    mode: CameraMode;
    animationTime?: number;
  };
  selection: SelectionSummary | null;
  canUndo: boolean;
  canRedo: boolean;
  saved: boolean;
}
export const INITIAL_STATUS: EngineStatus = {
  phase: "loading",
  message: "Opening Medieval Town…",
  fps: 0,
  frame: 0,
  revision: 0,
  triangles: 0,
  instances: 0,
  camera: [0, 0, 0],
  selection: null,
  canUndo: false,
  canRedo: false,
  saved: false,
};
export type PreviewEvent = (
  | { type: "mmorpgFocusChat" }
  | { type: "agentDiscovery"; result: AgentDiscovery }
  | { type: "agentInspection"; result: AgentInspection }
  | { type: "agentCapture"; result: AgentCaptureEnvelope }
  | { type: "cpuCullProbeResult"; result: Record<string, unknown> }
  | { type: "cTownBenchmarkResult"; result: CTownBenchmarkResult }
  | { type: "cTownReloadResult"; result: Record<string, unknown> }
  | {
      type: "ready";
      assets: AssetSummary[];
      status: EngineStatus;
      shaderSource?: string;
      hasSourceSky?: boolean;
      sourceSkyLightIntensity?: number;
    }
  | { type: "status"; status: EngineStatus }
  | {
      type: "environment";
      settings: EnvironmentSettings;
      sourceSkyLightIntensity?: number;
    }
  | {
      type: "receipt";
      ok: boolean;
      message: string;
      revision: number;
      agent?: AgentReceipt;
    }
  | { type: "capture"; blob: Blob }
  | { type: "capturePixels"; pixels: ArrayBuffer }
  | { type: "materialView"; result: MaterialViewResult }
  | { type: "projectDocument"; document: ProjectDocument }
  | { type: "error"; message: string }
) & {
  protocol: typeof PREVIEW_PROTOCOL;
  sessionId: string;
  requestId?: string;
};

/** Each pixel response changes ownership on both worker→preview and preview→editor hops. */
export function previewEventTransfers(
  value: Record<string, unknown>,
): Transferable[] {
  if (
    value.type === "agentCapture" &&
    value.result &&
    typeof value.result === "object" &&
    "pixels" in value.result &&
    value.result.pixels instanceof ArrayBuffer
  )
    return [value.result.pixels];
  return value.type === "capturePixels" && value.pixels instanceof ArrayBuffer
    ? [value.pixels]
    : [];
}

export function isPreviewEvent(
  value: unknown,
  sessionId: string,
): value is PreviewEvent {
  if (!value || typeof value !== "object") return false;
  const event = value as Record<string, unknown>;
  if (event.protocol !== PREVIEW_PROTOCOL || event.sessionId !== sessionId)
    return false;
  if (
    ["agentDiscovery", "agentInspection", "agentCapture"].includes(
      String(event.type),
    )
  ) {
    if (
      !boundedString(event.requestId, 128) ||
      !event.result ||
      typeof event.result !== "object"
    )
      return false;
    const result = event.result as Record<string, unknown>;
    if (event.type === "agentCapture") {
      return (
        result.schema === "aethyrforge.agent.capture.v1" &&
        (result.actors === undefined ||
          sameCaptureActorReference(result.actors, result.actors)) &&
        result.pixels instanceof ArrayBuffer &&
        result.pixels.byteLength > 0 &&
        result.pixels.byteLength <= 32 * 1024 * 1024 &&
        result.pixels.byteLength ===
          Number(result.width) * Number(result.height) * 4
      );
    }
    try {
      return (
        result.schema === "aethyrforge.agent.v1" &&
        new TextEncoder().encode(JSON.stringify(result)).length <= 512 * 1024
      );
    } catch {
      return false;
    }
  }
  if (
    event.type === "cTownBenchmarkResult" ||
    event.type === "cTownReloadResult" ||
    event.type === "cpuCullProbeResult"
  ) {
    if (
      !boundedString(event.requestId, 128) ||
      !event.result ||
      typeof event.result !== "object"
    )
      return false;
    try {
      if (
        new TextEncoder().encode(JSON.stringify(event.result)).byteLength >
        (event.type === "cTownBenchmarkResult" ? 8 : 4) * 1024 * 1024
      )
        return false;
    } catch {
      return false;
    }
    if (event.type === "cTownReloadResult") return true;
    if (event.type === "cpuCullProbeResult") {
      const result = event.result as Record<string, unknown>;
      return (
        result.schema === "aethyrforge.cpu-cull-probe.v1" &&
        [0, 1, 2].includes(Number(result.workers)) &&
        result.exact === true &&
        Array.isArray(result.measurements) &&
        result.measurements.length <= 100
      );
    }
    const result = event.result as CTownBenchmarkResult;
    return (
      ["baseline", "rust-packed", "ts-packed", "c", "c-frame"].includes(
        result.variant,
      ) &&
      typeof result.recording === "boolean" &&
      (result.callbackClock === undefined ||
        result.callbackClock === "entry") &&
      (result.instrumented === undefined ||
        typeof result.instrumented === "boolean") &&
      typeof result.truncated === "boolean" &&
      Number.isSafeInteger(result.capacity) &&
      result.capacity >= 1 &&
      result.capacity <= 20_000 &&
      ["static", "sway"].includes(result.route) &&
      Array.isArray(result.samples) &&
      result.samples.length <= result.capacity &&
      (![
        "observation",
        "completionRange",
        "completedFrames",
        "completionTruncated",
      ].some((key) => Object.hasOwn(result, key)) ||
        (!!result.observation &&
          [
            result.observation.startedAtMs,
            result.observation.stoppedAtMs,
          ].every(
            (value) =>
              value === null ||
              (typeof value === "number" &&
                Number.isFinite(value) &&
                value >= 0),
          ) &&
          (result.observation.stoppedAtMs === null ||
            (result.observation.startedAtMs !== null &&
              result.observation.stoppedAtMs >=
                result.observation.startedAtMs)) &&
          typeof result.completionTruncated === "boolean" &&
          !!result.completionRange &&
          [
            result.completionRange.firstFrame,
            result.completionRange.lastFrame,
          ].every(
            (value) =>
              value === null || (Number.isSafeInteger(value) && value! >= 0),
          ) &&
          (result.completionRange.firstFrame === null) ===
            (result.completionRange.lastFrame === null) &&
          (result.completionRange.firstFrame === null ||
            result.completionRange.lastFrame! >=
              result.completionRange.firstFrame) &&
          Array.isArray(result.completedFrames) &&
          result.completedFrames.length <= result.capacity &&
          result.completedFrames.every(
            (entry) =>
              entry &&
              Number.isSafeInteger(entry.frame) &&
              entry.frame >= 0 &&
              [entry.submittedAt, entry.completedAt, entry.latencyMs].every(
                (value) => Number.isFinite(value) && value >= 0,
              ) &&
              entry.completedAt >= entry.submittedAt &&
              (entry.submitReturnedAt === undefined ||
                (Number.isFinite(entry.submitReturnedAt) &&
                  entry.submitReturnedAt >= entry.submittedAt &&
                  entry.submitReturnedAt <= entry.completedAt)) &&
              typeof entry.ok === "boolean" &&
              result.completionRange.firstFrame !== null &&
              entry.frame >= result.completionRange.firstFrame &&
              entry.frame <= result.completionRange.lastFrame!,
          ))) &&
      result.samples.every(
        (s) =>
          s &&
          typeof s.submitted === "boolean" &&
          [s.frame, s.at, s.elapsedMs].every(
            (v) => Number.isFinite(v) && v >= 0,
          ) &&
          (s.callbackEnteredAt === undefined ||
            (typeof s.callbackEnteredAt === "number" &&
              Number.isFinite(s.callbackEnteredAt) &&
              s.callbackEnteredAt >= 0)) &&
          [s.stepMs, s.renderCpuMs, s.cullMs, s.clusterMs, s.gpuMs].every(
            (v) =>
              v === null ||
              (typeof v === "number" && Number.isFinite(v) && v >= 0),
          ) &&
          [
            s.bridgeEncodeMs,
            s.wasmRequestMs,
            s.bridgeDecodeMs,
            s.bridgeOverheadMs,
            s.commandEncodeMs,
            s.fullGpuMs,
          ].every(
            (v) =>
              v === undefined ||
              v === null ||
              (typeof v === "number" && Number.isFinite(v) && v >= 0),
          ) &&
          (s.cameraEye === undefined ||
            (Array.isArray(s.cameraEye) &&
              s.cameraEye.length === 3 &&
              s.cameraEye.every(
                (v) =>
                  typeof v === "number" &&
                  Number.isFinite(v) &&
                  Math.abs(v) <= 1_000_000,
              ))) &&
          [s.cameraYaw, s.cameraPitch].every(
            (v) =>
              v === undefined || (typeof v === "number" && Number.isFinite(v)),
          ) &&
          (s.animationTime === undefined ||
            (typeof s.animationTime === "number" &&
              Number.isFinite(s.animationTime) &&
              s.animationTime >= 0)) &&
          [
            s.kernelDroppedSeconds,
            s.hostClampedSeconds,
            s.submittedInstances,
            s.submittedTriangles,
          ].every(
            (v) =>
              v === undefined ||
              (typeof v === "number" && Number.isFinite(v) && v >= 0),
          ),
      )
    );
  }
  if (event.type === "ready")
    return (
      (event.shaderSource === undefined ||
        (typeof event.shaderSource === "string" &&
          new TextEncoder().encode(event.shaderSource).byteLength <= 131072)) &&
      (event.hasSourceSky === undefined ||
        typeof event.hasSourceSky === "boolean") &&
      validSourceIntensity(event.sourceSkyLightIntensity) &&
      Array.isArray(event.assets) &&
      event.assets.length <= 1024 &&
      event.assets.every(
        (asset) =>
          !!asset &&
          typeof asset === "object" &&
          boundedString(asset.id, 128) &&
          boundedString(asset.name, 256) &&
          boundedString(asset.category, 128),
      ) &&
      validStatus(event.status)
    );
  if (event.type === "mmorpgFocusChat")
    return Object.keys(event).every((key) =>
      ["type", "protocol", "sessionId"].includes(key),
    );
  if (event.type === "status") return validStatus(event.status);
  if (event.type === "materialView")
    return (
      boundedString(event.requestId, 128) &&
      !!event.result &&
      typeof event.result === "object" &&
      Number.isSafeInteger((event.result as any).frame) &&
      new TextEncoder().encode(JSON.stringify(event.result)).length <=
        512 * 1024
    );
  if (event.type === "projectDocument")
    return (
      boundedString(event.requestId, 128) &&
      !!event.document &&
      typeof event.document === "object" &&
      new TextEncoder().encode(JSON.stringify(event.document)).length <=
        48 * 1024
    );
  if (event.type === "environment")
    return (
      validEnvironment(event.settings) &&
      validSourceIntensity(event.sourceSkyLightIntensity)
    );
  if (event.type === "receipt")
    return (
      boundedString(event.requestId, 128) &&
      typeof event.ok === "boolean" &&
      typeof event.message === "string" &&
      event.message.length <= 2048 &&
      Number.isSafeInteger(event.revision)
    );
  if (event.type === "error")
    return typeof event.message === "string" && event.message.length <= 2048;
  if (event.type === "capture")
    return (
      typeof Blob !== "undefined" &&
      event.blob instanceof Blob &&
      event.blob.size <= 32 * 1024 * 1024
    );
  if (event.type === "capturePixels")
    return (
      boundedString(event.requestId, 128) &&
      event.pixels instanceof ArrayBuffer &&
      event.pixels.byteLength > 0 &&
      event.pixels.byteLength % 4 === 0 &&
      event.pixels.byteLength <= 32 * 1024 * 1024
    );
  return false;
}
function validEnvironment(value: unknown): value is EnvironmentSettings {
  if (!value || typeof value !== "object") return false;
  try {
    validateEnvironment(value as EnvironmentSettings);
    return true;
  } catch {
    return false;
  }
}
function validSourceIntensity(value: unknown): boolean {
  return (
    value === undefined ||
    (typeof value === "number" &&
      Number.isFinite(value) &&
      value >= 0 &&
      value <= 8)
  );
}
function validStatus(value: unknown): value is EngineStatus {
  if (!value || typeof value !== "object") return false;
  const s = value as Record<string, unknown>;
  return (
    ["loading", "ready", "error"].includes(String(s.phase)) &&
    typeof s.message === "string" &&
    s.message.length <= 2048 &&
    [s.fps, s.frame, s.revision, s.triangles, s.instances].every(
      (n) => typeof n === "number" && Number.isFinite(n) && n >= 0,
    ) &&
    Array.isArray(s.camera) &&
    s.camera.length === 3 &&
    s.camera.every((n) => typeof n === "number" && Number.isFinite(n)) &&
    (s.view === undefined || validView(s.view)) &&
    (s.character === undefined || validCharacterStatus(s.character)) &&
    (s.combat === undefined || validQuickCombat(s.combat)) &&
    (s.mmorpg === undefined || validMmoUiState(s.mmorpg)) &&
    (s.mmorpgEditing === undefined || typeof s.mmorpgEditing === "boolean") &&
    (s.battle === undefined || validBattleStatus(s.battle)) &&
    (s.combatRendering === undefined ||
      validCombatRendering(s.combatRendering)) &&
    (s.gameplay === undefined || validDruidStatus(s.gameplay)) &&
    (s.spellSwarm === undefined || validSpellSwarmStatus(s.spellSwarm)) &&
    (s.sceneEffects === undefined || validSceneEffects(s.sceneEffects)) &&
    (s.effectPreview === undefined || validNumericSummary(s.effectPreview)) &&
    (s.game === undefined || validGameStatus(s.game)) &&
    (s.streaming === undefined || validStreamingStatus(s.streaming)) &&
    (s.runtimeMetrics === undefined || validNumericSummary(s.runtimeMetrics)) &&
    (s.runtimeTables === undefined || validNumericSummary(s.runtimeTables)) &&
    (s.groundGrass === undefined || validNumericSummary(s.groundGrass)) &&
    (s.kernelWork === undefined || validNumericSummary(s.kernelWork)) &&
    (s.runtimeEditMetrics === undefined ||
      validNumericSummary(s.runtimeEditMetrics)) &&
    typeof s.canUndo === "boolean" &&
    typeof s.canRedo === "boolean" &&
    typeof s.saved === "boolean" &&
    (s.mesh === undefined || typeof s.mesh === "boolean") &&
    (s.selection === null || validSelection(s.selection))
  );
}
function validSpellSwarmStatus(value: unknown): boolean {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const swarm = value as Record<string, unknown>;
  return (
    Object.keys(swarm).every((key) => key === "count" || key === "enabled") &&
    validSpellSwarmCount(swarm.count) &&
    swarm.enabled === swarm.count > 0
  );
}
function validNumericSummary(value: unknown): boolean {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const entries = Object.entries(value);
  return (
    entries.length <= 64 &&
    entries.every(
      ([key, n]) =>
        key.length <= 64 &&
        typeof n === "number" &&
        Number.isFinite(n) &&
        n >= 0,
    )
  );
}
function validStreamingStatus(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const stream = value as NonNullable<EngineStatus["streaming"]>;
  return (
    (stream.activeCell === null || boundedString(stream.activeCell, 64)) &&
    typeof stream.loading === "boolean" &&
    boundedString(stream.quality, 128) &&
    Number.isFinite(stream.firstControlMs) &&
    stream.firstControlMs >= 0 &&
    validNumericSummary(stream.stats)
  );
}
function validGameStatus(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const game = value as NonNullable<EngineStatus["game"]>;
  return (
    ["find-key", "find-exit", "complete"].includes(game.phase) &&
    typeof game.message === "string" &&
    game.message.length <= 512 &&
    boundedString(game.keyId, 64) &&
    boundedString(game.exitId, 64) &&
    typeof game.hasKey === "boolean" &&
    [game.keyPosition, game.exitPosition].every(
      (p) =>
        Array.isArray(p) &&
        p.length === 3 &&
        p.every((n) => Number.isFinite(n) && Math.abs(n) <= 20000),
    )
  );
}
function validCharacterStatus(value: unknown): value is CharacterStatus {
  if (!value || typeof value !== "object") return false;
  const c = value as Record<string, unknown>;
  return (
    validCharacterPerspective(c.perspective) &&
    [c.position, c.velocity].every(
      (point) =>
        Array.isArray(point) &&
        point.length === 3 &&
        point.every(
          (n) =>
            typeof n === "number" &&
            Number.isFinite(n) &&
            Math.abs(n) <= 1_000_000,
        ),
    ) &&
    typeof c.grounded === "boolean" &&
    (c.modelLoaded === undefined || typeof c.modelLoaded === "boolean") &&
    (c.controller === undefined || c.controller === "quick-mmorpg-v1") &&
    (c.animationTime === undefined ||
      (c.controller === "quick-mmorpg-v1"
        ? validWorldAnimationTime(c.animationTime)
        : typeof c.animationTime === "number" &&
          Number.isFinite(c.animationTime) &&
          c.animationTime >= 0 &&
          c.animationTime <= 120)) &&
    [
      "idle",
      "walk",
      "run",
      "jump",
      "fall",
      "attack",
      "dance",
      "death",
    ].includes(String(c.animation)) &&
    validOrbitDistance(c.orbitDistance)
  );
}
function validView(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return (
    ["walk", "fly"].includes(String(v.mode)) &&
    (v.animationTime === undefined ||
      validWorldAnimationTime(v.animationTime)) &&
    [v.eye, v.target].every(
      (point) =>
        Array.isArray(point) &&
        point.length === 3 &&
        point.every(
          (n) =>
            typeof n === "number" &&
            Number.isFinite(n) &&
            Math.abs(n) <= 1000000,
        ),
    )
  );
}
function boundedString(value: unknown, max: number): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= max;
}
function validSelection(value: unknown): value is SelectionSummary {
  if (!value || typeof value !== "object") return false;
  const s = value as Record<string, unknown>;
  return (
    boundedString(s.id, 128) &&
    boundedString(s.name, 256) &&
    boundedString(s.kind, 128) &&
    Array.isArray(s.position) &&
    s.position.length === 3 &&
    s.position.every((n) => typeof n === "number" && Number.isFinite(n)) &&
    typeof s.scale === "number" &&
    Number.isFinite(s.scale) &&
    s.scale > 0 &&
    s.scale <= 100
  );
}
