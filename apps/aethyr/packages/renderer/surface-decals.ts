import { TexturePool } from "./texture-pool.ts";

export interface SurfaceDecalMaterial {
  id: string;
  /** Straight alpha and sRGB color, with the source color/grunge graph baked. */
  colorOpacityUrl: string;
  normalUrl?: string;
  mraUrl?: string;
  uvTransform: readonly [number, number, number, number];
  /** Source FlattenNormal input; values outside 0..1 are intentional. */
  normalFlatness: number;
  /** Already a tangent vector, not an encoded RGB normal map. */
  normalConstant: readonly [number, number, number];
  roughnessIntensity: number;
  /** Unreal's specular amount; packed storage converts it to dielectric F0. */
  sourceSpecularAmount: number;
  metallic: number;
  source?: unknown;
}

export interface SurfaceDecalInstance {
  id: string;
  material: number;
  sourceOrder: number;
  sortOrder: number;
  worldFromDecal: readonly number[];
  worldToDecal: readonly number[];
  bounds: { min: readonly number[]; max: readonly number[] };
  /** Retained source setting; its exact distance-fade curve is not implemented. */
  fadeScreenSize: number;
  sourceActor?: string;
  sourceComponent?: string;
}

export interface SurfaceDecalManifest {
  schema: "aethyrforge.surface-decals.v1";
  materials: readonly SurfaceDecalMaterial[];
  instances: readonly SurfaceDecalInstance[];
  source?: unknown;
  limitations?: readonly string[];
}

export interface SurfaceDecalFrame {
  width: number;
  height: number;
  /** Existing stored 4x opaque/masked depth; not attached during this pass. */
  depthView: GPUTextureView;
  viewProjection: ArrayLike<number>;
  inverseViewProjection: ArrayLike<number>;
}

export interface SurfaceDecalStats {
  total: number;
  visible: number;
  draws: number;
  /** Conservative projected rectangles, summed in units of viewport area. */
  coverage: number;
  bytes: number;
}

export const SURFACE_DECAL_LIMITS = {
  instances: 4096,
  materials: 256,
  maxDimension: 8192,
} as const;
const INSTANCE_FLOATS = 40;
const MATERIAL_FLOATS = 16;
const CORNERS = Array.from({ length: 8 }, (_, i) => [
  i & 1 ? 1 : -1,
  i & 2 ? 1 : -1,
  i & 4 ? 1 : -1,
]);

function finiteArray(value: unknown, length: number): value is number[] {
  return (
    Array.isArray(value) &&
    value.length === length &&
    value.every(
      (v) =>
        typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= 1_000_000,
    )
  );
}
function affine(value: unknown): value is number[] {
  return (
    finiteArray(value, 16) &&
    Math.abs(value[3]) + Math.abs(value[7]) + Math.abs(value[11]) < 1e-7 &&
    Math.abs(value[15] - 1) < 1e-7
  );
}
function privateMap(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.startsWith("/private-assets/") &&
    !value.includes("\\") &&
    !value.includes("..") &&
    !value.includes("%") &&
    !value.includes("?") &&
    !value.includes("#")
  );
}
function point(m: ArrayLike<number>, x: number, y: number, z: number) {
  return [
    m[0] * x + m[4] * y + m[8] * z + m[12],
    m[1] * x + m[5] * y + m[9] * z + m[13],
    m[2] * x + m[6] * y + m[10] * z + m[14],
    m[3] * x + m[7] * y + m[11] * z + m[15],
  ];
}

