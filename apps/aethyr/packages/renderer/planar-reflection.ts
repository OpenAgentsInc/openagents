import { inverse, multiply, type V3 } from "./math.ts";

export interface PlanarCamera {
  viewProjection: Float32Array;
  inverseViewProjection: Float32Array;
  eye: V3;
  /** Keep world-space fragments whose dot(vec4(position, 1), clipPlane) is nonnegative. Sky is exempt. */
  clipPlane: [number, number, number, number];
  frontFace: "cw";
  planeY: number;
}

/** Mirrors the actual view transform, including its handedness, across horizontal water. */
export function planarCamera(
  viewProjection: ArrayLike<number>,
  eye: readonly number[],
  planeY: number,
  clipBias = 0.01,
): PlanarCamera {
  if (
    viewProjection.length !== 16 ||
    !Array.from(viewProjection).every(Number.isFinite) ||
    eye.length !== 3 ||
    !eye.every(Number.isFinite) ||
    !Number.isFinite(planeY) ||
    Math.abs(planeY) > 100_000 ||
    !Number.isFinite(clipBias) ||
    clipBias < 0 ||
    clipBias > 0.1
  )
    throw new Error(
      "A planar reflection needs a finite camera, horizontal plane, and bounded clip bias.",
    );
  const mirror = [1, 0, 0, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 2 * planeY, 0, 1];
  const reflected = multiply(viewProjection, mirror);
  const inverseViewProjection = inverse(reflected);
  if (![...reflected, ...inverseViewProjection].every(Number.isFinite))
    throw new Error("The mirrored camera exceeds the finite matrix range.");
  return {
    viewProjection: reflected,
    inverseViewProjection,
    eye: [eye[0], 2 * planeY - eye[1], eye[2]],
    clipPlane: [0, 1, 0, -planeY - clipBias],
    frontFace: "cw",
    planeY,
  };
}

export interface PlanarReflectionRequest {
  viewProjection: ArrayLike<number>;
  eye: readonly number[];
  planeY: number;
  width: number;
  height: number;
  /** Covers source/authored geometry, materials, and the active scene shader. */
  sceneRevision: string | number;
  lightingRevision: string | number;
  environmentRevision: string | number;
  clipBias?: number;
  /** Disable for animated contents, or vary an explicit revision with their time bucket. */
  cache?: boolean;
}
export interface PlanarRenderContext extends PlanarCamera {
  width: number;
  height: number;
}
export interface PlanarReflectionResult {
  /** Host may sample this view but must not destroy the module's texture. */
  view: GPUTextureView;
  viewProjection: Float32Array;
  planeY: number;
  width: number;
  height: number;
  updated: boolean;
  /** Call only after the containing encoder was successfully submitted. */
  markSubmitted(): void;
}

/** Owns one linear HDR reflection. The host owns shaders, scene resources, and submission. */
export class PlanarReflection {
  private color?: GPUTexture;
  private resolved?: GPUTexture;
  private depth?: GPUTexture;
  private size = "";
  private cachedKey?: string;
  private generation = 0;
  private disposed = false;
  private readonly scale: number;
  private readonly maxDimension: number;
  private readonly format: GPUTextureFormat;
  private readonly depthFormat: GPUTextureFormat;

  constructor(
    private readonly device: GPUDevice,
    options: {
      resolutionScale?: number;
      maxDimension?: number;
      format?: GPUTextureFormat;
      depthFormat?: GPUTextureFormat;
    } = {},
  ) {
    this.scale = options.resolutionScale ?? 0.5;
    this.maxDimension = options.maxDimension ?? 1024;
    this.format = options.format ?? "rgba16float";
    this.depthFormat = options.depthFormat ?? "depth24plus";
    if (
      !Number.isFinite(this.scale) ||
      this.scale <= 0 ||
      this.scale > 0.5 ||
      !Number.isInteger(this.maxDimension) ||
      this.maxDimension < 1 ||
      this.maxDimension > 1024 ||
      this.maxDimension > device.limits.maxTextureDimension2D
    )
      throw new Error(
        "Planar reflections use at most half resolution with a 1024-pixel long edge.",
      );
  }

