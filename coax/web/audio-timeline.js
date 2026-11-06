// Firm reason: 1 (browser glue: bounded encoded command copies and scalar Coax timeline calls).
const wasmURL=new URL('../build/host-audio-timeline.wasm',import.meta.url);
let compiled;
export async function openClipTimeline(){
 compiled??=(async()=>{const r=await fetch(wasmURL,{cache:'no-store'});if(!r.ok)throw Error('clip timeline reactor unavailable');return WebAssembly.compile(await r.arrayBuffer());})().catch(error=>{compiled=null;throw error;});
 const memory=new WebAssembly.Memory({initial:128,maximum:1024});
 const wasi={proc_exit:code=>{throw Error(`clip timeline reactor exited ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
  fd_write(fd,iov,count,written){const view=new DataView(memory.buffer);let n=0;for(let i=0;i<count;i++)n+=view.getUint32(iov+8*i+4,true);view.setUint32(written,n,true);return 0;}};
 const x=new WebAssembly.Instance(await compiled,{env:{memory},wasi_snapshot_preview1:wasi}).exports;x._initialize?.();x.clip_reset();
 const encoder=new TextEncoder(),scratch=new Uint8Array(1024),events=[],retirements=new Set();let active=true,retiring;
 const micros=ms=>Number.isFinite(ms)&&Number.isSafeInteger(Math.round(ms*1000))?BigInt(Math.round(ms*1000)):-1n;
 return {get active(){return active;},get events(){return events;},
  offer(ms,line){if(!active)return false;const bytes=encoder.encodeInto(line,scratch),us=micros(ms),n=bytes.read===line.length?bytes.written:1025;
   if(!x.clip_offer(us,BigInt(n)))return false;events.push([Number(x.clip_boundary(us)),scratch.slice(0,n),true]);return true;},
  seal(startMs,seconds){if(!active||!x.clip_plan(micros(startMs),micros(seconds*1000))||!x.clip_seal())throw Error('Coax refused complete clip timeline');return {start:Number(x.clip_info(3)),frames:Number(x.clip_info(4)),total:Number(x.clip_info(5))};},
  info:()=>({commands:Number(x.clip_info(0)),bytes:Number(x.clip_info(1)),failed:!!x.clip_info(2),sealed:!!x.clip_info(6),active}),
  onRetire(effect){if(!active){effect();return ()=>{};}retirements.add(effect);return ()=>retirements.delete(effect);},
  retire(){if(retiring)return retiring;active=false;return retiring=(async()=>{try{await Promise.all([...retirements].map(effect=>Promise.resolve().then(effect)));}finally{retirements.clear();events.length=0;x.clip_reset();}})();}
 };
}
