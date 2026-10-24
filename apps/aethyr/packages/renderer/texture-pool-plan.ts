import type { LayerTexture, LayerTextureRole } from "./layers.ts";

export const TEXTURE_POOL_ROLES = ["color", "normal", "data"] as const;
export interface TexturePoolBudgets {
  maxDecodedBytes?: number;
  maxGpuBytes?: number;
}
export interface TexturePoolLimits {
  maxTextureArrayLayers: number;
  maxTextureDimension2D: number;
}
export interface TextureImageSize {
  width: number;
  height: number;
}
export interface TextureBucketPlan {
  key: string;
  role: LayerTextureRole;
  size: number;
  mipLevelCount: number;
  urls: readonly string[];
  bytes: number;
}
export interface MaterialTexturePoolPlan {
  buckets: readonly TextureBucketPlan[];
  materials: ReadonlyMap<string, Readonly<Record<LayerTextureRole, string>>>;
  urls: readonly string[];
  bytes: number;
  decodedBytes: number;
  uploadBytes: number;
  peakGpuBytes: number;
}
export function texturePoolBudgets(options: TexturePoolBudgets = {}) {
  const values = {
    maxDecodedBytes: options.maxDecodedBytes ?? 256 * 1024 * 1024,
    maxGpuBytes: options.maxGpuBytes ?? 512 * 1024 * 1024,
  };
  for (const [key, value] of Object.entries(values))
    if (!Number.isSafeInteger(value) || value < 1 || value > 2_147_483_647)
      throw new Error(`${key} must be a positive byte budget up to 2 GiB.`);
  return values;
}
export function copyTextureRequirements(
  requirements: ReadonlyMap<string, readonly LayerTexture[]>,
): Map<string, readonly LayerTexture[]> {
  if (
    !requirements ||
    !Number.isSafeInteger(requirements.size) ||
    requirements.size > 4096 ||
    typeof requirements.entries !== "function"
  )
    throw new Error("Use up to 4096 material texture requirements.");
  const result = new Map<string, readonly LayerTexture[]>(),
    urls = new Set<string>();
  let references = 0;
  for (const [id, refs] of requirements) {
    if (
      typeof id !== "string" ||
      id.length < 1 ||
      id.length > 1024 ||
      !Array.isArray(refs)
    )
      throw new Error(
        "Material texture requirements need a nonempty ID and a texture list.",
      );
    const unique = new Map<string, LayerTexture>();
    for (const ref of refs) {
      if (
        !ref ||
        !TEXTURE_POOL_ROLES.includes(ref.role) ||
        typeof ref.url !== "string" ||
        ref.url.length < 1 ||
        ref.url.length > 4096 ||
        /[\u0000-\u001f]/.test(ref.url)
      )
        throw new Error(
          "A material map needs a supported role and a valid URL.",
        );
      if (++references > 32768)
        throw new Error("Material texture references exceed 32768 entries.");
      unique.set(`${ref.role}\0${ref.url}`, { role: ref.role, url: ref.url });
      urls.add(ref.url);
      if (urls.size > 2048)
        throw new Error("Material textures exceed 2048 distinct images.");
    }
    result.set(id, [...unique.values()]);
  }
  return result;
}
/** Exact RGBA8 array allocation over the complete power-of-two mip chain. */
export function textureBucketBytes(size: number, layers: number): number {
  if (
    !Number.isSafeInteger(size) ||
    size < 1 ||
    size > 2048 ||
    (size & (size - 1)) !== 0 ||
    !Number.isSafeInteger(layers) ||
    layers < 1
  )
    throw new Error("Use a bounded power-of-two texture bucket.");
  let pixels = 0;
  for (let edge = size; edge >= 1; edge /= 2) pixels += edge * edge;
  return pixels * layers * 4;
}
/** A material keeps all maps of one role together; unrelated materials keep their own size class. */
export function planMaterialTexturePools(
  requirements: ReadonlyMap<string, readonly LayerTexture[]>,
  dimensions: ReadonlyMap<string, TextureImageSize>,
  limits: TexturePoolLimits,
  options: TexturePoolBudgets = {},
): MaterialTexturePoolPlan {
  const requests = copyTextureRequirements(requirements),
    budgets = texturePoolBudgets(options);
  if (
    ![limits.maxTextureArrayLayers, limits.maxTextureDimension2D].every(
      (v) => Number.isSafeInteger(v) && v >= 1,
    )
  )
    throw new Error("The graphics device has invalid texture limits.");
  const urls = [
    ...new Set(
      [...requests.values()].flatMap((refs) => refs.map((r) => r.url)),
    ),
  ].sort();
  let decodedBytes = 0;
  for (const url of urls) {
    const image = dimensions.get(url);
    if (
      !image ||
      ![image.width, image.height].every(
        (v) =>
          Number.isSafeInteger(v) &&
          v >= 1 &&
          v <= 4096 &&
          v <= limits.maxTextureDimension2D,
      )
    )
      throw new Error(
        "A prepared map has unsupported dimensions or is missing.",
      );
    decodedBytes += image.width * image.height * 4;
    if (decodedBytes > budgets.maxDecodedBytes)
      throw new Error(
        "Decoded material images exceed the preparation byte budget.",
      );
  }
  const groups = new Map<
    string,
    { role: LayerTextureRole; size: number; urls: Set<string> }
  >();
  for (const role of TEXTURE_POOL_ROLES)
    groups.set(`${role}:1`, { role, size: 1, urls: new Set() });
  const materials = new Map<
    string,
    Readonly<Record<LayerTextureRole, string>>
  >();
  for (const [id, refs] of requests) {
    const selected = { color: "color:1", normal: "normal:1", data: "data:1" };
    for (const role of TEXTURE_POOL_ROLES) {
      const maps = refs.filter((ref) => ref.role === role);
      if (!maps.length) continue;
      const edge = Math.max(
        ...maps.map((ref) =>
          Math.max(
            dimensions.get(ref.url)!.width,
            dimensions.get(ref.url)!.height,
          ),
        ),
      );
      const size = Math.min(2048, 2 ** Math.ceil(Math.log2(edge))),
        key = `${role}:${size}`;
      if (!groups.has(key)) groups.set(key, { role, size, urls: new Set() });
      for (const ref of maps) groups.get(key)!.urls.add(ref.url);
      selected[role] = key;
    }
    materials.set(id, Object.freeze(selected));
  }
  let bytes = 0;
  const buckets = [...groups]
    .sort(
      ([, a], [, b]) =>
        TEXTURE_POOL_ROLES.indexOf(a.role) -
          TEXTURE_POOL_ROLES.indexOf(b.role) || a.size - b.size,
    )
    .map(([key, group]) => {
      if (group.urls.size + 1 > limits.maxTextureArrayLayers)
        throw new Error(
          "A material texture bucket exceeds the device array-layer limit.",
        );
      const allocation = textureBucketBytes(group.size, group.urls.size + 1);
      bytes += allocation;
      if (bytes > budgets.maxGpuBytes)
        throw new Error("Material texture buckets exceed the GPU byte budget.");
      return {
        key,
        role: group.role,
        size: group.size,
        mipLevelCount: Math.log2(group.size) + 1,
        urls: [...group.urls].sort(),
        bytes: allocation,
      };
    });
  // Source upload textures are reused across size buckets, but color and linear
  // roles need distinct GPU formats. Include their lifetime in the peak budget.
  const roleImages = new Set(
    [...requests.values()].flatMap((refs) =>
      refs.map((r) => `${r.role}\0${r.url}`),
    ),
  );
  let uploadBytes = 3 * 4;
  for (const key of roleImages) {
    const image = dimensions.get(key.slice(key.indexOf("\0") + 1))!;
    uploadBytes += image.width * image.height * 4;
  }
  const peakGpuBytes = bytes + uploadBytes;
  if (peakGpuBytes > budgets.maxGpuBytes)
    throw new Error(
      "Material texture arrays and uploads exceed the GPU byte budget.",
    );
  return {
    buckets,
    materials,
    urls,
    bytes,
    decodedBytes,
    uploadBytes,
    peakGpuBytes,
  };
}
