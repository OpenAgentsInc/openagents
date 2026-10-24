import type { CompiledScene } from "../assets/src/scene.ts";
import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";

export interface StaticCollisionManifest {
  schema: "aethyrforge.static-collision.v1";
  profile: "simplified-solid-surfaces-v1";
  source: {
    vertices: string;
    indices: string;
    instancesSha256: string;
    gameplayURL?: string;
  };
  triangles: { url: string; sha256: string; byteLength: number; count: number };
  owners?: { url: string; sha256: string; byteLength: number; count: number };
}
export interface StaticCollisionData {
  triangles: Float32Array;
  /** Destructible cell per triangle; 0xffffffff marks fixed geometry. */
  owners?: Uint32Array;
}
const MAX_TRIANGLES = 2_000_000;
const hash = async (bytes: ArrayBuffer) =>
  Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (n) => n.toString(16).padStart(2, "0"),
  ).join("");
async function boundedBytes(response: Response, limit: number) {
  const reader = response.body?.getReader();
  if (!reader) throw new Error("The town's walking surfaces are incomplete.");
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit)
        throw new Error("The town's walking surfaces exceed their size limit.");
      chunks.push(value);
    }
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
  const result = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }
  return result.buffer;
}

export async function collisionInstanceIdentity(
  instances: CompiledScene["instances"],
) {
  const bytes = new ArrayBuffer(instances.length * 132),
    view = new DataView(bytes);
  instances.forEach((instance, i) => {
    if (
      !Number.isSafeInteger(instance.mesh) ||
      instance.mesh < 0 ||
      instance.model.length !== 16 ||
      !instance.model.every(Number.isFinite)
    )
      throw new Error(
        "The collision scene needs complete instance transforms.",
      );
    view.setUint32(i * 132, instance.mesh, true);
    instance.model.forEach((n, j) =>
      view.setFloat64(i * 132 + 4 + j * 8, n, true),
    );
  });
  return hash(bytes);
}

export function validateStaticCollision(
  manifest: StaticCollisionManifest,
  scene: CompiledScene,
) {
  const t = manifest?.triangles,
    owners = manifest?.owners;
  if (
    manifest?.schema !== "aethyrforge.static-collision.v1" ||
    manifest.profile !== "simplified-solid-surfaces-v1" ||
    manifest.source?.vertices !== scene.vertices ||
    manifest.source.indices !== scene.indices ||
    !/^[a-f0-9]{64}$/.test(manifest.source.instancesSha256) ||
    !t ||
    !Number.isSafeInteger(t.count) ||
    t.count < 1 ||
    t.count > MAX_TRIANGLES ||
    t.byteLength !== t.count * 36 ||
    !/^[a-f0-9]{64}$/.test(t.sha256) ||
    !/^\/private-assets\/collision-[a-f0-9]{64}\.bin$/.test(t.url) ||
    (owners !== undefined &&
      (owners.count !== t.count ||
        owners.byteLength !== t.count * 4 ||
        !/^[a-f0-9]{64}$/.test(owners.sha256) ||
        !/^\/private-assets\/collision-owners-[a-f0-9]{64}\.bin$/.test(
          owners.url,
        ) ||
        manifest.source.gameplayURL !== scene.gameplayURL)) ||
    (scene.gameplayURL !== undefined && !owners)
  )
    throw new Error("Prepare collision for the current town scene.");
}

/** Static collision is prepared once; the fixed-step kernel owns contact queries. */
export async function loadStaticCollision(
  scene: CompiledScene,
  fetcher = fetch,
): Promise<StaticCollisionData | undefined> {
  if (!scene.collisionURL) return undefined;
  if (
    !/^\/private-assets\/collision-[a-f0-9]{64}\.json$/.test(scene.collisionURL)
  )
    throw new Error("The town contains an unavailable collision resource.");
  const encoded = await readAsset(
    scene.collisionURL,
    async (response) => {
      if (!response.ok)
        throw new Error("The town's walking surfaces could not open.");
      return boundedBytes(response, 16_384);
    },
    { fetcher },
  );
  const text = new TextDecoder().decode(encoded);
  const manifest = JSON.parse(text) as StaticCollisionManifest;
  validateStaticCollision(manifest, scene);
  if (
    (await collisionInstanceIdentity(scene.instances)) !==
    manifest.source.instancesSha256
  )
    throw new Error("Prepare collision after changing the town geometry.");
  const bytes = await readAsset(
    manifest.triangles.url,
    async (geometry) => {
      if (!geometry.ok)
        throw new Error("The town's walking surfaces could not open.");
      const bytes = await boundedBytes(geometry, manifest.triangles.byteLength);
      if (
        bytes.byteLength !== manifest.triangles.byteLength ||
        (await hash(bytes)) !== manifest.triangles.sha256
      )
        throw new AssetIntegrityError(
          "The town's walking surfaces are incomplete. Prepare them again.",
        );
      return bytes;
    },
    { fetcher },
  );
  const triangles = new Float32Array(bytes);
  if (!triangles.every((n) => Number.isFinite(n) && Math.abs(n) <= 20_000))
    throw new Error(
      "The town's walking surfaces exceed the supported world size.",
    );
  let owners: Uint32Array | undefined;
  if (manifest.owners) {
    const descriptor = manifest.owners;
    const bytes = await readAsset(
      descriptor.url,
      async (response) => {
        if (!response.ok)
          throw new Error("The town's breakable surfaces could not open.");
        const bytes = await boundedBytes(response, descriptor.byteLength);
        if (
          bytes.byteLength !== descriptor.byteLength ||
          (await hash(bytes)) !== descriptor.sha256
        )
          throw new AssetIntegrityError(
            "The town's breakable surfaces are incomplete. Prepare them again.",
          );
        return bytes;
      },
      { fetcher },
    );
    owners = new Uint32Array(bytes);
  }
  return { triangles, ...(owners ? { owners } : {}) };
}

export function installStaticCollision(
  kernel: { request: (message: object) => any },
  data?: Float32Array | StaticCollisionData,
  objectCount?: number,
) {
  const triangles = data instanceof Float32Array ? data : data?.triangles;
  const owners = data instanceof Float32Array ? undefined : data?.owners;
  if (
    triangles &&
    (triangles.length % 9 || triangles.length > MAX_TRIANGLES * 9)
  )
    throw new Error(
      "The town's walking surfaces have incorrect triangle counts.",
    );
  if (
    owners &&
    (owners.length !== (triangles?.length ?? 0) / 9 ||
      !owners.every(
        (owner) => owner === 0xffffffff || owner < (objectCount ?? 65_536),
      ))
  )
    throw new Error(
      "The town's breakable surfaces do not match its buildings.",
    );
  const chunk = 8192 * 9;
  for (
    let start = 0;
    start < Math.max(1, triangles?.length ?? 0);
    start += chunk
  ) {
    const batch: number[][][] = [];
    for (
      let i = start;
      triangles && i < Math.min(start + chunk, triangles.length);
      i += 9
    )
      batch.push([
        Array.from(triangles.subarray(i, i + 3)),
        Array.from(triangles.subarray(i + 3, i + 6)),
        Array.from(triangles.subarray(i + 6, i + 9)),
      ]);
    const result = kernel.request({
      type: "set_static_collision",
      triangles: batch,
      ...(owners
        ? {
            owners: Array.from(
              owners.subarray(start / 9, start / 9 + batch.length),
            ),
          }
        : {}),
      append: start > 0,
    });
    if (!result.ok)
      throw new Error(
        result.error?.message ??
          result.message ??
          "The town's walking surfaces could not open.",
      );
  }
}