  /** Callback must draw the host scene with CW pipelines, clipPlane enabled, and all water excluded. */
  render(
    encoder: GPUCommandEncoder,
    request: PlanarReflectionRequest,
    draw: (pass: GPURenderPassEncoder, context: PlanarRenderContext) => void,
  ): PlanarReflectionResult | undefined {
    if (this.disposed) throw new Error("This planar reflection is closed.");
    if (
      ![request.width, request.height].every(
        (n) =>
          Number.isInteger(n) &&
          n > 0 &&
          n <= this.device.limits.maxTextureDimension2D,
      ) ||
      ![
        request.sceneRevision,
        request.lightingRevision,
        request.environmentRevision,
      ].every((value) =>
        typeof value === "string"
          ? value.length <= 1024
          : typeof value === "number" && Number.isFinite(value),
      )
    )
      throw new Error(
        "Planar reflections need valid dimensions and explicit finite scene revisions.",
      );
    const camera = planarCamera(
      request.viewProjection,
      request.eye,
      request.planeY,
      request.clipBias,
    );
    if (request.eye[1] <= request.planeY + (request.clipBias ?? 0.01))
      return undefined;
    const scale = Math.min(
      this.scale,
      this.maxDimension / Math.max(request.width, request.height),
    );
    const width = Math.max(1, Math.floor(request.width * scale)),
      height = Math.max(1, Math.floor(request.height * scale));
    const size = `${width}:${height}`;
    const key = JSON.stringify([
      Array.from(request.viewProjection),
      Array.from(request.eye),
      request.planeY,
      request.clipBias ?? 0.01,
      width,
      height,
      request.sceneRevision,
      request.lightingRevision,
      request.environmentRevision,
    ]);
    if (this.size !== size) {
      this.releaseTargets();
      this.color = this.device.createTexture({
        label: "Planar HDR MSAA",
        size: [width, height],
        format: this.format,
        sampleCount: 4,
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      });
      this.resolved = this.device.createTexture({
        label: "Planar linear HDR",
        size: [width, height],
        format: this.format,
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
      this.depth = this.device.createTexture({
        label: "Planar depth",
        size: [width, height],
        format: this.depthFormat,
        sampleCount: 4,
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      });
      this.size = size;
    }
    const view = this.resolved!.createView();
    if (request.cache !== false && this.cachedKey === key)
      return {
        view,
        viewProjection: camera.viewProjection,
        planeY: request.planeY,
        width,
        height,
        updated: false,
        markSubmitted() {},
      };
    this.cachedKey = undefined;
    const generation = ++this.generation;
    const pass = encoder.beginRenderPass({
      label: "Local planar reflection (water excluded)",
      colorAttachments: [
        {
          view: this.color!.createView(),
          resolveTarget: view,
          loadOp: "clear",
          storeOp: "discard",
          clearValue: [0, 0, 0, 1],
        },
      ],
      depthStencilAttachment: {
        view: this.depth!.createView(),
        depthLoadOp: "clear",
        depthStoreOp: "discard",
        depthClearValue: 1,
      },
    });
    try {
      draw(pass, { ...camera, width, height });
    } finally {
      pass.end();
    }
    return {
      view,
      viewProjection: camera.viewProjection,
      planeY: request.planeY,
      width,
      height,
      updated: true,
      markSubmitted: () => {
        if (
          !this.disposed &&
          generation === this.generation &&
          request.cache !== false
        )
          this.cachedKey = key;
      },
    };
  }
  invalidate() {
    this.cachedKey = undefined;
    this.generation++;
  }
  private releaseTargets() {
    this.invalidate();
    this.color?.destroy();
    this.resolved?.destroy();
    this.depth?.destroy();
    this.color = undefined;
    this.resolved = undefined;
    this.depth = undefined;
    this.size = "";
  }
  dispose() {
    if (this.disposed) return;
    this.releaseTargets();
    this.disposed = true;
  }
}
