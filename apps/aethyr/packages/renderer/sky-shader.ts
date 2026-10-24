import {
  SCENE_LINEAR_SCALE,
  GENERATED_SKY_RADIANCE_SCALE,
} from "./radiometry.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_REFERENCE_HEIGHT,
  FOG_START_DISTANCE,
} from "./fog.ts";
import { WGSL_SKY_DOME } from "./sky.ts";
import { WGSL_FILMIC } from "./filmic.ts";
import { WGSL_DISPLAY_LUT } from "./display-lut.ts";
import { WGSL_VIGNETTE } from "./vignette.ts";
export const SKY_COMMON_WGSL = `fn hash(p:vec2<f32>)->f32{return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
fn noise(p:vec2<f32>)->f32{let i=floor(p);let f=fract(p);let u=f*f*(3.0-2.0*f);return mix(mix(hash(i),hash(i+vec2(1.0,0.0)),u.x),mix(hash(i+vec2(0.0,1.0)),hash(i+vec2(1.0,1.0)),u.x),u.y);}
fn fbm(p:vec2<f32>)->f32{var q=p;var v=0.0;var w=0.5;for(var i=0;i<5;i++){v+=noise(q)*w;q=q*2.03+vec2(19.1,7.3);w*=0.5;}return v;}
fn generatedSky(direction:vec3<f32>)->vec3<f32>{
 let d=normalize(direction);let h=max(d.y,0.0);let night=frame.extra.x;
 let horizon=mix(vec3(0.64,0.78,0.91),vec3(0.026,0.055,0.11),night);
 let zenith=mix(vec3(0.12,0.33,0.65),vec3(0.003,0.012,0.036),night);
 var color=mix(horizon,zenith,pow(h,0.48));let mu=clamp(dot(d,frame.sun.xyz),0.0,1.0);
 color+=frame.sunColor.xyz*(pow(mu,32.0)*0.09+pow(mu,2048.0)*4.0)*(1.0-night);
 let cloudUV=d.xz/max(d.y+0.14,0.08)*2.2+vec2(frame.eye.w*frame.cloud.y*0.006,0.0);
 let base=fbm(cloudUV);let cloud=smoothstep(1.0-frame.cloud.x,1.14-frame.cloud.x,base);
 let cloudLight=mix(vec3(1.05,1.03,1.0),vec3(0.075,0.11,0.17),night)*(0.65+base*0.55);
 color=mix(color,cloudLight,cloud*smoothstep(-0.02,0.12,d.y));
 let moonDir=safeNormalize(frame.sun.xyz,vec3(0.0,1.0,0.0));let moon=smoothstep(0.99972,0.99982,dot(d,moonDir));
 color+=vec3(0.55,0.72,1.0)*moon*night*frame.extra.y*2.0;
 let stars=pow(hash(floor(d.xz/max(d.y+0.4,0.05)*900.0)),160.0)*smoothstep(0.08,0.5,d.y)*(1.0-cloud);
 color+=vec3(stars)*night*0.7;
 return max(color,vec3(0.0))*frame.cloud.z*${SCENE_LINEAR_SCALE}*${GENERATED_SKY_RADIANCE_SCALE.toFixed(1)};
}
fn sky(direction:vec3<f32>)->vec3<f32>{
 if frame.skyEnabled.x<0.5{return generatedSky(direction);}
 let d=safeNormalize((frame.skyRotation*vec4(direction,0.0)).xyz,vec3(0.0,1.0,0.0));let uv=vec2(fract(atan2(d.x,d.z)/6.283185307*frame.skyUV.x+frame.skyUV.y),clamp(acos(clamp(d.y,-1.0,1.0))/3.141592654*frame.skyUV.z+frame.skyUV.w,0.0,1.0));
 var color=textureSampleLevel(environmentImage,environmentSampler,uv,0.0).rgb*frame.skyTint.rgb*frame.skyTint.w;
 return clamp(color*frame.cloud.z,vec3(0.0),vec3(65000.0));
}
`;
export const SKY_BACKGROUND_WGSL =
  WGSL_SKY_DOME +
  WGSL_FILMIC +
  WGSL_DISPLAY_LUT +
  WGSL_VIGNETTE +
  `
@group(0)@binding(15)var<uniform>sourceDome:vec4<f32>;
struct SkyDisplay{settings:vec4<f32>,filmic:FilmicParams,lut:vec4<f32>,vignette:vec4<f32>};
@group(1)@binding(2)var<uniform>skyDisplay:SkyDisplay;
@group(1)@binding(4)var skyDisplayImage:texture_3d<f32>;
@group(1)@binding(5)var skyDisplaySampler:sampler;
fn ownedSkyDisplay(color:vec3<f32>,uv:vec2<f32>)->vec3<f32>{
 var c=color*skyDisplay.settings.x;
 if skyDisplay.settings.w>.5{
  c*=vignetteWeight(uv,skyDisplay.vignette.y,skyDisplay.vignette.x);let l=dot(c,vec3(.2126,.7152,.0722));c=mix(vec3(l),c,skyDisplay.settings.y);
  if skyDisplay.lut.w>.5{return sampleDisplayLut(skyDisplayImage,skyDisplaySampler,skyDisplay.lut,c);}return filmicColor(c,skyDisplay.filmic);
 }
 c=clamp((c*(2.51*c+vec3(.03)))/(c*(2.43*c+vec3(.59))+vec3(.14)),vec3(0.0),vec3(1.0));let l=dot(c,vec3(.2126,.7152,.0722));return mix(vec3(l),c,skyDisplay.settings.y);
}
struct Out{@builtin(position)pos:vec4<f32>,@location(0)uv:vec2<f32>};
@vertex fn vs(@builtin(vertex_index)i:u32)->Out{let p=vec2(f32((i<<1u)&2u),f32(i&2u));var o:Out;o.pos=vec4(p*2.0-1.0,1.0,1.0);o.uv=p;return o;}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{
 let q=frame.inverseView*vec4(v.uv*2.0-1.0,1.0,1.0);let direction=normalize(q.xyz/q.w-frame.eye.xyz);var color=sky(direction);
 // Generated skies keep their owned profile; source domes use finite-path source transport.
 if frame.skyEnabled.x>0.5&&sourceDome.w>0.0{
  let distance=skyDomeDistance(sourceDome,frame.eye.xyz,direction);
  if distance>0.0{
   let transport=heightFogComposition(frame.eye.y,direction.y,distance,frame.fog.w,${FOG_HEIGHT_RATE},${FOG_REFERENCE_HEIGHT}.0,${FOG_START_DISTANCE}.0,frame.volumeMedium,frame.volumeRange.xy);
   color=color*transport.x+frame.fog.rgb*transport.y;
  }
 }
 // MMO geometry keeps source output; the retained HDR sky keeps its own display transform.
 if frame.skyLayers.w>.5{color=ownedSkyDisplay(color,v.uv)*${SCENE_LINEAR_SCALE};}
 return vec4(max(color,vec3(0.0)),1.0);
}
`;
