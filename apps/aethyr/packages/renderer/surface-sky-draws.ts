import type { CompiledScene } from "../assets/src/scene.ts";
import type { WindDraw } from "./wind-source.ts";
import { surfaceSkySelection, type SurfaceSkyManifest } from "./surface-sky.ts";

const bindingError = () =>
  Error("Prepare the selected objects’ surface lighting again.");
const primitiveKey = (p: {
  firstIndex: number;
  indexCount: number;
  baseVertex: number;
  material: number;
}) => [p.firstIndex, p.indexCount, p.baseVertex, p.material].join(":");

/** Match sourceDraws' effective material, including its invalid-index fallback. */
function renderMaterial(
  scene: CompiledScene,
  overrides: readonly number[],
  primitive: CompiledScene["meshes"][number]["primitives"][number],
) {
  const override = overrides[primitive.materialSlot];
  const candidate =
    override !== undefined && override >= 0 ? override : primitive.material;
  return candidate >= 0 && candidate < scene.materials.length ? candidate : 0;
}

/** Only selected source instances need this derived-batch consistency check. */
function selectedBindings(scene: CompiledScene, manifest: SurfaceSkyManifest) {
  const result = new Map<
    number,
    {
      primitives: Set<string>;
      selected: Map<string, number>;
      seen: Map<string, number>;
    }
  >();
  for (const selection of manifest.selections) {
    const matches = scene.instances.flatMap((instance, i) =>
      instance.actor === selection.actor &&
      instance.component === selection.component
        ? [i]
        : [],
    );
    if (matches.length !== 1) throw bindingError();
    const index = matches[0],
      instance = scene.instances[index],
      mesh = scene.meshes[instance.mesh];
    if (!mesh || mesh.id !== selection.mesh) throw bindingError();
    const batches = scene.batches.filter(
      (batch) =>
        Number.isInteger(batch.firstInstance) &&
        Number.isInteger(batch.instanceCount) &&
        batch.firstInstance >= 0 &&
        batch.instanceCount > 0 &&
        batch.firstInstance <= index &&
        index < batch.firstInstance + batch.instanceCount,
    );
    if (
      batches.length !== 1 ||
      batches[0].mesh !== instance.mesh ||
      batches[0].firstInstance + batches[0].instanceCount >
        scene.instances.length
    )
      throw bindingError();
    const primitives = new Set<string>(),
      selected = new Map<string, number>();
    for (const primitive of mesh.primitives) {
      const material = renderMaterial(
        scene,
        instance.materialOverrides,
        primitive,
      );
      if (
        material !==
        renderMaterial(scene, batches[0].materialOverrides, primitive)
      )
        throw bindingError();
      const key = primitiveKey({ ...primitive, material });
      primitives.add(key);
      if (selection.materialIds.includes(scene.materials[material]?.id))
        selected.set(key, (selected.get(key) ?? 0) + 1);
    }
    result.set(index, { primitives, selected, seen: new Map() });
  }
  return result;
}

/** Split shared instancing batches without changing source instance addresses. */
export function surfaceSkyDraws(
  scene: CompiledScene,
  draws: readonly WindDraw[],
  manifest: SurfaceSkyManifest | undefined,
  material: (original: number, selection: number) => number,
): WindDraw[] {
  if (!manifest?.scenarios[scene.scenario]) return [...draws];
  const result: WindDraw[] = [];
  const bindings = selectedBindings(scene, manifest);
  const bound = manifest.selections.map(() => new Set<string>());
  for (const draw of draws) {
    let start = draw.firstInstance;
    let previous = -1;
    const end = start + draw.count;
    for (let i = start; i <= end; i++) {
      const instance = scene.instances[i];
      if (i < end && bindings.has(i)) {
        const expected = bindings.get(i)!,
          key = primitiveKey(draw);
        if (!expected.primitives.has(key)) throw bindingError();
        if (expected.selected.has(key)) {
          const count = (expected.seen.get(key) ?? 0) + 1;
          if (count > expected.selected.get(key)!) throw bindingError();
          expected.seen.set(key, count);
        }
      }
      const selection =
        i < end
          ? surfaceSkySelection(
              manifest,
              instance.actor,
              instance.component,
              scene.materials[draw.material].id,
            )
          : -1;
      if (selection > 0 && draw.wind)
        throw Error("Prepare static surface lighting for a still object.");
      if (selection > 0)
        bound[selection - 1].add(scene.materials[draw.material].id);
      if (i === end || (previous >= 0 && previous !== selection)) {
        if (i > start)
          result.push({
            ...draw,
            firstInstance: start,
            count: i - start,
            material:
              previous > 0 ? material(draw.material, previous) : draw.material,
          });
        start = i;
      }
      previous = selection;
    }
  }
  if (
    [...bindings.values()].some((binding) =>
      [...binding.selected].some(
        ([key, count]) => binding.seen.get(key) !== count,
      ),
    ) ||
    manifest.selections.some((selection, i) =>
      selection.materialIds.some((id) => !bound[i].has(id)),
    )
  )
    throw bindingError();
  return result;
}
