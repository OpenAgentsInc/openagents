import { FOG_SEGMENT_WGSL } from "./fog.ts";
import { MMORPG_FOG_WGSL } from "./mmorpg-fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import { SHARED_WIND_WGSL } from "./shared-wind.ts";
import { MMORPG_HEIGHT_WGSL } from "./mmorpg-terrain-shader.ts";

/*
 * Grass shape, palette, wind, and lighting adapted from Quick_Grass.
 * https://github.com/simondevyoutube/Quick_Grass
 *
 * MIT License
 * Copyright (c) 2023 simondevyoutube
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

export const GRASS_LAYOUT = Object.freeze({
  nearPatches: 32,
  farPatches: 320,
  nearBlades: 3072,
  farBlades: 512,
  nearSegments: 6,
  farSegments: 3,
  bladeFloats: 16,
});
const NEAR_CAPACITY = GRASS_LAYOUT.nearPatches * GRASS_LAYOUT.nearBlades;
const FRAME_WGSL = /* wgsl */ `
${SHARED_WIND_WGSL}
struct Frame {
 view:mat4x4<f32>,rotation:mat4x4<f32>,eye:vec4<f32>,sun:vec4<f32>,
 sunColor:vec4<f32>,ambient:vec4<f32>,fog:vec4<f32>,fogColor:vec4<f32>,
 volumeMedium:vec4<f32>,volumeRange:vec4<f32>,sh:array<vec4<f32>,9>,
 player:vec4<f32>,grass:vec4<f32>,draw:vec4<f32>,wind:WindField,
 clearingInfo:vec4<f32>,clearings:array<vec4<f32>,16>
};
struct Blade { root:vec4<f32>,clump:vec4<f32>,motion:vec4<f32>,terrain:vec4<f32> };
@group(0) @binding(0) var<uniform> frame:Frame;
@group(0) @binding(1) var<storage,read> patches:array<vec4<f32>>;
fn hash(v:vec2<f32>)->f32 {
 var p=fract(vec3(v.x,v.y,v.x)*.1031);
 p+=vec3(dot(p,p.yzx+vec3(33.33)));return fract((p.x+p.y)*p.z);
}
fn safeXZ(v:vec2<f32>)->vec2<f32> {
 if dot(v,v)<.000001{return vec2(0.0,1.0);}return normalize(v);
}
`;

