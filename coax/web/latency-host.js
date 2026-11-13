// Firm reason: 1 (browser glue: concrete socket-owned Coax observer, SDK clocks and byte copies).
let compiled;
const us=()=>BigInt(Math.max(1,Math.floor(performance.now()*1000)));
// A binary input (#13278: opcode 177 I or 178 J1) as the text the observer reads, "I SEQ". SEQ is
// the first field: groups of 7 bits, the lowest first, each followed by a bit that says another follows,
// so from the second byte on each byte is one group and its bit. Any other binary frame is '' (not an input).
export function binaryInput(bytes){
 if(bytes[0]!==177&&bytes[0]!==178)return '';
 let seq=0,scale=1;
 for(let at=1;at<9;at++,scale*=128){if(at>=bytes.length)return '';seq+=(bytes[at]>>1)*scale;if(!(bytes[at]&1))return `I ${seq}`;}
 return '';
}
export async function openLatencyObserver(){
 compiled??=(async()=>{const r=await fetch(new URL('../build/latency-client.wasm',import.meta.url),{cache:'no-store'});if(!r.ok)throw Error(`latency observer ${r.status}`);return WebAssembly.compile(await r.arrayBuffer());})();
 const memory=new WebAssembly.Memory({initial:32,maximum:1024});
 const wasi={proc_exit:n=>{throw Error(`latency observer exited ${n}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
  fd_write(fd,iovs,n,written){const v=new DataView(memory.buffer);let bytes=0;for(let i=0;i<n;i++)bytes+=v.getUint32(iovs+8*i+4,true);v.setUint32(written,bytes,true);return 0;}};
 const x=new WebAssembly.Instance(await compiled,{env:{memory},wasi_snapshot_preview1:wasi}).exports;x._initialize?.();x.lt_init();
 const encoder=new TextEncoder(),decoder=new TextDecoder();
 function put(text){const bytes=encoder.encode(text);if(bytes.length>4300)return -1;new Uint8Array(memory.buffer,x.lt_inbox(),bytes.length).set(bytes);return bytes.length;}
 return {before(text){if(typeof text!=='string')text=binaryInput(text);const n=put(text);if(n<0)return '';const size=x.lt_before(n,us());return size?decoder.decode(new Uint8Array(memory.buffer,x.lt_outbox(),size)):'';},
  receive(text){const n=put(text);if(n>=0)x.lt_receive(n,us());},
  applied(ack,tick,lag){x.lt_applied(BigInt(ack),BigInt(tick),us(),BigInt(lag));},
  submitted(queue,lag,current){const id=x.lt_submit(us(),BigInt(lag));if(id)queue.onSubmittedWorkDone().then(()=>{if(current())x.lt_complete(id,us());else x.lt_retire();}).catch(()=>x.lt_retire());},
  retire:()=>x.lt_retire(),snapshot:()=>({rows:Array.from({length:x.lt_count()},(_,r)=>Array.from({length:23},(_,f)=>Number(x.lt_value(r,f)))),expired:Number(x.lt_expired()),refused:Number(x.lt_refused()),missing:Number(x.lt_missing())})};
}
