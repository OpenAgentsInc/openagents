import { IS_TOWN } from "./lab.ts";
import { TownFloor, isHouse } from "./town.ts";
import { townGeometry } from "./town-geometry.ts";
import { MedievalRenderer, medievalKinds } from "./medieval.ts";
import { HouseFloor, DEFAULT_LAYOUT } from "./layout.ts";
import { Effect } from "effect";
import {
  ShaderVersions,
  sourceHash,
  validEdit,
  type ShaderEdit,
} from "./protocol.ts";
import { GhostSimulation } from "./behaviors.ts";
import {
  cameraMatrix,
  houseGeometry,
  move,
  type AssetRange,
  type Pose,
} from "./scene.ts";

import {
  SceneHistory,
  projectFloor,
  readProject,
  PROJECT_ID,
  TINTS,
  type Transaction,
  type Operation,
  type Project,
} from "./project.ts";


export interface RuntimeHost {
  postMessage(message: object): void;
  requestAnimationFrame(callback: (time: number) => void): number;
  cancelAnimationFrame(id: number): void;
}

export class Runtime {
  host: RuntimeHost;
  medieval: MedievalRenderer | undefined;
  kitError: string | undefined;
  device: GPUDevice;
  context: GPUCanvasContext;
  canvas: OffscreenCanvas;
  format: GPUTextureFormat;
  uniform: GPUBuffer;
  vertices: GPUBuffer;
  bindGroup: GPUBindGroup;
  layout: GPUPipelineLayout;
  depth: GPUTexture | undefined;
  versions = new ShaderVersions<GPURenderPipeline>(() => {});
  pose: Pose = IS_TOWN
    ? { x: -11, z: 3, y: 0, yaw: 0, pitch: -0.03 }
    : { x: 0, z: 6.8, yaw: 0, pitch: -0.03 };
  keys: string[] = [];
  disposed = false;
  lost = false;
  paused = false;
  frame = 0;
  count: number;
  width = 1;
  height = 1;
  frameId = 0;
  previous = 0;
  lastReport = 0;
  maxFrameMs = 0;
  uniformData = new Float32Array(28);
  scene = new SceneHistory();
  floor = projectFloor(this.scene.project);
  ghosts = new GhostSimulation();
  ranges = new Map<string, AssetRange>();
  baseGeometry: Float32Array;
  motionBuffers = new Map<string, Float32Array>();
  geometryBuilds = 1;
  motionPoses = new Map<string, [number, number, number, number]>();
  captureRequest: string | undefined;
  captureBuffer: GPUBuffer | undefined;
  scenePending:
    | { request: string; transaction?: Transaction; restore?: Project }
    | undefined;
  pendingEdit: ShaderEdit | undefined;
  compiles = 0;
  inflight = 0;
  queuedEdit: ShaderEdit | undefined;
  deviceId = crypto.randomUUID();

  constructor(
    device: GPUDevice,
    context: GPUCanvasContext,
    canvas: OffscreenCanvas,
    format: GPUTextureFormat,
    host: RuntimeHost,
  ) {
    this.host = host;
    this.device = device;
    this.context = context;
    this.canvas = canvas;
    this.format = format;
    const geometry =
      this.floor instanceof TownFloor
        ? townGeometry(this.floor, true)
        : houseGeometry(
            this.scene.project.entities,
            this.ranges,
            this.scene.project.layout ?? DEFAULT_LAYOUT,
          );
    this.baseGeometry = geometry;
    this.ghosts.sync(this.scene.project.entities);
    this.count = geometry.length / 10;
    this.vertices = device.createBuffer({
      label: "Haunted House geometry",
      size: geometry.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(this.vertices, 0, geometry.buffer);
    this.uniform = device.createBuffer({
      label: "Camera and lantern",
      size: 112,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const binding = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform", minBindingSize: 112 },
        },
      ],
    });
    this.bindGroup = device.createBindGroup({
      layout: binding,
      entries: [{ binding: 0, resource: { buffer: this.uniform } }],
    });
    this.layout = device.createPipelineLayout({ bindGroupLayouts: [binding] });
    this.resize(960, 640);
    device.lost.then((info) => {
      if (this.disposed) return;
      this.lost = true;
      this.versions.dispose();
      this.host.postMessage({
        type: "lost",
        message: `The graphics device stopped (${info.reason}). Reopen the preview to reconnect.`,
        state: this.state(),
      });
    });
    device.addEventListener("uncapturederror", (event) =>
      this.host.postMessage({ type: "error", message: event.error.message }),
    );
    this.frameId = this.host.requestAnimationFrame((t) => this.draw(t));
  }

