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
  node.connect(destination);const pending=new Map(),assets=new Map();let serial=0n,closed=false,tail=Promise.resolve(),readyResolve,readyReject,failure=null,retirement=null,allocationRefusals=0;
  const ready=new Promise((ok,no)=>{readyResolve=ok;readyReject=no;});
  ready.catch(()=>{});
  const fail=error=>{closed=true;failure=error;node.disconnect();readyReject(error);for(const q of pending.values())q.no(error);pending.clear();};
  node.onprocessorerror=()=>fail(Error('audio processor error'));
  node.port.onmessage=event=>{const m=event.data;if(m.ready){readyResolve();return;}const q=pending.get(m.id);if(q){pending.delete(m.id);m.error?q.no(m.refused?new AudioRefusal(m.error):Error(m.error)):q.ok(m.result);}else if(m.error)fail(Error(m.error));};
  const request=(op,values,transfer=[],closing=false)=>new Promise((ok,no)=>{if(closed&&!closing){no(Error('audio engine retired'));return;}const id=++serial;pending.set(id,{ok,no});try{node.port.postMessage({id,op,...values},transfer);}catch(error){pending.delete(id);no(error);}});
  async function install(name,buffer){
    let transit,input,output,submitted=false,acknowledged=false,keepOutput=false;
    try{
      if(buffer.wav){
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
    finally{if(!submitted||acknowledged){drop(transit);drop(input);}if(!keepOutput)drop(output);}
  }
  async function command(line,started){
    await ready;if(closed)return -1;const effect=parser.parse(line);
    if(effect.name&&!assets.has(effect.name)){
      const buffer=await decode(effect.name,bytes=>({wav:bytes}));if(closed)return -1;if(!buffer)return -1;
      try{if(await install(effect.name,buffer)<0)return -1;}catch(error){if(error instanceof AudioRefusal){allocationRefusals++;return -1;}throw error;}
    }
    const bytes=typeof line==='string'?parser.encoder.encode(line):line;return request('command',{line:bytes,started},[bytes.buffer]);
  }
  return {ready,node,play(line,started,granted=false){const offer=granted?null:services.audioLine(line);if(!granted&&!offer)return Promise.resolve(-1);const result=tail.then(()=>command(granted?line:offer.bytes,started)).finally(()=>offer?.release());tail=result.catch(()=>{});return result;},
    info:async()=>({...await request('info',{}),memoryWords:memory.total(),allocationRefusals}),
    close(){if(retirement)return retirement;closed=true;memory.retire();node.disconnect();const before=memory.total();
      retirement=(failure?Promise.reject(failure):request('retire',{},[],true)).then(report=>{for(const q of [...owned])drop(q);parser.close();return {memoryBefore:before,memoryAfter:memory.total(),nodeBefore:report.before.words,nodeAfter:report.after.words};}).finally(()=>{node.port.close();for(const q of pending.values())q.no(Error('audio engine retired'));pending.clear();readyReject(Error('audio engine retired'));});return retirement;}};
}
export async function renderCoaxAudio(lines,frames,buffers){
  const module=await moduleForAudio(),context=new OfflineAudioContext(2,frames,48000);await context.audioWorklet.addModule(workletURL);
  const assets=Object.entries(buffers).map(([name,buffer])=>({name,samples:interleave(buffer),channels:buffer.numberOfChannels}));
  const encoder=new TextEncoder(),schedule=lines.map(([frame,line,started])=>[frame,encoder.encode(line),started]);
  const node=new AudioWorkletNode(context,'aethyr-coax-audio-v1',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{module,assets,schedule,observe:true}});
  const errors=[];let observation=null;node.port.onmessage=e=>{if(e.data.error)errors.push(e.data.error);if(e.data.observation)observation=e.data.observation;};node.onprocessorerror=()=>errors.push('audio processor error');node.connect(context.destination);
  const output=await context.startRendering();await new Promise(resolve=>setTimeout(resolve,0));node.disconnect();node.port.close();if(errors.length)throw Error(errors.join('\n'));output.coaxWorkletReceipt={...observation,errors:errors.length};return output;
}
