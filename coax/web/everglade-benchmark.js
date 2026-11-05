// Firm reason: 1 (browser measurements and replay of recorded room transport bytes).
// Injected only by the benchmark driver, before the ordinary page scripts.
(() => {
  const options=globalThis.coaxialBenchmarkOptions;
  if(!options)return;
  globalThis.coaxialFrameTrace=true;
  globalThis.coaxialPreferredFrameRate=60;globalThis.coaxialMinimumFrameRate=60;
  Object.defineProperty(globalThis,'devicePixelRatio',{configurable:true,get:()=>1});
  const B=globalThis.__evergladeBenchmark={records:[],gpu:[],submissions:[],input:[],errors:[],adapter:null,timestampSupported:false,readyAt:null,frames:0,workers:0};
  const start=performance.now(),NativeSocket=WebSocket;
  let recordStart=null,replayStart=null;
  addEventListener('error',e=>B.errors.push(String(e.message)));
  addEventListener('unhandledrejection',e=>B.errors.push(String(e.reason)));
  if(options.mode==='record') {
    globalThis.WebSocket=class extends NativeSocket {
      constructor(...args){super(...args);if(String(args[0]).includes('/api/room/relay/'))this.addEventListener('message',e=>{
        recordStart ??= performance.now();B.records.push([Math.round(performance.now()-recordStart),e.data]);
      });}
    };
  } else if(options.mode==='replay') {
    const nativeFetch=fetch.bind(globalThis),api=nativeFetch('/build/bench/api.json').then(r=>{if(!r.ok)throw Error('Missing API fixture');return r.json();});
    globalThis.fetch=(url,...args)=>{
      const u=new URL(typeof url==='string'?url:url.url,location.href);if(!u.pathname.startsWith('/api/'))return nativeFetch(url,...args);
      return api.then(rows=>{const row=rows.find(row=>row.url===u.pathname+u.search),wire=row?.wire||'404\n',cut=wire.indexOf('\n');return new Response(wire.slice(cut+1),{status:Number(wire.slice(0,cut))});});
    };
    const fixture=nativeFetch(options.fixture).then(r=>{if(!r.ok)throw Error('Missing raid fixture');return r.json();});
    globalThis.coaxialNativeRoomHost='ws://127.0.0.1/replay';
    globalThis.WebSocket=class extends EventTarget {
      static CONNECTING=0;static OPEN=1;static CLOSING=2;static CLOSED=3;
      constructor(url){super();this.readyState=0;this.onopen=this.onmessage=this.onerror=this.onclose=null;
        if(!String(url).includes('/api/room/')){this.readyState=3;queueMicrotask(()=>{const e=new Event('close');this.onclose?.(e);this.dispatchEvent(e);});return;}
        fixture.then(records=>{if(this.readyState!==0)return;this.readyState=1;replayStart=performance.now();
          this.onopen?.(new Event('open'));this.dispatchEvent(new Event('open'));let at=0;
          const pump=()=>{while(at<records.length && records[at][0]<=performance.now()-replayStart){const row=records[at++],e=new MessageEvent('message',{data:row[1]});this.onmessage?.(e);this.dispatchEvent(e);}if(this.readyState===1)this.timer=setTimeout(pump,4);};pump();
        }).catch(e=>{B.errors.push(String(e));this.onerror?.(new Event('error'));});
      }
      send(){} // outgoing intents never change a recorded fixture
      close(){this.readyState=3;clearTimeout(this.timer);const e=new Event('close');this.onclose?.(e);this.dispatchEvent(e);}
    };
  }
  const NativeWorker=Worker;
  globalThis.Worker=class extends NativeWorker {constructor(...args){super(...args);B.workers++;}};
  const requestAdapter=navigator.gpu?.requestAdapter.bind(navigator.gpu);
  if(requestAdapter)navigator.gpu.requestAdapter=async(...args)=>{
    const adapter=await requestAdapter(...args);if(!adapter)return adapter;
    B.adapter={vendor:adapter.info?.vendor,architecture:adapter.info?.architecture,device:adapter.info?.device,description:adapter.info?.description};
    const request=adapter.requestDevice.bind(adapter),supported=adapter.features.has('timestamp-query'),instrumented=supported&&options.timestamps!==false;B.timestampSupported=supported;B.timestampInstrumented=instrumented;
    adapter.requestDevice=async(descriptor={})=>{
      const requiredFeatures=[...new Set([...(descriptor.requiredFeatures||[]),...(instrumented?['timestamp-query']:[])])];
      const device=await request({...descriptor,requiredFeatures});
      device.addEventListener('uncapturederror',e=>B.errors.push(String(e.error?.message)));
      const inputDone=queue=>{if(B.inputStart!=null){const began=B.inputStart;B.inputStart=null;queue.onSubmittedWorkDone().then(()=>B.input.push(performance.now()-began));}};
      if(!instrumented){B.submitted=queue=>{B.submissions.push(performance.now()-start);inputDone(queue);};return device;}
      const create=device.createCommandEncoder.bind(device),slots=[];
      let pending=[];
      device.createCommandEncoder=(...args)=>{
        const encoder=create(...args);if(!B.inFrame)return encoder;
        let slot=slots.find(s=>!s.busy);
        if(!slot && slots.length<4){slot={set:device.createQuerySet({type:'timestamp',count:256}),
          resolve:device.createBuffer({size:2048,usage:GPUBufferUsage.QUERY_RESOLVE|GPUBufferUsage.COPY_SRC}),
          read:device.createBuffer({size:2048,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ}),busy:false};slots.push(slot);}
        if(!slot)return encoder;slot.busy=true;
        slot.count=0;const begin=encoder.beginRenderPass.bind(encoder);
        encoder.beginRenderPass=descriptor=>{const index=slot.count++;
          if(index>=128)throw Error('Benchmark render-pass query limit exceeded');
          return begin({...descriptor,timestampWrites:{querySet:slot.set,beginningOfPassWriteIndex:2*index,endOfPassWriteIndex:2*index+1}});
        };
        const finish=encoder.finish.bind(encoder);
        encoder.finish=(...a)=>{if(!slot.count){slot.busy=false;return finish(...a);}
          encoder.resolveQuerySet(slot.set,0,2*slot.count,slot.resolve,0);encoder.copyBufferToBuffer(slot.resolve,0,slot.read,0,16*slot.count);pending.push(slot);return finish(...a);};
        return encoder;
      };
      B.submitted=queue=>{
        B.submissions.push(performance.now()-start);const reads=pending;pending=[];
        for(const slot of reads)slot.read.mapAsync(GPUMapMode.READ).then(()=>{
          const t=new BigUint64Array(slot.read.getMappedRange());let total=0n,first=null,last=0n;
          for(let i=0;i<slot.count;i++)if(t[2*i+1]>=t[2*i] && t[2*i+1]>0n){total+=t[2*i+1]-t[2*i];first=first==null||t[2*i]<first?t[2*i]:first;last=t[2*i+1]>last?t[2*i+1]:last;}
          if(first!=null)B.gpu.push([performance.now()-start,Number(last-first)/1e6,Number(total)/1e6,slot.count]);
          slot.read.unmap();slot.busy=false;
        },e=>{slot.busy=false;B.errors.push(String(e));});
        inputDone(queue);
      };
      return device;
    };return adapter;
  };
  // Give both browsers a 60 Hz callback ceiling; every callback's own cadence is retained.
  const raf=requestAnimationFrame.bind(globalThis),last=new WeakMap();
  globalThis.requestAnimationFrame=f=>raf(function tick(t){
    const previous=last.get(f)??-Infinity;if(t-previous<15){raf(tick);return;}last.set(f,t);
    B.inFrame=true;try{f(t);}finally{B.inFrame=false;}
    if(globalThis.coaxial?.sceneReady){B.readyAt ??= performance.now()-start;B.frames++;}
  });
  B.tap=()=>{B.inputStart=performance.now();dispatchEvent(new KeyboardEvent('keydown',{key:'w',code:'KeyW',bubbles:true}));setTimeout(()=>dispatchEvent(new KeyboardEvent('keyup',{key:'w',code:'KeyW',bubbles:true})),40);};
  B.replaySeconds=()=>replayStart==null?0:(performance.now()-replayStart)/1000;
  B.snapshot=()=>({mode:options.mode,seconds:(performance.now()-start)/1000,replayOffsetMs:replayStart==null?null:replayStart-start,replayStartMs:replayStart,replaySeconds:B.replaySeconds(),
    readyAt:B.readyAt,frames:B.frames,workers:B.workers,gpu:B.gpu.splice(0),submissions:B.submissions.splice(0),input:B.input.splice(0),errors:B.errors.slice(),adapter:B.adapter,timestampSupported:B.timestampSupported,timestampInstrumented:B.timestampInstrumented,
    trace:globalThis.coaxial?.takeFrameTrace?.()||[],net:globalThis.coaxial?.netProbe?.(0),seat:globalThis.coaxial?.netProbe?.(1),
    quality:Array.from({length:9},(_,i)=>globalThis.coaxial?.quality?.(i)),canvas:[document.querySelector('canvas')?.width,document.querySelector('canvas')?.height]});
})();
