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
 const bytes=new Uint8Array(memory.buffer);assert(uniforms+uniformBytes+16<bytes.length);let peak=0,navigations=0;
 for(const[w,h,d]of[[1440,900,1],[390,844,1],[640,1136,2]]){
  bytes.fill(0,state,state+size);
  x.coaxial_state_init?.(state);
  const guards=[[state-8,state],[state+size,state+size+8],[vertices-8,vertices],[vertices+max*stride*4,vertices+max*stride*4+8],[uniforms-8,uniforms],[uniforms+uniformBytes,uniforms+uniformBytes+8]];
  for(const[a,b]of guards)bytes.fill(165,a,b);
  const n=x.coaxial_frame(state,vertices,uniforms,w,h,d,0,1);peak=Math.max(peak,n);
  assert(n>100&&n<max);assert(new Float32Array(memory.buffer,vertices,n*stride).every(Number.isFinite));
  // Discover links through the shipped pointer/cursor/action interface, not
  // copied layout coordinates. Navigation is observed without opening a URL.
  const wanted=new Set(['https://github.com/OpenAgentsInc/openagents/blob/main/coax/book/README.md','/coax/changelog']),found=new Set();
  for(let py=72*d;py<Math.min(h,450*d)&&found.size<wanted.size;py+=8*d){
   for(let px=24*d;px<w-24*d&&found.size<wanted.size;px+=8*d){
    x.coaxial_pointer(state,px,py,1,0,0);
    if(x.coaxial_cursor(state)!==1)continue;
    x.coaxial_pointer(state,px,py,0,0,0);x.coaxial_pointer(state,px,py,2,0,0);
    if(x.coaxial_action(state)===1){
     const url=new TextDecoder().decode(bytes.subarray(x.coaxial_outbox(state),x.coaxial_outbox(state)+x.coaxial_outbox_len(state)));
     if(wanted.has(url))found.add(url);
    }
    x.coaxial_action_done(state);
   }
  }
  assert.deepEqual([...found].sort(),[...wanted].sort(),'both independently clickable project resources');navigations+=found.size;
  for(const[a,b]of guards)assert(bytes.subarray(a,b).every(v=>v===165),'guard bytes');
 }
 console.log(JSON.stringify({viewports:3,resource_navigations:navigations,peak_vertices:peak,max_vertices:max,finite_geometry:true,guard_bytes:true}));
})().catch(e=>{console.error(e);process.exitCode=1});
