// Firm reason: 1 (browser SDK effects: real muted AudioContext gestures, worklet callbacks and deferred decoding).
globalThis.hostAudioLive={context:null,adapter:null,events:[],
  async prepare(samples){
    const {openCoaxAudio}=await import('/coaxial/audio-host.js');this.open=openCoaxAudio;
    this.samples=samples;this.context=new AudioContext({sampleRate:48000});await this.context.suspend();
    this.adapter=await openCoaxAudio(this.context,this.context.destination,()=>({samples:this.samples.slice(),channels:1}));await this.adapter.ready;
    return this.context.state;
  },
  async command(line,started){return this.adapter.play(line,started);},
  async info(){return {state:this.context.state,...await this.adapter.info()};},
  gesture(){this.gesturePromise=new Promise((ok,no)=>{addEventListener('keydown',()=>this.context.resume().then(ok,no),{once:true});});return true;},
  async pause(){await this.context.suspend();return this.info();},
  async resume(){await this.context.resume();return this.info();},
  async retireDuringDecode(line){
    let release,entered;const gate=new Promise(ok=>release=ok),began=new Promise(ok=>entered=ok);
    const adapter=await this.open(this.context,this.context.destination,async()=>{entered();await gate;return {samples:this.samples.slice(),channels:1};});
    await adapter.ready;const command=adapter.play(line,true);await began;adapter.close();release();return command;
  },
  async close(){this.adapter.close();await this.context.close();return this.context.state;}
};
'ready'
