import { BATTLE_COMMON_WGSL } from "./battle-crowd-shader.ts";
import {
  BATTLE_COMBAT_BYTES,
  BATTLE_COMBAT_TYPES,
  BATTLE_PROJECTILE_CAPACITY,
  BATTLE_PROJECTILE_OFFSET,
} from "./battle-combat-shader.ts";
import { MMORPG_FOG_WGSL } from "./mmorpg-fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";

const READ_TYPES = BATTLE_COMBAT_TYPES.replaceAll("atomic<u32>", "u32");
export const BATTLE_ORDNANCE_COMPACTION_WGSL =
  BATTLE_COMMON_WGSL +
  BATTLE_COMBAT_TYPES +
  /* wgsl */ `
struct Draw {indexCount:u32,instanceCount:atomic<u32>,firstIndex:u32,baseVertex:i32,firstInstance:u32}
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var<storage,read_write>combat:Combat;
@group(0)@binding(2)var<storage,read_write>draws:array<Draw>;
@group(0)@binding(3)var<storage,read_write>stats:array<atomic<u32>>;
@compute @workgroup_size(1)fn clearOrdnance(){atomicStore(&draws[0].instanceCount,0u);atomicStore(&draws[3].instanceCount,0u);atomicStore(&draws[1].instanceCount,8u);atomicStore(&draws[2].instanceCount,select(0u,1u,frame.participant.y==1u&&(u32(frame.participantAim.w)&1u)!=0u&&atomicLoad(&combat.data[1])>0u));atomicStore(&stats[50],0u);}
@compute @workgroup_size(128)fn compactOrdnance(@builtin(global_invocation_id)inv:vec3u){
 let id=inv.x;if id>=16384u||atomicLoad(&combat.occupied[id])==0u{return;}
 let a=combat.projectiles[id];if a.pose.w<0.0{return;}let p=a.pose.xyz;let point=vec4(p,1.0);var outside=false;let margin=select(select(2.0,10.0,a.info.z==3u),5.0,a.info.z==4u);
 for(var i=0u;i<6u;i++){outside=outside||dot(frame.planes[i],point)<-margin;}
 if outside{return;}let offset=atomicAdd(&draws[0].instanceCount,1u);combat.visible[offset]=id;if a.info.z==3u||a.info.z==4u{atomicMax(&draws[3].instanceCount,offset+1u);}atomicAdd(&stats[50],1u);
}
`;
export const BATTLE_ORDNANCE_RENDER_WGSL =
  BATTLE_COMMON_WGSL +
  READ_TYPES +
  MMORPG_FOG_WGSL +
  /* wgsl */ `
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var<storage,read>combat:Combat;
struct In {@location(0)p:vec3f,@location(1)n:vec3f,@location(2)part:u32}
struct Out {@builtin(position)clip:vec4f,@location(0)world:vec3f,@location(1)normal:vec3f,@location(2)color:vec3f,@location(3)emissive:f32}
fn output(p:vec3f,n:vec3f,color:vec3f)->Out{var o:Out;o.clip=frame.viewProjection*vec4(p,1);o.world=p;o.normal=n;o.color=color;o.emissive=0.0;return o;}
fn hiddenProjectile()->Out{var o:Out;o.clip=vec4(2,2,2,1);o.world=frame.eyeTime.xyz;o.normal=vec3(0,1,0);o.color=vec3(0);o.emissive=0.0;return o;}
@vertex fn projectileVS(v:In,@builtin(instance_index)i:u32)->Out{
 let a=combat.projectiles[combat.visible[i]];let meteor=a.info.z==3u;if a.info.z==4u{return hiddenProjectile();}
 let forward=a.velocity.xyz*inverseSqrt(max(dot(a.velocity.xyz,a.velocity.xyz),.000001));let axis=select(vec3f(0,1,0),vec3f(1,0,0),abs(forward.y)>.99);let right=normalize(cross(forward,axis));let up=cross(right,forward);
 let size=select(select(.22,.72,a.info.z==1u),.62,meteor);let local=v.p*vec3(size,size,size*select(2.5,1.0,meteor));
 let p=a.pose.xyz+right*local.x+up*local.y+forward*local.z;
 var color=select(vec3(.25,.65,2.2),vec3(2.8,.35,.08),a.info.x==1u);if meteor{color=vec3(.22,.095,.035);}
 return output(p,normalize(right*v.n.x+up*v.n.y+forward*v.n.z),color);
}
struct MeteorFxOut {@builtin(position)clip:vec4f,@location(0)uv:vec2f,@location(1)color:vec4f,@location(2)@interpolate(flat)kind:f32,@location(3)world:vec3f}
fn meteorFx(center:vec3f,x:vec3f,y:vec3f,uv:vec2f,color:vec4f,kind:f32)->MeteorFxOut{
 var o:MeteorFxOut;o.world=center+x*uv.x+y*uv.y;o.clip=frame.viewProjection*vec4(o.world,1);o.uv=uv;o.color=color;o.kind=kind;return o;
}
fn meteorBillboard(center:vec3f,radius:f32,uv:vec2f,color:vec4f)->MeteorFxOut{
 let delta=frame.eyeTime.xyz-center;let forward=select(vec3f(0,0,1),delta*inverseSqrt(max(dot(delta,delta),1e-12)),dot(delta,delta)>1e-8);
 let axis=select(vec3f(0,1,0),vec3f(1,0,0),abs(forward.y)>.99);let right=normalize(cross(forward,axis));let up=normalize(cross(right,forward));
 return meteorFx(center,right*radius,up*radius,uv,color,0.0);
}
@vertex fn meteorFxVS(v:In,@builtin(instance_index)i:u32)->MeteorFxOut{
 let projectileId=combat.visible[i];let a=combat.projectiles[projectileId];let part=v.part-10u;let uv=v.p.xy;
 if (a.info.z!=3u&&a.info.z!=4u)||(a.info.z==3u&&part>8u)||(a.info.z==4u&&part<9u){var hidden:MeteorFxOut;hidden.clip=vec4(2,2,2,1);return hidden;}
 if a.info.z==3u{
  let backward=-a.velocity.xyz*inverseSqrt(max(dot(a.velocity.xyz,a.velocity.xyz),1e-12));
  if part<8u{let j=f32(part);return meteorBillboard(a.pose.xyz+backward*j*1.15,.25+(1.0-j/8.0)*.8,uv,vec4(1.0,.13+j*.015,.012,(1.0-j/8.0)*4.0));}
  let toEye=frame.eyeTime.xyz-a.pose.xyz;let foreground=toEye*inverseSqrt(max(dot(toEye,toEye),1e-12))*.66;
  return meteorBillboard(a.pose.xyz+foreground,.7,uv,vec4(1,.65,.12,12));
 }
 let t=clamp(a.pose.w/.4,0.0,1.0);let normal=normalize(a.velocity.xyz);
 if part==9u{
  let axis=select(vec3f(0,0,1),vec3f(1,0,0),abs(normal.z)>.99);let right=normalize(cross(normal,axis));let tangent=cross(normal,right);let radius=max(.1,a.impact.w*sqrt(t));
  return meteorFx(a.pose.xyz+normal*.08,right*radius,tangent*radius,uv,vec4(1,.35,.045,(1.0-t)*2.0),1.0);
 }
 if part==10u{return meteorBillboard(a.pose.xyz+normal*.65,max(.1,a.impact.w*.55*(1.0-t)),uv,vec4(1,.5,.1,(1.0-t)*(1.0-t)*8.0));}
 let seed=projectileId^((part-11u)*0x9e3779b9u);let angle=random(seed)*6.2831853;let speed=2.0+random(seed^40u)*8.0;let age=a.pose.w;
 let center=a.pose.xyz+vec3(sin(angle)*speed*age,(3.0+random(seed^80u)*5.0)*age-4.9*age*age,cos(angle)*speed*age);
 return meteorBillboard(center,.035+.06*(1.0-t),uv,vec4(1,.36,.03,(1.0-t)*4.0));
}
@fragment fn meteorFxFS(v:MeteorFxOut)->@location(0)vec4f{
 let r=length(v.uv);var shape=pow(max(0.0,1.0-r),2.0);if v.kind>.5{shape=(1.0-smoothstep(.07,.18,abs(r-.83)))*step(r,1.0);}
 let scale=clamp(frame.ambient.w,1.0,100.0);let transmission=1.0-mmoFog(frame.eyeTime.y/scale,(v.world-frame.eyeTime.xyz)/scale);
 return vec4(v.color.rgb*(shape*v.color.a*transmission)*${SCENE_LINEAR_SCALE},0.0);
}
@vertex fn siegeVS(v:In,@builtin(instance_index)i:u32)->Out{
 let a=combat.siege[i];var p=v.p;var n=v.n;
 if v.part==1u{let angle=-.45+max(0.0,1.0-a.state.y/.8)*1.8;let s=sin(angle);let c=cos(angle);let q=p-vec3(0,2.0,0);p=vec3(q.x,c*q.y-s*q.z,s*q.y+c*q.z)+vec3(0,2,0);n=vec3(n.x,c*n.y-s*n.z,s*n.y+c*n.z);}
 let c=cos(a.pose.w);let s=sin(a.pose.w);let world=a.pose.xyz+vec3(c*p.x+s*p.z,p.y,-s*p.x+c*p.z);
 let color=select(vec3(.32,.19,.075),vec3(.23,.09,.045),a.state.z>0.5);return output(world,normalize(vec3(c*n.x+s*n.z,n.y,-s*n.x+c*n.z)),color);
}
@vertex fn weaponVS(v:In)->Out{
 let ranged=frame.participant.w!=0u;
 if (ranged&&v.part!=4u)||(!ranged&&v.part==4u){var hidden:Out;hidden.clip=vec4(2,2,2,1);hidden.world=frame.eyeTime.xyz;hidden.normal=vec3(0,1,0);hidden.color=vec3(0);return hidden;}
 let delta=frame.participantAim.xyz-frame.eyeTime.xyz;let forward=normalize(select(vec3(sin(frame.participantPose.w),0,-cos(frame.participantPose.w)),delta,dot(delta,delta)>.00001));let axis=select(vec3f(0,1,0),vec3f(1,0,0),abs(forward.y)>.99);let right=normalize(cross(forward,axis));let up=cross(right,forward);
 let ticks=f32(combat.data[5]);let duration=select(60.0,150.0,frame.participant.w==2u);let swinging=ticks<duration&&combat.data[9]>0u;let phase=select(0.0,sin(clamp(ticks/duration,0.0,1.0)*3.14159265),swinging);
 let angle=phase*1.8;let c=cos(angle);let s=sin(angle);var q=vec3(c*v.p.x-s*v.p.y,s*v.p.x+c*v.p.y,v.p.z);let n=vec3(c*v.n.x-s*v.n.y,s*v.n.x+c*v.n.y,v.n.z);
 if ranged{q=v.p*(1.0+.15*sin(frame.eyeTime.w*4.0)+phase*.35);}
 let local=q*.62+vec3(.25-phase*.22,-.3,.65);let world=frame.eyeTime.xyz+right*local.x+up*local.y+forward*local.z;
 var color=select(vec3(.7,.78,.88),vec3(.22,.1,.04),v.part==3u);if ranged{color=select(vec3(.5,2.0,5.0),vec3(6,1,.08),frame.participant.w==2u);}return output(world,normalize(right*n.x+up*n.y+forward*n.z),color);
}
@fragment fn fs(v:Out)->@location(0)vec4f{
 let light=max(0.0,dot(normalize(v.normal),normalize(frame.sun.xyz)));var color=v.color*mix(frame.ambient.rgb+frame.sunColor.rgb*light,vec3(1),v.emissive);
 let scale=clamp(frame.ambient.w,1.0,100.0);color=mix(color,mmoFogColor,mmoFog(frame.eyeTime.y/scale,(v.world-frame.eyeTime.xyz)/scale));return vec4(color*${SCENE_LINEAR_SCALE},1.0);
}
`;

