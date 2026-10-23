import type { OwnedAssetDescription as AssetDescription } from "./asset-import-protocol.ts";
import { copyImportedAsset } from "./asset-import-protocol.ts";

export type ImportedMesh = { id: string; vertex: GPUBuffer; index: GPUBuffer; indexCount: number;
  attributes: Partial<Record<"normal" | "color" | "glow" | "uv", GPUBuffer>>; material?: string };
export type ImportedImage = { id: string; texture: GPUTexture; width: number; height: number };
export type PreparedAsset = {
  description: AssetDescription;
  meshes: ImportedMesh[];
  images: ImportedImage[];
  dispose(): void;
};
export interface AssetRuntime {
  prepare(description: AssetDescription): Promise<PreparedAsset>;
  commit(id: string, revision: number, asset: PreparedAsset): void;
  get(id: string): { revision: number; description: AssetDescription } | undefined;
  dispose(): void;
}

// Runtime handles stay here. Source files and saved projects contain no GPU objects.
export class AssetRuntimeAdapter implements AssetRuntime {
  private readonly device: GPUDevice;
  private readonly accepted = new Map<string, { revision: number; asset: PreparedAsset }>();
  private readonly owned = new Set<PreparedAsset>();
  private closed = false;

  constructor(device: GPUDevice) { this.device = device; }

  async prepare(description: AssetDescription): Promise<PreparedAsset> {
    if (this.closed) throw new Error("Reopen the asset preview to import this file.");
    const meshes: ImportedMesh[] = [], images: ImportedImage[] = [];
    const buffers: GPUBuffer[] = [], textures: GPUTexture[] = [];
    let disposed = false;
    const asset: PreparedAsset = { description: copyImportedAsset(description), meshes, images, dispose: () => {
      if (disposed) return;
      disposed = true;
      for (const buffer of buffers) buffer.destroy();
      for (const texture of textures) texture.destroy();
      this.owned.delete(asset);
    } };
    this.owned.add(asset);
    this.device.pushErrorScope("out-of-memory");
    this.device.pushErrorScope("validation");
    let failure: unknown;
    try {
      for (const mesh of asset.description.meshes) {
        const vertex = this.device.createBuffer({ size: mesh.positions.byteLength,
          usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC });
        buffers.push(vertex);
        const index = this.device.createBuffer({ size: mesh.indices.byteLength,
          usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC });
        buffers.push(index);
        this.device.queue.writeBuffer(vertex, 0, mesh.positions.buffer as ArrayBuffer, mesh.positions.byteOffset, mesh.positions.byteLength);
        this.device.queue.writeBuffer(index, 0, mesh.indices.buffer as ArrayBuffer, mesh.indices.byteOffset, mesh.indices.byteLength);
        const attributes: ImportedMesh["attributes"] = {};
        for (const [name, data] of [["normal", mesh.normals], ["color", mesh.colors], ["glow", mesh.glow], ["uv", mesh.uvs]] as const) {
          if (!data) continue;
          const buffer = this.device.createBuffer({ size: data.byteLength,
            usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC });
          buffers.push(buffer);
          this.device.queue.writeBuffer(buffer, 0, data.buffer as ArrayBuffer, data.byteOffset, data.byteLength);
          attributes[name] = buffer;
        }
        meshes.push({ id: mesh.id, vertex, index, indexCount: mesh.indexCount, attributes,
          ...(mesh.material !== undefined ? { material: mesh.material } : {}) });
      }
      for (const image of asset.description.images) {
        const texture = this.device.createTexture({ size: [image.width, image.height], format: "rgba8unorm",
          usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC });
        textures.push(texture);
        this.device.queue.writeTexture({ texture }, image.bytes as Uint8Array<ArrayBuffer>,
          { bytesPerRow: image.width * 4, rowsPerImage: image.height }, [image.width, image.height]);
        images.push({ id: image.id, texture, width: image.width, height: image.height });
      }
    } catch (error) { failure = error; }
    // Pop both scopes before yielding, so parallel imports cannot interleave them.
    const scopes = [this.device.popErrorScope(), this.device.popErrorScope()];
    try {
      const errors = await Promise.all(scopes);
      if (failure || errors.some(Boolean) || this.closed || disposed) {
        throw failure ?? new Error("The asset could not be uploaded. The previous preview is still selected.");
      }
      return asset;
    } catch (error) {
      asset.dispose();
      throw error;
    }
  }

  commit(id: string, revision: number, asset: PreparedAsset): void {
    if (this.closed || !this.owned.has(asset) || asset.description.id !== id) throw new Error("The asset preview has closed.");
    const previous = this.accepted.get(id);
    this.accepted.set(id, { revision, asset });
    if (previous?.asset !== asset) previous?.asset.dispose();
  }

  get(id: string): { revision: number; description: AssetDescription } | undefined {
    const value = this.accepted.get(id);
    return value && { revision: value.revision, description: copyImportedAsset(value.asset.description) };
  }

  resources(id: string): { revision: number; meshes: readonly ImportedMesh[]; images: readonly ImportedImage[] } | undefined {
    const value = this.accepted.get(id);
    return value && { revision: value.revision, meshes: value.asset.meshes, images: value.asset.images };
  }

  stats(): { assets: number; buffers: number; textures: number } {
    let buffers = 0, textures = 0;
    for (const value of this.owned) {
      for (const mesh of value.meshes) buffers += 2 + Object.keys(mesh.attributes).length;
      textures += value.images.length;
    }
    return { assets: this.accepted.size, buffers, textures };
  }

  dispose(): void {
    this.closed = true;
    for (const value of [...this.owned]) value.dispose();
    this.accepted.clear();
  }
}
