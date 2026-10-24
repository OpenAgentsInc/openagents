import {
  inverse,
  lookAt,
  multiply,
  perspective,
  type V3,
} from "../renderer/math.ts";
import { createFishMesh, FISH_TRIANGLES, FISH_VERTEX_STRIDE } from "./mesh.ts";
import { fishShader, waterShader } from "./shaders.ts";

export interface BoidsView {
  state: GPUBuffer;
  count: number;
  label?: string;
}
export interface BoidsCamera {
  yaw: number;
  pitch: number;
  distance: number;
  target?: readonly [number, number, number];
}
export interface BoidsRenderOptions {
  /** Encodes simulation before graphics in this same command buffer. */
  beforeRender?: (encoder: GPUCommandEncoder) => void;
  onComplete?: () => void;
}
export interface BoidsRenderMetrics {
  gpuRenderMs: number | null;
  gpuRenderSample: { serial: number; ms: number } | null;
  gpuViewMs: readonly (number | null)[];
  gpuTimingSupported: boolean;
  gpuTimedFrames: number;
  gpuTimingSkipped: number;
  trianglesPerView: readonly number[];
  drawCalls: number;
  submittedFrames: number;
  completedFrames: number;
  inFlight: number;
  queueLatencyMs: number | null;
  sampleCount: 1 | 4;
  width: number;
  height: number;
  lastError: string | null;
}
interface TimingSlot {
  queries: GPUQuerySet;
  resolve: GPUBuffer;
  read: GPUBuffer;
  busy: boolean;
}
const DEFAULT_CAMERA: BoidsCamera = { yaw: 0.5, pitch: 0.1, distance: 64 };
const UNIFORM_BYTES = 192;
const MAX_BOIDS = 200_000;
const TIMING_SLOTS = 4;

/** A reusable Aethyrforge instancing path. No DOM or UI work runs per fish. */
export class BoidsRenderer {
  readonly device: GPUDevice;
  readonly format: GPUTextureFormat;
  private context: GPUCanvasContext;
  private vertex: GPUBuffer;
  private index: GPUBuffer;
  private uniforms: GPUBuffer[];
  private viewGroups: GPUBindGroup[];
  private uniformWords = [
    new Float32Array(UNIFORM_BYTES / 4),
    new Float32Array(UNIFORM_BYTES / 4),
  ];
  private stateGroups = new WeakMap<GPUBuffer, GPUBindGroup>();
  private stateLayout: GPUBindGroupLayout;
  private fishPipeline: GPURenderPipeline;
  private waterPipeline: GPURenderPipeline;
  private depth: GPUTexture | null = null;
  private depthView: GPUTextureView | null = null;
  private multisample: GPUTexture | null = null;
  private multisampleView: GPUTextureView | null = null;
  private depthWidth = 0;
  private depthHeight = 0;
  private timing: TimingSlot[] = [];
  private disposed = false;
  private scale = 1;
  private ownedBuffers = new Set<GPUBuffer>();
  private state: BoidsRenderMetrics;

  private constructor(
    private canvas: HTMLCanvasElement,
    device: GPUDevice,
    context: GPUCanvasContext,
    format: GPUTextureFormat,
    fishPipeline: GPURenderPipeline,
    waterPipeline: GPURenderPipeline,
    uniformLayout: GPUBindGroupLayout,
    stateLayout: GPUBindGroupLayout,
    private sampleCount: 1 | 4,
  ) {
    this.device = device;
    this.context = context;
    this.format = format;
    this.fishPipeline = fishPipeline;
    this.waterPipeline = waterPipeline;
    this.stateLayout = stateLayout;
    this.state = {
      gpuRenderMs: null,
      gpuRenderSample: null,
      gpuViewMs: [],
      gpuTimingSupported: device.features.has("timestamp-query"),
      gpuTimedFrames: 0,
      gpuTimingSkipped: 0,
      trianglesPerView: [],
      drawCalls: 0,
      submittedFrames: 0,
      completedFrames: 0,
      inFlight: 0,
      queueLatencyMs: null,
      sampleCount,
      width: 0,
      height: 0,
      lastError: null,
    };
    const mesh = createFishMesh();
    this.vertex = this.upload(
      mesh.vertices,
      GPUBufferUsage.VERTEX,
      "Boids: original 310-triangle fish vertices",
    );
    this.index = this.upload(
      mesh.indices,
      GPUBufferUsage.INDEX,
      "Boids: original 310-triangle fish indices",
    );
    this.uniforms = [0, 1].map((i) =>
      this.buffer(
        UNIFORM_BYTES,
        GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        `Boids: view ${i}`,
      ),
    );
    this.viewGroups = this.uniforms.map((buffer) =>
      device.createBindGroup({
        layout: uniformLayout,
        entries: [{ binding: 0, resource: { buffer } }],
      }),
    );
    if (this.state.gpuTimingSupported) {
      for (let i = 0; i < TIMING_SLOTS; i++) {
        this.timing.push({
          queries: device.createQuerySet({
            label: `Boids: timing ${i}`,
            type: "timestamp",
            count: 4,
          }),
          resolve: this.buffer(
            32,
            GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC,
            `Boids: timing resolve ${i}`,
          ),
          read: this.buffer(
            32,
            GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
            `Boids: timing read ${i}`,
          ),
          busy: false,
        });
      }
    }
    device.addEventListener("uncapturederror", this.onGpuError);
    void device.lost.then((lost) => {
      if (!this.disposed)
        this.state.lastError = `Graphics stopped: ${lost.message || lost.reason}`;
    });
  }

