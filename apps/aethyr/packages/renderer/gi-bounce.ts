import type { EnvironmentMap, SourceLight } from "../assets/src/scene.ts";
import type { GiProbeManifest } from "./gi-probes.ts";
import { sh9Basis } from "./environment-lighting.ts";
import {
  evaluateGiMaterial,
  GI_MATERIAL_PROFILE,
  giMaterialIdentityValue,
  type GiTextureSampler,
} from "./gi-materials.ts";
import { lightAttenuation, lightColor, lightIntensity } from "./lighting.ts";
import { cross, dot, normalize, type V3 } from "./math.ts";
import type { GiSurfaceTransport } from "./gi-transport.ts";
import type { GiBounds } from "./gi-acceleration.ts";
import type { CompiledMaterial } from "../assets/src/scene.ts";

export const GI_BOUNCE_PROFILE = "alpha-lambert-one-bounce-v1";
export const GI_BOUNCE_LAYERS = ["sky", "sun", "locals"] as const;
export type GiBounceLayer = (typeof GI_BOUNCE_LAYERS)[number];
export interface GiBounceBlob {
  url: string;
  sha256: string;
  bytes: number;
}
export interface GiBounceManifest {
  schema: "aethyrforge.private-bounce-probes.v1";
  profile: "alpha-lambert-one-bounce-v1";
  /** Every layer refers to the exact alpha-aware base probe positions/moments. */
  base: {
    identitySha256: string;
    scenarios: Partial<
      Record<"day" | "night", { recordsSha256: string; momentsSha256: string }>
    >;
  };
  grid: GiProbeManifest["grid"];
  layout: {
    order: "layer-major-sh9-rgb-vec4";
    layers: ["sky", "sun", "locals"];
    coefficientStrideFloats: 4;
    coefficientsPerProbe: 9;
    bytesPerProbePerLayer: 144;
  };
  scenarios: Partial<
    Record<
      "day" | "night",
      {
        sourceGeometrySha256: string;
        sourceCoverageSha256: string;
        sourceMaterialSha256: string;
        sourceEnvironmentSha256: string;
        sourceSunDirection: [number, number, number] | null;
        sourceLocalLightsSha256: string;
        layers: Record<GiBounceLayer, GiBounceBlob>;
        /** Constant captured fog's one-bounce response; never scaled by sunlight. */
        constantFog?: GiBounceBlob;
      }
    >
  >;
  lighting: {
    convention: "world-sh9-lambert-E-over-pi";
    diffuseBounceCount: 1;
    gainBaked: false;
    localSourceStrengthBaked: true;
    sun: "white-per-unit-lux";
    alphaPolicy: "alpha-tested-material-v1";
    materialProfile: string;
    emissiveGi: false;
  };
  limitations: string[];
}

/** Input maps may come from a source panorama or a separately computed atmosphere. */
export interface GiBounceEnvironment {
  description: EnvironmentMap;
  sample: (direction: [number, number, number]) => [number, number, number];
  constantFogSample?: (
    direction: [number, number, number],
  ) => [number, number, number];
}
export interface GiBounceLights {
  sun: SourceLight | null;
  locals: readonly SourceLight[];
}

/** Shared descriptor payloads are recomputable by worker admission without compiler files. */
export function giBounceCanonicalPayload(value: unknown): string {
  const sort = (v: unknown): unknown =>
    Array.isArray(v)
      ? v.map(sort)
      : v && typeof v === "object"
        ? Object.fromEntries(
            Object.entries(v)
              .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
              .map(([key, item]) => [key, sort(item)]),
          )
        : v;
  return JSON.stringify(sort(value));
}
export function giBounceLocalLightsIdentityPayload(
  lights: readonly SourceLight[],
): string {
  return giBounceCanonicalPayload(
    lights.filter((l) => l.enabled && l.kind !== "directional"),
  );
}
export function giBounceMaterialIdentityPayload(
  scene: Pick<
    import("../assets/src/scene.ts").CompiledScene,
    "materials" | "vertexColors" | "vertexUV1"
  >,
): string {
  return giBounceCanonicalPayload({
    profile: GI_MATERIAL_PROFILE,
    materials: scene.materials.map(giMaterialIdentityValue),
    vertexColors: scene.vertexColors,
    uv1: scene.vertexUV1,
  });
}
export function giBounceCoverageIdentityPayload(
  scene: Pick<
    import("../assets/src/scene.ts").CompiledScene,
    "materials" | "vertices" | "vertexColors"
  >,
): string {
  return giBounceCanonicalPayload({
    vertices: scene.vertices,
    vertexColors: scene.materials.some((m) => m.vertexOpacity)
      ? scene.vertexColors
      : undefined,
    materials: scene.materials.map((material) => ({
      name: material.name,
      alphaMode: material.alphaMode,
      alphaCutoff: material.alphaCutoff,
      uvScale: material.uvScale,
      vertexOpacity: material.vertexOpacity,
      baseAlpha: material.baseColorFactor[3],
      url: material.vegetation?.baseColorUrl ?? material.baseColorUrl,
      vegetationCutoff: material.vegetation?.alphaCutoff,
      family: [
        material.layers?.model,
        material.paint?.model,
        material.terrain?.model,
        material.vegetation?.model,
      ],
    })),
  });
}

