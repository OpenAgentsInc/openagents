// Firm reason: 1 (browser SDK effects: real Wasm natural-image bridge and copied byte readback).
globalThis.hostMediaEffects={
 async open(){const {MediaBridge}=await import('/coaxial/media-bridge.js');const r=await fetch('/host-media.wasm');if(!r.ok)throw Error(`media image reactor: ${r.status}`);this.bridge=new MediaBridge(await WebAssembly.compile(await r.arrayBuffer()));return true;},
 decode(file){this.output=this.bridge.decode(W.files[file]);const p=this.output;return [p.width,p.height,p.bytes,p.work,p.source_bytes,0];},
 refuse(file){this.output=null;let refused=0;try{this.output=this.bridge.decode(W.files[file]);}catch(error){this.error=String(error);refused=1;}const x=this.bridge.x;return [refused,...Array.from({length:5},(_,i)=>Number(x.media_info(i)))];},
 read(at,count){const bytes=this.output.data.subarray(at,at+count);let text='';for(let i=0;i<bytes.length;i+=32768)text+=String.fromCharCode(...bytes.subarray(i,i+32768));return btoa(text);},
 close(){this.output=null;this.bridge.close();return true;}
};
'ready'
