// Test tooling: the actual homepage Wasm through the generic socket ABI.
import {readFileSync} from 'node:fs'
export async function homeRuntime(path:string, options:{relay?:string,clock?:()=>number}={}) {
 const memory=new WebAssembly.Memory({initial:1024,maximum:1024}),sockets:(WebSocket|null)[]=Array(4).fill(null)
 let x:any,p=0,vertices=0,uniforms=0,opening=0,frames=0,peak=0,failure:any;const sent:string[]=[],clock=options.clock??(()=>Math.floor(Date.now()/1000))
 const decode=(at:number,n:number)=>new TextDecoder().decode(new Uint8Array(memory.buffer,at,n))
 function deliver(id:number,kind:number,text='') {
  const bytes=new TextEncoder().encode(text);if(bytes.length>x.coaxial_inbox_cap())throw Error('fixture frame cap')
  new Uint8Array(memory.buffer,x.coaxial_inbox(p),bytes.length).set(bytes);try{x.coaxial_message(p,256+4*id+kind,bytes.length)}catch(e){failure=e}
 }
 const env={memory,coaxial_http:()=>0,coaxial_now_seconds:()=>BigInt(clock()),
  coaxial_socket_open(at:number,n:number){const url=decode(at,n);if(url!=='wss://relay.openagents.com')throw Error('unexpected relay');const id=sockets.indexOf(null);if(id<0)return -1
   const ws=new WebSocket(options.relay??url);sockets[id]=ws;opening++
   ws.onopen=()=>{if(sockets[id]===ws)deliver(id,0)}
   ws.onmessage=e=>{if(sockets[id]===ws)deliver(id,1,String(e.data))}
   ws.onerror=()=>{if(sockets[id]===ws)deliver(id,3)}
   ws.onclose=()=>{if(sockets[id]===ws){sockets[id]=null;deliver(id,2)}};return id},
  coaxial_socket_send(id:number,at:number,n:number){const text=decode(at,n);sent.push(text);sockets[id]?.send(text)},
  coaxial_socket_close(id:number){const ws=sockets[id];sockets[id]=null;ws?.close();queueMicrotask(()=>deliver(id,2))}}
 const wasi={fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error('unexpected Wasm diagnostic')}}
 const {instance}=await WebAssembly.instantiate(readFileSync(path),{env,wasi_snapshot_preview1:wasi});x=instance.exports;x._initialize()
 const align=(n:number)=>(n+15)&~15;p=align(Number(x.__heap_base.value)+1024);vertices=align(p+x.coaxial_state_size()+16);uniforms=align(vertices+x.coaxial_max_vertices()*x.coaxial_vertex_floats()*4+16)
 if(uniforms+x.coaxial_uniform_floats()*4>=memory.buffer.byteLength)throw Error('arena capacity')
 let previous=performance.now()
 function frame(){const now=performance.now(),dt=(now-previous)/1000;previous=now;const n=x.coaxial_frame(p,vertices,uniforms,1440,900,1,dt,1);if(n<100||n>x.coaxial_max_vertices())throw Error('geometry budget');peak=Math.max(peak,n);frames++}
 frame();const timer=setInterval(()=>{try{frame()}catch(e){failure=e}},50)
 return {x,p,sent,sockets,frame,stats:()=>({ready:x.coaxial_land_ready_projects(p),beacons:x.coaxial_land_beacons_received(p),pylons:x.coaxial_land_pylons(p),refreshes:x.coaxial_land_refreshes(p),refused:x.coaxial_land_refused(p),project:x.coaxial_land_project_cursor(p),stage:x.coaxial_land_query_stage(p),opening,frames,peak}),
  async until(predicate:()=>boolean,timeout=10000){const start=performance.now();while(!predicate()){if(failure)throw failure;if(performance.now()-start>timeout)throw Error('homepage timeout '+JSON.stringify(this.stats()));await Bun.sleep(20)}},
  stop(){clearInterval(timer);for(let i=0;i<sockets.length;i++){const ws=sockets[i];sockets[i]=null;ws?.close()}}}
}
if(import.meta.main){const h=await homeRuntime(process.argv[2]);try{await h.until(()=>h.stats().ready===63&&h.stats().beacons>0,90000);console.log(JSON.stringify(h.stats()))}finally{h.stop()}}
