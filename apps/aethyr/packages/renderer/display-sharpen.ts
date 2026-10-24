import type { SceneLook } from "../assets/src/scene.ts";

export const SOURCE_SHARPEN_LUMINANCE = [0.212639, 0.715169, 0.072192] as const;
export const SOURCE_SHARPEN_UV_OFFSET = 0.001;

/** Admit only the recovered enabled, unbound source recipe. Other post materials remain unsupported. */
export function sourceSharpenStrength(
  look: SceneLook | undefined,
  mode: "day" | "night",
): number {
  let strength = 0;
  for (const actor of look?.scenarios[mode].environment ?? []) {
    const p = actor.properties as Record<string, unknown> | undefined;
    if (
      actor.class !== "PostProcessVolume" ||
      p?.enabled !== true ||
      p.unbound !== true ||
      typeof p.settings !== "string"
    )
      continue;
    const weight = typeof p.blend_weight === "number" ? p.blend_weight : 0;
    const match = p.settings.match(
      /WeightedBlendables=\(Array=\(\(Weight=([0-9.]+),Object="[^"\n]*\/MM_Sharp\.MM_Sharp'"/,
    );
    if (match && Number.isFinite(weight))
      strength = Math.max(
        strength,
        Math.min(1, Math.max(0, weight * Number(match[1]))),
      );
  }
  return strength;
}

/** Signed grayscale high-pass is added equally to each display-linear channel. */
export function sharpenDisplayPixel(
  center: readonly number[],
  neighbors: readonly (readonly number[])[],
  strength: number,
): number[] {
  if (
    center.length !== 3 ||
    neighbors.length !== 4 ||
    neighbors.some((v) => v.length !== 3) ||
    ![...center, ...neighbors.flat(), strength].every(Number.isFinite) ||
    strength < 0 ||
    strength > 1
  )
    throw new Error(
      "Sharpening needs finite RGB samples, four neighbors, and strength from zero to one.",
    );
  const highPass = center.map(
    (v, i) => v - neighbors.reduce((sum, n) => sum + n[i], 0) * 0.25,
  );
  const gray =
    highPass.reduce((sum, v, i) => sum + v * SOURCE_SHARPEN_LUMINANCE[i], 0) *
    strength;
  return center.map((v) => Math.max(0, Math.min(1, v + gray)));
}

export const DISPLAY_SHARPEN_WGSL = `
@group(0)@binding(0)var displayImage:texture_2d<f32>;
@group(0)@binding(1)var pointSampler:sampler;
@group(0)@binding(2)var<uniform>settings:vec4<f32>;
struct Out{@builtin(position)position:vec4<f32>,@location(0)uv:vec2<f32>};
@vertex fn vs(@builtin(vertex_index)i:u32)->Out{let q=vec2(f32((i<<1u)&2u),f32(i&2u));var out:Out;out.position=vec4(q*2.0-1.0,0.0,1.0);out.uv=vec2(q.x,1.0-q.y);return out;}
fn srgb(value:vec3<f32>)->vec3<f32>{let c=clamp(value,vec3(0.0),vec3(1.0));return select(c*12.92,1.055*pow(c,vec3(1.0/2.4))-vec3(0.055),c>vec3(0.0031308));}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{
 let center=textureSampleLevel(displayImage,pointSampler,v.uv,0.0).rgb;
 let offset=settings.y;
 let neighbors=(textureSampleLevel(displayImage,pointSampler,v.uv+vec2(offset,0.0),0.0).rgb+textureSampleLevel(displayImage,pointSampler,v.uv-vec2(offset,0.0),0.0).rgb+textureSampleLevel(displayImage,pointSampler,v.uv+vec2(0.0,offset),0.0).rgb+textureSampleLevel(displayImage,pointSampler,v.uv-vec2(0.0,offset),0.0).rgb)*0.25;
 let highPass=dot(center-neighbors,vec3(0.212639,0.715169,0.072192))*settings.x;
 return vec4(srgb(center+vec3(highPass)),1.0);
}
`;

/** Separates the scene-to-display transform from the source after-tone filter. */
export class DisplaySharpen {
  private image!: GPUTexture;
  private group!: GPUBindGroup;
  private width = 0;
  private height = 0;
  private constructor(
    private readonly device: GPUDevice,
    private readonly pipeline: GPURenderPipeline,
    private readonly settings: GPUBuffer,
    private readonly sampler: GPUSampler,
  ) {
    this.resize(1, 1);
  }
  static async create(device: GPUDevice, format: GPUTextureFormat) {
    const module = device.createShaderModule({ code: DISPLAY_SHARPEN_WGSL });
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: { module, entryPoint: "fs", targets: [{ format }] },
    });
    const settings = device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    return new DisplaySharpen(
      device,
      pipeline,
      settings,
      device.createSampler({ minFilter: "nearest", magFilter: "nearest" }),
    );
  }
  get view() {
    return this.image.createView();
  }
  resize(width: number, height: number) {
    if (width === this.width && height === this.height) return;
    if (
      ![width, height].every(
        (v) =>
          Number.isInteger(v) &&
          v > 0 &&
          v <= this.device.limits.maxTextureDimension2D,
      )
    )
      throw new Error("Use a supported display size.");
    this.image?.destroy();
    this.width = width;
    this.height = height;
    this.image = this.device.createTexture({
      label: "Display-linear color",
      size: [width, height],
      format: "rgba16float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.group = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: this.image.createView() },
        { binding: 1, resource: this.sampler },
        { binding: 2, resource: { buffer: this.settings } },
      ],
    });
  }
  render(encoder: GPUCommandEncoder, target: GPUTextureView, strength: number) {
    if (!Number.isFinite(strength) || strength < 0 || strength > 1)
      throw new Error("Use a sharpening strength from zero to one.");
    this.device.queue.writeBuffer(
      this.settings,
      0,
      new Float32Array([strength, SOURCE_SHARPEN_UV_OFFSET, 0, 0]),
    );
    const pass = encoder.beginRenderPass({
      label: "Display sharpening",
      colorAttachments: [{ view: target, loadOp: "clear", storeOp: "store" }],
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group);
    pass.draw(3);
    pass.end();
  }
  dispose() {
    this.image.destroy();
    this.settings.destroy();
  }
}
