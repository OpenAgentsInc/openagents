import {
  C_TOWN_BACKEND,
  C_TOWN_BENCHMARK,
} from "../../experiments/c-town/config.ts";
import { CTownGpuTimer } from "../../experiments/c-town/gpu-timer.ts";
import { bindResidentFrameScene } from "../../experiments/c-town/resident-frame.ts";
import type { RendererExperiment } from "./feature-contract.ts";

/** Optional editor experiment adapter; published player graphs never import it. */
export async function createRendererExperiment(): Promise<RendererExperiment> {
  const result: RendererExperiment = {
    backend: C_TOWN_BACKEND,
    compiledSourceBatches: import.meta.env?.VITE_COMPILED_SOURCE_BATCHES === "1",
    profiling: C_TOWN_BENCHMARK,
    ...(C_TOWN_BENCHMARK
      ? { createGpuTimer: (device: GPUDevice) => new CTownGpuTimer(device) }
      : {}),
  };
  if (C_TOWN_BACKEND === "c" || C_TOWN_BACKEND === "c-frame") {
    const { createCFrameCore } =
      await import("../../experiments/c-town/frame-core.ts");
    const core = await createCFrameCore("/c-town-frame.wasm");
    result.framePreparation = {
      implementation: "c",
      get memoryBytes() {
        return core.memoryBytes;
      },
      prepare: (source, version) =>
        bindResidentFrameScene(core, source, version),
      buildClusters: (input) => core.buildClusters(input),
    };
  } else if (C_TOWN_BACKEND === "ts-packed") {
    const { PackedFrameCore } =
      await import("../../experiments/c-town/packed-frame.ts");
    const core = new PackedFrameCore();
    result.framePreparation = {
      implementation: "pooled-typescript",
      memoryBytes: 0,
      prepare: (source, version) =>
        bindResidentFrameScene(core, source, version),
      buildClusters: (input) => core.buildClusters(input),
    };
  }
  return result;
}
