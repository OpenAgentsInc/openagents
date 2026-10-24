import { planarReceiverScissor } from "./planar-scissor.ts";
import { prepareCanonicalSourceIndices } from "./canonical-source-indices.ts";
import { EnemyCharacters } from "./enemy-characters.ts";
import { CharacterRenderer } from "./character-renderer.ts";
import { loadCharacterManifest } from "./character.ts";
import { sourceSkyDome } from "./sky.ts";
import { SKY_COMMON_WGSL, SKY_BACKGROUND_WGSL } from "./sky-shader.ts";
import type { RendererFeatures } from "./feature-contract.ts";

/** Full graphics without editor experiments, UI, or agent tooling. */
export const selectedRendererFeatures: RendererFeatures = {
  prepareCanonicalSourceIndices,
  planarScissor: planarReceiverScissor,
  character: {
    create: CharacterRenderer.create,
    load: loadCharacterManifest,
    createEnemies: EnemyCharacters.create,
    createMmorpg: async (...args) =>
      (await import("./mmorpg-characters.ts")).MmorpgCharacters.create(...args),
    createMmorpgEffects: async (...args) =>
      (await import("./mmorpg-effects.ts")).MmorpgEffects.create(...args),
    createMmorpgTerrain: async (...args) =>
      (await import("./mmorpg-terrain.ts")).MmorpgTerrain.create(...args),
    createMmorpgNames: async (...args) =>
      (await import("./mmorpg-names.ts")).MmorpgNames.create(...args),
  },
  sky: {
    commonWGSL: SKY_COMMON_WGSL,
    backgroundWGSL: SKY_BACKGROUND_WGSL,
    sourceDome: sourceSkyDome,
  },
};
