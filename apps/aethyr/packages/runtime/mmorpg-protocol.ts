/** The owned MMO wire contract and the editor's bounded presentation state. */
export const MMORPG_PROTOCOL = "aethyrforge.mmorpg.v1" as const;
export const MMO_UI_EVENT_LIMIT = 256;
export type MmoPlayerClass = "paladin" | "sorceror";
export type MmoClass = MmoPlayerClass | "zombie" | "warrok";
export type MmoMotion = "idle" | "walk" | "run" | "attack" | "dance" | "death";
export type MmoItem = "weapon.sword1" | "weapon.axe1" | "weapon.hammer1";
export type MmoPoint = [number, number, number];
export interface MmoStats {
  health: number;
  maxHealth: number;
  strength: number;
  wisdomness: number;
  benchpress: number;
  curl: number;
  experience: number;
  level: number;
}
export interface MmoPrediction {
  position: MmoPoint;
  yaw: number;
  verticalSpeed: number;
  grounded: boolean;
  velocity: number;
  state: MmoMotion;
  stateTicks: number;
  cooldownTicks: number;
}
export interface MmoActor {
  id: number;
  name: string;
  class: MmoClass;
  kind: "player" | "npc";
  position: MmoPoint;
  yaw: number;
  health: number;
  maxHealth: number;
  stats: MmoStats;
  state: MmoMotion;
  actionTime: number;
  animationTime: number;
  alive: boolean;
  weapon: MmoItem | null;
  attacks: number;
  hits: number;
  prediction?: MmoPrediction;
  inventory?: (MmoItem | null)[];
  inventoryVersion?: number;
}
export interface MmoEvent {
  id: number;
  tick: number;
  kind:
    | "attack"
    | "damage"
    | "death"
    | "chat"
    | "join"
    | "leave"
    | "respawn"
    | "inventory";
  actor?: number;
  target?: number;
  amount?: number;
  text?: string;
  name?: string;
}
export interface MmoPopulation {
  players: number;
  npcs: number;
  spawners: number;
}
export interface MmoSnapshot {
  type: "snapshot";
  tick: number;
  player: MmoActor;
  actors: MmoActor[];
  despawned: number[];
  events: MmoEvent[];
  ackInput: number;
  lastEventId: number;
  eventsTruncated: boolean;
  population: MmoPopulation;
}
export interface MmoWorld {
  seed: number;
  worldScale: number;
  sourceCommit: string;
  tickRate: 60;
  snapshotRate: 10;
  release: string;
  spawn: MmoPoint;
  yaw: number;
}
export type MmoServerMessage =
  | MmoSnapshot
  | {
      type: "welcome";
      protocol: typeof MMORPG_PROTOCOL;
      playerId: number;
      world: MmoWorld;
      snapshot: MmoSnapshot;
    }
  | {
      type: "receipt";
      requestId: string;
      ok: boolean;
      duplicate: boolean;
      message?: string;
      inventoryVersion?: number;
    }
  | { type: "error"; message: string };
export type MmoUiAction =
  | { type: "join"; name: string; class?: MmoPlayerClass }
  | { type: "swap"; version: number; from: number; to: number }
  | { type: "chat"; text: string }
  | { type: "respawn" };
export type MmoClientMessage =
  | Extract<MmoUiAction, { type: "join" }>
  | {
      type: "input";
      sequence: number;
      forward: number;
      turn: number;
      sprint: boolean;
      attack: boolean;
      dance: boolean;
    }
  | (Exclude<MmoUiAction, { type: "join" }> & { requestId: string });
export const MMO_ICON_KEYS = [
  "loginBackground",
  "healthFrame",
  "inventoryPortrait",
  "stats",
  "inventory",
  "quests",
  "sword",
  "axe",
  "hammer",
] as const;
export type MmoIconKey = (typeof MMO_ICON_KEYS)[number];
export interface MmoIcon {
  url: string;
  sha256: string;
  byteLength: number;
  width: number;
  height: number;
}
export interface MmoIconsManifest {
  schema: "aethyrforge.mmorpg-ui.v1";
  icons: Record<MmoIconKey, MmoIcon>;
}
export interface MmoUiState {
  connection: "login" | "connecting" | "online" | "disconnected" | "error";
  player?: MmoActor;
  actors: MmoActor[];
  events: MmoEvent[];
  population?: MmoPopulation;
  icons?: MmoIconsManifest;
  message?: string;
}
const record = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === "object" && !Array.isArray(v);
const finite = (v: unknown, max = 1e9): v is number =>
  typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= max;
const count = (v: unknown, max = Number.MAX_SAFE_INTEGER): v is number =>
  finite(v, max) && v >= 0 && Number.isSafeInteger(v);
