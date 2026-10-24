import type { ProjectDocument } from "./project-document.ts";
import { materialHash, type MaterialSource } from "./creator-material.ts";
import type { EnvironmentSettings } from "./protocol.ts";
export const PLANET_SOURCE = "planet-v1.json";
export type PlanetRecipe = {
  schema: "openagents.verse.planet.v1";
  seed: number;
  side_metres: 64;
  spawn: [0, 1.62, 12];
  style: "soft-pastels" | "bold-fantasy";
  initial_hour: number;
  cycle_enabled: boolean;
  day_minutes: number;
  clock_anchor_unix_ms: number;
};
export function checkPlanetRecipe(value: unknown): PlanetRecipe {
  const r = value as PlanetRecipe;
  const keys = [
    "schema",
    "seed",
    "side_metres",
    "spawn",
    "style",
    "initial_hour",
    "cycle_enabled",
    "day_minutes",
    "clock_anchor_unix_ms",
  ];
  if (
    !r ||
    typeof r !== "object" ||
    Array.isArray(r) ||
    Object.keys(r).length !== keys.length ||
    Object.keys(r).some((k) => !keys.includes(k)) ||
    r.schema !== "openagents.verse.planet.v1" ||
    !Number.isInteger(r.seed) ||
    r.seed < 0 ||
    r.seed > 0xffffffff ||
    r.side_metres !== 64 ||
    !Array.isArray(r.spawn) ||
    r.spawn.length !== 3 ||
    r.spawn.some((n, i) => n !== [0, 1.62, 12][i]) ||
    !["soft-pastels", "bold-fantasy"].includes(r.style) ||
    !Number.isFinite(r.initial_hour) ||
    r.initial_hour < 0 ||
    r.initial_hour >= 24 ||
    typeof r.cycle_enabled !== "boolean" ||
    !Number.isInteger(r.day_minutes) ||
    r.day_minutes < 2 ||
    r.day_minutes > 60 ||
    !Number.isSafeInteger(r.clock_anchor_unix_ms) ||
    r.clock_anchor_unix_ms < 0
  )
    throw new Error(
      "Choose a supported courtyard style, time, and day length.",
    );
  return structuredClone(r);
}
export function checkPlanetContent(value: {
  entities?: { position: number[]; scale?: number }[];
  rooms?: unknown[];
  removedWalls?: string[];
}) {
  if (
    (value.entities?.length ?? 0) > 128 ||
    value.rooms?.length ||
    value.removedWalls?.length ||
    value.entities?.some(
      (e) =>
        e.position.length !== 3 ||
        e.position.some((n) => !Number.isFinite(n)) ||
        Math.abs(e.position[0]) > 28 ||
        Math.abs(e.position[2]) > 28 ||
        e.position[1] < 0 ||
        e.position[1] > 12 ||
        (e.scale ?? 1) < 0.5 ||
        (e.scale ?? 1) > 2,
    )
  ) {
    throw new Error(
      "Keep up to 128 objects within this courtyard's size and height limits.",
    );
  }
}
export function planetCameraAllowed(pose: { eye: number[]; mode: string }) {
  return (
    pose.mode === "walk" &&
    pose.eye.length === 3 &&
    pose.eye.every(Number.isFinite) &&
    Math.abs(pose.eye[0]) < 31 &&
    Math.abs(pose.eye[2]) < 31 &&
    pose.eye[1] >= 1.5 &&
    pose.eye[1] <= 13.62
  );
}
function sameValues(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (
    !a ||
    !b ||
    typeof a !== "object" ||
    typeof b !== "object" ||
    Array.isArray(a) !== Array.isArray(b)
  )
    return false;
  if (Array.isArray(a) && Array.isArray(b))
    return a.length === b.length && a.every((v, i) => sameValues(v, b[i]));
  const aa = a as Record<string, unknown>,
    bb = b as Record<string, unknown>,
    keys = Object.keys(aa);
  return (
    keys.length === Object.keys(bb).length &&
    keys.every((k) => Object.hasOwn(bb, k) && sameValues(aa[k], bb[k]))
  );
}
export function personalSettingsOnly(
  before: ProjectDocument,
  after: ProjectDocument,
) {
  if (
    before.namespace !== "personal-planet" ||
    after.namespace !== "personal-planet" ||
    before.source_revision !== after.source_revision
  )
    return false;
  const source = (d: ProjectDocument) =>
    d.originals.find((s) => s.name === PLANET_SOURCE);
  if (
    !source(before) ||
    !source(after) ||
    JSON.parse(source(before)!.text).seed !==
      JSON.parse(source(after)!.text).seed
  )
    return false;
  const content = (d: ProjectDocument) => ({
    entities: d.entities,
    rooms: d.rooms,
    removed_walls: d.removed_walls,
    materials: d.materials,
    layers: d.layers,
    originals: Object.fromEntries(
      d.originals
        .filter((s) => s.name !== PLANET_SOURCE)
        .map((s) => [s.name, { text: s.text, sha256: s.sha256 }]),
    ),
  });
  return sameValues(content(before), content(after));
}
export function planetHour(r: PlanetRecipe, now: number) {
  const elapsed = r.cycle_enabled
    ? (Math.max(0, now - r.clock_anchor_unix_ms) * 24) / (r.day_minutes * 60000)
    : 0;
  return (((r.initial_hour + elapsed) % 24) + 24) % 24;
}
export function planetEnvironment(
  r: PlanetRecipe,
  base: EnvironmentSettings,
  now: number,
): EnvironmentSettings {
  const hour = planetHour(r, now),
    elevation = Math.max(
      -20,
      Math.min(80, 65 * Math.sin(((hour - 6) * Math.PI * 2) / 24)),
    ),
    day = Math.max(0, Math.min(1, elevation / 15));
  return {
    ...base,
    mode: hour < 6 || hour >= 19 ? "night" : "day",
    skySource: "generated",
    sunAzimuth: (hour * 15 + (hour < 6 || hour >= 19 ? 180 : 0)) % 360,
    sunElevation: hour < 6 || hour >= 19 ? -elevation : elevation,
    sunIntensity: day * 3,
    moonIntensity: (1 - day) * 3,
    ambientIntensity: 3.5 - day * 2.3,
    skyIntensity: 0.45 + day * 0.65,
    skyBackgroundIntensity: 0.45 + day * 0.55,
    fogDensity: 0.003,
  };
}
/** Checked sources stay attached to kernel history, including Undo and Redo. */
export class PlanetSources {
  #text: string | undefined;
  #recipe: PlanetRecipe | undefined;
  async accept(document: ProjectDocument): Promise<MaterialSource | undefined> {
    if (document.namespace !== "personal-planet") return undefined;
    const sources = document.originals.filter((s) => s.name === PLANET_SOURCE);
    if (sources.length !== 1)
      throw new Error("Open this courtyard with its saved world settings.");
    const source = sources[0];
    if (
      new TextEncoder().encode(source.text).length > 24 * 1024 ||
      (await materialHash(new TextEncoder().encode(source.text))) !==
        source.sha256
    )
      throw new Error("The saved world settings do not match this file.");
    checkPlanetRecipe(JSON.parse(source.text));
    if (
      Object.keys(document.rooms).length ||
      document.removed_walls.length ||
      Object.values(document.entities).some(
        (e) =>
          e.position.some((n) => !Number.isFinite(n)) ||
          Math.abs(e.position[0]) > 28 ||
          Math.abs(e.position[2]) > 28 ||
          e.position[1] < 0 ||
          e.position[1] > 12 ||
          e.scale < 0.5 ||
          e.scale > 2,
      )
    )
      throw new Error(
        "Keep objects within this courtyard's size and height limits.",
      );

    return structuredClone(source);
  }
  current(snapshot: any): PlanetRecipe | undefined {
    const source = snapshot.authoring?.planetSource;
    if (!source) return undefined;
    // Imports are checked before commit; kernel Undo/Redo returns those same sources.
    if (this.#text !== source.text) {
      this.#recipe = checkPlanetRecipe(JSON.parse(source.text));
      this.#text = source.text;
    }
    return this.#recipe;
  }
}
