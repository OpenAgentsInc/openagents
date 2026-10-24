import type { CompiledScene } from "../assets/src/scene.ts";
import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";
import { collisionInstanceIdentity } from "./static-collision.ts";

export interface DestructibleObject {
  id: number;
  sourceId: number;
  min: [number, number, number];
  max: [number, number, number];
  hp: number;
  anchored: boolean;
  supports: number[];
}
export interface DestructibleTargetGroups {
  castle: number[];
}
export interface DestructibleSceneManifest {
  schema: "aethyrforge.gameplay.v1";
  profile: "source-cells-v1";
  source: {
    vertices: string;
    indices: string;
    verticesSha256: string;
    indicesSha256: string;
    instancesSha256: string;
  };
  cellSize: [3.5, 3.2, 3.5];
  objects: DestructibleObject[];
  targetGroups?: DestructibleTargetGroups;
  statistics?: Record<string, number>;
}
export const DESTRUCTIBLE_LIMITS = Object.freeze({
  nodes: 65536,
  supports: 16,
  manifestBytes: 32 * 1024 * 1024,
  coordinate: 20000,
});
const HASH = /^[a-f0-9]{64}$/;
const MANIFEST = /^\/private-assets\/gameplay-([a-f0-9]{64})\.json$/;
const hash = async (bytes: ArrayBuffer) =>
  [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((v) => v.toString(16).padStart(2, "0"))
    .join("");
function localGeometry(url: string) {
  return (
    typeof url === "string" &&
    /^\/private-assets\/(?:[A-Za-z0-9._-]+\/)*[A-Za-z0-9._-]+\.bin$/.test(
      url,
    ) &&
    !url.split("/").some((part) => part === "." || part === "..")
  );
}
function count(value: number, min: number, max: number) {
  return Number.isSafeInteger(value) && value >= min && value <= max;
}
function castleSourceIds(scene: CompiledScene) {
  const named = scene.instances
    .map((instance, id) =>
      /(?:^|_)castle(?:_|$)/i.test(scene.meshes[instance.mesh]?.name ?? "")
        ? id
        : -1,
    )
    .filter((id) => id >= 0);
  const sources = new Set(named);
  if (!named.length) return sources;
  const bounds = (id: number) => {
    const instance = scene.instances[id],
      local = scene.meshes[instance.mesh]?.bounds;
    if (!local)
      throw new Error("Prepare castle placement bounds before destruction.");
    const min = [Infinity, Infinity, Infinity],
      max = [-Infinity, -Infinity, -Infinity];
    for (const x of [local.min[0], local.max[0]])
      for (const y of [local.min[1], local.max[1]])
        for (const z of [local.min[2], local.max[2]])
          for (let axis = 0; axis < 3; axis++) {
            const value =
              instance.model[axis] * x +
              instance.model[4 + axis] * y +
              instance.model[8 + axis] * z +
              instance.model[12 + axis];
            if (!Number.isFinite(value))
              throw new Error("Prepare finite castle placement bounds.");
            min[axis] = Math.min(min[axis], value);
            max[axis] = Math.max(max[axis], value);
          }
    return { min, max };
  };
  const assembly = named.map(bounds),
    min = [0, 1, 2].map((axis) =>
      Math.min(...assembly.map((item) => item.min[axis])),
    ),
    max = [0, 1, 2].map((axis) =>
      Math.max(...assembly.map((item) => item.max[axis])),
    );
  for (let id = 0; id < scene.instances.length; id++) {
    const name = scene.meshes[scene.instances[id].mesh]?.name ?? "";
    if (
      sources.has(id) ||
      /ground|floor|pavement|terrain|landscape|water|sky|cloud|fog|backdrop|particle|projected|sphere|mountain/i.test(
        name,
      )
    )
      continue;
    const item = bounds(id);
    if (
      item.min.every((value, axis) => value >= min[axis] - 1e-5) &&
      item.max.every((value, axis) => value <= max[axis] + 1e-5)
    )
      sources.add(id);
  }
  return sources;
}
export function validateDestructibleScene(
  manifest: DestructibleSceneManifest,
  scene: CompiledScene,
): DestructibleSceneManifest {
  const s = manifest?.source;
  if (
    manifest?.schema !== "aethyrforge.gameplay.v1" ||
    manifest.profile !== "source-cells-v1" ||
    !s ||
    s.vertices !== scene.vertices ||
    s.indices !== scene.indices ||
    !localGeometry(s.vertices) ||
    !localGeometry(s.indices) ||
    !HASH.test(s.verticesSha256) ||
    !HASH.test(s.indicesSha256) ||
    !HASH.test(s.instancesSha256) ||
    !Array.isArray(manifest.cellSize) ||
    manifest.cellSize.length !== 3 ||
    manifest.cellSize.some((v, i) => v !== [3.5, 3.2, 3.5][i]) ||
    !Array.isArray(manifest.objects) ||
    !count(manifest.objects.length, 1, DESTRUCTIBLE_LIMITS.nodes)
  )
    throw new Error("Prepare destruction for the current town scene.");
  const reverse = Array.from(
      { length: manifest.objects.length },
      () => [] as number[],
    ),
    reached = new Uint8Array(manifest.objects.length),
    queue: number[] = [];
  for (let i = 0; i < manifest.objects.length; i++) {
    const o = manifest.objects[i];
    if (
      !o ||
      o.id !== i ||
      !count(o.sourceId, 0, scene.instances.length - 1) ||
      ![12, 18, 22, 27].includes(o.hp) ||
      typeof o.anchored !== "boolean" ||
      !Array.isArray(o.supports) ||
      !count(
        o.supports.length,
        o.anchored ? 0 : 1,
        o.anchored ? 0 : DESTRUCTIBLE_LIMITS.supports,
      ) ||
      new Set(o.supports).size !== o.supports.length
    )
      throw new Error("The town has incomplete destructible cells.");
    for (const v of [o.min, o.max])
      if (
        !Array.isArray(v) ||
        v.length !== 3 ||
        v.some(
          (n) =>
            typeof n !== "number" ||
            !Number.isFinite(n) ||
            Math.abs(n) > DESTRUCTIBLE_LIMITS.coordinate,
        )
      )
        throw new Error(
          "Destructible cells exceed the supported world bounds.",
        );
    for (let axis = 0; axis < 3; axis++)
      if (
        o.max[axis] <= o.min[axis] ||
        o.max[axis] - o.min[axis] > manifest.cellSize[axis] + 1e-5
      )
        throw new Error("Destructible cells exceed the supported cell size.");
    if (o.anchored) {
      reached[i] = 1;
      queue.push(i);
    }
    for (const support of o.supports) {
      if (!count(support, 0, manifest.objects.length - 1) || support === i)
        throw new Error("A destructible cell has an invalid support.");
      reverse[support].push(i);
    }
  }
  for (let i = 0; i < queue.length; i++)
    for (const dependent of reverse[queue[i]])
      if (!reached[dependent]) {
        reached[dependent] = 1;
        queue.push(dependent);
      }
  if (queue.length !== manifest.objects.length)
    throw new Error("The town has destructible cells without a foundation.");
  for (const o of manifest.objects) {
    let lateral = 0;
    for (const id of o.supports) {
      const support = manifest.objects[id];
      if (
        o.min.some(
          (n, axis) =>
            n > support.max[axis] + 0.35001 ||
            o.max[axis] < support.min[axis] - 0.35001,
        )
      )
        throw new Error("Destructible support cells must touch.");
      if (!(
        support.max[1] <= o.min[1] + 0.35001 && support.min[1] < o.min[1] - 0.02
      ))
        lateral++;
    }
    if (lateral > 2)
      throw new Error("Destructible cells allow at most two lateral supports.");
  }
  if (manifest.targetGroups !== undefined) {
    const groups = manifest.targetGroups;
    const ids = groups?.castle;
    if (
      !groups ||
      Object.keys(groups).length !== 1 ||
      !Array.isArray(ids) ||
      !count(ids.length, 1, DESTRUCTIBLE_LIMITS.nodes) ||
      ids.some(
        (id, i) =>
          !count(id, 0, manifest.objects.length - 1) ||
          (i > 0 && id <= ids[i - 1]),
      )
    )
      throw new Error("Prepare a complete castle target group.");
    const sources = castleSourceIds(scene);
    const expected = manifest.objects.filter((o) => sources.has(o.sourceId));
    if (
      !sources.size ||
      new Set(expected.map((o) => o.sourceId)).size !== sources.size ||
      expected.length !== ids.length ||
      expected.some((o, i) => o.id !== ids[i])
    )
      throw new Error(
        "The castle target group must include every cell of its source placements.",
      );
    const members = new Set(ids);
    if (
      manifest.objects.some((o) =>
        o.supports.some((id) => members.has(o.id) !== members.has(id)),
      )
    )
      throw new Error(
        "Castle support links must stay inside the castle group.",
      );
  }
  return manifest;
}
async function bytes(
  url: string,
  limit: number,
  fetcher: typeof fetch,
  expected?: { length?: number; sha256: string; failure: string },
) {
  return readAsset(
    url,
    async (response) => {
      if (
        !response.ok ||
        !response.body ||
        Number(response.headers.get("content-length")) > limit
      )
        throw new Error("The town's destructible geometry could not open.");
      const reader = response.body.getReader(),
        chunks: Uint8Array[] = [];
      let length = 0;
      try {
        for (;;) {
          const next = await reader.read();
          if (next.done) break;
          length += next.value.length;
          if (length > limit)
            throw new Error(
              "The town's destructible geometry exceeds its size limit.",
            );
          chunks.push(next.value);
        }
      } finally {
        await reader.cancel().catch(() => {});
        reader.releaseLock();
      }
      const result = new Uint8Array(length);
      let at = 0;
      for (const chunk of chunks) {
        result.set(chunk, at);
        at += chunk.length;
      }
      if (
        expected &&
        ((expected.length !== undefined && length !== expected.length) ||
          (await hash(result.buffer)) !== expected.sha256)
      )
        throw new AssetIntegrityError(expected.failure);
      return result.buffer;
    },
    { fetcher, init: { credentials: "same-origin", redirect: "error" } },
  );
}
/** Admit the immutable cell rules against the exact rendered source and placements. */
export async function loadDestructibleScene(
  scene: CompiledScene & { gameplayURL?: string },
  fetcher = fetch,
): Promise<
  | { manifest: DestructibleSceneManifest; objects: DestructibleObject[] }
  | undefined
> {
  if (!scene.gameplayURL) return undefined;
  const identity = MANIFEST.exec(scene.gameplayURL)?.[1];
  if (!identity)
    throw new Error("The town contains an unavailable destruction resource.");
  const encoded = await bytes(
    scene.gameplayURL,
    DESTRUCTIBLE_LIMITS.manifestBytes,
    fetcher,
    {
      sha256: identity,
      failure: "The town's destruction rules changed. Prepare them again.",
    },
  );
  const manifest = validateDestructibleScene(
    JSON.parse(new TextDecoder().decode(encoded)),
    scene,
  );
  if (
    (await collisionInstanceIdentity(scene.instances)) !==
    manifest.source.instancesSha256
  )
    throw new Error("Prepare destruction after changing town placements.");
  for (const [field, countField, stride] of [
    ["vertices", "vertexCount", 48],
    ["indices", "indexCount", 4],
  ] as const) {
    const expected = scene[countField] * stride;
    if (!count(expected, 1, 512 * 1024 * 1024))
      throw new Error("The town's destruction source exceeds its byte budget.");
    await bytes(manifest.source[field], expected, fetcher, {
      length: expected,
      sha256:
        manifest.source[
          field === "vertices" ? "verticesSha256" : "indicesSha256"
        ],
      failure: "Prepare destruction after changing town geometry.",
    });
  }
  return { manifest, objects: manifest.objects };
}
