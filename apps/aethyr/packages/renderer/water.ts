/** Source-independent geometry for bounded cosmetic fountain streams. */
export type WaterVec3 = readonly [number, number, number];
/** Conservative envelope for the owned GPU stream surface displacement. */
export const WATER_JET_BREAKUP_METERS = 0.006;

export interface WaterSplash {
  /** Maximum horizontal travel and height of the inferred impact spray, in meters. */
  radius: number;
  height: number;
  droplets?: number;
  size?: number;
}

export interface WaterJet {
  id: string;
  origin: WaterVec3;
  /** Initial world-space velocity in meters per second. */
  velocity: WaterVec3;
  surfaceY: number;
  /** Stream radius at the nozzle in meters. */
  radius: number;
  /** Optional radius at impact; omitted values retain a constant-radius stream. */
  endRadius?: number;
  /** Radius interpolation exponent, from 1 to 4; defaults to 1.5. */
  taper?: number;
  splash?: WaterSplash;
  /** Positive downward acceleration; defaults to 9.81 meters per second squared. */
  gravity?: number;
  segments?: number;
  sides?: number;
}

export interface WaterImpact {
  id: string;
  position: [number, number, number];
  flightTime: number;
  splash?: WaterSplash;
}

export interface WaterJetGeometry {
  /** 48-byte position, normal, UV, tangent vertices. UV.y is flight time; tangent.w is normalized path progress. */
  vertices: Float32Array;
  indices: Uint32Array;
  impacts: WaterImpact[];
  bounds: {
    min: [number, number, number];
    max: [number, number, number];
  } | null;
}

export const WATER_JET_LIMITS = Object.freeze({
  jets: 64,
  segments: 256,
  sides: 32,
  vertices: 131_072,
  indices: 786_432,
  coordinate: 100_000,
  flightTime: 10,
});

type Vec3 = [number, number, number];
const length = (v: WaterVec3) => Math.hypot(v[0], v[1], v[2]);
const dot = (a: WaterVec3, b: WaterVec3) =>
  a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: WaterVec3, b: WaterVec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const normalized = (v: WaterVec3): Vec3 => {
  const size = length(v);
  if (size < 1e-10) throw new Error("A water stream needs a stable direction.");
  return [v[0] / size, v[1] / size, v[2] / size];
};
const finiteVector = (v: WaterVec3) =>
  Array.isArray(v) && v.length === 3 && v.every(Number.isFinite);