  static async create(
    canvas: HTMLCanvasElement,
    options: { sampleCount?: 1 | 4 } = {},
  ): Promise<BoidsRenderer> {
    if (!navigator.gpu)
      throw new Error("This browser needs WebGPU to run Boids Lab.");
    const adapter = await navigator.gpu.requestAdapter({
      powerPreference: "high-performance",
    });
    if (!adapter) throw new Error("No WebGPU graphics adapter is available.");
    const device = await adapter.requestDevice({
      requiredFeatures: adapter.features.has("timestamp-query")
        ? ["timestamp-query"]
        : [],
    });
    const context = canvas.getContext("webgpu");
    if (!context) {
      device.destroy();
      throw new Error("The browser could not create a WebGPU canvas.");
    }
    const format = navigator.gpu.getPreferredCanvasFormat();
    const sampleCount = options.sampleCount ?? 4;
    if (sampleCount !== 1 && sampleCount !== 4) {
      device.destroy();
      throw new Error("Antialiasing must use 1 or 4 samples.");
    }
    try {
      const uniformLayout = device.createBindGroupLayout({
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
        ],
      });
      const color = { format };
      const [fishPipeline, waterPipeline] = await Promise.all([
        device.createRenderPipelineAsync({
          label: "Boids: instanced GGX fish",
          layout: device.createPipelineLayout({
            bindGroupLayouts: [uniformLayout, stateLayout],
          }),
          vertex: {
            module: device.createShaderModule({ code: fishShader }),
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
            module: device.createShaderModule({ code: fishShader }),
            entryPoint: "fragmentMain",
            targets: [color],
          },
          primitive: { topology: "triangle-list", cullMode: "back" },
          depthStencil: {
            format: "depth24plus",
            depthWriteEnabled: true,
            depthCompare: "less",
          },
          multisample: { count: sampleCount },
        }),
        device.createRenderPipelineAsync({
          label: "Boids: procedural underwater environment",
          layout: device.createPipelineLayout({
            bindGroupLayouts: [uniformLayout],
          }),
          vertex: {
            module: device.createShaderModule({ code: waterShader }),
            entryPoint: "vertexMain",
          },
          fragment: {
            module: device.createShaderModule({ code: waterShader }),
            entryPoint: "fragmentMain",
            targets: [color],
          },
          primitive: { topology: "triangle-list" },
          depthStencil: {
            format: "depth24plus",
            depthWriteEnabled: false,
            depthCompare: "always",
          },
          multisample: { count: sampleCount },
        }),
      ]);
      context.configure({ device, format, alphaMode: "opaque" });
      return new BoidsRenderer(
        canvas,
        device,
        context,
        format,
        fishPipeline,
        waterPipeline,
        uniformLayout,
        stateLayout,
        sampleCount,
      );
    } catch (error) {
      context.unconfigure();
      device.destroy();
      throw error;
    }
  }

  get metrics(): Readonly<BoidsRenderMetrics> {
    return this.state;
  }

  /** Resolution uses CSS pixels by default; the UI reports this explicit scale. */
  setRenderScale(value: number): void {
    if (!Number.isFinite(value) || value < 0.25 || value > 2)
      throw new Error("Render scale must be between 0.25 and 2.");
    this.scale = value;
  }

  createStateBuffer(capacity: number, label = "Boids: state"): GPUBuffer {
    if (!Number.isInteger(capacity) || capacity < 1 || capacity > MAX_BOIDS)
      throw new Error("Fish count must be between 1 and 200,000.");
    if (this.disposed) throw new Error("This graphics view has closed.");
    return this.buffer(
      capacity * 32,
      GPUBufferUsage.STORAGE |
        GPUBufferUsage.COPY_DST |
        GPUBufferUsage.COPY_SRC,
      label,
    );
  }

  private buffer(
    size: number,
    usage: GPUBufferUsageFlags,
    label: string,
  ): GPUBuffer {
    const buffer = this.device.createBuffer({ size, usage, label });
    this.ownedBuffers.add(buffer);
    return buffer;
  }
  private upload(
    values: Float32Array | Uint16Array,
    usage: GPUBufferUsageFlags,
    label: string,
  ): GPUBuffer {
    const buffer = this.buffer(
      (values.byteLength + 3) & ~3,
      usage | GPUBufferUsage.COPY_DST,
      label,
    );
    this.device.queue.writeBuffer(
      buffer,
      0,
      values.buffer as ArrayBuffer,
      values.byteOffset,
      values.byteLength,
    );
    return buffer;
  }
  private onGpuError = (event: GPUUncapturedErrorEvent): void => {
    this.state.lastError = event.error.message;
  };

  private resize(): void {
    const limit = this.device.limits.maxTextureDimension2D;
    const width = Math.max(
      2,
      Math.min(
        limit,
        Math.round(
          (this.canvas.clientWidth || this.canvas.width || 1280) * this.scale,
        ),
      ),
    );
    const height = Math.max(
      1,
      Math.min(
        limit,
        Math.round(
          (this.canvas.clientHeight || this.canvas.height || 720) * this.scale,
        ),
      ),
    );
    if (width === this.depthWidth && height === this.depthHeight) return;
    const depth = this.device.createTexture({
      label: "Boids: bounded canvas depth",
      size: [width, height],
      format: "depth24plus",
      sampleCount: this.sampleCount,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
    let multisample: GPUTexture | null = null;
    try {
      if (this.sampleCount === 4)
        multisample = this.device.createTexture({
          label: "Boids: bounded four-sample color",
          size: [width, height],
          format: this.format,
          sampleCount: 4,
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        });
    } catch (error) {
      depth.destroy();
      throw error;
    }
    const old = this.depth,
      oldMultisample = this.multisample;
    this.canvas.width = width;
    this.canvas.height = height;
    this.depth = depth;
    this.depthView = depth.createView();
    this.multisample = multisample;
    this.multisampleView = multisample?.createView() ?? null;
    this.depthWidth = width;
    this.depthHeight = height;
    this.state.width = width;
    this.state.height = height;
    old?.destroy();
    oldMultisample?.destroy();
  }

  render(
    views: readonly BoidsView[],
    timeSeconds: number,
    camera: BoidsCamera = DEFAULT_CAMERA,
    options: BoidsRenderOptions = {},
  ): boolean {
    if (this.disposed || this.state.inFlight >= 2 || this.state.lastError)
      return false;
    if (views.length < 1 || views.length > 2)
      throw new Error("Boids Lab renders one or two views.");
    for (const view of views) {
      if (
        !Number.isInteger(view.count) ||
        view.count < 1 ||
        view.count > MAX_BOIDS ||
        view.state.size < view.count * 32
      )
        throw new Error("The fish count must fit the supplied state buffer.");
      if (!(view.state.usage & GPUBufferUsage.STORAGE))
        throw new Error("Fish state needs a graphics storage buffer.");
    }
    if (
      ![
        camera.yaw,
        camera.pitch,
        camera.distance,
        timeSeconds,
        ...(camera.target ?? [0, 0, 0]),
      ].every(Number.isFinite) ||
      camera.distance <= 0
    )
      throw new Error("Camera and time values must be finite.");
    this.resize();
    const target: V3 = camera.target ? [...camera.target] : [0, 0, 0];
    const pitch = Math.max(-1.5, Math.min(1.5, camera.pitch));
    const eye: V3 = [
      target[0] + Math.sin(camera.yaw) * Math.cos(pitch) * camera.distance,
      target[1] + Math.sin(pitch) * camera.distance,
      target[2] + Math.cos(camera.yaw) * Math.cos(pitch) * camera.distance,
    ];
    const encoder = this.device.createCommandEncoder({
      label: "Boids: simulation and two views",
    });
    options.beforeRender?.(encoder);
    const timing = this.timing.find((slot) => !slot.busy);
    if (timing) timing.busy = true;
    else if (this.state.gpuTimingSupported) this.state.gpuTimingSkipped++;
    try {
      const colorView = this.context.getCurrentTexture().createView();
      const width = this.depthWidth,
        height = this.depthHeight;
      const triangles: number[] = [];
      for (let i = 0; i < views.length; i++) {
        const x = Math.floor((i * width) / views.length),
          nextX = Math.floor(((i + 1) * width) / views.length);
        const viewWidth = nextX - x,
          view = views[i],
          u = this.uniformWords[i];
        const vp = multiply(
          perspective((50 * Math.PI) / 180, viewWidth / height, 0.1, 300),
          lookAt(eye, target),
        );
        u.set(vp, 0);
        u.set(inverse(vp), 16);
        u.set([...eye, timeSeconds], 32);
        u.set([viewWidth, height, this.scale, 0], 36);
        u.set([...target, 0.016], 40);
        u.set([0.25, 0, 0, 0], 44);
        this.device.queue.writeBuffer(this.uniforms[i], 0, u.buffer);
        let stateGroup = this.stateGroups.get(view.state);
        if (!stateGroup) {
          stateGroup = this.device.createBindGroup({
            layout: this.stateLayout,
            entries: [{ binding: 0, resource: { buffer: view.state } }],
          });
          this.stateGroups.set(view.state, stateGroup);
        }
        const pass = encoder.beginRenderPass({
          label: `Boids: ${view.label ?? i} fish and environment`,
          colorAttachments: [
            {
              view: this.multisampleView ?? colorView,
              ...(this.multisampleView ? { resolveTarget: colorView } : {}),
              loadOp: i === 0 ? "clear" : "load",
              storeOp: "store",
              clearValue: { r: 0, g: 0.2, b: 0.3, a: 1 },
            },
          ],
          depthStencilAttachment: {
            view: this.depthView!,
            depthLoadOp: "clear",
            depthStoreOp: "discard",
            depthClearValue: 1,
          },
          ...(timing
            ? {
                timestampWrites: {
                  querySet: timing.queries,
                  beginningOfPassWriteIndex: i * 2,
                  endOfPassWriteIndex: i * 2 + 1,
                },
              }
            : {}),
        });
        pass.setViewport(x, 0, viewWidth, height, 0, 1);
        pass.setScissorRect(x, 0, viewWidth, height);
        pass.setBindGroup(0, this.viewGroups[i]);
        pass.setPipeline(this.waterPipeline);
        pass.draw(3);
        pass.setPipeline(this.fishPipeline);
        pass.setBindGroup(1, stateGroup);
        pass.setVertexBuffer(0, this.vertex);
        pass.setIndexBuffer(this.index, "uint16");
        pass.drawIndexed(FISH_TRIANGLES * 3, view.count);
        pass.end();
        triangles.push(FISH_TRIANGLES * view.count);
      }
      if (timing) {
        encoder.resolveQuerySet(
          timing.queries,
          0,
          views.length * 2,
          timing.resolve,
          0,
        );
        encoder.copyBufferToBuffer(
          timing.resolve,
          0,
          timing.read,
          0,
          views.length * 16,
        );
      }
      this.device.queue.submit([encoder.finish()]);
      this.state.submittedFrames++;
      this.state.inFlight++;
      const serial = this.state.submittedFrames,
        submittedAt = performance.now();
      this.state.trianglesPerView = triangles;
      this.state.drawCalls = views.length * 2;
      if (timing) this.readTiming(timing, views.length, serial);
      void this.device.queue.onSubmittedWorkDone().then(
        () => {
          this.state.inFlight--;
          this.state.completedFrames++;
          this.state.queueLatencyMs = performance.now() - submittedAt;
          if (!this.disposed) options.onComplete?.();
        },
        (error: unknown) => {
          this.state.inFlight--;
          if (!this.disposed) this.state.lastError = String(error);
        },
      );
      return true;
    } catch (error) {
      if (timing) timing.busy = false;
      throw error;
    }
  }

  private readTiming(
    slot: TimingSlot,
    viewCount: number,
    serial: number,
  ): void {
    void slot.read.mapAsync(GPUMapMode.READ).then(
      () => {
        try {
          if (this.disposed) return;
          const times = new BigUint64Array(slot.read.getMappedRange());
          const values: number[] = [];
          for (let i = 0; i < viewCount; i++) {
            const begin = times[i * 2],
              end = times[i * 2 + 1];
            if (end <= begin) return;
            values.push(Number(end - begin) / 1e6);
          }
          if (times[viewCount * 2 - 1] <= times[0]) return;
          const ms = Number(times[viewCount * 2 - 1] - times[0]) / 1e6;
          if (serial > (this.state.gpuRenderSample?.serial ?? 0)) {
            this.state.gpuRenderMs = ms;
            this.state.gpuRenderSample = { serial, ms };
            this.state.gpuViewMs = values;
          }
          this.state.gpuTimedFrames++;
        } finally {
          if (slot.read.mapState === "mapped") slot.read.unmap();
          slot.busy = false;
        }
      },
      (error: unknown) => {
        slot.busy = false;
        if (!this.disposed)
          this.state.lastError = `Graphics timing failed: ${String(error)}`;
      },
    );
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.device.removeEventListener("uncapturederror", this.onGpuError);
    this.context.unconfigure();
    this.depth?.destroy();
    this.multisample?.destroy();
    for (const slot of this.timing) slot.queries.destroy();
    for (const buffer of this.ownedBuffers) buffer.destroy();
    this.ownedBuffers.clear();
    this.timing.length = 0;
    this.device.destroy();
  }
}
