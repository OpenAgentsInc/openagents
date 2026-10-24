import type { CompiledScene } from "../assets/src/scene.ts";
import { transformPoint, type V3 } from "./math.ts";

export interface SourceDraw {
  firstIndex: number;
  indexCount: number;
  baseVertex: number;
  firstInstance: number;
  count: number;
  material: number;
}

/** Unsupported source sky and fog graphs are replaced by the owned atmosphere. */
export function sourceDraws(scene: CompiledScene): SourceDraw[] {
  const draws: SourceDraw[] = [];
  for (const batch of scene.batches) {
    const mesh = scene.meshes[batch.mesh];
    if (/sky|fog|cloud/i.test(mesh.name)) continue;
    for (const primitive of mesh.primitives) {
      const override = batch.materialOverrides[primitive.materialSlot];
      const candidate =
        override !== undefined && override >= 0 ? override : primitive.material;
      const material =
        candidate >= 0 && candidate < scene.materials.length ? candidate : 0;
      if (
        /fog|cloud|sky|light.?beam|god.?ray/i.test(
          scene.materials[material].name,
        )
      )
        continue;
      let start = -1;
      const end = batch.firstInstance + batch.instanceCount;
      for (let i = batch.firstInstance; i <= end; i++) {
        const include =
          i < end &&
          !/fog.?card|sky.?sphere|cloud/i.test(scene.instances[i].actor);
        if (include && start < 0) start = i;
        if (!include && start >= 0) {
          draws.push({
            ...primitive,
            material,
            firstInstance: start,
            count: i - start,
          });
          start = -1;
        }
      }
    }
  }
  return draws;
}

/** Local-light positions inferred from emissive surfaces, not exported UE lights. */
export function emissiveAnchors(
  scene: CompiledScene,
  vertices: Float32Array,
  indices: Uint32Array,
): V3[] {
  const centers = new Map<string, V3>(),
    result: V3[] = [];
  for (const instance of scene.instances) {
    for (const p of scene.meshes[instance.mesh].primitives) {
      const override = instance.materialOverrides[p.materialSlot];
      const material =
        scene.materials[
          override !== undefined && override >= 0 ? override : p.material
        ];
      if (!material || Math.max(...material.emissiveFactor) <= 0) continue;
      const key = instance.mesh + ":" + p.firstIndex;
      let center = centers.get(key);
      if (!center) {
        const low: V3 = [Infinity, Infinity, Infinity],
          high: V3 = [-Infinity, -Infinity, -Infinity];
        for (let i = p.firstIndex; i < p.firstIndex + p.indexCount; i++) {
          const vertex = (indices[i] + p.baseVertex) * 12;
          for (let axis = 0; axis < 3; axis++) {
            low[axis] = Math.min(low[axis], vertices[vertex + axis]);
            high[axis] = Math.max(high[axis], vertices[vertex + axis]);
          }
        }
        center = low.map((v, i) => (v + high[i]) / 2) as V3;
        centers.set(key, center);
      }
      const world = transformPoint(instance.model, center);
      if (
        world.every(Number.isFinite) &&
        !result.some(
          (other) => Math.hypot(...other.map((v, i) => v - world[i])) < 0.35,
        )
      )
        result.push(world);
    }
  }
  return result;
}