export function giBounceBaseIdentity(manifest: GiProbeManifest): string {
  return JSON.stringify({
    grid: manifest.grid,
    scenarios: Object.fromEntries(
      Object.entries(manifest.scenarios)
        .sort(([a], [b]) => (a < b ? -1 : 1))
        .map(([mode, value]) => [
          mode,
          {
            recordsSha256: value!.sha256,
            momentsSha256: (value!.moments ?? manifest.moments).sha256,
            ...(value!.constantFog
              ? { constantFogSha256: value!.constantFog.sha256 }
              : {}),
          },
        ]),
    ),
  });
}

/** Conservative world cells reduce local-light work; exhaustion uses the complete list. */
function localIndex(lights: readonly SourceLight[]) {
  if (lights.length > 4096)
    throw new Error("The bounce light set exceeds4096sources.");
  const active = lights.filter((l) => l.enabled && l.kind !== "directional");
  const cells = new Map<string, number[]>();
  let memberships = 0,
    fallback = false;
  for (const [index, light] of active.entries()) {
    if (
      light.position.length !== 3 ||
      !light.position.every((v) => Number.isFinite(v) && Math.abs(v) <= 1e6) ||
      !Number.isFinite(light.range ?? 10) ||
      (light.range ?? 10) <= 0 ||
      (light.range ?? 10) > 1e6 ||
      light.color.length !== 3 ||
      !light.color.every((v) => Number.isFinite(v) && v >= 0) ||
      (light.kind !== "point" && light.kind !== "spot") ||
      (light.kind === "spot" &&
        (light.direction.length !== 3 ||
          !light.direction.every(Number.isFinite) ||
          Math.hypot(...light.direction) < 1e-12 ||
          !Number.isFinite(light.innerConeAngleRadians ?? 0) ||
          !Number.isFinite(light.outerConeAngleRadians ?? Math.PI / 4) ||
          (light.innerConeAngleRadians ?? 0) < 0 ||
          (light.outerConeAngleRadians ?? Math.PI / 4) <= 0 ||
          (light.outerConeAngleRadians ?? Math.PI / 4) > Math.PI ||
          (light.innerConeAngleRadians ?? 0) >
            (light.outerConeAngleRadians ?? Math.PI / 4)))
    )
      throw new Error("Use finite bounded local-light volumes.");
    const range = light.range ?? 10,
      lo = light.position.map((v) => Math.floor((v - range) / 16)),
      hi = light.position.map((v) => Math.floor((v + range) / 16));
    memberships +=
      (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1);
    if (memberships > 1_000_000) {
      fallback = true;
      cells.clear();
      break;
    }
    for (let z = lo[2]; z <= hi[2]; z++)
      for (let y = lo[1]; y <= hi[1]; y++)
        for (let x = lo[0]; x <= hi[0]; x++) {
          const key = `${x},${y},${z}`,
            list = cells.get(key);
          if (list) list.push(index);
          else cells.set(key, [index]);
        }
  }
  const all = active.map((_, i) => i),
    strength = active.map(lightIntensity),
    colors = active.map(lightColor),
    directions = active.map((l) =>
      l.kind === "spot" ? normalize(l.direction as V3) : ([0, 0, 0] as V3),
    );
  return {
    active,
    strength,
    colors,
    directions,
    fallback,
    at: (p: V3) =>
      fallback
        ? all
        : (cells.get(p.map((v) => Math.floor(v / 16)).join(",")) ?? []),
  };
}

