// Firm reason: 1 (browser SDK: real mapped readbacks, explicit encoder failure and actual shared-storage pressure before GPU construction).
import {openStreamWalk} from './stream-walk.js';
import {openPackHost} from '../../../coax/web/pack-host.js';
export async function captureFixture({sceneSHA,transportSHA}){
  const compile=WebAssembly.compile,inputs=[];WebAssembly.compile=async function(bytes){inputs.push(bytes);return compile.call(this,bytes);};
  const scene=await(await fetch('/scene/scene.json')).json(),m=scene.manifests.find(x=>x.backend===0&&x.tier===0),source=await(await fetch('/scene/scene.json')).arrayBuffer(),canvas=document.createElement('canvas');canvas.width=320;canvas.height=180;document.body.append(canvas);
  let app;try{app=await openStreamWalk({canvas,sceneURL:'/scene/scene.json',sceneSHA,sceneBytes:source.byteLength,manifestSHA:m.sha256,transportSHA,tier:0,wasmURL:'/build/stream-walk.wasm',shaderURL:'/build/stream-draw.wgsl'});}finally{WebAssembly.compile=compile;}
  const facts={compileInputsZeroed:inputs.length===2&&inputs.every(b=>new Uint8Array(b).every(v=>v===0))},host=await openPackHost(),frames=[];
  for(let i=0;i<10;i++){frames.push(await app.step(15625000,0));await app.settle();}
  const before=app.stats(),baseline=host.info(),initial=await app.capture(),bytes=initial.rowBytes*initial.height,copyPages=Math.ceil(bytes/65536)*65536,originalSlice=Uint8Array.prototype.slice,originalBtoa=window.btoa,copies=[];
  Uint8Array.prototype.slice=function(...args){const value=originalSlice.apply(this,args);if(this.byteLength===bytes)copies.push(value);return value;};
  let encoderFailed=false,releaseJoin,settled=false;const submitted=GPUQueue.prototype.onSubmittedWorkDone;
  try{window.btoa=()=>{throw Error('explicit diagnostic encoder failure');};GPUQueue.prototype.onSubmittedWorkDone=function(){return submitted.call(this).then(()=>new Promise(resolve=>{releaseJoin=resolve;}));};const pending=app.capture().catch(e=>{encoderFailed=String(e).includes('explicit diagnostic encoder failure');}).finally(()=>{settled=true;});for(let i=0;i<500&&!releaseJoin;i++)await new Promise(r=>setTimeout(r,1));if(!releaseJoin)throw Error('capture did not join its actual GPU work');const held=host.info(),info=app.stats().info;facts.joinRetainsOwners=!settled&&held.storage[0]===baseline.storage[0]+2&&held.storage[1]===baseline.storage[1]+copyPages+Math.ceil(initial.captureCosts.textBudgetBytes/65536)*65536&&info[12]===before.info[12]+1&&info[13]===before.info[13]+bytes/4&&copies.length===1&&copies[0].every(v=>v===0);releaseJoin();await pending;}finally{GPUQueue.prototype.onSubmittedWorkDone=submitted;window.btoa=originalBtoa;Uint8Array.prototype.slice=originalSlice;}
  const afterEncoder=host.info();facts.encoderFailure=encoderFailed&&copies.length===1&&copies[0].every(v=>v===0)&&JSON.stringify(afterEncoder.storage)===JSON.stringify(baseline.storage)&&app.stats().info[12]===before.info[12]&&app.stats().info[13]===before.info[13];
  const held=[];let remaining=268435456-host.info().storage[1]-copyPages;
  while(remaining>0){const size=Math.min(67108864,remaining);held.push(host.retainBytes(size));remaining-=size;}
  const pressured=host.info(),create=GPUDevice.prototype.createBuffer;let creates=0,refused=false;
  GPUDevice.prototype.createBuffer=function(descriptor){creates++;return create.call(this,descriptor);};
  try{try{await app.capture();}catch(e){refused=String(e).includes('Coax refused retained pack storage');}}finally{GPUDevice.prototype.createBuffer=create;}
  facts.preAllocationPressure=refused&&creates===0&&JSON.stringify(host.info().storage)===JSON.stringify(pressured.storage)&&app.stats().info[12]===before.info[12]&&app.stats().info[13]===before.info[13];
  for(const owner of held)owner.release();
  const final=await app.capture();facts.retryPixels=initial.data===final.data;facts.sourceExtents=initial.captureCosts.copyBytes===bytes&&initial.captureCosts.encodedUnits===initial.data.length&&initial.captureCosts.observedLogicalUTF16Peak<=initial.captureCosts.textBudgetBytes;
  const artifacts=app.stats().hostArtifacts,cleanup=await app.close();facts.cleanup=cleanup.transport.storage[0]===1&&cleanup.transport.storage[1]===8388608&&[8,10,11,12,13,14,22,23].every(f=>cleanup.info[f]===0)&&!cleanup.gpuErrors.length;
  const digest=async text=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',Uint8Array.from(atob(text),c=>c.charCodeAt(0))))).map(x=>x.toString(16).padStart(2,'0')).join('');
  return {facts,size:[initial.width,initial.height,initial.rowBytes],capture:initial.captureCosts,pixelSHA:await digest(initial.data),artifacts,readerCompiler:cleanup.transport.compiler,baseline:baseline.storage,pressured:pressured.storage,cleanup:cleanup.transport.storage,frameCount:frames.length};
}
