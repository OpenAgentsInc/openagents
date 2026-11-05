// Firm reason: 1 (browser glue: Wasm memory, immutable PCM copies and scalar Coax audio calls).
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
  line(text){const bytes=typeof text==='string'?this.encoder.encode(text):text;if(bytes.length>this.x.audio_line_capacity())return -1;new Uint8Array(this.memory.buffer,this.x.audio_line_ptr(),bytes.length).set(bytes);return bytes.length;}
  parse(text){const n=this.line(text);if(n<0)return {kind:0,name:''};const kind=this.x.audio_parse(n),bytes=new Uint8Array(this.memory.buffer,this.x.audio_name_ptr(),this.x.audio_name_bytes());return {kind,name:this.decoder?this.decoder.decode(bytes):String.fromCharCode(...bytes)};}
  install(name,samples,channels){
    if(this.assets.has(name))return this.assets.get(name).id;
    const id=this.x.audio_alloc(BigInt(samples.length));if(id<0)throw Error('Coax refused audio allocation');
    new Float32Array(this.memory.buffer,this.x.audio_asset_ptr(id),samples.length).set(samples);this.assets.set(name,{id,channels});return id;
  }
  decodeWav(bytes){
    const input=this.x.audio_alloc(BigInt(Math.ceil(bytes.length/4)));if(input<0)throw Error('Coax refused WAV allocation');let output=-1;
    try{new Uint8Array(this.memory.buffer,this.x.audio_asset_ptr(input),bytes.length).set(bytes);output=this.x.audio_decode_wav(input,bytes.length);if(output<0)throw Error('Coax refused canonical WAV');const channels=this.x.audio_wav_channels(),words=Number(this.x.audio_info(6));return {samples:new Float32Array(this.memory.buffer,this.x.audio_asset_ptr(output),words).slice(),channels};}
    finally{this.x.audio_release(input);if(output>=0)this.x.audio_release(output);}
  }
  command(text,started=true){const effect=this.parse(text),asset=this.assets.get(effect.name),n=this.line(text);return n<0?-1:this.x.audio_command(n,asset?.id??-1,asset?.channels??0,started?1:0);}
  block(){if(!this.x.audio_block())throw Error('Coax refused audio block');return new Float32Array(this.memory.buffer,this.x.audio_output_ptr(),2048);}
  info(){return {frames:Number(this.x.audio_info(0)),accepted:Number(this.x.audio_info(1)),refused:Number(this.x.audio_info(2)),roomPartitions:Number(this.x.audio_info(3)),voices:Number(this.x.audio_info(4)),words:Number(this.x.audio_info(5))};}
}
