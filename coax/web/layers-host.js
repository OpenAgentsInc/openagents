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
// The GPU format a device's layer array takes and the side it takes it at, Coax's pick from what
// the adapter offers and what the pack carries (GitHub #13014). warm() starts the reactor early.
export const LAYER_BC=1,LAYER_ASTC=2,LAYER_ETC2=4;
export function layerFeatures(features){return(features.has('texture-compression-bc')?LAYER_BC:0)|(features.has('texture-compression-astc')?LAYER_ASTC:0)|(features.has('texture-compression-etc2')?LAYER_ETC2:0);}
export function warmLayerPolicy(){bridge().catch(()=>{});}
export async function layerPolicy(features,carried,phone,role,declared){const b=await bridge(),format=b.format(features,carried,phone,role);return{...format,side:b.tierSide(declared,phone,format.code)};}
// A pack's block build for this device (GitHub #13305): Coax's pick for colour layers from what the
// adapter offers and the builds the realm lists by tag (NAME.TAG.vpk3 beside NAME.pack), or null for
// the pack itself. blockLayout reads a layer's header; layerTier is a format's side tier.
export async function layerBuild(features,tags,phone){
  const b=await bridge();let carried=0;
  for(let code=1;code<=7;code++){const tag=b.tag(code);if(tag&&tags.includes(tag))carried|=b.carries(code);}
  const format=b.format(features,carried,phone,0),tag=b.tag(format.code);return tag?{...format,tag}:null;
}
export async function blockLayout(bytes){return(await bridge()).block(bytes);}
export async function layerTier(declared,phone,code){return(await bridge()).tierSide(declared,phone,code);}
