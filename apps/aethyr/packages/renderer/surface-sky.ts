import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";
import type { CompiledScene, EnvironmentMap } from "../assets/src/scene.ts";
import { collectGiMaterialTextures } from "./gi-materials.ts";
import { sh9Basis } from "./environment-lighting.ts";
import { normalize, dot, type V3 } from "./math.ts";

/** A static surface refinement replaces direct sky, never a previously occluded term times AO. */
export const SURFACE_SKY_LIMITS = Object.freeze({
  selections: 8,
  vertices: 32768,
  rays: 512,
  bytes: 32 * 1024 * 1024,
});
export const SURFACE_SKY_VERTEX_FLOATS = 72;
export interface SurfaceSkySelection {
  actor: string;
  component: string;
  mesh: string;
  model: number[];
  materialIds: string[];
  firstVertex: number;
  vertexCount: number;
  recordStart: number;
  /** Omitted for legacy dense ranges; otherwise exact sorted source vertex IDs. */
  vertexIndices?: number[];
}
export interface SurfaceSkyBlob {
  url: string;
  sha256: string;
  bytes: number;
}
export interface SurfaceSkyManifest {
  schema: "aethyrforge.surface-sky.v1";
  profile: "static-surface-split-sky-sh9-v1";
  reflection?: { model: "sh9-ggx-rough-v1" };
  selections: SurfaceSkySelection[];
  vertexCount: number;
  records: { strideFloats: 72; primaryOffset: 0; constantOffset: 36 };
  transport: {
    nearMeters: number;
    farMeters: number;
    normalOffsetMeters: number;
    rays: number;
    alphaPolicy: "alpha-tested-material-v1";
    staticPose: true;
    lowerHemisphere: "world-up-black";
  };
  scenarios: Partial<
    Record<
      "day" | "night",
      SurfaceSkyBlob & {
        sourceSceneSha256: string;
        sourceEnvironmentSha256: string;
      }
    >
  >;
  /** Inputs are immutable, hashed separately; grid and optional effect links are intentionally absent. */
  sourceBlobs: Partial<Record<"day" | "night", Array<SurfaceSkyBlob>>>;
  metadata?: Record<string, unknown>;
}
/** Negative and out-of-range override sentinels retain the primitive material, matching source draws. */
export function surfaceSkyMaterialIndex(
  materialCount: number,
  overrides: readonly number[],
  slot: number,
  primitive: number,
) {
  const candidate = overrides[slot];
  return Number.isInteger(candidate) &&
    candidate >= 0 &&
    candidate < materialCount
    ? candidate
    : primitive;
}
function canonical(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(canonical);
  if (v && typeof v === "object")
    return Object.fromEntries(
      Object.entries(v)
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([k, x]) => [k, canonical(x)]),
    );
  return v;
}
/** Changing a model, material, UV stream or geometry disables the static refinement. */
export function surfaceSkySceneIdentityPayload(scene: CompiledScene) {
  return JSON.stringify(
    canonical({
      vertices: scene.vertices,
      indices: scene.indices,
      vertexColors: scene.vertexColors,
      uv1: scene.vertexUV1,
      vertexCount: scene.vertexCount,
      indexCount: scene.indexCount,
      vertexStride: scene.vertexStride,
      materials: scene.materials,
      meshes: scene.meshes,
      instances: scene.instances,
    }),
  );
}
export function surfaceSkyEnvironmentIdentityPayload(
  environment: EnvironmentMap,
) {
  return JSON.stringify(canonical(environment));
}
function finite(n: number, lo: number, hi: number) {
  if (!Number.isFinite(n) || n < lo || n > hi)
    throw Error("The surface sky data contains an unsupported number.");
}
function integer(n: number, lo: number, hi: number) {
  finite(n, lo, hi);
  if (!Number.isInteger(n))
    throw Error("The surface sky counts must be whole numbers.");
}
function name(s: string) {
  if (typeof s !== "string" || !s || s.length > 2048)
    throw Error("The surface sky selections need bounded names.");
}
function blob(b: SurfaceSkyBlob) {
  if (
    !b ||
    typeof b.url !== "string" ||
    !/^\/private-assets\/[A-Za-z0-9_/-]+\.(?:bin|png)$/.test(b.url) ||
    b.url.includes("..") ||
    !/^[a-f0-9]{64}$/.test(b.sha256)
  )
    throw Error("The surface sky files need safe private paths and hashes.");
  integer(b.bytes, 1, 256 * 1024 * 1024);
}
export function validateSurfaceSky(m: SurfaceSkyManifest) {
  if (
    m?.schema !== "aethyrforge.surface-sky.v1" ||
    m.profile !== "static-surface-split-sky-sh9-v1" ||
    !Array.isArray(m.selections) ||
    !m.selections.length ||
    m.selections.length > SURFACE_SKY_LIMITS.selections ||
    m.records?.strideFloats !== 72 ||
    m.records.primaryOffset !== 0 ||
    m.records.constantOffset !== 36 ||
    !m.transport ||
    m.transport.alphaPolicy !== "alpha-tested-material-v1" ||
    m.transport.staticPose !== true ||
    m.transport.lowerHemisphere !== "world-up-black"
  )
    throw Error("Prepare a supported static surface sky recipe.");
  if (
    m.reflection !== undefined &&
    (!m.reflection ||
      typeof m.reflection !== "object" ||
      Array.isArray(m.reflection) ||
      Object.keys(m.reflection).join() !== "model" ||
      m.reflection.model !== "sh9-ggx-rough-v1")
  )
    throw Error("Prepare a supported optional surface reflection recipe.");
  integer(m.vertexCount, 1, SURFACE_SKY_LIMITS.vertices);
  finite(m.transport.nearMeters, 0.01, 100);
  finite(m.transport.farMeters, m.transport.nearMeters + 0.01, 10000);
  finite(m.transport.normalOffsetMeters, 0.0001, 0.02);
  integer(m.transport.rays, 32, SURFACE_SKY_LIMITS.rays);
  const names = new Set<string>();
  let count = 0;
  for (const s of m.selections) {
    for (const n of [s.actor, s.component, s.mesh]) name(n);
    if (names.has(s.actor + "\0" + s.component))
      throw Error("The surface sky selections repeat.");
    names.add(s.actor + "\0" + s.component);
    if (
      !Array.isArray(s.model) ||
      s.model.length !== 16 ||
      !s.model.every(Number.isFinite) ||
      Math.abs(s.model[3]) +
        Math.abs(s.model[7]) +
        Math.abs(s.model[11]) +
        Math.abs(s.model[15] - 1) >
        0.00001
    )
      throw Error("The surface sky selections need affine source transforms.");
    if (
      !Array.isArray(s.materialIds) ||
      !s.materialIds.length ||
      s.materialIds.length > 16 ||
      new Set(s.materialIds).size !== s.materialIds.length
    )
      throw Error("The surface sky selection needs distinct materials.");
    s.materialIds.forEach(name);
    integer(s.firstVertex, 0, 0xffffff);
    integer(s.vertexCount, 1, SURFACE_SKY_LIMITS.vertices);
    integer(s.recordStart, 0, m.vertexCount - 1);
    if (
      s.recordStart !== count ||
      (s.vertexIndices === undefined &&
        s.firstVertex + s.vertexCount > 0x1000000)
    )
      throw Error("Surface sky vertex ranges must be consecutive and bounded.");
    if (s.vertexIndices !== undefined) {
      if (
        !Array.isArray(s.vertexIndices) ||
        s.vertexIndices.length !== s.vertexCount ||
        s.vertexIndices[0] !== s.firstVertex
      )
        throw Error(
          "Sparse surface sky addresses must match their vertex counts.",
        );
      s.vertexIndices.forEach((vertex, i) => {
        integer(vertex, 0, 0xffffff);
        if (i > 0 && vertex <= s.vertexIndices![i - 1])
          throw Error(
            "Sparse surface sky addresses must be distinct and sorted.",
          );
      });
    }
    count += s.vertexCount;
  }
  if (
    count !== m.vertexCount ||
    !m.scenarios ||
    !Object.keys(m.scenarios).length ||
    Object.keys(m.scenarios).some((k) => k !== "day" && k !== "night")
  )
    throw Error("The surface sky vertex ranges or scenarios are incomplete.");
  for (const mode of ["day", "night"] as const) {
    const b = m.scenarios[mode];
    if (!b) continue;
    blob(b);
    if (
      !/^[a-f0-9]{64}$/.test(b.sourceSceneSha256) ||
      !/^[a-f0-9]{64}$/.test(b.sourceEnvironmentSha256) ||
      b.bytes !== m.vertexCount * 72 * 4 ||
      surfaceSkyPackedFloatCount(m) * 4 > SURFACE_SKY_LIMITS.bytes
    )
      throw Error(
        "Surface sky records must match their scene and allocation limits.",
      );
    const refs = m.sourceBlobs?.[mode];
    if (!Array.isArray(refs) || refs.length < 3 || refs.length > 256)
      throw Error("Surface sky input hashes are incomplete.");
    refs.forEach(blob);
    if (new Set(refs.map((r) => r.url)).size !== refs.length)
      throw Error("Surface sky input paths repeat.");
  }
  return m;
}

