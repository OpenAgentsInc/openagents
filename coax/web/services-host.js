// Firm reason: 1 (browser glue: load the shared service authority and copy admitted spans).
const wasmURL=new URL('../build/host-services.wasm',import.meta.url);
let compiled;
function queueRelease(x,lease){return ()=>!!x.services_audio_queue_release(...lease);}
export async function openHostServices(){
 compiled??=(async()=>{const response=await fetch(wasmURL,{cache:'no-store'});if(!response.ok)throw Error(`host services reactor: ${response.status}`);return WebAssembly.compile(await response.arrayBuffer());})().catch(error=>{compiled=null;throw error;});
 const memory=new WebAssembly.Memory({initial:128,maximum:1024});
 const wasi={proc_exit:code=>{throw Error(`host services reactor exited ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
  clock_time_get(id,precision,at){new DataView(memory.buffer).setBigUint64(at,BigInt(Math.floor((id===0?Date.now():performance.now())*1e6)),true);return 0;},
  fd_write(fd,iov,count,written){const view=new DataView(memory.buffer);let n=0;for(let i=0;i<count;i++)n+=view.getUint32(iov+8*i+4,true);view.setUint32(written,n,true);return 0;}};
 const x=new WebAssembly.Instance(await compiled,{env:{memory},wasi_snapshot_preview1:wasi}).exports;x._initialize?.();
 if(!x.services_audio_queue_open())throw Error('Coax refused audio offer queue storage');
 const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
 return {presentation:(version,profile)=>!!x.services_render_presentation(BigInt(version),BigInt(profile)),httpFollow:version=>!!x.services_http_follow(BigInt(version)),httpResponse:(version,status)=>!!x.services_http_response(BigInt(version),BigInt(status)),
  span:(address,capacity,count,bytes)=>!!x.services_span(BigInt(address),BigInt(capacity),BigInt(count),BigInt(bytes)),
  mediaSpan:(address,capacity,count,bytes,present)=>!!x.services_media_span(BigInt(address),BigInt(capacity),BigInt(count),BigInt(bytes),Number(present)),
  bankLine(line,base,capacity){const storage=new Uint8Array(memory.buffer,x.services_line_ptr(),x.services_line_capacity()),written=encoder.encodeInto(line,storage);if(written.read!==line.length)return null;const id=x.services_bank_insert(written.written,BigInt(base),BigInt(capacity));if(id<0n||!x.services_bank_source(id))return null;
   const field=i=>Number(x.services_bank_field(i));return {id:Number(id),name:decoder.decode(new Uint8Array(memory.buffer,field(0),field(1))),address:field(2),bytes:field(3)};},
  bankCount:()=>Number(x.services_bank_count()),extent:(frames,channels)=>Number(x.services_extent(BigInt(frames),BigInt(channels))),reserve:words=>Number(x.services_reserve(BigInt(words))),release:ticket=>!!x.services_release(BigInt(ticket)),words:()=>Number(x.services_words()),
  audioLine(line){const storage=new Uint8Array(memory.buffer,x.services_line_ptr(),x.services_line_capacity()),written=encoder.encodeInto(line,storage);if(written.read!==line.length||x.services_audio_queue_offer(BigInt(written.written))!==0)return null;
   const lease=[0,1,2].map(field=>x.services_audio_queue_lease(field)),release=queueRelease(x,lease);
   try{return Object.freeze({bytes:storage.slice(0,written.written),release});}catch{release();return null;}},
  audioQueueInfo:()=>[0,1].map(field=>Number(x.services_audio_queue_info(field)))};
}
// Each handle is one concrete SDK allocation. Its ticket outlives retirement
// while immutable sample references remain; releasing a borrowed view ends use.
export function openPcmMemory(services){
 let retired=false;
 function grant(frames,channels,make){
  if(retired)return null;const words=services.extent(frames,channels);if(!words)return null;
  const ticket=services.reserve(words);if(ticket<0)return null;let samples=null,refs=1;
  try{if(make)samples=make(words);}catch{services.release(ticket);return null;}
  return Object.freeze({ticket,words,channels:Number(channels),get samples(){return samples;},
   retain(){if(!refs)return false;refs++;return true;},release(){if(!refs)return false;if(--refs===0){samples=null;services.release(ticket);}return true;}});
 }
 return Object.freeze({claim:(frames,channels)=>grant(frames,channels),allocate:(frames,channels)=>grant(frames,channels,words=>new Float32Array(words)),
  copy(source,frames,channels){const words=services.extent(frames,channels);if(!words||source.length!==words)return null;const allocation=grant(frames,channels,n=>new Float32Array(n));if(allocation)allocation.samples.set(source);return allocation;},
  total:()=>services.words(),retire(){retired=true;}});
}
