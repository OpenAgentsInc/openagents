// The actual shipped page Wasm, including the private C bridge and shared
// Coax verifier. Independent public throwaway signatures; no real key/network.
import {test,expect} from "bun:test"
import {readFileSync} from "node:fs"
import {join} from "node:path"
const root=join(import.meta.dir,"../../..")
const generated=Bun.spawnSync(["python3",join(import.meta.dir,"newland_fixtures.py")])
if(generated.exitCode)throw Error(generated.stderr.toString())
const f=JSON.parse(generated.stdout.toString()),enc=new TextEncoder(),dec=new TextDecoder()
async function page(nowSec=f.now){
  const memory=new WebAssembly.Memory({initial:1024,maximum:1024}),sent:string[]=[]
  const text=(p:number,n:number)=>dec.decode(new Uint8Array(memory.buffer,p,n))
  const env={memory,coaxial_socket_open:(p:number,n:number)=>{expect(text(p,n)).toBe("wss://relay.openagents.com");return 0},coaxial_socket_send:(_id:number,p:number,n:number)=>{sent.push(text(p,n))},coaxial_socket_close:()=>{}}
  const wasi=new Proxy({},{get:(_t,k)=>()=>{throw Error(`unexpected WASI call ${String(k)}`)}})
  const {instance}=await WebAssembly.instantiate(readFileSync(join(root,"apps/openagents-web/build/pylon.wasm")),{env,wasi_snapshot_preview1:wasi})
  const x=instance.exports as any;x._initialize()
  const state=Number(x.__heap_base.value),vertices=state+x.coaxial_state_size()+16,uniforms=vertices+300000*16*4
  new Uint8Array(memory.buffer,state,x.coaxial_state_size()).fill(0)
  const message=(kind:number,s:string)=>{const b=enc.encode(s);new Uint8Array(memory.buffer,x.coaxial_inbox(state),b.length).set(b);x.coaxial_message(state,kind,b.length)}
  message(600,`now=${nowSec*1000}`);message(256,"")
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
  expect(p.text(x.coaxial_outbox(state),x.coaxial_outbox_len(state))).toBe("curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh && bash /tmp/join-pylon.sh --model elephant-1 --self-test")
  x.coaxial_action_done(state);click(200)
  expect(x.coaxial_action(state)).toBe(1)
  expect(p.text(x.coaxial_outbox(state),x.coaxial_outbox_len(state))).toBe("/docs/connect-a-pylon")
})
test("executor network panel verifies signed kind 30201 pool aggregates and separates mainnet from testnets (#13928)",async()=>{
  // Expired pool aggregate at now=2_000_000_000 must be ignored by freshness policy
  const stale=await page()
  const poolOk={
    id:"c52ff2ccb6988ee0c54e4d9146ec148a92cb8b3ed6a4fde39328d5450cf67340",
    pubkey:"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
    created_at:1795200000,
    kind:30201,
    tags:[["d","exec"],["t","oa:pylon-pool:v2"],["x","c88f4618badf1b000875fd27c27506e8c4c165ddcf46159d807003315ba53dbd"]],
    content:"{\"v\":\"openagents.pylon-pool.v2\",\"generated_at\":1795200000,\"valid_until\":1795200300,\"totals\":{\"pylons_online\":3,\"jobs\":{\"accepted\":12},\"checks\":{\"pass\":11,\"fail\":1},\"paid_msat\":{\"bitcoin\":0,\"testnet\":0,\"signet\":0,\"regtest\":750000}}}",
    sig:"fd675fb83d7ea26d00424c5632559e40a287ce16435d65fff27ec22ff975c338e329b988933312095f70579fb80b0d4fd59659d5a8308907610b1a60d84a922a",
  }
  stale.event(poolOk)
  expect(stale.x.coaxial_pylon_pools(stale.state)).toBe(0)

  const p=await page(1795200100),{x,state}=p
  expect(JSON.parse(p.sent[1])).toEqual(["REQ","pylons-exec",{kinds:[30201],"#t":["oa:pylon-pool:v2"],limit:64},{kinds:[30210,30211,30212],limit:64}])
  const poolBadX={
    id:"1002cf3a7a8934f764cb2a3a04f96ddf01fecf41886bcd4923ea8f1b59630662",
    pubkey:"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
    created_at:1795200000,
    kind:30201,
    tags:[["d","exec"],["t","oa:pylon-pool:v2"],["x","0000000000000000000000000000000000000000000000000000000000000000"]],
    content:poolOk.content,
    sig:"8237e7fcf44c5d934db658492ac2ec5f003052d893450f1c6b577dd6fe03a682b5e6e304270d42a33328a33adc23f9089d81773fb733e0640181c210bc1b4db9",
  }
  const traceOk={
    id:"20a69f10f54826e94851fe7262d17b8469a60b68636728c2eebeedbe10df0202",
    pubkey:"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
    created_at:1795200000,
    kind:30210,
    tags:[["t","oa:exec-trace:v1"]],
    content:"{\"job_id\":\"j1\"}",
    sig:"d2626d6445494b8fa3283bbdd607bb5581986d68e4b3a03f7441565d90b3814f7da8f981dcb7dc20eb39c72529c1ac75bd7aad3c0af60c3b336a4130b7f7bbde",
  }
  // Honest zero-state before verified events arrive
  expect(x.coaxial_pylon_pools(state)).toBe(0)
  expect(x.coaxial_pylon_paid_bitcoin_msat(state)).toBe(0n)
  expect(x.coaxial_pylon_paid_test_msat(state)).toBe(0n)
  expect(x.coaxial_pylon_exec_traces(state)).toBe(0)
  // Tampered x-digest and tampered sig must be refused
  p.event(poolBadX)
  p.event({...poolOk,sig:"0".repeat(128)})
  expect(x.coaxial_pylon_pools(state)).toBe(0)
  expect(x.coaxial_pylon_refused(state)).toBe(2)
  // Valid signed kind 30201 pool aggregate and kind 30210 trace are admitted and deduplicated
  p.event(poolOk)
  expect(x.coaxial_pylon_pools(state)).toBe(1)
  expect(x.coaxial_pylon_paid_bitcoin_msat(state)).toBe(0n)
  expect(x.coaxial_pylon_paid_test_msat(state)).toBe(750000n)
  p.event(traceOk);p.event(traceOk)
  expect(x.coaxial_pylon_exec_traces(state)).toBe(1)
  expect(p.frame()).toBeGreaterThan(100)
})
