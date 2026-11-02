// Calibrate a declared backend profile. This never rewrites a production shader
// or chooses a universal raster rule: it retains every authored raw observation.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const [wgsl, output, casesPath] = process.argv.slice(2);
if (!wgsl || !output || !casesPath) throw Error("usage: check_deadgears_vs_webgpu.ts PROBE.wgsl RESULT.json CASES.json");
const digest = (path:string) => createHash("sha256").update(readFileSync(path)).digest("hex");
const sources = [wgsl, casesPath, import.meta.path, new URL("./render-deadgears-vs-probe.coax", import.meta.url).pathname, new URL("../../coax/web/chrome.ts", import.meta.url).pathname,new URL("./render-deadgears-vs-probe.bindings", import.meta.url).pathname];
const fixtureData=JSON.parse(readFileSync(casesPath,'utf8'));
for(const input of fixtureData.source_inputs){const path=new URL('../../'+input.path,import.meta.url).pathname;if(digest(path)!==input.sha256)throw Error('production source differs from retained package');sources.push(path);}
for(const input of Object.values(fixtureData.resources)as any[]){if(digest(input.file)!==input.sha256)throw Error('original resource changed');sources.push(input.file);}
const inputs = Object.fromEntries(sources.map(path => [path, digest(path)]));
const code = readFileSync(wgsl,"utf8");
const cases=JSON.parse(readFileSync(casesPath,"utf8"));if(cases.schema!==1||cases.cases.length!==96)throw Error("unexpected production-VS fixture");
const resources:any={};for(const[key,item]of Object.entries(cases.resources)as any)resources['/'+key+'.bin']=readFileSync(item.file);
const server = Bun.serve({hostname:'127.0.0.1',port:0,fetch:(r)=>{const data=resources[new URL(r.url).pathname];return data?new Response(data,{headers:{'content-type':'application/octet-stream'}}):new Response('<!doctype html><title>production vertex probe</title>',{headers:{'content-type':'text/html'}});}});
const chrome = await startChrome("aethyr-deadgears-vs-",[],1);let ws:WebSocket|undefined;
try {
 const pages = await (await fetch(`http://127.0.0.1:${chrome.port}/json/list`)).json();
 ws = new WebSocket(pages.find((page:any)=>page.type==="page").webSocketDebuggerUrl);
 let serial=0;const pending=new Map<number,any>();
 ws.onmessage=e=>{const packet=JSON.parse(String(e.data));if(!packet.id)return;const q=pending.get(packet.id);pending.delete(packet.id);clearTimeout(q?.timer);packet.error?q?.reject(packet.error):q?.resolve(packet.result);};
 await new Promise<void>((resolve,reject)=>{ws!.onopen=()=>resolve();ws!.onerror=reject;});
 const call=(method:string,params:any={})=>new Promise<any>((resolve,reject)=>{const id=++serial,timer=setTimeout(()=>{pending.delete(id);reject(Error(`${method} timeout`));},30000);pending.set(id,{resolve,reject,timer});ws!.send(JSON.stringify({id,method,params}));});
 const browser=await call("Browser.getVersion");await call("Page.enable");await call("Page.navigate",{url:`http://127.0.0.1:${server.port}/`});
 const expression=`(async()=>{
  const adapter=await navigator.gpu.requestAdapter();if(!adapter)throw Error('no WebGPU adapter');
  const device=await adapter.requestDevice(),errors=[];device.addEventListener('uncapturederror',e=>errors.push(e.error.message));device.pushErrorScope('validation');
  const module=device.createShaderModule({code:${JSON.stringify(code)}}),messages=(await module.getCompilationInfo()).messages.map(m=>({type:m.type,message:m.message,line:m.lineNum,column:m.linePos}));
  const fixture=${JSON.stringify(cases)};
  const globals=device.createBindGroupLayout({entries:[{binding:0,visibility:GPUShaderStage.VERTEX,buffer:{type:'uniform',minBindingSize:fixture.resources.u.bytes}},{binding:1,visibility:GPUShaderStage.VERTEX|GPUShaderStage.FRAGMENT,buffer:{type:'uniform',minBindingSize:16}}]});
  const storage=device.createBindGroupLayout({entries:[{binding:0,visibility:GPUShaderStage.VERTEX,buffer:{type:'read-only-storage',minBindingSize:64}}]});
  const uniform=device.createBuffer({size:fixture.resources.u.bytes,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});device.queue.writeBuffer(uniform,0,await(await fetch('/u.bin')).arrayBuffer());
  const objects=device.createBuffer({size:fixture.resources.objects.bytes,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST});device.queue.writeBuffer(objects,0,await(await fetch('/objects.bin')).arrayBuffer());
  const probes=device.createBuffer({size:16,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
  const group0=device.createBindGroup({layout:globals,entries:[{binding:0,resource:{buffer:uniform,offset:0,size:fixture.resources.u.bytes}},{binding:1,resource:{buffer:probes,offset:0,size:16}}]});
  const group1=device.createBindGroup({layout:storage,entries:[{binding:0,resource:{buffer:objects,offset:0,size:fixture.resources.objects.bytes}}]});
  const pipeline=await device.createRenderPipelineAsync({layout:device.createPipelineLayout({bindGroupLayouts:[globals,storage]}),vertex:{module,entryPoint:'render_deadgears_vs_probe_vs'},fragment:{module,entryPoint:'render_deadgears_vs_probe_fs',targets:[{format:'rgba32float'}]},primitive:{topology:'triangle-list'}});
  const target=device.createTexture({size:[fixture.width,fixture.height],format:'rgba32float',usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC}),results=[];
  for(const c of fixture.cases){
   device.queue.writeBuffer(probes,0,new Float32Array(c.uniform));
   const read=device.createBuffer({size:256,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ}),encoder=device.createCommandEncoder();
   const pass=encoder.beginRenderPass({colorAttachments:[{view:target.createView(),loadOp:'clear',storeOp:'store',clearValue:{r:-1,g:-1,b:-1,a:-1}}]});
   pass.setPipeline(pipeline);pass.setBindGroup(0,group0);pass.setBindGroup(1,group1);pass.draw(3);pass.end();
   encoder.copyTextureToBuffer({texture:target,origin:[...fixture.pixel,0]},{buffer:read,bytesPerRow:256},[1,1,1]);device.queue.submit([encoder.finish()]);
   await read.mapAsync(GPUMapMode.READ);const rgba=Array.from(new Float32Array(read.getMappedRange(),0,4));results.push({triangle:c.triangle,vertex:c.vertex,instance:c.instance,selector:c.selector,rgba});read.unmap();read.destroy();
  }
  const error=await device.popErrorScope();if(error)errors.push(error.message);const info=adapter.info;
  return {format:'rgba32float',width:fixture.width,height:fixture.height,device:{vendor:info.vendor,architecture:info.architecture,device:info.device,description:info.description},messages,errors,results};
 })()`;
 const response=await call("Runtime.evaluate",{expression,awaitPromise:true,returnByValue:true});if(response.exceptionDetails)throw Error(JSON.stringify(response.exceptionDetails));
 const result=response.result.value;result.browser=browser;result.inputs=inputs;result.source_changes=sources.filter(path=>digest(path)!==inputs[path]);
 writeFileSync(output,JSON.stringify(result,null,2)+"\n");
 if(result.messages.length||result.errors.length||result.source_changes.length||result.results.length!==96||result.results.some((r:any)=>r.rgba.some((v:number)=>!Number.isFinite(v))))throw Error("production VS probe shader/readback failed");
 console.log(JSON.stringify(result.results));
} finally {ws?.close();server.stop(true);chrome.proc.kill();await chrome.proc.exited;}
