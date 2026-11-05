// Firm reason: 1 (browser glue: Wasm memory, immutable PCM copies and scalar Coax audio calls).
export class AudioRefusal extends Error {}
export class AudioBridge {
  constructor(module, render = true) {
    this.memory=new WebAssembly.Memory({initial:256,maximum:1024});
    const memory=this.memory;
    const wasi={proc_exit: code=>{throw Error(`audio reactor exited ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
      clock_time_get(id,precision,at){const seconds=id===0?Date.now()/1000:typeof currentTime==='number'?currentTime:performance.now()/1000;new DataView(memory.buffer).setBigUint64(at,BigInt(Math.floor(seconds*1e9)),true);return 0;},
      fd_write(fd,iov,count,written){const v=new DataView(memory.buffer);let n=0;for(let i=0;i<count;i++)n+=v.getUint32(iov+8*i+4,true);v.setUint32(written,n,true);return 0;}};
    this.x=new WebAssembly.Instance(module,{env:{memory},wasi_snapshot_preview1:wasi}).exports;
    this.x._initialize?.();if(render&&!this.x.audio_init())throw Error('audio reactor allocation failed');
    this.encoder=globalThis.TextEncoder?new TextEncoder():null;this.decoder=globalThis.TextDecoder?new TextDecoder('utf-8',{fatal:true}):null;this.assets=new Map();
  }
  line(text){const storage=new Uint8Array(this.memory.buffer,this.x.audio_line_ptr(),this.x.audio_line_capacity());if(typeof text==='string'){const q=this.encoder.encodeInto(text,storage);return q.read===text.length?q.written:-1;}if(text.length>storage.length)return -1;storage.set(text);return text.length;}
  parse(text){const n=this.line(text);if(n<0)return {kind:0,name:''};const kind=this.x.audio_parse(n),bytes=new Uint8Array(this.memory.buffer,this.x.audio_name_ptr(),this.x.audio_name_bytes());return {kind,name:this.decoder?this.decoder.decode(bytes):String.fromCharCode(...bytes)};}
  install(name,samples,channels){
    if(this.assets.has(name))return this.assets.get(name).id;
    const id=this.x.audio_alloc(BigInt(samples.length));if(id<0)throw new AudioRefusal('Coax refused audio allocation');
    new Float32Array(this.memory.buffer,this.x.audio_asset_ptr(id),samples.length).set(samples);this.assets.set(name,{id,channels});return id;
  }
  decodeWav(bytes){
    const input=this.x.audio_alloc(BigInt(Math.ceil(bytes.length/4)));if(input<0)throw new AudioRefusal('Coax refused WAV allocation');let output=-1;
    try{new Uint8Array(this.memory.buffer,this.x.audio_asset_ptr(input),bytes.length).set(bytes);output=this.x.audio_decode_wav(input,bytes.length);if(output<0)throw new AudioRefusal('Coax refused canonical WAV');const channels=this.x.audio_wav_channels(),words=Number(this.x.audio_info(6));return {samples:new Float32Array(this.memory.buffer,this.x.audio_asset_ptr(output),words).slice(),channels};}
    finally{this.x.audio_release(input);if(output>=0)this.x.audio_release(output);}
  }
  planWav(bytes){
    const input=this.x.audio_alloc(BigInt(Math.ceil(bytes.length/4)));if(input<0)throw new AudioRefusal('Coax refused WAV plan storage');
    try{new Uint8Array(this.memory.buffer,this.x.audio_asset_ptr(input),bytes.length).set(bytes);if(!this.x.audio_prepare_wav(input,bytes.length))throw new AudioRefusal('Coax refused canonical WAV plan');return {channels:this.x.audio_wav_channels(),frames:Number(this.x.audio_wav_frames()),words:Number(this.x.audio_info(6))};}
    finally{this.x.audio_release(input);}
  }
  asset(name){const n=this.line(name);if(n<0||!this.x.audio_select(n))throw new AudioRefusal('Coax refused audio asset name');
    const text=(p,n)=>{const bytes=new Uint8Array(this.memory.buffer,p,n);return this.decoder?this.decoder.decode(bytes):String.fromCharCode(...bytes);};return {pinned:!!this.x.audio_descriptor(0),canonical:!!this.x.audio_descriptor(1),bytes:Number(this.x.audio_descriptor(2)),frames:Number(this.x.audio_descriptor(3)),channels:Number(this.x.audio_descriptor(4)),companionBytes:Number(this.x.audio_descriptor(5)),url:text(this.x.audio_source_url_ptr(),this.x.audio_source_url_bytes()),companionURL:text(this.x.audio_companion_url_ptr(),this.x.audio_companion_url_bytes())};}
  sourceOffer(current,incoming,limit){return Number(this.x.audio_source_offer(BigInt(current),BigInt(incoming),BigInt(limit)));}
  planOpus(name,bytes){
    this.asset(name);const input=this.x.audio_alloc(BigInt(Math.ceil(bytes.length/4)));if(input<0)throw new AudioRefusal('Coax refused Opus plan storage');
    try{new Uint8Array(this.memory.buffer,this.x.audio_asset_ptr(input),bytes.length).set(bytes);if(!this.x.audio_prepare_opus(input,bytes.length))throw new AudioRefusal('Coax refused Opus source identity or shape');return {frames:Number(this.x.audio_opus_info(0)),channels:Number(this.x.audio_opus_info(1)),words:Number(this.x.audio_opus_info(2)),decodeFrames:Number(this.x.audio_opus_info(3)),decodeWords:Number(this.x.audio_opus_info(4))};}
    finally{this.x.audio_release(input);}
  }
  outputMatches(buffer){return !!this.x.audio_output_matches(BigInt(buffer.length),BigInt(buffer.numberOfChannels),buffer.sampleRate);}
  installPcm(name,source,companion,expected,nameBytes=name){
    if(this.assets.has(name))return this.assets.get(name).id;this.asset(nameBytes);let input=-1,packed=-1,output=-1;
    try{input=this.x.audio_alloc(BigInt(Math.ceil(source.length/4)));packed=this.x.audio_alloc(BigInt(Math.ceil(companion.length/4)));if(input<0||packed<0)throw new AudioRefusal('Coax refused companion input storage');
      new Uint8Array(this.memory.buffer,this.x.audio_asset_ptr(input),source.length).set(source);new Uint8Array(this.memory.buffer,this.x.audio_asset_ptr(packed),companion.length).set(companion);
      if(!this.x.audio_prepare_opus(input,source.length)||Number(this.x.audio_opus_info(0))!==expected.frames||Number(this.x.audio_opus_info(1))!==expected.channels||Number(this.x.audio_opus_info(2))!==expected.words)throw new AudioRefusal('Coax refused companion source plan');
      output=this.x.audio_decode_pcm(input,source.length,packed,companion.length);if(output<0)throw new AudioRefusal('Coax refused complete PCM companion');this.assets.set(name,{id:output,channels:expected.channels});return output;
    }catch(error){if(output>=0)this.x.audio_release(output);throw error;}
    finally{if(input>=0)this.x.audio_release(input);if(packed>=0)this.x.audio_release(packed);}
  }
  installWav(name,bytes,expected){
    if(this.assets.has(name))return this.assets.get(name).id;
    const input=this.x.audio_alloc(BigInt(Math.ceil(bytes.length/4)));if(input<0)throw new AudioRefusal('Coax refused WAV input storage');let output=-1;
    try{new Uint8Array(this.memory.buffer,this.x.audio_asset_ptr(input),bytes.length).set(bytes);if(!this.x.audio_prepare_wav(input,bytes.length))throw new AudioRefusal('Coax refused canonical WAV plan');
      const channels=this.x.audio_wav_channels(),frames=Number(this.x.audio_wav_frames()),words=Number(this.x.audio_info(6));
      if(channels!==expected.channels||frames!==expected.frames||words!==expected.words)throw Error('Audio allocation observation changed');
      output=this.x.audio_decode_wav(input,bytes.length);if(output<0)throw new AudioRefusal('Coax refused canonical WAV decode');this.assets.set(name,{id:output,channels});return output;
    }catch(error){if(output>=0)this.x.audio_release(output);throw error;}
    finally{this.x.audio_release(input);}
  }
  command(text,started=true){const effect=this.parse(text),asset=this.assets.get(effect.name),n=this.line(text);return n<0?-1:this.x.audio_command(n,asset?.id??-1,asset?.channels??0,started?1:0);}
  block(){if(!this.x.audio_block())throw Error('Coax refused audio block');return new Float32Array(this.memory.buffer,this.x.audio_output_ptr(),2048);}
  info(){return {frames:Number(this.x.audio_info(0)),accepted:Number(this.x.audio_info(1)),refused:Number(this.x.audio_info(2)),roomPartitions:Number(this.x.audio_info(3)),voices:Number(this.x.audio_info(4)),words:Number(this.x.audio_info(5))};}
  close(){const before=this.info();this.x.audio_close();this.assets.clear();return {before,after:this.info()};}
}
