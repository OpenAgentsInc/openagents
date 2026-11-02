// Actual emitted WGSL in an isolated, muted headless Chrome. Test fixture
// buffers/expected pixels are independent of the production filter helper.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";

const [sourcePath, faultPath, arrayFaultPath, output] = process.argv.slice(2);
if (!sourcePath || !faultPath || !arrayFaultPath || !output) throw Error("usage: check_render_sampling_webgpu.ts NORMAL.wgsl FAULT.wgsl ARRAY-FAULT.wgsl RESULT.json");
const source = readFileSync(sourcePath, "utf8"), fault = readFileSync(faultPath, "utf8"), arrayFault = readFileSync(arrayFaultPath, "utf8");
const digest = (path: string) => createHash("sha256").update(readFileSync(path)).digest("hex");
const sourceInputs = Object.fromEntries([sourcePath, faultPath, arrayFaultPath, import.meta.path, new URL("../../coax/web/chrome.ts", import.meta.url).pathname].map(p => [p, digest(p)]));
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response("<!doctype html><title>shader qualification</title>", { headers: { "content-type": "text/html" } }) });
const chrome = await startChrome("render-sampling-", [], 1);
let ws: WebSocket | undefined;
try {
  const pages = await (await fetch(`http://127.0.0.1:${chrome.port}/json/list`)).json();
  ws = new WebSocket(pages.find((p: any) => p.type === "page").webSocketDebuggerUrl);
  const pending = new Map<number, { resolve: (v: any) => void, reject: (e: any) => void, timer: ReturnType<typeof setTimeout> }>();
  let serial = 0;
  ws.onmessage = (event) => { const p = JSON.parse(String(event.data)); if (p.id) { const q = pending.get(p.id); pending.delete(p.id); if (q) clearTimeout(q.timer); if (p.error) q?.reject(p.error); else q?.resolve(p.result); } };
  await new Promise<void>((resolve, reject) => { ws!.onopen = () => resolve(); ws!.onerror = reject; });
  const call = (method: string, params: any = {}) => new Promise<any>((resolve, reject) => { const id = ++serial; const timer = setTimeout(() => { pending.delete(id); reject(Error(`${method} timed out`)); }, 30000); pending.set(id, { resolve, reject, timer }); ws!.send(JSON.stringify({ id, method, params })); });
  const browser = await call("Browser.getVersion");
  await call("Page.enable");
  await call("Page.navigate", { url: `http://127.0.0.1:${server.port}/` });
  await Bun.sleep(250);
  const expression = `(async () => {
    const adapter = await navigator.gpu.requestAdapter(); if (!adapter) throw Error("no WebGPU adapter");
    const device = await adapter.requestDevice(); const errors = [], messages = [];
    device.addEventListener("uncapturederror", e => errors.push(e.error.message));
    const texture = device.createTexture({size:[8,1],format:"rgba16float",usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST});
    const halves = new Uint16Array([0,0,0x3400,0x3c00,0,0x3000,0x3400,0x3c00,0,0x3400,0x3400,0x3c00,0x3c00,0x3600,0x3400,0x3c00,0x3c00,0x3800,0x3400,0x3c00,0,0x3900,0x3400,0x3c00,0,0x3a00,0x3400,0x3c00,0,0x3b00,0x3400,0x3c00]);
    device.queue.writeTexture({texture},halves,{bytesPerRow:64,rowsPerImage:1},[8,1]);
    const layers=device.createTexture({size:[8,1,2],format:"rgba16float",usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST});
    const layerHalves=new Uint16Array(64);layerHalves.set(halves);
    for(let i=0;i<8;i++)layerHalves.set([0x3a00,0x3800,0x3000,0x3c00],32+i*4);
    device.queue.writeTexture({texture:layers},layerHalves,{bytesPerRow:64,rowsPerImage:1},[8,1,2]);
    const sampler=device.createSampler({addressModeU:"repeat",addressModeV:"repeat",minFilter:"linear",magFilter:"linear",mipmapFilter:"linear",maxAnisotropy:1});
    const results=[];
    for (const [text, entry, expected, array] of [[${JSON.stringify(source)},"render_sampling_fs",[.25,.4375,.25,1],false],[${JSON.stringify(source)},"render_sampling_invalid_fs",[.125,.25,.5,.75],false],[${JSON.stringify(source)},"render_sampling_array_fs",[.75,.5,.125,1],true],[${JSON.stringify(source)},"render_sampling_invalid_array_fs",[.125,.25,.5,.75],true],[${JSON.stringify(fault)},"render_sampling_invalid_fs",[.125,.25,.5,.75],false],[${JSON.stringify(arrayFault)},"render_sampling_invalid_array_fs",[.125,.25,.5,.75],true]]) {
      device.pushErrorScope("validation");
      const module=device.createShaderModule({code:text}); const info=await module.getCompilationInfo();
      messages.push(...info.messages.map(m=>({type:m.type,message:m.message,line:m.lineNum,column:m.linePos})));
      if(info.messages.length)return {device:{},messages,errors,results};
      const pipeline=await device.createRenderPipelineAsync({layout:"auto",vertex:{module,entryPoint:"render_sampling_vs"},fragment:{module,entryPoint:entry,targets:[{format:"rgba32float"}]},primitive:{topology:"triangle-list"}});
      const target=device.createTexture({size:[2,2],format:"rgba32float",usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC});
      const readback=device.createBuffer({size:512,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
      const group=device.createBindGroup({layout:pipeline.getBindGroupLayout(0),entries:[{binding:0,resource:array?layers.createView({dimension:"2d-array"}):texture.createView()},{binding:1,resource:sampler}]});
      const encoder=device.createCommandEncoder(); const pass=encoder.beginRenderPass({colorAttachments:[{view:target.createView(),loadOp:"clear",storeOp:"store",clearValue:{r:.125,g:.25,b:.5,a:.75}}]});
      pass.setPipeline(pipeline);pass.setBindGroup(0,group);pass.draw(3);pass.end();
      encoder.copyTextureToBuffer({texture:target},{buffer:readback,bytesPerRow:256,rowsPerImage:2},[2,2]);device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);const data=readback.getMappedRange(), actual=[];
      for(let y=0;y<2;y++)actual.push(...new Float32Array(data,y*256,8));
      readback.unmap();readback.destroy();target.destroy();
      const scoped=await device.popErrorScope();if(scoped)errors.push(scoped.message);
      const wrong=actual.filter((v,i)=>!Number.isFinite(v)||Math.abs(v-expected[i%4])>.00001).length;
      results.push({entry,wrong,actual});
    }
    const info=adapter.info;texture.destroy();layers.destroy();device.destroy();
    return {device:{vendor:info.vendor,architecture:info.architecture,device:info.device,description:info.description},messages,errors,results};
  })()`;
  const response = await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (response.exceptionDetails) throw Error(JSON.stringify(response.exceptionDetails));
  const result = response.result.value;
  result.browser = browser;result.source_inputs = sourceInputs;result.source_changes = Object.keys(sourceInputs).filter(p => digest(p) !== sourceInputs[p]);
  writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
  if (result.messages.length || result.errors.length || result.source_changes.length || result.results.length !== 6 || result.results.slice(0,4).some((r: any) => r.wrong) || result.results[4].wrong !== 16 || result.results[5].wrong !== 16) throw Error("WebGPU shader or pixel/control qualification failed");
  console.log("production eight-tap WGSL, invalid publication and planted control pass with no shader messages/errors");
} finally {
  ws?.close();server.stop(true);chrome.proc.kill();await chrome.proc.exited;
}