/** Small owned siege meshes; source character geometry remains untouched. */
export function createBattleSiegeGeometry() {
  const values: number[] = [],
    indices: number[] = [];
  const tri = (a: number[], b: number[], c: number[], part = 0) => {
    const ab = b.map((n, i) => n - a[i]),
      ac = c.map((n, i) => n - a[i]);
    const n = [
      ab[1] * ac[2] - ab[2] * ac[1],
      ab[2] * ac[0] - ab[0] * ac[2],
      ab[0] * ac[1] - ab[1] * ac[0],
    ];
    const length = Math.hypot(...n) || 1;
    for (const p of [a, b, c]) {
      indices.push(values.length / 8);
      values.push(...p, ...n.map((x) => x / length), part, 0);
    }
  };
  const top = [0, 1, 0],
    bottom = [0, -1, 0],
    ring = [
      [1, 0, 0],
      [0, 0, 1],
      [-1, 0, 0],
      [0, 0, -1],
    ];
  for (let i = 0; i < 4; i++) {
    tri(top, ring[i], ring[(i + 1) % 4]);
    tri(bottom, ring[(i + 1) % 4], ring[i]);
  }
  const projectile = { firstIndex: 0, indexCount: indices.length };
  const box = (center: number[], half: number[], part = 0) => {
    const p = Array.from({ length: 8 }, (_, i) =>
      center.map((n, j) => n + half[j] * ((i >> j) & 1 ? 1 : -1)),
    );
    for (const f of [
      [0, 4, 6, 2],
      [1, 3, 7, 5],
      [0, 1, 5, 4],
      [2, 6, 7, 3],
      [0, 2, 3, 1],
      [4, 5, 7, 6],
    ]) {
      tri(p[f[0]], p[f[1]], p[f[2]], part);
      tri(p[f[0]], p[f[2]], p[f[3]], part);
    }
  };
  const start = indices.length;
  box([0, 0.45, 0], [1.45, 0.22, 2]);
  box([-1.15, 1.45, 0], [0.18, 1, 0.22]);
  box([1.15, 1.45, 0], [0.18, 1, 0.22]);
  box([0, 2, 0], [1.5, 0.17, 0.17]);
  box([0, 2, 1.15], [0.16, 0.16, 2.1], 1);
  box([0, 2, -1.1], [0.65, 0.7, 0.55], 1);
  box([0, 2, 3.15], [0.45, 0.15, 0.45], 1);
  for (const x of [-1.55, 1.55])
    for (const z of [-1.35, 1.35]) box([x, 0.48, z], [0.18, 0.48, 0.48]);
  const siegeEnd = indices.length;
  box([0, 0.3, 0], [0.035, 0.45, 0.018], 2);
  box([0, -0.19, 0], [0.03, 0.12, 0.03], 3);
  box([0, -0.04, 0], [0.15, 0.025, 0.035], 3);
  const orbPoint = (p: number[]) =>
    p.map((n, i) => n * 0.2 + (i === 1 ? 0.12 : 0));
  for (let i = 0; i < 4; i++) {
    tri(orbPoint(top), orbPoint(ring[i]), orbPoint(ring[(i + 1) % 4]), 4);
    tri(orbPoint(bottom), orbPoint(ring[(i + 1) % 4]), orbPoint(ring[i]), 4);
  }
  const weapon = {
    firstIndex: siegeEnd,
    indexCount: indices.length - siegeEnd,
  };
  const fxStart = indices.length;
  for (let part = 10; part < 29; part++) {
    tri([-1, -1, 0], [1, -1, 0], [-1, 1, 0], part);
    tri([-1, 1, 0], [1, -1, 0], [1, 1, 0], part);
  }
  const meteorFx = {
    firstIndex: fxStart,
    indexCount: indices.length - fxStart,
  };
  const buffer = new ArrayBuffer(values.length * 4),
    floats = new Float32Array(buffer),
    words = new Uint32Array(buffer);
  floats.set(values);
  for (let i = 0; i < values.length / 8; i++)
    words[i * 8 + 6] = values[i * 8 + 6];
  return {
    vertices: buffer,
    indices: new Uint32Array(indices),
    projectile,
    siege: { firstIndex: start, indexCount: siegeEnd - start },
    weapon,
    meteorFx,
  };
}
export interface BattleParticipantSnapshot {
  position: [number, number, number];
  yaw: number;
  health: number;
  maxHealth: 200;
  dead: boolean;
  acceptedSequence: number;
  protectedTicks: number;
  attackTicks: number;
  attackKind: "melee" | "ranged" | "meteor_shower";
  resetEpoch: number;
  receivedSequence: number;
  lastImpact: { position: [number, number, number]; tick: number } | null;
}
export function decodeBattleOrdnanceProbe(raw: ArrayBuffer, offset = 0) {
  const f = new Float32Array(raw, offset, 8),
    data = new Uint32Array(raw, offset + 32, 16);
  const participant: BattleParticipantSnapshot = {
    position: [f[0], f[1], f[2]],
    yaw: f[3],
    health: data[1],
    maxHealth: 200,
    dead: data[1] === 0,
    acceptedSequence: data[3],
    protectedTicks: data[8],
    attackTicks: data[5],
    attackKind:
      data[4] === 2 ? "meteor_shower" : data[4] === 1 ? "ranged" : "melee",
    resetEpoch: data[11],
    receivedSequence: data[12],
    lastImpact:
      data[10] === 0
        ? null
        : {
            position: Array.from(new Float32Array(raw, offset + 84, 3)) as [
              number,
              number,
              number,
            ],
            tick: data[10] - 1,
          },
  };
  const projectiles = [];
  const occupiedOffset =
    offset + BATTLE_PROJECTILE_OFFSET + BATTLE_PROJECTILE_CAPACITY * 64;
  for (
    let id = 0;
    id < BATTLE_PROJECTILE_CAPACITY && projectiles.length < 32;
    id++
  )
    if (new Uint32Array(raw, occupiedOffset + id * 4, 1)[0]) {
      const p = new Float32Array(
          raw,
          offset + BATTLE_PROJECTILE_OFFSET + id * 64,
          16,
        ),
        w = new Uint32Array(p.buffer, p.byteOffset, 16);
      projectiles.push({
        id,
        position: Array.from(p.slice(0, 3)),
        velocity: Array.from(p.slice(4, 7)),
        age: p[3],
        flight: p[7],
        radius: p[11],
        team: w[12],
        damage: w[13],
        kind: w[14],
        target: Array.from(p.slice(8, 11)),
        ownerEpoch: w[15],
      });
    }
  const sieges = Array.from({ length: 8 }, (_, id) => {
    const p = new Float32Array(raw, offset + 96 + id * 32, 8);
    return {
      id,
      position: [p[0], p[1], p[2]],
      yaw: p[3],
      cooldown: p[4],
      armTime: p[5],
      team: p[6],
      health: p[7],
    };
  });
  return { participant, projectiles, sieges };
}

