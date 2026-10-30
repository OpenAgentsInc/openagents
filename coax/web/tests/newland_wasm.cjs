// Actual renderer ABI: facts clock, pause/expiry, layouts and guard bytes.
const fs=require('node:fs'),assert=require('node:assert/strict');
(async()=>{
 const source=fs.readFileSync('apps/openagents-web/newland-data.coax','utf8');
 const captured=Number(source.match(/LAND_CAPTURED :: (\d+);/)[1]);
 const pylons=JSON.parse(source.match(/^LAND_PYLONS :: (".*");$/m)[1]).trim().split('\n').filter(Boolean).map(row=>row.split('|'));
 const memory=new WebAssembly.Memory({initial:1024,maximum:1024});
 const wasi={clock_time_get:(_clock,_precision,pointer)=>{new DataView(memory.buffer).setBigUint64(pointer,0n,true);return 0},fd_close:()=>52,fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error('unexpected renderer diagnostic')}};
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi});
 const x=instance.exports;x._initialize();const align=n=>(n+15)&~15;
 assert.equal(x.coaxial_land_captured(),BigInt(captured),'module uses the checked capture');
 const p=align(Number(x.__heap_base.value)+1024),size=x.coaxial_state_size(),max=x.coaxial_max_vertices(),floats=x.coaxial_vertex_floats(),uf=x.coaxial_uniform_floats();
 const vertices=align(p+size+16),uniforms=align(vertices+max*floats*4+16),bytes=new Uint8Array(memory.buffer);
 assert(uniforms+uf*4+16<bytes.length,'arena fits module memory');let frames=0,peak=0;
 function frame(w,h,d,dt){
  const guards=[[p-8,p],[p+size,p+size+8],[vertices-8,vertices],[vertices+max*floats*4,vertices+max*floats*4+8],[uniforms-8,uniforms],[uniforms+uf*4,uniforms+uf*4+8]];
  for(const[a,b]of guards)bytes.fill(165,a,b);
  const n=x.coaxial_frame(p,vertices,uniforms,w,h,d,dt,1);frames++;peak=Math.max(peak,n);
  assert(n>0&&n<=max,'vertex budget');for(const[a,b]of guards)assert(bytes.subarray(a,b).every(v=>v===165),'guard');
  const geometry=new Float32Array(memory.buffer,vertices,n*floats);assert(geometry.every(Number.isFinite),'finite geometry');
 }
 for(const[w,h,d]of[[1440,900,1],[390,844,1],[780,1688,2]]){
  bytes.fill(0,p,p+size);frame(w,h,d,0);
  for(let i=0;i<x.coaxial_land_pylons(p);i++)if(pylons[i][0]!=='demo')assert.equal(x.coaxial_land_pylon_status(p,i),3,'unknown clock means stale real beacon');
  const facts=new TextEncoder().encode('now='+captured*1000+'\n');bytes.set(facts,x.coaxial_inbox(p));x.coaxial_message(p,600,facts.length);
  frame(w,h,d,0);
  for(let i=0;i<x.coaxial_land_pylons(p);i++){
   const row=pylons[i],status={online:0,draining:1,offline:2}[row[7]];
   assert.equal(x.coaxial_land_pylon_status(p,i),row[0]==='demo'||Number(row[24])>0?status:3,'facts clock enables an unexpired beacon');
  }
  frame(w,h,d,3600);
  for(let i=0;i<x.coaxial_land_pylons(p);i++)if(pylons[i][0]!=='demo')assert.equal(x.coaxial_land_pylon_status(p,i),3,'a paused page expires the captured beacon');
 }
 console.log(JSON.stringify({frames,viewports:3,peak_vertices:peak,max_vertices:max,clock_and_pause_expiry:true,finite_geometry:true,guard_bytes:true}));
})().catch(e=>{console.error(e);process.exitCode=1});
