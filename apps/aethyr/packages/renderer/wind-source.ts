import type { CompiledMaterial, CompiledScene } from "../assets/src/scene.ts";
import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";
import type { SourceDraw } from "./source.ts";
import { buildInstanceBounds, type InstanceBounds } from "./culling.ts";
import { expandWindBounds } from "./wind-shadow-plan.ts";
import { windSwayBound, validateWindSway, type WindSway } from "./wind-sway.ts";
import type { V3 } from "./math.ts";
import { decodeRgba8 } from "../assets/src/rgba.ts";
import {
  validateWindFlutter,
  windFlutterBound,
  type WindFlutter,
} from "./wind-flutter.ts";

export interface SourceWindFlutter extends WindFlutter {
  baseColorUrl: string;
  sourceGraphSha256: string;
  sourceFunctionSha256: string;
  sourceWindFunctionSha256: string;
}
export interface SourceWindTreeEvidence {
  profile: "source-tree-flutter-evidence-v1";
  factsSha256: string;
  sourceMapSha256: string;
  sourceMaterialsSha256: string;
  meshId: string;
  meshSphereRadiusCm: number;
  componentSphereRadiusCm: number;
  uniformScale: number;
  boundsScale: 1;
  materialId: string;
  baseColorSha256: string;
  verticesSha256: string;
  indicesSha256: string;
  vertexColorsSha256: string;
  bounds: {
    maxPivotDistanceMeters: number;
    maxVertexGreen: number;
    swayMeters: number;
    flutterMeters: number;
    additionalMeters: number;
    combinedMeters: number;
  };
}
const SUPPORTED_FLUTTER_FUNCTION =
  "67202b42b32d5d4181432ad134f08dfc0182a01f047e2e32d8bbda55fbbd3c9d";
const SUPPORTED_WIND_FUNCTION =
  "0071a94091d98da36f4bbc0f231e36c0211a1af625164cc194bcf2f1ec1ae5a5";
const SUPPORTED_MATERIAL_GRAPH =
  "07d7e27f4d04a38a2e68fa6a49409a62ac293be80ec6c6b74534878e24ad06ab";

/** Optional private source facts; ordinary sway may include a separately qualified flutter recipe. */
export interface SourceWindManifest {
  schema: "aethyrforge.source-wind-sway.v1";
  scenario: "day" | "night";
  /** SHA-256 of the read-only recovered component-bounds document. */
  sourceBoundsSha256: string;
  source: {
    vertices: string;
    indices: string;
    vertexColors: string;
    vertexCount: number;
    indexCount: number;
    instances: number;
  };
  records: Array<{
    instance: number;
    actor: string;
    component: string;
    model: number[];
    radiusMeters: number;
    pivot: V3;
    worldUp: V3;
    treeEvidence?: SourceWindTreeEvidence;
    materials: Array<{
      id: string;
      windSpeed: number;
      swaySpeed: number;
      swayAmount: number;
      flutter?: SourceWindFlutter;
    }>;
  }>;
}