export class BattleOrdnance {
  readonly arena: GPUBuffer;
  readonly bytes: number;
  private buffers: GPUBuffer[] = [];
  private indirect: GPUBuffer;
  private compute: GPUComputePipeline[];
  private group: GPUBindGroup;
  private renderGroup: GPUBindGroup;
  private pipelines: GPURenderPipeline[];
  private vertices: GPUBuffer;
  private indices: GPUBuffer;
  private constructor(
    private device: GPUDevice,
    arena: GPUBuffer,
    indirect: GPUBuffer,
    compute: GPUComputePipeline[],
    group: GPUBindGroup,
    renderGroup: GPUBindGroup,
    pipelines: GPURenderPipeline[],
    vertices: GPUBuffer,
    indices: GPUBuffer,
    buffers: GPUBuffer[],
  ) {
    this.arena = arena;
    this.indirect = indirect;
    this.compute = compute;
    this.group = group;
    this.renderGroup = renderGroup;
    this.pipelines = pipelines;
    this.vertices = vertices;
    this.indices = indices;
    this.buffers = buffers;
    this.bytes = buffers.reduce((n, b) => n + b.size, 0);
  }
  static async create(
    device: GPUDevice,
    uniform: GPUBuffer,
    counters: GPUBuffer,
    config: {
      format: GPUTextureFormat;
      depthFormat: GPUTextureFormat;
      sampleCount: number;
    },
  ) {
    const buffers: GPUBuffer[] = [];
    const make = (label: string, size: number, usage: GPUBufferUsageFlags) => {
      const b = device.createBuffer({ label, size, usage });
      buffers.push(b);
      return b;
    };
    try {
      const arena = make(
        "Battle crowd: bounded projectiles and participant",
        BATTLE_COMBAT_BYTES,
        GPUBufferUsage.STORAGE |
          GPUBufferUsage.COPY_DST |
          GPUBufferUsage.COPY_SRC,
      );
      const geometry = createBattleSiegeGeometry();
      const vertices = make(
          "Battle crowd: procedural siege vertices",
          geometry.vertices.byteLength,
          GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        ),
        indices = make(
          "Battle crowd: procedural siege indices",
          geometry.indices.byteLength,
          GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
        );
      device.queue.writeBuffer(vertices, 0, geometry.vertices);
      device.queue.writeBuffer(indices, 0, geometry.indices);
      const args = new Uint32Array([
        geometry.projectile.indexCount,
        0,
        0,
        0,
        0,
        geometry.siege.indexCount,
        8,
        geometry.siege.firstIndex,
        0,
        0,
        geometry.weapon.indexCount,
        0,
        geometry.weapon.firstIndex,
        0,
        0,
        geometry.meteorFx.indexCount,
        0,
        geometry.meteorFx.firstIndex,
        0,
        0,
      ]);
      const indirect = make(
        "Battle crowd: ordnance indirect arguments",
        80,
        GPUBufferUsage.STORAGE |
          GPUBufferUsage.INDIRECT |
          GPUBufferUsage.COPY_DST,
      );
      device.queue.writeBuffer(indirect, 0, args);
      const entry = (
          binding: number,
          type: GPUBufferBindingType,
          visibility: number,
        ) => ({ binding, visibility, buffer: { type } }),
        bind = (binding: number, buffer: GPUBuffer) => ({
          binding,
          resource: { buffer },
        });
      const layout = device.createBindGroupLayout({
        entries: [
          entry(0, "uniform", GPUShaderStage.COMPUTE),
          entry(1, "storage", GPUShaderStage.COMPUTE),
          entry(2, "storage", GPUShaderStage.COMPUTE),
          entry(3, "storage", GPUShaderStage.COMPUTE),
        ],
      });
      const group = device.createBindGroup({
        layout,
        entries: [
          bind(0, uniform),
          bind(1, arena),
          bind(2, indirect),
          bind(3, counters),
        ],
      });
      const module = device.createShaderModule({
        code: BATTLE_ORDNANCE_COMPACTION_WGSL,
      });
      const compute = await Promise.all(
        ["clearOrdnance", "compactOrdnance"].map((entryPoint) =>
          device.createComputePipelineAsync({
            layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
            compute: { module, entryPoint },
          }),
        ),
      );
      const rl = device.createBindGroupLayout({
        entries: [
          entry(0, "uniform", GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT),
          entry(1, "read-only-storage", GPUShaderStage.VERTEX),
        ],
      });
      const renderGroup = device.createBindGroup({
        layout: rl,
        entries: [bind(0, uniform), bind(1, arena)],
      });
      const shader = device.createShaderModule({
        code: BATTLE_ORDNANCE_RENDER_WGSL,
      });
      const pipelines = await Promise.all(
        ["projectileVS", "siegeVS", "weaponVS", "meteorFxVS"].map(
          (entryPoint) =>
            device.createRenderPipelineAsync({
              layout: device.createPipelineLayout({ bindGroupLayouts: [rl] }),
              vertex: {
                module: shader,
                entryPoint,
                buffers: [
                  {
                    arrayStride: 32,
                    attributes: [
                      { shaderLocation: 0, format: "float32x3", offset: 0 },
                      { shaderLocation: 1, format: "float32x3", offset: 12 },
                      { shaderLocation: 2, format: "uint32", offset: 24 },
                    ],
                  },
                ],
              },
              fragment: {
                module: shader,
                entryPoint: entryPoint === "meteorFxVS" ? "meteorFxFS" : "fs",
                targets: [
                  {
                    format: config.format,
                    ...(entryPoint === "meteorFxVS"
                      ? {
                          blend: {
                            color: {
                              srcFactor: "one" as const,
                              dstFactor: "one" as const,
                            },
                            alpha: {
                              srcFactor: "zero" as const,
                              dstFactor: "one" as const,
                            },
                          },
                        }
                      : {}),
                  },
                ],
              },
              primitive: { topology: "triangle-list", cullMode: "none" },
              depthStencil: {
                format: config.depthFormat,
                depthCompare: "less-equal",
                depthWriteEnabled: entryPoint !== "meteorFxVS",
              },
              multisample: {
                count: config.sampleCount,
                alphaToCoverageEnabled: false,
              },
            }),
        ),
      );
      return new BattleOrdnance(
        device,
        arena,
        indirect,
        compute,
        group,
        renderGroup,
        pipelines,
        vertices,
        indices,
        buffers,
      );
    } catch (error) {
      buffers.forEach((b) => b.destroy());
      throw error;
    }
  }
  reset(
    center: readonly number[],
    radius: number,
    height: (x: number, z: number) => number,
    sequence = 0,
  ) {
    const raw = new ArrayBuffer(BATTLE_COMBAT_BYTES),
      f = new Float32Array(raw),
      u = new Uint32Array(raw);
    u[9] = 200;
    u[11] = sequence;
    u[20] = sequence;
    u[14] = 1;
    for (let id = 0; id < 8; id++) {
      const team = id % 2,
        x = center[0] + (team ? 1 : -1) * radius * 0.88,
        z = center[2] + (Math.floor(id / 2) - 1.5) * radius * 0.43;
      f.set(
        [
          x,
          height(x, z),
          z,
          team ? -Math.PI / 2 : Math.PI / 2,
          (id % 4) * 1.1,
          0.9,
          team,
          750,
        ],
        24 + id * 8,
      );
    }
    this.device.queue.writeBuffer(this.arena, 0, raw);
  }
  resetParticipant(sequence: number, epoch: number) {
    const u = new Uint32Array(15);
    u[0] = 200;
    u[2] = sequence;
    u[5] = 1;
    u[7] = 120;
    u[10] = epoch;
    u[11] = sequence;
    this.device.queue.writeBuffer(this.arena, 36, u);
  }
  prepare(encoder: GPUCommandEncoder) {
    const p = encoder.beginComputePass({
      label: "Battle crowd: projectile visibility",
    });
    p.setBindGroup(0, this.group);
    p.setPipeline(this.compute[0]);
    p.dispatchWorkgroups(1);
    p.setPipeline(this.compute[1]);
    p.dispatchWorkgroups(BATTLE_PROJECTILE_CAPACITY / 128);
    p.end();
  }
  render(pass: GPURenderPassEncoder, firstPerson = false) {
    pass.setBindGroup(0, this.renderGroup);
    pass.setVertexBuffer(0, this.vertices);
    pass.setIndexBuffer(this.indices, "uint32");
    const count = firstPerson ? 3 : 2;
    for (let i = 0; i < count; i++) {
      pass.setPipeline(this.pipelines[i]);
      pass.drawIndexedIndirect(this.indirect, i * 20);
    }
    pass.setPipeline(this.pipelines[3]);
    pass.drawIndexedIndirect(this.indirect, 60);
    return count + 1;
  }
  dispose() {
    this.buffers.forEach((b) => b.destroy());
  }
}
