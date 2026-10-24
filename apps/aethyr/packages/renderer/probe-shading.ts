/** World-space SH irradiance with distance-moment interpolation, independent of UI state. */
export const WGSL_PROBE_FIELDS = `,probeOrigin:vec4<f32>,probeSpacing:vec4<f32>,probeCounts:vec4<f32>,probeAtlas:vec4<f32>`;
const PROBE_SOURCE = `
override AF_PROBE_GATHER:bool=true;
override AF_SELECTIVE_GI:bool=true;
@group(0)@binding(11)var probeMoments:texture_2d<f32>;
@group(0)@binding(12)var<storage,read> probeRecords:array<vec4<f32>>;
@group(0)@binding(14)var<storage,read> bounceRecords:array<vec4<f32>>;
struct LocalSkyLighting{irradiance:vec3<f32>,confidence:f32,specularVisibility:f32,skyBounce:vec3<f32>,sunBounce:vec3<f32>,localBounce:vec3<f32>};
fn emptyLocalSky(confidence:f32,visibility:f32)->LocalSkyLighting{return LocalSkyLighting(vec3(0.0),confidence,visibility,vec3(0.0),vec3(0.0),vec3(0.0));}
// Cantelli upper bound at the finite miss distance. Exact only for one hit distance plus misses.
fn probeSkyVisibility(moments:vec2<f32>)->f32{
 let miss=frame.probeSpacing.w;if moments.x>=miss-0.001{return 1.0;}
 let variance=max(moments.y-moments.x*moments.x,0.0);let gap=miss-moments.x;
 return clamp(variance/max(variance+gap*gap,0.000001),0.0,1.0);
}
fn probeOct(direction:vec3<f32>)->vec2<f32>{
 let d=direction/max(dot(abs(direction),vec3(1.0)),0.000001);var uv=d.xy;
 if d.z<0.0{uv=(vec2(1.0)-abs(uv.yx))*select(vec2(-1.0),vec2(1.0),uv>=vec2(0.0));}
 return uv*0.5+vec2(0.5);
}
fn distanceMomentsAt(tile:vec2<f32>,oct:vec2<f32>)->vec2<f32>{
 let pixel=tile*frame.probeAtlas.z+vec2(1.0)+oct*frame.probeAtlas.w-vec2(0.5);
 let lo=vec2<i32>(floor(pixel));let fraction=fract(pixel);
 if AF_PROBE_GATHER{
  // Center the gather on the original 2x2 footprint; keep the original weights.
  let gatherUV=(vec2<f32>(lo)+vec2(1.0))/vec2<f32>(textureDimensions(probeMoments));
  let r=textureGather(0,probeMoments,environmentSampler,gatherUV);
  let g=textureGather(1,probeMoments,environmentSampler,gatherUV);
  let a=vec2(r.w,g.w);let b=vec2(r.z,g.z);let c=vec2(r.x,g.x);let d=vec2(r.y,g.y);
  return mix(mix(a,b,fraction.x),mix(c,d,fraction.x),fraction.y);
 }
 let a=textureLoad(probeMoments,lo,0).rg;let b=textureLoad(probeMoments,lo+vec2(1,0),0).rg;
 let c=textureLoad(probeMoments,lo+vec2(0,1),0).rg;let d=textureLoad(probeMoments,lo+vec2(1,1),0).rg;
 return mix(mix(a,b,fraction.x),mix(c,d,fraction.x),fraction.y);
}
fn distanceMoments(index:u32,direction:vec3<f32>)->vec2<f32>{
 let columns=u32(frame.probeCounts.w);let tile=vec2<f32>(f32(index%columns),f32(index/columns));
 return distanceMomentsAt(tile,probeOct(direction));
}
// Returns raw world-space E/pi. The caller applies the source sky gain once.
fn localSkyLightingComponents(world:vec3<f32>,normal:vec3<f32>,geometric:vec3<f32>,view:vec3<f32>,needI:bool,needS:bool)->LocalSkyLighting{
 if frame.probeOrigin.w<0.5{return emptyLocalSky(0.0,1.0);}
 if !needI&&!needS&&all(frame.bounceSettings.xyz<=vec3(0.5)){return emptyLocalSky(0.0,1.0);}
 let biased=world+geometric*0.1+view*0.05;
 let grid=(biased-frame.probeOrigin.xyz)/frame.probeSpacing.xyz;
 let maximum=frame.probeCounts.xyz-vec3(1.0);
 if any(grid<vec3(0.0))||any(grid>maximum){return emptyLocalSky(0.0,1.0);}
 let cell=vec3<u32>(min(floor(grid),maximum-vec3(1.0)));let blend=clamp(grid-vec3<f32>(cell),vec3(0.0),vec3(1.0));
 let x=normal.x;let y=normal.y;let z=normal.z;
 let basis=array<f32,9>(0.2820947918,0.4886025119*y,0.4886025119*z,0.4886025119*x,1.0925484306*x*y,1.0925484306*y*z,0.3153915653*(3.0*z*z-1.0),1.0925484306*x*z,0.5462742153*(x*x-y*y));
 var irradiance=vec3(0.0);var weightSum=0.0;var validSum=0.0;var specularVisibility=0.0;
 var skyBounce=vec3(0.0);var sunBounce=vec3(0.0);var localBounce=vec3(0.0);
 let counts=vec3<u32>(frame.probeCounts.xyz);let columns=u32(frame.probeCounts.w);
 let probeCount=counts.x*counts.y*counts.z;let constantProbeBase=probeCount*10u;
 let count=u32(frame.bounceSettings.w);let constantBounceBase=count*27u;
 let skyGain=frame.skyLayers.x;
 let constantSkyEnabled=frame.skyLayers.z>0.5;let skyBounceEnabled=frame.bounceSettings.x>0.5;
 let constantBounceEnabled=frame.skyLayers.w>0.5;let sunBounceEnabled=frame.bounceSettings.y>0.5;
 let localBounceEnabled=frame.bounceSettings.z>0.5;let reflectedOct=probeOct(reflect(-view,normal));
 let edge=min(min(grid.x,maximum.x-grid.x),min(min(grid.y,maximum.y-grid.y),min(grid.z,maximum.z-grid.z)));
 let confidence=smoothstep(0.0,0.5,edge);
 for(var corner=0u;corner<8u;corner++){
  let offset=vec3<u32>(corner&1u,(corner>>1u)&1u,(corner>>2u)&1u);let p=cell+offset;
  let index=p.x+counts.x*(p.y+counts.y*p.z);let probeBase=index*10u;let shBase=index*9u;
  let record=probeRecords[probeBase];if record.w<0.5{continue;}
  let tile=vec2<f32>(f32(index%columns),f32(index/columns));
  let tri=select(vec3(1.0)-blend,blend,offset>vec3(0u));let trilinear=tri.x*tri.y*tri.z;
  validSum+=trilinear;
  let delta=biased-record.xyz;let distance=length(delta);let direction=safeNormalize(delta,geometric);
  let moments=distanceMomentsAt(tile,probeOct(direction));let variance=max(moments.y-moments.x*moments.x,0.0001);
  var visibility=1.0;if distance>moments.x{let gap=distance-moments.x;visibility=pow(variance/(variance+gap*gap),3.0);}
  let wrapped=(dot(-direction,geometric)+1.0)*0.5;var weight=trilinear*(wrapped*wrapped+0.2)*visibility;
  if weight<0.2{weight=weight*weight*weight/(0.2*0.2);}
  var value=vec3(0.0);
  if needI{if constantSkyEnabled{
   let constantBase=constantProbeBase+shBase;
   for(var coefficient=0u;coefficient<9u;coefficient++){
    value+=probeRecords[probeBase+1u+coefficient].rgb*basis[coefficient]*skyGain;
    value+=probeRecords[constantBase+coefficient].rgb*basis[coefficient];
   }
  }else{
   for(var coefficient=0u;coefficient<9u;coefficient++){
    value+=probeRecords[probeBase+1u+coefficient].rgb*basis[coefficient]*skyGain;
   }
  }}
  var skyValue=vec3(0.0);var sunValue=vec3(0.0);var localValue=vec3(0.0);
  if skyBounceEnabled{
   if constantBounceEnabled{
    let constantBase=constantBounceBase+shBase;
    for(var coefficient=0u;coefficient<9u;coefficient++){
     skyValue+=bounceRecords[shBase+coefficient].rgb*basis[coefficient]*skyGain;
     skyValue+=bounceRecords[constantBase+coefficient].rgb*basis[coefficient];
    }
   }else{
    for(var coefficient=0u;coefficient<9u;coefficient++){
     skyValue+=bounceRecords[shBase+coefficient].rgb*basis[coefficient]*skyGain;
    }
   }
  }
  if sunBounceEnabled{
   let sunBase=(count+index)*9u;
   for(var coefficient=0u;coefficient<9u;coefficient++){
    sunValue+=bounceRecords[sunBase+coefficient].rgb*basis[coefficient];
   }
  }
  if localBounceEnabled{
   let localBase=(count*2u+index)*9u;
   for(var coefficient=0u;coefficient<9u;coefficient++){
    localValue+=bounceRecords[localBase+coefficient].rgb*basis[coefficient];
   }
  }
  skyBounce+=max(skyValue,vec3(0.0))*weight;sunBounce+=max(sunValue,vec3(0.0))*weight;localBounce+=max(localValue,vec3(0.0))*weight;
  irradiance+=max(value,vec3(0.0))*weight;weightSum+=weight;
  if needS{specularVisibility+=probeSkyVisibility(distanceMomentsAt(tile,reflectedOct))*weight;}
 }
 if validSum<0.000001{return emptyLocalSky(0.0,1.0);}
 if weightSum<0.0000001{return emptyLocalSky(confidence,0.0);}
 return LocalSkyLighting(irradiance/weightSum,confidence,clamp(specularVisibility/weightSum,0.0,1.0),skyBounce/weightSum,sunBounce/weightSum,localBounce/weightSum);
}
fn localSkyLighting(world:vec3<f32>,normal:vec3<f32>,geometric:vec3<f32>,view:vec3<f32>)->LocalSkyLighting{
 return localSkyLightingComponents(world,normal,geometric,view,true,true);
}
fn localSkyIrradiance(world:vec3<f32>,normal:vec3<f32>,geometric:vec3<f32>,view:vec3<f32>)->vec4<f32>{
 let sample=localSkyLighting(world,normal,geometric,view);return vec4(sample.irradiance,sample.confidence);
}
`;

