export const EMPTY_WORLD_CLEAR = Object.freeze({
  r: 0.06,
  g: 0.09,
  b: 0.08,
  a: 1,
});
const captureLimit = { width: 1920, height: 1080, bytes: 32 * 1024 * 1024 };
function dimensions(width: number, height: number, capture = false) {
  const maxWidth = capture ? captureLimit.width : 16_384;
  const maxHeight = capture ? captureLimit.height : 16_384;
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > maxWidth ||
    height > maxHeight
  )
    throw new Error(
      capture
        ? "Choose a screenshot size up to 1920 by 1080 pixels."
        : "Choose a positive supported world view size.",
    );
}

/** Remove mapped row padding and normalize the browser's canvas format to RGBA. */
export function unpackEmptyPixels(
  source: Uint8Array,
  width: number,
  height: number,
  bytesPerRow: number,
  format: GPUTextureFormat,
): ArrayBuffer {
  dimensions(width, height, true);
  if (
    !Number.isInteger(bytesPerRow) ||
    bytesPerRow < width * 4 ||
    bytesPerRow % 256 !== 0 ||
    source.byteLength < bytesPerRow * height ||
    bytesPerRow * height > captureLimit.bytes ||
    !["rgba8unorm", "bgra8unorm"].includes(format)
  )
    throw new Error(
      "The screenshot pixels do not match their size and color format.",
    );
  const output = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    const row = source.subarray(y * bytesPerRow, y * bytesPerRow + width * 4);
    output.set(row, y * width * 4);
  }
  if (format === "bgra8unorm")
    for (let pixel = 0; pixel < output.length; pixel += 4)
      [output[pixel], output[pixel + 2]] = [output[pixel + 2], output[pixel]];
  return output.buffer;
}

/** One clear-only WebGPU world shared by the minimal player and editor preview. */
export class EmptyWorld {
  private disposed = false;
  private capturing = false;
  private frames = 0;
  private captures = 0;
  private readbackPeak = 0;
  private constructor(
    private readonly canvas: HTMLCanvasElement | OffscreenCanvas,
    readonly device: GPUDevice,
    private readonly context: GPUCanvasContext,
    private readonly format: GPUTextureFormat,
  ) {}

  static async create(
    canvas: HTMLCanvasElement | OffscreenCanvas,
    gpu: GPU | undefined = globalThis.navigator?.gpu,
  ): Promise<EmptyWorld> {
    const adapter = await gpu?.requestAdapter();
    if (!adapter)
      throw new Error(
        "This browser cannot open this world. Try a browser with WebGPU support.",
      );
    const device = await adapter.requestDevice();
    try {
      const context = canvas.getContext("webgpu") as GPUCanvasContext | null;
      if (!context)
        throw new Error("This browser could not create the world view.");
      const format = gpu!.getPreferredCanvasFormat();
      if (format !== "rgba8unorm" && format !== "bgra8unorm")
        throw new Error("This browser uses an unsupported world color format.");
      const world = new EmptyWorld(canvas, device, context, format);
      world.configure(Math.max(1, canvas.width), Math.max(1, canvas.height));
      return world;
    } catch (error) {
      device.destroy();
      throw error;
    }
  }
  private active() {
    if (this.disposed) throw new Error("This world is closed.");
  }
  private configure(width: number, height: number) {
    dimensions(width, height);
    const limit = this.device.limits.maxTextureDimension2D;
    if (width > limit || height > limit)
      throw new Error("Choose a world view size supported by this device.");
    this.canvas.width = width;
    this.canvas.height = height;
    this.context.configure({
      device: this.device,
      format: this.format,
      alphaMode: "opaque",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
  }
  resize(width: number, height: number) {
    this.active();
    if (this.capturing)
      throw new Error("Let the screenshot finish before resizing the world.");
    this.configure(width, height);
  }
  private clear(encoder: GPUCommandEncoder, texture: GPUTexture) {
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: texture.createView(),
          clearValue: EMPTY_WORLD_CLEAR,
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    });
    pass.end();
  }
  render() {
    this.active();
    if (this.capturing) return;
    const encoder = this.device.createCommandEncoder();
    this.clear(encoder, this.context.getCurrentTexture());
    this.device.queue.submit([encoder.finish()]);
    this.frames++;
  }
  async capturePixels(width: number, height: number): Promise<ArrayBuffer> {
    this.active();
    dimensions(width, height, true);
    if (this.capturing)
      throw new Error("Let the current screenshot finish first.");
    this.capturing = true;
    const old = { width: this.canvas.width, height: this.canvas.height };
    let buffer: GPUBuffer | undefined;
    let mapped = false;
    try {
      this.configure(width, height);
      const bytesPerRow = Math.ceil((width * 4) / 256) * 256;
      const size = bytesPerRow * height;
      if (size > captureLimit.bytes)
        throw new Error("Choose a smaller screenshot size.");
      buffer = this.device.createBuffer({
        size,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
      this.readbackPeak = Math.max(this.readbackPeak, size);
      const encoder = this.device.createCommandEncoder();
      const texture = this.context.getCurrentTexture();
      this.clear(encoder, texture);
      encoder.copyTextureToBuffer(
        { texture },
        { buffer, bytesPerRow, rowsPerImage: height },
        [width, height],
      );
      this.device.queue.submit([encoder.finish()]);
      this.frames++;
      await buffer.mapAsync(GPUMapMode.READ);
      mapped = true;
      this.active();
      const pixels = unpackEmptyPixels(
        new Uint8Array(buffer.getMappedRange()),
        width,
        height,
        bytesPerRow,
        this.format,
      );
      this.captures++;
      return pixels;
    } finally {
      if (mapped) buffer?.unmap();
      buffer?.destroy();
      this.capturing = false;
      if (!this.disposed) {
        this.configure(old.width, old.height);
        this.render();
      }
    }
  }
  benchmarkInfo() {
    return {
      frame: this.frames,
      submittedFrames: this.frames,
      width: this.canvas.width,
      height: this.canvas.height,
      gpuAccountedBytes: 0,
      readbackHighWaterBytes: this.readbackPeak,
      captures: this.captures,
      disposed: this.disposed,
    };
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    try {
      this.context.unconfigure();
    } finally {
      this.device.destroy();
    }
  }
}