/** Validate private source records before their values reach GPU storage. */
export function validateSurfaceDecals(value: SurfaceDecalManifest): void {
  if (
    !value ||
    value.schema !== "aethyrforge.surface-decals.v1" ||
    !Array.isArray(value.materials) ||
    value.materials.length > SURFACE_DECAL_LIMITS.materials ||
    !Array.isArray(value.instances) ||
    value.instances.length > SURFACE_DECAL_LIMITS.instances
  )
    throw new Error(
      "Use a supported surface decal collection within its limits.",
    );
  const ids = new Set<string>();
  for (const m of value.materials) {
    if (
      !m ||
      typeof m.id !== "string" ||
      !m.id ||
      ids.has(m.id) ||
      !privateMap(m.colorOpacityUrl) ||
      (m.normalUrl !== undefined && !privateMap(m.normalUrl)) ||
      (m.mraUrl !== undefined && !privateMap(m.mraUrl)) ||
      !finiteArray(m.uvTransform, 4) ||
      !finiteArray(m.normalConstant, 3) ||
      ![
        m.normalFlatness,
        m.roughnessIntensity,
        m.sourceSpecularAmount,
        m.metallic,
      ].every((v) => Number.isFinite(v) && Math.abs(v) <= 1_000_000) ||
      Math.hypot(...m.normalConstant) < 1e-8
    )
      throw new Error(
        "A surface decal material has invalid maps or parameters.",
      );
    ids.add(m.id);
  }
  ids.clear();
  const orders = new Set<number>();
  for (const d of value.instances as readonly SurfaceDecalInstance[]) {
    if (
      !d ||
      typeof d.id !== "string" ||
      !d.id ||
      d.id.length > 1024 ||
      ids.has(d.id) ||
      !Number.isSafeInteger(d.material) ||
      d.material < 0 ||
      d.material >= value.materials.length ||
      !Number.isSafeInteger(d.sourceOrder) ||
      d.sourceOrder < 0 ||
      orders.has(d.sourceOrder) ||
      !Number.isInteger(d.sortOrder) ||
      Math.abs(d.sortOrder) > 2147483647 ||
      !affine(d.worldFromDecal) ||
      !affine(d.worldToDecal) ||
      !d.bounds ||
      !finiteArray(d.bounds.min, 3) ||
      !finiteArray(d.bounds.max, 3) ||
      !Number.isFinite(d.fadeScreenSize) ||
      d.fadeScreenSize < 0
    )
      throw new Error(
        "A surface decal needs a unique order and finite volume.",
      );
    ids.add(d.id);
    orders.add(d.sourceOrder);
    for (let column = 0; column < 4; column++) {
      const inverse = point(
        d.worldFromDecal,
        d.worldToDecal[column * 4],
        d.worldToDecal[column * 4 + 1],
        d.worldToDecal[column * 4 + 2],
      );
      // The first three columns are directions, so remove the affine offset.
      for (let row = 0; row < 4; row++) {
        const n = inverse[row] - (column < 3 ? d.worldFromDecal[12 + row] : 0);
        if (Math.abs(n - (column === row ? 1 : 0)) > 1e-3)
          throw new Error("The surface decal transforms must be inverses.");
      }
    }
    for (let axis = 0; axis < 3; axis++) {
      const column = d.worldFromDecal.slice(axis * 4, axis * 4 + 3);
      if (Math.hypot(...column) < 1e-8)
        throw new Error("The surface decal volume must have nonzero size.");
      if (d.bounds.min[axis] > d.bounds.max[axis])
        throw new Error("The surface decal bounds must have positive size.");
    }
    for (const [a, b] of [
      [0, 1],
      [0, 2],
      [1, 2],
    ]) {
      const x = d.worldFromDecal.slice(a * 4, a * 4 + 3);
      const y = d.worldFromDecal.slice(b * 4, b * 4 + 3);
      const dot = x.reduce((sum, v, i) => sum + v * y[i], 0);
      if (Math.abs(dot) / (Math.hypot(...x) * Math.hypot(...y)) > 1e-4)
        throw new Error(
          "Surface decal transforms support rotation and scale without shear.",
        );
    }
    for (const c of CORNERS) {
      const p = point(d.worldFromDecal, c[0], c[1], c[2]);
      if (
        p
          .slice(0, 3)
          .some(
            (v, axis) =>
              Math.abs(v) > 100_000 ||
              v < d.bounds.min[axis] - 1e-3 ||
              v > d.bounds.max[axis] + 1e-3,
          )
      )
        throw new Error("The surface decal bounds must contain its volume.");
    }
  }
}