  resize(width: number, height: number) {
    if (this.disposed || this.lost) return;
    const limit = Math.min(4096, this.device.limits.maxTextureDimension2D);
    this.width = Math.max(1, Math.min(limit, Math.floor(width)));
    this.height = Math.max(1, Math.min(limit, Math.floor(height)));
    this.canvas.width = this.width;
    this.canvas.height = this.height;
    this.depth?.destroy();
    this.depth = this.device.createTexture({
      size: [this.width, this.height],
      format: "depth24plus",
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
  }

  edit(edit: ShaderEdit) {
    if (this.disposed || this.lost || !this.versions.begin(edit.revision))
      return;
    this.pendingEdit = undefined;
    if (this.inflight) {
      this.queuedEdit = edit;
      return;
    }
    this.compile(edit);
  }

  compile(edit: ShaderEdit) {
    this.inflight++;
    const { device, layout, format } = this;
    const build = Effect.gen(function* () {
      if ((yield* Effect.promise(() => sourceHash(edit.source))) !== edit.hash)
        return yield* Effect.fail(
          new Error("Shader contents do not match this save."),
        );
      device.pushErrorScope("validation");
      const shader = device.createShaderModule({
        label: `material.wgsl revision ${edit.revision}`,
        code: edit.source,
      });
      const validation = device.popErrorScope();
      const [info, moduleError] = yield* Effect.promise(() =>
        Promise.all([shader.getCompilationInfo(), validation]),
      );
      const failures = info.messages.filter((m) => m.type === "error");
      if (failures.length)
        return yield* Effect.fail(
          new Error(
            failures
              .map(
                (m) => `material.wgsl:${m.lineNum}:${m.linePos} ${m.message}`,
              )
              .join("\n"),
          ),
        );
      if (moduleError)
        return yield* Effect.fail(new Error(moduleError.message));
      return yield* Effect.tryPromise({
        try: () =>
          device.createRenderPipelineAsync({
            label: `Haunted House material ${edit.revision}`,
            layout,
            vertex: {
              module: shader,
              entryPoint: "vertex_main",
              buffers: [
                {
                  arrayStride: 40,
                  attributes: [
                    { shaderLocation: 0, offset: 0, format: "float32x3" },
                    { shaderLocation: 1, offset: 12, format: "float32x3" },
                    { shaderLocation: 2, offset: 24, format: "float32x3" },
                    { shaderLocation: 3, offset: 36, format: "float32" },
                  ],
                },
              ],
            },
            fragment: {
              module: shader,
              entryPoint: "fragment_main",
              targets: [{ format }],
            },
            primitive: { topology: "triangle-list", cullMode: "none" },
            depthStencil: {
              format: "depth24plus",
              depthWriteEnabled: true,
              depthCompare: "less",
            },
          }),
        catch: (e) =>
          new Error(
            `material.wgsl: pipeline or binding interface: ${e instanceof Error ? e.message : String(e)}`,
          ),
      });
    });
    Effect.runPromise(build)
      .then((pipeline) => {
        this.compiles++;
        if (this.versions.stage(edit.revision, edit.hash, pipeline))
          this.pendingEdit = edit;
      })
      .catch((error) => {
        if (!this.disposed && edit.revision === this.versions.draft)
          this.host.postMessage({
            type: "rejected",
            revision: edit.revision,
            request: edit.request,
            message: String(error),
            state: this.state(),
          });
      })
      .finally(() => {
        this.inflight--;
        const queued = this.queuedEdit;
        this.queuedEdit = undefined;
        if (queued && !this.disposed && !this.lost) this.compile(queued);
      });
  }

  capture(request: string) {
    if (
      this.disposed ||
      this.lost ||
      this.captureRequest ||
      this.captureBuffer ||
      this.width * this.height > 4194304
    ) {
      this.host.postMessage({
        type: "capture",
        request,
        message: "Wait for the current capture, or make the preview smaller.",
      });
      return;
    }
    this.captureRequest = request;
  }

  finishCapture(
    buffer: GPUBuffer,
    request: string,
    width: number,
    height: number,
    bytesPerRow: number,
  ) {
    const format = this.format;
    const readback = Effect.scoped(
      Effect.gen(function* () {
        yield* Effect.acquireRelease(Effect.succeed(buffer), (b) =>
          Effect.sync(() => {
            if (b.mapState === "mapped") b.unmap();
            b.destroy();
          }),
        );
        yield* Effect.tryPromise({
          try: () => buffer.mapAsync(GPUMapMode.READ),
          catch: () => new Error("Couldn't read this frame."),
        });
        const source = new Uint8Array(buffer.getMappedRange()),
          pixels = new Uint8ClampedArray(width * height * 4);
        for (let y = 0; y < height; y++)
          pixels.set(
            source.subarray(y * bytesPerRow, y * bytesPerRow + width * 4),
            y * width * 4,
          );
        if (format.startsWith("bgra"))
          for (let i = 0; i < pixels.length; i += 4) {
            const red = pixels[i];
            pixels[i] = pixels[i + 2];
            pixels[i + 2] = red;
          }
        const image = new OffscreenCanvas(width, height),
          context = image.getContext("2d")!;
        context.putImageData(new ImageData(pixels, width, height), 0, 0);
        return yield* Effect.tryPromise({
          try: () => image.convertToBlob({ type: "image/png" }),
          catch: () => new Error("Couldn't encode this frame."),
        });
      }),
    );
    Effect.runPromise(readback)
      .then((blob) => {
        this.captureBuffer = undefined;
        this.host.postMessage({ type: "capture", request, blob, state: this.state() });
      })
      .catch(() => {
        this.captureBuffer = undefined;
        this.host.postMessage({
          type: "capture",
          request,
          message: "Couldn't capture this frame.",
          state: this.state(),
        });
      });
  }

  queueScene(message: {
    request: string;
    transaction?: Transaction;
    restore?: Project;
  }) {
    if (this.disposed || this.lost || this.scenePending) {
      this.host.postMessage({
        type: "scene-rejected",
        request: message.request,
        message: "Wait for the current scene change to finish.",
      });
      return;
    }
    this.scenePending = message;
  }

  applyScene() {
    const message = this.scenePending;
    this.scenePending = undefined;
    if (!message) return;
    try {
      let candidate: Project, commit: () => void;
      if (message.restore) {
        if (this.scene.project.revision !== 0)
          throw new Error("A saved scene can only open before editing begins.");
        candidate = readProject(message.restore);
        commit = () => {
          this.scene = new SceneHistory(candidate);
        };
      } else {
        const tx = message.transaction!;
        const freeze = (operation: Operation): Operation =>
          operation?.kind === "set_behavior" &&
          operation.behavior === "still" &&
          Array.isArray(operation.ids)
            ? { ...operation, positions: this.ghosts.positions(operation.ids) }
            : operation;
        const operation = tx.operation;
        const plan = this.scene.prepare({
          ...tx,
          operation:
            operation?.kind === "batch" && Array.isArray(operation.operations)
              ? { ...operation, operations: operation.operations.map(freeze) }
              : freeze(operation),
        });
        candidate = plan.project;
        commit = plan.commit;
      }
      if (
        IS_TOWN &&
        !this.medieval &&
        !message.restore &&
        JSON.stringify(
          candidate.entities.filter((e) => medievalKinds.has(e.kind)),
        ) !==
          JSON.stringify(
            this.scene.project.entities.filter((e) =>
              medievalKinds.has(e.kind),
            ),
          )
      )
        throw new Error(
          this.kitError ??
            "Prepare the medieval kit and reopen the preview before adding these assets.",
        );
      const nextFloor = projectFloor(candidate);
      const kitPlan =
        this.medieval && nextFloor instanceof TownFloor
          ? this.medieval.prepare(candidate.entities, nextFloor)
          : undefined;
      if (
        !(nextFloor instanceof TownFloor
          ? nextFloor.canStand(this.pose.x, this.pose.z, 0.24, this.pose.y ?? 0)
          : nextFloor.canStand(this.pose.x, this.pose.z))
      )
        throw new Error(
          "Step back into another room before applying or undoing this layout change.",
        );
      if (
        JSON.stringify(candidate.town) !==
          JSON.stringify(this.scene.project.town) ||
        JSON.stringify(candidate.layout) !==
          JSON.stringify(this.scene.project.layout) ||
        JSON.stringify(
          candidate.entities
            .filter(
              (e) => !medievalKinds.has(e.kind) || (IS_TOWN && isHouse(e.kind)),
            )
            .map(({ id, kind, x, z, scale }) => ({
              id,
              kind,
              x,
              z,
              scale,
            })),
        ) !==
          JSON.stringify(
            this.scene.project.entities
              .filter(
                (e) =>
                  !medievalKinds.has(e.kind) || (IS_TOWN && isHouse(e.kind)),
              )
              .map(({ id, kind, x, z, scale }) => ({
                id,
                kind,
                x,
                z,
                scale,
              })),
          )
      ) {
        const geometry =
          nextFloor instanceof TownFloor
            ? townGeometry(nextFloor, !this.medieval)
            : houseGeometry(
                candidate.entities,
                this.ranges,
                candidate.layout ?? DEFAULT_LAYOUT,
              );
        this.baseGeometry = geometry;
        this.motionBuffers.clear();
        this.motionPoses.clear();
        this.geometryBuilds++;
        const buffer = this.device.createBuffer({
          label: "Haunted House scene geometry",
          size: geometry.byteLength,
          usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(buffer, 0, geometry.buffer);
        const previous = this.vertices;
        this.vertices = buffer;
        this.count = geometry.length / 10;
        previous.destroy();
      }
      if (kitPlan) this.medieval!.apply(kitPlan);
      commit();
      this.floor = nextFloor;
      this.ghosts.sync(candidate.entities);
      for (const motion of this.ghosts.motions.values())
        if (!this.floor.canStand(motion.x, motion.z)) {
          motion.x = motion.homeX;
          motion.z = motion.homeZ;
        }
      return message;
    } catch (error) {
      this.host.postMessage({
        type: "scene-rejected",
        request: message.request,
        message: String(error),
        state: this.state(),
      });
    }
  }

  state() {
    return {
      medieval: {
        available: !!this.medieval && !this.disposed,
        error: this.kitError,
        instances: this.medieval?.instanceCount ?? 0,
        parts: this.medieval?.partCount ?? 0,
        meshes: this.medieval ? medievalKinds.size : 0,
        pipelines: this.medieval && !this.disposed ? 1 : 0,
        buffers: this.medieval && !this.disposed ? 2 : 0,
        textures: this.medieval && !this.disposed ? 1 : 0,
      },
      project: this.scene.project,
      actors: this.ghosts.snapshot(),
      behaviorTicks: this.ghosts.ticks,
      geometryBuilds: this.geometryBuilds,
      aimedWall: this.floor.pick(this.pose),
      stairs:
        this.floor instanceof TownFloor &&
        !!this.floor.stairAt(this.pose.x, this.pose.z),
      room:
        this.floor instanceof TownFloor
          ? this.floor.roomAt(this.pose.x, this.pose.z, this.pose.y ?? 0)
          : undefined,
      canUndo: this.scene.past.length > 0,
      canRedo: this.scene.future.length > 0,
      pose: { ...this.pose },
      frame: this.frame,
      deviceId: this.deviceId,
      draft: this.versions.draft,
      accepted: this.versions.active?.revision ?? 0,
      hash: this.versions.active?.hash ?? null,
      width: this.width,
      height: this.height,
      compiles: this.compiles,
      inflight: this.inflight,
      activePipelines: this.versions.active ? 1 : 0,
      pendingPipelines: this.versions.pending ? 1 : 0,
      buffers: this.disposed
        ? 0
        : 2 + (this.medieval ? 2 : 0) + (this.captureBuffer ? 1 : 0),
      depthTextures: this.disposed ? 0 : 1,
      disposed: this.disposed,
      paused: this.paused,
      maxFrameMs: this.maxFrameMs,
    };
  }

  animate(seconds: number) {
    this.ghosts.step(this.pose, seconds, this.floor.canStand, this.floor.guide);
    for (const motion of this.ghosts.motions.values()) {
      const range = this.ranges.get(motion.id);
      if (!range) continue;
      let last = this.motionPoses.get(motion.id);
      if (
        last &&
        last[0] === motion.x &&
        last[1] === motion.y &&
        last[2] === motion.z &&
        last[3] === motion.yaw
      )
        continue;
      if (!last) {
        last = [motion.x, motion.y, motion.z, motion.yaw];
        this.motionPoses.set(motion.id, last);
      } else {
        last[0] = motion.x;
        last[1] = motion.y;
        last[2] = motion.z;
        last[3] = motion.yaw;
      }
      let vertices = this.motionBuffers.get(motion.id);
      if (!vertices) {
        vertices = this.baseGeometry.slice(range.start, range.end);
        this.motionBuffers.set(motion.id, vertices);
      }
      const c = Math.cos(motion.yaw),
        s = Math.sin(motion.yaw);
      for (let i = range.start, j = 0; i < range.end; i += 10, j += 10) {
        const x = this.baseGeometry[i] - motion.homeX,
          z = this.baseGeometry[i + 2] - motion.homeZ;
        vertices[j] = motion.x + c * x + s * z;
        vertices[j + 1] = this.baseGeometry[i + 1] + motion.y;
        vertices[j + 2] = motion.z - s * x + c * z;
        vertices[j + 3] =
          c * this.baseGeometry[i + 3] + s * this.baseGeometry[i + 5];
        vertices[j + 5] =
          -s * this.baseGeometry[i + 3] + c * this.baseGeometry[i + 5];
      }
      this.device.queue.writeBuffer(
        this.vertices,
        range.start * 4,
        vertices.buffer,
      );
    }
  }

  pause() {
    if (this.paused || this.disposed || this.lost) return;
    this.paused = true;
    this.host.cancelAnimationFrame(this.frameId);
    this.keys = [];
  }

  resume() {
    if (!this.paused || this.disposed || this.lost) return;
    this.paused = false;
    this.previous = 0;
    this.keys = [];
    this.frameId = this.host.requestAnimationFrame((t) => this.draw(t));
  }

  draw(now: number) {
    if (this.disposed || this.lost || this.paused) return;
    const sceneChange = this.applyScene();
    const delta = this.previous ? now - this.previous : 0;
    this.previous = now;
    this.maxFrameMs = Math.max(this.maxFrameMs, delta);
    const held = (...keys: string[]) => keys.some((k) => this.keys.includes(k));
    const forward =
      Number(held("KeyW", "ArrowUp")) - Number(held("KeyS", "ArrowDown"));
    const right = Number(held("KeyD")) - Number(held("KeyA"));
    if (this.floor instanceof TownFloor)
      this.floor.move(this.pose, forward, right, delta / 1000);
    else
      move(
        this.pose,
        Number(held("KeyW", "ArrowUp")) - Number(held("KeyS", "ArrowDown")),
        Number(held("KeyD")) - Number(held("KeyA")),
        delta / 1000,
        this.floor.canStand,
      );
    if (held("ArrowLeft")) this.pose.yaw -= delta * 0.0015;
    if (held("ArrowRight")) this.pose.yaw += delta * 0.0015;
    this.animate(delta / 1000);
    const committed = this.versions.commit();
    cameraMatrix(this.uniformData, this.pose, this.width / this.height);
    this.uniformData.set(
      [this.pose.x, (this.pose.y ?? 0) + 1.55, this.pose.z, now / 1000],
      16,
    );
    this.uniformData.set(
      [this.pose.x + 0.3, (this.pose.y ?? 0) + 1.35, this.pose.z - 0.25, 1],
      20,
    );
    this.uniformData.set([...TINTS[this.scene.project.tint], 1], 24);
    this.device.queue.writeBuffer(this.uniform, 0, this.uniformData.buffer);
    const encoder = this.device.createCommandEncoder();
    const texture = this.context.getCurrentTexture();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: texture.createView(),
          clearValue: IS_TOWN
            ? { r: 0.64, g: 0.76, b: 0.85, a: 1 }
            : { r: 0.055, g: 0.075, b: 0.085, a: 1 },
          loadOp: "clear",
          storeOp: "store",
        },
      ],
      depthStencilAttachment: {
        view: this.depth!.createView(),
        depthClearValue: 1,
        depthLoadOp: "clear",
        depthStoreOp: "store",
      },
    });
    if (this.versions.active) {
      pass.setPipeline(this.versions.active.resource);
      pass.setBindGroup(0, this.bindGroup);
      pass.setVertexBuffer(0, this.vertices);
      pass.draw(this.count);
    }
    this.medieval?.draw(pass);
    pass.end();
    const capture = this.captureRequest;
    this.captureRequest = undefined;
    const captureWidth = this.width,
      captureHeight = this.height,
      bytesPerRow = Math.ceil((this.width * 4) / 256) * 256;
    if (capture) {
      this.captureBuffer = this.device.createBuffer({
        label: "Frame capture",
        size: bytesPerRow * captureHeight,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
      encoder.copyTextureToBuffer(
        { texture },
        {
          buffer: this.captureBuffer,
          bytesPerRow,
          rowsPerImage: captureHeight,
        },
        [captureWidth, captureHeight],
      );
    }
    this.device.queue.submit([encoder.finish()]);
    if (capture)
      this.finishCapture(
        this.captureBuffer!,
        capture,
        captureWidth,
        captureHeight,
        bytesPerRow,
      );
    this.frame++;
    if (sceneChange)
      this.host.postMessage({
        type: "scene-accepted",
        request: sceneChange.request,
        visibleAt: performance.timeOrigin + performance.now(),
        state: this.state(),
      });
    if (committed && this.pendingEdit) {
      const edit = this.pendingEdit;
      this.pendingEdit = undefined;
      // Receipt follows submission of the first frame using this pipeline.
      this.host.postMessage({
        type: "accepted",
        revision: edit.revision,
        request: edit.request,
        hash: edit.hash,
        visibleAt: performance.timeOrigin + performance.now(),
        state: this.state(),
      });
    }
    if (now - this.lastReport > 100) {
      this.host.postMessage({ type: "state", state: this.state() });
      this.lastReport = now;
    }
    this.frameId = this.host.requestAnimationFrame((t) => this.draw(t));
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.host.cancelAnimationFrame(this.frameId);
    this.keys = [];
    this.motionBuffers.clear();
    this.motionPoses.clear();
    this.ghosts.motions.clear();
    this.versions.dispose();
    this.pendingEdit = undefined;
    this.queuedEdit = undefined;
    this.scenePending = undefined;
    this.captureRequest = undefined;
    this.captureBuffer?.destroy();
    this.captureBuffer = undefined;
    this.depth?.destroy();
    this.medieval?.dispose();
    this.vertices.destroy();
    this.uniform.destroy();
    this.context.unconfigure();
    this.device.destroy();
    this.host.postMessage({ type: "disposed", state: this.state() });
  }
}

export function installPreviewWorker(
  scope: DedicatedWorkerGlobalScope,
): void {
  let preview = "",
    token = "";
  let runtime: Runtime | undefined;
  let endLifetime: (() => void) | undefined;
  const send = (message: object) =>
    scope.postMessage({ ...message, preview, token });

  scope.onmessage = (event: MessageEvent) => {
  const message = event.data;
  if (message?.type === "init" && !preview) {
    if (
      typeof message.preview !== "string" ||
      typeof message.token !== "string" ||
      !(message.canvas instanceof OffscreenCanvas)
    )
      return;
    preview = message.preview;
    token = message.token;
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const renderer = yield* Effect.acquireRelease(
            Effect.tryPromise({
              try: async () => {
                const gpu = scope.navigator.gpu;
                if (!gpu)
                  throw new Error(
                    "This browser cannot run WebGPU. Try a current Chrome or Edge with graphics acceleration enabled.",
                  );
                const adapter = await gpu.requestAdapter();
                if (!adapter)
                  throw new Error(
                    "WebGPU could not find a graphics device. Enable graphics acceleration and reopen this preview.",
                  );
                const device = await adapter.requestDevice();
                try {
                  const context = message.canvas.getContext(
                    "webgpu",
                  ) as GPUCanvasContext | null;
                  if (!context)
                    throw new Error("WebGPU could not open the canvas.");
                  const format = gpu.getPreferredCanvasFormat();
                  context.configure({
                    device,
                    format,
                    alphaMode: "opaque",
                    usage:
                      GPUTextureUsage.RENDER_ATTACHMENT |
                      GPUTextureUsage.COPY_SRC,
                  });
                  const renderer = new Runtime(
                    device,
                    context,
                    message.canvas,
                    format,
                    {
                      postMessage: (message) => send(message),
                      requestAnimationFrame: (callback) => scope.requestAnimationFrame(callback),
                      cancelAnimationFrame: (id) => scope.cancelAnimationFrame(id),
                    },
                  );
                  try {
                    if (IS_TOWN)
                      renderer.medieval = await MedievalRenderer.create(
                        device,
                        renderer.uniform,
                        format,
                      );
                  } catch (error) {
                    renderer.kitError =
                      "The medieval kit could not load. Prepare it again and reopen the preview.";
                    console.warn("Medieval kit load failed", error);
                  }
                  if (
                    renderer.medieval &&
                    renderer.floor instanceof TownFloor
                  ) {
                    renderer.medieval.apply(
                      renderer.medieval.prepare(
                        renderer.scene.project.entities,
                        renderer.floor,
                      ),
                    );
                    const geometry = townGeometry(renderer.floor);
                    const buffer = device.createBuffer({
                      label: "Town floors and stairs",
                      size: geometry.byteLength,
                      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
                    });
                    device.queue.writeBuffer(buffer, 0, geometry.buffer);
                    renderer.vertices.destroy();
                    renderer.vertices = buffer;
                    renderer.baseGeometry = geometry;
                    renderer.count = geometry.length / 10;
                  }
                  return renderer;
                } catch (error) {
                  device.destroy();
                  throw error;
                }
              },
              catch: (e) => (e instanceof Error ? e : new Error(String(e))),
            }),
            (r) => Effect.sync(() => r.dispose()),
          );
          runtime = renderer;
          const info = renderer.device.adapterInfo;
          send({
            type: "ready",
            adapter: info
              ? {
                  vendor: info.vendor,
                  architecture: info.architecture,
                  device: info.device,
                  description: info.description,
                }
              : null,
            state: renderer.state(),
          });
          yield* Effect.promise(
            () =>
              new Promise<void>((resolve) => {
                endLifetime = resolve;
              }),
          );
        }),
      ),
    ).catch((error) => send({ type: "unsupported", message: String(error) }));
    return;
  }
  if (!runtime || message?.preview !== preview || message?.token !== token)
    return;
  if (message.type === "pause") {
    runtime.pause();
    send({ type: "state", state: runtime.state() });
    return;
  }
  if (message.type === "resume") {
    runtime.resume();
    send({ type: "state", state: runtime.state() });
    return;
  }
  if (
    message.type === "scene" &&
    message.project === PROJECT_ID &&
    typeof message.request === "string" &&
    message.request.length <= 128
  ) {
    runtime.queueScene(message);
    return;
  }
  if (
    message.type === "capture" &&
    typeof message.request === "string" &&
    message.request.length <= 128
  ) {
    runtime.capture(message.request);
    return;
  }
  if (message.type === "shader") {
    if (validEdit(message, preview, token)) runtime.edit(message);
    return;
  }
  if (
    message.type === "resize" &&
    Number.isFinite(message.width) &&
    Number.isFinite(message.height)
  )
    runtime.resize(message.width, message.height);
  if (message.type === "input" && Array.isArray(message.keys))
    runtime.keys = message.keys
      .filter((k: unknown) => typeof k === "string")
      .slice(0, 12);
  if (
    message.type === "look" &&
    Number.isFinite(message.dx) &&
    Number.isFinite(message.dy)
  ) {
    runtime.pose.yaw += Math.max(-500, Math.min(500, message.dx)) * 0.003;
    runtime.pose.pitch = Math.max(
      -1.2,
      Math.min(1.2, runtime.pose.pitch - message.dy * 0.003),
    );
  }
  if (message.type === "dispose") endLifetime?.();
  };
}
