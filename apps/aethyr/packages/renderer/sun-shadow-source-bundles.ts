import type {
  BundledSourceDraw,
  SourceRenderBundles,
} from "./source-render-bundles.ts";

export type SunShadowPartition = "all" | "static" | "moving";

export function isSunShadowBundleView(
  key: string,
  stockShader: boolean,
): boolean {
  return (
    stockShader &&
    (key === "nearSun" || key === "farSun" || key === "distantSun")
  );
}

/** Select the same source records as the preceding direct static/wind passes. */
export function sunShadowPartitionIncludes(
  draw: { wind?: boolean },
  partition?: SunShadowPartition,
): boolean {
  return partition === "static"
    ? !draw.wind
    : partition === "moving"
      ? Boolean(draw.wind)
      : true;
}

interface SunShadowSource {
  instances: GPUBuffer;
  wind: GPUBuffer;
  draws: readonly object[];
  bundleDraws?: readonly object[];
  bundles?: SourceRenderBundles;
  shadowPartition?: SunShadowPartition;
}

export interface SunShadowBundleBindings {
  enabled: boolean;
  stockShader: boolean;
  frameGroup?: GPUBindGroup;
  pipeline: GPURenderPipeline;
  materials: GPUBindGroup[];
  vertices: GPUBuffer;
  colors: GPUBuffer;
  uv1: GPUBuffer;
  zeroWind: GPUBuffer;
  indices: GPUBuffer;
  include(draw: BundledSourceDraw): boolean;
}

const DESCRIPTOR: GPURenderBundleEncoderDescriptor = {
  colorFormats: [],
  depthStencilFormat: "depth32float",
  sampleCount: 1,
};

/** Undefined retains direct drawing; a boolean means source commands were handled. */
export function executeSunShadowSourceBundle(
  pass: GPURenderPassEncoder,
  source: SunShadowSource,
  bindings: SunShadowBundleBindings,
): boolean | undefined {
  if (
    !bindings.enabled ||
    !bindings.stockShader ||
    !bindings.frameGroup ||
    !source.shadowPartition ||
    !source.bundles ||
    source.bundleDraws !== source.draws
  )
    return undefined;

  const partition = source.shadowPartition;
  const bundle = source.bundles.get({
    key: "shadow:" + partition,
    descriptor: DESCRIPTOR,
    frameGroup: bindings.frameGroup,
    shader: bindings.pipeline,
    pipelineVersion: 0,
    materials: bindings.materials,
    vertices: bindings.vertices,
    instances: source.instances,
    colors: bindings.colors,
    uv1: bindings.uv1,
    wind: source.wind,
    zeroWind: bindings.zeroWind,
    indices: bindings.indices,
    include: (draw) =>
      sunShadowPartitionIncludes(draw, partition) && bindings.include(draw),
    material: (draw) => bindings.materials[draw.material],
    pipeline: () => bindings.pipeline,
  });
  if (bundle.length) pass.executeBundles(bundle);
  // executeBundles clears state. The existing authored/room/debris tails draw next.
  pass.setPipeline(bindings.pipeline);
  pass.setBindGroup(0, bindings.frameGroup);
  pass.setVertexBuffer(0, bindings.vertices);
  pass.setVertexBuffer(1, source.instances);
  pass.setVertexBuffer(2, bindings.colors);
  pass.setVertexBuffer(3, bindings.uv1);
  pass.setIndexBuffer(bindings.indices, "uint32");
  return bundle.length > 0;
}