const firstNormal = (tangent: WaterVec3) =>
  normalized(
    cross(tangent, Math.abs(tangent[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0]),
  );

/** Builds static stream meshes; the GPU can animate flow from UV.y and frame time. */
export function buildWaterJets(jets: readonly WaterJet[]): WaterJetGeometry {
  if (!Array.isArray(jets) || jets.length > WATER_JET_LIMITS.jets)
    throw new Error("Use no more than 64 water streams in one mesh.");
  const ids = new Set<string>();
  let vertexCount = 0;
  let indexCount = 0;
  const prepared = jets.map((jet: WaterJet) => {
    if (
      !jet ||
      typeof jet.id !== "string" ||
      !jet.id.trim() ||
      jet.id.length > 128 ||
      ids.has(jet.id)
    )
      throw new Error(
        "Each water stream needs a unique name up to 128 characters.",
      );
    ids.add(jet.id);
    if (
      !finiteVector(jet.origin) ||
      !finiteVector(jet.velocity) ||
      !Number.isFinite(jet.surfaceY) ||
      !Number.isFinite(jet.radius) ||
      jet.origin.some((n) => Math.abs(n) > WATER_JET_LIMITS.coordinate) ||
      Math.abs(jet.surfaceY) > WATER_JET_LIMITS.coordinate ||
      length(jet.velocity) < 1e-4 ||
      length(jet.velocity) > 100 ||
      jet.radius <= 0 ||
      jet.radius > 1 ||
      jet.origin[1] < jet.surfaceY
    )
      throw new Error(
        "Choose finite stream positions, speed, radius, and water height.",
      );
    // A straight upward arc doubles back along itself and needs a spray model.
    if (
      Math.hypot(jet.velocity[0], jet.velocity[2]) < 1e-4 &&
      jet.velocity[1] > 0
    )
      throw new Error("An upward water stream needs some horizontal velocity.");
    const gravity = jet.gravity ?? 9.81;
    const endRadius = jet.endRadius ?? jet.radius;
    const taper = jet.taper ?? 1.5;
    if (
      !Number.isFinite(endRadius) ||
      endRadius <= 0 ||
      endRadius > 1 ||
      !Number.isFinite(taper) ||
      taper < 1 ||
      taper > 4
    )
      throw new Error(
        "Choose a positive impact radius up to one meter and taper from 1 to 4.",
      );
    if (
      jet.splash &&
      (!Number.isFinite(jet.splash.radius) ||
        jet.splash.radius <= 0 ||
        jet.splash.radius > 2 ||
        !Number.isFinite(jet.splash.height) ||
        jet.splash.height <= 0 ||
        jet.splash.height > 2 ||
        !Number.isInteger(jet.splash.droplets ?? 24) ||
        (jet.splash.droplets ?? 24) < 1 ||
        (jet.splash.droplets ?? 24) > 64 ||
        !Number.isFinite(jet.splash.size ?? 0.008) ||
        (jet.splash.size ?? 0.008) <= 0 ||
        (jet.splash.size ?? 0.008) > 0.05)
    )
      throw new Error(
        "Choose bounded impact spray dimensions and 1 to 64 droplets.",
      );
    const segments = jet.segments ?? 24;
    const sides = jet.sides ?? 8;
    if (!Number.isFinite(gravity) || gravity < 0.01 || gravity > 100)
      throw new Error("Choose water gravity from 0.01 to 100.");
    if (
      !Number.isInteger(segments) ||
      segments < 2 ||
      segments > WATER_JET_LIMITS.segments ||
      !Number.isInteger(sides) ||
      sides < 3 ||
      sides > WATER_JET_LIMITS.sides
    )
      throw new Error(
        "Use 2 to 256 path segments and 3 to 32 sides per stream.",
      );
    const height = jet.origin[1] - jet.surfaceY;
    const vy = jet.velocity[1];
    const root = Math.sqrt(vy * vy + 2 * gravity * height);
    // Avoid cancellation when a fast downward stream starts near the surface.
    const flightTime =
      vy < 0 ? (2 * height) / (root - vy) : (vy + root) / gravity;
    if (
      !Number.isFinite(flightTime) ||
      flightTime <= 1e-6 ||
      flightTime > WATER_JET_LIMITS.flightTime
    )
      throw new Error(
        "A water stream must meet its surface within 10 seconds.",
      );
    const impact: WaterImpact = {
      id: jet.id,
      position: [
        jet.origin[0] + jet.velocity[0] * flightTime,
        jet.surfaceY,
        jet.origin[2] + jet.velocity[2] * flightTime,
      ],
      flightTime,
      ...(jet.splash ? { splash: { ...jet.splash } } : {}),
    };
    const extrema: WaterVec3[] = [jet.origin, impact.position];
    const apex = vy / gravity;
    if (apex > 0 && apex < flightTime)
      extrema.push([
        jet.origin[0] + jet.velocity[0] * apex,
        jet.origin[1] + vy * apex - 0.5 * gravity * apex * apex,
        jet.origin[2] + jet.velocity[2] * apex,
      ]);
    if (
      extrema.some((point) =>
        point.some(
          (n) =>
            Math.abs(n) +
              Math.max(jet.radius, endRadius) +
              WATER_JET_BREAKUP_METERS >
            WATER_JET_LIMITS.coordinate,
        ),
      )
    )
      throw new Error(
        "Keep the whole stream and its impact within the world bounds.",
      );
    if (
      jet.splash &&
      impact.position.some(
        (n, axis) =>
          Math.abs(n) +
            (axis === 1 ? jet.splash!.height : jet.splash!.radius) +
            (jet.splash!.size ?? 0.008) * 2.8 >
          WATER_JET_LIMITS.coordinate,
      )
    )
      throw new Error("Keep impact spray within the world bounds.");
    vertexCount += (segments + 1) * (sides + 1);
    indexCount += segments * sides * 6;
    if (
      vertexCount > WATER_JET_LIMITS.vertices ||
      indexCount > WATER_JET_LIMITS.indices
    )
      throw new Error("Reduce the number or detail of the water streams.");
    return { jet, gravity, segments, sides, impact, endRadius, taper };
  });
  const vertices = new Float32Array(vertexCount * 12);
  const indices = new Uint32Array(indexCount);
  const bounds: WaterJetGeometry["bounds"] = jets.length
    ? {
        min: [Infinity, Infinity, Infinity],
        max: [-Infinity, -Infinity, -Infinity],
      }
    : null;
  let vertex = 0;
  let index = 0;
  for (const {
    jet,
    gravity,
    segments,
    sides,
    impact,
    endRadius,
    taper,
  } of prepared) {
    const first = vertex;
    let normal: Vec3 | undefined;
    for (let ring = 0; ring <= segments; ring++) {
      const time = (ring / segments) * impact.flightTime;
      const progress = ring / segments;
      const radius =
        jet.radius + (endRadius - jet.radius) * Math.pow(progress, taper);
      const drdt =
        ((endRadius - jet.radius) * taper * Math.pow(progress, taper - 1)) /
        impact.flightTime;
      const center: Vec3 = [
        jet.origin[0] + jet.velocity[0] * time,
        jet.origin[1] + jet.velocity[1] * time - 0.5 * gravity * time * time,
        jet.origin[2] + jet.velocity[2] * time,
      ];
      const velocity: Vec3 = [
        jet.velocity[0],
        jet.velocity[1] - gravity * time,
        jet.velocity[2],
      ];
      const tangent = normalized(velocity);
      if (normal) {
        const parallel = dot(normal, tangent);
        const projected: Vec3 = normal.map(
          (n, axis) => n - parallel * tangent[axis],
        ) as Vec3;
        normal =
          length(projected) > 1e-8
            ? normalized(projected)
            : firstNormal(tangent);
      } else normal = firstNormal(tangent);
      const binormal = normalized(cross(tangent, normal));
      for (let side = 0; side <= sides; side++) {
        const angle = (side / sides) * Math.PI * 2;
        const radial = normal.map(
          (n, axis) => Math.cos(angle) * n + Math.sin(angle) * binormal[axis],
        ) as Vec3;
        const position = center.map(
          (n, axis) => n + radius * radial[axis],
        ) as Vec3;
        const surfaceNormal = normalized(
          radial.map(
            (n, axis) => n - (tangent[axis] * drdt) / length(velocity),
          ) as Vec3,
        );
        const uvTangent = normalized(cross(tangent, radial));
        vertices.set(
          [
            ...position,
            ...surfaceNormal,
            side / sides,
            time,
            ...uvTangent,
            progress,
          ],
          vertex * 12,
        );
        for (let axis = 0; axis < 3; axis++) {
          bounds!.min[axis] = Math.min(
            bounds!.min[axis],
            vertices[vertex * 12 + axis],
          );
          bounds!.max[axis] = Math.max(
            bounds!.max[axis],
            vertices[vertex * 12 + axis],
          );
        }
        vertex++;
      }
    }
    for (let ring = 0; ring < segments; ring++)
      for (let side = 0; side < sides; side++) {
        const a = first + ring * (sides + 1) + side,
          b = a + sides + 1;
        indices.set([a, a + 1, b, a + 1, b + 1, b], index);
        index += 6;
      }
  }
  if (bounds)
    for (let axis = 0; axis < 3; axis++) {
      bounds.min[axis] -= WATER_JET_BREAKUP_METERS;
      bounds.max[axis] += WATER_JET_BREAKUP_METERS;
    }
  return { vertices, indices, impacts: prepared.map((p) => p.impact), bounds };
}

/** Packs deterministic, GPU-animated ballistic droplets, with no asset-specific anchors. */
export function buildWaterSplashes(impacts: readonly WaterImpact[]) {
  if (!Array.isArray(impacts) || impacts.length > WATER_JET_LIMITS.jets)
    throw new Error("Use no more than 64 water impacts.");
  const vertices: number[] = [],
    indices: number[] = [];
  for (const impact of impacts as readonly WaterImpact[]) {
    const splash = impact.splash;
    if (!splash) continue;
    // Reuse the stream's finite limits before any geometry allocation.
    if (
      !finiteVector(impact.position) ||
      !Number.isFinite(splash.radius) ||
      splash.radius <= 0 ||
      splash.radius > 2 ||
      !Number.isFinite(splash.height) ||
      splash.height <= 0 ||
      splash.height > 2 ||
      !Number.isInteger(splash.droplets ?? 24) ||
      (splash.droplets ?? 24) < 1 ||
      (splash.droplets ?? 24) > 64 ||
      !Number.isFinite(splash.size ?? 0.008) ||
      (splash.size ?? 0.008) <= 0 ||
      (splash.size ?? 0.008) > 0.05 ||
      impact.position.some(
        (n, axis) =>
          Math.abs(n) +
            (axis === 1 ? splash.height : splash.radius) +
            (splash.size ?? 0.008) * 2.8 >
          WATER_JET_LIMITS.coordinate,
      )
    )
      throw new Error("Choose finite, bounded impact spray dimensions.");
    for (let i = 0; i < (splash.droplets ?? 24); i++) {
      const phase = (i * 0.61803398875) % 1;
      const angle = i * 2.39996322973;
      const height = splash.height * (0.4 + 0.6 * ((i * 0.754877666) % 1));
      const up = Math.sqrt(2 * 9.81 * height),
        lifetime = (2 * up) / 9.81;
      const speed = (splash.radius * (0.35 + 0.65 * phase)) / lifetime;
      const first = vertices.length / 12;
      for (const [x, y] of [
        [-1, -1],
        [1, -1],
        [-1, 1],
        [1, 1],
      ])
        vertices.push(
          ...impact.position,
          Math.cos(angle) * speed,
          up,
          Math.sin(angle) * speed,
          x,
          y,
          phase,
          lifetime,
          (splash.size ?? 0.008) * (0.3 + 0.7 * phase ** 3),
          9.81,
        );
      indices.push(
        first,
        first + 1,
        first + 2,
        first + 2,
        first + 1,
        first + 3,
      );
    }
  }
  return {
    vertices: new Float32Array(vertices),
    indices: new Uint32Array(indices),
  };
}
