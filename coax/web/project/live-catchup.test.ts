// Actual Coax native/browser consumer over capped, signed relay transcripts.
import { afterAll, beforeAll, expect, test } from 'bun:test'
import { dlopen, FFIType, ptr, CString } from 'bun:ffi'
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, rmSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { getPublicKey, finalizeEvent, type Event } from 'nostr-effect/pure'
import { fold, projectTags, repoTags, TASK_PLAN, filters, followups, KIND_ISSUE, type ProjectConfig } from './model'
import { tables } from './tables'
import { Relay } from './relay'

const root=join(import.meta.dir,'../../..'), app=join(root,'apps/openagents-web')
const temp=mkdtempSync(join(tmpdir(),'coax-live-catchup-'))
const sk=new Uint8Array(32);sk[31]=3 // public BIP-340 test key only
const pk=getPublicKey(sk)
const config:ProjectConfig={slug:'coax',name:'Capped fixture',description:'Signed catch-up fixture',relay:'ws://127.0.0.1',path:'coax/',contributions:'open',license:'Apache-2.0',repo:{d:'test',name:'Test repository',description:'Fixture',web:'https://example.com',clone:'https://example.com/test.git',euc:'0'.repeat(40)}}
const sign=(kind:number,at:number,tags:string[][],content='')=>finalizeEvent({kind,created_at:at,tags,content},sk)
const coord=`30621:${pk}:coax`
const issues:Event[]=[], events:Event[]=[]
events.push(sign(30617,1,repoTags(config)),sign(30621,2,projectTags(pk,config)))
for(let i=0;i<155;i++) {
  const tags=[['a',coord],['slug',`task-${i}`],['subject',`Task ${i.toString().padStart(3,'0')}`],['size','S']]
  if(i===0)tags.push(['t','initiative'])
  else if(i===1)tags.push(['t','milestone'],['parent',issues[0].id])
  else tags.push(['parent',issues[1].id])
  const issue=sign(1621,1000+i,tags,`Body ${i}.\n\n## Acceptance\n- Check ${i}`);issues.push(issue);events.push(issue)
}
const comment=(at:number,index:number,tags:string[][],body:string)=>sign(1111,at,[['E',issues[index].id],['K','1621'],['P',pk],...tags],body)
events.push(comment(2000,2,[['claim','old-worker'],['by','old-worker']],'Taking it'),comment(2001,2,[['plan',TASK_PLAN],['by','old-worker']],'- [x] read\n- [>] implement'),sign(1631,2002,[['e',issues[3].id,'','root']]))
for(let i=0;i<180;i++)events.push(comment(10000+i,4+i%151,[['by','notes-worker']],`Note ${i}`))
for(let i=0;i<120;i++)events.push(sign(1630,11000+i,[['e',issues[4+i%151].id,'','root']]))
events.push(sign(30623,30000,[['d','coax'],['a',coord],...issues.map((x,i)=>['item',x.id,`rank-${i.toString().padStart(4,'0')}`])]))
const expected=(rows:Event[])=>tables(fold(pk,config,rows))

