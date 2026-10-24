import { GpuBoidSimulation } from "../boids/gpu-simulation.ts";
import {
  createFishMesh,
  FISH_TRIANGLES,
  FISH_VERTEX_STRIDE,
} from "../boids/mesh.ts";
import { BoidSimulation, BOID_MAX_COUNT } from "../boids/simulation.ts";
import {
  preparedSpellSwarmShader,
  spellSwarmOrientationShader,
  spellSwarmShader,
} from "./spell-swarm-shader.ts";

const UNIFORM_BYTES = 96;
export const SPELL_SWARM_ORIENTATION_BYTES = 48;
export const SPELL_SWARM_DEFAULT_COUNT = 100_000;
export const SPELL_SWARM_TRIANGLES_PER_BOID = FISH_TRIANGLES;
export type SpellSwarmCenter = readonly [number, number, number];

export interface SpellSwarmOptions {
  count?: number;
  center?: SpellSwarmCenter;
  radius?: number;
  size?: number;
  seed?: number;
  format?: GPUTextureFormat;
  depthFormat?: GPUTextureFormat;
  sampleCount?: 1 | 4;
  /** Internal comparison option; both paths render the same complete geometry. */
  orientationPreparation?: "vertex" | "compute";
}

export interface SpellSwarmConfig {
  readonly count: number;
  readonly center: SpellSwarmCenter;
  readonly radius: number;
  readonly size: number;
  readonly seed: number;
  readonly format: GPUTextureFormat;
  readonly depthFormat: GPUTextureFormat;
  readonly sampleCount: 1 | 4;
  readonly orientationPreparation: "vertex" | "compute";
}

export function validateSpellSwarmOptions(
  options: SpellSwarmOptions = {},
): SpellSwarmConfig {
  const count = options.count ?? SPELL_SWARM_DEFAULT_COUNT;
  const center = options.center ?? [6, 14, -4];
  const radius = options.radius ?? 12;
  const size = options.size ?? 0.11;
  const seed = options.seed ?? 0x5eaf00d;
  const sampleCount = options.sampleCount ?? 4;
  const orientationPreparation = options.orientationPreparation ?? "vertex";
  if (!Number.isInteger(count) || count < 1 || count > BOID_MAX_COUNT)
    throw new RangeError("Choose from 1 to 200,000 spell creatures.");
  validateCenter(center);
  if (!Number.isFinite(radius) || radius < 1 || radius > 64)
    throw new RangeError("Choose a spell radius from 1 to 64 meters.");
  if (!Number.isFinite(size) || size < 0.01 || size > 1)
    throw new RangeError("Choose a spell creature size from 0.01 to 1 meter.");
  if (!Number.isInteger(seed) || seed < 0 || seed > 0xffffffff)
    throw new RangeError("Choose an unsigned 32-bit spell seed.");
  if (sampleCount !== 1 && sampleCount !== 4)
    throw new RangeError("Spell antialiasing must use 1 or 4 samples.");
  if (
    orientationPreparation !== "vertex" &&
    orientationPreparation !== "compute"
  )
    throw new RangeError(
      "Choose vertex or compute spell orientation preparation.",
    );
  return Object.freeze({
    count,
    center: Object.freeze([...center]) as SpellSwarmCenter,
    radius,
    size,
    seed,
    format: options.format ?? "rgba16float",
    depthFormat: options.depthFormat ?? "depth24plus",
    sampleCount,
    orientationPreparation,
  });
}

function validateCenter(center: readonly number[]): void {
  if (
    center.length !== 3 ||
    center.some((v) => !Number.isFinite(v) || Math.abs(v) > 1_000_000)
  )
    throw new RangeError("Choose a finite spell position inside the world.");
}

export interface SpellSwarmMetrics {
  readonly enabled: boolean;
  readonly count: number;
  readonly trianglesPerBoid: number;
  readonly trianglesPerDraw: number;
  readonly submittedDraws: number;
  readonly simulationSteps: number;
  readonly accountedGpuBytes: number;
  readonly gpuComputeSample: {
    readonly serial: number;
    readonly ms: number;
  } | null;
  readonly gpuTimingSupported: boolean;
  readonly center: SpellSwarmCenter;
  readonly orientationPreparation: "vertex" | "compute";
  readonly orientationBytes: number;
  readonly orientationPasses: number;
}

export interface SpellSwarmProbe {
  readonly count: number;
  readonly finite: boolean;
  /** Distance in local simulation space, before the world center is added. */
  readonly maxDistanceFromCenter: number;
  readonly stateSha256: string;
  /** The first eight boid positions in world coordinates. */
  readonly positions: readonly SpellSwarmCenter[];
  readonly simulationSteps: number;
}

