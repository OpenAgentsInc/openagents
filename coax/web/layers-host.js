// Firm reason: 1 (browser SDK: load the shared image reactor and copy its immutable results).
import {LayerBridge} from './layers-bridge.js';
const url=new URL('../build/host-layers.wasm',import.meta.url);
let compiled,decoder;
async function bridge(){
  compiled??=(async()=>{const r=await fetch(url,{cache:'no-store'});if(!r.ok)throw Error(`image reactor: ${r.status}`);return WebAssembly.compile(await r.arrayBuffer());})().catch(error=>{compiled=null;throw error;});
  const module=await compiled;return decoder??=new LayerBridge(module);
}
export async function canonicalLayerSide(profile){return(await bridge()).side(profile);}
export async function decodeCanonicalLayer(bytes,side){return(await bridge()).decode(bytes,side);}
