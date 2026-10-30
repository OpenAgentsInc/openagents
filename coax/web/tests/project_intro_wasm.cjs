// The actual project renderer on desktop and phones, without relay effects.
const fs=require('node:fs'),assert=require('node:assert/strict');
(async()=>{
 const memory=new WebAssembly.Memory({initial:1024,maximum:1024});
 const env={memory,coaxial_http:()=>0,coaxial_socket_open:()=>-1,coaxial_socket_send:()=>{},coaxial_socket_close:()=>{}};
 const wasi={fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error('unexpected renderer diagnostic')}};
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env,wasi_snapshot_preview1:wasi});
 const x=instance.exports;x._initialize();const align=n=>(n+15)&~15;
 const state=align(Number(x.__heap_base.value)+1024),size=x.coaxial_state_size(),max=x.coaxial_max_vertices(),stride=x.coaxial_vertex_floats();
 const vertices=align(state+size+16),uniforms=align(vertices+max*stride*4+16),uniformBytes=x.coaxial_uniform_floats()*4;
 const bytes=new Uint8Array(memory.buffer);assert(uniforms+uniformBytes+16<bytes.length);let peak=0;
 for(const[w,h,d]of[[1440,900,1],[390,844,1],[640,1136,2]]){
  bytes.fill(0,state,state+size);
  const guards=[[state-8,state],[state+size,state+size+8],[vertices-8,vertices],[vertices+max*stride*4,vertices+max*stride*4+8],[uniforms-8,uniforms],[uniforms+uniformBytes,uniforms+uniformBytes+8]];
  for(const[a,b]of guards)bytes.fill(165,a,b);
  const n=x.coaxial_frame(state,vertices,uniforms,w,h,d,0,1);peak=Math.max(peak,n);
  assert(n>100&&n<max);assert(new Float32Array(memory.buffer,vertices,n*stride).every(Number.isFinite));
  for(const[a,b]of guards)assert(bytes.subarray(a,b).every(v=>v===165),'guard bytes');
 }
 console.log(JSON.stringify({viewports:3,peak_vertices:peak,max_vertices:max,finite_geometry:true,guard_bytes:true}));
})().catch(e=>{console.error(e);process.exitCode=1});
