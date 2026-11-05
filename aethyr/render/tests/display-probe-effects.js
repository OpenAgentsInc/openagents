// Firm reason: 1 (browser SDK: real admitted renderer, WebGPU display and copied readback).
globalThis.displayProbeEffects={
 async run(){
  const {RenderHost}=await import('/coaxial/render.js'),memory=new WebAssembly.Memory({initial:1024,maximum:1024});
  const wasi={proc_exit:n=>{throw Error(`reactor exited ${n}`)},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,clock_time_get(id,p,at){new DataView(memory.buffer).setBigUint64(at,BigInt(Math.floor((id===0?Date.now():performance.now())*1e6)),true);return 0;},fd_write(fd,iov,n,at){const v=new DataView(memory.buffer);let count=0;for(let i=0;i<n;i++)count+=v.getUint32(iov+i*8+4,true);v.setUint32(at,count,true);return 0;}};
  const reply=await fetch('/renderer.wasm');if(!reply.ok)throw Error(`reactor ${reply.status}`);const {instance}=await WebAssembly.instantiate(await reply.arrayBuffer(),{env:{memory},wasi_snapshot_preview1:wasi});instance.exports._initialize?.();
  const plan=JSON.parse(new TextDecoder().decode(W.files['plan.json'])),adapter=await navigator.gpu.requestAdapter(),device=await adapter.requestDevice();this.device=device;this.errors=[];device.addEventListener('uncapturederror',event=>this.errors.push(event.error.message));
  const canvas=document.createElement('canvas');canvas.width=plan.width;canvas.height=plan.height;document.body.append(canvas);const context=canvas.getContext('webgpu');context.configure({device,format:plan.display_format,alphaMode:'opaque',usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC});
  const r=this.renderer=new RenderHost({device,context,format:plan.display_format,memory,exports:instance.exports,arena:{},url:'/package/render.json',profile:plan.profile,coarse:false,presentation:'unorm8-v1',presentationOnly:true});
  await r.initialize();const result=await r.probePresentation(W.files['source.rgba16f'],plan.width,plan.height);this.output=new Uint8Array(result.bytes);delete result.bytes;this.receipt=result;this.info=adapter.info?{vendor:adapter.info.vendor,architecture:adapter.info.architecture,device:adapter.info.device,description:adapter.info.description}:{};
  this.uniforms=Array.from(new Float32Array(memory.buffer,instance.exports.coaxial_render_presentation_uniforms(),instance.exports.coaxial_render_presentation_uniform_bytes()/4));r.destroy();this.ledger=r.ledger();device.destroy();return {receipt:this.receipt,uniforms:this.uniforms,ledger:this.ledger,errors:this.errors,adapter:this.info};
 },
 read(at,count){const bytes=this.output.subarray(at,at+count);let text='';for(let i=0;i<bytes.length;i+=32768)text+=String.fromCharCode(...bytes.subarray(i,i+32768));return btoa(text);}
};
'ready'
