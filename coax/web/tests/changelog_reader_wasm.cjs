// Firm reason: 2 (independent native-browser ABI and memory oracle).
const fs=require('node:fs'),assert=require('node:assert/strict');
(async()=>{
 const raw=fs.readFileSync(process.argv[3]),firstRelease=Number(process.argv[4]);assert(Number.isInteger(firstRelease)&&firstRelease>=0);const memory=new WebAssembly.Memory({initial:1024,maximum:1024});
 const env={memory,coaxial_http:()=>0};const wasi={fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error('unexpected reader diagnostic')}};
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env,wasi_snapshot_preview1:wasi});const x=instance.exports;x._initialize();
 const align=n=>(n+15)&~15,state=align(Number(x.__heap_base.value)+1024),size=x.coaxial_state_size(),max=x.coaxial_max_vertices(),stride=x.coaxial_vertex_floats();
 const vertices=align(state+size+32),uniforms=align(vertices+max*stride*4+32),un=x.coaxial_uniform_floats()*4,bytes=new Uint8Array(memory.buffer);assert(uniforms+un+32<bytes.length);
 let peak=0,frames=0;const css=[];const text=()=>Buffer.from(bytes.subarray(x.coaxial_outbox(state),x.coaxial_outbox(state)+x.coaxial_outbox_len(state)));
 for(const[w,h,d]of[[320,568,1],[390,844,1],[1440,900,1],[390,844,2],[768,1024,3]]){
  bytes.fill(0,state,state+size);if(x.coaxial_state_init)x.coaxial_state_init(state);
  const guards=[[state-16,state],[state+size,state+size+16],[vertices-16,vertices],[vertices+max*stride*4,vertices+max*stride*4+16],[uniforms-16,uniforms],[uniforms+un,uniforms+un+16]];
  for(const[a,b]of guards)bytes.fill(165,a,b);
  for(let position=0;position<3;position++){
   if(position===1)x.coaxial_wheel(state,12345);if(position===2)assert(x.coaxial_key(state,-9,0));
   const wanted=x.coaxial_changelog_scroll(state);
   const n=x.coaxial_frame(state,vertices,uniforms,w*d,h*d,d,.016,1);assert(n>100&&n<=max);peak=Math.max(peak,n);frames++;
   assert(new Float32Array(memory.buffer,vertices,n*stride).every(Number.isFinite));assert(x.coaxial_changelog_first_visible(state)>=0);assert(x.coaxial_changelog_last_visible(state)>=x.coaxial_changelog_first_visible(state));
   if(position===0)assert(x.coaxial_changelog_first_visible(state)===firstRelease,'first frame must show latest release');
   else assert(x.coaxial_changelog_scroll(state)===wanted,'frames retain user scroll');
   if(position===2)assert(x.coaxial_changelog_scroll(state)===x.coaxial_changelog_limit(state)&&x.coaxial_changelog_first_visible(state)>0);
   if(w===390&&d===1)css[position]=[x.coaxial_changelog_first_visible(state),x.coaxial_changelog_last_visible(state)];if(w===390&&d===2)assert.deepEqual([x.coaxial_changelog_first_visible(state),x.coaxial_changelog_last_visible(state)],css[position]);
   if(position===2)assert(x.coaxial_probe(state)===x.coaxial_guard_probe(state,0));
   for(const[a,b]of guards)assert(bytes.subarray(a,b).every(v=>v===165),'guard bytes');
  }
  for(let control=0;control<4;control++){
   const b=[0,1,2,3].map(a=>x.coaxial_changelog_control(state,control,a));assert(b[0]>=16&&b[2]<=w-16&&b[3]-b[1]>=44);
   const cx=(b[0]+b[2])*.5*d,cy=(b[1]+b[3])*.5*d;assert(x.coaxial_pointer(state,cx,cy,0,1,3));assert(x.coaxial_pointer(state,cx,cy,2,1,3));
   if(control===0)assert(text().toString()==='/projects/coax');if(control===2)assert(text().toString()==='/coax/changelog/index.md');if(control===3)assert(x.coaxial_action(state)===2&&text().equals(raw));x.coaxial_action_done(state);
   assert(x.coaxial_pointer(state,cx,cy,0,1,3));assert(x.coaxial_pointer(state,cx,cy+30*d,1,1,3));assert(x.coaxial_pointer(state,cx,cy+30*d,2,1,3));assert(x.coaxial_action(state)===0);
  }
  assert(x.coaxial_key(state,-15,0));assert(x.coaxial_key(state,-2,0));assert(text().toString()==='/projects/coax');x.coaxial_action_done(state);
 }
 const previous=x.coaxial_changelog_limit(state);assert(x.coaxial_frame(state,vertices,uniforms,768*3,1224*3,3,0,1)>0);assert(x.coaxial_changelog_limit(state)<=previous&&x.coaxial_changelog_scroll(state)<=x.coaxial_changelog_limit(state));frames++;
 console.log(JSON.stringify({viewports:5,frames,peak_vertices:peak,max_vertices:max,guard_bytes:true,finite_geometry:true,copy_bytes:raw.length}));
})().catch(e=>{console.error(e);process.exitCode=1});