/** Cache world-space clumps only when a patch changes; animate once per blade. */
export const GRASS_PREPARE_WGSL = /* wgsl */ `
${FRAME_WGSL}
${MMORPG_HEIGHT_WGSL}
@group(0) @binding(2) var<storage,read_write> blades:array<Blade>;
fn noise(p:vec2<f32>)->f32 {
 let i=floor(p);let f=fract(p);let s=f*f*(3.0-2.0*f);
 return mix(mix(hash(i),hash(i+vec2(1.0,0.0)),s.x),mix(hash(i+vec2(0.0,1.0)),hash(i+vec2(1.0)),s.x),s.y);
}
// Every coarse cell contributes one far blade and six near blades. The prefix is identical.
fn rootPosition(origin:vec2<f32>,id:u32)->vec2<f32> {
 let coarseId=id%${GRASS_LAYOUT.farBlades}u;let child=id/${GRASS_LAYOUT.farBlades}u;
 let coarse=vec2(f32(coarseId%32u),f32(coarseId/32u));
 let coarseSeed=origin+coarse*.671;
 let slot=(child+u32(hash(coarseSeed+vec2(171.3))*6.0))%6u;
 let sub=vec2(f32(slot%2u),f32(slot/2u));
 let jitter=vec2(.1)+vec2(hash(coarseSeed+vec2(f32(slot)*.173+71.3)),hash(coarseSeed+vec2(f32(slot)*.397+181.1)))*.8;
 return origin+((coarse+(sub+jitter)/vec2(2.0,3.0))/vec2(32.0,16.0)-vec2(.5))*10.0;
}
fn clearingScale(root:vec2<f32>)->f32 {
 var scale=1.0;
 for(var i=0u;i<min(16u,u32(frame.clearingInfo.x));i++) {
  let circle=frame.clearings[i];
  scale=min(scale,smoothstep(max(0.0,circle.z-.75),circle.z,length(root-circle.xy)));
 }
 return scale;
}
fn prepare(id:u32,density:u32,patchOffset:u32,cacheOffset:u32) {
 let cell=patches[patchOffset+id/density];let bladeId=id%density;let index=cacheOffset+id;
 var value=blades[index];
 if cell.z>.5 {
  let rawRoot=rootPosition(cell.xy,bladeId);let grid=floor(rawRoot/1.6);
  var closest=1e20;var center=rawRoot;var seed=0.0;
  // Nine Voronoi candidates run once per new blade, never per drawn vertex.
  for(var z=-1;z<=1;z++){for(var x=-1;x<=1;x++){
   let neighbor=grid+vec2(f32(x),f32(z));
   let point=(neighbor+vec2(hash(neighbor+vec2(17.3)),hash(neighbor+vec2(71.9))))*1.6;
   let delta=point-rawRoot;let distance2=dot(delta,delta);
   if distance2<closest{closest=distance2;center=point;seed=hash(neighbor+vec2(377.1));}
  }}
  let root=mix(rawRoot,center,.12);let random=hash(rawRoot+vec2(541.1));
  let clumpAngle=seed*6.28318530718;
  let clumpFacing=safeXZ(mix(vec2(sin(clumpAngle),cos(clumpAngle)),safeXZ(center-root),.3));
  let facingJitter=(hash(rawRoot+vec2(219.7))-.5)*1.5;
  let facing=vec2(clumpFacing.x*cos(facingJitter)+clumpFacing.y*sin(facingJitter),clumpFacing.y*cos(facingJitter)-clumpFacing.x*sin(facingJitter));
  let height=clamp(mix(.85,1.35,hash(vec2(seed+71.3,seed+181.1)))*mix(.82,1.14,random),.65,1.5);
  let retirement=mix(frame.draw.x*.35,frame.draw.x*.88,hash(rawRoot+vec2(941.7)));
  value.root=vec4(root,height,retirement);value.clump=vec4(facing,0.0,seed);
  var ground=frame.grass.x;
  if frame.volumeRange.w>.5 {ground=mmorpgHeight(root);}
  value.terrain=vec4(ground,clearingScale(root),0.0,0.0);
 }
 let root=value.root.xy;let time=frame.eye.w;
 let playerDistance=length(root-frame.player.xz);
 let influence=(1.0-smoothstep(.65,frame.draw.z,playerDistance))*frame.player.w;
 let windResponse=mix(.88,1.12,hash(root+vec2(93.1)));
 var windLean=0.0;
 var windAxis=vec2(1.0,0.0);
 var flutter=sin(time*1.4+dot(root,vec2(3.1,2.7)))*.025;
 if frame.wind.state.x>.5 {
  let windSample=sampleWorldWind(vec3(root.x,frame.grass.x,root.y),time,frame.wind);
  let strength=length(windSample.xz);
  flutter*=min(1.0,strength*.25)*frame.grass.w;
  windAxis=vec2(safeXZ(windSample.xz).y,-safeXZ(windSample.xz).x);
  windLean=min(1.1,strength*.1*frame.grass.w*windResponse)*(1.0-influence*.6);
 }else{
  let gust=noise(root*.25+vec2(time*.7,time*.32));
  let windAngle=noise(root*.05+vec2(time*.05))*6.28318530718;
  windLean=min(1.1,pow(mix(.25,1.0,gust),2.0)*1.25*frame.grass.w*windResponse)*(1.0-influence*.6);
  windAxis=vec2(cos(windAngle),sin(windAngle));
 }
 let individualLean=mix(-.08,.08,hash(root+vec2(212.7)));
 value.clump.z=mix(.12,.35,hash(vec2(value.clump.w+41.7,value.clump.w+17.3)))+individualLean+flutter;
 value.motion=vec4(windAxis,windLean,influence*.6);
 blades[index]=value;
}
@compute @workgroup_size(64) fn prepareNear(@builtin(global_invocation_id)id:vec3<u32>){prepare(id.x,${GRASS_LAYOUT.nearBlades}u,0u,0u);}
@compute @workgroup_size(64) fn prepareFar(@builtin(global_invocation_id)id:vec3<u32>){prepare(id.x,${GRASS_LAYOUT.farBlades}u,${GRASS_LAYOUT.nearPatches}u,${NEAR_CAPACITY}u);}
`;

