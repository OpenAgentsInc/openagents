import type { CompiledScene, EnvironmentMap } from "../assets/src/scene.ts";
import { giMaterialIdentityValue } from "./gi-materials.ts";
import {
  halfToFloat,
  sh9Basis,
  type EnvironmentImage,
} from "./environment-lighting.ts";
import { dot, normalize, type V3 } from "./math.ts";
import {
  GI_INVALID,
  GI_LEAF_BIT,
  type GiAcceleration,
  type GiHit,
} from "./gi-acceleration.ts";

export const GI_PROBE_PROFILE = "occluded-sky-sh9-v1";
export const GI_PROBE_RECORD_FLOATS = 40;
export const GI_PROBE_MOMENT_INTERIOR = 8;
export const GI_PROBE_TILE_SIZE = 10;
export interface GiProbeCoefficientBlob {
  url: string;
  sha256: string;
  bytes: number;
}

export interface GiProbeManifest {
  schema: "aethyrforge.private-sky-probes.v1";
  profile: "occluded-sky-sh9-v1";
  /** SHA-256 of giSceneIdentityPayload; auxiliary links do not change geometry identity. */
  sourceGeometrySha256: string;
  /** Optional one-bounce field bound to this exact published probe field. */
  bounceURL?: string;
  grid: {
    origin: [number, number, number];
    spacing: [number, number, number];
    dimensions: [number, number, number];
    count: number;
  };
  records: { strideFloats: 40; positionValidOffset: 0; shOffset: 4 };
  moments: {
    url: string;
    sha256: string;
    bytes: number;
    format: "rgba16float";
    width: number;
    height: number;
    interior: 8;
    tileSize: 10;
    tilesPerRow: number;
    rayDistanceMeters: number;
    distanceConvention: "mean-meters,mean-square-meters-squared";
  };
  scenarios: Partial<
    Record<
      "day" | "night",
      {
        url: string;
        sha256: string;
        bytes: number;
        sourceEnvironmentSha256: string;
        sourceGeometrySha256?: string;
        moments?: GiProbeManifest["moments"];
        /** Separate source constant-fog SH9 vec4 coefficients, count * 144 bytes. */
        constantFog?: GiProbeCoefficientBlob;
      }
    >
  >;
  /** Incoming sky only, Lambert-convolved E/pi, world-space SH9 RGB padded vec4. */
  lighting: {
    convention: "world-sh9-lambert-E-over-pi";
    gainBaked: false;
    skyBounceCount: 0;
    rayCount: number;
    alphaPolicy: "masked-solid-preview" | "alpha-tested-material-v1";
    lowerHemisphere: "world-up-black" | "input-map";
  };
}

/** Stable geometry/material content, independent of JSON formatting and extra artifact links. */
export function giSceneIdentityPayload(
  scene: Pick<
    CompiledScene,
    | "vertices"
    | "indices"
    | "vertexCount"
    | "indexCount"
    | "vertexStride"
    | "materials"
    | "meshes"
    | "instances"
  >,
): string {
  return JSON.stringify(
    canonical({
      vertices: scene.vertices,
      indices: scene.indices,
      vertexCount: scene.vertexCount,
      indexCount: scene.indexCount,
      vertexStride: scene.vertexStride,
      // Prepared diffuse transport omits normal-map microgeometry. The runtime
      // channel convention changes neither triangle coverage nor diffuse albedo.
      // Keep all other (including unknown) material fields in this identity.
      materials: scene.materials.map(giMaterialIdentityValue),
      meshes: scene.meshes,
      instances: scene.instances,
    }),
  );
}

function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value)
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([key, item]) => [key, canonical(item)]),
    );
  return value;
}
/** Canonical map metadata,including its content-addressed image URL; gains remain runtime values. */
export function giEnvironmentIdentityPayload(
  environment: EnvironmentMap,
): string {
  return JSON.stringify(canonical(environment));
}

export type GiProbeTrace = (
  origin: V3,
  direction: V3,
  minimum: number,
  maximum: number,
) =>
  | Pick<GiHit, "distance" | "normal" | "frontFace" | "maskedApproximation">
  | undefined;