export interface SurfaceDecalDraw {
  instance: SurfaceDecalInstance;
  /** Integer screen rectangle, including a pixel of conservative padding. */
  scissor: readonly [number, number, number, number];
}

/** Keep source order while rejecting offscreen volumes and bounding raster work. */
export function buildSurfaceDecalDraws(
  instances: readonly SurfaceDecalInstance[],
  viewProjection: ArrayLike<number>,
  width: number,
  height: number,
): SurfaceDecalDraw[] {
  if (
    viewProjection.length !== 16 ||
    !Array.from(viewProjection).every(Number.isFinite) ||
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > SURFACE_DECAL_LIMITS.maxDimension ||
    height > SURFACE_DECAL_LIMITS.maxDimension
  )
    throw new Error("Use finite camera data and a supported decal image size.");
  const result: SurfaceDecalDraw[] = [];
  const clip = new Float64Array(32);
  for (const instance of [...instances].sort(
    (a, b) => a.sortOrder - b.sortOrder || a.sourceOrder - b.sourceOrder,
  )) {
    const m = instance.worldFromDecal;
    let left = true,
      right = true,
      below = true,
      above = true,
      near = true,
      far = true,
      positive = true;
    for (let i = 0; i < 8; i++) {
      const c = CORNERS[i];
      const x = m[0] * c[0] + m[4] * c[1] + m[8] * c[2] + m[12];
      const y = m[1] * c[0] + m[5] * c[1] + m[9] * c[2] + m[13];
      const z = m[2] * c[0] + m[6] * c[1] + m[10] * c[2] + m[14];
      const at = i * 4;
      for (let axis = 0; axis < 4; axis++)
        clip[at + axis] =
          viewProjection[axis] * x +
          viewProjection[axis + 4] * y +
          viewProjection[axis + 8] * z +
          viewProjection[axis + 12];
      const cx = clip[at],
        cy = clip[at + 1],
        cz = clip[at + 2],
        cw = clip[at + 3];
      left &&= cx < -cw;
      right &&= cx > cw;
      below &&= cy < -cw;
      above &&= cy > cw;
      near &&= cz < 0;
      far &&= cz > cw;
      positive &&= cw > 1e-7 && cz >= 0;
    }
    if (left || right || below || above || near || far) continue;
    let scissor: [number, number, number, number] = [0, 0, width, height];
    // Eye/near-plane intersections retain the full viewport. Keep the original
    // double-precision operation order and one-pixel conservative padding.
    if (positive) {
      let minX = Infinity,
        maxX = -Infinity,
        minY = Infinity,
        maxY = -Infinity;
      for (let i = 0; i < 8; i++) {
        const at = i * 4;
        const x = ((clip[at] / clip[at + 3]) * 0.5 + 0.5) * width;
        const y = (0.5 - (clip[at + 1] / clip[at + 3]) * 0.5) * height;
        minX = Math.min(minX, x);
        maxX = Math.max(maxX, x);
        minY = Math.min(minY, y);
        maxY = Math.max(maxY, y);
      }
      const x0 = Math.max(0, Math.floor(minX) - 1);
      const y0 = Math.max(0, Math.floor(minY) - 1);
      const x1 = Math.min(width, Math.ceil(maxX) + 1);
      const y1 = Math.min(height, Math.ceil(maxY) + 1);
      if (x1 <= x0 || y1 <= y0) continue;
      scissor = [x0, y0, x1 - x0, y1 - y0];
    }
    result.push({ instance, scissor });
  }
  return result;
}

