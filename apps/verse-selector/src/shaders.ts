export const shader = `
struct Camera { matrix: mat4x4f, eye: vec4f, viewport: vec4f };
@group(0) @binding(0) var<uniform> camera: Camera;
struct Input { @location(0) position: vec3f, @location(1) normal: vec3f, @location(2) color: vec3f, @location(3) kind: f32 };
struct Surface { @builtin(position) clip: vec4f, @location(0) normal: vec3f, @location(1) color: vec3f, @location(2) position: vec3f, @location(3) kind: f32 };
@vertex fn vertex(v: Input) -> Surface {
  var out: Surface; out.clip=camera.matrix*vec4f(v.position,1.0);out.normal=v.normal;out.color=v.color;out.position=v.position;out.kind=v.kind;return out;
}
fn noise(p:vec3f)->f32 { return sin(p.x*13.7+sin(p.y*8.0))*sin(p.z*10.4+p.y*11.0)*0.5+0.5; }
@fragment fn planet(v: Surface) -> @location(0) vec4f {
  let n=normalize(v.normal);let light=normalize(vec3f(-.8,.5,1.0));let l=dot(n,light);
  let view=normalize(camera.eye.xyz-v.position);let rim=pow(1.0-max(dot(n,view),0.0),3.5);
  let grain=noise(n*.23)*.6+noise(n*.65)*.25+noise(n*1.8)*.15;let cloud=smoothstep(.38,.65,grain);
  let bands=.5+.5*sin(n.y*16.0+noise(n*.8)*2.0);
  let surface=mix(v.color*.43,v.color*1.3,cloud)*(.82+bands*.18);
  let lit=surface*(.045+max(l,0.0)*1.15);
  let atmosphere=v.color*rim*(.24+max(l,0.0)*.9);
  let color=lit+atmosphere+vec3f(.4,.7,1.0)*pow(rim,3.0)*.1;
  if(v.kind>19.0){return vec4f(v.color*1.45,1.0);}
  return vec4f(pow(color,vec3f(.82)),1.0);
}
@fragment fn orbit(v: Surface) -> @location(0) vec4f { return vec4f(v.color*.7,select(.15,.3,v.kind<11.0)); }
struct Sky { @builtin(position) clip:vec4f,@location(0) uv:vec2f };
@vertex fn skyVertex(@builtin(vertex_index) i:u32)->Sky {
  let positions=array<vec2f,3>(vec2f(-1.0,-1.0),vec2f(3.0,-1.0),vec2f(-1.0,3.0));
  var out:Sky;out.clip=vec4f(positions[i],.9999,1.0);out.uv=positions[i];return out;
}
fn hash(p:vec2f)->f32{return fract(sin(dot(p,vec2f(127.1,311.7)))*43758.5453);}
@fragment fn sky(v:Sky)->@location(0) vec4f{
  let uv=v.uv*vec2f(camera.viewport.x/camera.viewport.y,1.0);
  var color=vec3f(.006,.011,.018);
  let haze=exp(-pow((uv.y+uv.x*.27-.18)*2.4,2.0))*exp(-length(uv)*.48);
  color+=vec3f(.015,.025,.042)*haze;
  let grid=uv*240.0;let cell=floor(grid);let rnd=hash(cell);
  let center=vec2f(hash(cell+2.1),hash(cell+4.7));
  let star=(1.0-smoothstep(.015,.11,length(fract(grid)-center)))*step(.974,rnd);
  color+=vec3f(.6,.73,.9)*star*(.5+rnd);
  let fine=uv*470.0;let s=hash(floor(fine));
  color+=vec3f(.28,.34,.42)*(1.0-smoothstep(.01,.07,length(fract(fine)-.5)))*step(.99,s);
  color*=1.0-smoothstep(.3,2.5,length(uv))*.72;
  return vec4f(color,1.0);
}`;
