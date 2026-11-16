// Firm reason: 1 (WebGPU effects for the admitted Coax composition-v1 plan).
// No layout, transform, blur-kernel or admission decisions live here. An app
// reserves BEFORE producing a frame, then submits the shared Coax plan. Image
// bytes are the canonical decoder's straight RGBA; generated shader owns math.
export function uiCompositor(device, format, shader) {
  const module=device.createShaderModule({code:shader});
  const pipeline=device.createRenderPipeline({layout:'auto',vertex:{module,entryPoint:'uc_vs',buffers:[{arrayStride:112,attributes:Array.from({length:7},(_,i)=>({shaderLocation:i,offset:i*16,format:'float32x4'}))}]},fragment:{module,entryPoint:'uc_fs',targets:[{format,blend:{color:{srcFactor:'one',dstFactor:'one-minus-src-alpha'},alpha:{srcFactor:'one',dstFactor:'one-minus-src-alpha'}}}]},primitive:{topology:'triangle-list'}});
  let busy=false,lost=false,targets=[],width=0,height=0;
  device.lost.then(()=>{lost=true;targets=[];});
  function reserve() {if(busy||lost)return false;busy=true;return true;}
  function abort() {busy=false;}
  function submit(plan,vertices,mono,color,images,output,w,h) {
    let uniform;try {
    if(!busy||lost)throw Error('composition reservation required');
    const count=plan.length/8;
    if(!Number.isInteger(count)||!count||count>4096) {abort();throw Error('invalid composition profile');}
    let need=0;
    for(let i=0;i<count;i++)if(plan[i*8]>=0)need=Math.max(need,plan[i*8]+1);
    // The producer has already admitted dimensions, residency and every range
    // in Coax. These guards keep malformed foreign callers out of the SDK.
    if(w<1||h<1||w>4096||h>4096||need>16){abort();throw Error('invalid composition extent');}
    if(w!==width||h!==height){for(const t of targets)t.destroy();targets=[];width=w;height=h;}
    while(targets.length<need)targets.push(device.createTexture({size:[w,h],format,usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.TEXTURE_BINDING}));
    while(targets.length>need)targets.pop().destroy();
    uniform=device.createBuffer({size:16,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});device.queue.writeBuffer(uniform,0,new Float32Array([w,h,0,0]));
    const encoder=device.createCommandEncoder();
    for(let i=0;i<count;i++) {
      const o=i*8,dst=plan[o],src=plan[o+1],first=plan[o+2],n=plan[o+3];
      const source=src<0?color:src>=64?images[src-64]:targets[src];
      if(!source||dst===src||dst< -1||dst>=need||first<0||n<0){uniform.destroy();abort();throw Error('invalid composition resource');}
      const pass=encoder.beginRenderPass({colorAttachments:[{view:dst<0?output:targets[dst].createView(),loadOp:plan[o+4]?'clear':'load',storeOp:'store',clearValue:[0,0,0,0]}]});
      if(n){const bind=device.createBindGroup({layout:pipeline.getBindGroupLayout(0),entries:[{binding:0,resource:{buffer:uniform}},{binding:1,resource:mono.createView()},{binding:2,resource:color.createView()},{binding:3,resource:source.createView()}]});pass.setPipeline(pipeline);pass.setBindGroup(0,bind);pass.setVertexBuffer(0,vertices);pass.draw(n,1,first);}
      pass.end();
    }
    device.queue.submit([encoder.finish()]);
    return device.queue.onSubmittedWorkDone().finally(()=>{uniform.destroy();busy=false;});
    } catch(error) {if(uniform)uniform.destroy();abort();throw error;}
  }
  return {reserve,abort,submit,close(){if(busy)throw Error('composition still in flight');for(const t of targets)t.destroy();targets=[];}};
}
