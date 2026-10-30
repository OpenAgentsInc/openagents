import {test,expect} from "bun:test"
import {readFileSync,writeFileSync,mkdtempSync,rmSync} from "node:fs"
import {tmpdir} from "node:os"
import {join} from "node:path"
import {reader,PublicRelay,projectFilters} from "../project/landing-reader"
import {fromBeacon,pylonRows} from "../project/pylons"

const root=join(import.meta.dir,"../../..")
const generated=Bun.spawnSync(["python3",join(import.meta.dir,"newland_fixtures.py")])
if(generated.exitCode)throw Error(generated.stderr.toString())
const f=JSON.parse(generated.stdout.toString()),raw=(v:any)=>JSON.stringify(v)

test("actual Wasm verifies signatures and the shared beacon contract",()=>{
  expect(reader.verify(raw(f.beacon))).toBe(true)
  expect(reader.verify(raw({...f.beacon,content:"changed"}))).toBe(false)
  expect(reader.verify(raw({...f.beacon,sig:"0".repeat(128)}))).toBe(false)
  expect(fromBeacon(f.beacon,f.now)?.pylon).toBe("studio-gpu")
  for(const e of Object.values(f.invalid_beacons))expect(fromBeacon(e as any,f.now)).toBeNull()
  expect(fromBeacon(f.beacon,f.now+300)).toBeNull()
  expect(reader.newer(7,f.now+1,7,f.now)).toBe(true)
  expect(reader.newer(6,f.now+2,7,f.now)).toBe(false)
})

test("only signed provider observations count; missing timing is unknown",()=>{
  expect(reader.stats([f.answer,f.answer,f.status],[f.request],f.beacon.pubkey,f.now)).toEqual({answers:1,statuses:1,last:f.now,timed:1,average:4})
  expect(reader.stats([f.answer],[],f.beacon.pubkey,f.now).average).toBe(-1)
  expect(()=>reader.stats([f.wrong_author],[],f.beacon.pubkey,f.now)).toThrow()
  expect(()=>reader.stats([{...f.answer,sig:"0".repeat(128)}],[],f.beacon.pubkey,f.now)).toThrow()
  const p=fromBeacon(f.beacon,f.now)!
  const rows=pylonRows([{...p,answers_24h:1,statuses_24h:1,avg_reply_s:4,last_answer_at:f.now}],new Map([["psionic",[1,0,0]]]),f.now)
  expect(rows[0].split("|").at(-1)).toBe("300")
})

test("captured project events fold through Coax with frozen independent rows",()=>{
  const lines=readFileSync(join(root,"coax/lib/nostr/fixtures/coax_relay.jsonl"),"utf8").trim().split("\n").map(s=>JSON.parse(s))
  const events=lines.filter(m=>m[0]==="EVENT").map(m=>m[2])
  const key="9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575"
  const result=reader.fold(events,key,"coax")
  const oracle=readFileSync(join(root,"coax/lib/nostr/fixtures/coax_relay_tables.coax"),"utf8")
  const rows=JSON.parse(oracle.match(/^COAX_RELAY_ITEMS :: (".*");$/m)![1]).trim().split("\n").map((r:string)=>r.split("|"))
  expect(result.issues.length).toBe(rows.length)
  for(const row of rows){
    const issue=result.issues.find(i=>i.id.startsWith(row[0]))!
    expect(issue).toBeDefined()
    expect([issue.kind,issue.status,issue.title,issue.claimed_by]).toEqual([row[1],row[2],row[3],row[6]])
    expect(issue.parent.slice(0,row[7].length)).toBe(row[7])
    expect(issue.rank).toBe(row[9]||"~")
    expect(issue.commit.slice(0,10)).toBe(row[11])
    if(row[12])expect(issue.status_at).toBe(Number(row[12]))
  }
  expect(()=>reader.fold([{...events[0],sig:"0".repeat(128)}],key,"coax")).toThrow()
  expect(()=>reader.fold([],key,"coax")).toThrow()
  expect(()=>reader.fold(events,key,"coax",["0".repeat(64)])).toThrow()
  expect(projectFilters(key,{slug:"coax",repo:{d:"openagents"}})).toHaveLength(3)
})

