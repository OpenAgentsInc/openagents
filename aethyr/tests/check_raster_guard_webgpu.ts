// Calibrate a declared backend profile. This never rewrites a production shader
// or chooses a universal raster rule: it retains every authored raw observation.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const [wgsl, output] = process.argv.slice(2);
if (!wgsl || !output) throw Error("usage: check_raster_webgpu.ts PROBE.wgsl RESULT.json");
const digest = (path:string) => createHash("sha256").update(readFileSync(path)).digest("hex");
const sources = [wgsl, import.meta.path, new URL("./render-raster-guard-probe.coax", import.meta.url).pathname, new URL("../../coax/web/chrome.ts", import.meta.url).pathname];
const inputs = Object.fromEntries(sources.map(path => [path, digest(path)]));
const code = readFileSync(wgsl,"utf8");
const server = Bun.serve({hostname:"127.0.0.1",port:0,fetch:()=>new Response("<!doctype html><title>raster guard-band probe</title>",{headers:{"content-type":"text/html"}})});
const chrome = await startChrome("aethyr-raster-guard-profile-",[],1);let ws:WebSocket|undefined;
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
  const layout=device.createBindGroupLayout({entries:[{binding:0,visibility:GPUShaderStage.VERTEX,buffer:{type:'uniform',minBindingSize:16}}]});
  const uniform=device.createBuffer({size:16,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
  const group=device.createBindGroup({layout,entries:[{binding:0,resource:{buffer:uniform,offset:0,size:16}}]});
  const pipeline=await device.createRenderPipelineAsync({layout:device.createPipelineLayout({bindGroupLayouts:[layout]}),vertex:{module,entryPoint:'render_raster_probe_vs'},fragment:{module,entryPoint:'render_raster_probe_fs',targets:[{format:'rgba32float'}]},primitive:{topology:'triangle-list'}});
  const target=device.createTexture({size:[64,64],format:'rgba32float',usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC}),results=[];
  for(const delta of [0,1/1024,2/1024,3/1024,6/1024,7/1024]){
   device.queue.writeBuffer(uniform,0,new Float32Array([delta,0,0,0]));
   const read=device.createBuffer({size:256,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ}),encoder=device.createCommandEncoder();
   const pass=encoder.beginRenderPass({colorAttachments:[{view:target.createView(),loadOp:'clear',storeOp:'store',clearValue:{r:0,g:0,b:0,a:0}}]});
   pass.setPipeline(pipeline);pass.setBindGroup(0,group);pass.draw(3);pass.end();
   encoder.copyTextureToBuffer({texture:target,origin:[9,9,0]},{buffer:read,bytesPerRow:256},[1,1,1]);device.queue.submit([encoder.finish()]);
   await read.mapAsync(GPUMapMode.READ);const rgba=Array.from(new Float32Array(read.getMappedRange(),0,4));results.push({delta,pixel:[9,9],rgba});read.unmap();read.destroy();
  }
  const error=await device.popErrorScope();if(error)errors.push(error.message);const info=adapter.info;
  return {format:'rgba32float',width:64,height:64,device:{vendor:info.vendor,architecture:info.architecture,device:info.device,description:info.description},messages,errors,results};
 })()`;
 const response=await call("Runtime.evaluate",{expression,awaitPromise:true,returnByValue:true});if(response.exceptionDetails)throw Error(JSON.stringify(response.exceptionDetails));
 const result=response.result.value;result.browser=browser;result.inputs=inputs;result.source_changes=sources.filter(path=>digest(path)!==inputs[path]);
 writeFileSync(output,JSON.stringify(result,null,2)+"\n");
 if(result.messages.length||result.errors.length||result.source_changes.length||result.results.length!==6||result.results.some((r:any)=>r.rgba.some((v:number)=>!Number.isFinite(v))||r.rgba[3]!==1))throw Error("raster probe shader/readback failed");
 console.log(JSON.stringify(result.results));
} finally {ws?.close();server.stop(true);chrome.proc.kill();await chrome.proc.exited;}