export function packSurfaceDecalDraws(
  draws: readonly SurfaceDecalDraw[],
  width: number,
  height: number,
): Float32Array {
  const data = new Float32Array(draws.length * INSTANCE_FLOATS);
  for (let i = 0; i < draws.length; i++) {
    const { instance, scissor: r } = draws[i],
      offset = i * INSTANCE_FLOATS;
    data.set(instance.worldFromDecal, offset);
    data.set(instance.worldToDecal, offset + 16);
    data.set(
      [
        (r[0] / width) * 2 - 1,
        1 - ((r[1] + r[3]) / height) * 2,
        ((r[0] + r[2]) / width) * 2 - 1,
        1 - (r[1] / height) * 2,
        instance.material,
        1,
        0,
        0,
      ],
      offset + 32,
    );
  }
  return data;
}

export function packSurfaceDecalMaterials(
  materials: readonly SurfaceDecalMaterial[],
  index: (url: string, role: "color" | "normal" | "data") => number,
): Float32Array {
  const result = new Float32Array(
    Math.max(1, materials.length) * MATERIAL_FLOATS,
  );
  for (let i = 0; i < materials.length; i++) {
    const m = materials[i];
    result.set(
      [
        ...m.uvTransform,
        m.normalFlatness,
        m.roughnessIntensity,
        Math.max(0, Math.min(1, m.sourceSpecularAmount)) * 0.08,
        Math.max(0, Math.min(1, m.metallic)),
        ...m.normalConstant,
        m.normalUrl ? 1 : 0,
        index(m.colorOpacityUrl, "color"),
        m.normalUrl ? index(m.normalUrl, "normal") : 0,
        m.mraUrl ? index(m.mraUrl, "data") : 0,
        0,
      ],
      i * MATERIAL_FLOATS,
    );
  }
  return result;
}

/** CPU reference for the same premultiplied source-over accumulation as the MRTs. */
export function accumulateSurfaceDecal(
  accumulator: readonly [number, number, number, number],
  value: readonly [number, number, number],
  opacity: number,
): [number, number, number, number] {
  const a = Math.max(0, Math.min(1, opacity));
  return [
    value[0] * a + accumulator[0] * (1 - a),
    value[1] * a + accumulator[1] * (1 - a),
    value[2] * a + accumulator[2] * (1 - a),
    accumulator[3] * (1 - a),
  ];
}

export const SURFACE_DECAL_COMPOSE_WGSL = `
struct SurfaceDecalResult {base:vec3<f32>,normal:vec3<f32>,properties:vec3<f32>,colorTransmittance:f32};
// properties=(metallic,dielectric F0,roughness). Caller gates receiver/response bits.
fn composeSurfaceDecals(image:texture_2d_array<f32>,pixel:vec2<i32>,base:vec3<f32>,normal:vec3<f32>,properties:vec3<f32>,response:u32)->SurfaceDecalResult{
 let color=textureLoad(image,pixel,0,0);let n=textureLoad(image,pixel,1,0);let p=textureLoad(image,pixel,2,0);
 var out:SurfaceDecalResult;out.base=base;out.normal=normal;out.properties=properties;out.colorTransmittance=1.0;
 if (response&1u)!=0u{out.base=base*color.a+color.rgb;out.colorTransmittance=color.a;}
 if (response&2u)!=0u{let mixed=normal*n.a+n.rgb;out.normal=mixed/max(length(mixed),1e-8);}
 if (response&4u)!=0u{out.properties=properties*p.a+p.rgb;}
 return out;
}`;

