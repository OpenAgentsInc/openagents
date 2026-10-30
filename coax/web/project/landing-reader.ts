// Build-tool effects bridge. All Nostr admission, messages, filters, paging,
// names, beacon validation and folding execute in the linked Coax units.
import {existsSync, readFileSync, writeFileSync, renameSync} from "node:fs"
import {dirname, join, resolve} from "node:path"
import {createHash} from "node:crypto"

const ROOT=resolve(import.meta.dir,"../../.."), OUTPUT=join(ROOT,"apps/openagents-web/build/landing-reader.wasm")
const encoder=new TextEncoder(),decoder=new TextDecoder()
const originals=new WeakMap<object,string>()
export type PublicEvent={id:string;pubkey:string;kind:number;created_at:number;tags:string[][];content:string;sig:string}
export type LandingIssue={id:string;kind:string;status:string;title:string;parent:string;rank:string;claimed_by:string;claimed_at:number;commit:string;status_at:number}
export const original=(event:PublicEvent)=>originals.get(event)??JSON.stringify(event)

async function artifact() {
  const sources=new Set<string>()
  function collect(path:string) {
    path=resolve(path);if(sources.has(path))return;sources.add(path)
    for(const m of readFileSync(path,"utf8").matchAll(/^\s*#load\s+"([^"]+)"/gm))collect(resolve(dirname(path),m[1]))
  }
  for(const name of ["landing-browser.coax","landing-fold.coax"])collect(join(import.meta.dir,name))
  for(const name of ["landing-reader-face.c","landing-fold-face.c","build-reader.sh"])sources.add(join(import.meta.dir,name))
  sources.add(join(ROOT,"coax/web/toolchain.sh"));sources.add(join(ROOT,"coax/bin/coax"))
  const hash=createHash("sha256");for(const path of [...sources].sort())hash.update(path).update("\0").update(readFileSync(path)).update("\0")
  const source=hash.digest("hex"),record=OUTPUT+".json"
  const digest=(bytes:Uint8Array)=>createHash("sha256").update(bytes).digest("hex")
  let cache:any;try{cache=JSON.parse(readFileSync(record,"utf8"))}catch{}
  if(!existsSync(OUTPUT)||cache?.source!==source||cache?.wasm!==digest(readFileSync(OUTPUT))){
    const p=Bun.spawn(["bash",join(import.meta.dir,"build-reader.sh"),OUTPUT],{stdout:"inherit",stderr:"inherit"})
    if(await p.exited)throw Error("Coax public reader build failed")
    const temporary=record+`.${process.pid}`;writeFileSync(temporary,JSON.stringify({source,wasm:digest(readFileSync(OUTPUT))}));renameSync(temporary,record)
  }
  return readFileSync(OUTPUT)
}

export class LandingReader {
  memory:WebAssembly.Memory;x:any
  private constructor(memory:WebAssembly.Memory,x:any){this.memory=memory;this.x=x}
  static async create(bytes?:Uint8Array){
    const memory=new WebAssembly.Memory({initial:1024,maximum:1024})
    const wasi={fd_close:()=>52,fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error("Coax public reader diagnostic")}}
    const {instance}=await WebAssembly.instantiate(bytes??await artifact(),{env:{memory},wasi_snapshot_preview1:wasi})
    ;(instance.exports as any)._initialize()
    return new LandingReader(memory,instance.exports)
  }
  input(slot:number,text:string){const bytes=encoder.encode(text);if(bytes.length>this.x.nl_input_cap())throw Error("public capture input capacity exceeded");new Uint8Array(this.memory.buffer,this.x.nl_input(slot),bytes.length).set(bytes);return bytes.length}
  output(){return decoder.decode(new Uint8Array(this.memory.buffer,this.x.nl_output(),this.x.nl_output_len()))}
  require(ok:number,what:string){if(!ok)throw Error(`Coax refused ${what}`);return this.output()}
  verify(text:string){return this.x.nl_verify(this.input(0,text))===1}
  npub(key:string){return this.require(this.x.nl_npub(this.input(0,key)),"public key")}
  filter(mode:number,key="",value=""){return this.require(this.x.nl_filter(mode,this.input(0,key),this.input(1,value)),"query filter")}
  beacon(event:PublicEvent,now:number){const ok=this.x.nl_beacon(this.input(0,original(event)),BigInt(now));return ok?JSON.parse(this.output()):null}
  newer(g:number,t:number,hg:number,ht:number){return this.x.nl_newer(BigInt(g),BigInt(t),BigInt(hg),BigInt(ht))===1}
  stats(work:PublicEvent[],requests:PublicEvent[],key:string,now:number){
    const pack=(events:PublicEvent[])=>"["+events.map(original).join(",")+"]"
    return JSON.parse(this.require(this.x.nl_stats(this.input(0,pack(work)),this.input(1,pack(requests)),this.input(2,key),BigInt(now)),"signed activity"))
  }
  fold(events:PublicEvent[],key:string,slug:string,known:string[]=[]):{issues:LandingIssue[]}{
    this.require(this.x.nl_fold_begin(this.input(0,key),this.input(1,slug),this.input(2,JSON.stringify(known))),"project identity")
    for(const event of events)this.require(this.x.nl_fold_feed(this.input(0,original(event))),"project event")
    return JSON.parse(this.require(this.x.nl_fold_result(),"complete project fold"))
  }
  begin(filter:string){this.require(this.x.nl_begin(this.input(0,filter)),"query")}
  request(){return this.require(this.x.nl_request(),"page request")}
  close(){return this.require(this.x.nl_close(),"subscription close")}
  frame(text:string){const result=this.x.nl_frame(this.input(0,text));if(result<0)throw Error(`Coax public read refused (${result})`);if(result!==1)return {result};const raw=this.output(),event=JSON.parse(raw);originals.set(event,raw);return {result,event:event as PublicEvent}}
  state(){return this.x.nl_state()}
  budget(events:number,bytes:number){if(!this.x.nl_budget(BigInt(events),BigInt(bytes)))throw Error("Coax public capture budget exceeded")}
}
export const reader=await LandingReader.create()
let publicQueryBusy=false

// One public connection, one request at a time. No key, AUTH response,
// publishing, SDK or protocol parsing in the transport. Refusal/timeout
// rejects; the generator commits its output only after all reads complete.
export class PublicRelay {
  private ws:WebSocket;private pending:any;private busy=false;private observed=0;private bytes=0
  private constructor(readonly url:string){this.ws=new WebSocket(url)}
  static async open(url:string){
    if(!/^wss?:\/\//.test(url))throw Error("relay URL must be ws:// or wss://")
    const r=new PublicRelay(url)
    await new Promise<void>((resolve,reject)=>{const timer=setTimeout(()=>{r.ws.close();reject(Error("public relay connect timeout"))},10000);r.ws.onopen=()=>{clearTimeout(timer);resolve()};r.ws.onerror=()=>{clearTimeout(timer);reject(Error("public relay unavailable"))}})
    r.ws.onmessage=e=>{if(r.pending)r.pending(String(e.data))}
    r.ws.onerror=()=>r.pending?.(null);r.ws.onclose=()=>r.pending?.(null)
    return r
  }
  async all(filters:string[]):Promise<PublicEvent[]>{
    if(this.busy||publicQueryBusy)throw Error("overlapping public queries are unsupported")
    this.busy=true;publicQueryBusy=true;const events=new Map<string,PublicEvent>()
    try{
      for(const filter of filters){
        reader.begin(filter)
        while(reader.state()!==3){
          const request=reader.request()
          await new Promise<void>((resolve,reject)=>{
            const timer=setTimeout(()=>finish(Error("public relay page timed out")),15000)
            const finish=(error?:Error)=>{clearTimeout(timer);this.pending=null;error?reject(error):resolve()}
            this.pending=(text:string|null)=>{
              if(text===null)return finish(Error("public relay connection closed"))
              try{
                const result=reader.frame(text)
                if(result.event){this.observed++;this.bytes+=encoder.encode(original(result.event)).length;reader.budget(this.observed,this.bytes);events.set(result.event.id,result.event)}
                if(result.result===2){this.ws.send(reader.close());finish()}
              }catch(e){finish(e as Error)}
            }
            this.ws.send(request)
          })
        }
      }
      return [...events.values()]
    }finally{this.pending=null;this.busy=false;publicQueryBusy=false}
  }
  close(){this.pending=null;this.ws.close()}
}

export const projectFilters=(key:string,config:{repo:{d:string};slug:string})=>[reader.filter(0,key,config.repo.d),reader.filter(1,key,config.slug),reader.filter(2,key,config.slug)]
export const followupFilters=(key:string,ids:string[])=>ids.length?[reader.filter(3,key,JSON.stringify(ids)),reader.filter(4,key,JSON.stringify(ids))]:[]