type Behavior={cap:number; omit?:string; repair?:boolean; caps?:number[]; calls?:number; reverse?:boolean}
function answer(request:any[], rows:Event[],behavior:Behavior):string[] {
  const [,id,f]=request
  if(request[0]!=='REQ')return[]
  let selected=rows.filter(e=>(!f.kinds||f.kinds.includes(e.kind))&&(!f.authors||f.authors.includes(e.pubkey))&&(!f.ids||f.ids.includes(e.id))&&(f.until===undefined||e.created_at<=f.until))
  for(const [key,values] of Object.entries(f))if(key.startsWith('#'))selected=selected.filter(e=>e.tags.some(t=>t[0]===key.slice(1)&&(values as string[]).includes(t[1])))
  if(behavior.omit)selected=selected.filter(e=>e.id!==behavior.omit||(behavior.repair&&f.ids))
  selected.sort((a,b)=>b.created_at-a.created_at||a.id.localeCompare(b.id))
  const cap=behavior.caps?behavior.caps[(behavior.calls??0)%behavior.caps.length]:behavior.cap;behavior.calls=(behavior.calls??0)+1
  selected=selected.slice(0,Math.min(cap,f.limit??cap));if(behavior.reverse)selected.reverse()
  return [...selected.map(e=>JSON.stringify(['EVENT',id,e])),JSON.stringify(['EOSE',id,['finish']])]
}
let lib:any
const cString=(s:string)=>Buffer.from(s+'\0')
class NativeUI {
  state:any
  constructor(publicKey=pk){this.state=lib.symbols.probe_new();const text=cString(`${publicKey} coax`);lib.symbols.probe_project(this.state,ptr(text),text.length-1);this.frame()}
  frame(){lib.symbols.probe_frame(this.state)}
  message(text:string,kind=257){const bytes=cString(text);lib.symbols.probe_message(this.state,kind,ptr(bytes),bytes.length-1)}
  info(k:number){return Number(lib.symbols.probe_info(this.state,k))}
  text(k:number){const n=Number(lib.symbols.probe_len(this.state,k));return n>0?new CString(lib.symbols.probe_text(this.state,k),0,n).toString():''}
  commands(){const n=this.info(10),result:any[]=[];for(let i=0;i<n;i++){const op=Number(lib.symbols.probe_op(this.state,i));const len=Number(lib.symbols.probe_cmd_len(this.state,i));const text=len>0?new CString(lib.symbols.probe_cmd(this.state,i),0,len).toString():'';result.push({op,text})}lib.symbols.probe_drained(this.state);return result}
  connect(){this.commands();lib.symbols.probe_open(this.state,0);this.message('',256)}
  pump(rows=events,behavior:Behavior={cap:37}){for(let turn=0;turn<2000;turn++){const commands=this.commands();if(!commands.length){this.frame();return}for(const c of commands){if(c.op===2)for(const line of answer(JSON.parse(c.text),rows,behavior))this.message(line)}}throw Error('test transcript exceeded its own command budget')}
  reconnect(){this.message('',258);lib.symbols.probe_advance(this.state,2);this.connect()}
  close(){lib.symbols.probe_free(this.state)}
}
const nativeC=`#include "project.h"
#include <stdlib.h>
#include <string.h>
typedef struct {coax_t_ProjectState page;coax_t_Tree tree;coax_t_Work work;coax_t_Live live;float *vertices;float *uniforms;} Probe;
static coax_v_t_Live lv(Probe*p){return(coax_v_t_Live){&p->live,1};}
void *probe_new(void){Probe*p=calloc(1,sizeof(*p));p->vertices=calloc(coaxial_project_max_vertices()*coaxial_project_vertex_floats(),sizeof(float));p->uniforms=calloc(coaxial_project_uniform_floats(),sizeof(float));return p;}
void probe_free(Probe*p){free(p->vertices);free(p->uniforms);free(p);}
void probe_project(Probe*p,const char*t,int n){memcpy(p->live.inbox.e,t,n);coaxial_project_message(lv(p),2,n);}
void probe_frame(Probe*p){coaxial_project_frame((coax_v_t_ProjectState){&p->page,1},(coax_v_t_Tree){&p->tree,1},(coax_v_t_Work){&p->work,1},lv(p),(coax_v_f32){p->vertices,coaxial_project_max_vertices()*coaxial_project_vertex_floats()},(coax_v_f32){p->uniforms,coaxial_project_uniform_floats()},1440,900,1,0.016,1);}
void probe_message(Probe*p,int kind,const char*t,int n){memcpy(p->live.inbox.e,t,n);coaxial_project_message(lv(p),kind,n);}
void probe_open(Probe*p,int id){coaxial_project_opened(lv(p),id);}
void probe_budget(Probe*p){p->live.pages=256;}
void probe_advance(Probe*p,int seconds){p->page.time+=seconds;probe_frame(p);}
int probe_info(Probe*p,int k){switch(k){case 0:return p->live.state;case 1:return p->work.n;case 2:return p->work.working_n;case 3:return p->live.incomplete;case 4:return p->live.pages;case 5:return p->live.store.e[0].n;case 6:return p->live.connects;case 10:return p->live.pipe.e[0].n;}return -1;}
static coax_string table(Probe*p,int k){switch(k){case 0:return p->work.text;case 1:return p->work.plans;case 2:return p->work.details;case 3:return p->work.log;case 4:return p->work.plan_info;}return(coax_string){0};}
const char *probe_text(Probe*p,int k){return table(p,k).data;}int probe_len(Probe*p,int k){return table(p,k).count;}
int probe_op(Probe*p,int k){return p->live.pipe.e[0].op.e[k];}int probe_cmd_len(Probe*p,int k){return p->live.pipe.e[0].len.e[k];}const char*probe_cmd(Probe*p,int k){return(const char*)p->live.pipe.e[0].buf.e+p->live.pipe.e[0].from.e[k];}
void probe_drained(Probe*p){p->live.pipe.e[0].n=0;p->live.pipe.e[0].used=0;}
`
function run(command:string[],cwd=root){const r=Bun.spawnSync(command,{cwd,stdout:'pipe',stderr:'pipe'});if(r.exitCode!==0)throw Error(new TextDecoder().decode(r.stderr));return r.stdout}
beforeAll(()=>{
  const source=run([join(root,'coax/bin/coax'),'c',join(app,'project.coax'),'--library','--header',join(temp,'project.h')]);writeFileSync(join(temp,'project.c'),source);writeFileSync(join(temp,'probe.c'),nativeC)
  run(['cc','-std=c11','-O1','-shared','-fPIC',join(temp,'project.c'),join(temp,'probe.c'),'-I'+temp,'-lm','-o',join(temp,'probe.dylib')])
  const signature:any={probe_new:{returns:FFIType.ptr},probe_free:{args:[FFIType.ptr]},probe_project:{args:[FFIType.ptr,FFIType.ptr,FFIType.i32]},probe_frame:{args:[FFIType.ptr]},probe_open:{args:[FFIType.ptr,FFIType.i32]},probe_advance:{args:[FFIType.ptr,FFIType.i32]},probe_budget:{args:[FFIType.ptr]},probe_message:{args:[FFIType.ptr,FFIType.i32,FFIType.ptr,FFIType.i32]},probe_info:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},probe_text:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.ptr},probe_len:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},probe_op:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},probe_cmd:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.ptr},probe_cmd_len:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},probe_drained:{args:[FFIType.ptr]}}
  lib=dlopen(join(temp,'probe.dylib'),signature)
},60000)
afterAll(()=>{lib?.close();rmSync(temp,{recursive:true,force:true})})
function assertTables(ui:NativeUI,rows:Event[]){const t=expected(rows);expect(ui.text(0)).toBe(t.rows);expect(ui.text(1)).toBe(t.plans);expect(ui.text(2)).toBe(t.details);expect(ui.text(3)).toBe(t.log);expect(ui.text(4)).toBe(t.planInfo)}

