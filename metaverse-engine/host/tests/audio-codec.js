// Firm reason: 1 (browser SDK oracle effects: decode exact frozen Ogg bytes and export every unshifted PCM word).
globalThis.hostCodec={context:new OfflineAudioContext(2,1,48000),samples:null,
 async decode(name,frames,channels){
  const buffer=await this.context.decodeAudioData(W.files[name].slice().buffer),observed={frames:buffer.length,channels:buffer.numberOfChannels,rate:buffer.sampleRate};
  this.samples=null;if(buffer.length!==frames||buffer.numberOfChannels!==channels||buffer.sampleRate!==48000)return observed;
  this.samples=new Float32Array(frames*channels);for(let c=0;c<channels;c++){const p=buffer.getChannelData(c);for(let i=0;i<frames;i++)this.samples[i*channels+c]=p[i];}return observed;
 },
 read(at,count){const bytes=new Uint8Array(this.samples.buffer,at*4,count*4);let text='';for(let i=0;i<bytes.length;i+=8192)text+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(text);}
};
'ready'
