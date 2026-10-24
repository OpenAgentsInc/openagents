import { decodeRgba8 } from "../assets/src/rgba.ts";
import { readAsset } from "../assets/src/asset-download.ts";
import type { LayerTexture, LayerTextureRole } from "./layers.ts";
import {
  copyTextureRequirements,
  planMaterialTexturePools,
  texturePoolBudgets,
  type MaterialTexturePoolPlan,
  type TextureImageSize,
  type TexturePoolBudgets,
} from "./texture-pool-plan.ts";
export {
  planMaterialTexturePools,
  textureBucketBytes,
} from "./texture-pool-plan.ts";
export type { TexturePoolBudgets } from "./texture-pool-plan.ts";

type DecodedImage = TextureImageSize & {
  pixels?: Uint8Array;
  bitmap?: ImageBitmap;
};
const MAX_IMAGE_BYTES = 4096 * 4096 * 4 + 16;
async function boundedImage(response: Response): Promise<ArrayBuffer> {
  const declared = Number(response.headers.get("content-length"));
  if (declared > MAX_IMAGE_BYTES)
    throw new Error("A material image exceeds its input byte limit.");
  if (!response.body) {
    const bytes = await response.arrayBuffer();
    if (bytes.byteLength > MAX_IMAGE_BYTES)
      throw new Error("A material image exceeds its input byte limit.");
    return bytes;
  }
  const reader = response.body.getReader(),
    chunks: Uint8Array[] = [];
  let length = 0;
  try {
    for (;;) {
      const item = await reader.read();
      if (item.done) break;
      length += item.value.byteLength;
      if (length > MAX_IMAGE_BYTES)
        throw new Error("A material image exceeds its input byte limit.");
      chunks.push(item.value);
    }
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes.buffer;
}
function checkDimensions(
  width: number,
  height: number,
  device: GPUDevice,
): void {
  if (
    ![width, height].every(
      (value) =>
        Number.isInteger(value) &&
        value >= 1 &&
        value <= 4096 &&
        value <= device.limits.maxTextureDimension2D,
    )
  )
    throw new Error("A prepared material map has unsupported dimensions.");
}
function closeImages(images: ReadonlyMap<string, DecodedImage>) {
  for (const image of images.values()) image.bitmap?.close();
}
async function decodeImages(
  device: GPUDevice,
  urls: readonly string[],
  options: TexturePoolBudgets,
): Promise<Map<string, DecodedImage>> {
  const images = new Map<string, DecodedImage>(),
    queue = [...urls],
    budget = texturePoolBudgets(options).maxDecodedBytes;
  let reserved = 0,
    stopped = false;
  const reserve = (width: number, height: number) => {
    checkDimensions(width, height, device);
    reserved += width * height * 4;
    if (reserved > budget)
      throw new Error(
        "Decoded material images exceed the preparation byte budget.",
      );
  };
  const workers = await Promise.allSettled(
    Array.from({ length: Math.min(6, queue.length) }, async () => {
      try {
        while (!stopped) {
          const url = queue.shift();
          if (url === undefined) break;
          const bytes = await readAsset(url, async (response) => {
            if (!response.ok)
              throw new Error(
                "Prepare the private layered material maps before opening this world.",
              );
            return boundedImage(response);
          });
          const locationBase = globalThis.location?.href ?? "http://localhost/";
          if (new URL(url, locationBase).pathname.endsWith(".bin")) {
            const image = decodeRgba8(bytes);
            reserve(image.width, image.height);
            images.set(url, image);
          } else {
            // PNG dimensions are checked before browser decoding. Other supported bitmap
            // formats are checked immediately afterward and closed on rejection.
            const header = new DataView(bytes);
            let known = false;
            if (
              bytes.byteLength >= 24 &&
              header.getUint32(0) === 0x89504e47 &&
              header.getUint32(4) === 0x0d0a1a0a
            ) {
              reserve(header.getUint32(16), header.getUint32(20));
              known = true;
            }
            const bitmap = await createImageBitmap(new Blob([bytes]), {
              colorSpaceConversion: "none",
              premultiplyAlpha: "none",
            });
            try {
              if (!known) reserve(bitmap.width, bitmap.height);
              checkDimensions(bitmap.width, bitmap.height, device);
              images.set(url, {
                width: bitmap.width,
                height: bitmap.height,
                bitmap,
              });
            } catch (error) {
              bitmap.close();
              throw error;
            }
          }
        }
      } catch (error) {
        stopped = true;
        throw error;
      }
    }),
  );
  const failure = workers.find((item) => item.status === "rejected");
  if (failure?.status === "rejected") {
    closeImages(images);
    throw failure.reason;
  }
  return images;
}

/** One typed role array. The owner must destroy its texture exactly once. */
export class TexturePool {
  readonly indices = new Map<string, number>();
  private disposed = false;
  constructor(
    readonly texture: GPUTexture,
    readonly bytes: number,
    readonly role: LayerTextureRole,
    readonly size: number,
    readonly mipLevelCount: number,
  ) {}
  index(url: string): number {
    if (this.disposed)
      throw new Error("This material texture pool has been disposed.");
    const index = this.indices.get(url);
    if (index === undefined)
      throw new Error("A layered material map was not prepared.");
    return index;
  }
  dispose(): void {
    if (!this.disposed) {
      this.disposed = true;
      this.texture.destroy();
    }
  }
  /** Backward-compatible single-role pool; callers may continue owning texture directly. */
  static async create(
    device: GPUDevice,
    role: LayerTextureRole,
    urls: readonly string[],
    options: TexturePoolBudgets = {},
  ): Promise<TexturePool> {
    const requests = new Map([["single", urls.map((url) => ({ role, url }))]]);
    const result = await preparePools(device, requests, options, role);
    return result.pools.get(result.plan.materials.get("single")![role])!;
  }
}
export type MaterialRolePools = Readonly<Record<LayerTextureRole, TexturePool>>;
/** Owns all bucket textures. Renderer must not also track them as independent textures. */
export class MaterialTexturePools {
  private disposed = false;
  private constructor(
    private readonly pools: Map<string, TexturePool>,
    private readonly plan: MaterialTexturePoolPlan,
    private readonly requirements: ReadonlyMap<string, readonly LayerTexture[]>,
  ) {}
  get bytes(): number {
    return this.plan.bytes;
  }
  /** Decoded RGBA bytes retained during preparation; CPU images are released before return. */
  get uploadBytes(): number {
    return this.plan.uploadBytes;
  }
  get decodedBytes(): number {
    return this.plan.decodedBytes;
  }
  get peakGpuBytes(): number {
    return this.plan.peakGpuBytes;
  }
  forMaterial(
    id: string,
    requirements?: readonly LayerTexture[],
  ): MaterialRolePools {
    if (this.disposed)
      throw new Error("These material texture pools have been disposed.");
    const known = this.requirements.get(id);
    if (requirements) {
      const actual = copyTextureRequirements(new Map([[id, requirements]])).get(
        id,
      )!;
      const signature = (refs: readonly LayerTexture[]) =>
        JSON.stringify(refs.map((r) => `${r.role}\0${r.url}`).sort());
      if (
        (!known && actual.length) ||
        (known && signature(actual) !== signature(known))
      )
        throw new Error(
          "Prepare the material texture requirements before using this material.",
        );
    }
    const selected = this.plan.materials.get(id) ?? {
      color: "color:1",
      normal: "normal:1",
      data: "data:1",
    };
    return Object.freeze({
      color: this.pools.get(selected.color)!,
      normal: this.pools.get(selected.normal)!,
      data: this.pools.get(selected.data)!,
    });
  }
  dispose(): void {
    if (!this.disposed) {
      this.disposed = true;
      for (const pool of this.pools.values()) pool.dispose();
    }
  }
  static async create(
    device: GPUDevice,
    requirements: ReadonlyMap<string, readonly LayerTexture[]>,
    options: TexturePoolBudgets = {},
  ): Promise<MaterialTexturePools> {
    const requests = copyTextureRequirements(requirements),
      result = await preparePools(device, requests, options);
    return new MaterialTexturePools(result.pools, result.plan, requests);
  }
}

const BLIT = `@group(0)@binding(0)var image:texture_2d<f32>;@group(0)@binding(1)var filtering:sampler;struct O{@builtin(position)p:vec4<f32>,@location(0)uv:vec2<f32>};@vertex fn v(@builtin(vertex_index)i:u32)->O{let q=vec2(f32((i<<1u)&2u),f32(i&2u));var o:O;o.p=vec4(q*2.0-1.0,0.0,1.0);o.uv=vec2(q.x,1.0-q.y);return o;}@fragment fn f(i:O)->@location(0)vec4<f32>{return textureSample(image,filtering,i.uv);}`;
async function preparePools(
  device: GPUDevice,
  requirements: ReadonlyMap<string, readonly LayerTexture[]>,
  options: TexturePoolBudgets,
  onlyRole?: LayerTextureRole,
) {
  const requests = copyTextureRequirements(requirements),
    urls = [
      ...new Set(
        [...requests.values()].flatMap((refs) => refs.map((r) => r.url)),
      ),
    ];
  texturePoolBudgets(options);
  const images = await decodeImages(device, urls, options),
    created = new Set<GPUTexture>(),
    retained = new Set<GPUTexture>(),
    pools = new Map<string, TexturePool>();
  let scopes = 0;
  try {
    const plan = planMaterialTexturePools(
      requests,
      images,
      device.limits,
      options,
    );
    device.pushErrorScope("validation");
    scopes++;
    device.pushErrorScope("out-of-memory");
    scopes++;
    const pipelines = new Map<GPUTextureFormat, Promise<GPURenderPipeline>>(),
      sourceTextures = new Map<string, GPUTexture>();
    const allocate = (descriptor: GPUTextureDescriptor) => {
      const texture = device.createTexture(descriptor);
      created.add(texture);
      return texture;
    };
    const sampler = device.createSampler({
      minFilter: "linear",
      magFilter: "linear",
    });
    const source = (
      role: LayerTextureRole,
      url: string | undefined,
      format: GPUTextureFormat,
    ) => {
      const key = `${role}\0${url ?? ""}`,
        existing = sourceTextures.get(key);
      if (existing) return existing;
      const image = url ? images.get(url)! : undefined,
        width = image?.width ?? 1,
        height = image?.height ?? 1;
      const texture = allocate({
        label: `Source ${role} material image`,
        size: [width, height],
        format,
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      });
      if (image?.bitmap)
        device.queue.copyExternalImageToTexture(
          { source: image.bitmap },
          { texture, premultipliedAlpha: false },
          [width, height],
        );
      else
        device.queue.writeTexture(
          { texture },
          image?.pixels ??
            new Uint8Array(
              role === "normal"
                ? [128, 128, 255, 255]
                : role === "data"
                  ? [0, 255, 255, 255]
                  : [255, 255, 255, 255],
            ),
          { bytesPerRow: width * 4 },
          [width, height],
        );
      sourceTextures.set(key, texture);
      return texture;
    };
    for (const bucket of plan.buckets) {
      if (onlyRole && bucket.key !== plan.materials.get("single")![onlyRole])
        continue;
      const format: GPUTextureFormat =
        bucket.role === "color" ? "rgba8unorm-srgb" : "rgba8unorm";
      const texture = allocate({
        label: `Layer ${bucket.key} maps`,
        size: [bucket.size, bucket.size, bucket.urls.length + 1],
        mipLevelCount: bucket.mipLevelCount,
        format,
        usage:
          GPUTextureUsage.TEXTURE_BINDING |
          GPUTextureUsage.COPY_DST |
          GPUTextureUsage.RENDER_ATTACHMENT,
      });
      if (!pipelines.has(format))
        pipelines.set(
          format,
          device.createRenderPipelineAsync({
            layout: "auto",
            vertex: {
              module: device.createShaderModule({ code: BLIT }),
              entryPoint: "v",
            },
            fragment: {
              module: device.createShaderModule({ code: BLIT }),
              entryPoint: "f",
              targets: [{ format }],
            },
          }),
        );
      const pipeline = await pipelines.get(format)!,
        encoder = device.createCommandEncoder();
      const blit = (src: GPUTextureView, dst: GPUTextureView) => {
        const group = device.createBindGroup({
          layout: pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: src },
            { binding: 1, resource: sampler },
          ],
        });
        const pass = encoder.beginRenderPass({
          colorAttachments: [{ view: dst, loadOp: "clear", storeOp: "store" }],
        });
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, group);
        pass.draw(3);
        pass.end();
      };
      const pool = new TexturePool(
        texture,
        bucket.bytes,
        bucket.role,
        bucket.size,
        bucket.mipLevelCount,
      );
      for (let layer = 0; layer <= bucket.urls.length; layer++) {
        const url = bucket.urls[layer - 1];
        const view = (mip: number) =>
          texture.createView({
            dimension: "2d",
            baseArrayLayer: layer,
            arrayLayerCount: 1,
            baseMipLevel: mip,
            mipLevelCount: 1,
          });
        blit(source(bucket.role, url, format).createView(), view(0));
        for (let mip = 1; mip < bucket.mipLevelCount; mip++)
          blit(view(mip - 1), view(mip));
        if (url) pool.indices.set(url, layer);
      }
      device.queue.submit([encoder.finish()]);
      pools.set(bucket.key, pool);
      retained.add(texture);
    }
    await device.queue.onSubmittedWorkDone();
    const errors: GPUError[] = [];
    while (scopes) {
      scopes--;
      const error = await device.popErrorScope();
      if (error) errors.push(error);
    }
    if (errors.length)
      throw new Error(
        `Material texture preparation failed: ${errors.map((e) => e.message).join("; ")}`,
      );
    for (const texture of created)
      if (!retained.has(texture)) texture.destroy();
    return { plan, pools };
  } catch (error) {
    for (const texture of created) texture.destroy();
    throw error;
  } finally {
    while (scopes) {
      scopes--;
      await device.popErrorScope().catch(() => null);
    }
    closeImages(images);
  }
}