/** Curved ribbons with rounded edge normals and the world's linear lighting/fog. */
export const GRASS_WGSL = /* wgsl */ `
${FOG_SEGMENT_WGSL}
${MMORPG_FOG_WGSL}
${FRAME_WGSL}
@group(0) @binding(2) var<storage,read> blades:array<Blade>;
struct Out {
 @builtin(position) clip:vec4<f32>,@location(0) world:vec3<f32>,
 @location(1) normal:vec3<f32>,@location(2) color:vec3<f32>,
 @location(3) params:vec4<f32>,@location(4) commonNormal:vec3<f32>
};
fn rotate(v:vec3<f32>,axis:vec3<f32>,c:f32,s:f32)->vec3<f32>{return v*c+cross(axis,v)*s+axis*dot(axis,v)*(1.0-c);}
fn blade(vertex:u32,instance:u32,segments:u32,density:u32,cacheOffset:u32)->Out {
 let data=blades[cacheOffset+instance];let id=instance%density;
 let root=vec3(data.root.x,data.terrain.x,data.root.y);
 let t=f32(vertex/2u)/f32(segments);let side=f32(vertex%2u);
 let distance=length(root.xz-frame.eye.xz);let lowLOD=smoothstep(frame.draw.x*.5,frame.draw.x,distance);
 let outerFade=1.0-smoothstep(frame.draw.y*.75,frame.draw.y,distance);
 let retirement=data.root.w;let retireEnd=min(frame.draw.x,retirement+max(1.0,frame.draw.x*.12));
 let coverage=select(1.0-smoothstep(retirement,retireEnd,distance),1.0,id<${GRASS_LAYOUT.farBlades}u);
 let edgeFade=1.0-smoothstep(1021.0,1023.5,max(abs(root.x),abs(root.z)));
 let cameraFade=smoothstep(.45,1.6,distance);let visible=select(0.0,1.0,coverage>.001);
 let clearing=data.terrain.y;
 let height=frame.grass.z*data.root.z*outerFade*edgeFade*cameraFade*visible*clearing;
 let widthVariation=mix(.75,1.08,fract(data.root.w*1.619));
 let width=frame.grass.y*widthVariation*mix(pow(max(1.0-t,0.0),.85),1.0-t,lowLOD)*outerFade*edgeFade*cameraFade*visible*clearing;
 let face=vec3(data.clump.x,0.0,data.clump.y);let baseWidth=vec3(face.z,0.0,-face.x);
 let lean=data.clump.z*mix(1.0,.65,lowLOD);
 // Cubic Bezier controls: root, .32*up, .86*up+.14*lean*face, (1-.2*lean)*up+lean*face.
 // Keep the analytic derivative paired with this curve for deformation and rounded normals.
 let center=vec3(0.0,t*(.96+t*(.66+t*(-.62-.2*lean))),0.0)+face*(lean*t*t*(.42+.58*t));
 let slope=vec3(0.0,.96+1.32*t+3.0*(-.62-.2*lean)*t*t,0.0)+face*(lean*(.84*t+1.74*t*t));
 let windAxis=vec3(data.motion.x,0.0,data.motion.y);let windAngle=data.motion.z*t;
 let wc=cos(windAngle);let ws=sin(windAngle);
 let windCenter=rotate(center,windAxis,wc,ws);
 let windSlope=rotate(slope,windAxis,wc,ws)+data.motion.z*cross(windAxis,windCenter);
 let windWidth=rotate(baseWidth,windAxis,wc,ws);
 let away=safeXZ(root.xz-frame.player.xz);let playerAxis=vec3(away.y,0.0,-away.x);
 let playerAngle=data.motion.w*t;let pc=cos(playerAngle);let ps=sin(playerAngle);
 let bentCenter=rotate(windCenter,playerAxis,pc,ps);
 let bentSlope=rotate(windSlope,playerAxis,pc,ps)+data.motion.w*cross(playerAxis,bentCenter);
 let widthAxis=rotate(windWidth,playerAxis,pc,ps);
 var world=root+bentCenter*height+widthAxis*((side-.5)*width);
 let surfaceNormal=normalize(cross(widthAxis,bentSlope));
 let roundAxis=normalize(cross(bentSlope,surfaceNormal));
 // Left/right vertices carry ±54-degree normals; rasterization interpolates across width.
 let normal=normalize(surfaceNormal*.5877852523+roundAxis*(side*2.0-1.0)*.8090169944);
 let toCamera=safeXZ(frame.eye.xz-root.xz);let faceDot=abs(dot(safeXZ(surfaceNormal.xz),toCamera));
 let thicken=(1.0-pow(faceDot,4.0))*smoothstep(0.0,.2,faceDot);
 let cameraRight=normalize(vec3(frame.view[0].x,frame.view[1].x,frame.view[2].x));
 let outward=select(-1.0,1.0,dot(widthAxis,cameraRight)>=0.0);
 world+=cameraRight*((side-.5)*width*thicken*.5*outward);
 let seed=data.clump.w;let base=mix(vec3(.02,.075,.01),vec3(.025,.1,.01),seed);
 let tip=mix(vec3(.65,.8,.25),vec3(.8,.9,.4),hash(vec2(seed+71.3,seed+17.9)));
 let color=mix(base,tip,pow(t,mix(4.0,2.0,lowLOD)))*mix(.88,1.0,data.root.z/1.46);
 let clumpAngle=seed*6.28318530718;let commonNormal=normalize(vec3(-sin(clumpAngle)*.13,1.0,-cos(clumpAngle)*.13));
 var o:Out;o.clip=frame.view*vec4(world,1.0);o.world=world;o.normal=normal;
 o.color=color;o.params=vec4(t,side,lowLOD,coverage);o.commonNormal=commonNormal;return o;
}
@vertex fn vsNear(@builtin(vertex_index)vertex:u32,@builtin(instance_index)instance:u32)->Out{return blade(vertex,instance,${GRASS_LAYOUT.nearSegments}u,${GRASS_LAYOUT.nearBlades}u,0u);}
@vertex fn vsFar(@builtin(vertex_index)vertex:u32,@builtin(instance_index)instance:u32)->Out{return blade(vertex,instance,${GRASS_LAYOUT.farSegments}u,${GRASS_LAYOUT.farBlades}u,${NEAR_CAPACITY}u);}
fn skyDiffuse(normal:vec3<f32>)->vec3<f32> {
 let n=normalize((frame.rotation*vec4(normal,0.0)).xyz);
 let b=array<f32,9>(.2820947918,.4886025119*n.y,.4886025119*n.z,.4886025119*n.x,1.0925484306*n.x*n.y,1.0925484306*n.y*n.z,.3153915653*(3.0*n.z*n.z-1.0),1.0925484306*n.x*n.z,.5462742153*(n.x*n.x-n.y*n.y));
 var color=frame.ambient.xyz;for(var i=0;i<9;i++){color+=frame.sh[i].xyz*b[i];}return max(color,vec3(0.0));
}
@fragment fn fs(v:Out,@builtin(front_facing)front:bool)->@location(0)vec4<f32> {
 if v.params.w<.001{discard;}
 if frame.draw.w<2.0&&hash(floor(v.clip.xy))>v.params.w{discard;}
 let delta=v.world-frame.eye.xyz;let distance=length(delta);
 let bladeNormal=select(-v.normal,v.normal,front);
 let nearNormal=normalize(mix(vec3(0.0,1.0,0.0),bladeNormal,.25));
 let n=normalize(mix(nearNormal,v.commonNormal,smoothstep(12.0,50.0,distance)));
 let ao=mix(.25,1.0,v.params.x*v.params.x);
 let middle=mix(.85,1.0,1.0-smoothstep(0.0,.1,abs(v.params.y-.5)));let albedo=v.color*ao*middle;
 let wrap=clamp((dot(n,frame.sun.xyz)+.5)/1.5,0.0,1.0);
 let view=normalize(-delta);let scatter=clamp((dot(view,-frame.sun.xyz)+.5)/1.5,0.0,1.0)*.5*(1.0-v.params.z);
 let halfVector=normalize(view+frame.sun.xyz);let gloss=pow(max(dot(n,halfVector),0.0),16.0)*.025*(1.0-smoothstep(12.0,45.0,distance));
 let raw=albedo*(frame.sunColor.rgb*(wrap+scatter)/3.14159265359+skyDiffuse(n))+frame.sunColor.rgb*gloss;
 if frame.fog.z>.5{return vec4(mix(raw,mmoFogColor,mmoFog(frame.eye.y/frame.fog.z,delta/frame.fog.z))*${SCENE_LINEAR_SCALE},v.params.w);}
 let fog=heightFogComposition(frame.eye.y,delta.y/max(distance,.000001),distance,frame.fog.x,frame.fog.y,1.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 return vec4((raw*fog.x+frame.fogColor.rgb*fog.y)*${SCENE_LINEAR_SCALE},v.params.w);
}
`;
