import { MMORPG_TERRAIN_WGSL } from "./mmorpg-terrain-shader.ts";

export interface MmorpgTerrainMetadata {
  firstVertex: number;
  vertexCount: number;
  resolution: number;
  sizeMetres: number;
}

/** A moving fixed-size patch keeps the original hills without world-sized uploads. */
export class MmorpgTerrain {
  readonly bytes = 32;
  readonly stats = { updates: 0, vertices: 0, center: [0, 0] };
  private center: [number, number] = [Infinity, Infinity];
  private constructor(
    private device: GPUDevice,
    private metadata: MmorpgTerrainMetadata,
    private uniform: GPUBuffer,
    private pipeline: GPUComputePipeline,
    private bindings: GPUBindGroup,
  ) {}
  static async create(
    device: GPUDevice,
    vertices: GPUBuffer,
    metadata: MmorpgTerrainMetadata,
  ) {
    const { firstVertex, vertexCount, resolution, sizeMetres } = metadata;
    if (
      firstVertex !== 0 ||
      resolution !== 257 ||
      vertexCount !== resolution ** 2 ||
      sizeMetres !== 3328 ||
      (firstVertex + vertexCount) * 48 > vertices.size
    )
      throw new Error("Prepare the supported MMO terrain patch.");
    const module = device.createShaderModule({
      code: MMORPG_TERRAIN_WGSL,
      label: "MMO source terrain",
    });
    const pipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module, entryPoint: "update" },
    });
    const uniform = device.createBuffer({
      size: 32,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const bindings = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: uniform } },
        { binding: 1, resource: { buffer: vertices } },
      ],
    });
    return new MmorpgTerrain(device, metadata, uniform, pipeline, bindings);
  }
  prepare(encoder: GPUCommandEncoder, center: readonly number[]) {
    const x = Math.floor(center[0] / 16) * 16,
      z = Math.floor(center[2] / 16) * 16;
    if (!Number.isFinite(x) || !Number.isFinite(z)) return;
    if (x === this.center[0] && z === this.center[1]) return;
    this.center = [x, z];
    const data = new ArrayBuffer(32),
      f = new Float32Array(data),
      u = new Uint32Array(data);
    f[0] = x;
    f[1] = z;
    f[2] = 2;
    u[3] = this.metadata.resolution;
    u[4] = this.metadata.firstVertex;
    u[5] = this.metadata.vertexCount;
    this.device.queue.writeBuffer(this.uniform, 0, data);
    const pass = encoder.beginComputePass({
      label: "Move source terrain patch",
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.bindings);
    pass.dispatchWorkgroups(Math.ceil(this.metadata.vertexCount / 64));
    pass.end();
    this.stats.updates++;
    this.stats.vertices = this.metadata.vertexCount;
    this.stats.center = [x, z];
  }
  dispose() {
    this.uniform.destroy();
  }
}
