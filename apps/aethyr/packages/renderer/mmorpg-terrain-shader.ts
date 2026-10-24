// Terrain noise adapted from Jonas Wagner; see third_party/simplex-noise/LICENSE.
export const MMORPG_HEIGHT_WGSL = /* wgsl */ `
const mmPerm=array<u32,256>(134u,32u,123u,160u,45u,84u,8u,75u,127u,33u,73u,189u,243u,14u,227u,213u,116u,101u,71u,118u,232u,177u,38u,36u,132u,247u,23u,223u,201u,145u,29u,203u,113u,131u,200u,129u,192u,37u,179u,195u,198u,158u,253u,34u,69u,242u,251u,30u,252u,254u,110u,219u,186u,222u,89u,121u,185u,235u,2u,107u,218u,249u,119u,25u,124u,156u,136u,238u,161u,55u,7u,40u,208u,43u,22u,144u,142u,39u,163u,80u,141u,140u,99u,130u,44u,92u,31u,109u,209u,68u,204u,180u,115u,170u,199u,20u,173u,157u,27u,175u,149u,230u,54u,137u,91u,26u,28u,85u,202u,63u,56u,205u,5u,67u,83u,96u,49u,117u,62u,48u,229u,90u,42u,120u,150u,61u,153u,72u,212u,16u,59u,224u,220u,17u,122u,228u,81u,174u,233u,6u,78u,112u,35u,57u,53u,143u,93u,47u,217u,105u,221u,100u,250u,0u,50u,162u,10u,102u,194u,245u,111u,41u,128u,88u,13u,4u,184u,58u,241u,155u,178u,172u,3u,248u,87u,214u,52u,171u,76u,196u,24u,240u,103u,97u,159u,138u,82u,225u,108u,133u,135u,239u,176u,65u,11u,181u,79u,234u,246u,244u,86u,125u,216u,9u,169u,187u,106u,206u,188u,74u,210u,1u,197u,226u,164u,60u,139u,193u,168u,98u,66u,154u,182u,77u,215u,191u,147u,21u,167u,211u,114u,12u,166u,94u,207u,51u,95u,165u,19u,183u,152u,237u,148u,255u,231u,151u,126u,15u,70u,236u,46u,104u,18u,190u,146u,64u);
const mmGrad=array<vec3<f32>,12>(vec3(1.,1.,0.),vec3(-1.,1.,0.),vec3(1.,-1.,0.),vec3(-1.,-1.,0.),vec3(1.,0.,1.),vec3(-1.,0.,1.),vec3(1.,0.,-1.),vec3(-1.,0.,-1.),vec3(0.,1.,1.),vec3(0.,-1.,1.),vec3(0.,1.,-1.),vec3(0.,-1.,-1.));
fn mmP(i:i32)->i32{return i32(mmPerm[u32(i)&255u]);}
fn mmSimplex(point:vec3<f32>)->f32 {
 let cell=vec3<i32>(floor(point+vec3((point.x+point.y+point.z)/3.0)));
 let a=point-vec3<f32>(cell)+vec3(f32(cell.x+cell.y+cell.z)/6.0);
 var first=vec3<i32>(0,1,0);var second=vec3<i32>(1,1,0);
 if a.x>=a.y {
  if a.y>=a.z{first=vec3(1,0,0);second=vec3(1,1,0);}
  else if a.x>=a.z{first=vec3(1,0,0);second=vec3(1,0,1);}
  else{first=vec3(0,0,1);second=vec3(1,0,1);}
 }else{
  if a.y<a.z{first=vec3(0,0,1);second=vec3(0,1,1);}
  else if a.x<a.z{first=vec3(0,1,0);second=vec3(0,1,1);}
 }
 let corners=array<vec3<i32>,4>(vec3(0),first,second,vec3(1));var sum=0.0;
 for(var n=0;n<4;n++){
  let c=corners[n];let d=a-vec3<f32>(c)+vec3(f32(n)/6.0);
  var q=.6-dot(d,d);if q<0.{continue;}
  let g=mmGrad[u32(mmP(cell.x+c.x+mmP(cell.y+c.y+mmP(cell.z+c.z))))%12u];
  q*=q;sum+=q*q*dot(g,d);
 }
 return 32.0*sum;
}
fn mmorpgHeight(p:vec2<f32>)->f32 {
 var amplitude=1.0;var frequency=1.0;var total=0.0;var norm=0.0;
 for(var o=0;o<10;o++){
  total+=(mmSimplex(vec3(p.x/300.0*frequency,0.,p.y/300.0*frequency))*.5+.5)*amplitude;
  norm+=amplitude;amplitude*=.7071067811865476;frequency*=1.6;
 }
 return pow(max(0.,total/norm),7.5)*(800.0/6.0);
}
fn mmorpgGroundHeight(p:vec2<f32>)->f32 {
 let cell=2.0;let g=floor(p/cell)*cell;let f=(p-g)/cell;
 let a=mmorpgHeight(g);let b=mmorpgHeight(g+vec2(cell,0.));let c=mmorpgHeight(g+vec2(0.,cell));let d=mmorpgHeight(g+vec2(cell));
 if f.x+f.y<=1.{return a+(b-a)*f.x+(c-a)*f.y;}
 return d+(c-d)*(1.-f.x)+(b-d)*(1.-f.y);
}
`;
export const MMORPG_TERRAIN_WGSL = /* wgsl */ `
${MMORPG_HEIGHT_WGSL}
struct Terrain {center:vec2<f32>,cell:f32,resolution:u32,firstVertex:u32,vertexCount:u32,padding:vec2<u32>};
@group(0) @binding(0) var<uniform> terrain:Terrain;
@group(0) @binding(1) var<storage,read_write> vertices:array<f32>;
@compute @workgroup_size(64) fn update(@builtin(global_invocation_id) id:vec3<u32>){
 if id.x>=terrain.vertexCount{return;}
 let grid=vec2(f32(id.x%terrain.resolution),f32(id.x/terrain.resolution));
 let offset=grid-vec2(f32(terrain.resolution-1u)*.5);
 let radius=abs(offset);
 let displacement=select(radius*terrain.cell,vec2(128.)+(radius-vec2(64.))*24.,radius>vec2(64.));
 let p=terrain.center+sign(offset)*displacement;
 let height=mmorpgHeight(p);let e=.5;
 let normal=normalize(vec3(mmorpgHeight(p-vec2(e,0.))-mmorpgHeight(p+vec2(e,0.)),2.0*e,mmorpgHeight(p-vec2(0.,e))-mmorpgHeight(p+vec2(0.,e))));
 let at=(terrain.firstVertex+id.x)*12u;
 vertices[at]=p.x;vertices[at+1u]=height;vertices[at+2u]=p.y;
 vertices[at+3u]=normal.x;vertices[at+4u]=normal.y;vertices[at+5u]=normal.z;
 vertices[at+6u]=p.x*.1;vertices[at+7u]=p.y*.1;
}
`;
