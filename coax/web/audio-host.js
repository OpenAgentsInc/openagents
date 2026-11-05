// Firm reason: 1 (browser SDK: compile the shared reactor, decode/copy assets, and operate its AudioWorklet).
import {AudioBridge} from './audio-bridge.js';
const wasmURL=new URL('../build/host-audio.wasm',import.meta.url),workletURL=new URL('./audio-worklet.js',import.meta.url);
let compiled;
async function moduleForAudio(){return compiled??=(async()=>{const r=await fetch(wasmURL,{cache:'no-store'});if(!r.ok)throw Error('audio reactor unavailable');return WebAssembly.compile(await r.arrayBuffer());})().catch(error=>{compiled=null;throw error;});}
function interleave(buffer){const data=new Float32Array(buffer.length*buffer.numberOfChannels);for(let c=0;c<buffer.numberOfChannels;c++){const p=buffer.getChannelData(c);for(let i=0;i<p.length;i++)data[i*buffer.numberOfChannels+c]=p[i];}return data;}
export async function openCoaxAudio(context,destination,decode){
  const module=await moduleForAudio(),parser=new AudioBridge(module,false);await context.audioWorklet.addModule(workletURL);
  const node=new AudioWorkletNode(context,'aethyr-coax-audio-v1',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{module}});
  node.connect(destination);const pending=new Map(),assets=new Map();let serial=0,closed=false,tail=Promise.resolve(),readyResolve,readyReject;
  const ready=new Promise((ok,no)=>{readyResolve=ok;readyReject=no;});
  const fail=error=>{closed=true;node.disconnect();readyReject(error);for(const q of pending.values())q.no(error);pending.clear();};
  node.onprocessorerror=()=>fail(Error('audio processor error'));
  node.port.onmessage=event=>{const m=event.data;if(m.ready){readyResolve();return;}const q=pending.get(m.id);if(q){pending.delete(m.id);m.error?q.no(Error(m.error)):q.ok(m.result);}else if(m.error)fail(Error(m.error));};
  const request=(op,values,transfer=[])=>new Promise((ok,no)=>{if(closed){no(Error('audio engine retired'));return;}const id=++serial;pending.set(id,{ok,no});node.port.postMessage({id,op,...values},transfer);});
  async function command(line,started){
    await ready;if(closed)return -1;const effect=parser.parse(line);
    if(effect.name&&!assets.has(effect.name)){
      const buffer=await decode(effect.name);if(closed)return -1;if(!buffer) return -1;
      const samples=interleave(buffer);await request('install',{name:effect.name,samples,channels:buffer.numberOfChannels},[samples.buffer]);assets.set(effect.name,true);
    }
    const bytes=parser.encoder.encode(line);return request('command',{line:bytes,started},[bytes.buffer]);
  }
  return {ready,node,play(line,started){const result=tail.then(()=>command(line,started));tail=result.catch(()=>{});return result;},info:()=>request('info',{}),close(){if(closed)return;closed=true;node.disconnect();node.port.close();for(const q of pending.values())q.no(Error('audio engine retired'));pending.clear();readyReject(Error('audio engine retired'));}};
}
export async function renderCoaxAudio(lines,frames,buffers){
  const module=await moduleForAudio(),context=new OfflineAudioContext(2,frames,48000);await context.audioWorklet.addModule(workletURL);
  const assets=Object.entries(buffers).map(([name,buffer])=>({name,samples:interleave(buffer),channels:buffer.numberOfChannels}));
  const encoder=new TextEncoder(),schedule=lines.map(([frame,line,started])=>[frame,encoder.encode(line),started]);
  const node=new AudioWorkletNode(context,'aethyr-coax-audio-v1',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{module,assets,schedule,observe:true}});
  const errors=[];let observation=null;node.port.onmessage=e=>{if(e.data.error)errors.push(e.data.error);if(e.data.observation)observation=e.data.observation;};node.onprocessorerror=()=>errors.push('audio processor error');node.connect(context.destination);
  const output=await context.startRendering();await new Promise(resolve=>setTimeout(resolve,0));node.disconnect();node.port.close();if(errors.length)throw Error(errors.join('\n'));output.coaxWorkletReceipt={...observation,errors:errors.length};return output;
}
