import {expect,test} from "bun:test";
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join,resolve} from "node:path";
const policy=process.env.FLEET_WATCH_BIN!,probe=process.env.FLEET_WATCH_PROBE_BIN!;
if(!policy||!probe)throw new Error("provide native policy/probe binaries");
const root=resolve(import.meta.dir,"../..");
function cycle(extra:Record<string,string>={}){
 const dir=mkdtempSync(join(tmpdir(),"fleet-watch-effects-")),bin=join(dir,"bin"),trace=join(dir,"trace");mkdirSync(bin);
 const fake=String.raw`#!${process.execPath}
const a=process.argv.slice(2),u=a.find(x=>/^https?:/.test(x))||"";
const fs=await import("node:fs");fs.appendFileSync(process.env.TEST_TRACE,JSON.stringify({url:u,post:a.includes("POST")})+"\n");
if(u.includes("/default/token")){console.log('{"access_token":"public-fixture"}');process.exit(0);}
if(u.includes("/default/identity")){console.log("public-fixture");process.exit(0);}
if(u.includes("compute.googleapis.com")){console.log(u.endsWith("/start")?'{"name":"operation-fixture"}':JSON.stringify({status:process.env.TEST_VM_STATE||"RUNNING"}));process.exit(0);}
if(u.includes("run.googleapis.com")){console.log('{"name":"operation-fixture"}');process.exit(0);}
const elephant=u.includes("oa-mesh-elephant"),model=elephant?"elephant-1":"mammoth-1";
const sha=elephant?"682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba":"4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019";
const provider=process.env.TEST_FORGE?"forged":elephant?"59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7":"d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829";
console.log(JSON.stringify({model,answers:{wet:{type:"choice",choice:"yes",confidence:.7,probabilities:{yes:.7,no:.3}}},service:{identity:model+"@sha256:"+sha,provider,door:"pylon:"+(elephant?"oct30-gcloud-elephant-rtx":"oct30-gcloud-mammoth")},latency_ms:7}));`;
 writeFileSync(join(bin,"curl"),fake,{mode:0o700});
 for(const name of ["confidential","text"]){writeFileSync(join(bin,name),'#!/bin/sh\nprintf \'{"kind":"'+name+'","ok":true}\\n\'\n',{mode:0o700});}
 const now=Math.floor(Date.now()/1000),start=now-10;
 const p=Bun.spawnSync(["bash",join(root,"psionic/platform/fleet-watch-run.sh")],{env:{...process.env,PATH:bin+":"+process.env.PATH,TEST_TRACE:trace,PYLON_WATCH_START:String(start),PYLON_WATCH_END:String(start+21600),PYLON_WATCH_DRY_RUN:"0",PYLON_WATCH_POLICY:policy,PYLON_WATCH_PROBE:probe,PYLON_WATCH_CONFIDENTIAL_PROBE:join(bin,"confidential"),PYLON_WATCH_TEXT_PROBE:join(bin,"text"),PYLON_WATCH_GPTOSS_PROVIDER:"public-fixture",...extra}});
 const events=(()=>{try{return readFileSync(trace,"utf8").trim().split("\n").filter(Boolean).map(x=>JSON.parse(x));}catch{return[];}})();
 const result={code:p.exitCode,out:p.stdout.toString(),error:p.stderr.toString(),events};rmSync(dir,{recursive:true,force:true});return result;
}
test("expired window performs no metadata, recovery or inference request",()=>{
 const r=cycle({PYLON_WATCH_START:"100",PYLON_WATCH_END:"21700"});expect(r.code).toBe(0);expect(r.events).toHaveLength(0);expect(r.out).toContain("outside_window");
});
test("healthy complete role set passes without any recovery mutation",()=>{
 const r=cycle();expect(r.error).toBe("");expect(r.code).toBe(0);expect(r.out).toContain('"required_roles":4');expect(r.out).toContain('"kind":"cycle","ok":true');expect(r.events.filter(e=>e.post&&(e.url.endsWith("/start")||e.url.endsWith(":run")))).toHaveLength(0);
});
test("dry terminated hosts never start or dispatch recovery",()=>{
 const r=cycle({TEST_VM_STATE:"TERMINATED",PYLON_WATCH_DRY_RUN:"1"});expect(r.code).toBe(0);expect(r.events.filter(e=>e.post&&(e.url.endsWith("/start")||e.url.endsWith(":run")))).toHaveLength(0);
});
test("active termination starts only TDX and delegates only the existing RTX job",()=>{
 const r=cycle({TEST_VM_STATE:"TERMINATED"});expect(r.code).toBe(0);const calls=r.events.filter(e=>e.post&&(e.url.endsWith("/start")||e.url.endsWith(":run")));expect(calls).toHaveLength(2);expect(calls.some(e=>e.url.endsWith("instances/oct30-att-tdx-clef/start"))).toBe(true);expect(calls.some(e=>e.url.endsWith("jobs/pylon-spot-recovery:run"))).toBe(true);
});
test("HTTP200 with substituted provider cannot be credited as inference",()=>{
 const r=cycle({TEST_FORGE:"1"});expect(r.code).toBe(1);expect(r.out).toContain("answer_or_binding_refused");expect(r.out).toContain('"kind":"cycle","ok":false');
});
test("three working roles cannot hide a missing GPT-OSS probe",()=>{
 const r=cycle({PYLON_WATCH_GPTOSS_PROVIDER:""});expect(r.code).toBe(1);expect(r.out).toContain("probe_or_provider_pending");expect(r.out).toContain('"kind":"cycle","ok":false');
});
