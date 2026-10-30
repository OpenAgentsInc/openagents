// Actual emitted Wasm: every page/case, desktop/phone/2x, controls and guards.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
(async()=>{
 const rows=JSON.parse(fs.readFileSync(path.join(__dirname,'../../../apps/openagents-web/nip/catalog.json'))).pages;
 const memory=new WebAssembly.Memory({initial:128,maximum:1024});
 const wasi={fd_close:()=>52,fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error('unexpected Wasm diagnostic')}};
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi});
 const x=instance.exports;x._initialize();const align=n=>(n+15)&~15;
 const p=align(Number(x.__heap_base.value)+1024),size=x.coaxial_state_size(),max=x.coaxial_max_vertices();
 const vertices=align(p+size+16),uniforms=align(vertices+max*16*4+16),need=uniforms+80;
 if(need>memory.buffer.byteLength)memory.grow(Math.ceil((need-memory.buffer.byteLength)/65536));
 const bytes=new Uint8Array(memory.buffer);let frames=0,peak=0;
 const route=()=>new TextDecoder().decode(bytes.subarray(x.coaxial_outbox(p),x.coaxial_outbox(p)+x.coaxial_outbox_len(p)));
 function frame(w=1440,h=900,d=1){
  for(const[a,b]of[[p-8,p],[p+size,p+size+8],[vertices-8,vertices],[vertices+max*64,vertices+max*64+8],[uniforms-8,uniforms],[uniforms+64,uniforms+72]])bytes.fill(165,a,b);
  const n=x.coaxial_frame(p,vertices,uniforms,w,h,d,1/60,1);frames++;peak=Math.max(peak,n);
  assert(n>100&&n<=max,'vertex budget '+n);
  for(const[a,b]of[[p-8,p],[p+size,p+size+8],[vertices-8,vertices],[vertices+max*64,vertices+max*64+8],[uniforms-8,uniforms],[uniforms+64,uniforms+72]])assert(bytes.subarray(a,b).every(v=>v===165),'guard');
 }
 for(const[w,h,d]of[[1440,900,1],[390,844,1],[780,1688,2]]){
  x.coaxial_page(p,0);frame(w,h,d);x.coaxial_wheel(p,100000);frame(w,h,d);x.coaxial_wheel(p,-100000);frame(w,h,d);
  for(let i=0;i<rows.length;i++){
   x.coaxial_page(p,i+1);
   for(let mode=0;mode<3;mode++){
    assert.equal(x.coaxial_key(p,49+mode,0),1);frame(w,h,d);
    assert.equal(Number(x.coaxial_nip_code(p)),rows[i].expected[mode],rows[i].number+' case '+mode);
   }
   assert.equal(x.coaxial_key(p,52,0),1);frame(w,h,d);assert.equal(Number(x.coaxial_nip_view(p)),1);
   assert.equal(x.coaxial_key(p,-1,0),1);assert.equal(route(),'/nip');x.coaxial_action_done(p);
  }
  // Bottom navigation uses the same rectangles on all device scales.
  x.coaxial_page(p,6);frame(w,h,d);
  const unit=(w/d-64)/5,y=h-36*d;
  for(const[index,want]of[[0,'/nip'],[1,'/nip/17'],[2,'/nip/22'],[4,'/nip/19/index.md']]){
   const xx=(16+index*(unit+8)+unit/2)*d;
   x.coaxial_pointer(p,xx,y,0,0,1);x.coaxial_pointer(p,xx,y,2,0,1);
   assert.equal(route(),want);x.coaxial_action_done(p);
  }
 }
 x.coaxial_page(p,9999);frame();assert.equal(Number(x.coaxial_nip_page(p)),0);
 console.log(JSON.stringify({pages:rows.length,cases:rows.length*3,viewports:3,frames,peak_vertices:peak,max_vertices:max,guard_bytes:true}));
})().catch(e=>{console.error(e);process.exitCode=1});