/** Allocation-light,nonreentrant offline query for the same packed hierarchy contract. */
export function createGiProbeTrace(a: GiAcceleration): GiProbeTrace {
  const bf = new Float32Array(a.blasNodes),
    bu = new Uint32Array(a.blasNodes),
    tf = new Float32Array(a.tlasNodes),
    tu = new Uint32Array(a.tlasNodes),
    f = new Float32Array(a.instances),
    u = new Uint32Array(a.instances),
    positions = a.positions,
    triangles = a.triangles;
  const outer = new Uint32Array(64),
    inner = new Uint32Array(64);
  function box(
    data: Float32Array,
    node: number,
    ox: number,
    oy: number,
    oz: number,
    dx: number,
    dy: number,
    dz: number,
    min: number,
    max: number,
  ): boolean {
    const o = node * 8;
    if (dx === 0) {
      if (ox < data[o] || ox > data[o + 4]) return false;
    } else {
      let lo = (data[o] - ox) / dx,
        hi = (data[o + 4] - ox) / dx;
      if (lo > hi) {
        const t = lo;
        lo = hi;
        hi = t;
      }
      min = Math.max(min, lo);
      max = Math.min(max, hi);
      if (min > max) return false;
    }
    if (dy === 0) {
      if (oy < data[o + 1] || oy > data[o + 5]) return false;
    } else {
      let lo = (data[o + 1] - oy) / dy,
        hi = (data[o + 5] - oy) / dy;
      if (lo > hi) {
        const t = lo;
        lo = hi;
        hi = t;
      }
      min = Math.max(min, lo);
      max = Math.min(max, hi);
      if (min > max) return false;
    }
    if (dz === 0) {
      if (oz < data[o + 2] || oz > data[o + 6]) return false;
    } else {
      let lo = (data[o + 2] - oz) / dz,
        hi = (data[o + 6] - oz) / dz;
      if (lo > hi) {
        const t = lo;
        lo = hi;
        hi = t;
      }
      min = Math.max(min, lo);
      max = Math.min(max, hi);
      if (min > max) return false;
    }
    return true;
  }
  return (origin, direction, minimum, maximum) => {
    if (a.tlasRoot === GI_INVALID) return;
    let nearest = maximum,
      bestInstance = -1,
      bestTriangle = -1,
      bestMaterial = 0;
    const [wx, wy, wz] = origin,
      [vx, vy, vz] = direction;
    let top = 1;
    outer[0] = a.tlasRoot;
    while (top) {
      const node = outer[--top];
      if (!box(tf, node, wx, wy, wz, vx, vy, vz, minimum, nearest)) continue;
      const first = tu[node * 8 + 3],
        meta = tu[node * 8 + 7];
      if (!(meta & GI_LEAF_BIT)) {
        outer[top++] = meta;
        outer[top++] = first;
        continue;
      }
      for (
        let instance = first;
        instance < first + (meta & ~GI_LEAF_BIT);
        instance++
      ) {
        const offset = instance * 32,
          q = offset + 12;
        const ox = f[q] * wx + f[q + 1] * wy + f[q + 2] * wz + f[q + 3],
          oy = f[q + 4] * wx + f[q + 5] * wy + f[q + 6] * wz + f[q + 7],
          oz = f[q + 8] * wx + f[q + 9] * wy + f[q + 10] * wz + f[q + 11];
        const dx = f[q] * vx + f[q + 1] * vy + f[q + 2] * vz,
          dy = f[q + 4] * vx + f[q + 5] * vy + f[q + 6] * vz,
          dz = f[q + 8] * vx + f[q + 9] * vy + f[q + 10] * vz;
        let depth = 1;
        inner[0] = u[offset + 24];
        while (depth) {
          const b = inner[--depth];
          if (!box(bf, b, ox, oy, oz, dx, dy, dz, minimum, nearest)) continue;
          const start = bu[b * 8 + 3],
            flags = bu[b * 8 + 7];
          if (!(flags & GI_LEAF_BIT)) {
            inner[depth++] = flags;
            inner[depth++] = start;
            continue;
          }
          for (
            let triangle = start;
            triangle < start + (flags & ~GI_LEAF_BIT);
            triangle++
          ) {
            const t = triangle * 8,
              slot = triangles[t + 4],
              override =
                slot < u[offset + 27]
                  ? a.materialOverrides[u[offset + 26] + slot]
                  : GI_INVALID,
              material = override === GI_INVALID ? triangles[t + 3] : override;
            if (!a.materialTransport[material]) continue;
            const ia = triangles[t] * 4,
              ib = triangles[t + 1] * 4,
              ic = triangles[t + 2] * 4;
            const ax = positions[ia],
              ay = positions[ia + 1],
              az = positions[ia + 2],
              e1x = positions[ib] - ax,
              e1y = positions[ib + 1] - ay,
              e1z = positions[ib + 2] - az,
              e2x = positions[ic] - ax,
              e2y = positions[ic + 1] - ay,
              e2z = positions[ic + 2] - az;
            const px = dy * e2z - dz * e2y,
              py = dz * e2x - dx * e2z,
              pz = dx * e2y - dy * e2x,
              det = e1x * px + e1y * py + e1z * pz;
            const determinantScale =
              (e1x * e1x + e1y * e1y + e1z * e1z) *
              (e2x * e2x + e2y * e2y + e2z * e2z) *
              (dx * dx + dy * dy + dz * dz);
            if (det * det <= 1e-24 * determinantScale) continue;
            const tx = ox - ax,
              ty = oy - ay,
              tz = oz - az,
              inv = 1 / det,
              baryU = (tx * px + ty * py + tz * pz) * inv;
            if (baryU < 0 || baryU > 1) continue;
            const qx = ty * e1z - tz * e1y,
              qy = tz * e1x - tx * e1z,
              qz = tx * e1y - ty * e1x,
              baryV = (dx * qx + dy * qy + dz * qz) * inv;
            if (baryV < 0 || baryU + baryV > 1) continue;
            const distance = (e2x * qx + e2y * qy + e2z * qz) * inv;
            if (distance < minimum || distance > nearest) continue;
            nearest = distance;
            bestInstance = instance;
            bestTriangle = triangle;
            bestMaterial = material;
          }
        }
      }
    }
    if (bestInstance < 0) return;
    const t = bestTriangle * 8,
      offset = bestInstance * 32,
      ia = triangles[t] * 4,
      ib = triangles[t + 1] * 4,
      ic = triangles[t + 2] * 4;
    const ex = positions[ib] - positions[ia],
      ey = positions[ib + 1] - positions[ia + 1],
      ez = positions[ib + 2] - positions[ia + 2],
      gx = positions[ic] - positions[ia],
      gy = positions[ic + 1] - positions[ia + 1],
      gz = positions[ic + 2] - positions[ia + 2];
    const e: [number, number, number] = [
      f[offset] * ex + f[offset + 1] * ey + f[offset + 2] * ez,
      f[offset + 4] * ex + f[offset + 5] * ey + f[offset + 6] * ez,
      f[offset + 8] * ex + f[offset + 9] * ey + f[offset + 10] * ez,
    ];
    const g: [number, number, number] = [
      f[offset] * gx + f[offset + 1] * gy + f[offset + 2] * gz,
      f[offset + 4] * gx + f[offset + 5] * gy + f[offset + 6] * gz,
      f[offset + 8] * gx + f[offset + 9] * gy + f[offset + 10] * gz,
    ];
    const normal = normalize([
      e[1] * g[2] - e[2] * g[1],
      e[2] * g[0] - e[0] * g[2],
      e[0] * g[1] - e[1] * g[0],
    ]);
    return {
      distance: nearest,
      normal,
      frontFace: dot(normal, direction) < 0,
      maskedApproximation: a.materialTransport[bestMaterial] === 2,
    };
  };
}
export interface GiProbeBake {
  records: Record<string, Float32Array>;
  moments: Float32Array;
  width: number;
  height: number;
  tilesPerRow: number;
  active: number;
  relocated: number;
  traceCount: number;
}

