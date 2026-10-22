import pinned from "./medieval-kit.json" with { type: "json" };
import type { SceneAsset } from "./project.ts";

const shader = `
struct Scene { view_proj: mat4x4<f32>, eye_time: vec4<f32>, light: vec4<f32>, tint: vec4<f32> }
@group(0) @binding(0) var<uniform> scene: Scene;
@group(0) @binding(1) var images: texture_2d_array<f32>;
@group(0) @binding(2) var filtering: sampler;
struct Input {
 @location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>,
 @location(2) color: vec3<f32>, @location(3) uv: vec2<f32>,
 @location(4) layer: f32, @location(5) instance: vec3<f32>
}
struct Surface {
 @builtin(position) pos: vec4<f32>, @location(0) world: vec3<f32>,
 @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>,
 @location(3) uv: vec2<f32>, @location(4) @interpolate(flat) layer: u32
}
@vertex fn vertex_main(v: Input) -> Surface {
 var o: Surface;
 o.world = v.pos * v.instance.z + vec3<f32>(v.instance.x, 0, v.instance.y);
 o.pos = scene.view_proj * vec4<f32>(o.world, 1);
 o.normal = v.normal; o.color = v.color; o.uv = v.uv; o.layer = u32(v.layer);
 return o;
}
@fragment fn fragment_main(v: Surface) -> @location(0) vec4<f32> {
 let tex = textureSample(images, filtering, v.uv, i32(v.layer));
 if (tex.a < 0.5) { discard; }
 let to_light = scene.light.xyz-v.world;
 let lamp = 9.0/(3.0+dot(to_light,to_light));
 let diffuse = max(dot(normalize(v.normal),normalize(to_light)),0.0);
 let daylight = 0.32 + 0.28 * max(dot(normalize(v.normal),normalize(vec3<f32>(0.4,0.8,0.3))),0.0);
 var color = tex.rgb * v.color * (vec3<f32>(daylight) + scene.tint.xyz * vec3<f32>(1,0.72,0.36) * diffuse * lamp);
 let fog = 1.0-exp(-length(scene.eye_time.xyz-v.world)*0.018);
 color = mix(color,vec3<f32>(0.055,0.075,0.085),fog);
 return vec4<f32>(pow(max(color,vec3<f32>(0)),vec3<f32>(0.8)),1);
}`;
type Mesh = { first: number; count: number; dimensions: number[] };
export const medievalKinds = new Set(Object.keys(pinned.meshes));
async function verified(name: keyof typeof pinned.files) {
  const response = await fetch(`/medieval-kit/${name}`);
  if (!response.ok)
    throw new Error(
      "The medieval kit could not load. Reopen the preview after preparing the kit.",
    );
  const bytes = await response.arrayBuffer();
  const hash = [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((x) => x.toString(16).padStart(2, "0"))
    .join("");
  if (
    bytes.byteLength !== pinned.files[name].bytes ||
    hash !== pinned.files[name].sha256
  )
    throw new Error(
      "The medieval kit files changed. Prepare the reviewed kit before reopening.",
    );
  return bytes;
}
/** Immutable kit geometry and images; edits update only a small instance buffer. */
export class MedievalRenderer {
  vertices: GPUBuffer;
  instances: GPUBuffer;
  texture: GPUTexture;
  pipeline: GPURenderPipeline;
  group: GPUBindGroup;
  draws: { mesh: Mesh; first: number; count: number }[] = [];
  instanceCount = 0;
  constructor(
    private device: GPUDevice,
    vertices: GPUBuffer,
    instances: GPUBuffer,
    texture: GPUTexture,
    pipeline: GPURenderPipeline,
    group: GPUBindGroup,
  ) {
    this.vertices = vertices;
    this.instances = instances;
    this.texture = texture;
    this.pipeline = pipeline;
    this.group = group;
  }
  static async create(
    device: GPUDevice,
    uniform: GPUBuffer,
    format: GPUTextureFormat,
  ) {
    const available = await fetch("/medieval-kit/catalog.json");
    if (!available.ok) return undefined;
    const [bytes, png] = await Promise.all([
      verified("meshes.bin"),
      verified("atlas.png"),
    ]);
    const bitmap = await createImageBitmap(
      new Blob([png], { type: "image/png" }),
      { colorSpaceConversion: "none" },
    );
    let vertices: GPUBuffer | undefined,
      instances: GPUBuffer | undefined,
      texture: GPUTexture | undefined;
    try {
      vertices = device.createBuffer({
        label: "Medieval kit geometry",
        size: bytes.byteLength,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      });
      device.queue.writeBuffer(vertices, 0, bytes);
      instances = device.createBuffer({
        label: "Medieval kit instances",
        size: 64 * 12,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      });
      texture = device.createTexture({
        label: "Medieval kit base colors",
        size: [256, 256, pinned.layers],
        format: "rgba8unorm-srgb",
        usage:
          GPUTextureUsage.TEXTURE_BINDING |
          GPUTextureUsage.COPY_DST |
          GPUTextureUsage.RENDER_ATTACHMENT,
      });
      for (let i = 0; i < pinned.layers; i++)
        device.queue.copyExternalImageToTexture(
          { source: bitmap, origin: [(i % 8) * 256, Math.floor(i / 8) * 256] },
          { texture, origin: [0, 0, i] },
          [256, 256],
        );
      const module = device.createShaderModule({
        label: "Medieval kit textured material",
        code: shader,
      });
      const pipeline = await device.createRenderPipelineAsync({
        label: "Medieval kit instances",
        layout: "auto",
        vertex: {
          module,
          entryPoint: "vertex_main",
          buffers: [
            {
              arrayStride: 48,
              attributes: [
                { shaderLocation: 0, offset: 0, format: "float32x3" },
                { shaderLocation: 1, offset: 12, format: "float32x3" },
                { shaderLocation: 2, offset: 24, format: "float32x3" },
                { shaderLocation: 3, offset: 36, format: "float32x2" },
                { shaderLocation: 4, offset: 44, format: "float32" },
              ],
            },
            {
              arrayStride: 12,
              stepMode: "instance",
              attributes: [
                { shaderLocation: 5, offset: 0, format: "float32x3" },
              ],
            },
          ],
        },
        fragment: {
          module,
          entryPoint: "fragment_main",
          targets: [{ format }],
        },
        primitive: { topology: "triangle-list", cullMode: "none" },
        depthStencil: {
          format: "depth24plus",
          depthWriteEnabled: true,
          depthCompare: "less",
        },
      });
      const group = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: uniform } },
          {
            binding: 1,
            resource: texture.createView({ dimension: "2d-array" }),
          },
          {
            binding: 2,
            resource: device.createSampler({
              addressModeU: "repeat",
              addressModeV: "repeat",
              magFilter: "linear",
              minFilter: "linear",
            }),
          },
        ],
      });
      return new MedievalRenderer(
        device,
        vertices,
        instances,
        texture,
        pipeline,
        group,
      );
    } catch (error) {
      vertices?.destroy();
      instances?.destroy();
      texture?.destroy();
      throw error;
    } finally {
      bitmap.close();
    }
  }
  update(assets: SceneAsset[]) {
    const values: number[] = [];
    this.draws = [];
    for (const [kind, mesh] of Object.entries(pinned.meshes)) {
      const members = assets.filter((a) => a.kind === kind);
      if (!members.length) continue;
      this.draws.push({
        mesh,
        first: values.length / 3,
        count: members.length,
      });
      for (const a of members) values.push(a.x, a.z, a.scale);
    }
    this.instanceCount = values.length / 3;
    if (values.length)
      this.device.queue.writeBuffer(
        this.instances,
        0,
        new Float32Array(values),
      );
  }
  draw(pass: GPURenderPassEncoder) {
    if (!this.draws.length) return;
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group);
    pass.setVertexBuffer(0, this.vertices);
    pass.setVertexBuffer(1, this.instances);
    for (const d of this.draws)
      pass.draw(d.mesh.count, d.count, d.mesh.first, d.first);
  }
  dispose() {
    this.vertices.destroy();
    this.instances.destroy();
    this.texture.destroy();
    this.draws = [];
  }
}
