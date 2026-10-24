/** Screen-space contact occlusion affects ambient light, never direct lights. */
export class ContactOcclusion {
  private texture?: GPUTexture;
  private group?: GPUBindGroup;
  private width = 0;
  private height = 0;
  readonly uniforms: GPUBuffer;
  private constructor(
    private device: GPUDevice,
    private pipeline: GPURenderPipeline,
  ) {
    this.uniforms = device.createBuffer({
      size: 160,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
  }
  static async create(device: GPUDevice) {
    const module = device.createShaderModule({ code: OCCLUSION });
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: { module, entryPoint: "fs", targets: [{ format: "r8unorm" }] },
    });
    return new ContactOcclusion(device, pipeline);
  }
  resize(width: number, height: number, depth: GPUTextureView) {
    this.texture?.destroy();
    this.width = width;
    this.height = height;
    this.texture = this.device.createTexture({
      label: "Contact occlusion",
      size: [Math.ceil(width / 2), Math.ceil(height / 2)],
      format: "r8unorm",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.group = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: depth },
        { binding: 1, resource: { buffer: this.uniforms } },
      ],
    });
  }
  get view() {
    return this.texture!.createView();
  }
  render(
    encoder: GPUCommandEncoder,
    viewProjection: Float32Array,
    inverseViewProjection: Float32Array,
    eye: readonly number[],
  ) {
    const data = new Float32Array(40);
    data.set(viewProjection);
    data.set(inverseViewProjection, 16);
    data.set(eye, 32);
    data.set([this.width, this.height, 1.25, 0.045], 36);
    this.device.queue.writeBuffer(this.uniforms, 0, data);
    const pass = encoder.beginRenderPass({
      label: "Contact occlusion",
      colorAttachments: [
        {
          view: this.view,
          loadOp: "clear",
          storeOp: "store",
          clearValue: [1, 0, 0, 1],
        },
      ],
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group!);
    pass.draw(3);
    pass.end();
  }
  dispose() {
    this.texture?.destroy();
    this.uniforms.destroy();
  }
}

const OCCLUSION = `
struct Settings {projection:mat4x4<f32>,inverseProjection:mat4x4<f32>,eye:vec4<f32>,sizeRadius:vec4<f32>};
@group(0)@binding(0)var depth:texture_depth_multisampled_2d;
@group(0)@binding(1)var<uniform>s:Settings;
@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let p=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(p*2.0-1.0,0.0,1.0);}
fn closest(p:vec2<i32>)->f32{let q=clamp(p,vec2(0),vec2<i32>(textureDimensions(depth))-vec2(1));var z=1.0;for(var i=0;i<4;i++){z=min(z,textureLoad(depth,q,i));}return z;}
fn reconstruct(pixel:vec2<f32>,z:f32)->vec3<f32>{let uv=(pixel+0.5)/s.sizeRadius.xy;let q=s.inverseProjection*vec4(uv*vec2(2.0,-2.0)+vec2(-1.0,1.0),z,1.0);return q.xyz/q.w;}
fn position(p:vec2<i32>)->vec3<f32>{return reconstruct(vec2<f32>(p),closest(p));}
@fragment fn fs(@builtin(position)fragment:vec4<f32>)->@location(0)f32{
 let pixel=vec2<i32>(fragment.xy*2.0);let z=closest(pixel);if z>=0.999999{return 1.0;}
 let p=reconstruct(vec2<f32>(pixel),z);let distance=length(s.eye.xyz-p);if distance>100.0{return 1.0;}
 let xr=position(pixel+vec2(1,0))-p;let xl=p-position(pixel-vec2(1,0));let yr=position(pixel+vec2(0,1))-p;let yl=p-position(pixel-vec2(0,1));
 let dx=select(xr,xl,dot(xl,xl)<dot(xr,xr));let dy=select(yr,yl,dot(yl,yl)<dot(yr,yr));
 let crossN=cross(dy,dx);if dot(crossN,crossN)<0.000000000001{return 1.0;}var n=normalize(crossN);
 n*=select(-1.0,1.0,dot(n,s.eye.xyz-p)>=0.0);
 let axis=select(vec3(0.0,1.0,0.0),vec3(1.0,0.0,0.0),abs(n.y)>0.95);let tangent=normalize(cross(axis,n));let bitangent=cross(n,tangent);
 let rotation=fract(sin(dot(vec2<f32>(pixel),vec2(12.9898,78.233)))*43758.5453)*6.2831853;
 var obscured=0.0;
 for(var i=0;i<16;i++){
  let k=(f32(i)+0.5)/16.0;let angle=f32(i)*2.3999632+rotation;let r=sqrt(k);let direction=tangent*(cos(angle)*r)+bitangent*(sin(angle)*r)+n*sqrt(1.0-k);
  let radius=s.sizeRadius.z*(0.2+0.8*k);let candidate=p+n*s.sizeRadius.w+direction*radius;
  let clip=s.projection*vec4(candidate,1.0);if clip.w<=0.0{continue;}let uv=clip.xy/clip.w*vec2(0.5,-0.5)+vec2(0.5);
  if any(uv<vec2(0.0))||any(uv>vec2(1.0)){continue;}
  let samplePixel=vec2<i32>(uv*s.sizeRadius.xy);let sampleZ=closest(samplePixel);if sampleZ>=0.999999{continue;}
  let surface=reconstruct(vec2<f32>(samplePixel),sampleZ);let ray=normalize(surface-s.eye.xyz);
  let behind=dot(candidate-s.eye.xyz,ray)-length(surface-s.eye.xyz);
  if behind>s.sizeRadius.w{obscured+=1.0-smoothstep(radius,radius*1.5,length(surface-p));}
 }
 return clamp(1.0-obscured/16.0*(1.0-smoothstep(50.0,100.0,distance)),0.0,1.0);
}
`;
