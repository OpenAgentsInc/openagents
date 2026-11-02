// Calibrate a declared backend profile. This never rewrites a production shader
// or chooses a universal raster rule: it retains every authored raw observation.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const [wgsl, output, casesPath] = process.argv.slice(2);
if (!wgsl || !output || !casesPath) throw Error("usage: check_filter_format_webgpu.ts PROBE.wgsl RESULT.json CASES.json");
const digest = (path:string) => createHash("sha256").update(readFileSync(path)).digest("hex");
const sources = [wgsl, casesPath, import.meta.path, new URL("./render-filter-format-probe.coax", import.meta.url).pathname, new URL("../../coax/web/chrome.ts", import.meta.url).pathname,new URL("./render-filter-format-probe.bindings", import.meta.url).pathname];
const inputs = Object.fromEntries(sources.map(path => [path, digest(path)]));
const code = readFileSync(wgsl,"utf8");
const cases = JSON.parse(readFileSync(casesPath,"utf8"));
if(cases.schema!==1 || cases.cases.length!==20 || cases.textures.length!==2)throw Error("unexpected authored probe fixture");
const server = Bun.serve({hostname:"127.0.0.1",port:0,fetch:()=>new Response("<!doctype html><title>raster profile probe</title>",{headers:{"content-type":"text/html"}})});
const chrome = await startChrome("aethyr-filter-format-",[],1);let ws:WebSocket|undefined;
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
  const layout=device.createBindGroupLayout({entries:[{binding:0,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'uniform',minBindingSize:16}},{binding:1,visibility:GPUShaderStage.FRAGMENT,texture:{sampleType:'float'}},{binding:2,visibility:GPUShaderStage.FRAGMENT,sampler:{type:'filtering'}}]});
  const uniform=device.createBuffer({size:16,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
  const target=device.createTexture({size:[64,64],format:'rgba32float',usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC}),results=[];
  const sampler=device.createSampler({magFilter:'linear',minFilter:'linear',mipmapFilter:'linear',addressModeU:'clamp-to-edge',addressModeV:'clamp-to-edge'});
  const pipeline=await device.createRenderPipelineAsync({layout:device.createPipelineLayout({bindGroupLayouts:[layout]}),vertex:{module,entryPoint:'render_filter_format_vs'},fragment:{module,entryPoint:'render_filter_format_fs',targets:[{format:'rgba32float'}]},primitive:{topology:'triangle-list'}});
  const fixture=${JSON.stringify(cases)};
  for(const texture of fixture.textures){
   const image=device.createTexture({size:[fixture.width,fixture.height],mipLevelCount:fixture.levels,format:texture.format,usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST});
   const raw=new Uint8Array(texture.bytes);
   for(let level=0;level<texture.levels.length;level++){const m=texture.levels[level];device.queue.writeTexture({texture:image,mipLevel:level},raw,{offset:m.offset,bytesPerRow:m.row_bytes},[m.width,m.height]);}
   const group=device.createBindGroup({layout,entries:[{binding:0,resource:{buffer:uniform,offset:0,size:16}},{binding:1,resource:image.createView()},{binding:2,resource:sampler}]});
   for(const c of fixture.cases){
    device.queue.writeBuffer(uniform,0,new Float32Array([c.x,c.y,c.lod,0]));
    const read=device.createBuffer({size:256,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ}),encoder=device.createCommandEncoder();
    const pass=encoder.beginRenderPass({colorAttachments:[{view:target.createView(),loadOp:'clear',storeOp:'store',clearValue:{r:0,g:0,b:0,a:0}}]});
    pass.setPipeline(pipeline);pass.setBindGroup(0,group);pass.draw(3);pass.end();
    encoder.copyTextureToBuffer({texture:target,origin:fixture.pixel.concat([0])},{buffer:read,bytesPerRow:256},[1,1,1]);device.queue.submit([encoder.finish()]);
    await read.mapAsync(GPUMapMode.READ);const rgba=Array.from(new Float32Array(read.getMappedRange(),0,4));results.push({format:texture.format,...c,pixel:fixture.pixel,rgba});read.unmap();read.destroy();
   }
   image.destroy();
  }
  const error=await device.popErrorScope();if(error)errors.push(error.message);const info=adapter.info;
  return {format:'rgba32float',width:64,height:64,device:{vendor:info.vendor,architecture:info.architecture,device:info.device,description:info.description},messages,errors,results};
 })()`;
 const response=await call("Runtime.evaluate",{expression,awaitPromise:true,returnByValue:true});if(response.exceptionDetails)throw Error(JSON.stringify(response.exceptionDetails));
 const result=response.result.value;result.browser=browser;result.inputs=inputs;result.source_changes=sources.filter(path=>digest(path)!==inputs[path]);
 writeFileSync(output,JSON.stringify(result,null,2)+"\n");
 if(result.messages.length||result.errors.length||result.source_changes.length||result.results.length!==40||result.results.some((r:any)=>r.rgba.some((v:number)=>!Number.isFinite(v))))throw Error("filter format probe shader/readback failed");
 console.log(JSON.stringify(result.results));
} finally {ws?.close();server.stop(true);chrome.proc.kill();await chrome.proc.exited;}
