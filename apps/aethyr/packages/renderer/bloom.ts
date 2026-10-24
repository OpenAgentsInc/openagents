export const BLOOM_THRESHOLD = 1;
export const BLOOM_MAX_SOURCE_SIZE = 16_384;
export const BLOOM_MAX_SIZE = [640, 360] as const;
export const BLOOM_RADIUS = 4;
export const BLOOM_SIGMA = 2;

/** Normalized, contiguous Gaussian samples; index zero is the center. */
export const BLOOM_WEIGHTS: readonly number[] = (() => {
  const weights = Array.from({ length: BLOOM_RADIUS + 1 }, (_, i) =>
    Math.exp(-(i * i) / (2 * BLOOM_SIGMA * BLOOM_SIGMA)),
  );
  const sum = weights[0] + 2 * weights.slice(1).reduce((a, b) => a + b, 0);
  return Object.freeze(weights.map((v) => v / sum));
})();

/** Half resolution, capped without stretching the input's aspect ratio. */
export function bloomSize(width: number, height: number): [number, number] {
  if (
    ![width, height].every(
      (v) => Number.isInteger(v) && v > 0 && v <= BLOOM_MAX_SOURCE_SIZE,
    )
  )
    throw new Error("Use a supported bloom source size.");
  const scale = Math.min(
    0.5,
    BLOOM_MAX_SIZE[0] / width,
    BLOOM_MAX_SIZE[1] / height,
  );
  return [
    Math.min(BLOOM_MAX_SIZE[0], Math.max(1, Math.ceil(width * scale))),
    Math.min(BLOOM_MAX_SIZE[1], Math.max(1, Math.ceil(height * scale))),
  ];
}

export const BLOOM_WGSL = `
@group(0)@binding(0)var source:texture_2d<f32>;
@group(0)@binding(1)var<uniform>sizes:vec4<f32>;
const weights=array<f32,${BLOOM_RADIUS + 1}>(${BLOOM_WEIGHTS.map((v) => v.toPrecision(12)).join(",")});
@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.0-1.0,0.0,1.0);}
@fragment fn prefilter(@builtin(position)p:vec4<f32>)->@location(0)vec4<f32>{
 let low=vec2<f32>(textureDimensions(source))/sizes.zw;
 let first=(floor(p.xy))*low;let last=first+low;
 let begin=vec2<i32>(floor(first));let end=vec2<i32>(ceil(last));
 let limit=vec2<i32>(textureDimensions(source))-vec2(1);
 var sum=vec3(0.0);
 // Integrate every source texel footprint, including odd dimensions and capped views.
 // With the 16384 source limit, neither axis visits more than 47 texels.
 for(var y=begin.y;y<end.y;y++){for(var x=begin.x;x<end.x;x++){
  let coverage=max(vec2(0.0),min(last,vec2(f32(x+1),f32(y+1)))-max(first,vec2(f32(x),f32(y))));
  let value=max(textureLoad(source,clamp(vec2(x,y),vec2(0),limit),0).rgb-vec3(${BLOOM_THRESHOLD}.0),vec3(0.0));
  sum+=value*coverage.x*coverage.y;
 }}
 return vec4(sum/(low.x*low.y),1.0);
}
fn blurred(p:vec2<i32>,axis:vec2<i32>)->vec4<f32>{
 let limit=vec2<i32>(textureDimensions(source))-vec2(1);
 var value=textureLoad(source,clamp(p,vec2(0),limit),0).rgb*weights[0];
 for(var i=1;i<=${BLOOM_RADIUS};i++){
  value+=(textureLoad(source,clamp(p+axis*i,vec2(0),limit),0).rgb+textureLoad(source,clamp(p-axis*i,vec2(0),limit),0).rgb)*weights[i];
 }
 return vec4(value,1.0);
}
@fragment fn horizontal(@builtin(position)p:vec4<f32>)->@location(0)vec4<f32>{return blurred(vec2<i32>(p.xy),vec2(1,0));}
@fragment fn vertical(@builtin(position)p:vec4<f32>)->@location(0)vec4<f32>{return blurred(vec2<i32>(p.xy),vec2(0,1));}
`;

export interface BloomFrame {
  width: number;
  height: number;
  /** Resolved scene HDR after fog and particles, matching width/height; never the bloom output. */
  source: GPUTextureView;
  enabled: boolean;
}

/** Three bounded HDR passes. Exposure and the user's bloom strength belong to POST. */
export class Bloom {
  private images: GPUTexture[] = [];
  private views: GPUTextureView[] = [];
  private groups: GPUBindGroup[] = [];
  private input?: { source: GPUTextureView; group: GPUBindGroup };
  private width = 0;
  private height = 0;
  private sourceWidth = 0;
  private sourceHeight = 0;
  private black = false;
  private disposed = false;

