// Independent Chapter20 black-box harness. Product routing/crypto stay in
// compiled Coax; this tooling is an adversarial relay and an HTTP caller.
// No public relay, real identity, money, imported secret or production host.
import {createRequire} from "node:module"
import {readFileSync,writeFileSync,mkdirSync,mkdtempSync,existsSync,readdirSync} from "node:fs"
import {join,resolve} from "node:path"
import {tmpdir} from "node:os"
import {createServer} from "node:net"
const root=resolve(import.meta.dir,"../../.."),require=createRequire(join(root,"psionic/tests/pylon/package.json"))
const {schnorr}=require("@noble/curves/secp256k1"),{sha256}=require("@noble/hashes/sha256"),{bytesToHex,hexToBytes}=require("@noble/hashes/utils"),nip44=require("nostr-effect/nip44")
const enc=new TextEncoder(),identity=readFileSync(join(root,"psionic/fixtures/pylon/psionic-toy.selfcheck"),"utf8").match(/^identity (.+)$/m)![1]
const out=resolve(process.env.OUT_DIR||mkdtempSync(join(tmpdir(),"psionic-fleet-e2e-")));mkdirSync(out,{recursive:true})
if(![undefined,"all","native","peers"].includes(process.env.FLEET_SCOPE))throw Error("FLEET_SCOPE must be all, native or peers")
const call={model:"psionic-toy",state:"Water is wet.",questions:{wet:{type:"choice",instructions:"Is the water wet?",criteria:{yes:null,no:null}}}}
const multiCall={...call,questions:{...call.questions,uncertain:{type:"noul",instructions:"Is this certain?"},level:{type:"score",instructions:"How wet?",criteria:["dry","wet"]}}}
type Event={id:string;pubkey:string;created_at:number;kind:number;tags:string[][];content:string;sig:string}
type Mode="early-answer"|"early-reject"|"late-answer"|"answer"|"stale"|"draining"|"mismatch"|"full"|"superseded"|"silence"|"blocked"|"closed"|"retry"|"bad-signature"|"bad-provider"|"bad-job"|"bad-recipient"|"bad-request"|"bad-attempt"|"bad-model"|"bad-identity"|"bad-door"|"bad-question"|"bad-probability"
const now=()=>Math.floor(Date.now()/1000),tag=(e:Event,k:string)=>e.tags.find(t=>t[0]===k)?.[1]
function key(n:number){const k=new Uint8Array(32);k[31]=n;return {secret:k,pubkey:bytesToHex(schnorr.getPublicKey(k)),slug:`peer-${n}`}}
function sign(k:ReturnType<typeof key>,kind:number,tags:string[][],content:string,at=now()):Event{
  const id=sha256(enc.encode(JSON.stringify([0,k.pubkey,at,kind,tags,content])))
  return {id:bytesToHex(id),pubkey:k.pubkey,created_at:at,kind,tags,content,sig:bytesToHex(schnorr.sign(id,k.secret))}
}
function verified(e:Event){try{return bytesToHex(sha256(enc.encode(JSON.stringify([0,e.pubkey,e.created_at,e.kind,e.tags,e.content]))))===e.id&&schnorr.verify(hexToBytes(e.sig),hexToBytes(e.id),hexToBytes(e.pubkey))}catch{return false}}
function response(provider="",slug="",choice="no",id=identity){return {model:"psionic-toy",service:{door:provider?`pylon:${slug}`:"psionic",identity:id,...provider?{provider}:{}},answers:{wet:{type:"choice",choice,confidence:0.8,probabilities:choice==="no"?{yes:0.2,no:0.8}:{yes:0.8,no:0.2}}}}}
function beacon(k:ReturnType<typeof key>,mode:Mode,generation=1){
  const observed=now()-(mode==="stale"?301:0),until=observed+300
  const body={v:"openagents.pylon-beacon.v1",requires:[],provider:k.pubkey,pylon:k.slug,label:"Independent fleet test",status:mode==="draining"?"draining":"online",generation,since:observed-20,observed_at:observed,valid_until:until,class:{family:"cpu",tier:"small",memory_gb:8},slots:{total:2,free:mode==="full"?0:generation===1?2:1},services:[{capability:k.pubkey+":pylon/decision",model:mode==="mismatch"?identity.replace(/.$/,"0"):identity,lanes:["cj-decision"],offering:null,price_hint_msat:null}],settlement:["free-v1"],pools:[]}
  const content=JSON.stringify(body)
  return sign(k,30200,[["d",k.slug],["t","oa:pylon-beacon:v1"],["x",bytesToHex(sha256(enc.encode(content)))],["expiration",String(until)]],content,observed)
}
function relayPeer(mode:Mode){
  let discovered=0,answerSubscriptions=0;const peers=[key(10),key(11)],jobs:any[]=[],errors:string[]=[],authentications:string[]=[],timers:ReturnType<typeof setTimeout>[]=[]
  const server=Bun.serve<{challenge:string;authenticated:boolean;sub:string}>({hostname:"127.0.0.1",port:0,
    fetch(req,s){if(s.upgrade(req,{data:{challenge:"fleet-test-challenge",authenticated:false,sub:""}}))return;return new Response("websocket only",{status:400})},
    websocket:{open(ws){ws.send(JSON.stringify(["AUTH",ws.data.challenge]))},message(ws,raw){try{
      const f=JSON.parse(String(raw))
      if(f[0]==="AUTH"){
        const e=f[1] as Event
        if(!verified(e)||e.kind!==22242||tag(e,"challenge")!==ws.data.challenge||tag(e,"relay")!==`ws://localhost:${server.port}`)throw Error("invalid native AUTH")
        ws.data.authenticated=true;authentications.push(e.pubkey);ws.send(JSON.stringify(["OK",e.id,true,""]))
      }else if(f[0]==="REQ"){
        if(f.slice(2).some((q:any)=>q.kinds?.includes(30200)))discovered++;
        if(f.slice(2).some((q:any)=>q.kinds?.includes(26910)))answerSubscriptions++;
        if(mode==="closed"){ws.send(JSON.stringify(["CLOSED",f[1],"restricted: independent test"]));return}
        if(f.slice(2).some((q:any)=>q.kinds?.includes(30200))){
          ws.send(JSON.stringify(["EVENT",f[1],beacon(peers[0],mode)]))
          if(mode==="retry"||mode==="late-answer")ws.send(JSON.stringify(["EVENT",f[1],beacon(peers[1],"answer",2)]))
          if(mode==="superseded")ws.send(JSON.stringify(["EVENT",f[1],beacon(peers[0],"draining",2)]))
        }
        if(f.slice(2).some((q:any)=>q.kinds?.includes(26910)))ws.data.sub=f[1]
        ws.send(JSON.stringify(["EOSE",f[1]]))
      }else if(f[0]==="EVENT"){
        const e=f[1] as Event
        if(!ws.data.authenticated||!verified(e)||e.kind!==25910)throw Error("invalid or unauthenticated native job")
        const p=peers.find(p=>p.pubkey===tag(e,"p"));if(!p)throw Error("unknown target")
        const conversation=nip44.getConversationKey(p.secret,e.pubkey),job=JSON.parse(nip44.decrypt(e.content,conversation))
        if(job.v!=="openagents.systemone.v1"||job.model!=="psionic-toy"||!job.request||job.attempt<1||job.deadline<now()||job.questions.wet.type!=="choice")throw Error("native job schema/correlation/deadline")
        jobs.push({provider:p.pubkey,job,event_id:e.id})
        const acknowledge=()=>ws.send(JSON.stringify(["OK",e.id,mode!=="blocked"&&mode!=="early-reject",mode==="blocked"||mode==="early-reject"?"blocked: test refusal":""]));
        if(mode==="early-answer"||mode==="early-reject")timers.push(setTimeout(acknowledge,100));else acknowledge();
        if(mode==="blocked"||mode==="silence")return
        const body:any={v:"openagents.systemone.v1",requires:[],type:"result",request:job.request,attempt:job.attempt,outcome:"answered",response:response(p.pubkey,p.slug)}
        let kind=26910,signer=p,tags=[["p",e.pubkey],["e",e.id]],bad=mode.startsWith("bad-")
        if(bad||(mode==="late-answer"&&p===peers[0]))body.response=response(p.pubkey,p.slug,"yes")
        if(mode==="bad-request")body.request+="-unrelated"
        if(mode==="bad-attempt")body.attempt+=1
        if(mode==="bad-model")body.response.model="another-model"
        if(mode==="bad-identity")body.response.service.identity=identity.replace(/.$/,"0")
        if(mode==="bad-door")body.response.service.door="pylon:another-machine"
        if(mode==="bad-question")body.response.answers={unasked:body.response.answers.wet}
        if(mode==="bad-probability")body.response.answers.wet.confidence=0.1
        if(mode==="bad-job")tags[1][1]="0".repeat(64)
        if(mode==="bad-recipient")tags[0][1]="0".repeat(64)
        if(mode==="bad-provider")signer=peers[1]
        if(mode==="retry"&&p===peers[0]){kind=27010;delete body.response;delete body.outcome;body.type="status";body.status="error";body.code="busy"}
        const content=nip44.encrypt(JSON.stringify(body),nip44.getConversationKey(signer.secret,e.pubkey)),answer=sign(signer,kind,tags,content)
        if(mode==="bad-signature")answer.sig="0".repeat(128)
        if(!ws.data.sub)throw Error("job was published before subscribing to answers")
        timers.push(setTimeout(()=>ws.send(JSON.stringify(["EVENT",ws.data.sub,answer])),mode==="late-answer"?(p===peers[0]?650:150):20))
      }
    }catch(e){errors.push(String(e));ws.close()}}}})
  return {url:`ws://localhost:${server.port}`,jobs,errors,authentications,peers,get discovered(){return discovered},get answerSubscriptions(){return answerSubscriptions},stop(){for(const t of timers)clearTimeout(t);server.stop(true)}}
}
async function freePort(){return await new Promise<number>((resolve,reject)=>{const s=createServer();s.on("error",reject);s.listen(0,"127.0.0.1",()=>{const p=(s.address() as any).port;s.close(()=>resolve(p))})})}
const processes:Bun.Subprocess[]=[],results:any[]=[];let serial=0
function start(name:string,cmd:string[]){const label=`${++serial}-${name}`,p=Bun.spawn(cmd,{cwd:root,stdout:Bun.file(join(out,label+".log")),stderr:Bun.file(join(out,label+".err")),env:{...process.env,ASAN_OPTIONS:"detect_leaks=0:abort_on_error=1",UBSAN_OPTIONS:"halt_on_error=1:print_stacktrace=1"}});processes.push(p);return p}
async function stop(p:Bun.Subprocess){if(p.exitCode!==null)return;p.kill("SIGTERM");await Promise.race([p.exited,Bun.sleep(2000)]);if(p.exitCode===null){p.kill("SIGKILL");await p.exited}}
async function until(f:()=>boolean|Promise<boolean>,ms:number){const end=Date.now()+ms;while(Date.now()<end){if(await f())return true;await Bun.sleep(20)}return false}
async function listening(port:number,p:Bun.Subprocess){if(!await until(async()=>{if(p.exitCode!==null)throw Error(`process exited ${p.exitCode}`);try{const s=await Bun.connect({hostname:"127.0.0.1",port,socket:{data(){},open(s){s.end()},close(){},error(){}}});return !!s}catch{return false}},20000))throw Error(`port ${port} did not open`)}
async function ask(port:number,body:any=call){const started=performance.now(),r=await fetch(`http://127.0.0.1:${port}/v1/systemone`,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(body),signal:AbortSignal.timeout(12000)}),text=await r.text();let value:any;try{value=JSON.parse(text)}catch{throw Error(`HTTP ${r.status} not JSON: ${text.slice(0,200)}`)}return {status:r.status,body:value,ms:Math.round(performance.now()-started)}}
function assert(ok:any,what:string){if(!ok)throw Error(what)}
let selected=!process.env.FLEET_FROM;
async function check(name:string,f:()=>Promise<any>){if(process.env.FLEET_ONLY&&name!==process.env.FLEET_ONLY)return;if(!selected){if(name!==process.env.FLEET_FROM)return;selected=true}const t=performance.now();try{const detail=await f();results.push({name,ok:true,ms:Math.round(performance.now()-t),...detail});console.log("ok   "+name)}catch(e){results.push({name,ok:false,error:String(e)});console.error("FAIL "+name+": "+e);throw e}}
const fleet=resolve(process.env.FLEET||join(out,"fleet/psionic-fleet")),relay=resolve(process.env.RELAY_BIN||join(out,"relay")),pylon=resolve(process.env.PYLON||join(out,"pylon/pylon")),serve=resolve(process.env.SERVE||join(out,"serve/psionic-serve"))
async function build(script:string,args:string[],env:any={}){const p=Bun.spawn([join(root,script),...args],{cwd:root,stdout:"inherit",stderr:"inherit",env:{...process.env,...env}});let timer:ReturnType<typeof setTimeout>;try{const code=await Promise.race([p.exited,new Promise<number>((_resolve,reject)=>{timer=setTimeout(()=>{p.kill();reject(Error(`build exceeded 14 minutes: ${script}`))},840000)})]);if(code)throw Error(`${script} failed ${code}`)}finally{clearTimeout(timer!)}}
async function ready(peer:ReturnType<typeof relayPeer>){assert(await until(()=>peer.authentications.length>0&&peer.discovered>0&&peer.answerSubscriptions>0,5000),"native buyer initial relay readiness")}
async function fleetProcess(relayURL:string,fallback="",timeout=2200,attempt=600){const port=await freePort(),args=[fleet,"--relay",relayURL,"--port",String(port),"--identity",identity,"--timeout-ms",String(timeout),"--attempt-ms",String(attempt),"--discover-ms","500"];if(fallback)args.push("--fallback",fallback);const p=start("fleet",args);await listening(port,p);return {port,p}}
function localPeer(badIdentity=false,delay=0){let requests=0;const s=Bun.serve({hostname:"127.0.0.1",port:0,async fetch(req){requests++;const b=await req.json();assert(JSON.stringify(b)===JSON.stringify(call),"fallback altered the admitted call");if(delay)await Bun.sleep(delay);return Response.json(response("","","no",badIdentity?identity.replace(/.$/,"0"):identity))}});return {url:`http://127.0.0.1:${s.port}`,get requests(){return requests},stop(){s.stop(true)}}}
async function rawLocalPeer(mode:string){
  const body=JSON.stringify(response()),length=enc.encode(body).length;let requests=0
  const s=createServer(socket=>{socket.on("error",()=>{});let incoming="",sent=false;socket.on("data",chunk=>{
    incoming+=chunk.toString();const end=incoming.indexOf("\r\n\r\n");if(sent||end<0)return
    const wanted=Number(incoming.slice(0,end).match(/\r\ncontent-length:\s*(\d+)/i)?.[1]||0)
    if(enc.encode(incoming.slice(end+4)).length<wanted)return;sent=true;requests++
    const status=mode==="status-prefix"?"2000 Bogus":"200 OK"
    let headers=mode==="missing-length"?"":`Content-Length: ${mode==="truncated"?length+5:length}\r\n`
    if(mode==="duplicate-length")headers+=`Content-Length: ${length-1}\r\n`
    if(mode==="transfer-encoding")headers+="Transfer-Encoding: chunked\r\n"
    socket.end(`HTTP/1.1 ${status}\r\n${headers}\r\n${body}${mode==="trailing-body"?"unframed": ""}`)
  })})
  await new Promise<void>((resolve,reject)=>{s.on("error",reject);s.listen(0,"127.0.0.1",resolve)})
  return {url:`http://127.0.0.1:${(s.address() as any).port}`,get requests(){return requests},stop(){s.close()}}
}
try{
  if(!process.env.FLEET)await build("psionic/platform/fleet-build.sh",[join(out,"fleet"),"sanitized"])
  if(process.env.FLEET_SCOPE!=="peers"&&!process.env.RELAY_BIN)await build("coax/relay/build.sh",[relay,"--sanitize"])
  if(process.env.FLEET_SCOPE!=="peers"&&!process.env.PYLON)await build("psionic/platform/pylon-build.sh",[join(out,"pylon"),"sanitized"])
  if(process.env.FLEET_SCOPE!=="peers"&&!process.env.SERVE)await build("psionic/platform/serve-build.sh",[join(out,"serve"),"release","plain"])
  for(const path of (process.env.FLEET_SCOPE==="peers"?[fleet]:[fleet,relay,pylon,serve]))assert(existsSync(path),"missing binary "+path)
  if(process.env.FLEET_SCOPE!=="peers"){
  // A real Coax relay, actual native Coax Pylon and toy engine; the HTTP
  // response must carry the native provider identity, not a harness answer.
  const rp=await freePort(),ep=await freePort(),relayURL=`ws://localhost:${rp}`
  const rr=start("coax-relay",[relay,"--listen","127.0.0.1","--port",String(rp),"--url",relayURL,"--store",join(out,"relay.store")]);await listening(rp,rr)
  const engine=start("coax-engine",[serve,"--toy","--port",String(ep),"--fixture-delay-ms","1500"]);await listening(ep,engine)
  const label=join(out,"throwaway-label"),stopFile=join(out,"pylon.stop"),state=join(out,"pylon.identity")
  writeFileSync(label,bytesToHex(crypto.getRandomValues(new Uint8Array(32))),{mode:0o600})
  const native=start("coax-pylon",[pylon,"--relay",relayURL,"--engine",`http://127.0.0.1:${ep}`,"--model","psionic-toy","--decide-ms","30000","--slug","ch20-real","--label","Chapter20 local test","--test-key-label-file",label,"--stop-file",stopFile,"--references",join(root,"psionic/fixtures/pylon/psionic-toy.selfcheck"),"--identity-state",state])
  const nativeLog=join(out,`${serial}-coax-pylon.log`)
  assert(await until(()=>{if(native.exitCode!==null)throw Error("native Pylon exited");return readFileSync(nativeLog,"utf8").includes("online, waiting for jobs")},90000),"real Pylon did not become ready")
  assert(readFileSync(state,"utf8").trim()===identity,"native Pylon identity mismatch")
  await check("real HTTP -> compiled fleet -> Coax relay -> Coax Pylon -> toy engine",async()=>{
    const f=await fleetProcess(relayURL,"",6500,3500)
    try{const r=await ask(f.port,multiCall);assert(r.status===200&&r.body.service.identity===identity&&r.body.service.door==="pylon:ch20-real"&&/^[0-9a-f]{64}$/.test(r.body.service.provider),JSON.stringify(r));assert(r.body.answers.wet.type==="choice"&&r.body.answers.wet.choice==="no",JSON.stringify(r));assert(r.body.answers.uncertain.type==="noul"&&r.body.answers.uncertain.noul>=0&&r.body.answers.uncertain.noul<=1,"native noul answer");assert(r.body.answers.level.type==="score"&&["0","1"].includes(r.body.answers.level.selected)&&r.body.answers.level.score>=0&&r.body.answers.level.score<=1,"native score answer");return {http:r.status,door:r.body.service.door,identity:r.body.service.identity,ms:r.ms}}finally{await stop(f.p)}
  })
  await check("real delayed Pylon cannot answer after the fleet deadline",async()=>{const f=await fleetProcess(relayURL,"",1000,300);try{const r=await ask(f.port);assert([503,524].includes(r.status)&&r.ms<3500,JSON.stringify(r));return {http:r.status,elapsed:r.ms}}finally{await stop(f.p)}})
  writeFileSync(stopFile,"stop\n");await Promise.race([native.exited,Bun.sleep(5000)]);await stop(native)
  await check("same-identity local Coax engine fallback after native Pylon stops",async()=>{const f=await fleetProcess(relayURL,`http://127.0.0.1:${ep}`,6500,1000);try{const r=await ask(f.port);assert(r.status===200&&r.body.service.door==="psionic"&&r.body.service.identity===identity,JSON.stringify(r));return {http:r.status,door:r.body.service.door,elapsed:r.ms}}finally{await stop(f.p)}})
  await stop(engine);await stop(rr)
  }
  if(process.env.FLEET_SCOPE!=="native"){
  for(const mode of ["answer","early-answer","early-reject","late-answer","stale","draining","mismatch","full","superseded","blocked","closed","silence","retry","bad-signature","bad-provider","bad-job","bad-recipient","bad-request","bad-attempt","bad-model","bad-identity","bad-door","bad-question","bad-probability"] as Mode[]){
    await check("independent relay: "+mode,async()=>{const peer=relayPeer(mode),f=await fleetProcess(peer.url);try{await ready(peer);const r=await ask(f.port);assert(peer.errors.length===0,peer.errors.join("; "));assert(peer.authentications.length>0,"native buyer did not authenticate");
      if(mode==="answer"||mode==="early-answer"||mode==="retry"||mode==="late-answer"){assert(r.status===200&&r.body.answers.wet.choice==="no",JSON.stringify(r));assert(r.body.service.provider===(mode==="retry"||mode==="late-answer"?peer.peers[1].pubkey:peer.peers[0].pubkey),"wrong provider response");if(mode==="retry"||mode==="late-answer")assert(peer.jobs.length===2,"did not fail over to another provider")}
      else {assert([503,524].includes(r.status),"invalid peer result was accepted "+JSON.stringify(r));if(["stale","draining","mismatch","full","superseded","closed"].includes(mode))assert(peer.jobs.length===0,"unavailable peer received a job")}
      assert(r.ms<5000,"unbounded dispatch "+r.ms);return {http:r.status,jobs:peer.jobs.length,elapsed:r.ms}
    }finally{await stop(f.p);peer.stop()}})
  }
  for(const wrong of [false,true])await check(wrong?"different-identity local fallback is refused":"empty fleet uses same-identity local fallback",async()=>{const peer=relayPeer("draining"),local=localPeer(wrong),f=await fleetProcess(peer.url,local.url);try{await ready(peer);const r=await ask(f.port);assert(local.requests===1&&peer.jobs.length===0,"fallback routing count");assert(wrong?[503,524].includes(r.status):r.status===200&&r.body.service.door==="psionic"&&r.body.service.identity===identity,JSON.stringify(r));return {http:r.status,local_requests:local.requests}}finally{await stop(f.p);peer.stop();local.stop()}})
  for(const mode of ["status-prefix","duplicate-length","transfer-encoding","missing-length","truncated","trailing-body"])await check("local HTTP framing refuses "+mode,async()=>{const peer=relayPeer("draining"),local=await rawLocalPeer(mode),f=await fleetProcess(peer.url,local.url);try{await ready(peer);const r=await ask(f.port);assert(local.requests===1&&peer.jobs.length===0,"raw fallback routing count");assert([503,524].includes(r.status),"unframed local response was accepted "+JSON.stringify(r));return {http:r.status}}finally{await stop(f.p);peer.stop();local.stop()}})
  await check("request model mismatch dispatches neither remote nor local",async()=>{const peer=relayPeer("answer"),local=localPeer(),f=await fleetProcess(peer.url,local.url);try{await ready(peer);const r=await ask(f.port,{...call,model:"other"});assert(r.status>=400&&r.status<500&&peer.jobs.length===0&&local.requests===0,JSON.stringify(r));return {http:r.status}}finally{await stop(f.p);peer.stop();local.stop()}})
  await check("slow fallback cannot extend the total request deadline",async()=>{const peer=relayPeer("draining"),local=localPeer(false,2500),f=await fleetProcess(peer.url,local.url,1000,300);try{await ready(peer);const r=await ask(f.port);assert(local.requests===1&&[503,524].includes(r.status)&&r.ms<3500,JSON.stringify(r));return {http:r.status,elapsed:r.ms}}finally{await stop(f.p);peer.stop();local.stop()}})
  }
}finally{
  for(const p of processes.reverse())await stop(p)
  const sanitizerErrors=readdirSync(out).filter(n=>n.endsWith(".err")||n.endsWith(".log")).filter(n=>/ERROR: (AddressSanitizer|LeakSanitizer)|runtime error:|SUMMARY: UndefinedBehaviorSanitizer/.test(readFileSync(join(out,n),"utf8")));
  const binaries=Object.fromEntries(Object.entries({fleet,relay,pylon,serve}).filter(([_k,p])=>existsSync(p)).map(([k,p])=>[k,{path:p,sha256:bytesToHex(sha256(readFileSync(p)))}]));
  writeFileSync(join(out,"results.json"),JSON.stringify({binaries,sanitizer_errors:sanitizerErrors,source_commit:process.env.FLEET_SOURCE_COMMIT||null,at:new Date().toISOString(),selection:process.env.FLEET_SCOPE||"all",from:process.env.FLEET_FROM||null,only:process.env.FLEET_ONLY||null,identity,product:"compiled Coax fleet",scope:"loopback only; independent protocol peer and real Coax relay/Pylon/engine",results},null,2)+"\n")
  if((process.env.FLEET_ONLY||process.env.FLEET_FROM)&&results.length===0)throw Error("no fleet E2E cases selected");
  if(sanitizerErrors.length)throw Error("sanitizer failures: "+sanitizerErrors.join(", "));
  console.log(`fleet e2e: ${results.filter(r=>r.ok).length}/${results.length}, artifacts ${out}`)
}
