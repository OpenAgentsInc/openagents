/**
 * World characters with a small text dataset and simple capabilities: greet
 * when the player comes near, give a hint about the next step when asked,
 * reply when named, and emote. No model is called; every line comes from
 * `npc-dataset.json`.
 */
import dataset from "./npc-dataset.json" with { type: "json" };

export type NpcV3 = [number, number, number];
export type NpcEmote = "wave" | "hop" | "spin" | "bow";
export type NpcLineKind = "greet" | "hint" | "reply" | "emote";
export type TalkIntent = "reply" | "hint" | "emote";
export const NPC_FIGURES = [
  "cat",
  "witch",
  "ghost",
  "gardener",
  "ranger",
] as const;
export type NpcFigure = (typeof NPC_FIGURES)[number];
export const NPC_LIMITS = Object.freeze({
  npcs: 16,
  lines: 4,
  name: 64,
  text: 512,
  /** Talking needs the player this close when no character is named. */
  talkRadius: 6,
  /** A greeting re-arms after the player walks this much farther away. */
  rearm: 2,
  /** Seconds an emote animates. */
  emoteSeconds: 1.4,
});

export interface NpcDefinition {
  id: string;
  name: string;
  aliases?: string[];
  figure: NpcFigure;
  position: NpcV3;
  face: [number, number];
  greetRadius: number;
  lines: {
    greet: string[];
    reply: string[];
    emote: { emote: NpcEmote; text: string }[];
  };
}
export interface NpcPlace {
  id: string;
  name: string;
  center: [number, number];
  size: [number, number];
}
export type NpcHint =
  | { unvisited: string; text: string }
  | { unmet: string; text: string }
  | { text: string };
export interface NpcWorld {
  places: NpcPlace[];
  npcs: NpcDefinition[];
  hints: NpcHint[];
  styles?: Record<string, string>;
}
export interface NpcLine {
  sequence: number;
  npc: string;
  name: string;
  kind: NpcLineKind;
  text: string;
  emote?: NpcEmote;
}
export interface NpcView {
  npcs: { id: string; name: string }[];
  nearby: string | null;
  lines: NpcLine[];
}
export interface NpcPose {
  id: string;
  name: string;
  figure: NpcFigure;
  position: NpcV3;
  /** Front is local +z; yaw turns it toward atan2(dx, dz). */
  yaw: number;
  emote?: NpcEmote;
  /** 0..1 progress through the current emote. */
  emoteProgress: number;
}
/** One on-screen name label, in canvas fractions (0..1, top left origin). */
export interface NpcLabel {
  id: string;
  name: string;
  x: number;
  y: number;
  nearby: boolean;
  line?: { kind: NpcLineKind; text: string };
}
/** A wall box that hides a label behind it. */
export interface NpcOccluder {
  min: NpcV3;
  max: NpcV3;
}
/** Walls only: floors and ceilings never sit between eye and label. */
export function npcOccluders(
  colliders: readonly { id?: string; min: number[]; max: number[] }[],
): NpcOccluder[] {
  return colliders
    .filter(
      (c) =>
        !/(?:^|:)(?:ceiling|floor)(?::|$)/.test(c.id ?? "") &&
        c.min?.length === 3 &&
        c.max?.length === 3 &&
        c.max[1] - c.min[1] > 0.5,
    )
    .map((c) => ({
      min: [c.min[0], c.min[1], c.min[2]] as NpcV3,
      max: [c.max[0], c.max[1], c.max[2]] as NpcV3,
    }));
}
/** Whether any box crosses the open segment from `a` to `b` (slab test). */
export function segmentBlocked(
  a: NpcV3,
  b: NpcV3,
  boxes: readonly NpcOccluder[],
): boolean {
  const d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  for (const box of boxes) {
    let near = 0.02,
      far = 0.98,
      hit = true;
    for (let k = 0; k < 3 && hit; k++) {
      if (Math.abs(d[k]) < 1e-9) {
        if (a[k] < box.min[k] || a[k] > box.max[k]) hit = false;
        continue;
      }
      let t0 = (box.min[k] - a[k]) / d[k],
        t1 = (box.max[k] - a[k]) / d[k];
      if (t0 > t1) [t0, t1] = [t1, t0];
      near = Math.max(near, t0);
      far = Math.min(far, t1);
      if (near > far) hit = false;
    }
    if (hit) return true;
  }
  return false;
}