  private constructor(
    private readonly device: GPUDevice,
    private readonly pipelines: readonly GPURenderPipeline[],
    private readonly layout: GPUBindGroupLayout,
    private readonly settings: GPUBuffer,
  ) {}

  static async create(device: GPUDevice): Promise<Bloom> {
    const layout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        {
          binding: 1,
          visibility: GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform" },
        },
      ],
    });
    const pipelineLayout = device.createPipelineLayout({
      bindGroupLayouts: [layout],
    });
    const module = device.createShaderModule({
      label: "Contiguous HDR bloom",
      code: BLOOM_WGSL,
    });
    const pipelines: GPURenderPipeline[] = [];
    // Pipeline failures happen before any owned buffers or images are allocated.
    for (const entryPoint of ["prefilter", "horizontal", "vertical"])
      pipelines.push(
        await device.createRenderPipelineAsync({
          layout: pipelineLayout,
          vertex: { module, entryPoint: "vs" },
          fragment: {
            module,
            entryPoint,
            targets: [{ format: "rgba16float" }],
          },
        }),
      );
    const settings = device.createBuffer({
      label: "Bloom dimensions",
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const bloom = new Bloom(device, pipelines, layout, settings);
    try {
      bloom.resize(1, 1);
      return bloom;
    } catch (error) {
      bloom.dispose();
      throw error;
    }
  }

  /** Stable until the low-resolution dimensions change. */
  get view(): GPUTextureView {
    if (this.disposed) throw new Error("Open bloom before reading its output.");
    return this.views[2];
  }

  /** Exact owned image and uniform bytes; pipelines and bindings are not byte-addressable. */
  get bytes(): number {
    return this.disposed ? 0 : this.width * this.height * 8 * 3 + 16;
  }

  get size(): [number, number] {
    return [this.width, this.height];
  }

  resize(width: number, height: number): void {
    if (this.disposed) throw new Error("Open bloom before resizing it.");
    const [w, h] = bloomSize(width, height);
    if (
      [width, height, w, h].some(
        (v) => v > this.device.limits.maxTextureDimension2D,
      )
    )
      throw new Error("Use a supported bloom source size.");
    if (w !== this.width || h !== this.height) {
      const images: GPUTexture[] = [];
      try {
        for (const name of ["Bright pass", "Horizontal bloom", "Bloom output"])
          images.push(
            this.device.createTexture({
              label: name,
              size: [w, h],
              format: "rgba16float",
              usage:
                GPUTextureUsage.RENDER_ATTACHMENT |
                GPUTextureUsage.TEXTURE_BINDING |
                GPUTextureUsage.COPY_SRC,
            }),
          );
        const views = images.map((image) => image.createView());
        const groups = views.slice(0, 2).map((source) => this.group(source));
        this.images.forEach((image) => image.destroy());
        this.images = images;
        this.views = views;
        this.groups = groups;
        this.width = w;
        this.height = h;
        this.input = undefined;
        this.black = false;
      } catch (error) {
        images.forEach((image) => image.destroy());
        throw error;
      }
    }
    if (width !== this.sourceWidth || height !== this.sourceHeight) {
      this.device.queue.writeBuffer(
        this.settings,
        0,
        new Float32Array([width, height, w, h]),
      );
      this.sourceWidth = width;
      this.sourceHeight = height;
    }
  }

  private group(source: GPUTextureView): GPUBindGroup {
    return this.device.createBindGroup({
      layout: this.layout,
      entries: [
        { binding: 0, resource: source },
        { binding: 1, resource: { buffer: this.settings } },
      ],
    });
  }

  /** Submit each encoded frame in order; retain source view identity to reuse its binding. */
  render(encoder: GPUCommandEncoder, frame: BloomFrame): void {
    this.resize(frame.width, frame.height);
    if (!frame.enabled) {
      if (!this.black) {
        const clear = encoder.beginRenderPass({
          colorAttachments: [
            {
              view: this.view,
              loadOp: "clear",
              storeOp: "store",
              clearValue: [0, 0, 0, 0],
            },
          ],
        });
        clear.end();
        this.black = true;
      }
      return;
    }
    if (this.views.includes(frame.source))
      throw new Error("Use the scene color as the bloom source.");
    if (this.input?.source !== frame.source)
      this.input = { source: frame.source, group: this.group(frame.source) };
    for (let i = 0; i < 3; i++) {
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: this.views[i],
            loadOp: "clear",
            storeOp: "store",
            clearValue: [0, 0, 0, 0],
          },
        ],
      });
      pass.setPipeline(this.pipelines[i]);
      pass.setBindGroup(0, i === 0 ? this.input!.group : this.groups[i - 1]);
      pass.draw(3);
      pass.end();
    }
    this.black = false;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.images.forEach((image) => image.destroy());
    this.settings.destroy();
    this.images = [];
    this.views = [];
    this.groups = [];
    this.input = undefined;
  }
}
