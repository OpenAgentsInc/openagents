// Actual built app, including its Coax UI, shared verifier and C transport.
const fs=require('node:fs'),assert=require('node:assert/strict');
(async()=>{
  const memory=new WebAssembly.Memory({initial:128,maximum:1024});
  // Match the generic browser host's optional libc refusals. Unexpected
  // diagnostics are test failures, rather than silently swallowed assertions.
  const wasi={fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error('unexpected Wasm diagnostic');}};
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi});instance.exports._initialize();
  const x=instance.exports,align=n=>(n+15)&~15,p=align(Number(x.__heap_base.value)+1024),stateSize=x.coaxial_state_size();
  const vertices=align(p+stateSize+16),uniforms=align(vertices+50400*16*4+16),u=new Uint8Array(memory.buffer);
  u.fill(165,p-8,p+stateSize+8);u.fill(0,p,p+stateSize);u.fill(165,vertices-8,vertices+50400*16*4+8);u.fill(165,uniforms-8,uniforms+64+8);
  const errors=[0,7,7,8,2];let frames=0;
  function frame(w=1280,h=800,d=1){
    const count=x.coaxial_frame(p,vertices,uniforms,w,h,d,1/60,1);frames++;
    assert(count>0&&count<=50400);let mode=Number(x.coaxial_probe_mode(p));
    assert.equal(Number(x.coaxial_probe_error(p)),errors[mode]);assert.equal(x.coaxial_probe_verified(p),mode===0?1:0);
    assert.equal(Number(x.coaxial_probe_written(p))>0,mode===0);
    for(const [begin,end]of[[p-8,p],[p+stateSize,p+stateSize+8],[vertices-8,vertices],[vertices+50400*16*4,vertices+50400*16*4+8],[uniforms-8,uniforms],[uniforms+64,uniforms+72]])assert(u.subarray(begin,end).every(v=>v===165),'guard bytes');
  }
  assert.equal(x.coaxial_pointer(p,0,0,0,0,1),0);frame();
  for(let i=0;i<5;i++){assert.equal(x.coaxial_key(p,49+i,0),1);frame();assert.equal(Number(x.coaxial_probe_mode(p)),i);}
  for(const[w,h,d]of[[1280,800,1],[390,844,1],[780,1688,2]]){
    frame(w,h,d);const first=(w/d-244)/2+22,y=h-34*d;
    for(let i=0;i<5;i++){
      assert.equal(x.coaxial_pointer(p,(first+50*i)*d,y,0,0,1),1);
      assert.equal(x.coaxial_pointer(p,(first+50*i)*d,y,2,0,1),1);frame(w,h,d);assert.equal(Number(x.coaxial_probe_mode(p)),i);
    }
    const previous=x.coaxial_probe_mode(p);
    x.coaxial_pointer(p,first*d,y,0,0,1);x.coaxial_pointer(p,first*d,y,3,0,1);x.coaxial_pointer(p,first*d,y,2,0,1);frame(w,h,d);assert.equal(x.coaxial_probe_mode(p),previous);
    x.coaxial_pointer(p,first*d,y,0,0,1);x.coaxial_pointer(p,(first+50)*d,y,2,0,1);frame(w,h,d);assert.equal(x.coaxial_probe_mode(p),previous);
  }
  assert.equal(x.coaxial_key(p,48,0),0);
  process.stdout.write(JSON.stringify({frames,keyboard_cases:5,hud_clicks:15,cancel_or_mismatch:6,guard_bytes:true,engine:`Node${process.version} V8${process.versions.v8}`}));
})().catch(e=>{console.error(e);process.exitCode=1;});
