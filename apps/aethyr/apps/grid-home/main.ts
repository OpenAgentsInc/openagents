/// <reference types="@webgpu/types" />
import { createGpuHost } from "../render-lab/gpu-imports.ts";
declare const __AETHYR_HOME_WASM__: string;
const canvas = document.querySelector<HTMLCanvasElement>("canvas")!;
const native = (value: any) => {
  if(value.kind==="ready") {document.body.dataset.renderer=value.backend??"error";document.body.dataset.rendered=String(value.rendered??false);document.body.dataset.gpu=String(value.gpu??false);}
  (window as any).webkit?.messageHandlers?.gridHome?.postMessage(value);
};
document.getElementById("grid-enter")!.onclick = () => native({kind:"enter"});
for (const event of ["contextmenu", "selectstart", "dragstart"]) document.addEventListener(event, e => e.preventDefault());
let paused = false, frameId = 0, destroyed = false;
let render: ((now:number) => void) | undefined;
(window as any).aethyrHomeLifecycle = (phase:string) => {
  paused = phase !== "foreground";
  cancelAnimationFrame(frameId);
  if (!paused && render && !destroyed) frameId = requestAnimationFrame(render);
};
async function start() {
  const bytes=Uint8Array.from(atob(__AETHYR_HOME_WASM__),c=>c.charCodeAt(0));
  const module=await WebAssembly.compile(bytes);
  const gpu=navigator.gpu;
  const adapter=gpu ? await gpu.requestAdapter() : null;
  if (!adapter) {
    const imports=Object.fromEntries(WebAssembly.Module.imports(module).map(i=>[i.name,()=>{throw Error("GPU unavailable");}]));
    const engine=new WebAssembly.Instance(module,{af_gpu:imports}).exports as any;
    const draw=()=>{
      const scale=Math.min(1,512/Math.max(innerWidth,innerHeight));
      canvas.width=Math.max(1,Math.round(innerWidth*scale));canvas.height=Math.max(1,Math.round(innerHeight*scale));
      const ptr=engine.af_home_still(canvas.width,canvas.height);
      const pixels=new Uint8ClampedArray(engine.memory.buffer,ptr,canvas.width*canvas.height*4);
      canvas.getContext("2d")!.putImageData(new ImageData(pixels,canvas.width,canvas.height),0,0);
      canvas.classList.add("on");
      native({kind:"ready",backend:"Aethyr af-home",rendered:true,gpu:false,width:canvas.width,height:canvas.height});
    };
    draw();addEventListener("resize",draw);return;
  }
  const device = await adapter.requestDevice();
  const context = canvas.getContext("webgpu")!;
  const format = gpu.getPreferredCanvasFormat();
  context.configure({device,format,alphaMode:"opaque"});
  const host = createGpuHost(device,adapter,context,format);
  const instance = new WebAssembly.Instance(module,{af_gpu:host.imports});
  const engine = instance.exports as any;
  host.bind(engine);
  engine.af_home_start();
  const still = matchMedia("(prefers-reduced-motion:reduce)").matches;
  const begin = performance.now(); let last = -100, ready = false;
  render = now => {
    if (paused || destroyed) return;
    // The quiet field needs only 30 fps and a bounded backing texture.
    if (now-last >= 1000/30 || !ready) {
      last=now;
      const scale=Math.min(devicePixelRatio,2,1000/Math.max(innerWidth,innerHeight));
      const width=Math.max(1,Math.round(innerWidth*scale)),height=Math.max(1,Math.round(innerHeight*scale));
      if(canvas.width!==width)canvas.width=width;
      if(canvas.height!==height)canvas.height=height;
      engine.af_home_frame(canvas.width,canvas.height,still ? 20 : 20+(now-begin)/1000);
      if (host.errors.length) throw Error(host.errors.join("; "));
      if (!ready) {
        ready=true; canvas.classList.add("on");
        void device.queue.onSubmittedWorkDone().then(()=>native({kind:"ready",backend:"Aethyr af-home",rendered:host.errors.length===0,gpu:true,errors:host.errors,width:canvas.width,height:canvas.height}));
      }
    }
    if (!still) frameId=requestAnimationFrame(render!);
  };
  frameId=requestAnimationFrame(render);
  addEventListener("resize",()=>{if(still && !paused)render?.(performance.now());});
  addEventListener("pagehide",()=>{destroyed=true;cancelAnimationFrame(frameId);engine.af_home_stop();context.unconfigure();device.destroy();},{once:true});
  document.addEventListener("visibilitychange",()=>(window as any).aethyrHomeLifecycle(document.hidden?"background":"foreground"));
}
start().catch(error=>native({kind:"ready",gpu:false,error:String(error),secure:isSecureContext}));
