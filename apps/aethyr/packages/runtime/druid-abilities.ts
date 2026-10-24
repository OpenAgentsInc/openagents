/** Local play actions adapted from the OpenAgents Grove druid. */
export const DRUID_ABILITIES = [
  {
    id: "meteor_shower",
    name: "Meteor Shower",
    key: "1",
    hint: "Six meteors break the area you aim at.",
  },
  {
    id: "lightning_bolt",
    name: "Lightning Bolt",
    key: "2",
    hint: "A bolt shoots forward from your hands.",
  },
  {
    id: "thunderbolt",
    name: "Thunderbolt",
    key: "3",
    hint: "Lightning strikes your target from above.",
  },
  {
    id: "destroy_castle",
    name: "Destroy the Castle",
    key: "4",
    hint: "A sustained meteor and lightning barrage reduces the distant castle to rubble.",
  },
] as const;
export type DruidAbility = (typeof DRUID_ABILITIES)[number]["id"];
export function isDruidAbility(value: unknown): value is DruidAbility {
  return DRUID_ABILITIES.some((ability) => ability.id === value);
}
export interface DruidStatus {
  enabled: boolean;
  preparing?: boolean;
  selected: DruidAbility | null;
  destroyed: number;
  debris: number;
  rubble?: number;
  effects: number;
  casts: number;
  impacts: number;
  tick: number;
  enemies?: {
    total: number;
    alive: number;
    killed: number;
    modelLoaded: boolean;
    rendered: number;
  };
  castle?: {
    total: number;
    remaining: number;
    destroyed: number;
    active: boolean;
    complete: boolean;
    cancelled: boolean;
    preparing: boolean;
    waves: number;
    impacts: number;
    elapsed: number;
    rubble: number;
  };
  target?: {
    point: [number, number, number];
    normal: [number, number, number];
    distance: number;
    nodeId?: number;
    enemyId?: number;
  };
  casting?: { ability: DruidAbility; remaining: number };
  lastImpact?: { point: [number, number, number]; radius: number };
  rendered?: {
    maskedNodes: number;
    fracturedSources: number;
    standingTriangles: number;
    debrisTriangles: number;
    effectTriangles: number;
    aimTriangles?: number;
    rubbleCount?: number;
    rubbleTriangles?: number;
    geometryBytes: number;
  };
}
export function validDruidStatus(value: unknown): value is DruidStatus {
  if (!value || typeof value !== "object") return false;
  const s = value as DruidStatus;
  const vector = (v: unknown) =>
    Array.isArray(v) &&
    v.length === 3 &&
    v.every(
      (x) =>
        typeof x === "number" && Number.isFinite(x) && Math.abs(x) <= 20_000,
    );
  return (
    typeof s.enabled === "boolean" &&
    (s.preparing === undefined || typeof s.preparing === "boolean") &&
    (s.selected === null || isDruidAbility(s.selected)) &&
    [s.destroyed, s.debris, s.effects, s.casts, s.impacts, s.tick].every(
      (n) => Number.isSafeInteger(n) && n >= 0,
    ) &&
    s.destroyed <= 65_536 &&
    s.debris <= 256 &&
    (s.rubble === undefined ||
      (Number.isSafeInteger(s.rubble) && s.rubble >= 0 && s.rubble <= 1024)) &&
    s.effects <= 256 &&
    (s.enemies === undefined ||
      ([
        s.enemies.total,
        s.enemies.alive,
        s.enemies.killed,
        s.enemies.rendered,
      ].every((n) => Number.isSafeInteger(n) && n >= 0 && n <= 16) &&
        s.enemies.alive + s.enemies.killed === s.enemies.total &&
        s.enemies.rendered <= s.enemies.total &&
        typeof s.enemies.modelLoaded === "boolean")) &&
    (s.castle === undefined ||
      ([s.castle.total, s.castle.remaining, s.castle.destroyed].every(
        (n) => Number.isSafeInteger(n) && n >= 0 && n <= 65_536,
      ) &&
        s.castle.total > 0 &&
        s.castle.remaining + s.castle.destroyed === s.castle.total &&
        [
          s.castle.active,
          s.castle.complete,
          s.castle.cancelled,
          s.castle.preparing,
        ].every((v) => typeof v === "boolean") &&
        s.castle.complete === (s.castle.remaining === 0) &&
        [s.castle.waves, s.castle.impacts].every(
          (n) => Number.isSafeInteger(n) && n >= 0,
        ) &&
        Number.isFinite(s.castle.elapsed) &&
        s.castle.elapsed >= 0 &&
        Number.isSafeInteger(s.castle.rubble) &&
        s.castle.rubble >= 0 &&
        s.castle.rubble <= 1024)) &&
    (s.target === undefined ||
      (vector(s.target.point) &&
        vector(s.target.normal) &&
        Number.isFinite(s.target.distance) &&
        s.target.distance >= 0 &&
        s.target.distance <=
          (s.selected === "destroy_castle" ? 20_000 : 100))) &&
    (s.casting === undefined ||
      (isDruidAbility(s.casting.ability) &&
        Number.isFinite(s.casting.remaining) &&
        s.casting.remaining >= 0 &&
        s.casting.remaining <= 10)) &&
    (s.lastImpact === undefined ||
      (vector(s.lastImpact.point) &&
        Number.isFinite(s.lastImpact.radius) &&
        s.lastImpact.radius > 0 &&
        s.lastImpact.radius <= 20)) &&
    (s.rendered === undefined ||
      (Object.values(s.rendered).every(
        (n) => Number.isSafeInteger(n) && n >= 0,
      ) &&
        (s.rendered.aimTriangles === undefined ||
          s.rendered.aimTriangles <= 128)))
  );
}
