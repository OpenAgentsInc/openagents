import { MESH_FRAGMENT, triangleEdges } from "./mesh-view.ts";

/** Editor overlay resources use the renderer's tracked buffer allocator. */
export async function createMeshInspection(
  device: GPUDevice,
  source: string,
  indices: Uint32Array,
  describe: (module: GPUShaderModule) => GPURenderPipelineDescriptor,
  upload: (data: ArrayBuffer, label: string) => GPUBuffer,
  release: (buffer?: GPUBuffer) => void,
  current: () => boolean,
) {
  device.pushErrorScope("validation");
  let pipeline: GPURenderPipeline | undefined;
  let failure: unknown;
  try {
    const module = device.createShaderModule({ code: source + MESH_FRAGMENT });
    const descriptor = describe(module);
    descriptor.fragment = { ...descriptor.fragment!, entryPoint: "meshFS" };
    descriptor.primitive = { topology: "line-list", cullMode: "none" };
    descriptor.depthStencil = {
      ...descriptor.depthStencil!,
      depthWriteEnabled: false,
      depthCompare: "less-equal",
    };
    pipeline = await device.createRenderPipelineAsync(descriptor);
  } catch (error) {
    failure = error;
  }
  const error = await device.popErrorScope();
  if (failure || error) throw failure ?? new Error(error!.message);
  if (!current())
    throw new Error("The world closed before mesh inspection was ready.");
  let edges: GPUBuffer | undefined, rooms: GPUBuffer | undefined;
  try {
    edges = upload(
      triangleEdges(indices).buffer as ArrayBuffer,
      "Mesh inspection edges",
    );
    rooms = upload(
      triangleEdges(new Uint32Array(Array.from({ length: 36 }, (_, i) => i)))
        .buffer as ArrayBuffer,
      "Room inspection edges",
    );
    return { pipeline: pipeline!, indices: edges, rooms };
  } catch (error) {
    release(edges);
    release(rooms);
    throw error;
  }
}
