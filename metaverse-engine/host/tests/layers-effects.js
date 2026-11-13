// Firm reason: 1 (browser SDK effects: real Wasm image bridge and copied byte readback).
globalThis.hostLayerEffects={
 async open(){const {LayerBridge}=await import('/coaxial/layers-bridge.js');const r=await fetch('/host-layers.wasm');if(!r.ok)throw Error(`image reactor: ${r.status}`);this.bridge=new LayerBridge(await WebAssembly.compile(await r.arrayBuffer()));return [this.bridge.side(2),this.bridge.side(-1),this.bridge.side(1),this.bridge.side(18)];},
 decode(file,side){this.output=this.bridge.decode(W.files[file],side);const p=this.output,x=this.bridge.x;return [p.side,p.width,p.height,p.levels,p.bytes,p.work,Number(x.layers_info(6)),Number(x.layers_info(7)),0];},
 read(at,count){const bytes=this.output.data.subarray(at,at+count);let text='';for(let i=0;i<bytes.length;i+=32768)text+=String.fromCharCode(...bytes.subarray(i,i+32768));return btoa(text);},
 close(){this.output=null;this.bridge.close();return true;}
};
'ready'
