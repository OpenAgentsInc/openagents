// Firm reason: 1 (browser SDK effects: exercise actual Wasm catalogue/preflight/companion copies and expose measured words).
globalThis.hostOpusFixture={bridge:null,samples:null,
 async open(){const {AudioBridge}=await import('/coaxial/audio-bridge.js');const r=await fetch('/build/host-audio.wasm');this.bridge=new AudioBridge(await WebAssembly.compile(await r.arrayBuffer()),false);return true;},
 prepare(name){const asset=this.bridge.asset(name),plan=this.bridge.planOpus(name,W.files['input.ogg']);this.samples=null;
  if(asset.canonical){const id=this.bridge.installPcm(name,W.files['input.ogg'],W.files['input.apc'],plan);this.samples=new Uint8Array(this.bridge.memory.buffer,this.bridge.x.audio_asset_ptr(id),plan.words*4);}
  return {asset,plan};
 },
 read(at,count){const bytes=this.samples.subarray(at,count+at);let text='';for(let i=0;i<bytes.length;i+=8192)text+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(text);},
 close(){this.samples=null;return this.bridge.close().after.words;}
};
'ready'
