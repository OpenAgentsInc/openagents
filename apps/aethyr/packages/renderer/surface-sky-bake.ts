import type { CompiledScene } from "../assets/src/scene.ts";
import {
  SURFACE_SKY_LIMITS,
  surfaceSkyMaterialIndex,
  type SurfaceSkyPoint,
  type SurfaceSkySelection,
} from "./surface-sky.ts";
import { cross, normalize, sub, type V3 } from "./math.ts";
export interface SurfaceSkySelector {
  actor: string;
  component: string;
  materialIds: string[];
}
/** Selects static source geometry; no camera or GI-grid coordinates enter this mapping. */
export function buildSurfaceSkySamples(
  scene: CompiledScene,
  vertices: Float32Array,
  indices: Uint32Array,
  selectors: SurfaceSkySelector[],
) {
  if (
    !Array.isArray(selectors) ||
    !selectors.length ||
    selectors.length > SURFACE_SKY_LIMITS.selections ||
    vertices.length !== scene.vertexCount * 12 ||
    indices.length !== scene.indexCount ||
    !vertices.every(Number.isFinite)
  )
    throw Error(
      "Surface sky preparation needs bounded source geometry and selections.",
    );
  if (
    new Set(selectors.map((s) => s.actor + "\0" + s.component)).size !==
    selectors.length
  )
    throw Error("The source surface selections repeat.");
  const selections: SurfaceSkySelection[] = [],
    points: Array<SurfaceSkyPoint | undefined> = [];
  for (const selector of selectors) {
    const matches = scene.instances.filter(
      (i) => i.actor === selector.actor && i.component === selector.component,
    );
    if (
      matches.length !== 1 ||
      !Array.isArray(selector.materialIds) ||
      !selector.materialIds.length ||
      new Set(selector.materialIds).size !== selector.materialIds.length
    )
      throw Error(
        "Select one source instance and distinct effective materials.",
      );
    const instance = matches[0],
      mesh = scene.meshes[instance.mesh],
      model = instance.model;
    if (
      !mesh ||
      model.length !== 16 ||
      !model.every(Number.isFinite) ||
      Math.abs(model[3]) +
        Math.abs(model[7]) +
        Math.abs(model[11]) +
        Math.abs(model[15] - 1) >
        1e-6
    )
      throw Error("Surface sky needs an unchanged affine source model.");
    const addressed = new Set<number>();
    const chosen = new Set(selector.materialIds),
      used = new Map<number, { normal: V3; area: number }>(),
      found = new Set<string>();
    function position(vertex: number): V3 {
      if (
        !Number.isInteger(vertex) ||
        vertex < 0 ||
        vertex >= scene.vertexCount
      )
        throw Error("Surface sky indices leave their vertex stream.");
      const p = vertex * 12,
        x = vertices[p],
        y = vertices[p + 1],
        z = vertices[p + 2];
      return [
        model[0] * x + model[4] * y + model[8] * z + model[12],
        model[1] * x + model[5] * y + model[9] * z + model[13],
        model[2] * x + model[6] * y + model[10] * z + model[14],
      ];
    }
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
      if (!material || !chosen.has(material.id)) continue;
      found.add(material.id);
      if (material.alphaMode === "BLEND" || material.glass || material.flame)
        throw Error(
          "Surface sky refinement supports opaque or masked diffuse surfaces.",
        );
      if (
        !Number.isInteger(primitive.firstIndex) ||
        primitive.firstIndex < 0 ||
        primitive.indexCount % 3 ||
        primitive.firstIndex + primitive.indexCount > indices.length
      )
        throw Error("Surface sky primitive indices are invalid.");
      for (
        let i = primitive.firstIndex;
        i < primitive.firstIndex + primitive.indexCount;
        i += 3
      ) {
        const ids = [indices[i], indices[i + 1], indices[i + 2]].map(
            (v) => v + primitive.baseVertex,
          ),
          p = ids.map(position),
          n = cross(sub(p[1], p[0]), sub(p[2], p[0])),
          area = Math.hypot(...n);
        ids.forEach((id) => addressed.add(id));
        if (addressed.size + points.length > SURFACE_SKY_LIMITS.vertices)
          throw Error("The selected surface vertices exceed the bake budget.");
        if (area < 1e-12) continue;
        for (const id of ids)
          if (area > (used.get(id)?.area ?? 0))
            used.set(id, { normal: normalize(n), area });
      }
    }
    if (found.size !== chosen.size || !used.size)
      throw Error(
        "A selected surface material has no supported source triangles.",
      );
    const ids = [...addressed].sort((a, b) => a - b),
      firstVertex = ids[0],
      last = ids.at(-1)!,
      sparse = last - firstVertex + 1 > ids.length,
      vertexCount = sparse ? ids.length : last - firstVertex + 1;
    if (vertexCount + points.length > SURFACE_SKY_LIMITS.vertices)
      throw Error("The selected surface vertex ranges exceed the bake budget.");
    const recordStart = points.length;
    for (const v of ids)
      points.push(
        used.has(v)
          ? { position: position(v), geometricNormal: used.get(v)!.normal }
          : undefined,
      );
    selections.push({
      actor: instance.actor,
      component: instance.component,
      mesh: mesh.id,
      model: [...model],
      materialIds: [...selector.materialIds],
      firstVertex,
      vertexCount,
      recordStart,
      ...(sparse ? { vertexIndices: ids } : {}),
    });
  }
  return { selections, points };
}