/** Check only the optional flutter inputs; old sway records need no new fetches or hashing. */
export async function validateSourceWindBuffers(
  manifests: Partial<Record<"day" | "night", SourceWindManifest>> | undefined,
  buffers: {
    vertices: ArrayBuffer;
    indices: ArrayBuffer;
    vertexColors: ArrayBuffer;
  },
) {
  const hashes = new Map<string, Promise<string>>();
  const digest = async (data: ArrayBuffer) =>
    Array.from(
      new Uint8Array(await crypto.subtle.digest("SHA-256", data)),
      (v) => v.toString(16).padStart(2, "0"),
    ).join("");
  for (const manifest of Object.values(manifests ?? {}))
    for (const record of manifest?.records ?? []) {
      const flutter = record.materials.find((m) => m.flutter)?.flutter;
      if (!flutter) continue;
      if (
        !/^\/private-assets\/[A-Za-z0-9_/-]+\.bin$/.test(flutter.baseColorUrl)
      )
        throw Error("Use a prepared private flutter color map.");
      const evidence = record.treeEvidence;
      if (!evidence)
        throw Error("Recover source tree bounds before enabling flutter.");
      for (const kind of ["vertices", "indices", "vertexColors"] as const) {
        let pending = hashes.get(kind);
        if (!pending) {
          pending = digest(buffers[kind]);
          hashes.set(kind, pending);
        }
        if ((await pending) !== evidence[`${kind}Sha256`])
          throw Error(
            "Source flutter geometry no longer matches its prepared input.",
          );
      }
      let color = hashes.get(flutter.baseColorUrl);
      if (!color) {
        color = readAsset(flutter.baseColorUrl, async (response) => {
          if (!response.ok || !response.body)
            throw Error("The flutter color map is missing.");
          const reader = response.body.getReader(),
            chunks: Uint8Array[] = [];
          let bytes = 0;
          try {
            for (;;) {
              const { done, value } = await reader.read();
              if (done) break;
              bytes += value.byteLength;
              if (bytes > 16 + 2048 * 2048 * 4)
                throw Error(
                  "The flutter color map exceeds its bounded profile.",
                );
              chunks.push(value);
            }
          } catch (error) {
            await reader.cancel();
            throw error;
          }
          const data = new ArrayBuffer(bytes),
            view = new Uint8Array(data);
          let offset = 0;
          for (const chunk of chunks) {
            view.set(chunk, offset);
            offset += chunk.byteLength;
          }
          const decoded = decodeRgba8(data);
          if (decoded.width > 2048 || decoded.height > 2048)
            throw Error("Prepare the supported flutter color resolution.");
          const actual = await digest(data);
          if (actual !== evidence.baseColorSha256)
            throw new AssetIntegrityError(
              "The source flutter color map changed after preparation.",
            );
          return actual;
        });
        hashes.set(flutter.baseColorUrl, color);
      }
      if ((await color) !== evidence.baseColorSha256)
        throw Error("The source flutter color map changed after preparation.");
    }
}

export function validateSourceWindFlutter(
  flutter: SourceWindFlutter,
  material: CompiledMaterial,
) {
  validateWindFlutter(flutter);
  const vegetation = material.vegetation,
    source = vegetation?.source,
    parameters = source?.parameters as
      | {
          scalars?: Record<string, unknown>;
          switches?: Record<string, unknown>;
        }
      | undefined;
  if (
    flutter.sourceFunctionSha256 !== SUPPORTED_FLUTTER_FUNCTION ||
    flutter.sourceWindFunctionSha256 !== SUPPORTED_WIND_FUNCTION ||
    flutter.sourceGraphSha256 !== SUPPORTED_MATERIAL_GRAPH ||
    source?.master !==
      "/Game/Medieval_Town/Materials/Masters/MM_vegetation_01.MM_vegetation_01" ||
    source?.graphSha256 !== flutter.sourceGraphSha256 ||
    !vegetation?.sourceWindEnabled ||
    parameters?.switches?.["Use Wind?"] !== true ||
    parameters.scalars?.["wind strenght"] !== flutter.intensity ||
    parameters.scalars?.["wind weight"] !== flutter.weight ||
    flutter.baseColorUrl !== vegetation.baseColorUrl
  )
    throw Error(
      "Prepare flutter again after changing its source graph or color map.",
    );
}

