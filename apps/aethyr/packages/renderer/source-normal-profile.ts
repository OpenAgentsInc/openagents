import type {
  CompiledScene,
  SourceNormalFrameProfile,
} from "../assets/src/scene.ts";
import { foliageTransmissionCode } from "./foliage-transmission.ts";

const hash = /^[a-f0-9]{64}$/;
const object = (v: unknown): v is Record<string, unknown> =>
  Boolean(v && typeof v === "object" && !Array.isArray(v));
const keys = (v: Record<string, unknown>, expected: string[]) =>
  Object.keys(v).sort().join() === [...expected].sort().join();

/** Metadata is retained evidence; actual prepared bytes are checked independently. */
export function validateNormalFrameProfile(
  value: unknown,
): value is SourceNormalFrameProfile {
  if (value === undefined) return false;
  if (
    !object(value) ||
    !keys(value, ["model", "evidence"]) ||
    !["source-mesh-noscale-v1", "source-vegetation-noscale-v1"].includes(
      String(value.model),
    ) ||
    !object(value.evidence)
  )
    throw new Error("Prepare a supported source normal frame.");
  const e = value.evidence;
  if (
    !keys(e, [
      "sourceFactsSha256",
      "sourceMeshId",
      "verticesSha256",
      "indicesSha256",
      "normalTextureSha256",
      ...(value.model === "source-vegetation-noscale-v1"
        ? [
            "sourceMaterialId",
            "sourceMaterialsSha256",
            "sourceNormalMetadataSha256",
          ]
        : []),
    ]) ||
    typeof e.sourceMeshId !== "string" ||
    !e.sourceMeshId.length ||
    e.sourceMeshId.length > 2048 ||
    ![
      e.sourceFactsSha256,
      e.verticesSha256,
      e.indicesSha256,
      e.normalTextureSha256,
      ...(value.model === "source-vegetation-noscale-v1"
        ? [e.sourceMaterialsSha256, e.sourceNormalMetadataSha256]
        : []),
    ].every((v) => typeof v === "string" && hash.test(v))
  )
    throw new Error("The prepared source normal frame is incomplete.");
  if (
    value.model === "source-vegetation-noscale-v1" &&
    (typeof e.sourceMaterialId !== "string" ||
      !e.sourceMaterialId.length ||
      e.sourceMaterialId.length > 2048)
  )
    throw new Error("The prepared vegetation source material is incomplete.");
  return true;
}

/** Native compatibility profiles must reach the same material in the actual render batches. */
export function validateSourceNormalBatches(
  scene: CompiledScene,
  meshIndex: number,
): void {
  const mesh = scene.meshes[meshIndex];
  if (!mesh || !Array.isArray(scene.batches))
    throw new Error("Reprepare the native material render batches.");
  const selected = new Set(
    scene.instances.flatMap((instance, i) =>
      instance.mesh === meshIndex ? [i] : [],
    ),
  );
  const covered = new Set<number>();
  const material = (
    overrides: number[],
    primitive: (typeof mesh.primitives)[number],
  ) => {
    if (
      !Array.isArray(overrides) ||
      !Number.isInteger(primitive.materialSlot) ||
      primitive.materialSlot < 0
    )
      throw new Error("The native material render slots are invalid.");
    const candidate = overrides[primitive.materialSlot] ?? -1;
    const value = candidate >= 0 ? candidate : primitive.material;
    if (
      !Number.isInteger(candidate) ||
      !Number.isInteger(value) ||
      value < 0 ||
      value >= scene.materials.length
    )
      throw new Error("The native material render slots are invalid.");
    return value;
  };
  for (const batch of scene.batches) {
    const first = batch.firstInstance,
      count = batch.instanceCount;
    const valid =
      Number.isInteger(first) &&
      Number.isInteger(count) &&
      first >= 0 &&
      count > 0 &&
      first + count <= scene.instances.length;
    if (batch.mesh !== meshIndex) {
      if (valid && [...selected].some((i) => first <= i && i < first + count))
        throw new Error(
          "A native material instance belongs to another render mesh.",
        );
      continue;
    }
    if (!valid) throw new Error("The native material render range is invalid.");
    for (let i = first; i < first + count; i++) {
      const instance = scene.instances[i];
      if (!selected.has(i) || covered.has(i))
        throw new Error(
          "The native material render batches overlap or name another mesh.",
        );
      covered.add(i);
      for (const primitive of mesh.primitives)
        if (
          material(instance.materialOverrides, primitive) !==
          material(batch.materialOverrides, primitive)
        )
          throw new Error(
            "Reprepare the native material render batches to match their source instances.",
          );
    }
  }
  if (covered.size !== selected.size)
    throw new Error("A native material instance is missing its render batch.");
}

