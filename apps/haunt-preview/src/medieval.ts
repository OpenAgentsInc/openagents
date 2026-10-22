import { TownFloor, isHouse, type KitInstance } from "./town.ts";
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
 @location(4) layer: f32, @location(5) instance: vec4<f32>, @location(6) size: vec3<f32>
}
struct Surface {
 @builtin(position) pos: vec4<f32>, @location(0) world: vec3<f32>,
 @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>,
 @location(3) uv: vec2<f32>, @location(4) @interpolate(flat) layer: u32
}
@vertex fn vertex_main(v: Input) -> Surface {
 var o: Surface;
 let c=cos(v.instance.w); let s=sin(v.instance.w); let p=v.pos*v.size; o.world=vec3<f32>(c*p.x+s*p.z,p.y,-s*p.x+c*p.z)+v.instance.xyz;
 o.pos = scene.view_proj * vec4<f32>(o.world, 1);
 let n=v.normal/v.size; o.normal=normalize(vec3<f32>(c*n.x+s*n.z,n.y,-s*n.x+c*n.z)); o.color = v.color; o.uv = v.uv; o.layer = u32(v.layer);
 return o;
}
@fragment fn fragment_main(v: Surface) -> @location(0) vec4<f32> {
 let tex = textureSample(images, filtering, v.uv, i32(v.layer));
 if (tex.a < 0.5) { discard; }
 let to_light = scene.light.xyz-v.world;
 let lamp = 9.0/(3.0+dot(to_light,to_light));
 let diffuse = max(dot(normalize(v.normal),normalize(to_light)),0.0);
 let daylight = 0.62 + 0.38 * max(dot(normalize(v.normal),normalize(vec3<f32>(0.4,0.8,0.3))),0.0);
 var color = tex.rgb * v.color * vec3<f32>(daylight) * scene.tint.xyz;
 let fog = 1.0-exp(-length(scene.eye_time.xyz-v.world)*0.004);
 color = mix(color,vec3<f32>(0.64,0.76,0.85),fog);
 return vec4<f32>(pow(max(color,vec3<f32>(0)),vec3<f32>(0.8)),1);
}`;
type Mesh = { first: number; count: number; dimensions: number[] };
export const medievalKinds = new Set(
  Object.keys(pinned.meshes).filter((k) => !k.startsWith("piece/")),
);
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
  partCount = 0;
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
        size: 8192 * 32,
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
              arrayStride: 32,
              stepMode: "instance",
              attributes: [
                { shaderLocation: 5, offset: 0, format: "float32x4" },
                { shaderLocation: 6, offset: 16, format: "float32x3" },
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
  prepare(assets: SceneAsset[], floor: TownFloor) {
    const groups = new Map<string, { mesh: Mesh; instances: KitInstance[] }>();
    const add = (mesh: Mesh, instance: KitInstance) => {
      const key = `${mesh.first}:${mesh.count}`;
      if (!groups.has(key)) groups.set(key, { mesh, instances: [] });
      groups.get(key)!.instances.push(instance);
    };
    for (const asset of assets) {
      const mesh = pinned.meshes[asset.kind as keyof typeof pinned.meshes];
      if (!mesh) continue;
      const instance = {
        mesh: asset.kind,
        x: asset.x,
        y: asset.y ?? 0,
        z: asset.z,
        yaw: 0,
        sx: asset.scale,
        sy: asset.scale,
        sz: asset.scale,
      };
      if (isHouse(asset.kind)) {
        for (const part of mesh.parts)
          if (part.role === "roof")
            add({ ...part, dimensions: mesh.dimensions }, instance);
      } else add(mesh, instance);
    }
    for (const piece of floor.pieces) {
      const mesh = pinned.meshes[piece.mesh as keyof typeof pinned.meshes];
      if (!mesh)
        throw new Error("This building piece is unavailable. Reopen the lab.");
      add(mesh, piece);
    }
    const values: number[] = [],
      draws: { mesh: Mesh; first: number; count: number }[] = [];
    for (const group of groups.values()) {
      draws.push({
        mesh: group.mesh,
        first: values.length / 8,
        count: group.instances.length,
      });
      for (const p of group.instances)
        values.push(p.x, p.y, p.z, p.yaw, p.sx, p.sy, p.sz, 0);
    }
    if (values.length / 8 > 8192)
      throw new Error(
        "This town has too many building pieces. Remove a room before adding more.",
      );
    return { values: new Float32Array(values), draws, objects: assets.length };
  }
  apply(plan: ReturnType<MedievalRenderer["prepare"]>) {
    if (plan.values.length)
      this.device.queue.writeBuffer(this.instances, 0, plan.values);
    this.draws = plan.draws;
    this.instanceCount = plan.objects;
    this.partCount = plan.values.length / 8;
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
