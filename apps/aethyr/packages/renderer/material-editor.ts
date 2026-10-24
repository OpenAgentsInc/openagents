import type { CompiledMaterial, CompiledScene } from "../assets/src/scene.ts";
import {
  checkMaterialDocument,
  savedMaterialDocuments,
  type MaterialBundle,
  type MaterialDocument,
} from "../runtime/material-source.ts";

export interface MaterialEditorPlan {
  ready(): boolean;
  remap(entity: string, material: number): number;
  activate(): void;
  discard(): void;
}
export interface MaterialEditorHost {
  scene: CompiledScene;
  device: GPUDevice;
  images(document: MaterialDocument): MaterialBundle | undefined;
  count(): number;
  material(
    source: CompiledMaterial,
    texture: GPUTexture,
    uniform: (buffer: GPUBuffer, original: Float32Array) => void,
  ): Promise<GPUBindGroup>;
  install(
    start: number,
    groups: GPUBindGroup[],
    sources: CompiledMaterial[],
  ): void;
  release(buffer?: GPUBuffer): void;
}
type Variant = {
  source: string;
  entity: string;
  materialId: string;
  group: GPUBindGroup;
  texture: GPUTexture;
  buffer: GPUBuffer;
  original: Float32Array;
};

/** Editor-owned material staging. The host keeps ownership of every uniform buffer. */
export class MaterialEditor {
  private start?: number;
  private variants: Variant[] = [];
  private pending = new Set<Variant>();
  private uniforms = new Map<
    string,
    { buffer: GPUBuffer; original: Float32Array }
  >();
  private closed = false;
  constructor(private readonly host: MaterialEditorHost) {}

