import {afterAll,expect,test} from "bun:test";
import {mkdtempSync,writeFileSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
const bin=process.env.FLEET_WATCH_BIN!,probe=process.env.FLEET_WATCH_PROBE_BIN!;
if(!bin||!probe)throw new Error("provide qualified native watch binaries");
const dir=mkdtempSync(join(tmpdir(),"fleet-watch-native-"));afterAll(()=>rmSync(dir,{recursive:true,force:true}));
function run(args:string[]){const r=Bun.spawnSync([bin,...args]);return{code:r.exitCode,text:r.stdout.toString().trim()};}
test("native time/target adapter keeps before/after and unknown resources closed",()=>{
 expect(run(["window","100","100","21700"])).toEqual({code:0,text:"0"});
 expect(run(["window","21700","100","21700"]).text).toBe("2");
 const base=["decision","openagentsgemini","us-central1-a","oct30-att-tdx-clef","TERMINATED"];
 expect(run([...base,"100","100","21700","0"]).text).toBe("3");
 expect(run([...base,"99","100","21700","0"]).text).toBe("-2");
 expect(run([...base,"21700","100","21700","0"]).text).toBe("-3");
 expect(run(["decision","openagentsgemini","us-central1-a","other","TERMINATED","100","100","21700","0"]).text).toBe("-1");
});
test("native numeric I/O rejects overflow and trailing syntax",()=>{
 expect(run(["window","9223372036854775808","100","21700"]).code).toBe(64);
 expect(run(["window","100x","100","21700"]).code).toBe(64);
 expect(run(["window","","100","21700"]).code).toBe(64);
 expect(run([]).code).toBe(64);
});
const key="f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
const identity="psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573";
const request={model:"psionic-toy",state:"Water is wet.",questions:{wet:{type:"choice",criteria:{yes:null,no:null}}}};
const response={model:"psionic-toy",answers:{wet:{type:"choice",choice:"yes",confidence:.7,probabilities:{yes:.7,no:.3}}},service:{door:"pylon:studio-gpu",provider:key,identity}};
const req=join(dir,"request.json"),res=join(dir,"response.json");writeFileSync(req,JSON.stringify(request));
function check(body:unknown,provider=key){writeFileSync(res,typeof body==="string"?body:JSON.stringify(body));return Bun.spawnSync([probe,req,res,identity,provider,"studio-gpu"]).exitCode;}
test("native probe validates a complete answer and rejects tuple/shape substitution",()=>{
 expect(check(response)).toBe(0);
 expect(check(response,"other")).toBe(2);
 expect(check({...response,answers:{}})).toBe(2);
 expect(check({...response,answers:{wet:{...response.answers.wet,probabilities:{yes:1.7,no:.3}}}})).toBe(2);
 expect(check("not JSON")).toBe(2);
});
test("native probe bounds file reads before Coax sees bytes",()=>{
 expect(check("x".repeat(262145))).toBe(65);
 expect(Bun.spawnSync([probe,join(dir,"missing"),res,identity,key,"studio-gpu"]).exitCode).toBe(65);
});