export const SURFACE_DECAL_WGSL = `
struct Frame{inverseViewProjection:mat4x4<f32>,extent:vec4<f32>};
struct Instance{worldFromDecal:mat4x4<f32>,worldToDecal:mat4x4<f32>,rectangle:vec4<f32>,params:vec4<f32>};
struct Material{uv:vec4<f32>,params:vec4<f32>,normal:vec4<f32>,maps:vec4<f32>};
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var depthImage:texture_depth_multisampled_2d;
@group(0)@binding(2)var<storage,read>instances:array<Instance>;
@group(0)@binding(3)var<storage,read>materials:array<Material>;
@group(0)@binding(4)var colors:texture_2d_array<f32>;
@group(0)@binding(5)var normals:texture_2d_array<f32>;
@group(0)@binding(6)var packed:texture_2d_array<f32>;
@group(0)@binding(7)var filtering:sampler;
struct V{@builtin(position)position:vec4<f32>,@location(0)@interpolate(flat)instance:u32};
@vertex fn vs(@builtin(vertex_index)vertex:u32,@builtin(instance_index)instance:u32)->V{
 let corners=array<vec2<f32>,6>(vec2(0.0,0.0),vec2(1.0,0.0),vec2(0.0,1.0),vec2(0.0,1.0),vec2(1.0,0.0),vec2(1.0,1.0));
 let r=instances[instance].rectangle;var out:V;out.position=vec4(mix(r.xy,r.zw,corners[vertex]),0.0,1.0);out.instance=instance;return out;
}
struct Outputs{@location(0)color:vec4<f32>,@location(1)normal:vec4<f32>,@location(2)properties:vec4<f32>};
@fragment fn fs(v:V)->Outputs{
 let pixel=clamp(vec2<i32>(v.position.xy),vec2(0),vec2<i32>(frame.extent.xy)-vec2(1));
 var depth=1.0;for(var sample=0;sample<4;sample++){depth=min(depth,textureLoad(depthImage,pixel,sample));}
 let ndc=vec2((f32(pixel.x)+0.5)/frame.extent.x*2.0-1.0,1.0-(f32(pixel.y)+0.5)/frame.extent.y*2.0);
 let homogeneous=frame.inverseViewProjection*vec4(ndc,depth,1.0);let world=homogeneous.xyz/select(-max(abs(homogeneous.w),1e-8),max(abs(homogeneous.w),1e-8),homogeneous.w>=0.0);
 let instance=instances[v.instance];let material=materials[u32(instance.params.x)];let local=(instance.worldToDecal*vec4(world,1.0)).xyz;
 let uv=local.zy*0.5+0.5;let dx=dpdx(uv);let dy=dpdy(uv);let atlasUV=uv*material.uv.xy+material.uv.zw;
 let color=textureSampleGrad(colors,filtering,uv,i32(material.maps.x),dx,dy);
 let normalMap=textureSampleGrad(normals,filtering,atlasUV,i32(material.maps.y),dx*material.uv.xy,dy*material.uv.xy).xyz*2.0-1.0;
 let mra=textureSampleGrad(packed,filtering,atlasUV,i32(material.maps.z),dx*material.uv.xy,dy*material.uv.xy);
 if depth>=1.0||any(abs(local)>vec3(1.0)){discard;}
 let alpha=clamp(color.a*clamp(4.0-4.0*abs(local.x),0.0,1.0)*instance.params.y,0.0,1.0);
 if alpha<=0.0{discard;}
 var tangent=material.normal.xyz;if material.normal.w>0.5{tangent=mix(normalMap,vec3(0.0,0.0,1.0),material.params.x);}
 let x=normalize(instance.worldFromDecal[0].xyz);let y=normalize(instance.worldFromDecal[1].xyz);let z=normalize(instance.worldFromDecal[2].xyz);
 let direction=-tangent.z*x+tangent.y*y+tangent.x*z;let normal=direction/max(length(direction),1e-8);
 let roughness=clamp(material.params.y*select(1.0,mra.g,material.maps.z>0.5),0.0,1.0);
 var out:Outputs;out.color=vec4(color.rgb*alpha,alpha);out.normal=vec4(normal*alpha,alpha);out.properties=vec4(vec3(material.params.w,material.params.z,roughness)*alpha,alpha);return out;
}`;

