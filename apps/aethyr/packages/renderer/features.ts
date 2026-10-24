import { createCompiledSourceHost } from "./compiled-source-host.ts";
import { createSpellSwarmHost } from "./spell-swarm-host.ts";
import { selectedRendererFeatures as playerFeatures } from "./player-features.ts";
import { createRendererExperiment } from "./renderer-experiment.ts";
import { createMeshInspection } from "./mesh-inspection.ts";
import { createMaterialEditor } from "./material-editor.ts";
import type { RendererFeatures } from "./feature-contract.ts";

/** Editor default. Player builds replace this module with their declared feature profile. */
export const selectedRendererFeatures: RendererFeatures = {
  ...playerFeatures,
  preparedSurfaceBrdf: import.meta.env?.VITE_PREPARED_SURFACE_BRDF === "1",
  createCompiledSourceHost,
  createBattleCrowd: async (...args) =>
    (await import("./battle-crowd.ts")).BattleCrowd.create(...args),
  loadBattleCrowdAssets: async (...args) =>
    (await import("./battle-assets.ts")).loadBattleCrowdAssets(...args),
  createGrass: async (...args) =>
    (await import("./grass.ts")).Grass.create(...args),
  createSpellSwarmHost: (context) =>
    createSpellSwarmHost(context, { orientationPreparation: "compute" }),
  createExperiment: createRendererExperiment,
  createMeshInspection,
  createMaterialEditor,
};