export interface ProbeShaderOptions {
  unrollSH9?: boolean;
  compactIdentifiers?: boolean;
}

/** Generate one SH evaluator; the default retains coefficient loops for comparison. */
export function probeShadingWGSL(options: ProbeShaderOptions = {}): string {
  let source = PROBE_SOURCE;
  if (options.unrollSH9) {
    let loops = 0;
    source = source.replace(
      /for\(var coefficient=0u;coefficient<9u;coefficient\+\+\)\{([^{}]+)\}/g,
      (_, body: string) => {
        loops++;
        return Array.from({ length: 9 }, (_, index) =>
          body.replace(/\bcoefficient\b/g, `${index}u`),
        ).join("");
      },
    );
    if (loops !== 6)
      throw new Error(
        "The probe SH generator requires six complete coefficient loops.",
      );
  }
  if (options.compactIdentifiers !== false) {
    // These names belong only to this generated module. Keep public helpers,
    // result fields, frame fields, and pipeline constants unchanged.
    const names = [
      "probeMoments",
      "probeRecords",
      "bounceRecords",
      "probeSkyVisibility",
      "probeOct",
      "distanceMomentsAt",
      "constantProbeBase",
      "constantBounceBase",
      "constantSkyEnabled",
      "skyBounceEnabled",
      "constantBounceEnabled",
      "sunBounceEnabled",
      "localBounceEnabled",
      "constantBase",
      "reflectedOct",
      "probeBase",
      "shBase",
      "sunBase",
      "localBase",
      "coefficient",
      "skyValue",
      "sunValue",
      "localValue",
      "weightSum",
      "validSum",
      "value",
      "basis",
      "skyGain",
      "probeCount",
      "trilinear",
      "distance",
      "visibility",
      "maximum",
      "geometric",
      "normal",
      "counts",
      "columns",
      "index",
      "offset",
      "corner",
      "variance",
      "moments",
      "direction",
      "biased",
      "weight",
      "wrapped",
      "delta",
      "tile",
      "fraction",
      "pixel",
      "gatherUV",
      "edge",
      "blend",
      "cell",
    ];
    const aliases = new Map(
      names.map((name, index) => [name, `P${index.toString(36)}`]),
    );
    source = source.replace(
      /\b[A-Za-z_][A-Za-z0-9_]*\b/g,
      (name) => aliases.get(name) ?? name,
    );
  }
  return source;
}

export const WGSL_PROBES = probeShadingWGSL();
