// Calibrate a declared backend profile. This never rewrites a production shader
// or chooses a universal raster rule: it retains every authored raw observation.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const [wgsl, output, casesPath] = process.argv.slice(2);
if (!wgsl || !output || !casesPath) throw Error("usage: check_ring8_webgpu.ts PROBE.wgsl RESULT.json CASES.json");
const digest = (path:string) => createHash("sha256").update(readFileSync(path)).digest("hex");
const sources = [wgsl,casesPath,import.meta.path,...["./render-ring8-probe.coax","../render/ring8.coax","../../apps/openagents-web/gpu/deadgears.coax","../../verse/gpu/wgsl.coax","../../coax/web/chrome.ts"].map(p=>new URL(p,import.meta.url).pathname)];
const cases=JSON.parse(readFileSync(casesPath,"utf8"));
if(cases.schema!=="aethyr.render.ring8-primitive-cases.v1" || cases.total_cases!==728 || cases.cases.length!==728 || cases.width!==8 || cases.height!==8 || cases.pixel.join(",")!=="1,1")throw Error("unexpected authored ring fixture");
const inputs = Object.fromEntries(sources.map(path => [path, digest(path)]));
const code = readFileSync(wgsl,"utf8");
const server = Bun.serve({hostname:"127.0.0.1",port:0,fetch:()=>new Response("<!doctype html><title>canonical ring probe</title>",{headers:{"content-type":"text/html"}})});
const chrome = await startChrome("aethyr-ring8-",[],1);let ws:WebSocket|undefined;
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
  if(messages.some(m=>m.type==='error'))return {format:'rgba32float',width:8,height:8,device:{},messages,errors,results:[]};
  const layout=device.createBindGroupLayout({entries:[{binding:0,visibility:GPUShaderStage.VERTEX|GPUShaderStage.FRAGMENT,buffer:{type:'uniform',minBindingSize:16}}]});
  const uniform=device.createBuffer({size:16,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
  const group=device.createBindGroup({layout,entries:[{binding:0,resource:{buffer:uniform,offset:0,size:16}}]});
  const pipeline=await device.createRenderPipelineAsync({layout:device.createPipelineLayout({bindGroupLayouts:[layout]}),vertex:{module,entryPoint:'render_ring8_probe_vs'},fragment:{module,entryPoint:'render_ring8_probe_fs',targets:[{format:'rgba32float'}]},primitive:{topology:'triangle-list'}});
  const target=device.createTexture({size:[8,8],format:'rgba32float',usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC}),results=[];
  const fixtures=${JSON.stringify(cases.cases)};
  for(let index=0;index<fixtures.length;index++){
   const row=fixtures[index];device.queue.writeBuffer(uniform,0,new Float32Array(row.uniform));
   const read=device.createBuffer({size:256,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ}),encoder=device.createCommandEncoder();
   const pass=encoder.beginRenderPass({colorAttachments:[{view:target.createView(),loadOp:'clear',storeOp:'store',clearValue:{r:-2,g:-2,b:-2,a:-2}}]});
   pass.setPipeline(pipeline);pass.setBindGroup(0,group);pass.draw(3);pass.end();
   encoder.copyTextureToBuffer({texture:target,origin:[1,1,0]},{buffer:read,bytesPerRow:256},[1,1,1]);device.queue.submit([encoder.finish()]);
   await read.mapAsync(GPUMapMode.READ);const mapped=read.getMappedRange();const rgba=Array.from(new Float32Array(mapped,0,4)),words=Array.from(new Uint32Array(mapped,0,4));results.push({index,rgba,words});read.unmap();read.destroy();
  }
  const error=await device.popErrorScope();if(error)errors.push(error.message);const info=adapter.info;
  return {format:'rgba32float',width:8,height:8,device:{vendor:info.vendor,architecture:info.architecture,device:info.device,description:info.description},messages,errors,results};
 })()`;
 const response=await call("Runtime.evaluate",{expression,awaitPromise:true,returnByValue:true});if(response.exceptionDetails)throw Error(JSON.stringify(response.exceptionDetails));
 const result=response.result.value;result.browser=browser;result.inputs=inputs;result.source_changes=sources.filter(path=>digest(path)!==inputs[path]);
 const discrepancies=[];
 for(let i=0;i<cases.cases.length;i++)for(let k=0;k<4;k++)if(!Number.isFinite(result.results[i]?.rgba[k]) || result.results[i].rgba[k]!==Math.fround(cases.cases[i].expected[k]))discrepancies.push({case:i,channel:k,expected:Math.fround(cases.cases[i].expected[k]),actual:result.results[i]?.rgba[k],word:result.results[i]?.words[k]});
 result.discrepancies=discrepancies;result.comparison=cases.comparison;result.cases_sha256=digest(casesPath);
 writeFileSync(output,JSON.stringify(result,null,2)+"\n");
 if(discrepancies.length||result.messages.length||result.errors.length||result.source_changes.length||result.results.length!==728)throw Error("ring probe shader/readback failed");
 console.log(JSON.stringify({cases:result.results.length,channels:result.results.length*4,discrepancies:discrepancies.length,source_changes:result.source_changes,errors:result.errors}));
} finally {ws?.close();server.stop(true);chrome.proc.kill();await chrome.proc.exited;}