/** Maps are padded to vec4 boundaries; coefficient records retain their exact 72-float layout. */
export function surfaceSkyPackedFloatCount(
  m: Pick<SurfaceSkyManifest, "selections" | "vertexCount">,
) {
  return (
    m.selections.length * 4 +
    m.selections.reduce(
      (sum, s) =>
        sum + (s.vertexIndices ? Math.ceil(s.vertexCount / 4) * 4 : 0),
      0,
    ) +
    m.vertexCount * 72
  );
}
export type SurfaceSkyTrace = (
  origin: V3,
  direction: V3,
  minimum: number,
  maximum: number,
) => boolean;
export interface SurfaceSkyPoint {
  position: V3;
  geometricNormal: V3;
}
/** A sphere quadrature independent of the camera and shading normal. */
export function bakeSurfaceSkyPoint(
  point: SurfaceSkyPoint,
  options: SurfaceSkyManifest["transport"],
  occluded: SurfaceSkyTrace,
  primary: (direction: V3) => V3,
  constant: (direction: V3) => V3 = () => [0, 0, 0],
  seed = 0,
) {
  if (
    !point.geometricNormal.every(Number.isFinite) ||
    Math.hypot(...point.geometricNormal) < 1e-9
  )
    throw Error("Surface sky samples need a nonzero geometric normal.");
  const output = new Float32Array(72),
    n = normalize(point.geometricNormal);
  if (
    !point.position.every(Number.isFinite) ||
    !n.every(Number.isFinite) ||
    Math.hypot(...n) < 0.99
  )
    throw Error(
      "Surface sky samples need a finite point and geometric normal.",
    );
  integer(options.rays, 32, SURFACE_SKY_LIMITS.rays);
  finite(options.normalOffsetMeters, 0.0001, 0.02);
  finite(options.nearMeters, 0.01, 100);
  finite(options.farMeters, options.nearMeters + 0.01, 10000);
  let nearBlocked = 0,
    farBlocked = 0;
  const rotation =
    ((Math.imul(seed >>> 0, 2654435761) >>> 0) / 0x100000000) * Math.PI * 2;
  for (let ray = 0; ray < options.rays; ray++) {
    const y = 1 - (2 * (ray + 0.5)) / options.rays,
      radius = Math.sqrt(Math.max(0, 1 - y * y)),
      phi = ray * 2.399963229728653 + rotation;
    const d: V3 = [radius * Math.cos(phi), y, radius * Math.sin(phi)],
      side = dot(n, d) < 0 ? -1 : 1;
    const origin = point.position.map(
      (v, a) => v + n[a] * side * options.normalOffsetMeters,
    ) as V3;
    if (
      occluded(origin, d, options.normalOffsetMeters * 0.25, options.nearMeters)
    ) {
      nearBlocked++;
      continue;
    }
    if (occluded(origin, d, options.nearMeters, options.farMeters)) {
      farBlocked++;
      continue;
    }
    const basis = sh9Basis(d),
      values = [primary(d), constant(d)];
    for (let layer = 0; layer < 2; layer++)
      for (let k = 0; k < 9; k++)
        for (let c = 0; c < 3; c++) {
          finite(values[layer][c], 0, 65504);
          const band = k === 0 ? 1 : k < 4 ? 2 / 3 : 1 / 4;
          output[layer * 36 + k * 4 + c] +=
            values[layer][c] * basis[k] * ((4 * Math.PI) / options.rays) * band;
        }
  }
  // Every selected record is valid, including a fully blocked (all-zero) surface.
  output[3] = 1;
  return {
    coefficients: output,
    nearBlocked,
    farBlocked,
    visible: options.rays - nearBlocked - farBlocked,
  };
}
export function evaluateSurfaceSky(
  coefficients: ArrayLike<number>,
  normal: V3,
  solarRatio: number,
) {
  if (coefficients.length !== 72)
    throw Error("A surface sky record needs both SH9 layers.");
  finite(solarRatio, 0, 65536);
  const basis = sh9Basis(normalize(normal)),
    sum: V3 = [0, 0, 0];
  for (let k = 0; k < 9; k++)
    for (let c = 0; c < 3; c++) {
      const a = coefficients[k * 4 + c],
        b = coefficients[36 + k * 4 + c];
      finite(a, -1e6, 1e6);
      finite(b, -1e6, 1e6);
      sum[c] += (a * solarRatio + b) * basis[k];
    }
  return sum.map((v) => Math.max(0, v)) as V3;
}
export function packSurfaceSky(m: SurfaceSkyManifest, values: Float32Array) {
  validateSurfaceSky(m);
  if (
    values.length !== m.vertexCount * 72 ||
    !values.every((v) => Number.isFinite(v) && Math.abs(v) <= 1e6)
  )
    throw Error("The surface sky coefficients are incomplete or nonfinite.");
  for (let i = 0; i < m.vertexCount; i++) {
    if (values[i * 72 + 3] !== 0 && values[i * 72 + 3] !== 1)
      throw Error("The surface sky validity mask is invalid.");
    for (let k = 1; k < 18; k++)
      if (values[i * 72 + k * 4 + 3] !== 0)
        throw Error("The surface sky padding must be zero.");
  }
  const result = new Float32Array(surfaceSkyPackedFloatCount(m));
  let mapStart = m.selections.length * 4;
  const recordsStart = result.length - values.length;
  m.selections.forEach((s, i) => {
    result.set(
      [
        s.vertexIndices ? mapStart : s.firstVertex,
        s.vertexCount,
        recordsStart / 4 + s.recordStart * 18,
        (s.vertexIndices ? 3 : 1) + Number(Boolean(m.reflection)),
      ],
      i * 4,
    );
    if (s.vertexIndices) {
      result.set(s.vertexIndices, mapStart);
      mapStart += Math.ceil(s.vertexCount / 4) * 4;
    }
  });
  result.set(values, recordsStart);
  return result;
}
/** The marker is material-local: a selected instance must have its own draw/material binding. */
export function surfaceSkySelection(
  m: SurfaceSkyManifest,
  actor: string,
  component: string,
  materialId: string,
) {
  return (
    m.selections.findIndex(
      (s) =>
        s.actor === actor &&
        s.component === component &&
        s.materialIds.includes(materialId),
    ) + 1
  );
}
async function readHashed(b: SurfaceSkyBlob) {
  return readAsset(
    b.url,
    async (response) => {
      if (!response.ok || !response.body)
        throw Error("The prepared surface sky file is missing.");
      const chunks: Uint8Array[] = [];
      let size = 0;
      const reader = response.body.getReader();
      try {
        while (true) {
          const next = await reader.read();
          if (next.done) break;
          size += next.value.byteLength;
          if (size > b.bytes)
            throw Error("The surface sky file exceeds its declared size.");
          chunks.push(next.value);
        }
      } finally {
        await reader.cancel().catch(() => {});
      }
      if (size !== b.bytes)
        throw new AssetIntegrityError("The surface sky file is incomplete.");
      const bytes = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      const hash = Array.from(
        new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
        (x) => x.toString(16).padStart(2, "0"),
      ).join("");
      if (hash !== b.sha256)
        throw new AssetIntegrityError(
          "The surface sky file hash does not match.",
        );
      return bytes;
    },
    {},
  );
}
export type SurfaceSkySourceCache = Map<
  string,
  Promise<{ sha256: string; bytes: number }>
