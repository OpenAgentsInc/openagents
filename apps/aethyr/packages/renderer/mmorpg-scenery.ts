import type { CompiledInstance, CompiledScene } from "../assets/src/scene.ts";
import type {
  MmorpgWorldManifest,
  MmorpgSceneryPlacement,
} from "../assets/src/mmorpg.ts";
import { mmorpgHeight, sampleMmorpgNoise } from "../runtime/mmorpg-terrain.ts";
import { modelMatrix } from "./math.ts";
const noiseParams = {
  seed: 2 as const,
  octaves: 1,
  persistence: 0.5,
  lacunarity: 2,
  exponentiation: 1,
  scale: 1,
  height: 1,
};
const biomeParams = {
  ...noiseParams,
  octaves: 2,
  exponentiation: 2,
  scale: 1024,
};
export function mmorpgSceneryCenter(
  position: readonly number[],
): [number, number] {
  return [
    Math.round((position[0] * 6) / 50),
    Math.round((position[2] * 6) / 50),
  ];
}
/** The original bounded scenery recipe, with source seed2 and shared terrain heights. */
export function mmorpgSceneryPlacements(
  world: MmorpgWorldManifest,
  center: readonly number[],
): MmorpgSceneryPlacement[] {
  const [cx, cz] = mmorpgSceneryCenter(center),
    out: MmorpgSceneryPlacement[] = [];
  const n = (x: number, y: number, z: number) =>
    sampleMmorpgNoise(noiseParams, x, y, z);
  for (let x = -10; x <= 10; x++)
    for (let z = -10; z <= 10; z++) {
      let px = (x + cx) * 50,
        pz = (z + cz) * 50;
      const vx = px,
        vz = pz;
      px += (n(px, 0, pz) * 2 - 1) * 25;
      pz += (n(px, 1, pz) * 2 - 1) * 25;
      const height = mmorpgHeight(px / 6, pz / 6),
        moisture = sampleMmorpgNoise(biomeParams, px, 0, pz);
      const biome =
        (height * 6) / 100 < 0.05
          ? "desert"
          : moisture > 0.5
            ? "forest"
            : "arid";
      if (n(vx, 2, vz) > { desert: 0.1, forest: 0.8, arid: 0.6 }[biome])
        continue;
      const options = world.scenery.catalog.filter((d) =>
        d.biomes.includes(biome),
      );
      const model = options[Math.round(n(px, 3, pz) * (options.length - 1))];
      out.push({
        kind: model.id,
        cell: [x + cx, z + cz],
        position: [px / 6, height, pz / 6],
        yaw: n(px, 5, pz) * 360,
        scale: model.scale * (0.8 + n(px, 4, pz) * 0.4),
      });
    }
  return out;
}
export interface MmorpgSceneryMetadata {
  firstInstance: number;
  firstMesh: number;
  firstVertex: number;
  firstIndex: number;
}
export function mmorpgSceneryMetadata(
  scene: CompiledScene,
): MmorpgSceneryMetadata | undefined {
  return (
    scene.source as CompiledScene["source"] & {
      mmorpgScenery?: MmorpgSceneryMetadata;
    }
  ).mmorpgScenery;
}
export function mmorpgSceneryInstances(
  world: MmorpgWorldManifest,
  placements: readonly MmorpgSceneryPlacement[],
  firstMesh: number,
): CompiledInstance[] {
  const ordered = [...placements].sort(
    (a, b) =>
      world.scenery.catalog.findIndex((d) => d.id === a.kind) -
      world.scenery.catalog.findIndex((d) => d.id === b.kind),
  );
  return ordered.map((p) => ({
    mesh: firstMesh + world.scenery.catalog.findIndex((d) => d.id === p.kind),
    model: modelMatrix(p.position, p.yaw, p.scale),
    materialOverrides: [],
    actor: "Scenery " + p.cell.join(":"),
    component: p.kind,
    sourceClass: "MmorpgDecoration",
  }));
}
export function mmorpgSceneBatches(instances: readonly CompiledInstance[]) {
  const result: CompiledScene["batches"] = [];
  instances.forEach((instance, i) => {
    const last = result.at(-1);
    if (
      last &&
      last.mesh === instance.mesh &&
      last.firstInstance + last.instanceCount === i
    )
      last.instanceCount++;
    else
      result.push({
        mesh: instance.mesh,
        materialOverrides: [],
        firstInstance: i,
        instanceCount: 1,
      });
  });
  return result;
}