test("the landing import graph contains no legacy Nostr implementation",()=>{
  const seen=new Set<string>()
  function visit(path:string){
    if(seen.has(path))return;seen.add(path)
    const source=readFileSync(path,"utf8")
    for(const m of source.matchAll(/from\s+["']([^"']+)["']/g)){
      expect(m[1]).not.toMatch(/^(nostr-effect|nostr-tools|@noble\/|@scure\/)/)
      if(m[1].startsWith("."))visit(join(path.slice(0,path.lastIndexOf('/')),m[1]+".ts"))
    }
  }
  visit(join(root,"coax/web/project/landing.ts"));expect(seen.size).toBe(3)
})

test("a failed relay read preserves the last complete capture",async()=>{
  const dir=mkdtempSync(join(tmpdir(),"newland-capture-test-")),out=join(dir,"capture.coax")
  writeFileSync(out,"// last complete capture\n")
  try {
    const p=Bun.spawn([process.execPath,join(root,"coax/web/project/landing.ts"),"--out",out,"--relay","ws://127.0.0.1:1"],{stdout:"ignore",stderr:"pipe"})
    const timer=setTimeout(()=>p.kill(),12000)
    try{expect(await p.exited).not.toBe(0)}finally{clearTimeout(timer)}
    const error=await new Response(p.stderr).text()
    expect(error).toContain("public relay unavailable")
    expect(error).not.toContain("SyntaxError")
    expect(readFileSync(out,"utf8")).toBe("// last complete capture\n")
  }finally{rmSync(dir,{recursive:true,force:true})}
})

test("the actual transport pages through Coax, deduplicates and refuses partial reads",async()=>{
  let requests=0
  const server=Bun.serve({port:0,fetch(req,server){if(server.upgrade(req))return;return new Response(null,{status:400})},websocket:{message(ws,data){
    const m=JSON.parse(String(data));if(m[0]!=="REQ")return;requests++
    ws.send(JSON.stringify(["EVENT",m[1],f.answer]));ws.send(JSON.stringify(["EVENT",m[1],f.answer]));ws.send(JSON.stringify(["EOSE",m[1],["finish"]]))
  }}})
  const relay=await PublicRelay.open(`ws://127.0.0.1:${server.port}`)
  try{const events=await relay.all([reader.filter(6,f.beacon.pubkey,String(f.now))]);expect(events).toHaveLength(1);expect(events[0].id).toBe(f.answer.id);expect(requests).toBe(1)}finally{relay.close();server.stop(true)}
  const denied=Bun.serve({port:0,fetch(req,server){if(server.upgrade(req))return;return new Response(null,{status:400})},websocket:{message(ws,data){const m=JSON.parse(String(data));if(m[0]==="REQ")ws.send(JSON.stringify(["CLOSED",m[1],"restricted: no"]))}}})
  const blocked=await PublicRelay.open(`ws://127.0.0.1:${denied.port}`)
  try{await expect(blocked.all([reader.filter(5)])).rejects.toThrow()}finally{blocked.close();denied.stop(true)}
})

test("multiple history pages retain an inclusive boundary; a one-record cap refuses",async()=>{
  let page=0
  const serve=(singleton:boolean)=>Bun.serve({port:0,fetch(req,s){if(s.upgrade(req))return;return new Response(null,{status:400})},websocket:{message(ws,data){
    const m=JSON.parse(String(data));if(m[0]!=="REQ")return;page++
    if(singleton||page===1)ws.send(JSON.stringify(["EVENT",m[1],f.answer]))
    if(!singleton&&page<=2)ws.send(JSON.stringify(["EVENT",m[1],f.older_answer]))
    ws.send(JSON.stringify(["EOSE",m[1]]))
  }}})
  const s=serve(false),r=await PublicRelay.open(`ws://127.0.0.1:${s.port}`)
  try{const e=await r.all([reader.filter(7,"",JSON.stringify([f.answer.id,f.older_answer.id]))]);expect(e).toHaveLength(2);expect(page).toBe(3)}finally{r.close();s.stop(true)}
  page=0;const capped=serve(true),c=await PublicRelay.open(`ws://127.0.0.1:${capped.port}`)
  try{await expect(c.all([reader.filter(7,"",JSON.stringify([f.answer.id]))])).rejects.toThrow();expect(page).toBe(2)}finally{c.close();capped.stop(true)}
})
