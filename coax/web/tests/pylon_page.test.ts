// The actual shipped page Wasm, including the private C bridge and shared
// Coax verifier. Independent public throwaway signatures; no real key/network.
import {test,expect} from "bun:test"
import {readFileSync} from "node:fs"
import {join} from "node:path"
const root=join(import.meta.dir,"../../..")
const generated=Bun.spawnSync(["python3",join(import.meta.dir,"newland_fixtures.py")])
if(generated.exitCode)throw Error(generated.stderr.toString())
const f=JSON.parse(generated.stdout.toString()),enc=new TextEncoder(),dec=new TextDecoder()
async function page(){
  const memory=new WebAssembly.Memory({initial:1024,maximum:1024}),sent:string[]=[]
  const text=(p:number,n:number)=>dec.decode(new Uint8Array(memory.buffer,p,n))
  const env={memory,coaxial_socket_open:(p:number,n:number)=>{expect(text(p,n)).toBe("wss://relay.openagents.com");return 0},coaxial_socket_send:(_id:number,p:number,n:number)=>{sent.push(text(p,n))},coaxial_socket_close:()=>{}}
  const wasi=new Proxy({},{get:(_t,k)=>()=>{throw Error(`unexpected WASI call ${String(k)}`)}})
  const {instance}=await WebAssembly.instantiate(readFileSync(join(root,"apps/openagents-web/build/pylon.wasm")),{env,wasi_snapshot_preview1:wasi})
  const x=instance.exports as any;x._initialize()
  const state=Number(x.__heap_base.value),vertices=state+x.coaxial_state_size()+16,uniforms=vertices+300000*16*4
  new Uint8Array(memory.buffer,state,x.coaxial_state_size()).fill(0)
  const message=(kind:number,s:string)=>{const b=enc.encode(s);new Uint8Array(memory.buffer,x.coaxial_inbox(state),b.length).set(b);x.coaxial_message(state,kind,b.length)}
  message(600,`now=${f.now*1000}`);message(256,"")
  const event=(e:any)=>message(257,JSON.stringify(["EVENT","pylons",e]))
  const frame=()=>x.coaxial_frame(state,vertices,uniforms,390,844,1,0,0)
  return {x,state,event,sent,frame,text}
}
test("page verifies beacons and observes only live signed activity, with duplicate and tamper refusal",async()=>{
  const p=await page(),{x,state}=p
  expect(JSON.parse(p.sent[0])).toEqual(["REQ","pylons",{kinds:[30200],limit:200},{kinds:[26910,27010],since:f.now}])
  for(const e of Object.values(f.invalid_beacons))p.event(e)
  p.event({...f.beacon,sig:"0".repeat(128)})
  expect(x.coaxial_pylon_count(state)).toBe(0)
  p.event(f.beacon);p.event(f.beacon)
  expect(x.coaxial_pylon_count(state)).toBe(1)
  expect(p.text(x.coaxial_pylon_slug(state,0),x.coaxial_pylon_slug_len(state,0))).toBe("studio-gpu")
  expect(p.text(x.coaxial_pylon_identity(state,0),x.coaxial_pylon_identity_len(state,0))).toBe(JSON.parse(f.beacon.content).services[0].model)
  expect(x.coaxial_pylon_identity_len(state,-1)).toBe(0)
  expect(x.coaxial_pylon_slug(state,-1)).toBe(0)
  expect(x.coaxial_pylon_slug_len(state,1)).toBe(0)
  p.event(f.older_answer);p.event(f.wrong_author)
  expect(x.coaxial_pylon_answers(state)).toBe(0)
  expect(x.coaxial_pylon_statuses(state)).toBe(0)
  p.event(f.answer);p.event(f.answer);p.event(f.status)
  p.event({...f.answer,content:"tampered"})
  expect(x.coaxial_pylon_answers(state)).toBe(1)
  expect(x.coaxial_pylon_statuses(state)).toBe(1)
  expect(x.coaxial_pylon_row_answers(state,0)).toBe(1)
  expect(x.coaxial_pylon_row_statuses(state,0)).toBe(1)
  expect(x.coaxial_pylon_refused(state)).toBe(8)
  expect(p.frame()).toBeGreaterThan(100)
})
test("join control copies the full executable command and guide uses the existing navigation action",async()=>{
  const p=await page(),{x,state}=p;p.frame()
  const click=(a:number)=>{x.coaxial_pointer(state,a,210,0,0,0);x.coaxial_pointer(state,a,210,2,0,0)}
  click(50)
  expect(x.coaxial_action(state)).toBe(2)
  expect(p.text(x.coaxial_outbox(state),x.coaxial_outbox_len(state))).toBe("curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh && bash /tmp/join-pylon.sh")
  x.coaxial_action_done(state);click(200)
  expect(x.coaxial_action(state)).toBe(1)
  expect(p.text(x.coaxial_outbox(state),x.coaxial_outbox_len(state))).toBe("/pylon/index.md")
})