test('capped history and exact ID repair load all155 rows plus older current workers/plans/status',()=>{
 const ui=new NativeUI();try{ui.connect();ui.pump(events,{cap:37,omit:issues[0].id,repair:true});expect(ui.info(0)).toBe(3);expect(ui.info(1)).toBe(155);expect(ui.info(2)).toBe(1);expect(ui.info(4)).toBeGreaterThan(10);assertTables(ui,events)
 const update=comment(40000,4,[['claim','new-worker'],['by','new-worker']],'Taking it live');ui.message(JSON.stringify(['EVENT','comments',update]));ui.frame();expect(ui.info(2)).toBe(2);assertTables(ui,[...events,update])
 }finally{ui.close()}
})
test('missing known ID, auth CLOSED, timestamp ambiguity and budget preserve the last complete view',()=>{
 for(const mode of ['missing','closed','ties','budget']){const ui=new NativeUI();try{ui.connect();ui.pump();const before=ui.text(0);ui.reconnect()
 if(mode==='closed'){ui.message(JSON.stringify(['CLOSED','g2-issues','auth-required: approval']));ui.frame()}
 else {if(mode==='budget')lib.symbols.probe_budget(ui.state);let rows=events;if(mode==='ties')rows=[...events,comment(25000,2,[],'tie one'),comment(25000,3,[],'tie two'),comment(25000,4,[],'hidden tie')];ui.pump(rows,{cap:mode==='budget'?1:mode==='ties'?2:37,omit:mode==='missing'?issues[0].id:undefined})}
 expect(ui.info(0)).toBe(4);expect(ui.info(1)).toBe(155);expect(ui.text(0)).toBe(before);expect(ui.info(3)).toBe(mode==='budget'?3:mode==='ties'?2:mode==='missing'?4:1)
 }finally{ui.close()}}
})
test('table output exhaustion and catch-up timeout retain the previous complete view',()=>{
 for(const mode of ['output','timeout']){const ui=new NativeUI();try{ui.connect();ui.pump();const before=ui.text(0)
 if(mode==='output'){const update=comment(50000,4,[['by','large-note']], 'x'.repeat(20000));ui.message(JSON.stringify(['EVENT','comments',update]));ui.frame();expect(ui.info(3)).toBe(5)}
 else{ui.reconnect();lib.symbols.probe_advance(ui.state,30);expect(ui.info(3)).toBe(3)}
 expect(ui.info(0)).toBe(4);expect(ui.text(0)).toBe(before)
 }finally{ui.close()}}
})
test('old subscription generations cannot inject events after a reused socket reconnects',()=>{
 const ui=new NativeUI();try{ui.connect();ui.pump();const before=ui.text(0);ui.reconnect();const stale=comment(50000,4,[['claim','stale-worker']],'wrong generation');ui.message(JSON.stringify(['EVENT','comments',stale]));ui.message('["EOSE","issues"]');expect(ui.info(5)).toBe(0);ui.pump();expect(ui.info(0)).toBe(3);expect(ui.text(0)).toBe(before);expect(ui.info(2)).toBe(1)}finally{ui.close()}
})
test('changing caps and out-of-order page delivery preserve full parity; unprovable cap1 stays offline',()=>{
 const ui=new NativeUI();try{ui.connect();ui.pump(events,{cap:37,caps:[37,13,5],reverse:true});expect(ui.info(0)).toBe(3);assertTables(ui,events);const before=ui.text(0);ui.reconnect();ui.pump(events,{cap:1});expect(ui.info(0)).toBe(4);expect(ui.info(3)).toBe(2);expect(ui.text(0)).toBe(before)}finally{ui.close()}
})
test('page and fold capacity both cover every combined seed item',()=>{
 const count=(path:string,name:string)=>Number(readFileSync(path,'utf8').match(new RegExp(name+' :: (\\d+);'))![1])
 const display=count(join(app,'project.coax'),'MAX_ITEMS'),foldCap=count(join(app,'project-fold.coax'),'FOLD_ISSUES')
 const seeds=join(app,'projects/coax/work');const countSeeds=readdirSync(seeds).filter(f=>f.endsWith('.json')).map(f=>JSON.parse(readFileSync(join(seeds,f),'utf8'))).reduce((n,s)=>n+1+s.milestones.length+s.tasks.length,0)
 expect(foldCap).toBe(display);expect(display).toBeGreaterThanOrEqual(countSeeds);expect(foldCap).toBeGreaterThanOrEqual(155)
})

