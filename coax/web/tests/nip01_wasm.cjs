// /nip/01's built module (nip01.coax, nip01-verify.coax and the C face) run
// as the browser host runs it: every case through the real verifier in Wasm,
// keys and the hotbar on desktop, phone and 2x, and guard bytes untouched.
//   node coax/web/tests/nip01_wasm.cjs apps/openagents-web/build/nip01.wasm
const fs=require('node:fs'),assert=require('node:assert/strict');
(async()=>{
  const memory=new WebAssembly.Memory({initial:128,maximum:1024});
  const wasi={fd_close:()=>52,fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error('unexpected Wasm diagnostic');}};
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi});instance.exports._initialize();
  const x=instance.exports,align=n=>(n+15)&~15,p=align(Number(x.__heap_base.value)+1024),stateSize=x.coaxial_state_size();
  const vertices=align(p+stateSize+16),uniforms=align(vertices+50400*16*4+16),u=new Uint8Array(memory.buffer);
  u.fill(165,p-8,p+stateSize+8);u.fill(0,p,p+stateSize);u.fill(165,vertices-8,vertices+50400*16*4+8);u.fill(165,uniforms-8,uniforms+64+8);
  // event.coax's codes for REAL, ID, TEXT, SIG, KIND: verified, NE_ID, NE_ID, NE_SIGNATURE, NE_SHAPE
  const errors=[0,7,7,8,2];let frames=0,ms=0;
  function frame(w=1280,h=800,d=1,still=0){
    const t=performance.now();const count=x.coaxial_frame(p,vertices,uniforms,w,h,d,1/60,still);ms=Math.max(ms,performance.now()-t);frames++;
    assert(count>100&&count<=50400,'vertex count '+count);
    const mode=Number(x.coaxial_nip_mode(p));
    if(Number(x.coaxial_nip_view(p))===0){assert.equal(Number(x.coaxial_nip_error(p)),errors[mode]);assert.equal(x.coaxial_nip_verified(p),mode===0?1:0);}
    for(const [begin,end]of[[p-8,p],[p+stateSize,p+stateSize+8],[vertices-8,vertices],[vertices+50400*16*4,vertices+50400*16*4+8],[uniforms-8,uniforms],[uniforms+64,uniforms+72]])assert(u.subarray(begin,end).every(v=>v===165),'guard bytes');
  }
  frame();assert.equal(x.coaxial_nip_verified(p),1);
  for(let i=0;i<5;i++){assert.equal(x.coaxial_key(p,49+i,0),1);frame();assert.equal(Number(x.coaxial_nip_mode(p)),i);}
  assert.equal(x.coaxial_key(p,54,0),1);frame();assert.equal(Number(x.coaxial_nip_view(p)),1);
  for(let k=0;k<40;k++)frame(1440,900,1,k&1);
  let clicks=0;
  for(const[w,h,d]of[[1280,800,1],[390,844,1],[780,1688,2]]){
    frame(w,h,d);const first=(w/d-294)/2+22,y=h-34*d;
    if(w/d<760){
      assert(x.coaxial_nip_scroll_max(p)>0,'long explanations scroll on phones');
      x.coaxial_wheel(p,100000);frame(w,h,d);assert(x.coaxial_nip_scroll(p)>0,'wheel reaches lower content');
      assert.equal(x.coaxial_key(p,-8,0),1);frame(w,h,d);assert.equal(x.coaxial_nip_scroll(p),0);
    }
    for(let i=0;i<6;i++){
      assert.equal(x.coaxial_pointer(p,(first+50*i)*d,y,0,0,1),1);
      assert.equal(x.coaxial_pointer(p,(first+50*i)*d,y,2,0,1),1);frame(w,h,d);clicks++;
      if(i<5){assert.equal(Number(x.coaxial_nip_view(p)),0);assert.equal(Number(x.coaxial_nip_mode(p)),i);}else {
        assert.equal(Number(x.coaxial_nip_view(p)),1);
        if(w/d<760){x.coaxial_wheel(p,100000);frame(w,h,d);assert(x.coaxial_nip_scroll(p)>0,'parts remain reachable');}
      }
    }
    x.coaxial_key(p,49,0);frame(w,h,d);
    x.coaxial_pointer(p,(first+50)*d,y,0,0,1);x.coaxial_pointer(p,(first+50)*d,y,3,0,1);x.coaxial_pointer(p,(first+50)*d,y,2,0,1);frame(w,h,d);assert.equal(Number(x.coaxial_nip_mode(p)),0);
    const ix=w-78*d,iy=42*d;
    x.coaxial_pointer(p,ix,iy,0,0,1);x.coaxial_pointer(p,ix,iy,2,0,1);
    assert.equal(x.coaxial_action(p),1);
    assert.equal(new TextDecoder().decode(u.subarray(x.coaxial_outbox(p),x.coaxial_outbox(p)+x.coaxial_outbox_len(p))),'/nip');
    x.coaxial_action_done(p);assert.equal(x.coaxial_action(p),0);
    assert.equal(x.coaxial_key(p,-1,0),1);assert.equal(x.coaxial_action(p),1);x.coaxial_action_done(p);
  }
  assert.equal(x.coaxial_key(p,48,0),0);assert.equal(x.coaxial_key(p,55,0),0);
  // Golden mode forwards the host's deterministic/reduced-motion request to
  // Coax even when the OS motion preference permits the normal animation.
  u.fill(0,p,p+stateSize);x.coaxial_golden(1);frame();
  const golden=new Uint8Array(u.slice(vertices,vertices+50400*64));
  frame(1280,800,1,1);assert.deepEqual(u.subarray(vertices,vertices+50400*64),golden);
  x.coaxial_golden(0);
  process.stdout.write(JSON.stringify({frames,keys:10,hud_clicks:clicks,index_navigation_viewports:3,guard_bytes:true,slowest_frame_ms:Math.round(ms*10)/10,engine:`Node${process.version}`})+'\n');
})().catch(e=>{console.error(e);process.exitCode=1;});
