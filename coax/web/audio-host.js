// Firm reason: 1 (browser SDK: compile the shared reactor, decode/copy assets, and operate its AudioWorklet).
import {AudioBridge,AudioRefusal} from './audio-bridge.js';
import {openHostServices,openPcmMemory} from './services-host.js';
const wasmURL=new URL('../build/host-audio.wasm',import.meta.url),workletURL=new URL('./audio-worklet.js',import.meta.url);
let compiled,decoder;
async function moduleForAudio(){return compiled??=(async()=>{const r=await fetch(wasmURL,{cache:'no-store'});if(!r.ok)throw Error('audio reactor unavailable');return WebAssembly.compile(await r.arrayBuffer());})().catch(error=>{compiled=null;throw error;});}
function interleave(buffer){const data=new Float32Array(buffer.length*buffer.numberOfChannels);for(let c=0;c<buffer.numberOfChannels;c++){const p=buffer.getChannelData(c);for(let i=0;i<p.length;i++)data[i*buffer.numberOfChannels+c]=p[i];}return data;}
export async function decodeCoaxWav(bytes){decoder??=new AudioBridge(await moduleForAudio(),false);return decoder.decodeWav(bytes);}
export async function openCoaxAudio(context,destination,decode,options={}){
  if(context.sampleRate!==48000)throw Error('host contract v1 requires a 48kHz audio context');
  const services=options.services??await openHostServices(),memory=options.memory??openPcmMemory(services),owned=new Set();
  const hold=(frames,channels)=>{const q=memory.claim(frames,channels);if(!q)throw new AudioRefusal('Coax refused PCM allocation');owned.add(q);return q;};
  const drop=q=>{if(q&&owned.delete(q))q.release();};
  const module=await moduleForAudio(),parser=new AudioBridge(module,false);await context.audioWorklet.addModule(workletURL);
  const node=new AudioWorkletNode(context,'aethyr-coax-audio-v1',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{module}});
  node.connect(destination);const pending=new Map(),assets=new Map(),physical=new Set(),loads=new Set();let serial=0n,closed=false,tail=Promise.resolve(),readyResolve,readyReject,failure=null,retirement=null,allocationRefusals=0;
  const ready=new Promise((ok,no)=>{readyResolve=ok;readyReject=no;});
  ready.catch(()=>{});
  const fail=error=>{closed=true;failure=error;node.disconnect();readyReject(error);for(const q of pending.values())q.no(error);pending.clear();};
  node.onprocessorerror=()=>fail(Error('audio processor error'));
  node.port.onmessage=event=>{const m=event.data;if(m.ready){readyResolve();return;}const q=pending.get(m.id);if(q){pending.delete(m.id);m.error?q.no(m.refused?new AudioRefusal(m.error):Error(m.error)):q.ok(m.result);}else if(m.error)fail(Error(m.error));};
  const request=(op,values,transfer=[],closing=false)=>new Promise((ok,no)=>{if(closed&&!closing){no(Error('audio engine retired'));return;}const id=++serial;pending.set(id,{ok,no});try{node.port.postMessage({id,op,...values},transfer);}catch(error){pending.delete(id);no(error);}});
  async function sourceBytes(provided,url,limit){
    if(provided){if(parser.sourceOffer(0,provided.length,limit)<0)throw new AudioRefusal('Coax refused audio source bytes');const lease=hold(Math.ceil(provided.length/4),1);try{return {bytes:provided.slice(),lease};}catch(error){drop(lease);throw error;}}
    const controller=new AbortController(),chunks=[];loads.add(controller);let reader,lease=null;
    try{const response=await fetch(url,{signal:controller.signal});if(!response.ok||!response.body)throw new AudioRefusal('Audio source unavailable');reader=response.body.getReader();let count=0;
      while(true){const q=await reader.read();if(q.done)break;if(!q.value.length)continue;const next=parser.sourceOffer(count,q.value.length,limit);if(next<0)throw new AudioRefusal('Coax refused complete audio source');
        const part=hold(Math.ceil(q.value.length/4),1);try{chunks.push({bytes:q.value.slice(),lease:part});}catch(error){drop(part);throw error;}count=next;
      }
      lease=hold(Math.ceil(count/4),1);const bytes=new Uint8Array(count);let at=0;for(const chunk of chunks){bytes.set(chunk.bytes,at);at+=chunk.bytes.length;chunk.bytes=null;drop(chunk.lease);}chunks.length=0;return {bytes,lease};
    }catch(error){drop(lease);controller.abort();throw error instanceof AudioRefusal?error:new AudioRefusal('Audio source read refused');}
    finally{for(const chunk of chunks){chunk.bytes=null;drop(chunk.lease);}chunks.length=0;reader?.releaseLock();loads.delete(controller);}
  }
  async function install(name,buffer){
    let transit,input,output,source=null,companion=null,decoded=null,decodedLease=null,encoded=null,encodedLease=null,submitted=false,acknowledged=false,keepOutput=false;const extra=[];
    try{
      if(buffer.opus||buffer.url){
        const asset=parser.asset(name);source=await sourceBytes(buffer.opus,asset.pinned?asset.url:buffer.url,asset.pinned?asset.bytes:8388608);if(closed)throw new AudioRefusal('Audio owner retired');
        const scratch=hold(Math.ceil(source.bytes.length/4),1);let plan;try{plan=parser.planOpus(name,source.bytes);}finally{drop(scratch);}
        if(asset.canonical){
          companion=await sourceBytes(buffer.companion,asset.companionURL,asset.companionBytes);if(closed)throw new AudioRefusal('Audio owner retired');
          input=hold(Math.ceil(source.bytes.length/4),1);extra.push(hold(Math.ceil(companion.bytes.length/4),1));output=hold(plan.frames,plan.channels);
          submitted=true;await request('install-pcm',{name,nameBytes:parser.encoder.encode(name),source:source.bytes,companion:companion.bytes,plan},[source.bytes.buffer,companion.bytes.buffer]);acknowledged=true;
        }else{
          encodedLease=hold(Math.ceil(source.bytes.length/4),1);decodedLease=hold(plan.decodeFrames,plan.channels);encoded=source.bytes.slice();
          try{decoded=await context.decodeAudioData(encoded.buffer);}catch(error){throw new AudioRefusal('SDK audio decode refused');}finally{encoded=null;drop(encodedLease);encodedLease=null;}
          if(closed)throw new AudioRefusal('Audio owner retired');if(!parser.outputMatches(decoded))throw new AudioRefusal('Coax refused SDK output shape');
          transit=hold(plan.frames,plan.channels);output=hold(plan.frames,plan.channels);const samples=interleave(decoded);submitted=true;await request('install',{name,samples,channels:plan.channels},[samples.buffer]);acknowledged=true;
        }
      }else if(buffer.wav){
        const words=Math.ceil(buffer.wav.length/4),scratch=hold(words,1);let plan;
        try{plan=parser.planWav(buffer.wav);}finally{drop(scratch);}
        transit=hold(words,1);input=hold(words,1);output=hold(plan.frames,plan.channels);
        const wav=buffer.wav.slice();submitted=true;await request('install-wav',{name,wav,plan},[wav.buffer]);acknowledged=true;
      }else{
        const channels=buffer.channels??buffer.numberOfChannels,frames=buffer.samples?buffer.samples.length/channels:buffer.length;
        if(!Number.isInteger(frames))throw new AudioRefusal('Coax refused incomplete PCM frames');
        transit=hold(frames,channels);output=hold(frames,channels);
        const samples=buffer.samples?buffer.samples.slice():interleave(buffer);submitted=true;await request('install',{name,samples,channels},[samples.buffer]);acknowledged=true;
      }
      if(closed){keepOutput=true;return -1;}assets.set(name,output);output=null;return 1;
    }catch(error){if(error instanceof AudioRefusal)acknowledged=true;else if(submitted)keepOutput=true;throw error;}
    finally{decoded=null;drop(decodedLease);encoded=null;drop(encodedLease);if(!submitted||acknowledged){drop(transit);drop(input);for(const q of extra)drop(q);if(source){source.bytes=null;drop(source.lease);}if(companion){companion.bytes=null;drop(companion.lease);}}if(!keepOutput)drop(output);}
  }
  async function command(line,started){
    await ready;if(closed)return -1;const effect=parser.parse(line);
    if(effect.name&&!assets.has(effect.name)){
      try{const buffer=await decode(effect.name,bytes=>({wav:bytes}),parser.asset(effect.name));if(closed)return -1;if(!buffer)return -1;const task=install(effect.name,buffer);physical.add(task);try{if(await task<0)return -1;}finally{physical.delete(task);}}catch(error){if(error instanceof AudioRefusal){allocationRefusals++;return -1;}throw error;}
    }
    const bytes=typeof line==='string'?parser.encoder.encode(line):line;return request('command',{line:bytes,started},[bytes.buffer]);
  }
  return {ready,node,play(line,started,granted=false){const offer=granted?null:services.audioLine(line);if(!granted&&!offer)return Promise.resolve(-1);const result=tail.then(()=>command(granted?line:offer.bytes,started)).finally(()=>offer?.release());tail=result.catch(()=>{});return result;},
    info:async()=>({...await request('info',{}),memoryWords:memory.total(),assetCount:assets.size,allocationRefusals}),
    close(){if(retirement)return retirement;closed=true;memory.retire();node.disconnect();for(const load of loads)load.abort();const before=memory.total();
      retirement=(failure?Promise.reject(failure):request('retire',{},[],true)).then(async report=>{await Promise.allSettled([...physical]);for(const q of [...owned])drop(q);parser.close();return {memoryBefore:before,memoryAfter:memory.total(),nodeBefore:report.before.words,nodeAfter:report.after.words};}).finally(()=>{node.port.close();for(const q of pending.values())q.no(Error('audio engine retired'));pending.clear();readyReject(Error('audio engine retired'));});return retirement;}};
}
export async function renderCoaxAudio(lines,frames,buffers){
  const module=await moduleForAudio(),context=new OfflineAudioContext(2,frames,48000);await context.audioWorklet.addModule(workletURL);
  const assets=Object.entries(buffers).map(([name,buffer])=>({name,samples:interleave(buffer),channels:buffer.numberOfChannels}));
  const encoder=new TextEncoder(),schedule=lines.map(([frame,line,started])=>[frame,encoder.encode(line),started]);
  const node=new AudioWorkletNode(context,'aethyr-coax-audio-v1',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{module,assets,schedule,observe:true}});
  const errors=[];let observation=null;node.port.onmessage=e=>{if(e.data.error)errors.push(e.data.error);if(e.data.observation)observation=e.data.observation;};node.onprocessorerror=()=>errors.push('audio processor error');node.connect(context.destination);
  const output=await context.startRendering();await new Promise(resolve=>setTimeout(resolve,0));node.disconnect();node.port.close();if(errors.length)throw Error(errors.join('\n'));output.coaxWorkletReceipt={...observation,errors:errors.length};return output;
}