export function giOctEncode(direction: V3): [number, number] {
  const sum =
    Math.abs(direction[0]) + Math.abs(direction[1]) + Math.abs(direction[2]);
  if (!Number.isFinite(sum) || sum < 1e-12)
    throw new Error("Use a finite,nonzero probe direction.");
  let x = direction[0] / sum,
    y = direction[1] / sum;
  if (direction[2] < 0)
    [x, y] = [
      (1 - Math.abs(y)) * (x >= 0 ? 1 : -1),
      (1 - Math.abs(x)) * (y >= 0 ? 1 : -1),
    ];
  return [x * 0.5 + 0.5, y * 0.5 + 0.5];
}
export function giOctDecode(u: number, v: number): V3 {
  let x = u * 2 - 1,
    y = v * 2 - 1;
  const z = 1 - Math.abs(x) - Math.abs(y);
  if (z < 0)
    [x, y] = [
      (1 - Math.abs(y)) * (x >= 0 ? 1 : -1),
      (1 - Math.abs(x)) * (y >= 0 ? 1 : -1),
    ];
  return normalize([x, y, z]);
}
export function giMomentVisibility(
  mean: number,
  meanSquare: number,
  distance: number,
): number {
  if (distance <= mean) return 1;
  const variance = Math.max(meanSquare - mean * mean, 0.0001),
    delta = distance - mean;
  return (variance / (variance + delta * delta)) ** 3;
}