const id = (v: unknown): v is number => count(v, 0xffff_ffff);
const text = (v: unknown, max: number, min = 0): v is string =>
  typeof v === "string" &&
  [...v].length >= min &&
  [...v].length <= max &&
  !/[\u0000-\u001f\u007f-\u009f]/u.test(v);
const point = (v: unknown): v is MmoPoint =>
  Array.isArray(v) && v.length === 3 && v.every((n) => finite(n, 20_000));
const motion = (v: unknown): v is MmoMotion =>
  ["idle", "walk", "run", "attack", "dance", "death"].includes(String(v));
const item = (v: unknown): v is MmoItem | null =>
  v === null ||
  ["weapon.sword1", "weapon.axe1", "weapon.hammer1"].includes(String(v));
const optional = (v: unknown, test: (x: unknown) => boolean) =>
  v === undefined || test(v);
const only = (v: Record<string, unknown>, keys: string[]) =>
  Object.keys(v).every((key) => keys.includes(key));
const MAX_MMO_STATE_TICKS = 216_000;
// Rust's f32 tick duration makes the capped state clock slightly greater than 3600.
export const MAX_MMO_STATE_TIME = Math.fround(
  MAX_MMO_STATE_TICKS * Math.fround(1 / 60),
);
export function validMmoPrediction(v: unknown): v is MmoPrediction {
  return (
    record(v) &&
    point(v.position) &&
    finite(v.yaw, 100_000) &&
    finite(v.verticalSpeed, 100) &&
    typeof v.grounded === "boolean" &&
    finite(v.velocity, 100) &&
    motion(v.state) &&
    count(v.stateTicks, MAX_MMO_STATE_TICKS) &&
    count(v.cooldownTicks, 600)
  );
}
export function validMmoActor(v: unknown): v is MmoActor {
  if (
    !record(v) ||
    !id(v.id) ||
    !text(v.name, 64, 1) ||
    !["paladin", "sorceror", "zombie", "warrok"].includes(String(v.class)) ||
    !["player", "npc"].includes(String(v.kind)) ||
    !point(v.position) ||
    !finite(v.yaw, 100_000) ||
    !finite(v.health, 1e9) ||
    !finite(v.maxHealth, 1e9) ||
    v.health < 0 ||
    v.maxHealth <= 0 ||
    v.health > v.maxHealth ||
    !record(v.stats) ||
    !motion(v.state) ||
    !finite(v.actionTime, MAX_MMO_STATE_TIME) ||
    v.actionTime < 0 ||
    !finite(v.animationTime, MAX_MMO_STATE_TIME) ||
    v.animationTime < 0 ||
    typeof v.alive !== "boolean" ||
    v.alive !== v.health > 0 ||
    (v.state === "death") !== !v.alive ||
    !item(v.weapon) ||
    !count(v.attacks) ||
    !count(v.hits)
  )
    return false;
  const s = v.stats;
  return (
    s.health === v.health &&
    s.maxHealth === v.maxHealth &&
    [s.strength, s.wisdomness, s.benchpress, s.curl, s.level].every(id) &&
    count(s.experience) &&
    optional(v.prediction, validMmoPrediction) &&
    optional(
      v.inventory,
      (slots) =>
        Array.isArray(slots) && slots.length === 32 && slots.every(item),
    ) &&
    optional(v.inventoryVersion, count)
  );
}
export function validMmoEvent(v: unknown): v is MmoEvent {
  return (
    record(v) &&
    count(v.id) &&
    count(v.tick) &&
    [
      "attack",
      "damage",
      "death",
      "chat",
      "join",
      "leave",
      "respawn",
      "inventory",
    ].includes(String(v.kind)) &&
    optional(v.actor, id) &&
    optional(v.target, id) &&
    optional(v.amount, (n) => finite(n, 1e9) && n >= 0) &&
    optional(v.text, (s) => text(s, 512)) &&
    optional(v.name, (s) => text(s, 64))
  );
}
export function validMmoPopulation(v: unknown): v is MmoPopulation {
  return (
    record(v) &&
    count(v.players, 32) &&
    count(v.npcs, 1024) &&
    count(v.spawners, 1024)
  );
}
function actors(v: unknown, playerId?: number): v is MmoActor[] {
  return (
    Array.isArray(v) &&
    v.length <= 64 &&
    v.every(validMmoActor) &&
    new Set(v.map((a) => a.id)).size === v.length &&
    (playerId === undefined || v.every((a) => a.id !== playerId))
  );
}
function events(v: unknown, max: number): v is MmoEvent[] {
  return (
    Array.isArray(v) &&
    v.length <= max &&
    v.every(validMmoEvent) &&
    v.every((e, i) => i === 0 || e.id > v[i - 1].id)
  );
}
export function validMmoSnapshot(v: unknown): v is MmoSnapshot {
  return (
    record(v) &&
    v.type === "snapshot" &&
    count(v.tick) &&
    validMmoActor(v.player) &&
    v.player.kind === "player" &&
    v.player.prediction !== undefined &&
    v.player.inventory !== undefined &&
    v.player.inventoryVersion !== undefined &&
    actors(v.actors, v.player.id) &&
    Array.isArray(v.despawned) &&
    v.despawned.length <= 64 &&
    v.despawned.every(id) &&
    new Set(v.despawned).size === v.despawned.length &&
    events(v.events, 2048) &&
    count(v.ackInput) &&
    count(v.lastEventId) &&
    v.events.every((e) => e.id <= (v.lastEventId as number)) &&
    typeof v.eventsTruncated === "boolean" &&
    validMmoPopulation(v.population)
  );
}
export function validMmoUiAction(v: unknown): v is MmoUiAction {
  if (!record(v)) return false;
  switch (v.type) {
    case "join":
      return (
        only(v, ["type", "name", "class"]) &&
        text(v.name, 64, 1) &&
        v.name.trim().length > 0 &&
        optional(v.class, (c) => c === "paladin" || c === "sorceror")
      );
    case "swap":
      return (
        only(v, ["type", "version", "from", "to"]) &&
        count(v.version) &&
        count(v.from, 31) &&
        count(v.to, 31) &&
        v.from !== v.to
      );
    case "chat":
      return (
        only(v, ["type", "text"]) &&
        text(v.text, 64, 1) &&
        v.text.trim().length > 0
      );
    case "respawn":
      return only(v, ["type"]);
    default:
      return false;
  }
}
export function validMmoIconsManifest(v: unknown): v is MmoIconsManifest {
  return (
    record(v) &&
    v.schema === "aethyrforge.mmorpg-ui.v1" &&
    record(v.icons) &&
    Object.keys(v.icons).length === MMO_ICON_KEYS.length &&
    MMO_ICON_KEYS.every((key) => {
      const icon = (v.icons as Record<string, unknown>)[key];
      return (
        record(icon) &&
        typeof icon.url === "string" &&
        /^\/private-assets\/[a-zA-Z0-9_.-]+\.(?:png|jpg|jpeg)$/.test(
          icon.url,
        ) &&
        typeof icon.sha256 === "string" &&
        /^[a-f0-9]{64}$/.test(icon.sha256) &&
        count(icon.byteLength, 32 * 1024 * 1024) &&
        icon.byteLength > 0 &&
        count(icon.width, 8192) &&
        icon.width > 0 &&
        count(icon.height, 8192) &&
        icon.height > 0
      );
    })
  );
}
export function validMmoUiState(v: unknown): v is MmoUiState {
  return (
    record(v) &&
    ["login", "connecting", "online", "disconnected", "error"].includes(
      String(v.connection),
    ) &&
    optional(v.player, validMmoActor) &&
    actors(v.actors, record(v.player) ? (v.player.id as number) : undefined) &&
    events(v.events, MMO_UI_EVENT_LIMIT) &&
    optional(v.population, validMmoPopulation) &&
    optional(v.icons, validMmoIconsManifest) &&
    optional(v.message, (s) => text(s, 2048))
  );
}
export function validMmoServerMessage(v: unknown): v is MmoServerMessage {
  if (!record(v)) return false;
  if (v.type === "snapshot") return validMmoSnapshot(v);
  if (v.type === "error") return text(v.message, 2048, 1);
  if (v.type === "receipt")
    return (
      text(v.requestId, 128, 1) &&
      typeof v.ok === "boolean" &&
      typeof v.duplicate === "boolean" &&
      optional(v.message, (s) => text(s, 2048)) &&
      optional(v.inventoryVersion, count)
    );
  if (
    v.type !== "welcome" ||
    v.protocol !== MMORPG_PROTOCOL ||
    !id(v.playerId) ||
    !validMmoSnapshot(v.snapshot) ||
    v.snapshot.player.id !== v.playerId ||
    !record(v.world)
  )
    return false;
  const w = v.world;
  return (
    count(w.seed) &&
    finite(w.worldScale, 10) &&
    w.worldScale > 0 &&
    typeof w.sourceCommit === "string" &&
    /^[a-f0-9]{40}$/.test(w.sourceCommit) &&
    w.tickRate === 60 &&
    w.snapshotRate === 10 &&
    text(w.release, 128, 1) &&
    point(w.spawn) &&
    finite(w.yaw, 100_000)
  );
}
