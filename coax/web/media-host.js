// Firm reason: 1 (browser SDK: load the shared media image reactor and copy immutable results).
import {MediaBridge} from './media-bridge.js';
const url=new URL('../build/host-media.wasm',import.meta.url);
let compiled,decoder;
async function bridge(){
  compiled??=(async()=>{const r=await fetch(url,{cache:'no-store'});if(!r.ok)throw Error(`media image reactor: ${r.status}`);return WebAssembly.compile(await r.arrayBuffer());})().catch(error=>{compiled=null;throw error;});
  const module=await compiled;return decoder??=new MediaBridge(module);
}
export async function decodeCanonicalMedia(bytes){return(await bridge()).decode(bytes);}
