// Runs the production layer upload/readback effects with the compiled catalog.
// This is a resource conformance fixture, not a whole-realm frame verdict.
import { startChrome } from "../../coax/web/chrome.ts";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve, dirname, extname } from "node:path";
import { createHash } from "node:crypto";
const [wasmArg,packageArg,outArg,profileArg="0"]=process.argv.slice(2);
if(!wasmArg||!packageArg||!outArg)throw Error("usage: check_render_layers.ts HOST.wasm PACKAGE_DIR OUTDIR [PROFILE]");
const root=resolve(dirname(import.meta.path),"../.."),wasm=resolve(wasmArg),pack=resolve(packageArg),out=resolve(outArg),profile=Number(profileArg);
mkdirSync(out,{recursive:true});const hash=(b:Uint8Array)=>createHash("sha256").update(b).digest("hex"),inputs=new Map<string,string>();
for(const p of [wasm,`${root}/coax/web/render.js`,`${root}/coax/web/render-face.c`,import.meta.path])inputs.set(p,hash(readFileSync(p)));
const html=`<!doctype html><script type="module">
import {RenderHost} from '/coaxial/render.js';
try{
 const adapter=await navigator.gpu.requestAdapter(),device=await adapter.requestDevice(),memory=new WebAssembly.Memory({initial:256,maximum:1024});
 const module=await WebAssembly.compile(await(await fetch('/test.wasm')).arrayBuffer()),imports={env:{memory}};
 for(const row of WebAssembly.Module.imports(module))if(row.kind==='function')(imports[row.module]||={})[row.name]=()=>{throw Error('unexpected import '+row.module+'.'+row.name)};
 const instance=await WebAssembly.instantiate(module,imports),x=instance.exports;x._initialize?.();x.coaxial_render_reset();x.coaxial_render_device(1,1,0);
 const data=new Uint8Array(await(await fetch('/package/render.json')).arrayBuffer());if(x.coaxial_render_json_admit(data.length)!==0)throw Error('package capacity');new Uint8Array(memory.buffer,x.coaxial_render_json_ptr(),data.length).set(data);if(x.coaxial_render_prepare(data.length,${profile})!==0)throw Error('package admission');
 const renderer=new RenderHost({device,memory,exports:x,arena:{},profile:${profile},capture:true});
 renderer.ids={asset:Object.fromEntries(['invalid','data','storage','index','layers','sound','loop','particles','mesh'].map((n,i)=>[n,x.coaxial_render_enum(4,i)]))};renderer.readAssets();
 const rows=renderer.assetRows.filter(a=>a.kindId===renderer.ids.asset.layers);if(rows.length!==1)throw Error('fixture needs one declared layer asset');
 for(let resource=0;resource<x.coaxial_render_binding_count(0);resource++)if(Number(x.coaxial_render_binding(0,resource,6))===x.coaxial_render_enum(2,7))renderer.layerRequest={set:0,resource};
 if(!renderer.layerRequest)throw Error('no compiled layer binding');
 renderer.textureCaptureSink=async(receipt,bytes)=>{await fetch('/capture/layers.bin',{method:'POST',body:bytes});globalThis.layerCapture=receipt;};
 const packed=[];let images;
 if(rows[0].packed){
  const response=await fetch(rows[0].url);if(!response.ok)throw Error('pack fetch '+response.status);const bytes=new Uint8Array(await response.arrayBuffer()),view=new DataView(bytes.buffer);
  if(bytes.length<32||String.fromCharCode(...bytes.slice(0,4))!=='VPK1'||view.getUint32(8,true)!==bytes.length)throw Error('fixture pack framing');
  const count=view.getUint32(4,true),table=view.getUint32(16,true);if(table>bytes.length||count>(bytes.length-table)/48)throw Error('fixture pack table');
  const blobs=[];for(let i=0;i<count;i++){const row=table+i*48,from=view.getUint32(row+4,true),size=view.getUint32(row+8,true);if(from>bytes.length||size>bytes.length-from)throw Error('fixture pack span');if(bytes[row]!==rows[0].pack_part)continue;
   const image=bytes.slice(from,from+size),digest=new Uint8Array(await crypto.subtle.digest('SHA-256',image));packed.push({row:i,from,bytes:size,sha256:[...digest].map(b=>b.toString(16).padStart(2,'0')).join('')});blobs.push(new Blob([image]));
  }images=await Promise.all(blobs.map(b=>createImageBitmap(b)));
 }else images=await Promise.all(rows[0].urls.map(async url=>{const r=await fetch(url);if(!r.ok)throw Error(url+':'+r.status);return createImageBitmap(await r.blob());}));
 await renderer.makeLayers(images);globalThis.layerResult={status:'passed',profile:${profile},urls:rows[0].urls,packed,adapter:adapter.info,capture:globalThis.layerCapture};
}catch(error){globalThis.layerResult={status:'failed',error:String(error.stack||error)};}
</script>`;
const server=Bun.serve({hostname:"127.0.0.1",port:0,maxRequestBodySize:256*1024*1024,async fetch(req){const path=new URL(req.url).pathname;if(path==="/")return new Response(html,{headers:{"content-type":"text/html"}});if(path==="/capture/layers.bin"){writeFileSync(`${out}/layers.bin`,new Uint8Array(await req.arrayBuffer()));return new Response("ok");}const file=path==="/test.wasm"?wasm:path.startsWith("/package/")?resolve(pack,path.slice(9)):path.startsWith("/coaxial/")?resolve(root,"coax/web",path.slice(9)):resolve(root,"apps/openagents-web",path.slice(1));try{const bytes=readFileSync(file);inputs.set(file,hash(bytes));return new Response(bytes,{headers:{"content-type":extname(file)===".js"?"text/javascript":"application/octet-stream"}});}catch{return new Response("not found",{status:404});}}});
const browser=await startChrome("aethyr-layer-capture-",[],1);let ws:WebSocket|undefined;
try{
 const pages=await(await fetch(`http://127.0.0.1:${browser.port}/json/list`)).json();ws=new WebSocket(pages.find((p:any)=>p.type==="page").webSocketDebuggerUrl);let serial=0;const pending=new Map<number,any>();
 ws.onmessage=e=>{const r=JSON.parse(String(e.data)),q=pending.get(r.id);if(!q)return;pending.delete(r.id);clearTimeout(q.timer);r.error?q.reject(r.error):q.resolve(r.result);};await new Promise<void>((r,j)=>{ws!.onopen=()=>r();ws!.onerror=j;});
 const call=(method:string,params:any={})=>new Promise<any>((resolve,reject)=>{const id=++serial,timer=setTimeout(()=>reject(Error(method+" timed out")),30000);pending.set(id,{resolve,reject,timer});ws!.send(JSON.stringify({id,method,params}));});
 const version=await call("Browser.getVersion");await call("Page.navigate",{url:`http://127.0.0.1:${server.port}/`});let status:any;const until=Date.now()+180000;
 while(Date.now()<until){const r=await call("Runtime.evaluate",{expression:"globalThis.layerResult",returnByValue:true});status=r.result.value;if(status)break;await Bun.sleep(100);}
 const changed=[...inputs].filter(([p,h])=>hash(readFileSync(p))!==h).map(([p])=>p);
 const receipt={schema:"aethyr.render.layer-effects.v1",version,inputs:Object.fromEntries(inputs),source_changes:changed,status};writeFileSync(`${out}/receipt.json`,JSON.stringify(receipt,null,2)+"\n");
 if(status?.status!=="passed"||changed.length)throw Error(JSON.stringify(status));
 const capture=status.capture,actual=readFileSync(`${out}/layers.bin`);if(hash(actual)!==capture.sha256)throw Error("readback transport hash mismatch");
 writeFileSync(`${out}/textures.config`,[`textures 1`,`${capture.name} ${capture.format} ${capture.layers} ${capture.levels.length} ${out}/layers.bin`,...capture.levels.map((l:any)=>`${l.width} ${l.height} ${l.offset} ${l.row_bytes} ${l.layer_bytes}`)].join("\n")+"\n");
 console.log(JSON.stringify({status:status.status,layers:capture.layers,levels:capture.levels.length,bytes:actual.length,sha256:capture.sha256}));
}finally{ws?.close();server.stop(true);browser.proc.kill();await browser.proc.exited;}