const sha = async (b: ArrayBuffer) =>
  Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", b)), (v) =>
    v.toString(16).padStart(2, "0"),
  ).join("");

/** Check selected mesh uses, native attributes, transforms and bytes before upload. */
export async function validateSourceNormalFrames(
  scene: CompiledScene,
  vertices: ArrayBuffer,
  indices: ArrayBuffer,
  read: (url: string) => Promise<ArrayBuffer>,
): Promise<void> {
  const selected = scene.materials.flatMap((m, index) =>
    validateNormalFrameProfile(m.normalFrameProfile)
      ? [{ m, index, profile: m.normalFrameProfile! }]
      : [],
  );
  if (!selected.length) return;
  if (
    scene.vertexStride !== 48 ||
    vertices.byteLength !== scene.vertexCount * 48 ||
    indices.byteLength !== scene.indexCount * 4
  )
    throw new Error("The prepared source normal geometry is incomplete.");
  const [verticesHash, indicesHash] = await Promise.all([
    sha(vertices),
    sha(indices),
  ]);
  const v = new Float32Array(vertices),
    ix = new Uint32Array(indices);
  const textures = new Map<string, Promise<string>>();
  for (const { m, index, profile } of selected) {
    const e = profile.evidence;
    const vegetation = profile.model === "source-vegetation-noscale-v1";
    if (
      m.normalConvention !== "opengl" ||
      m.normalEncoding !== "xy-positive-z" ||
      m.uvScale[0] !== 1 ||
      m.uvScale[1] !== 1 ||
      !m.normalUrl ||
      m.layers ||
      m.paint ||
      m.terrain ||
      m.glass ||
      m.flame ||
      (vegetation
        ? m.vegetation?.model !== "ue-mm-vegetation-1-v1"
        : Boolean(m.vegetation)) ||
      e.verticesSha256 !== verticesHash ||
      e.indicesSha256 !== indicesHash
    )
      throw new Error(
        "The prepared source normal recipe does not match its geometry.",
      );
    if (vegetation) {
      foliageTransmissionCode(m.vegetation!.transmission);
      const originals = scene.materials.filter(
        (original) => original.id === e.sourceMaterialId,
      );
      const recipe = (original: typeof m, ownedClone = false) => {
        const {
          id: _id,
          name: _name,
          normalConvention: _convention,
          normalFrameProfile: _profile,
          ...rest
        } = original;
        // Only the validated, explicitly owned transport extension is outside
        // the recovered color/normal recipe. The original remains untagged.
        if (ownedClone && rest.vegetation?.transmission !== undefined) {
          const { transmission: _transmission, ...sourceVegetation } =
            rest.vegetation;
          return JSON.stringify({ ...rest, vegetation: sourceVegetation });
        }
        return JSON.stringify(rest);
      };
      if (
        originals.length !== 1 ||
        originals[0].normalFrameProfile ||
        originals[0].vegetation?.transmission !== undefined ||
        m.id === e.sourceMaterialId ||
        recipe(m, true) !== recipe(originals[0])
      )
        throw new Error(
          "The prepared vegetation clone differs from its source recipe.",
        );
    }
    const normalUrl = vegetation ? m.vegetation!.normalUrl : m.normalUrl;
    if (!textures.has(normalUrl))
      textures.set(normalUrl, read(normalUrl).then(sha));
    if ((await textures.get(normalUrl)) !== e.normalTextureSha256)
      throw new Error("The prepared source normal image has changed.");
    const meshes = scene.meshes.filter((mesh) => mesh.id === e.sourceMeshId);
    if (meshes.length !== 1)
      throw new Error("The source normal mesh is missing.");
    if (vegetation)
      validateSourceNormalBatches(scene, scene.meshes.indexOf(meshes[0]));
    let uses = 0;
    const checked = new Set<number>();
    for (const instance of scene.instances) {
      const mesh = scene.meshes[instance.mesh];
      for (const primitive of mesh.primitives) {
        const override = instance.materialOverrides?.[primitive.materialSlot];
        const material =
          override !== undefined && override >= 0
            ? override
            : primitive.material;
        if (material !== index) continue;
        if (mesh.id !== e.sourceMeshId)
          throw new Error("A source normal material is used by another mesh.");
        uses++;
        const model = instance.model;
        if (
          model.length !== 16 ||
          !model.every(Number.isFinite) ||
          Math.abs(model[3]) +
            Math.abs(model[7]) +
            Math.abs(model[11]) +
            Math.abs(model[15] - 1) >
            1e-7
        )
          throw new Error("The source normal transform is invalid.");
        const axes = [0, 4, 8].map((i) => model.slice(i, i + 3));
        const scales = axes.map((a) => Math.hypot(...a));
        if (scales.some((x) => x < 1e-8 || x > 1e6))
          throw new Error("The source normal transform has an invalid scale.");
        const r = axes.map((a, i) => a.map((x) => x / scales[i]));
        const dot = (a: number[], b: number[]) =>
          a.reduce((sum, x, i) => sum + x * b[i], 0);
        if (
          [dot(r[0], r[1]), dot(r[0], r[2]), dot(r[1], r[2])].some(
            (x) => Math.abs(x) > 1e-5,
          )
        )
          throw new Error("The source normal transform cannot contain shear.");
        if (
          !Number.isInteger(primitive.firstIndex) ||
          !Number.isInteger(primitive.indexCount) ||
          primitive.firstIndex < 0 ||
          primitive.indexCount <= 0 ||
          primitive.firstIndex + primitive.indexCount > ix.length
        )
          throw new Error("The source normal primitive is incomplete.");
        for (
          let i = primitive.firstIndex;
          i < primitive.firstIndex + primitive.indexCount;
          i++
        ) {
          const vertex = ix[i] + primitive.baseVertex;
          if (
            !Number.isInteger(vertex) ||
            vertex < 0 ||
            vertex >= scene.vertexCount
          )
            throw new Error("The source normal vertex is missing.");
          if (checked.has(vertex)) continue;
          checked.add(vertex);
          const n = Array.from(v.subarray(vertex * 12 + 3, vertex * 12 + 6)),
            t = Array.from(v.subarray(vertex * 12 + 8, vertex * 12 + 11)),
            h = v[vertex * 12 + 11];
          if (
            !v.subarray(vertex * 12, vertex * 12 + 12).every(Number.isFinite) ||
            ![...n, ...t, h].every(Number.isFinite) ||
            Math.abs(Math.hypot(...n) - 1) > 0.02 ||
            Math.abs(Math.hypot(...t) - 1) > 0.03 ||
            Math.abs(dot(n, t)) > 0.02 ||
            Math.abs(Math.abs(h) - 1) > 1e-6
          )
            throw new Error("The source mesh tangent frame is invalid.");
        }
      }
    }
    if (!uses) throw new Error("The source normal material has no mesh uses.");
  }
}