/** One GPU flock and one indexed draw inside the caller's existing HDR pass. */
export class SpellSwarm {
  readonly count: number;
  /** Explicit GPU buffer sizes; native pipeline and query storage is opaque. */
  readonly bytes: number;
  private simulation: GpuBoidSimulation;
  private vertex: GPUBuffer;
  private index: GPUBuffer;
  private uniform: GPUBuffer;
  private viewGroup: GPUBindGroup;
  private stateGroups: readonly [GPUBindGroup, GPUBindGroup];
  private words = new Float32Array(UNIFORM_BYTES / 4);
  private target: [number, number, number] = [0, 0, 0];
  private obstacle: [number, number, number] = [0, 0, 0];
  private center: [number, number, number];
  private enabled = true;
  private disposed = false;
  private simulationSteps = 0;
  private submittedDraws = 0;
  private pendingSimulation = false;
  private orientations?: GPUBuffer;
  private orientationGroups?: readonly [GPUBindGroup, GPUBindGroup];
  private orientationPasses = 0;
  private preparedForDraw = false;

  private constructor(
    private device: GPUDevice,
    private config: SpellSwarmConfig,
    private pipeline: GPURenderPipeline,
    viewLayout: GPUBindGroupLayout,
    stateLayout: GPUBindGroupLayout,
    private orientation?: {
      pipeline: GPUComputePipeline;
      layout: GPUBindGroupLayout;
    },
  ) {
    this.count = config.count;
    this.center = [...config.center];
    // CPU state is seeded once. Only GPU clear/scatter/update runs per frame.
    const seed = new BoidSimulation({
      count: config.count,
      seed: config.seed,
      radius: config.radius,
      cellSize: config.radius / 8,
      speed: Math.min(16, config.radius * 0.16),
      obstacleRadius: config.radius * 0.14,
      targetWeight: 0.25,
      obstacleWeight: 1.5,
    });
    this.simulation = new GpuBoidSimulation(device, seed, {
      timestamps: false,
    });
    const mesh = createFishMesh();
    this.vertex = this.upload(
      mesh.vertices,
      GPUBufferUsage.VERTEX,
      "Spell swarm: full fish vertices",
    );
    this.index = this.upload(
      mesh.indices,
      GPUBufferUsage.INDEX,
      "Spell swarm: full fish indices",
    );
    this.uniform = device.createBuffer({
      label: "Spell swarm: camera and world center",
      size: UNIFORM_BYTES,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.viewGroup = device.createBindGroup({
      layout: viewLayout,
      entries: [{ binding: 0, resource: { buffer: this.uniform } }],
    });
    if (orientation) {
      this.orientations = device.createBuffer({
        label: "Spell swarm: prepared orientation and seed",
        size: config.count * SPELL_SWARM_ORIENTATION_BYTES,
        usage: GPUBufferUsage.STORAGE,
      });
      this.orientationGroups = this.simulation.states.map((buffer) =>
        device.createBindGroup({
          layout: orientation.layout,
          entries: [
            { binding: 0, resource: { buffer } },
            { binding: 1, resource: { buffer: this.orientations! } },
          ],
        }),
      ) as unknown as readonly [GPUBindGroup, GPUBindGroup];
    }
    this.stateGroups = this.simulation.states.map((buffer) =>
      device.createBindGroup({
        layout: stateLayout,
        entries: [
          { binding: 0, resource: { buffer } },
          ...(this.orientations
            ? [{ binding: 1, resource: { buffer: this.orientations } }]
            : []),
        ],
      }),
    ) as unknown as readonly [GPUBindGroup, GPUBindGroup];
    this.bytes =
      this.simulation.bytes +
      this.vertex.size +
      this.index.size +
      UNIFORM_BYTES +
      (this.orientations?.size ?? 0);
  }

  static async create(
    device: GPUDevice,
    options: SpellSwarmOptions = {},
  ): Promise<SpellSwarm> {
    const config = validateSpellSwarmOptions(options);
    const viewLayout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform", minBindingSize: UNIFORM_BYTES },
        },
      ],
    });
    const stateLayout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX,
          buffer: { type: "read-only-storage", minBindingSize: 32 },
        },
        ...(config.orientationPreparation === "compute"
          ? [
              {
                binding: 1,
                visibility: GPUShaderStage.VERTEX,
                buffer: {
                  type: "read-only-storage" as const,
                  minBindingSize: SPELL_SWARM_ORIENTATION_BYTES,
                },
              },
            ]
          : []),
      ],
    });
    let orientation:
      { pipeline: GPUComputePipeline; layout: GPUBindGroupLayout } | undefined;
    if (config.orientationPreparation === "compute") {
      const layout = device.createBindGroupLayout({
        entries: [
          {
            binding: 0,
            visibility: GPUShaderStage.COMPUTE,
            buffer: { type: "read-only-storage", minBindingSize: 32 },
          },
          {
            binding: 1,
            visibility: GPUShaderStage.COMPUTE,
            buffer: {
              type: "storage",
              minBindingSize: SPELL_SWARM_ORIENTATION_BYTES,
            },
          },
        ],
      });
      const orientationModule = device.createShaderModule({
        label: "Spell swarm: per-boid orientation preparation",
        code: spellSwarmOrientationShader,
      });
      const pipeline = await device.createComputePipelineAsync({
        label: "Spell swarm: prepare shared orientation",
        layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
        compute: {
          module: orientationModule,
          entryPoint: "prepareOrientation",
        },
      });
      orientation = { pipeline, layout };
    }
    const module = device.createShaderModule({
      label: "Spell swarm: luminous full-geometry flock",
      code: orientation ? preparedSpellSwarmShader : spellSwarmShader,
    });
    const pipeline = await device.createRenderPipelineAsync({
      label: "Spell swarm: depth-tested HDR fish",
      layout: device.createPipelineLayout({
        bindGroupLayouts: [viewLayout, stateLayout],
      }),
      vertex: {
        module,
        entryPoint: "vertexMain",
        buffers: [
          {
            arrayStride: FISH_VERTEX_STRIDE,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x3" },
              { shaderLocation: 1, offset: 12, format: "float32x3" },
              { shaderLocation: 2, offset: 24, format: "float32x2" },
            ],
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fragmentMain",
        targets: [{ format: config.format }],
      },
      primitive: { topology: "triangle-list", cullMode: "back" },
      depthStencil: {
        format: config.depthFormat,
        depthWriteEnabled: true,
        depthCompare: "less-equal",
      },
      multisample: { count: config.sampleCount },
    });
    return new SpellSwarm(
      device,
      config,
      pipeline,
      viewLayout,
      stateLayout,
      orientation,
    );
  }

  private upload(
    values: Float32Array | Uint16Array,
    usage: GPUBufferUsageFlags,
    label: string,
  ): GPUBuffer {
    const buffer = this.device.createBuffer({
      label,
      size: (values.byteLength + 3) & ~3,
      usage: usage | GPUBufferUsage.COPY_DST,
    });
    this.device.queue.writeBuffer(
      buffer,
      0,
      values.buffer as ArrayBuffer,
      values.byteOffset,
      values.byteLength,
    );
    return buffer;
  }

  setEnabled(enabled: boolean): void {
    this.assertOpen();
    this.enabled = enabled;
    if (!enabled) this.preparedForDraw = false;
  }

  setCenter(center: SpellSwarmCenter): void {
    this.assertOpen();
    validateCenter(center);
    this.center[0] = center[0];
    this.center[1] = center[1];
    this.center[2] = center[2];
  }

  /** Encode before beginning the caller's render pass; submit that buffer once. */
  encodeSimulation(
    encoder: GPUCommandEncoder,
    dtSeconds: number,
    timeSeconds: number,
  ): boolean {
    this.assertOpen();
    if (!this.enabled) return false;
    if (
      !Number.isFinite(dtSeconds) ||
      dtSeconds < 0 ||
      dtSeconds > 0.1 ||
      !Number.isFinite(timeSeconds)
    )
      throw new RangeError(
        "Choose a spell step from 0 to 0.1 seconds and a finite time.",
      );
    if (this.pendingSimulation)
      throw new Error(
        "Submit the preceding spell step before encoding another.",
      );
    if (dtSeconds === 0) return false;
    const radius = this.config.radius;
    this.target[0] = Math.sin(timeSeconds * 0.21) * radius * 0.42;
    this.target[1] = Math.sin(timeSeconds * 0.17) * radius * 0.2;
    this.target[2] = Math.cos(timeSeconds * 0.19) * radius * 0.42;
    this.obstacle[0] = Math.cos(timeSeconds * 0.27) * radius * 0.3;
    this.obstacle[1] = Math.sin(timeSeconds * 0.23) * radius * 0.22;
    this.obstacle[2] = Math.sin(timeSeconds * 0.31) * radius * 0.3;
    this.preparedForDraw = false;
    this.simulation.encode(encoder, dtSeconds, this.target, this.obstacle);
    this.pendingSimulation = true;
    this.simulationSteps++;
    return true;
  }

  /** Prepare after simulation and before each HDR pass, including still captures. */
  prepareRender(encoder: GPUCommandEncoder): boolean {
    this.assertOpen();
    if (!this.enabled || !this.orientation || !this.orientationGroups)
      return false;
    const state = this.simulation.current;
    const pass = encoder.beginComputePass({
      label: "prepare spell orientation",
    });
    pass.setPipeline(this.orientation.pipeline);
    pass.setBindGroup(
      0,
      this.orientationGroups[state === this.simulation.states[0] ? 0 : 1],
    );
    pass.dispatchWorkgroups(Math.ceil(this.count / 128));
    pass.end();
    this.preparedForDraw = true;
    this.orientationPasses++;
    return true;
  }

  /** Render all 310 triangles of every boid into the existing Town attachments. */
  render(
    pass: GPURenderPassEncoder,
    viewProjection: ArrayLike<number>,
    cameraEye: readonly number[],
    timeSeconds: number,
  ): boolean {
    this.assertOpen();
    if (!this.enabled) return false;
    if (this.orientation && !this.preparedForDraw)
      throw new Error("Prepare spell orientation before each render pass.");
    if (
      viewProjection.length !== 16 ||
      cameraEye.length !== 3 ||
      !Number.isFinite(timeSeconds)
    )
      throw new RangeError("Choose a complete spell camera and a finite time.");
    for (let i = 0; i < 16; i++) {
      if (!Number.isFinite(viewProjection[i]))
        throw new RangeError("The spell camera must be finite.");
      this.words[i] = viewProjection[i];
    }
    for (let i = 0; i < 3; i++) {
      if (!Number.isFinite(cameraEye[i]))
        throw new RangeError("The spell camera must be finite.");
      this.words[16 + i] = cameraEye[i];
      this.words[20 + i] = this.center[i];
    }
    this.words[19] = timeSeconds;
    this.words[23] = this.config.size;
    this.device.queue.writeBuffer(this.uniform, 0, this.words.buffer);
    const state = this.simulation.current;
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.viewGroup);
    pass.setBindGroup(
      1,
      this.stateGroups[state === this.simulation.states[0] ? 0 : 1],
    );
    pass.setVertexBuffer(0, this.vertex);
    pass.setIndexBuffer(this.index, "uint16");
    pass.drawIndexed(FISH_TRIANGLES * 3, this.count);
    this.preparedForDraw = false;
    this.submittedDraws++;
    return true;
  }

  /** Call only after submitting the encoder that contains encodeSimulation(). */
  afterSubmit(): void {
    if (this.disposed || !this.pendingSimulation) return;
    this.pendingSimulation = false;
    this.simulation.afterSubmit();
  }

  inspect(): SpellSwarmMetrics {
    return {
      enabled: this.enabled && !this.disposed,
      count: this.count,
      trianglesPerBoid: FISH_TRIANGLES,
      trianglesPerDraw: FISH_TRIANGLES * this.count,
      submittedDraws: this.submittedDraws,
      simulationSteps: this.simulationSteps,
      accountedGpuBytes: this.disposed ? 0 : this.bytes,
      gpuComputeSample: this.simulation.lastSample,
      gpuTimingSupported: false,
      center: [...this.center],
      orientationPreparation: this.config.orientationPreparation,
      orientationBytes: this.disposed ? 0 : (this.orientations?.size ?? 0),
      orientationPasses: this.orientationPasses,
    };
  }

  /** Read back only for an explicit diagnostic, outside frame-rate windows. */
  async probe(): Promise<SpellSwarmProbe> {
    this.assertOpen();
    const center: SpellSwarmCenter = [...this.center];
    const simulationSteps = this.simulationSteps;
    const state = await this.simulation.readState();
    this.assertOpen();
    if (state.length !== this.count * 8)
      throw new Error("The spell state does not contain every creature.");
    let finite = true;
    let maxDistanceFromCenter = 0;
    const positions: SpellSwarmCenter[] = [];
    for (let i = 0; i < state.length; i += 8) {
      for (let word = 0; word < 8; word++)
        if (!Number.isFinite(state[i + word])) finite = false;
      const distance = Math.hypot(state[i], state[i + 1], state[i + 2]);
      if (Number.isFinite(distance))
        maxDistanceFromCenter = Math.max(maxDistanceFromCenter, distance);
      else maxDistanceFromCenter = Infinity;
      if (positions.length < 8)
        positions.push([
          state[i] + center[0],
          state[i + 1] + center[1],
          state[i + 2] + center[2],
        ]);
    }
    const digest = await crypto.subtle.digest(
      "SHA-256",
      new Uint8Array(
        state.buffer as ArrayBuffer,
        state.byteOffset,
        state.byteLength,
      ),
    );
    return {
      count: this.count,
      finite,
      maxDistanceFromCenter,
      stateSha256: Array.from(new Uint8Array(digest), (byte) =>
        byte.toString(16).padStart(2, "0"),
      ).join(""),
      positions,
      simulationSteps,
    };
  }

  private assertOpen(): void {
    if (this.disposed) throw new Error("This spell swarm has closed.");
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.enabled = false;
    this.simulation.dispose();
    this.vertex.destroy();
    this.index.destroy();
    this.uniform.destroy();
    this.orientations?.destroy();
  }
}