/** One admitted clock/profile per material; instance wind data still selects moving draws. */
export function sourceWindFlutterMaterial(
  manifests: Partial<Record<"day" | "night", SourceWindManifest>> | undefined,
  material: CompiledMaterial,
) {
  let selected: SourceWindFlutter | undefined,
    omitted = false;
  for (const manifest of Object.values(manifests ?? {}))
    for (const record of manifest?.records ?? [])
      for (const entry of record.materials) {
        if (entry.id !== material.id) continue;
        if (!entry.flutter) {
          omitted = true;
          continue;
        }
        validateSourceWindFlutter(entry.flutter, material);
        if (
          selected &&
          Object.keys(selected).some(
            (key) =>
              selected![key as keyof SourceWindFlutter] !==
              entry.flutter![key as keyof SourceWindFlutter],
          )
        )
          throw Error(
            "Separate different flutter profiles before batching materials.",
          );
        selected = entry.flutter;
      }
  if (selected && omitted)
    throw Error("Separate moving uses with different flutter settings.");
  return selected;
}
export interface WindDraw extends SourceDraw {
  wind?: boolean;
}
export interface PreparedSourceWind {
  parameters: Float32Array;
  moving: Uint8Array;
  draws: WindDraw[];
  bounds: InstanceBounds;
  padding: Float64Array;
  sourceCount: number;
}
function vector(value: unknown): value is V3 {
  return (
    Array.isArray(value) &&
    value.length === 3 &&
    value.every((n) => Number.isFinite(n) && Math.abs(n) <= 1e6)
  );
}
function treeEvidence(
  scene: CompiledScene,
  record: SourceWindManifest["records"][number],
  materialId: string,
) {
  const e = record.treeEvidence,
    instance = scene.instances[record.instance];
  if (
    !e ||
    e.profile !== "source-tree-flutter-evidence-v1" ||
    ![
      e.factsSha256,
      e.sourceMapSha256,
      e.sourceMaterialsSha256,
      e.baseColorSha256,
      e.verticesSha256,
      e.indicesSha256,
      e.vertexColorsSha256,
    ].every((v) => typeof v === "string" && /^[a-f0-9]{64}$/.test(v)) ||
    e.meshId !== scene.meshes[instance.mesh].id ||
    e.materialId !== materialId ||
    e.boundsScale !== 1 ||
    ![e.meshSphereRadiusCm, e.componentSphereRadiusCm, e.uniformScale].every(
      (v) => Number.isFinite(v) && v > 0 && v <= 1e7,
    ) ||
    Math.abs(e.componentSphereRadiusCm * 0.01 - record.radiusMeters) > 1e-8 ||
    Math.abs(
      e.meshSphereRadiusCm * e.uniformScale - e.componentSphereRadiusCm,
    ) > Math.max(1e-4, e.componentSphereRadiusCm * 1e-6) ||
    !e.bounds ||
    !Object.values(e.bounds).every(
      (v) => Number.isFinite(v) && v >= 0 && v <= 1e5,
    )
  )
    throw Error("Recover coherent source tree radius and input identities.");
  const axes = [0, 4, 8].map((i) => record.model.slice(i, i + 3));
  if (
    axes.some(
      (axis) => Math.abs(Math.hypot(...axis) - e.uniformScale) > 1e-6,
    ) ||
    axes.some((a, i) =>
      axes.some(
        (b, j) =>
          i !== j &&
          Math.abs(a.reduce((sum, n, k) => sum + n * b[k], 0)) >
            1e-5 * e.uniformScale ** 2,
      ),
    )
  )
    throw Error(
      "The tree flutter bounds require its unchanged uniform source scale.",
    );
  return e;
}
/** Validate actual indexed vertices and masks, rather than trusting a sidecar displacement bound. */
export function prepareSourceWind(
  scene: CompiledScene,
  draws: readonly SourceDraw[],
  vertices: Float32Array,
  indices: Uint32Array,
  colors: Uint8Array,
  manifest?: SourceWindManifest,
): PreparedSourceWind {
  const count = scene.instances.length,
    parameters = new Float32Array(count * 4),
    moving = new Uint8Array(count),
    padding = new Float64Array(count),
    bounds = buildInstanceBounds(scene);
  if (count > 65536)
    throw Error("The wind profile supports up to 65,536 source instances.");
  if (!manifest)
    return {
      parameters,
      moving,
      draws: draws.map((d) => ({ ...d })),
      bounds,
      padding,
      sourceCount: 0,
    };
  if (
    manifest.schema !== "aethyrforge.source-wind-sway.v1" ||
    typeof manifest.sourceBoundsSha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(manifest.sourceBoundsSha256) ||
    manifest.scenario !== scene.scenario ||
    !Array.isArray(manifest.records) ||
    manifest.records.length > 96 ||
    !manifest.source ||
    Object.entries({
      vertices: scene.vertices,
      indices: scene.indices,
      vertexColors: scene.vertexColors,
      vertexCount: scene.vertexCount,
      indexCount: scene.indexCount,
      instances: count,
    }).some(
      ([k, v]) => manifest.source[k as keyof typeof manifest.source] !== v,
    )
  )
    throw Error("Prepare wind facts for this exact source scene.");
  if (
    vertices.length !== scene.vertexCount * 12 ||
    indices.length !== scene.indexCount ||
    colors.length !== scene.vertexCount * 4
  )
    throw Error("Wind geometry and vertex masks must match this scene.");
  type Settings = WindSway & { flutter?: SourceWindFlutter };
  const records = new Map<number, Map<number, Settings>>();
  for (const record of manifest.records) {
    const instance = scene.instances[record.instance];
    if (
      !Number.isInteger(record.instance) ||
      !instance ||
      records.has(record.instance) ||
      record.actor !== instance.actor ||
      record.component !== instance.component ||
      !Array.isArray(record.model) ||
      record.model.length !== 16 ||
      record.model.some(
        (v, i) => !Number.isFinite(v) || v !== instance.model[i],
      ) ||
      !vector(record.pivot) ||
      !vector(record.worldUp) ||
      Math.hypot(...record.pivot.map((n, i) => n - instance.model[12 + i])) >
        1e-5 ||
      !Array.isArray(record.materials) ||
      !record.materials.length ||
      record.materials.length > 8
    )
      throw Error(
        "Wind facts must identify unique original source components.",
      );
    const up = instance.model.slice(4, 7),
      size = Math.hypot(...up);
    if (
      size < 1e-8 ||
      Math.hypot(...record.worldUp.map((n, i) => n - up[i] / size)) > 1e-5
    )
      throw Error("Wind up axes must match the original component transform.");
    const selected = new Map<number, Settings>();
    for (const material of record.materials) {
      const materialIndex = scene.materials.findIndex(
        (m) => m.id === material.id,
      );
      if (materialIndex < 0 || selected.has(materialIndex))
        throw Error("Wind materials must exist once in the source scene.");
      const settings: Settings = {
        enabled: true,
        pivot: record.pivot,
        worldUp: record.worldUp,
        windDirection: [0, 0, 1],
        radiusMeters: record.radiusMeters,
        windSpeed: material.windSpeed,
        swaySpeed: material.swaySpeed,
        swayAmount: material.swayAmount,
      };
      validateWindSway(settings);
      if (material.flutter) {
        validateSourceWindFlutter(
          material.flutter,
          scene.materials[materialIndex],
        );
        settings.flutter = material.flutter;
      }
      // One immutable instance parameter record is shared across its selected primitive uses.
      if (
        selected.size &&
        [...selected.values()].some(
          (s) =>
            s.windSpeed !== settings.windSpeed ||
            s.swaySpeed !== settings.swaySpeed ||
            s.swayAmount !== settings.swayAmount,
        )
      )
        throw Error(
          "Separate source wind settings before batching this component.",
        );
      const recipe = scene.materials[materialIndex].vegetation?.source;
      if (recipe) {
        const parameters = recipe.parameters as {
          scalars?: Record<string, unknown>;
          switches?: Record<string, unknown>;
        };
        if (
          parameters.switches?.["Use Wind?"] !== true ||
          parameters.scalars?.["wind speed"] !== settings.windSpeed ||
          parameters.scalars?.["sway speed"] !== settings.swaySpeed ||
          parameters.scalars?.["sway amount"] !== settings.swayAmount
        )
          throw Error(
            "Prepare wind facts again after changing source motion controls.",
          );
      }
      selected.set(materialIndex, settings);
    }
    records.set(record.instance, selected);
    const s = [...selected.values()][0];
    parameters.set(
      [s.windSpeed, s.swaySpeed, s.swayAmount, s.radiusMeters],
      record.instance * 4,
    );
  }
  const result: WindDraw[] = [],
    used = new Set<string>(),
    measured = new Map<string, { distance: number; green: number }>();
  for (const draw of draws) {
    let start = draw.firstInstance,
      previous: boolean | undefined;
    const end = draw.firstInstance + draw.count;
    for (let i = start; i <= end; i++) {
      let dynamic = false;
      const settings = records.get(i)?.get(draw.material);
      if (i < end && settings) {
        let maxDistance = 0,
          maximumGreen = 0;
        if (
          draw.firstIndex < 0 ||
          draw.indexCount < 0 ||
          draw.firstIndex + draw.indexCount > indices.length
        )
          throw Error("Wind primitive indices exceed the source geometry.");
        const m = scene.instances[i].model;
        for (
          let n = draw.firstIndex;
          n < draw.firstIndex + draw.indexCount;
          n++
        ) {
          const index = indices[n] + draw.baseVertex;
          if (
            !Number.isInteger(index) ||
            index < 0 ||
            index >= scene.vertexCount
          )
            throw Error("Wind vertex indices exceed the source geometry.");
          const v = index * 12,
            x = vertices[v],
            y = vertices[v + 1],
            z = vertices[v + 2];
          const p = [
            m[0] * x + m[4] * y + m[8] * z + m[12],
            m[1] * x + m[5] * y + m[9] * z + m[13],
            m[2] * x + m[6] * y + m[10] * z + m[14],
          ];
          maxDistance = Math.max(
            maxDistance,
            Math.hypot(...p.map((n, a) => n - settings.pivot[a])),
          );
          maximumGreen = Math.max(maximumGreen, colors[index * 4 + 1] / 255);
        }
        const bound =
          windSwayBound(maxDistance, maximumGreen, settings) +
          (settings.flutter
            ? windFlutterBound(maximumGreen, settings.flutter)
            : 0);
        if (!Number.isFinite(bound) || bound > 10)
          throw Error(
            "Source wind displacement exceeds the ten-meter profile bound.",
          );
        dynamic = bound > 0;
        padding[i] = Math.max(padding[i], bound);
        moving[i] ||= Number(dynamic);
        used.add(i + ":" + draw.material);
        const key = i + ":" + draw.material,
          previous = measured.get(key);
        measured.set(key, {
          distance: Math.max(previous?.distance ?? 0, maxDistance),
          green: Math.max(previous?.green ?? 0, maximumGreen),
        });
      }
      if (i === end || (previous !== undefined && previous !== dynamic)) {
        if (i > start)
          result.push({
            ...draw,
            firstInstance: start,
            count: i - start,
            wind: previous,
          });
        start = i;
      }
      previous = dynamic;
    }
  }
  for (const [i, materials] of records)
    for (const [material, settings] of materials)
      if (!used.has(i + ":" + material))
        throw Error(
          "A selected source wind material has no rendered primitive.",
        );
      else if (settings.flutter) {
        const record = manifest.records.find((r) => r.instance === i)!,
          evidence = treeEvidence(scene, record, scene.materials[material].id),
          actual = measured.get(i + ":" + material)!,
          sway = windSwayBound(actual.distance, actual.green, settings),
          flutter =
            0.2 *
            settings.flutter.intensity *
            settings.flutter.weight *
            actual.green,
          additional = Math.sqrt(3) * 0.01 * actual.green,
          expected = {
            maxPivotDistanceMeters: actual.distance,
            maxVertexGreen: actual.green,
            swayMeters: sway,
            flutterMeters: flutter,
            additionalMeters: additional,
            combinedMeters: sway + flutter + additional,
          };
        if (
          Object.entries(expected).some(
            ([key, value]) =>
              !Number.isFinite(evidence.bounds[key as keyof typeof expected]) ||
              Math.abs(evidence.bounds[key as keyof typeof expected] - value) >
                1e-6,
          )
        )
          throw Error(
            "Recompute flutter bounds from the actual indexed source vertices.",
          );
      }
  return {
    parameters,
    moving,
    draws: result,
    padding,
    bounds: expandWindBounds(bounds, padding),
    sourceCount: records.size,
  };
}

/** A fragment-only edit may keep the exact owned vertex dependency prefix. */
export function usesOwnedWindVertex(
  source: string,
  reference: string,
): boolean {
  const end = reference.indexOf("fn sourceFrontFacing");
  return end > 0 && source.startsWith(reference.slice(0, end));
}

/** Time-varying or edited shadow coverage must not reuse the static depth copy. */
export function usesOwnedWindShadow(
  source: string,
  reference: string,
): boolean {
  const start = reference.indexOf("@fragment fn shadowFS"),
    end = reference.indexOf("fn shadowDepthBias", start);
  const candidateStart = source.indexOf("@fragment fn shadowFS"),
    candidateEnd = source.indexOf("fn shadowDepthBias", candidateStart);
  return (
    usesOwnedWindVertex(source, reference) &&
    start >= 0 &&
    end > start &&
    candidateStart >= 0 &&
    candidateEnd > candidateStart &&
    source.slice(candidateStart, candidateEnd) === reference.slice(start, end)
  );
}