>;
/** Admission checks run before GPU allocation. Cache scope is one world load, not persistent mutable URLs. */
export async function validateSurfaceSkySources(
  manifest: SurfaceSkyManifest,
  scenes: Partial<Record<"day" | "night", CompiledScene>>,
  environments: Partial<Record<"day" | "night", EnvironmentMap>>,
  cache: SurfaceSkySourceCache = new Map(),
) {
  validateSurfaceSky(manifest);
  const digest = async (value: string) =>
    Array.from(
      new Uint8Array(
        await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value)),
      ),
      (v) => v.toString(16).padStart(2, "0"),
    ).join("");
  const dependencies = new Map<string, SurfaceSkyBlob>();
  const indexUrls = new Set<string>();
  const indicesByURL = new Map<string, Uint32Array>();
  let total = 0;
  for (const mode of ["day", "night"] as const) {
    const profile = manifest.scenarios[mode];
    if (!profile) continue;
    const scene = scenes[mode],
      environment = environments[mode];
    if (
      !scene ||
      scene.scenario !== mode ||
      !environment ||
      (await digest(surfaceSkySceneIdentityPayload(scene))) !==
        profile.sourceSceneSha256 ||
      (await digest(surfaceSkyEnvironmentIdentityPayload(environment))) !==
        profile.sourceEnvironmentSha256
    )
      throw Error(
        "Rebuild the prepared surface lighting for this world and environment.",
      );
    const refs = manifest.sourceBlobs[mode]!;
    const expected = new Set([
      scene.vertices,
      scene.indices,
      scene.vertexColors,
      ...(scene.vertexUV1 ? [scene.vertexUV1] : []),
      environment.url,
      ...(environment.constantFog ? [environment.constantFog.url] : []),
      ...scene.materials
        .filter((m) => m.alphaMode === "MASK")
        .flatMap(collectGiMaterialTextures)
        .map((r) => r.url),
    ]);
    if (refs.length !== expected.size || refs.some((r) => !expected.has(r.url)))
      throw Error("The surface lighting source list omits or adds an input.");
    const bytesFor = (url: string) => refs.find((r) => r.url === url)!;
    if (
      bytesFor(scene.vertices).bytes !== scene.vertexCount * 48 ||
      bytesFor(scene.indices).bytes !== scene.indexCount * 4 ||
      bytesFor(scene.vertexColors).bytes !== scene.vertexCount * 4 ||
      (scene.vertexUV1 &&
        bytesFor(scene.vertexUV1).bytes !== scene.vertexCount * 8) ||
      bytesFor(environment.url).bytes !==
        environment.width * environment.height * 8
    )
      throw Error(
        "Surface lighting source sizes do not match their geometry or map dimensions.",
      );
    if (
      environment.constantFog &&
      (environment.constantFog.format !== "rgba16float" ||
        environment.constantFog.bytes !==
          environment.width * environment.height * 8 ||
        bytesFor(environment.constantFog.url).sha256 !==
          environment.constantFog.sha256 ||
        bytesFor(environment.constantFog.url).bytes !==
          environment.constantFog.bytes)
    )
      throw Error(
        "The surface lighting constant map does not match its parent.",
      );
    indexUrls.add(scene.indices);
    for (const input of manifest.sourceBlobs[mode]!) {
      const prior = dependencies.get(input.url);
      if (
        prior &&
        (prior.sha256 !== input.sha256 || prior.bytes !== input.bytes)
      )
        throw Error(
          "The surface lighting scenarios disagree about a shared input.",
        );
      if (!prior) {
        dependencies.set(input.url, input);
        total += input.bytes;
      }
    }
  }
  if (total > 512 * 1024 * 1024)
    throw Error("The surface lighting source checks exceed their byte budget.");
  // Sequential hashing bounds temporary memory to the largest admitted source, rather than their sum.
  for (const input of dependencies.values()) {
    if (indexUrls.has(input.url)) {
      const bytes = await readHashed(input),
        data = new DataView(bytes.buffer);
      indicesByURL.set(
        input.url,
        Uint32Array.from({ length: bytes.byteLength / 4 }, (_, i) =>
          data.getUint32(i * 4, true),
        ),
      );
    }
    let pending = cache.get(input.url);
    if (!pending) {
      pending = indexUrls.has(input.url)
        ? Promise.resolve({ sha256: input.sha256, bytes: input.bytes })
        : readHashed(input).then((bytes) => ({
            sha256: input.sha256,
            bytes: bytes.byteLength,
          }));
      cache.set(input.url, pending);
    }
    try {
      const result = await pending;
      if (result.sha256 !== input.sha256 || result.bytes !== input.bytes)
        throw Error("The surface lighting input changed during loading.");
    } catch (error) {
      cache.delete(input.url);
      throw error;
    }
  }
  for (const mode of ["day", "night"] as const) {
    if (!manifest.scenarios[mode]) continue;
    const scene = scenes[mode]!,
      indices = indicesByURL.get(scene.indices)!;
    for (const selection of manifest.selections) {
      const matching = scene.instances.filter(
        (i) =>
          i.actor === selection.actor && i.component === selection.component,
      );
      if (matching.length !== 1)
        throw Error("A surface lighting instance is missing or duplicated.");
      const instance = matching[0],
        mesh = scene.meshes[instance.mesh];
      if (
        !mesh ||
        mesh.id !== selection.mesh ||
        JSON.stringify(instance.model) !== JSON.stringify(selection.model)
      )
        throw Error("The surface lighting source model changed.");
      const wanted = new Set(selection.materialIds),
        found = new Set<string>();
      let first = Infinity,
        last = -Infinity;
      const addressed = new Set<number>();
      for (const primitive of mesh.primitives) {
        const material =
          scene.materials[
            surfaceSkyMaterialIndex(
              scene.materials.length,
              instance.materialOverrides,
              primitive.materialSlot,
              primitive.material,
            )
          ];
        if (!material || !wanted.has(material.id)) continue;
        if (
          material.alphaMode === "BLEND" ||
          material.glass ||
          material.flame ||
          !Number.isInteger(primitive.firstIndex) ||
          primitive.firstIndex < 0 ||
          !Number.isInteger(primitive.indexCount) ||
          primitive.indexCount < 3 ||
          primitive.indexCount % 3 ||
          primitive.firstIndex + primitive.indexCount > indices.length ||
          !Number.isInteger(primitive.baseVertex)
        )
          throw Error("The surface lighting primitive is unsupported.");
        found.add(material.id);
        for (
          let i = primitive.firstIndex;
          i < primitive.firstIndex + primitive.indexCount;
          i++
        ) {
          const v = indices[i] + primitive.baseVertex;
          if (v < 0 || v >= scene.vertexCount)
            throw Error(
              "The surface lighting index leaves its source vertices.",
            );
          first = Math.min(first, v);
          last = Math.max(last, v);
          if (selection.vertexIndices) {
            addressed.add(v);
            if (addressed.size > selection.vertexCount)
              throw Error(
                "The surface lighting sparse addresses omit selected vertices.",
              );
          }
        }
      }
      if (
        found.size !== wanted.size ||
        first !== selection.firstVertex ||
        (selection.vertexIndices
          ? addressed.size !== selection.vertexCount ||
            selection.vertexIndices.some((vertex) => !addressed.has(vertex))
          : last - first + 1 !== selection.vertexCount)
      )
        throw Error(
          "The surface lighting vertex range does not match its selected materials.",
        );
    }
  }
}
export class SurfaceSky {
  private readonly buffers = new Map<"day" | "night", GPUBuffer>();
  readonly fallback: GPUBuffer;
  bytes = 16;
  private disposed = false;
  private constructor(
    private readonly device: GPUDevice,
    readonly manifest?: SurfaceSkyManifest,
  ) {
    this.fallback = device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(this.fallback, 0, new Float32Array(4));
  }
  static async create(device: GPUDevice, manifest?: SurfaceSkyManifest) {
    const result = new SurfaceSky(device, manifest);
    if (!manifest) return result;
    try {
      validateSurfaceSky(manifest);
      for (const mode of ["day", "night"] as const) {
        const b = manifest.scenarios[mode];
        if (!b) continue;
        const bytes = await readHashed(b),
          data = new DataView(bytes.buffer),
          values = Float32Array.from({ length: bytes.byteLength / 4 }, (_, i) =>
            data.getFloat32(i * 4, true),
          ),
          packed = packSurfaceSky(manifest, values);
        const buffer = device.createBuffer({
          label: mode + " static surface sky",
          size: packed.byteLength,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });
        result.buffers.set(mode, buffer);
        result.bytes += packed.byteLength;
        device.queue.writeBuffer(buffer, 0, packed);
      }
      if (result.bytes > SURFACE_SKY_LIMITS.bytes)
        throw Error("Surface sky buffers exceed their combined memory budget.");
      return result;
    } catch (error) {
      result.dispose();
      throw error;
    }
  }
  get(mode: "day" | "night") {
    return this.buffers.get(mode) ?? this.fallback;
  }
  has(mode: "day" | "night") {
    return this.buffers.has(mode);
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.fallback.destroy();
    for (const buffer of this.buffers.values()) buffer.destroy();
    this.buffers.clear();
  }
}
/** Seven vec4 outputs accompany eight main-vertex locations; front-facing uses the sixteenth input. */
export function surfaceSkyWGSL(binding = 16) {
  integer(binding, 0, 31);
  return `
@group(0)@binding(${binding})var<storage,read>surfaceSkyRecords:array<vec4<f32>>;
struct SurfaceSkyVertex{a:vec4<f32>,b:vec4<f32>,c:vec4<f32>,d:vec4<f32>,e:vec4<f32>,f:vec4<f32>,g:vec4<f32>};
fn surfaceSkyAddress(start:u32,rank:u32)->u32{
 let index=start+rank;return u32(surfaceSkyRecords[index/4u][index%4u]);
}
fn surfaceSkyVertex(selection:f32,vertex:u32,solarRatio:f32,enabled:bool)->SurfaceSkyVertex{
 var out:SurfaceSkyVertex;if !enabled||selection<1.0{return out;}
 let selected=u32(selection)-1u;if selected>=arrayLength(&surfaceSkyRecords){return out;}
 let h=surfaceSkyRecords[selected];let first=u32(h.x);let count=u32(h.y);let start=u32(h.z);
 if count==0u||count>32768u||(h.w!=1.0&&h.w!=2.0&&h.w!=3.0&&h.w!=4.0){return out;}
 var rank=0u;
 if h.w>=3.0{
  let floats=arrayLength(&surfaceSkyRecords)*4u;if first>=floats||count>floats-first{return out;}
  let low=surfaceSkyAddress(first,0u);let high=surfaceSkyAddress(first,count-1u);
  if vertex<low||vertex>high{return out;}
  if vertex==low{rank=0u;}else if vertex==high{rank=count-1u;}else{
   var lo=1u;var hi=count-1u;
   for(var step=0u;step<15u;step++){if lo>=hi{break;}let middle=lo+(hi-lo)/2u;if surfaceSkyAddress(first,middle)<vertex{lo=middle+1u;}else{hi=middle;}}
   if lo>=count||surfaceSkyAddress(first,lo)!=vertex{return out;}rank=lo;
  }
 }else{if vertex<first||vertex-first>=count{return out;}rank=vertex-first;}
 let offset=start+rank*18u;if offset+17u>=arrayLength(&surfaceSkyRecords)||surfaceSkyRecords[offset].w<0.5{return out;}
 var c:array<f32,28>;for(var k=0u;k<9u;k++){let value=surfaceSkyRecords[offset+k].rgb*solarRatio+surfaceSkyRecords[offset+9u+k].rgb;c[k*3u]=value.r;c[k*3u+1u]=value.g;c[k*3u+2u]=value.b;}c[27]=select(1.0,-1.0,h.w==2.0||h.w==4.0);
 out.a=vec4(c[0],c[1],c[2],c[3]);out.b=vec4(c[4],c[5],c[6],c[7]);out.c=vec4(c[8],c[9],c[10],c[11]);out.d=vec4(c[12],c[13],c[14],c[15]);out.e=vec4(c[16],c[17],c[18],c[19]);out.f=vec4(c[20],c[21],c[22],c[23]);out.g=vec4(c[24],c[25],c[26],c[27]);return out;
}
fn surfaceSkyIrradiance(coefficients:SurfaceSkyVertex,normal:vec3<f32>)->vec4<f32>{
 if abs(coefficients.g.w)<0.9999{return vec4(0.0);}
 let n=normalize(normal);let x=n.x;let y=n.y;let z=n.z;let basis=array<f32,9>(0.2820947918,0.4886025119*y,0.4886025119*z,0.4886025119*x,1.0925484306*x*y,1.0925484306*y*z,0.3153915653*(3.0*z*z-1.0),1.0925484306*x*z,0.5462742153*(x*x-y*y));
 let values=array<vec4<f32>,7>(coefficients.a,coefficients.b,coefficients.c,coefficients.d,coefficients.e,coefficients.f,coefficients.g);var sum=vec3(0.0);
 for(var k=0u;k<9u;k++){let r=k*3u;sum+=vec3(values[r/4u][r%4u],values[(r+1u)/4u][(r+1u)%4u],values[(r+2u)/4u][(r+2u)%4u])*basis[k];}return vec4(max(sum,vec3(0.0)),1.0);
}
`;
}