/** One ordered instanced raster pass; each rectangle clips against its own volume. */
export class SurfaceDecalPass {
  private output: GPUTexture;
  private outputView: GPUTextureView;
  private width = 1;
  private height = 1;
  private readonly frameBuffer: GPUBuffer;
  private readonly instanceBuffer: GPUBuffer;
  private readonly materialBuffer: GPUBuffer;
  private readonly sampler: GPUSampler;
  private constructor(
    private readonly device: GPUDevice,
    readonly manifest: SurfaceDecalManifest,
    private readonly pools: Record<"color" | "normal" | "data", TexturePool>,
    private readonly pipeline: GPURenderPipeline,
  ) {
    this.frameBuffer = device.createBuffer({
      size: 80,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.instanceBuffer = device.createBuffer({
      size: Math.max(1, manifest.instances.length) * INSTANCE_FLOATS * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    const materialData = packSurfaceDecalMaterials(
      manifest.materials,
      (url, role) => pools[role].index(url),
    );
    this.materialBuffer = device.createBuffer({
      size: materialData.byteLength,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(this.materialBuffer, 0, materialData);
    this.sampler = device.createSampler({
      addressModeU: "repeat",
      addressModeV: "repeat",
      minFilter: "linear",
      magFilter: "linear",
      mipmapFilter: "linear",
    });
    this.output = this.createOutput(1, 1);
    this.outputView = this.output.createView({ dimension: "2d-array" });
    device.queue.writeTexture(
      { texture: this.output },
      new Uint16Array([0, 0, 0, 0x3c00, 0, 0, 0, 0x3c00, 0, 0, 0, 0x3c00]),
      { bytesPerRow: 8, rowsPerImage: 1 },
      [1, 1, 3],
    );
  }
  static async create(
    device: GPUDevice,
    manifest: SurfaceDecalManifest,
  ): Promise<SurfaceDecalPass> {
    validateSurfaceDecals(manifest);
    const roles = ["color", "normal", "data"] as const;
    const results = await Promise.allSettled(
      roles.map((role) =>
        TexturePool.create(
          device,
          role,
          manifest.materials.flatMap((m) => {
            const url =
              role === "color"
                ? m.colorOpacityUrl
                : role === "normal"
                  ? m.normalUrl
                  : m.mraUrl;
            return url ? [url] : [];
          }),
        ),
      ),
    );
    const pools = {} as Record<(typeof roles)[number], TexturePool>;
    for (let i = 0; i < roles.length; i++) {
      const result = results[i];
      if (result.status === "fulfilled") pools[roles[i]] = result.value;
    }
    try {
      const failed = results.find((r) => r.status === "rejected");
      if (failed?.status === "rejected") throw failed.reason;
      const module = device.createShaderModule({
        label: "Source surface decals",
        code: SURFACE_DECAL_WGSL,
      });
      const info = await module.getCompilationInfo();
      if (info.messages.some((m) => m.type === "error"))
        throw new Error("The surface decal shader could not be prepared.");
      const blend: GPUBlendState = {
        color: {
          operation: "add",
          srcFactor: "one",
          dstFactor: "one-minus-src-alpha",
        },
        alpha: {
          operation: "add",
          srcFactor: "zero",
          dstFactor: "one-minus-src-alpha",
        },
      };
      const pipeline = await device.createRenderPipelineAsync({
        layout: "auto",
        vertex: { module, entryPoint: "vs" },
        fragment: {
          module,
          entryPoint: "fs",
          targets: Array.from({ length: 3 }, () => ({
            format: "rgba16float" as const,
            blend,
          })),
        },
        primitive: { topology: "triangle-list", cullMode: "none" },
      });
      return new SurfaceDecalPass(device, manifest, pools, pipeline);
    } catch (error) {
      for (const pool of Object.values(pools)) pool.texture.destroy();
      throw error;
    }
  }
  private createOutput(width: number, height: number): GPUTexture {
    return this.device.createTexture({
      label: "Surface decal accumulation",
      size: [width, height, 3],
      format: "rgba16float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT |
        GPUTextureUsage.TEXTURE_BINDING |
        GPUTextureUsage.COPY_SRC |
        GPUTextureUsage.COPY_DST,
    });
  }
  /** Call before rebuilding the host's bind group; the view changes only on resize. */
  resize(width: number, height: number): boolean {
    if (
      !Number.isInteger(width) ||
      !Number.isInteger(height) ||
      width < 1 ||
      height < 1 ||
      width > SURFACE_DECAL_LIMITS.maxDimension ||
      height > SURFACE_DECAL_LIMITS.maxDimension
    )
      throw new Error("Use a supported surface decal image size.");
    if (width === this.width && height === this.height) return false;
    this.output.destroy();
    this.output = this.createOutput(width, height);
    this.outputView = this.output.createView({ dimension: "2d-array" });
    this.width = width;
    this.height = height;
    return true;
  }
  get view(): GPUTextureView {
    return this.outputView;
  }
  get bytes(): number {
    return (
      this.width * this.height * 24 +
      80 +
      Math.max(1, this.manifest.instances.length) * INSTANCE_FLOATS * 4 +
      Math.max(1, this.manifest.materials.length) * MATERIAL_FLOATS * 4 +
      Object.values(this.pools).reduce((n, p) => n + p.bytes, 0)
    );
  }
  render(
    encoder: GPUCommandEncoder,
    frame: SurfaceDecalFrame,
  ): SurfaceDecalStats {
    this.resize(frame.width, frame.height);
    if (
      frame.inverseViewProjection.length !== 16 ||
      !Array.from(frame.inverseViewProjection).every(Number.isFinite)
    )
      throw new Error(
        "Use a finite inverse camera transform for surface decals.",
      );
    const draws = buildSurfaceDecalDraws(
      this.manifest.instances,
      frame.viewProjection,
      frame.width,
      frame.height,
    );
    const data = packSurfaceDecalDraws(draws, frame.width, frame.height);
    if (data.length)
      this.device.queue.writeBuffer(this.instanceBuffer, 0, data);
    const camera = new Float32Array(20);
    camera.set(Array.from(frame.inverseViewProjection));
    camera.set([frame.width, frame.height, 0, 0], 16);
    this.device.queue.writeBuffer(this.frameBuffer, 0, camera);
    const group = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.frameBuffer } },
        { binding: 1, resource: frame.depthView },
        { binding: 2, resource: { buffer: this.instanceBuffer } },
        { binding: 3, resource: { buffer: this.materialBuffer } },
        {
          binding: 4,
          resource: this.pools.color.texture.createView({
            dimension: "2d-array",
          }),
        },
        {
          binding: 5,
          resource: this.pools.normal.texture.createView({
            dimension: "2d-array",
          }),
        },
        {
          binding: 6,
          resource: this.pools.data.texture.createView({
            dimension: "2d-array",
          }),
        },
        { binding: 7, resource: this.sampler },
      ],
    });
    const pass = encoder.beginRenderPass({
      label: "Source surface decal pass",
      colorAttachments: Array.from({ length: 3 }, (_, layer) => ({
        view: this.output.createView({
          dimension: "2d",
          baseArrayLayer: layer,
          arrayLayerCount: 1,
        }),
        loadOp: "clear" as const,
        storeOp: "store" as const,
        clearValue: [0, 0, 0, 1],
      })),
    });
    if (draws.length) {
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, group);
      pass.draw(6, draws.length);
    }
    pass.end();
    return {
      total: this.manifest.instances.length,
      visible: draws.length,
      draws: draws.length ? 1 : 0,
      coverage: draws.reduce(
        (n, d) =>
          n + (d.scissor[2] * d.scissor[3]) / (frame.width * frame.height),
        0,
      ),
      bytes: this.bytes,
    };
  }
  destroy(): void {
    this.output.destroy();
    this.frameBuffer.destroy();
    this.instanceBuffer.destroy();
    this.materialBuffer.destroy();
    for (const pool of Object.values(this.pools)) pool.texture.destroy();
  }
}