/** Exactly one diffuse surface interaction; the direct sky field remains separate. */
export function bakeDiffuseBounce(input: {
  transport: GiSurfaceTransport;
  materials: readonly CompiledMaterial[];
  sample: GiTextureSampler;
  baseRecords: Float32Array;
  environment: GiBounceEnvironment;
  lights: GiBounceLights;
  bounds?: GiBounds;
  layers?: readonly GiBounceLayer[];
  /** Allows rebuilding the optional constant layer without changing the legacy three. */
  bakeConstantFog?: boolean;
  rayCount?: number;
  skyRayCount?: number;
  rayDistanceMeters?: number;
  progress?: (completed: number, total: number) => void;
}) {
  const selected = new Set(input.layers ?? GI_BOUNCE_LAYERS);
  if ([...selected].some((v) => !GI_BOUNCE_LAYERS.includes(v)))
    throw new Error("Use a supported bounce layer.");
  const rays = input.rayCount ?? 128,
    skyRays = input.skyRayCount ?? 4,
    reach = input.rayDistanceMeters ?? 64,
    count = input.baseRecords.length / 40;
  if (
    !Number.isInteger(count) ||
    count < 1 ||
    count > 4096 ||
    !input.baseRecords.every(Number.isFinite) ||
    !Number.isInteger(rays) ||
    rays < 32 ||
    rays > 256 ||
    !Number.isInteger(skyRays) ||
    skyRays < 1 ||
    skyRays > 16 ||
    !Number.isFinite(reach) ||
    reach < 1 ||
    reach > 128
  )
    throw new Error("Use bounded bounce records and sample counts.");
  const layers = Object.fromEntries(
    GI_BOUNCE_LAYERS.map((k) => [k, new Float32Array(count * 36)]),
  ) as Record<GiBounceLayer, Float32Array>;
  const constantFog =
    (input.bakeConstantFog ?? !!input.environment.constantFogSample)
      ? new Float32Array(count * 36)
      : undefined;
  if (constantFog && !input.environment.constantFogSample)
    throw new Error(
      "Constant fog bounce needs its separate environment sampler.",
    );
  const stats = {
    activeProbes: 0,
    primaryHits: 0,
    materialEvaluations: 0,
    skyShadowRays: 0,
    sunShadowRays: 0,
    localShadowRays: 0,
    localEvaluations: 0,
    localIndexFallback: false,
  };
  const locals = localIndex(input.lights.locals);
  stats.localIndexFallback = locals.fallback;
  const sun = input.lights.sun?.enabled ? input.lights.sun : null;
  if (
    sun &&
    (sun.direction.length !== 3 ||
      !sun.direction.every(Number.isFinite) ||
      Math.hypot(...sun.direction) < 1e-12)
  )
    throw new Error("Use a finite nonzero sun direction.");
  const sunDirection = sun
    ? normalize(sun.direction.map((v) => -v) as V3)
    : undefined;
  const sunReach = input.bounds
    ? Math.hypot(...input.bounds.max.map((v, i) => v - input.bounds!.min[i])) +
      reach
    : reach;
  if (!Number.isFinite(sunReach) || sunReach > 1e6)
    throw new Error("The GI sunlight shadow bound is invalid.");
  const bands = [1, 2 / 3, 2 / 3, 2 / 3, 0.25, 0.25, 0.25, 0.25, 0.25];
  const directions: V3[] = Array.from({ length: rays }, (_, r) => {
    const y = 1 - (2 * (r + 0.5)) / rays,
      p = r * 2.399963229728653,
      s = Math.sqrt(1 - y * y);
    return [s * Math.cos(p), y, s * Math.sin(p)];
  });
  const basis = directions.map(sh9Basis),
    epsilon = 0.01;
  const project = (
    target: Float32Array,
    probe: number,
    ray: number,
    radiance: V3,
  ) => {
    if (!radiance.every((v) => Number.isFinite(v) && v >= 0))
      throw new Error("Bounce radiance must be finite and nonnegative.");
    for (let coefficient = 0; coefficient < 9; coefficient++)
      for (let c = 0; c < 3; c++)
        target[probe * 36 + coefficient * 4 + c] +=
          (radiance[c] *
            basis[ray][coefficient] *
            bands[coefficient] *
            4 *
            Math.PI) /
          rays;
  };
  const accumulateEnvironment = (
    sample: GiBounceEnvironment["sample"],
    direction: V3,
    rho: V3,
    target: V3,
  ) => {
    const value = sample(direction);
    if (
      value.length !== 3 ||
      !value.every((v) => Number.isFinite(v) && v >= 0 && v <= 65504)
    )
      throw new Error("Use bounded raw environment radiance.");
    for (let c = 0; c < 3; c++) target[c] += (rho[c] * value[c]) / skyRays;
  };
  for (let p = 0; p < count; p++) {
    if (![0, 1].includes(input.baseRecords[p * 40 + 3]))
      throw new Error("Use explicit zero or one probe validity.");
    if (input.baseRecords[p * 40 + 3] === 0) continue;
    stats.activeProbes++;
    const origin = Array.from(
      input.baseRecords.subarray(p * 40, p * 40 + 3),
    ) as V3;
    for (let r = 0; r < rays; r++) {
      const hit = input.transport.trace(origin, directions[r], 0.01, reach);
      if (!hit) continue;
      stats.primaryHits++;
      const material = evaluateGiMaterial(
        input.materials[hit.material],
        hit.context,
        input.sample,
      );
      stats.materialEvaluations++;
      const rho = material.reflectance;
      if (Math.max(...rho) <= 0) continue;
      const n = hit.context.geometricNormal,
        offset = hit.position.map((v, i) => v + n[i] * epsilon) as V3;
      const axis: V3 = Math.abs(n[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0],
        tangent = normalize(cross(axis, n)),
        bitangent = cross(n, tangent);
      const sky: V3 = [0, 0, 0],
        constant: V3 = [0, 0, 0];
      for (
        let s = 0;
        (selected.has("sky") || constantFog) && s < skyRays;
        s++
      ) {
        const u = (s + 0.5) / skyRays,
          phase =
            ((p * 0.754877666 + r * 0.569840291 + s * 0.618033989) % 1) *
            2 *
            Math.PI,
          rad = Math.sqrt(u),
          y = Math.sqrt(1 - u);
        const direction = normalize(
          n.map(
            (v, i) =>
              v * y +
              tangent[i] * rad * Math.cos(phase) +
              bitangent[i] * rad * Math.sin(phase),
          ) as V3,
        );
        stats.skyShadowRays++;
        if (input.transport.occluded(offset, direction, 0.001, reach)) continue;
        if (selected.has("sky"))
          accumulateEnvironment(input.environment.sample, direction, rho, sky);
        if (constantFog)
          accumulateEnvironment(
            input.environment.constantFogSample!,
            direction,
            rho,
            constant,
          );
      }
      project(layers.sky, p, r, sky);
      if (constantFog) project(constantFog, p, r, constant);
      if (selected.has("sun") && sunDirection && sun) {
        const nl = Math.max(dot(n, sunDirection), 0);
        let visible = nl > 0;
        if (visible && sun.castShadows) {
          stats.sunShadowRays++;
          visible = !input.transport.occluded(
            offset,
            sunDirection,
            0.001,
            sunReach,
          );
        }
        if (visible)
          project(layers.sun, p, r, rho.map((v) => (v * nl) / Math.PI) as V3);
      }
      const local: V3 = [0, 0, 0];
      for (const index of selected.has("locals")
        ? locals.at(hit.position)
        : []) {
        const light = locals.active[index],
          delta = light.position.map((v, i) => v - hit.position[i]) as V3,
          distance = Math.hypot(...delta),
          range = light.range ?? 10;
        if (distance >= range || distance < 0.000001) continue;
        stats.localEvaluations++;
        const direction = delta.map((v) => v / distance) as V3,
          nl = Math.max(dot(n, direction), 0);
        if (nl <= 0) continue;
        let attenuation = lightAttenuation(light, distance);
        if (light.kind === "spot") {
          const inner = Math.cos(light.innerConeAngleRadians ?? 0),
            outer = Math.cos(light.outerConeAngleRadians ?? Math.PI / 4),
            angle = -dot(direction, locals.directions[index]);
          attenuation *=
            inner - outer <= 0.00001
              ? Number(angle >= outer)
              : Math.max(0, Math.min(1, (angle - outer) / (inner - outer))) **
                2;
        }
        if (attenuation <= 0) continue;
        if (light.castShadows) {
          stats.localShadowRays++;
          const toLight = light.position.map((v, i) => v - offset[i]) as V3,
            length = Math.hypot(...toLight);
          if (
            length > 0.002 &&
            input.transport.occluded(
              offset,
              toLight.map((v) => v / length) as V3,
              0.001,
              length - 0.001,
            )
          )
            continue;
        }
        for (let c = 0; c < 3; c++)
          local[c] +=
            (rho[c] *
              locals.colors[index][c] *
              locals.strength[index] *
              attenuation *
              nl) /
            Math.PI;
      }
      project(layers.locals, p, r, local);
    }
    if (p % 16 === 15 || p === count - 1) input.progress?.(p + 1, count);
  }
  for (const values of Object.values(layers))
    if (!values.every(Number.isFinite))
      throw new Error("Bounce coefficients exceed float32 storage.");
  if (constantFog && !constantFog.every(Number.isFinite))
    throw new Error("Constant fog bounce coefficients exceed float32 storage.");
  return { layers, stats, constantFog };
}
