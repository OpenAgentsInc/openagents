// Firm reason: 1 (browser AudioWorklet SDK: copies shared Coax PCM blocks to measured output planes).
import {AudioBridge,AudioRefusal} from './audio-bridge.js';
class AethyrAudio extends AudioWorkletProcessor {
  constructor(options){
    super();const p=options.processorOptions;this.cursor=1024;this.block=null;this.callbacks=0;this.deviceFrames=0;this.failed=null;this.schedule=p.schedule||[];this.next=0;this.observe=p.observe===true;
    try{this.bridge=new AudioBridge(p.module);}catch(error){this.failed=String(error.stack||error);this.port.postMessage({error:this.failed});return;}
    for(const a of p.assets||[])this.bridge.install(a.name,a.samples,a.channels);
    this.port.onmessage=event=>{const m=event.data;try{let result;
      if(m.op==='install')result=this.bridge.install(m.name,m.samples,m.channels);
      else if(m.op==='install-wav')result=this.bridge.installWav(m.name,m.wav,m.plan);
      else if(m.op==='install-pcm')result=this.bridge.installPcm(m.name,m.source,m.companion,m.plan,m.nameBytes);
      else if(m.op==='command')result=this.bridge.command(m.line,m.started);
      else if(m.op==='schedule'){if(this.deviceFrames!==0||this.schedule.length)throw Error('audio schedule already started');this.schedule=m.schedule;this.next=0;result=this.schedule.length;}
      else if(m.op==='info')result={...this.bridge.info(),callbacks:this.callbacks,deviceFrames:this.deviceFrames,failed:this.failed};
      else if(m.op==='retire'){this.retired=true;result=this.bridge.close();this.block=null;this.schedule.length=0;}
      else throw Error('unsupported audio effect');
      this.port.postMessage({id:m.id,result});
    }catch(error){this.port.postMessage({id:m.id,error:String(error.message),refused:error instanceof AudioRefusal});}};
    this.port.postMessage({ready:true});
  }
  process(inputs,outputs){
    const output=outputs[0];if(!output?.length)return true;
    if(this.retired){for(const channel of output)channel.fill(0);return false;}
    if(this.failed){for(const channel of output)channel.fill(0);return false;}
    try{let at=0;while(at<output[0].length){
      if(this.cursor===1024){const frame=this.bridge.info().frames;while(this.next<this.schedule.length&&this.schedule[this.next][0]<=frame){const q=this.schedule[this.next++];this.bridge.command(q[1],q[2]!==false);}this.block=this.bridge.block();this.cursor=0;}
      const n=Math.min(1024-this.cursor,output[0].length-at);
      for(let c=0;c<output.length;c++)for(let i=0;i<n;i++)output[c][at+i]=this.block[2*(this.cursor+i)+c];
      at+=n;this.cursor+=n;
    }this.callbacks++;this.deviceFrames+=output[0].length;if(this.observe&&this.cursor===1024)this.port.postMessage({observation:{...this.bridge.info(),callbacks:this.callbacks,deviceFrames:this.deviceFrames}});
    }catch(error){this.failed=String(error.message);for(const channel of output)channel.fill(0);this.port.postMessage({error:this.failed});return false;}
    return true;
  }
}
registerProcessor('aethyr-coax-audio-v1',AethyrAudio);
