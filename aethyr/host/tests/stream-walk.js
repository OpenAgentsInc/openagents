// Firm reason: 1 (browser SDK: actual pack fetches, Wasm malloc spans and WebGPU effects; Coax owns all residency, movement, decoding and draw selection).
import {openPackHost} from '../../../coax/web/pack-host.js';

export async function openStreamWalk({canvas,sceneURL,sceneSHA,sceneBytes,manifestSHA,tier,wasmURL,shaderURL,onRecord}){
  const packs=await openPackHost(),arena=packs.retainBytes(8388608),startup=[];let memory;
  startup.push(()=>{if(memory)new Uint8Array(memory.buffer).fill(0);memory=null;arena.release();});
  try{
  // libc may need a new page after the linker's initial heap tail. Reserve
  // the existing 8 MiB ceiling, and let its 2 MiB initial extent grow within it.
  memory=new WebAssembly.Memory({initial:32,maximum:128});
  const runtimeMessages=[];
  const wasi={proc_exit(code){throw Error(`stream reactor exited ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
    fd_write(fd,iov,count,written){const d=new DataView(memory.buffer);let bytes=0,text='';for(let i=0;i<count;i++){const at=d.getUint32(iov+i*8,true),n=d.getUint32(iov+i*8+4,true);bytes+=n;text+=new TextDecoder().decode(new Uint8Array(memory.buffer,at,n));}if(text){runtimeMessages.push(text);if(runtimeMessages.length>32)runtimeMessages.shift();console.error(text);}d.setUint32(written,bytes,true);return 0;}};
  const module=await WebAssembly.compile(await(await fetch(wasmURL)).arrayBuffer());
  const instance=await WebAssembly.instantiate(module,{env:{memory},wasi_snapshot_preview1:wasi});
  let x=instance.exports;x._initialize?.();
  const info=f=>Number(x.sw_host_info(f)),row=(i,f)=>Number(x.sw_host_row(i,f));
  const authenticate=(bytes,hex)=>{if(bytes.byteLength>32768)throw Error('fixture metadata exceeds retained config span');new Uint8Array(memory.buffer,x.sw_config_ptr(),bytes.byteLength).set(bytes);new Uint8Array(memory.buffer,x.sw_root_ptr(),64).set(new TextEncoder().encode(hex));if(!x.sw_host_auth(bytes.byteLength))throw Error('Coax refused complete content identity');};
  const base=new URL('.',new URL(sceneURL,location.href));
  const metadataOwner={kind:'scene-index'},metadata=await packs.loadBytes(sceneURL,sceneBytes,{owner:metadataOwner});
  let scene;
  try{await metadata.use(({bytes})=>{authenticate(bytes,sceneSHA);scene=JSON.parse(new TextDecoder().decode(bytes));});}finally{await packs.retire(metadataOwner);}
  const descriptor=scene.manifests.find(m=>m.backend===0&&m.tier===tier&&m.sha256===manifestSHA);
  if(!descriptor)throw Error('no pinned manifest for the requested browser tier');
  const manifestOwner={kind:'manifest'},manifest=await packs.loadBytes(new URL(`${manifestSHA}.rcm`,base),28208,{owner:manifestOwner});
  try{await manifest.use(({bytes})=>{new Uint8Array(memory.buffer,x.sw_config_ptr(),bytes.byteLength).set(bytes);new Uint8Array(memory.buffer,x.sw_root_ptr(),64).set(new TextEncoder().encode(manifestSHA));if(!x.sw_host_start(bytes.byteLength,tier,0))throw Error('Coax refused scene manifest');});}finally{await packs.retire(manifestOwner);}
  let adapter=await navigator.gpu.requestAdapter({powerPreference:'high-performance'});if(!adapter)throw Error('WebGPU adapter unavailable');
  const errors=[],shaderSource=await(await fetch(shaderURL)).text(),format=navigator.gpu.getPreferredCanvasFormat(),context=canvas.getContext('webgpu');
  let device,uniforms,depth,color,pipeline,sampler;
  async function makeSurface(){
    const created=[],tokens=[];device=await adapter.requestDevice();device.addEventListener('uncapturederror',e=>errors.push(e.error.message));
    try{
      const shader=device.createShaderModule({code:shaderSource}),diagnostics=await shader.getCompilationInfo();
      if(diagnostics.messages.some(m=>m.type==='error'))throw Error(diagnostics.messages.map(m=>m.message).join('\n'));
      context.configure({device,format,usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_DST,alphaMode:'opaque'});
      for(const [index,kind,words] of [[0,1,1024n],[1,3,BigInt(canvas.width*canvas.height)],[2,3,BigInt(canvas.width*canvas.height)]]){if(!x.sw_host_fixed(index,kind,words))throw Error('Coax refused persistent draw storage');tokens.push(index);}
      uniforms=device.createBuffer({size:4096,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});created.push(uniforms);
      depth=device.createTexture({size:[canvas.width,canvas.height],format:'depth32float',usage:GPUTextureUsage.RENDER_ATTACHMENT});created.push(depth);
      color=device.createTexture({size:[canvas.width,canvas.height],format,usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC});created.push(color);
      pipeline=await device.createRenderPipelineAsync({layout:'auto',vertex:{module:shader,entryPoint:'sd_vs',buffers:[{arrayStride:48,attributes:[{shaderLocation:0,offset:0,format:'float32x3'},{shaderLocation:1,offset:12,format:'float32x3'},{shaderLocation:2,offset:24,format:'float32x2'}]}]},fragment:{module:shader,entryPoint:'sd_fs',targets:[{format}]},primitive:{topology:'triangle-list',cullMode:'none'},depthStencil:{format:'depth32float',depthWriteEnabled:true,depthCompare:'less'}});
      sampler=device.createSampler({magFilter:'nearest',minFilter:'nearest',mipmapFilter:'nearest'});
    }catch(error){for(const object of created)object.destroy();for(const token of tokens)x.sw_host_fixed_release(token);context.unconfigure();device.destroy();throw error;}
  }
  await makeSurface();startup.push(()=>{uniforms.destroy();depth.destroy();color.destroy();for(let i=0;i<3;i++)x.sw_host_fixed_release(i);context.unconfigure();device.destroy();});
  const gpu=new Map(),pending=new Set(),transfers=new Map();
  let closed=false,framing=false,loadFailures=0,drawnCommands=0,fallbackDraws=0,finalSnapshot,observedLive=0;
  const stages=[],recentFailures=[];
  // WebGPU error scopes are a device-wide stack. Effects sharing that
  // stack must finish/pop before another owner pushes its scope.
  let gpuTail=Promise.resolve();
  function gpuTask(effect){const task=gpuTail.then(effect);gpuTail=task.catch(()=>{});return task;}
  const current=k=>!closed&&x.sw_host_row(k.owner,10)===k.generation&&row(k.owner,8)===1&&row(k.owner,9)===1;
  function upload(k){return gpuTask(()=>uploadNow(k));}
  async function uploadNow(k){
    const format=row(k.owner,0),bytes=row(k.owner,3);if(!bytes)return;
    if(!x.sw_host_gpu_live(k.owner))throw Error('GPU ledger grant is not live');
    device.pushErrorScope('validation');let object,validation;
    try{
      if(format===5){
        const side=row(k.owner,4),levels=row(k.owner,6),address=x.sw_host_pointer(k.owner,1);
        object=device.createTexture({size:[side,side],mipLevelCount:levels,format:'rgba8unorm',usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST});gpu.set(k.owner,{object,generation:k.generation,texture:true,bytes});
        let offset=0;for(let level=0;level<levels;level++){const size=Math.max(1,side>>level),n=size*size*4;device.queue.writeTexture({texture:object,mipLevel:level},new Uint8Array(memory.buffer,address+offset,n),{bytesPerRow:size*4,rowsPerImage:size},[size,size]);offset+=n;}
      }else{
        object=device.createBuffer({size:bytes,usage:(format===2?GPUBufferUsage.INDEX:GPUBufferUsage.VERTEX)|GPUBufferUsage.COPY_DST});gpu.set(k.owner,{object,generation:k.generation,texture:false,bytes});
        device.queue.writeBuffer(object,0,new Uint8Array(memory.buffer,x.sw_host_pointer(k.owner,0),row(k.owner,1)));
      }
      await device.queue.onSubmittedWorkDone();
    }finally{validation=await device.popErrorScope();}
    if(validation)throw Error(validation.message);
  }
  async function load(k){
    const started=performance.now(),stage={owner:k.owner,generation:String(k.generation),epoch:String(k.epoch),source:scene.resources[k.owner].sha256};
    const owner={...k},allocation=x.sw_host_allocate(k.owner,k.generation,k.epoch);let status=1;
    transfers.set(k.owner,{key:k,owner});
    stage.allocationMs=performance.now()-started;
    try{
      if(!allocation)throw Error('physical allocation refused');
      const location=scene.resources[k.owner],url=new URL(`${location.container_sha256}${location.raw?'.raw':'.vpk'}`,base),ioStart=performance.now();
      const transfer=location.raw?await packs.loadBytes(url,location.bytes,{owner,current:()=>current(k)}):await packs.load({url,quiet:true},{owner,current:()=>current(k)});
      await transfer.use(source=>{
        let bytes;
        if(location.raw)bytes=source.bytes;
        else{
          authenticate(source.source,location.container_sha256);
          const entry=source.entries.find(e=>e.bytes.byteOffset-source.source.byteOffset===location.offset&&e.bytes.byteLength>=location.bytes);
          if(!entry)throw Error('resource span is not an admitted pack entry');bytes=entry.bytes.subarray(0,location.bytes);
        }
        if(bytes.byteLength!==row(k.owner,1))throw Error('source does not fill its exact grant');
        new Uint8Array(memory.buffer,x.sw_host_pointer(k.owner,0),bytes.byteLength).set(bytes);
      });
      stage.sourceMs=performance.now()-ioStart;const decodeStart=performance.now();
      if(x.sw_host_decode(k.owner,k.generation,k.epoch)!==0)throw Error('canonical source/decoder refusal');
      stage.decodeMs=performance.now()-decodeStart;const uploadStart=performance.now();
      await upload(k);
      stage.uploadMs=performance.now()-uploadStart;
      status=0;
    }catch(error){loadFailures++;stage.error=String(error);recentFailures.push({owner:k.owner,error:String(error),allocator:Array.from({length:5},(_,f)=>Number(x.sw_host_allocator(f)))});if(recentFailures.length>8)recentFailures.shift();console.warn('stream resource refused',k.owner,String(error));}
    finally{await packs.retire(owner);transfers.delete(k.owner);if(!x.sw_host_finish(k.owner,k.generation,k.epoch,status))throw Error('completion queue pressure requires retained retry');stage.status=status;stage.totalMs=performance.now()-started;stages.push(stage);}
  }
  const command=(retiring,index)=>({owner:Number(x.sw_host_command(retiring,index,0)),generation:x.sw_host_command(retiring,index,1),epoch:x.sw_host_command(retiring,index,2)});
  function launch(k){let task;task=load(k).finally(()=>pending.delete(task));pending.add(task);}
  async function retire(k){const resource=gpu.get(k.owner);if(resource){if(resource.generation!==k.generation)throw Error('retirement would destroy another generation');resource.object.destroy();gpu.delete(k.owner);}if(!x.sw_host_destroy(k.owner,k.generation,k.epoch))throw Error('destruction queue pressure requires retained retry');}
  function render(){return gpuTask(renderNow);}
  async function renderNow(){
    const commands=[],borrowed=new Set(),count=x.sw_host_draw_count();
    device.pushErrorScope('validation');
    try{
    for(let i=0;i<count;i++){
      const fields=Array.from({length:7},(_,f)=>Number(x.sw_host_draw(i,f)));
      for(const owner of new Set(fields.slice(0,4))){if(!gpu.has(owner)||!x.sw_host_gpu_live(owner))throw Error('draw has no concrete live GPU owner');if(!borrowed.has(owner)){if(!x.sw_host_borrow(owner))throw Error('draw borrow refused');borrowed.add(owner);}}
      commands.push(fields);
    }
      const encoder=device.createCommandEncoder(),pass=encoder.beginRenderPass({colorAttachments:[{view:color.createView(),clearValue:{r:.03,g:.05,b:.08,a:1},loadOp:'clear',storeOp:'store'}],depthStencilAttachment:{view:depth.createView(),depthClearValue:1,depthLoadOp:'clear',depthStoreOp:'store'}});pass.setPipeline(pipeline);
      for(let i=0;i<commands.length;i++){
        const [geometry,indices,texture,lighting,cell,n,fallback]=commands[i];const address=x.sw_host_uniforms(cell,canvas.width/canvas.height);if(!address)throw Error('camera/placement uniform refused');
        device.queue.writeBuffer(uniforms,i*256,new Uint8Array(memory.buffer,address,256));
        const group=device.createBindGroup({layout:pipeline.getBindGroupLayout(0),entries:[{binding:0,resource:{buffer:uniforms,offset:i*256,size:256}},{binding:1,resource:gpu.get(texture).object.createView()},{binding:2,resource:gpu.get(lighting).object.createView()},{binding:3,resource:sampler}]});
        pass.setBindGroup(0,group);pass.setVertexBuffer(0,gpu.get(geometry).object);pass.setIndexBuffer(gpu.get(indices).object,'uint32');pass.drawIndexed(n);drawnCommands++;fallbackDraws+=fallback;
      }
      pass.end();encoder.copyTextureToTexture({texture:color},{texture:context.getCurrentTexture()},[canvas.width,canvas.height]);device.queue.submit([encoder.finish()]);await device.queue.onSubmittedWorkDone();
    }finally{for(const owner of borrowed)if(!x.sw_host_return(owner))throw Error('final draw borrow return refused');const validation=await device.popErrorScope();if(validation)throw Error(validation.message);}
  }
  async function step(ns,direction=0){
    if(closed||framing)throw Error('frame is closed or already running');framing=true;const start=performance.now();
    try{
      if(!x.sw_host_frame(BigInt(ns),direction))throw Error('paid frame refused');
      const stepDigests=Array.from({length:x.sw_host_step_count()},(_,step)=>Array.from({length:8},(_,word)=>Number(x.sw_host_step_word(step,word)).toString(16).padStart(8,'0')).join(''));
      const loads=Array.from({length:info(1)},(_,i)=>command(0,i)),retiring=Array.from({length:info(2)},(_,i)=>command(1,i));
      const records=Array.from({length:info(3)},(_,i)=>Array.from({length:23},(_,f)=>String(x.sw_host_record(i,f))));
      onRecord?.(records);x.sw_host_ack();
      for(const transfer of transfers.values())if(!current(transfer.key))packs.abortReads(transfer.owner);
      for(const k of retiring)await retire(k);for(const k of loads)launch(k);
      await render();if(errors.length)throw Error(errors.join('\n'));
      const traceCount=x.sw_host_trace_count();if(traceCount<0)throw Error('physical allocation trace overflow');
      const allocations=Array.from({length:traceCount},(_,i)=>Array.from({length:6},(_,f)=>Number(x.sw_host_trace(i,f))));x.sw_host_trace_ack();
      for(const [freed,owner,generation,epoch,field,bytes] of allocations)observedLive+=freed?-bytes:bytes;
      const cpu=info(14),gpuBytes=[...gpu.values()].reduce((n,r)=>n+r.bytes,0);
      if(observedLive!==cpu||cpu>info(10)||cpu+gpuBytes>info(8))throw Error('physical allocations disagree with original tickets/declared peak');
      return {frameNs:ns,direction,tick:info(0),position:Array.from({length:3},(_,i)=>x.sw_host_player(i)),active:info(4),drawn:info(5),updated:info(6),declared:info(8),cpu,gpuBytes,allocations,stages:stages.splice(0),records,stepDigests,frameMs:performance.now()-start};
    }finally{framing=false;}
  }
  async function close(){
    if(closed||framing||!x.sw_host_close())throw Error('close refused');
    for(const transfer of transfers.values())packs.abortReads(transfer.owner);
    await Promise.all([...pending]);
    const closingFrames=[];for(let i=0;i<128&&!info(24);i++)closingFrames.push(await step(15625000,0));
    if(!info(24)||info(14)||info(10)||info(22)||info(23))throw Error('physical resources did not drain');
    await device.queue.onSubmittedWorkDone();uniforms.destroy();depth.destroy();color.destroy();for(let i=0;i<3;i++)if(!x.sw_host_fixed_release(i))throw Error('fixed ledger cleanup refused');
    const result={info:Array.from({length:28},(_,f)=>info(f)),closingFrames,loadFailures,drawnCommands,fallbackDraws,wasmBytesBeforeDispose:memory.buffer.byteLength,wasmReservedBytes:arena.bytes,transportBeforeDispose:packs.info(),gpuErrors:errors.slice()};
    if(info(12)||info(13))throw Error('GPU ledger did not drain');closed=true;context.unconfigure();device.destroy();new Uint8Array(memory.buffer).fill(0);x=null;memory=null;arena.release();result.transport=packs.info();finalSnapshot=result;return result;
  }
  async function capture(){
    if(closed||framing)throw Error('capture needs a completed frame');framing=true;
    try{return await gpuTask(captureNow);}finally{framing=false;}
  }
  async function captureNow(){
    const rowBytes=Math.ceil(canvas.width*4/256)*256,bytes=rowBytes*canvas.height;
    if(!x.sw_host_fixed(3,2,BigInt(bytes/4)))throw Error('readback ledger refused');
    let buffer,retained;
    try{
      buffer=device.createBuffer({size:bytes,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
      const encoder=device.createCommandEncoder();encoder.copyTextureToBuffer({texture:color},{buffer,bytesPerRow:rowBytes,rowsPerImage:canvas.height},[canvas.width,canvas.height]);device.queue.submit([encoder.finish()]);await buffer.mapAsync(GPUMapMode.READ);
      retained=packs.retainBytes(bytes);const copy=new Uint8Array(buffer.getMappedRange()).slice();buffer.unmap();
      let binary='';for(let at=0;at<copy.length;at+=32768)binary+=String.fromCharCode(...copy.subarray(at,at+32768));const data=btoa(binary);copy.fill(0);
      return {width:canvas.width,height:canvas.height,rowBytes,format,data};
    }finally{buffer?.destroy();retained?.release();if(!x.sw_host_fixed_release(3))throw Error('readback cleanup refused');}
  }
  // Explicit SDK replacement probe. Automatic unsolicited-loss scheduling
  // and failed-replacement rescue retain their separate qualification exit.
  async function recover(){
    if(closed||framing||!x.sw_host_recover)throw Error('replacement unavailable or frame still active');framing=true;const started=performance.now();
    try{
      for(const transfer of transfers.values())packs.abortReads(transfer.owner);await Promise.all([...pending]);await device.queue.onSubmittedWorkDone();
      const before=Array.from({length:28},(_,f)=>info(f)),lostDevice=device;
      lostDevice.destroy();const loss=await lostDevice.lost;
      for(const resource of gpu.values())resource.object.destroy();gpu.clear();uniforms.destroy();depth.destroy();color.destroy();context.unconfigure();
      if(!x.sw_host_recover())throw Error(`Coax refused GPU replacement:${x.sw_host_recover_info(0)}`);
      adapter=await navigator.gpu.requestAdapter({powerPreference:'high-performance'});if(!adapter)throw Error('replacement WebGPU adapter unavailable');
      await makeSurface();let uploaded=0;
      for(let owner=0;owner<64;owner++)if(x.sw_host_uploadable(owner)){await upload({owner,generation:x.sw_host_row(owner,10)});uploaded++;}
      await render();if(errors.length)throw Error(errors.join('\n'));
      return {before,after:Array.from({length:28},(_,f)=>info(f)),epoch:Number(x.sw_host_recover_info(2)),granted:Number(x.sw_host_recover_info(1)),uploaded,lossReason:loss.reason,adapter:{vendor:adapter.info.vendor,architecture:adapter.info.architecture,device:adapter.info.device,description:adapter.info.description},ms:performance.now()-started};
    }finally{framing=false;}
  }
  return Object.freeze({step,settle:()=>Promise.all([...pending]),close,capture,recover,profile:Object.freeze({tier,manifestSHA,sceneSHA,stateSchema:'aethyr.stream-walk-state.v1'}),cells:()=>Array.from({length:12},(_,i)=>Array.from({length:5},(_,f)=>Number(x.sw_host_cell(i,f)))),stats:()=>closed?finalSnapshot:({info:Array.from({length:28},(_,f)=>info(f)),wasmBytes:memory.buffer.byteLength,wasmReservedBytes:arena.bytes,transport:packs.info(),loadFailures,recentFailures:recentFailures.slice(),drawnCommands,fallbackDraws,gpuErrors:errors.slice(),runtimeMessages:runtimeMessages.slice()}),adapter:{vendor:adapter.info.vendor,architecture:adapter.info.architecture,device:adapter.info.device,description:adapter.info.description,userAgent:navigator.userAgent}});
  }catch(error){for(const dispose of startup.reverse())await dispose();throw error;}
}