test('actual WebGPU browser draws the same155-task state and a live worker update without reload',async()=>{
  const wasm=join(temp,'project.wasm'),face=join(temp,'browser-face.c')
  writeFileSync(face,`#include "${join(app,'project-face.c')}"\n
EXPORT("probe_count") uint32_t probe_count(uint32_t p){return state_at(p)->work.n;}
EXPORT("probe_workers") uint32_t probe_workers(uint32_t p){return state_at(p)->work.working_n;}
EXPORT("probe_state") uint32_t probe_state(uint32_t p){return state_at(p)->live.state;}
static coax_string probe_table(uint32_t p,int k){State*s=state_at(p);switch(k){case 0:return s->work.text;case 1:return s->work.plans;case 2:return s->work.details;case 3:return s->work.log;case 4:return s->work.plan_info;}return(coax_string){0};}
EXPORT("probe_text") uint32_t probe_text(uint32_t p,int k){return(uint32_t)(uintptr_t)probe_table(p,k).data;}
EXPORT("probe_len") uint32_t probe_len(uint32_t p,int k){return probe_table(p,k).count;}
`)
  run(['bash',join(root,'coax/web/wasm.sh'),join(app,'project.coax'),face,wasm])
  const sockets=new Set<any>(), queries:any[]=[]
  let fixtureRows=[...events]
  const server=Bun.serve({port:0,fetch(request,server){
    const url=new URL(request.url)
    if(url.pathname==='/relay'){if(server.upgrade(request,{data:{subs:new Map()}}))return;return new Response('upgrade required',{status:400})}
    if(url.pathname==='/host.js')return new Response(readFileSync(join(root,'coax/web/host.js')),{headers:{'content-type':'text/javascript'}})
    if(url.pathname==='/project.wasm')return new Response(readFileSync(wasm),{headers:{'content-type':'application/wasm'}})
    if(url.pathname==='/project.wgsl')return new Response(readFileSync(join(app,'project.wgsl')),{headers:{'content-type':'text/plain'}})
    if(url.pathname.startsWith('/api/'))return new Response('ok')
    if(url.pathname!=='/')return new Response('missing',{status:404})
    return new Response(`<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;background:black}canvas{width:100vw;height:100vh;display:block}</style></head><body><canvas id="coaxial"></canvas><script>(()=>{const Raw=WebSocket;globalThis.WebSocket=class extends Raw{constructor(url,...args){super(String(url).startsWith('wss://relay.openagents.com')?'ws://127.0.0.1:${server.port}/relay':url,...args)}}})()</script><script src="/host.js" data-app="project" data-wasm="/project.wasm" data-wgsl="/project.wgsl" data-text="${pk} coax"></script></body></html>`,{headers:{'content-type':'text/html'}})
  },websocket:{open(ws){sockets.add(ws)},message(ws,message){const request=JSON.parse(String(message));if(request[0]==='CLOSE'){(ws.data as any).subs.delete(request[1]);return}(ws.data as any).subs.set(request[1],request);queries.push(request);for(const line of answer(request,fixtureRows,{cap:37}))ws.send(line)},close(ws){sockets.delete(ws)}}})
  const profile=join(temp,'chrome');mkdirSync(profile)
  const chrome=Bun.spawn([process.env.CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome','--headless=new',`--user-data-dir=${profile}`,'--no-first-run','--hide-scrollbars','--enable-unsafe-webgpu','--enable-features=Vulkan','--use-angle=metal','--window-size=1440,900','--remote-debugging-port=0','about:blank'],{stdout:'ignore',stderr:'ignore'})
  let ws:WebSocket|undefined
  try{
    let port=''
    for(let i=0;i<100;i++){try{port=readFileSync(join(profile,'DevToolsActivePort'),'utf8').split('\n')[0];break}catch{}await Bun.sleep(50)}
    expect(port).not.toBe('')
    const target:any=(await(await fetch(`http://127.0.0.1:${port}/json/list`)).json() as any[]).find(t=>t.type==='page')
    ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(resolve=>ws!.addEventListener('open',resolve,{once:true}))
    let serial=0;const pending=new Map<number,(value:any)=>void>()
    ws.addEventListener('message',e=>{const m=JSON.parse(String(e.data));if(pending.has(m.id)){pending.get(m.id)!(m);pending.delete(m.id)}})
    const send=(method:string,params:any={})=>new Promise<any>((resolve,reject)=>{const id=++serial;const timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout '+method))},10000);pending.set(id,v=>{clearTimeout(timer);resolve(v)});ws!.send(JSON.stringify({id,method,params}))})
    await send('Page.navigate',{url:`http://127.0.0.1:${server.port}/?probe`})
    const expression=`(()=>{if(typeof engine==='undefined'||!engine||!arena)return null;const x=engine.exports,s=arena.state;const read=k=>new TextDecoder().decode(new Uint8Array(memory.buffer,x.probe_text(s,k),x.probe_len(s,k)));return{state:x.probe_state(s),count:x.probe_count(s),workers:x.probe_workers(s),tables:[0,1,2,3,4].map(read),frames}})()`
    let current:any
    async function waitFor(workers:number){for(let i=0;i<200;i++){const response=await send('Runtime.evaluate',{expression,returnByValue:true});current=response.result?.result?.value;if(current?.state===3&&current?.workers===workers)return current;await Bun.sleep(50)}throw Error('browser did not catch up: '+JSON.stringify(current))}
    await waitFor(1)
    const want=expected(events);expect(current.count).toBe(155);expect(current.frames).toBeGreaterThan(0);expect(current.tables).toEqual([want.rows,want.plans,want.details,want.log,want.planInfo])
    const update=comment(40000,4,[['claim','browser-worker'],['by','browser-worker']],'Taking it from the live browser')
    fixtureRows.push(update)
    for(const socket of sockets)for(const request of socket.data.subs.values()){const lines=answer(request,[update],{cap:37});if(lines.length>1)socket.send(lines[0])}
    await waitFor(2)
    const updated=expected(fixtureRows);expect(current.tables).toEqual([updated.rows,updated.plans,updated.details,updated.log,updated.planInfo]);expect(current.count).toBe(155)
    expect(queries.filter(q=>String(q[1]).startsWith('catchup-')).length).toBeGreaterThan(10)
    const screenshot=await send('Page.captureScreenshot',{format:'png'});writeFileSync('/tmp/coax-live-catchup-browser.png',Buffer.from(screenshot.result.data,'base64'))
    writeFileSync('/tmp/coax-live-catchup-browser.json',JSON.stringify({count:current.count,workers:current.workers,state:current.state,frames:current.frames,queries:queries.length,tableParity:true},null,2)+'\n')
  }finally{ws?.close();chrome.kill();await chrome.exited;server.stop(true)}
},60000)

// Explicit read-only production capture, never part of the offline default run.
if(process.env.COAX_LIVE_REAL==='1')test('published public queue matches verified snapshot through the capped native consumer',async()=>{
 const actual=JSON.parse(readFileSync(join(app,'projects/coax/project.json'),'utf8'))
 const relay=await Relay.open(actual.relay,null)
 let rows:Event[]=[]
 try{rows=await relay.all(filters(actual.pubkey,actual));const ids=rows.filter(e=>e.kind===KIND_ISSUE).map(e=>e.id);for(let i=0;i<ids.length;i+=50)rows.push(...await relay.all(followups(actual.pubkey,ids.slice(i,i+50))))}finally{relay.close()}
 const authoritative=fold(actual.pubkey,actual,rows),t=tables(authoritative)
 const ui=new NativeUI(actual.pubkey)
 try{ui.connect();ui.pump(rows,{cap:37});expect(ui.info(0)).toBe(3);expect(ui.info(1)).toBe(authoritative.issues.length);expect(ui.text(0)).toBe(t.rows);expect(ui.text(1)).toBe(t.plans);expect(ui.text(2)).toBe(t.details);expect(ui.text(3)).toBe(t.log);expect(ui.text(4)).toBe(t.planInfo)
 writeFileSync('/tmp/coax-live-catchup-public.json',JSON.stringify({relay:actual.relay,rows:ui.info(1),workers:ui.info(2),state:ui.info(0),pages:ui.info(4),capturedEvents:rows.length,verifiedIssues:authoritative.issues.length,tableParity:true},null,2)+'\n')
 }finally{ui.close()}
},60000)