  track(id: string, buffer: GPUBuffer, original: Float32Array) {
    this.uniforms.set(id, { buffer, original: original.slice() });
  }
  remap(entity: string, material: number) {
    const index = this.variants.findIndex(
      (variant) =>
        variant.entity === entity &&
        this.host.scene.materials[material]?.id === variant.materialId,
    );
    return index < 0 ? material : this.start! + index;
  }
  bindings(snapshot: any, meshes: Map<string, number>) {
    const bindings = new Map<string, Set<string>>();
    for (const entity of snapshot.entities ?? []) {
      const index = meshes.get(entity.kind);
      if (index === undefined) continue;
      for (const primitive of this.host.scene.meshes[index].primitives) {
        const id = this.host.scene.materials[primitive.material].id;
        if (!bindings.has(id)) bindings.set(id, new Set());
        bindings.get(id)!.add(entity.id);
      }
    }
    return [...bindings].map(([id, objects]) => ({
      id,
      objects: [...objects],
    }));
  }
  applyValues(values: Record<string, Record<string, number>>) {
    const offsets: Record<string, number> = {
      red: 0,
      green: 1,
      blue: 2,
      roughness: 8,
      metallic: 9,
      normal: 10,
    };
    for (const [id, { buffer, original }] of this.uniforms) {
      const next = original.slice();
      for (const [parameter, value] of Object.entries(values[id] ?? {}))
        next[offsets[parameter]] = value;
      this.host.device.queue.writeBuffer(buffer, 0, next);
    }
    for (const variant of this.variants) {
      const next = variant.original.slice();
      for (const [parameter, value] of Object.entries(
        values[variant.materialId] ?? {},
      ))
        next[offsets[parameter]] = value;
      this.host.device.queue.writeBuffer(variant.buffer, 0, next);
    }
  }
  async prepare(snapshot: any): Promise<MaterialEditorPlan> {
    if (this.closed)
      throw new Error("The world closed before its materials were ready.");
    const variants: Variant[] = [],
      added: Variant[] = [];
    this.start ??= this.host.count();
    const start = this.start;
    const discard = () => {
      for (const variant of added) {
        if (!this.pending.delete(variant)) continue;
        variant.texture.destroy();
        this.host.release(variant.buffer);
      }
    };
    try {
      for (const document of savedMaterialDocuments(snapshot)) {
        checkMaterialDocument(document, snapshot);
        const source = JSON.stringify(document);
        const reused = this.variants.find(
          (variant) => variant.source === source,
        );
        if (reused) {
          variants.push(reused);
          continue;
        }
        const meshKind = (snapshot.entities ?? []).find(
          (entity: any) => entity.id === document.target.entity,
        )?.kind;
        const material = this.host.scene.materials.find(
          (item) => item.id === document.target.material,
        );
        if (
          !material ||
          material.layers ||
          material.paint ||
          material.terrain ||
          material.vegetation ||
          material.glass ||
          material.flame ||
          meshKind !== document.target.asset
        )
          throw new Error("This world has no editable werewolf face material.");
        const bundle = this.host.images(document);
        if (!bundle)
          throw new Error("Reopen the face material with its source images.");
        const canvas = new OffscreenCanvas(256, 256),
          context = canvas.getContext("2d");
        if (!context) throw new Error("The face image could not be prepared.");
        for (const name of ["generated", "handmade"] as const) {
          const layer = document[name],
            image = bundle[name];
          if (!image || !layer.visible || layer.strength === 0) continue;
          const bytes = Uint8Array.from(atob(image.png_base64), (character) =>
            character.charCodeAt(0),
          );
          const bitmap = await createImageBitmap(
            new Blob([bytes], { type: "image/png" }),
          );
          try {
            if (bitmap.width !== 256 || bitmap.height !== 256)
              throw new Error("Use a 256 × 256 PNG for each face layer.");
            context.globalAlpha = layer.strength;
            context.drawImage(bitmap, 0, 0);
          } finally {
            bitmap.close();
          }
        }
        const texture = this.host.device.createTexture({
          label: `Creator face ${document.target.entity}`,
          size: [256, 256],
          format: "rgba8unorm-srgb",
          usage:
            GPUTextureUsage.TEXTURE_BINDING |
            GPUTextureUsage.COPY_DST |
            GPUTextureUsage.RENDER_ATTACHMENT,
        });
        let buffer: GPUBuffer | undefined, original: Float32Array | undefined;
        try {
          this.host.device.queue.copyExternalImageToTexture(
            { source: canvas },
            { texture, premultipliedAlpha: false },
            [256, 256],
          );
          const group = await this.host.material(
            material,
            texture,
            (created, values) => {
              buffer = created;
              original = values;
            },
          );
          if (this.closed)
            throw new Error(
              "The world closed before its materials were ready.",
            );
          const variant = {
            source,
            entity: document.target.entity,
            materialId: material.id,
            group,
            texture,
            buffer: buffer!,
            original: original!,
          };
          variants.push(variant);
          added.push(variant);
          this.pending.add(variant);
        } catch (error) {
          texture.destroy();
          this.host.release(buffer);
          throw error;
        }
      }
    } catch (error) {
      discard();
      throw error;
    }
    let used = false;
    const ready = () => !used && !this.closed;
    return {
      ready,
      remap: (entity, material) => {
        const index = variants.findIndex(
          (variant) =>
            variant.entity === entity &&
            this.host.scene.materials[material]?.id === variant.materialId,
        );
        return index < 0 ? material : start + index;
      },
      activate: () => {
        if (used) return;
        if (!ready())
          throw new Error("The world closed before its materials were ready.");
        used = true;
        const retired = this.variants.filter(
          (variant) => !variants.includes(variant),
        );
        this.host.install(
          start,
          variants.map((variant) => variant.group),
          variants.map((variant) =>
            this.host.scene.materials.find(
              (material) => material.id === variant.materialId,
            )!,
          ),
        );
        for (const variant of added) this.pending.delete(variant);
        this.variants = variants;
        const release = () => {
          for (const variant of retired) {
            variant.texture.destroy();
            this.host.release(variant.buffer);
          }
        };
        void this.host.device.queue
          .onSubmittedWorkDone()
          .then(release, release);
      },
      discard: () => {
        if (!used) {
          used = true;
          discard();
        }
      },
    };
  }
  dispose() {
    if (this.closed) return;
    this.closed = true;
    for (const variant of this.variants) variant.texture.destroy();
    for (const variant of this.pending) {
      variant.texture.destroy();
      this.host.release(variant.buffer);
    }
    this.pending.clear();
    this.variants = [];
    this.uniforms.clear();
  }
}
export function createMaterialEditor(host: MaterialEditorHost) {
  return new MaterialEditor(host);
}