const ID = /^[a-z0-9][a-z0-9_-]{0,63}$/;
const text = (value: unknown, max: number) =>
  typeof value === "string" &&
  value.trim().length > 0 &&
  value.length <= max &&
  !/[\u0000-\u001f]/.test(value);
const finite = (value: unknown, bound: number) =>
  typeof value === "number" &&
  Number.isFinite(value) &&
  Math.abs(value) <= bound;
const pair = (value: unknown, bound: number) =>
  Array.isArray(value) &&
  value.length === 2 &&
  value.every((v) => finite(v, bound));

/** Checks one world's characters; any problem refuses the whole dataset. */
export function validateNpcWorld(value: unknown): NpcWorld {
  const fail = (why: string): never => {
    throw new Error(`The character dataset is invalid: ${why}.`);
  };
  if (!value || typeof value !== "object") fail("world");
  const world = value as NpcWorld;
  if (!Array.isArray(world.places) || world.places.length > 16) fail("places");
  const places = new Set<string>();
  for (const place of world.places) {
    if (!ID.test(place.id) || places.has(place.id)) fail("place id");
    if (!text(place.name, 64) || !pair(place.center, 10000))
      fail(`place ${place.id}`);
    if (!pair(place.size, 10000) || place.size.some((v) => v <= 0))
      fail(`place ${place.id} size`);
    places.add(place.id);
  }
  if (
    !Array.isArray(world.npcs) ||
    !world.npcs.length ||
    world.npcs.length > NPC_LIMITS.npcs
  )
    fail("characters");
  const ids = new Set<string>();
  for (const npc of world.npcs) {
    const names = [npc.id, ...(npc.aliases ?? [])];
    if (names.some((id) => !ID.test(id) || ids.has(id))) fail("character id");
    names.forEach((id) => ids.add(id));
    if (!text(npc.name, NPC_LIMITS.name)) fail(`${npc.id} name`);
    if (!NPC_FIGURES.includes(npc.figure)) fail(`${npc.id} figure`);
    if (
      !Array.isArray(npc.position) ||
      npc.position.length !== 3 ||
      !npc.position.every((v) => finite(v, 10000)) ||
      !pair(npc.face, 10000)
    )
      fail(`${npc.id} position`);
    if (!finite(npc.greetRadius, 50) || npc.greetRadius <= 0)
      fail(`${npc.id} greeting distance`);
    const lines = npc.lines;
    for (const kind of ["greet", "reply"] as const)
      if (
        !Array.isArray(lines?.[kind]) ||
        !lines[kind].length ||
        lines[kind].length > 32 ||
        !lines[kind].every((line) => text(line, NPC_LIMITS.text))
      )
        fail(`${npc.id} ${kind} lines`);
    if (
      !Array.isArray(lines.emote) ||
      !lines.emote.length ||
      lines.emote.length > 16 ||
      !lines.emote.every(
        (e) =>
          ["wave", "hop", "spin", "bow"].includes(e.emote) &&
          text(e.text, NPC_LIMITS.text),
      )
    )
      fail(`${npc.id} emotes`);
  }
  if (
    !Array.isArray(world.hints) ||
    !world.hints.length ||
    world.hints.length > 32
  )
    fail("hints");
  for (const hint of world.hints) {
    if (!text(hint.text, NPC_LIMITS.text)) fail("hint text");
    if ("unvisited" in hint && !places.has(hint.unvisited))
      fail(`hint place ${hint.unvisited}`);
    if ("unmet" in hint && !world.npcs.some((n) => n.id === hint.unmet))
      fail(`hint character ${hint.unmet}`);
  }
  // The last hint always applies, so asking always gets an answer.
  const last = world.hints.at(-1)!;
  if ("unvisited" in last || "unmet" in last)
    fail("the last hint needs no condition");
  for (const [style, id] of Object.entries(world.styles ?? {}))
    if (!ID.test(style) || !world.npcs.some((n) => n.id === id))
      fail(`style ${style}`);
  return world;
}

