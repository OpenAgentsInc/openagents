// Firm reason: 1 (browser glue: load the shared service authority and copy admitted spans).
const wasmURL=new URL('../build/host-services.wasm',import.meta.url);
let compiled;
export async function openHostServices(){
 compiled??=(async()=>{const response=await fetch(wasmURL,{cache:'no-store'});if(!response.ok)throw Error(`host services reactor: ${response.status}`);return WebAssembly.compile(await response.arrayBuffer());})().catch(error=>{compiled=null;throw error;});
 const memory=new WebAssembly.Memory({initial:128,maximum:1024});
 const wasi={proc_exit:code=>{throw Error(`host services reactor exited ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
  clock_time_get(id,precision,at){new DataView(memory.buffer).setBigUint64(at,BigInt(Math.floor((id===0?Date.now():performance.now())*1e6)),true);return 0;},
  fd_write(fd,iov,count,written){const view=new DataView(memory.buffer);let n=0;for(let i=0;i<count;i++)n+=view.getUint32(iov+8*i+4,true);view.setUint32(written,n,true);return 0;}};
 const x=new WebAssembly.Instance(await compiled,{env:{memory},wasi_snapshot_preview1:wasi}).exports;x._initialize?.();
 const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
 return {span:(address,capacity,count,bytes)=>!!x.services_span(BigInt(address),BigInt(capacity),BigInt(count),BigInt(bytes)),
  bankLine(line,base,capacity){const storage=new Uint8Array(memory.buffer,x.services_line_ptr(),x.services_line_capacity()),written=encoder.encodeInto(line,storage);if(written.read!==line.length)return null;const id=x.services_bank_insert(written.written,BigInt(base),BigInt(capacity));if(id<0n||!x.services_bank_source(id))return null;
   const field=i=>Number(x.services_bank_field(i));return {id:Number(id),name:decoder.decode(new Uint8Array(memory.buffer,field(0),field(1))),address:field(2),bytes:field(3)};},
  extent:(frames,channels)=>Number(x.services_extent(BigInt(frames),BigInt(channels))),reserve:words=>Number(x.services_reserve(BigInt(words))),release:ticket=>!!x.services_release(BigInt(ticket)),words:()=>Number(x.services_words())};
}