/** Prepared raw panorama radiance in world directions; source tint/brightness are not baked. */
export function giEnvironmentSampler(
  image: EnvironmentImage,
  worldToEnvironment: ArrayLike<number>,
): (direction: V3) => V3 {
  if (
    image.width < 1 ||
    image.width > 4096 ||
    image.height < 1 ||
    image.height > 2048 ||
    image.pixels.length !== image.width * image.height * 4 ||
    worldToEnvironment.length !== 16 ||
    !Array.from(worldToEnvironment).every(Number.isFinite)
  )
    throw new Error(
      "Use bounded panorama data and a finite lighting transform.",
    );
  const half =
    image.pixels instanceof Uint16Array
      ? Float32Array.from({ length: 65536 }, (_, i) => halfToFloat(i))
      : undefined;
  const channel = (pixel: number, c: number) =>
    half ? half[image.pixels[pixel * 4 + c]] : image.pixels[pixel * 4 + c];
  return (direction) => {
    const d = normalize(
      [0, 1, 2].map(
        (r) =>
          worldToEnvironment[r] * direction[0] +
          worldToEnvironment[4 + r] * direction[1] +
          worldToEnvironment[8 + r] * direction[2],
      ) as V3,
    );
    const u = Math.atan2(d[0], d[2]) / (2 * Math.PI),
      v = Math.acos(Math.max(-1, Math.min(1, d[1]))) / Math.PI;
    const x = (u - Math.floor(u)) * image.width - 0.5,
      y = Math.max(0, Math.min(image.height - 1, v * image.height - 0.5)),
      xBase = Math.floor(x),
      y0 = Math.floor(y),
      x0 = (xBase + image.width) % image.width,
      x1 = (x0 + 1) % image.width,
      y1 = Math.min(image.height - 1, y0 + 1),
      fx = x - xBase,
      fy = y - y0;
    return [0, 1, 2].map((c) => {
      const a = channel(y0 * image.width + x0, c),
        b = channel(y0 * image.width + x1, c),
        e = channel(y1 * image.width + x0, c),
        f = channel(y1 * image.width + x1, c);
      return (a + (b - a) * fx) * (1 - fy) + (e + (f - e) * fx) * fy;
    }) as V3;
  };
}

