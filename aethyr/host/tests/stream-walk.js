// Firm reason: 1 (browser SDK: actual pack fetches, Wasm malloc spans and WebGPU effects; Coax owns all residency, movement, decoding and draw selection).
import {openPackHost} from '../../../coax/web/pack-host.js';
import {openRescueStore,rescueHash,showRescueStop} from './stream-walk-rescue.js';

export async function openStreamWalk({canvas,sceneURL,sceneSHA,sceneBytes,manifestSHA,tier,wasmURL,shaderURL,hudShaderURL='/build/stream-hud.wgsl',presentShaderURL='/build/stream-present.wgsl',onRecord,rescueSlot='stream-walk'}){
  const packs=await openPackHost(),arena=packs.retainBytes(8388608),startup=[];let memory;
  startup.push(()=>{if(memory)new Uint8Array(memory.buffer).fill(0);memory=null;arena.release();});
  try{
  // libc may need a new page after the linker's initial heap tail. Reserve
  // the existing 8 MiB ceiling, and let its 2 MiB initial extent grow within it.
  memory=new WebAssembly.Memory({initial:32,maximum:128});
  const runtimeMessages=[];
  const wasi={proc_exit(code){throw Error(`stream reactor exited ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
    fd_write(fd,iov,count,written){const d=new DataView(memory.buffer);let bytes=0,text='';for(let i=0;i<count;i++){const at=d.getUint32(iov+i*8,true),n=d.getUint32(iov+i*8+4,true);bytes+=n;text+=new TextDecoder().decode(new Uint8Array(memory.buffer,at,n));}if(text){runtimeMessages.push(text);if(runtimeMessages.length>32)runtimeMessages.shift();console.error(text);}d.setUint32(written,bytes,true);return 0;}};
  const engineBytes=await(await fetch(wasmURL)).arrayBuffer(),module=await WebAssembly.compile(engineBytes);
  const instance=await WebAssembly.instantiate(module,{env:{memory},wasi_snapshot_preview1:wasi});
  let x=instance.exports;x._initialize?.();
  const rescuable=!!x.sw_rescue_check,engineSHA=rescuable?await rescueHash(engineBytes):null;
  const automatic=!!x.sw_loss_open;if(automatic&&!x.sw_loss_open())throw Error('device policy initialization refused');
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
  const errors=[],shaderSource=await(await fetch(shaderURL)).text(),hudShaderSource=await(await fetch(hudShaderURL)).text(),presentShaderSource=await(await fetch(presentShaderURL)).text(),format=navigator.gpu.getPreferredCanvasFormat(),context=canvas.getContext('webgpu');
  let device,uniforms,depth,color,hdr,pipeline,sampler,hudVertices,hudUniforms,hudPipeline,presentPipeline;
  let hudDraws=0,lastHudVertices=0;
  const gpu=new Map(),pending=new Set(),transfers=new Map(),retiredDevices=new WeakSet();
  let closed=false,framing=false,loadFailures=0,drawnCommands=0,fallbackDraws=0,finalSnapshot,observedLive=0;
  let gpuTail=Promise.resolve(),recovery=null,replacementWork=null,initialized=false,earlyLoss=false;
  let lastRecovery=null,recoveryError=null,surfaceLive=false;
  let held=null,heldOwner=null,heldEvery=0,restoredFrames=-1,trapArmed=false,rescueSaving=null,rescuePanel=null;
  const rescueState={supported:rescuable,restored:false,saved:null,suppressed:false,refusal:null,slot:rescueSlot};
  const rescueStore=rescuable?await openRescueStore(`aethyr.stream-walk.${rescueSlot}`):null;
  startup.push(()=>{heldOwner?.release();rescueStore?.close();rescuePanel?.();});
  const saved=rescueStore?await rescueStore.take():null;
  if(saved){
    // Validate the byte extent before hashing or constructing a memory view.
    // The Coax gate still decides the full supported layout/profile/identity.
    const size=saved.bytes instanceof ArrayBuffer?saved.bytes.byteLength:0;
    let retained;
    try{
      if(size<2097152||size>8388608){rescueState.refusal=4;}else{
      retained=packs.retainBytes(size);
      const identities=[engineSHA,sceneSHA,manifestSHA,saved.engine,saved.scene,saved.manifest,saved.snapshot,await rescueHash(saved.bytes)];
      if(identities.some(v=>typeof v!=='string'||v.length!==64)||![saved.layout,saved.size,saved.tier].every(Number.isSafeInteger)){rescueState.refusal=2;}else{
      new Uint8Array(memory.buffer,x.sw_config_ptr(),512).set(new TextEncoder().encode(identities.join('')));
      const admitted=Number(x.sw_rescue_check(BigInt(saved.layout??0),BigInt(size),BigInt(saved.size??0),BigInt(tier),BigInt(saved.tier??-1)));
      if(admitted!==1){rescueState.refusal=admitted;}else{
        memory.grow((size-memory.buffer.byteLength)/65536);new Uint8Array(memory.buffer).set(new Uint8Array(saved.bytes));
        // The page has no old device or live JS callbacks. Replace only GPU
        // handles; malloc/static pointers retain their exact saved addresses.
        if(!x.sw_loss_open()||!x.sw_host_recover())throw Error('restored GPU owner rebinding refused');
        observedLive=info(14);for(let i=0;i<x.sw_host_trace_count();i++)observedLive+=Number(x.sw_host_trace(i,0))?Number(x.sw_host_trace(i,5)):-Number(x.sw_host_trace(i,5));
        restoredFrames=0;rescueState.restored=true;rescueState.restoredDigest=rescueDigest();rescueState.restoredTick=info(0);
      }
      }}
    }catch(error){throw Error(`rescue restore failed: ${error}`);}finally{retained?.release();}
  }
  function holdFrame(){
    if(!rescuable)return;
    if(restoredFrames>=0)restoredFrames++;
    const size=memory.buffer.byteLength;if(size>2097152&&heldEvery++%30)return;
    if(!held||held.bytes.byteLength!==size){heldOwner?.release();heldOwner=packs.retainBytes(size);held={bytes:new ArrayBuffer(size)};}
    const digest=rescueDigest();new Uint8Array(held.bytes).set(new Uint8Array(memory.buffer));
    held.digest=digest;held.tick=info(0);held.size=size;
  }
  function saveStopped(){
    if(!rescuable||rescueSaving)return;
    rescueState.suppressed=!x.sw_rescue_save_allowed(BigInt(restoredFrames));
    const record=!rescueState.suppressed&&held?{...held,layout:1,tier,engine:engineSHA,scene:sceneSHA,manifest:manifestSHA}:null;
    rescueState.heldDigest=record?.digest??null;rescueState.heldTick=record?.tick??null;
    rescueSaving=(async()=>{if(!record){await rescueStore.write(null);return false;}record.snapshot=await rescueHash(record.bytes);return rescueStore.write(record);})().then(saved=>{rescueState.saved=saved;return saved;},error=>{rescueState.saved=false;rescueState.error=String(error);return false;});
    rescuePanel=showRescueStop(rescueSaving);
  }
  const lossInfo=f=>automatic?Number(x.sw_loss_info(f)):0,wallNow=()=>BigInt(Math.floor(performance.now()));
  async function makeSurface(check=()=>{}){
    const created=[],tokens=[];let candidate;
    try{
      candidate=await adapter.requestDevice();check();device=candidate;
      candidate.addEventListener('uncapturederror',e=>errors.push(e.error.message));
      if(automatic)candidate.lost.then(info=>deviceLost(candidate,info));
      const shader=device.createShaderModule({code:shaderSource}),hudShader=device.createShaderModule({code:hudShaderSource}),presentShader=device.createShaderModule({code:presentShaderSource}),diagnostics=await shader.getCompilationInfo(),hudDiagnostics=await hudShader.getCompilationInfo(),presentDiagnostics=await presentShader.getCompilationInfo();
      check();
      if(diagnostics.messages.some(m=>m.type==='error'))throw Error(diagnostics.messages.map(m=>m.message).join('\n'));
      if(hudDiagnostics.messages.some(m=>m.type==='error'))throw Error(hudDiagnostics.messages.map(m=>m.message).join('\n'));
      if(presentDiagnostics.messages.some(m=>m.type==='error'))throw Error(presentDiagnostics.messages.map(m=>m.message).join('\n'));
      context.configure({device,format,usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_DST,alphaMode:'opaque'});
      for(const [index,kind,words] of [[0,1,1024n],[1,3,BigInt(canvas.width*canvas.height)],[2,3,BigInt(canvas.width*canvas.height)],[6,4,24576n],[7,1,64n],[8,3,BigInt(canvas.width*canvas.height*2)]]){if(!x.sw_host_fixed(index,kind,words))throw Error('Coax refused persistent draw storage');tokens.push(index);}
      uniforms=device.createBuffer({size:4096,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});created.push(uniforms);
      depth=device.createTexture({size:[canvas.width,canvas.height],format:'depth32float',usage:GPUTextureUsage.RENDER_ATTACHMENT});created.push(depth);
      color=device.createTexture({size:[canvas.width,canvas.height],format,usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC});created.push(color);
      hdr=device.createTexture({size:[canvas.width,canvas.height],format:'rgba16float',usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.TEXTURE_BINDING});created.push(hdr);
      pipeline=await device.createRenderPipelineAsync({layout:'auto',vertex:{module:shader,entryPoint:'sd_vs',buffers:[{arrayStride:48,attributes:[{shaderLocation:0,offset:0,format:'float32x3'},{shaderLocation:1,offset:12,format:'float32x3'},{shaderLocation:2,offset:24,format:'float32x2'}]}]},fragment:{module:shader,entryPoint:'sd_fs',targets:[{format:'rgba16float'}]},primitive:{topology:'triangle-list',cullMode:'none'},depthStencil:{format:'depth32float',depthWriteEnabled:true,depthCompare:'less'}});
      check();
      sampler=device.createSampler({magFilter:'nearest',minFilter:'nearest',mipmapFilter:'nearest'});
      hudVertices=device.createBuffer({size:98304,usage:GPUBufferUsage.VERTEX|GPUBufferUsage.COPY_DST});created.push(hudVertices);
      hudUniforms=device.createBuffer({size:256,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});created.push(hudUniforms);
      device.queue.writeBuffer(hudUniforms,0,new Float32Array([canvas.width,canvas.height,0,0]));
      hudPipeline=await device.createRenderPipelineAsync({layout:'auto',vertex:{module:hudShader,entryPoint:'sh_vs',buffers:[{arrayStride:48,attributes:[{shaderLocation:0,offset:0,format:'float32x4'},{shaderLocation:1,offset:16,format:'float32x4'},{shaderLocation:2,offset:32,format:'float32x4'}]}]},fragment:{module:hudShader,entryPoint:'sh_fs',targets:[{format:'rgba16float',blend:{color:{operation:'add',srcFactor:'one',dstFactor:'one-minus-src-alpha'},alpha:{operation:'add',srcFactor:'one',dstFactor:'one-minus-src-alpha'}}}]},primitive:{topology:'triangle-list',cullMode:'none'},depthStencil:{format:'depth32float',depthWriteEnabled:false,depthCompare:'always'}});
      check();
      presentPipeline=await device.createRenderPipelineAsync({layout:'auto',vertex:{module:presentShader,entryPoint:'sp_vs'},fragment:{module:presentShader,entryPoint:'sp_fs',targets:[{format}]},primitive:{topology:'triangle-list'}});
      check();surfaceLive=true;
    }catch(error){for(const object of created)object.destroy();for(const token of tokens)x.sw_host_fixed_release(token);context.unconfigure();if(candidate){retiredDevices.add(candidate);candidate.destroy();}throw error;}
  }
  await makeSurface();startup.push(()=>{destroySurface();for(const i of [0,1,2,6,7,8])x.sw_host_fixed_release(i);});
  if(earlyLoss)throw Error('GPU lost during startup');initialized=true;
  const stages=[],recentFailures=[];
  // WebGPU error scopes are a device-wide stack. Effects sharing that
  // stack must finish/pop before another owner pushes its scope.
  function gpuTask(effect){const task=gpuTail.then(effect);gpuTail=task.catch(()=>{});return task;}
  const current=k=>!closed&&x.sw_host_row(k.owner,10)===k.generation&&row(k.owner,8)===1&&row(k.owner,9)===1;
  function upload(k,replacing=false){return gpuTask(()=>uploadNow(k,replacing));}
  async function uploadNow(k,replacing){
    if(automatic&&!replacing&&(lossInfo(0)||lossInfo(1)))throw Error('load interrupted by device loss');
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
  async function load(k,resuming=false){
    const started=performance.now(),stage={owner:k.owner,generation:String(k.generation),epoch:String(k.epoch),source:scene.resources[k.owner].sha256};
    const owner={...k},allocation=resuming?x.sw_rescue_allocated(k.owner):x.sw_host_allocate(k.owner,k.generation,k.epoch);let status=1;
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
  function launch(k,resuming=false){let task;task=load(k,resuming).finally(()=>pending.delete(task));pending.add(task);}
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
      const encoder=device.createCommandEncoder(),pass=encoder.beginRenderPass({colorAttachments:[{view:hdr.createView(),clearValue:{r:.03,g:.05,b:.08,a:1},loadOp:'clear',storeOp:'store'}],depthStencilAttachment:{view:depth.createView(),depthClearValue:1,depthLoadOp:'clear',depthStoreOp:'store'}});pass.setPipeline(pipeline);
      for(let i=0;i<commands.length;i++){
        const [geometry,indices,texture,lighting,cell,n,fallback]=commands[i];const address=x.sw_host_uniforms(cell,canvas.width/canvas.height);if(!address)throw Error('camera/placement uniform refused');
        device.queue.writeBuffer(uniforms,i*256,new Uint8Array(memory.buffer,address,256));
        const group=device.createBindGroup({layout:pipeline.getBindGroupLayout(0),entries:[{binding:0,resource:{buffer:uniforms,offset:i*256,size:256}},{binding:1,resource:gpu.get(texture).object.createView()},{binding:2,resource:gpu.get(lighting).object.createView()},{binding:3,resource:sampler}]});
        pass.setBindGroup(0,group);pass.setVertexBuffer(0,gpu.get(geometry).object);pass.setIndexBuffer(gpu.get(indices).object,'uint32');pass.drawIndexed(n);drawnCommands++;fallbackDraws+=fallback;
      }
      const hudCount=x.sw_host_hud(canvas.width,canvas.height);if(hudCount<=0)throw Error('persistent Coax HUD refused');
      device.queue.writeBuffer(hudVertices,0,new Uint8Array(memory.buffer,x.sw_host_hud_pointer(),hudCount*48));
      pass.setPipeline(hudPipeline);pass.setBindGroup(0,device.createBindGroup({layout:hudPipeline.getBindGroupLayout(0),entries:[{binding:0,resource:{buffer:hudUniforms,offset:0,size:256}}]}));pass.setVertexBuffer(0,hudVertices);pass.draw(hudCount);hudDraws++;lastHudVertices=hudCount;
      pass.end();const display=encoder.beginRenderPass({colorAttachments:[{view:color.createView(),loadOp:'clear',storeOp:'store'}]});display.setPipeline(presentPipeline);display.setBindGroup(0,device.createBindGroup({layout:presentPipeline.getBindGroupLayout(0),entries:[{binding:0,resource:{buffer:hudUniforms,offset:0,size:256}},{binding:1,resource:hdr.createView()},{binding:2,resource:sampler}]}));display.draw(3);display.end();
      encoder.copyTextureToTexture({texture:color},{texture:context.getCurrentTexture()},[canvas.width,canvas.height]);device.queue.submit([encoder.finish()]);await device.queue.onSubmittedWorkDone();
    }finally{for(const owner of borrowed)if(!x.sw_host_return(owner))throw Error('final draw borrow return refused');const validation=await device.popErrorScope();if(validation)throw Error(validation.message);}
  }
  async function step(ns,direction=0,closing=false){
    if(automatic&&!closing){if(recovery)await recovery;if(lossInfo(1))throw Error('GPU recovery stopped; retained CPU owners await close');}
    if(closed||framing)throw Error('frame is closed or already running');framing=true;const start=performance.now();
    try{
      if(!closing){holdFrame();if(trapArmed){trapArmed=false;x.sw_rescue_trap();}}
      if(!x.sw_host_frame(BigInt(ns),direction))throw Error('paid frame refused');
      const stepDigests=Array.from({length:x.sw_host_step_count()},(_,step)=>Array.from({length:8},(_,word)=>Number(x.sw_host_step_word(step,word)).toString(16).padStart(8,'0')).join(''));
      const loads=Array.from({length:info(1)},(_,i)=>command(0,i)),retiring=Array.from({length:info(2)},(_,i)=>command(1,i));
      const records=Array.from({length:info(3)},(_,i)=>Array.from({length:23},(_,f)=>String(x.sw_host_record(i,f))));
      onRecord?.(records);x.sw_host_ack();
      for(const transfer of transfers.values())if(!current(transfer.key))packs.abortReads(transfer.owner);
      for(const k of retiring)await retire(k);for(const k of loads)launch(k);
      let presentationInterrupted=false;
      if(surfaceLive){try{await render();}catch(error){if(!automatic||(!lossInfo(0)&&!lossInfo(1)))throw error;presentationInterrupted=true;}}
      if(errors.length)throw Error(errors.join('\n'));
      const traceCount=x.sw_host_trace_count();if(traceCount<0)throw Error('physical allocation trace overflow');
      const allocations=Array.from({length:traceCount},(_,i)=>Array.from({length:6},(_,f)=>Number(x.sw_host_trace(i,f))));x.sw_host_trace_ack();
      for(const [freed,owner,generation,epoch,field,bytes] of allocations)observedLive+=freed?-bytes:bytes;
      const cpu=info(14),gpuBytes=[...gpu.values()].reduce((n,r)=>n+r.bytes,0);
      if(observedLive!==cpu||cpu>info(10)||cpu+gpuBytes>info(8))throw Error('physical allocations disagree with original tickets/declared peak');
      return {frameNs:ns,direction,tick:info(0),position:Array.from({length:3},(_,i)=>x.sw_host_player(i)),active:info(4),drawn:info(5),updated:info(6),declared:info(8),cpu,gpuBytes,allocations,stages:stages.splice(0),records,stepDigests,frameMs:performance.now()-start,presentationInterrupted};
    }catch(error){if(rescuable&&!closing&&!lossInfo(0)&&!lossInfo(1)){x.sw_rescue_stop(wallNow());stopRecovery(error);}throw error;}finally{framing=false;}
  }
  async function close(){
    if(recovery)await recovery;if(replacementWork)await replacementWork.catch(()=>{});if(rescueSaving)await rescueSaving;
    if(closed||framing||!x.sw_host_close())throw Error('close refused');
    for(const transfer of transfers.values())packs.abortReads(transfer.owner);
    await Promise.all([...pending]);
    const closingFrames=[];for(let i=0;i<128&&!info(24);i++)closingFrames.push(await step(15625000,0,true));
    if(!info(24)||info(14)||info(10)||info(22)||info(23))throw Error('physical resources did not drain');
    if(surfaceLive)await device.queue.onSubmittedWorkDone();destroySurface();for(const i of [0,1,2,6,7,8])if(fixedLive(i)&&!x.sw_host_fixed_release(i))throw Error('fixed ledger cleanup refused');
    const result={info:Array.from({length:28},(_,f)=>info(f)),closingFrames,loadFailures,drawnCommands,fallbackDraws,recovery:deviceStatus(),hud:{draws:hudDraws,vertices:lastHudVertices,info:Array.from({length:5},(_,f)=>Number(x.sw_host_hud_info(f)))},wasmBytesBeforeDispose:memory.buffer.byteLength,wasmReservedBytes:arena.bytes,transportBeforeDispose:packs.info(),gpuErrors:errors.slice()};
    if(info(12)||info(13))throw Error('GPU ledger did not drain');closed=true;context.unconfigure();device.destroy();heldOwner?.release();heldOwner=null;held=null;rescueStore?.close();rescuePanel?.();new Uint8Array(memory.buffer).fill(0);x=null;memory=null;arena.release();result.transport=packs.info();finalSnapshot=result;return result;
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
  function fixedLive(index){return !automatic||Number(x.sw_host_fixed_live(index))===1;}
  function destroySurface(){
    if(device){retiredDevices.add(device);device.destroy();}
    for(const resource of gpu.values())resource.object.destroy();gpu.clear();
    if(surfaceLive){uniforms.destroy();hudVertices.destroy();hudUniforms.destroy();depth.destroy();color.destroy();hdr.destroy();surfaceLive=false;context.unconfigure();}
  }
  let stopWake=null;
  function stopRecovery(error){
    recoveryError=String(error);destroySurface();stopWake?.();saveStopped();
  }
  function deviceStatus(){
    if(closed)return finalSnapshot.recovery;
    return {state:lossInfo(1)?'stopped':lossInfo(0)?'recovering':lastRecovery?'recovered':'ready',serial:lossInfo(2),generation:automatic?String(x.sw_loss_info(2)):'0',losses:lossInfo(3),began:lossInfo(4),reason:lossInfo(5),last:lossInfo(6),lastRecovery,error:recoveryError,retainedCPU:info(14)};
  }
  function rescueDigest(){const at=x.sw_host_rescue_digest?.();if(!at)throw Error('checkpoint observer unavailable');return new TextDecoder().decode(new Uint8Array(memory.buffer,at,64));}
  function stateDigest(){const at=x.sw_host_device_digest?.();if(!at)throw Error('canonical device observer unavailable');return new TextDecoder().decode(new Uint8Array(memory.buffer,at,64));}
  function deviceLost(lost,reason){
    if(closed||retiredDevices.has(lost)||lost!==device)return;
    if(!initialized){earlyLoss=true;return;}
    const status=Number(x.sw_loss_lost(wallNow()));
    if(status!==1){stopRecovery(`device loss refused (${status}, reason ${lossInfo(5)})`);return;}
    if(!recovery)recovery=replaceAutomatically(reason).finally(()=>{recovery=null;});
  }
  async function replaceAutomatically(reason){
    let timer;
    const stopped=new Promise(resolve=>{stopWake=resolve;const deadline=()=>{if(Number(x.sw_loss_poll(wallNow(),x.sw_loss_info(2)))===1){timer=setTimeout(deadline,1);return;}stopRecovery('GPU replacement deadline');};timer=setTimeout(deadline,Math.max(0,Math.ceil(lossInfo(4)+10000-performance.now())));});
    const loop=(async()=>{
      while(!lossInfo(1)){
        const serial=x.sw_loss_info(2),started=performance.now();
        const check=()=>{if(Number(x.sw_loss_poll(wallNow(),serial))!==1)throw Error('GPU replacement attempt is stale or stopped');};
        try{
          replacementWork=(async()=>{
            while(framing){await new Promise(resolve=>setTimeout(resolve,0));check();}
            for(const transfer of transfers.values())packs.abortReads(transfer.owner);
            await Promise.all([...pending]);await gpuTail;check();
            const before=Array.from({length:28},(_,f)=>info(f));destroySurface();
            if(!x.sw_host_recover())throw Error(`Coax refused GPU replacement:${x.sw_host_recover_info(0)}`);
            adapter=await navigator.gpu.requestAdapter({powerPreference:'high-performance'});check();
            if(!adapter)throw Error('replacement WebGPU adapter unavailable');
            await makeSurface(check);let uploaded=0;
            for(let owner=0;owner<64;owner++)if(x.sw_host_uploadable(owner)){check();await upload({owner,generation:x.sw_host_row(owner,10)},true);uploaded++;}
            check();await render();check();
            return {before,after:Array.from({length:28},(_,f)=>info(f)),epoch:Number(x.sw_host_recover_info(2)),granted:Number(x.sw_host_recover_info(1)),uploaded,lossReason:reason.reason,adapter:{vendor:adapter.info.vendor,architecture:adapter.info.architecture,device:adapter.info.device,description:adapter.info.description},ms:performance.now()-started};
          })();
          const result=await replacementWork;
          if(Number(x.sw_loss_finish(wallNow(),serial,1))===2){lastRecovery=result;return;}
        }catch(error){
          if(x.sw_loss_info(2)!==serial&&!lossInfo(1))continue;
          x.sw_loss_finish(wallNow(),serial,0);stopRecovery(error);return;
        }
      }
    })();
    try{await Promise.race([loop,stopped]);}finally{clearTimeout(timer);stopWake=null;}
  }
  // Explicit observation remains available; the automatic carrier routes
  // it through the same real device-loss callback as unsolicited loss.
  async function recover(){
    if(automatic){if(closed||framing||recovery||lossInfo(1))throw Error('replacement unavailable');device.destroy();await device.lost;if(recovery)await recovery;if(lossInfo(1))throw Error(recoveryError);return lastRecovery;}
    if(closed||framing||!x.sw_host_recover)throw Error('replacement unavailable or frame still active');framing=true;const started=performance.now();
    try{
      for(const transfer of transfers.values())packs.abortReads(transfer.owner);await Promise.all([...pending]);await device.queue.onSubmittedWorkDone();
      const before=Array.from({length:28},(_,f)=>info(f)),lostDevice=device;
      retiredDevices.add(lostDevice);lostDevice.destroy();const loss=await lostDevice.lost;
      for(const resource of gpu.values())resource.object.destroy();gpu.clear();uniforms.destroy();hudVertices.destroy();hudUniforms.destroy();depth.destroy();color.destroy();hdr.destroy();context.unconfigure();
      if(!x.sw_host_recover())throw Error(`Coax refused GPU replacement:${x.sw_host_recover_info(0)}`);
      adapter=await navigator.gpu.requestAdapter({powerPreference:'high-performance'});if(!adapter)throw Error('replacement WebGPU adapter unavailable');
      await makeSurface();let uploaded=0;
      for(let owner=0;owner<64;owner++)if(x.sw_host_uploadable(owner)){await upload({owner,generation:x.sw_host_row(owner,10)});uploaded++;}
      await render();if(errors.length)throw Error(errors.join('\n'));
      return {before,after:Array.from({length:28},(_,f)=>info(f)),epoch:Number(x.sw_host_recover_info(2)),granted:Number(x.sw_host_recover_info(1)),uploaded,lossReason:loss.reason,adapter:{vendor:adapter.info.vendor,architecture:adapter.info.architecture,device:adapter.info.device,description:adapter.info.description},ms:performance.now()-started};
    }finally{framing=false;}
  }
  if(rescueState.restored){
    for(let owner=0;owner<64;owner++)if(x.sw_host_uploadable(owner))await upload({owner,generation:x.sw_host_row(owner,10)},true);
    await render();
    for(let owner=0;owner<64;owner++)if(x.sw_rescue_object(owner,0)&&!x.sw_rescue_object(owner,1))launch({owner,generation:x.sw_rescue_object(owner,2),epoch:x.sw_rescue_object(owner,3)},true);
  }
  return Object.freeze({rescueDigest,rescueStatus:()=>({...rescueState,restoredFrames,heldBytes:held?.size??0}),rescueSaved:()=>rescueSaving??Promise.resolve(false),trapNextFrame:()=>{if(!rescuable)throw Error('rescue unsupported');trapArmed=true;},step,settle:()=>Promise.all([...pending]),close,capture,recover,loseDevice:()=>device.destroy(),deviceStatus,stateDigest,hud:()=>({draws:hudDraws,vertices:lastHudVertices,info:Array.from({length:5},(_,f)=>Number(x.sw_host_hud_info(f)))}),profile:Object.freeze({tier,manifestSHA,sceneSHA,stateSchema:'aethyr.stream-walk-state.v1',presentationSchema:'aethyr.stream-walk-display.v1'}),cells:()=>Array.from({length:12},(_,i)=>Array.from({length:5},(_,f)=>Number(x.sw_host_cell(i,f)))),stats:()=>closed?finalSnapshot:({info:Array.from({length:28},(_,f)=>info(f)),wasmBytes:memory.buffer.byteLength,wasmReservedBytes:arena.bytes,transport:packs.info(),loadFailures,recentFailures:recentFailures.slice(),drawnCommands,fallbackDraws,gpuErrors:errors.slice(),runtimeMessages:runtimeMessages.slice()}),adapter:{vendor:adapter.info.vendor,architecture:adapter.info.architecture,device:adapter.info.device,description:adapter.info.description,userAgent:navigator.userAgent}});
  }catch(error){for(const dispose of startup.reverse())await dispose();throw error;}
}
