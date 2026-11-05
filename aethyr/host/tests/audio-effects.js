// Firm reason: 1 (browser SDK effects: decoded AudioBuffers, the real host offline graph, and raw sample readback).
globalThis.hostAudioEffects = {
  buffers: {}, output: null,
  async canonical(file){const {decodeCoaxWav}=await import('/coaxial/audio-host.js');this.canonicalPcm=await decodeCoaxWav(W.files[file]);return {channels:this.canonicalPcm.channels,samples:this.canonicalPcm.samples.length};},
  readCanonical(at,words){const samples=this.canonicalPcm.samples.subarray(at,at+words),bytes=new Uint8Array(samples.buffer,samples.byteOffset,samples.byteLength);let text='';for(let i=0;i<bytes.length;i+=32768)text+=String.fromCharCode(...bytes.subarray(i,i+32768));return btoa(text);},
  async installCanonical(name,file){await this.canonical(file);return this.installPcm(name,this.canonicalPcm.samples,this.canonicalPcm.channels);},
  install(name, file, channels) {
    const bytes=W.files[file], pcm=new Float32Array(bytes.buffer,bytes.byteOffset,bytes.byteLength/4);
    return this.installPcm(name,pcm,channels);
  },
  installPcm(name,pcm,channels){
    const b=new AudioBuffer({numberOfChannels:channels,length:pcm.length/channels,sampleRate:48000});
    for(let c=0;c<channels;c++){const p=b.getChannelData(c);for(let i=0;i<p.length;i++)p[i]=pcm[i*channels+c];}
    this.buffers[name]=b;return pcm.length;
  },
  async render(lines,frames) {try{this.output=await (this.mode==='coax'?coaxial.audio.offlineCoax:coaxial.audio.offline)(lines,frames,this.buffers);return this.output.length;}catch(error){this.failure=String(error.stack||error);throw error;}},
  receipt(){return this.output.coaxWorkletReceipt??null;},
  read(at,frames) {
    const l=this.output.getChannelData(0),r=this.output.getChannelData(1),interleaved=new Float32Array(frames*2);
    for(let i=0;i<frames;i++){interleaved[2*i]=l[at+i];interleaved[2*i+1]=r[at+i];}
    const bytes=new Uint8Array(interleaved.buffer);let text='';
    for(let i=0;i<bytes.length;i+=32768)text+=String.fromCharCode(...bytes.subarray(i,i+32768));
    return btoa(text);
  }
};
'ready'