/** Pure bounded sky-visibility bake; trace geometry once for all environment samplers. */
export function bakeSkyProbes(
  trace: GiProbeTrace,
  samplers: Record<string, (direction: V3) => V3>,
  grid: GiProbeManifest["grid"],
  options: {
    rayCount?: number;
    rayDistanceMeters?: number;
    progress?: (completed: number, total: number) => void;
  } = {},
): GiProbeBake {
  const rays = options.rayCount ?? 128,
    reach = options.rayDistanceMeters ?? 64,
    count = grid.dimensions.reduce((a, b) => a * b, 1);
  if (
    grid.origin.length !== 3 ||
    grid.spacing.length !== 3 ||
    grid.dimensions.length !== 3 ||
    ![...grid.origin, ...grid.spacing].every(Number.isFinite) ||
    grid.spacing.some((v) => v <= 0 || v > 32) ||
    grid.dimensions.some((v) => !Number.isInteger(v) || v < 2 || v > 64) ||
    count !== grid.count ||
    count > 4096 ||
    !Number.isInteger(rays) ||
    rays < 32 ||
    rays > 256 ||
    !Number.isFinite(reach) ||
    reach < 1 ||
    reach > 128
  )
    throw new Error("Use a bounded probe grid and32to256rays.");
  const names = Object.keys(samplers);
  if (!names.length || names.length > 2)
    throw new Error("Prepare one or two source sky scenarios.");
  const tilesPerRow = Math.ceil(Math.sqrt(count)),
    width = tilesPerRow * 10,
    height = Math.ceil(count / tilesPerRow) * 10,
    moments = new Float32Array(width * height * 4),
    records = Object.fromEntries(
      names.map((name) => [name, new Float32Array(count * 40)]),
    );
  let active = 0,
    relocated = 0,
    traceCount = 0;
  const directions: V3[] = Array.from({ length: rays }, (_, i) => {
    const y = 1 - (2 * (i + 0.5)) / rays,
      phi = i * 2.399963229728653,
      r = Math.sqrt(Math.max(0, 1 - y * y));
    return [r * Math.cos(phi), y, r * Math.sin(phi)];
  });
  const kernels = Array.from({ length: 64 }, (_, texel) => {
    const direction = giOctDecode(
      ((texel % 8) + 0.5) / 8,
      (Math.floor(texel / 8) + 0.5) / 8,
    );
    return directions.map((ray) => Math.max(dot(direction, ray), 0) ** 50);
  });
  const basis = directions.map(sh9Basis),
    bands = [1, 2 / 3, 2 / 3, 2 / 3, 0.25, 0.25, 0.25, 0.25, 0.25];
  for (let index = 0; index < count; index++) {
    const x = index % grid.dimensions[0],
      y = Math.floor(index / grid.dimensions[0]) % grid.dimensions[1],
      z = Math.floor(index / (grid.dimensions[0] * grid.dimensions[1]));
    let position = grid.origin.map(
      (v, i) => v + [x, y, z][i] * grid.spacing[i],
    ) as V3;
    const run = (p: V3) =>
      directions.map((direction) => {
        traceCount++;
        return trace(p, direction, 0.01, reach);
      });
    let hits = run(position);
    let backfaces = hits.filter(
      (hit) => hit && !hit.frontFace && !hit.maskedApproximation,
    ).length;
    const maxRelocation = Math.min(...grid.spacing) * 0.45;
    if (backfaces > rays * 0.25) {
      let nearest = -1;
      for (let i = 0; i < hits.length; i++)
        if (
          hits[i] &&
          !hits[i]!.frontFace &&
          !hits[i]!.maskedApproximation &&
          (nearest < 0 || hits[i]!.distance < hits[nearest]!.distance)
        )
          nearest = i;
      if (nearest >= 0 && hits[nearest]!.distance + 0.08 <= maxRelocation) {
        const step = hits[nearest]!.distance + 0.08;
        position = position.map(
          (v, i) => v + directions[nearest][i] * step,
        ) as V3;
        hits = run(position);
        backfaces = hits.filter(
          (hit) => hit && !hit.frontFace && !hit.maskedApproximation,
        ).length;
        relocated++;
      }
    }
    const valid = backfaces <= rays * 0.25;
    if (valid) active++;
    for (const name of names) {
      const record = records[name];
      record.set([...position, valid ? 1 : 0], index * 40);
      if (valid)
        for (let r = 0; r < rays; r++)
          if (!hits[r]) {
            const radiance = samplers[name](directions[r]);
            if (
              radiance.length !== 3 ||
              !radiance.every((v) => Number.isFinite(v) && v >= 0 && v <= 65504)
            )
              throw new Error(
                "Probe sky radiance must be finite and nonnegative.",
              );
            for (let coefficient = 0; coefficient < 9; coefficient++)
              for (let c = 0; c < 3; c++)
                record[index * 40 + 4 + coefficient * 4 + c] +=
                  (radiance[c] *
                    basis[r][coefficient] *
                    bands[coefficient] *
                    4 *
                    Math.PI) /
                  rays;
          }
    }
    const interior = new Float32Array(64 * 4);
    for (let texel = 0; texel < 64; texel++) {
      let weight = 0,
        mean = 0,
        square = 0;
      for (let r = 0; r < rays; r++) {
        const w = kernels[texel][r],
          distance = Math.min(reach, hits[r]?.distance ?? reach);
        weight += w;
        mean += w * distance;
        square += w * distance * distance;
      }
      interior.set(
        [
          mean / Math.max(weight, 1e-20),
          square / Math.max(weight, 1e-20),
          0,
          1,
        ],
        texel * 4,
      );
    }
    const tileX = index % tilesPerRow,
      tileY = Math.floor(index / tilesPerRow);
    for (let ty = 0; ty < 10; ty++)
      for (let tx = 0; tx < 10; tx++) {
        let sx = tx - 1,
          sy = ty - 1;
        if ((tx === 0 || tx === 9) && (ty === 0 || ty === 9)) {
          sx = tx === 0 ? 7 : 0;
          sy = ty === 0 ? 7 : 0;
        } else if (tx === 0 || tx === 9) {
          sx = tx === 0 ? 0 : 7;
          sy = 8 - ty;
        } else if (ty === 0 || ty === 9) {
          sx = 8 - tx;
          sy = ty === 0 ? 0 : 7;
        }
        moments.set(
          interior.subarray((sy * 8 + sx) * 4, (sy * 8 + sx) * 4 + 4),
          ((tileY * 10 + ty) * width + tileX * 10 + tx) * 4,
        );
      }
    if (index % 32 === 31 || index === count - 1)
      options.progress?.(index + 1, count);
  }
  return {
    records,
    moments,
    width,
    height,
    tilesPerRow,
    active,
    relocated,
    traceCount,
  };
}