/** The checked-in characters for one world, or undefined when it has none. */
export function npcWorld(worldId: string): NpcWorld | undefined {
  if (dataset.schema !== "openagents.aethyr.npcs.v1")
    throw new Error("The character dataset needs a newer engine.");
  const world = (dataset.worlds as Record<string, unknown>)[worldId];
  return world === undefined ? undefined : validateNpcWorld(world);
}

const inside = (place: NpcPlace, x: number, z: number) =>
  Math.abs(x - place.center[0]) <= place.size[0] / 2 &&
  Math.abs(z - place.center[1]) <= place.size[1] / 2;

export class NpcDirector {
  readonly npcs: NpcDefinition[];
  private readonly places: NpcPlace[];
  private readonly hints: NpcHint[];
  private sequence: number;
  private lines: NpcLine[] = [];
  private readonly spoken = new Map<string, { line: NpcLine; at: number }>();
  private readonly turns = new Map<string, Record<string, number>>();
  private readonly greeted = new Set<string>();
  private readonly met = new Set<string>();
  private readonly visited = new Set<string>();
  private readonly yaw = new Map<string, number>();
  private readonly emotes = new Map<string, { emote: NpcEmote; at: number }>();
  private player: NpcV3 | undefined;
  private time = 0;

  /** `style` picks one character per planet style; worlds without styles keep all. */
  /** `startSequence` keeps line numbers rising when a planet changes style. */
  constructor(world: NpcWorld, style?: string, startSequence = 0) {
    this.sequence = startSequence;
    const chosen = world.styles
      ? (world.styles[style ?? ""] ?? Object.values(world.styles)[0])
      : undefined;
    this.npcs = world.npcs.filter((n) => !chosen || n.id === chosen);
    this.places = world.places;
    this.hints = world.hints;
    for (const npc of this.npcs)
      this.yaw.set(
        npc.id,
        Math.atan2(
          npc.face[0] - npc.position[0],
          npc.face[1] - npc.position[2],
        ),
      );
  }
  static forWorld(worldId: string, style?: string, startSequence = 0) {
    const world = npcWorld(worldId);
    return world ? new NpcDirector(world, style, startSequence) : undefined;
  }
  get lastSequence() {
    return this.sequence;
  }
  private distance(npc: NpcDefinition, point: NpcV3) {
    return Math.hypot(point[0] - npc.position[0], point[2] - npc.position[2]);
  }
  private say(
    npc: NpcDefinition,
    kind: NpcLineKind,
    text: string,
    emote?: NpcEmote,
  ): NpcLine {
    const line: NpcLine = {
      sequence: ++this.sequence,
      npc: npc.id,
      name: npc.name,
      kind,
      text,
      ...(emote ? { emote } : {}),
    };
    this.lines = [...this.lines, line].slice(-NPC_LIMITS.lines);
    this.spoken.set(npc.id, { line, at: this.time });
    this.met.add(npc.id);
    if (emote) this.emotes.set(npc.id, { emote, at: this.time });
    return line;
  }
  private next<T>(npc: NpcDefinition, kind: string, items: T[]): T {
    const turns = this.turns.get(npc.id) ?? {};
    const turn = turns[kind] ?? 0;
    turns[kind] = turn + 1;
    this.turns.set(npc.id, turns);
    return items[turn % items.length];
  }
  /** The closest character within talking distance. */
  nearest(point = this.player): NpcDefinition | undefined {
    if (!point) return undefined;
    let best: NpcDefinition | undefined,
      bestDistance: number = NPC_LIMITS.talkRadius;
    for (const npc of this.npcs) {
      const d = this.distance(npc, point);
      if (d <= bestDistance) ((best = npc), (bestDistance = d));
    }
    return best;
  }
  /** Advance with the player's feet position; returns any greetings said. */
  update(player: NpcV3, seconds: number): NpcLine[] {
    if (!player.every(Number.isFinite) || !Number.isFinite(seconds)) return [];
    const elapsed = Math.max(0, Math.min(0.25, seconds - this.time));
    this.time = seconds;
    this.player = [...player] as NpcV3;
    for (const place of this.places)
      if (inside(place, player[0], player[2])) this.visited.add(place.id);
    const said: NpcLine[] = [];
    for (const npc of this.npcs) {
      const d = this.distance(npc, player);
      // Turn toward a nearby player; otherwise drift back to the rest pose.
      const rest = Math.atan2(
        npc.face[0] - npc.position[0],
        npc.face[1] - npc.position[2],
      );
      const want =
        d <= npc.greetRadius + NPC_LIMITS.rearm
          ? Math.atan2(player[0] - npc.position[0], player[2] - npc.position[2])
          : rest;
      const current = this.yaw.get(npc.id) ?? rest;
      const delta = Math.atan2(
        Math.sin(want - current),
        Math.cos(want - current),
      );
      this.yaw.set(npc.id, current + delta * Math.min(1, elapsed * 4));
      if (d <= npc.greetRadius && !this.greeted.has(npc.id)) {
        this.greeted.add(npc.id);
        said.push(
          this.say(npc, "greet", this.next(npc, "greet", npc.lines.greet)),
        );
        // Greeting comes with a gesture; only an asked emote adds its own line.
        this.emotes.set(npc.id, {
          emote: npc.lines.emote[0].emote,
          at: this.time,
        });
      } else if (d > npc.greetRadius + NPC_LIMITS.rearm)
        this.greeted.delete(npc.id);
    }
    return said;
  }
  /** Resolve an exact character ID or alias. */
  find(id: string): NpcDefinition | undefined {
    const key = id.toLowerCase();
    return this.npcs.find((n) => n.id === key || n.aliases?.includes(key));
  }
  hint(): string {
    const hint = this.hints.find((h) =>
      "unvisited" in h
        ? !this.visited.has(h.unvisited)
        : "unmet" in h
          ? !this.met.has(h.unmet) && this.npcs.some((n) => n.id === h.unmet)
          : true,
    );
    return hint!.text;
  }
  /** One line for a host or keyboard request. Throws a readable reason. */
  talk(intent: TalkIntent, id?: string): NpcLine {
    const npc = id === undefined ? this.nearest() : this.find(id);
    if (!npc)
      throw new Error(
        id === undefined
          ? "Walk up to someone to talk."
          : `There's no one called ${id.slice(0, 32)} here.`,
      );
    if (intent === "hint") return this.say(npc, "hint", this.hint());
    if (intent === "reply")
      return this.say(npc, "reply", this.next(npc, "reply", npc.lines.reply));
    if (intent === "emote") {
      const e = this.next(npc, "emote", npc.lines.emote);
      return this.say(npc, "emote", e.text, e.emote);
    }
    throw new Error("Choose reply, hint, or emote.");
  }
  view(): NpcView {
    return {
      npcs: this.npcs.map((n) => ({ id: n.id, name: n.name })),
      nearby: this.nearest()?.id ?? null,
      lines: [...this.lines],
    };
  }
  poses(): NpcPose[] {
    return this.npcs.map((npc) => {
      const emote = this.emotes.get(npc.id);
      const progress = emote
        ? (this.time - emote.at) / NPC_LIMITS.emoteSeconds
        : 1;
      return {
        id: npc.id,
        name: npc.name,
        figure: npc.figure,
        position: [...npc.position] as NpcV3,
        yaw: this.yaw.get(npc.id) ?? 0,
        ...(emote && progress < 1 ? { emote: emote.emote } : {}),
        emoteProgress: Math.max(0, Math.min(1, progress)),
      };
    });
  }
  /** Each character's newest line said within the last `seconds`. */
  recent(seconds: number): Map<string, NpcLine> {
    const out = new Map<string, NpcLine>();
    for (const [id, said] of this.spoken)
      if (this.time - said.at <= seconds) out.set(id, said.line);
    return out;
  }
}
