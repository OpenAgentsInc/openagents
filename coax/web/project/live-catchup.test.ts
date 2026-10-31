// Actual Coax native/browser consumer over capped, signed relay transcripts.
import { afterAll, beforeAll, expect, test } from 'bun:test'
import { dlopen, FFIType, ptr, CString } from 'bun:ffi'
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, rmSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { getPublicKey, finalizeEvent, type Event } from 'nostr-effect/pure'
import { fold, projectTags, repoTags, TASK_PLAN, filters, followups, KIND_ISSUE, SEED_SCHEMA, WORK_SCHEMA, seedItems, type ProjectConfig } from './model'
import { tables } from './tables'
import { launch as launchChrome } from '../chrome'
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
#include "project-capacity.h"
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
int probe_info(Probe*p,int k){switch(k){case 0:return p->live.state;case 1:return p->work.n;case 2:return p->work.working_n;case 3:return p->live.incomplete;case 4:return p->live.pages;case 5:return p->live.store.e[0].n;case 6:return p->live.connects;case 7:return p->live.stage;case 8:return p->live.query;case 9:return p->live.page_min;case 10:return p->live.pipe.e[0].n;case 11:return p->live.page_count;case 12:return p->live.page_min_count;case 13:return p->live.history_peak;case 14:return p->live.known_n;case 15:return p->live.store.e[0].used;}return -1;}
static coax_string table(Probe*p,int k){switch(k){case 0:return p->work.text;case 1:return p->work.plans;case 2:return p->work.details;case 3:return p->work.log;case 4:return p->work.plan_info;}return(coax_string){0};}
const char *probe_text(Probe*p,int k){return table(p,k).data;}int probe_len(Probe*p,int k){return table(p,k).count;}
int probe_op(Probe*p,int k){return p->live.pipe.e[0].op.e[k];}int probe_cmd_len(Probe*p,int k){return p->live.pipe.e[0].len.e[k];}const char*probe_cmd(Probe*p,int k){return(const char*)p->live.pipe.e[0].buf.e+p->live.pipe.e[0].from.e[k];}
void probe_drained(Probe*p){p->live.pipe.e[0].n=0;p->live.pipe.e[0].used=0;}
int probe_rows_fit(const char*text,int64_t length,int64_t maximum){return project_rows_fit((coax_string){text,length},maximum);}
int probe_count_fits(int64_t count,int64_t maximum){return project_row_count_fits(count,maximum);}
int probe_choose_refused(Probe*p){char items[321];memset(items,10,sizeof items);return project_capacity_choose_refused((coax_v_t_Tree){&p->tree,1},(coax_v_t_Work){&p->work,1},lv(p),(coax_string){items,sizeof items});}
int probe_live_refused(Probe*p,int oversized){return project_capacity_live_refused(lv(p),(coax_v_t_Work){&p->work,1},oversized!=0);}
`
function run(command:string[],cwd=root){const r=Bun.spawnSync(command,{cwd,stdout:'pipe',stderr:'pipe'});if(r.exitCode!==0)throw Error(new TextDecoder().decode(r.stderr));return r.stdout}
beforeAll(()=>{
  // Compile the authority unit apart: loading it into the app would make
  // unrelated captured-transcript checks part of ordinary app compilation.
  const kernel=run([join(root,'coax/bin/coax'),'c',join(app,'project-capacity.coax'),'--library','--header',join(temp,'project-capacity.h')])
  writeFileSync(join(temp,'project-capacity.c'),kernel)
  run(['cc','-std=c11','-O1','-fPIC','-c',join(temp,'project-capacity.c'),'-o',join(temp,'project-capacity.o')])
  const adapter=join(root,'coax/tests/nostr/project_capacity_adapter.coax')
  const source=run([join(root,'coax/bin/coax'),'c',adapter,'--library','--header',join(temp,'project.h')]);writeFileSync(join(temp,'project.c'),source);writeFileSync(join(temp,'probe.c'),nativeC)
  run(['cc','-std=c11','-O1','-shared','-fPIC',join(temp,'project.c'),join(temp,'probe.c'),join(temp,'project-capacity.o'),'-I'+temp,'-lm','-o',join(temp,'probe.dylib')])
  const signature:any={probe_new:{returns:FFIType.ptr},probe_free:{args:[FFIType.ptr]},probe_project:{args:[FFIType.ptr,FFIType.ptr,FFIType.i32]},probe_frame:{args:[FFIType.ptr]},probe_open:{args:[FFIType.ptr,FFIType.i32]},probe_advance:{args:[FFIType.ptr,FFIType.i32]},probe_budget:{args:[FFIType.ptr]},probe_message:{args:[FFIType.ptr,FFIType.i32,FFIType.ptr,FFIType.i32]},probe_info:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},probe_text:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.ptr},probe_len:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},probe_op:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},probe_cmd:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.ptr},probe_cmd_len:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},probe_drained:{args:[FFIType.ptr]}}
  Object.assign(signature,{
    probe_rows_fit:{args:[FFIType.ptr,FFIType.i64,FFIType.i64],returns:FFIType.i32},
    probe_count_fits:{args:[FFIType.i64,FFIType.i64],returns:FFIType.i32},
    probe_choose_refused:{args:[FFIType.ptr],returns:FFIType.i32},
    probe_live_refused:{args:[FFIType.ptr,FFIType.i32],returns:FFIType.i32},
  })
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
 const seeds=join(app,'projects/coax/work')
 const countItems=(doc:any)=>{
   if(doc.v===SEED_SCHEMA)return seedItems(doc).length
   if(doc.v===WORK_SCHEMA && Array.isArray(doc.items))return doc.items.length
   throw Error('unsupported project work schema in capacity census')
 }
 const countSeeds=readdirSync(seeds).filter(f=>f.endsWith('.json')).map(f=>JSON.parse(readFileSync(join(seeds,f),'utf8'))).reduce((n,s)=>n+countItems(s),0)
 expect(foldCap).toBe(display);expect(display).toBeGreaterThanOrEqual(countSeeds);expect(foldCap).toBeGreaterThanOrEqual(155)
})

test('actual WebGPU browser draws 263 roots with 45 KiB of retained plans and a live update without reload',async()=>{
  const wasm=join(temp,'project.wasm'),face=join(temp,'browser-face.c')
  writeFileSync(face,`#include "${join(app,'project-face.c')}"\n
EXPORT("probe_count") uint32_t probe_count(uint32_t p){return state_at(p)->work.n;}
EXPORT("probe_workers") uint32_t probe_workers(uint32_t p){return state_at(p)->work.working_n;}
EXPORT("probe_state") uint32_t probe_state(uint32_t p){return state_at(p)->live.state;}
static coax_string probe_table(uint32_t p,int k){State*s=state_at(p);switch(k){case 0:return s->work.text;case 1:return s->work.plans;case 2:return s->work.details;case 3:return s->work.log;case 4:return s->work.plan_info;}return(coax_string){0};}
EXPORT("probe_text") uint32_t probe_text(uint32_t p,int k){return(uint32_t)(uintptr_t)probe_table(p,k).data;}
EXPORT("probe_len") uint32_t probe_len(uint32_t p,int k){return probe_table(p,k).count;}
`)
  run(['bash',join(root,'coax/web/wasm.sh'),join(app,'project.coax'),face,wasm,join(app,'project-names.coax'),join(app,'project-capacity.coax')])
  const sockets=new Set<any>(), queries:any[]=[]
  const fixture=planProjectionInventory(45*1024),fixtureRows=fixture.rows
  const server=Bun.serve({port:0,fetch(request,server){
    const url=new URL(request.url)
    if(url.pathname==='/relay'){if(server.upgrade(request,{data:{subs:new Map()}}))return;return new Response('upgrade required',{status:400})}
    // Observe module-local state only in this test response. The actual host
    // and its effect modules run unchanged; production exposes no test hook.
    if(url.pathname==='/host.js')return new Response(readFileSync(join(root,'coax/web/host.js'),'utf8')+`
      globalThis.projectCapacityProbe=()=>{if(!engine||!arena)return null;const x=engine.exports,s=arena.state;const read=k=>new TextDecoder().decode(new Uint8Array(memory.buffer,x.probe_text(s,k),x.probe_len(s,k)));return{state:x.probe_state(s),count:x.probe_count(s),workers:x.probe_workers(s),tables:[0,1,2,3,4].map(read),frames,timeOrigin:performance.timeOrigin}};
    `,{headers:{'content-type':'text/javascript'}})
    if(['/entropy.js','/entropy-import.js','/drop.js','/uploads.js','/docs.js','/card.js','/voice.js','/gpui.js','/loader.js','/http-requests.js'].includes(url.pathname))return new Response(readFileSync(join(root,'coax/web',url.pathname.slice(1))),{headers:{'content-type':'text/javascript'}})
    if(url.pathname==='/project.wasm')return new Response(readFileSync(wasm),{headers:{'content-type':'application/wasm'}})
    if(url.pathname==='/project.wgsl')return new Response(readFileSync(join(app,'project.wgsl')),{headers:{'content-type':'text/plain'}})
    if(url.pathname.startsWith('/api/'))return new Response('ok')
    if(url.pathname!=='/')return new Response('missing',{status:404})
    return new Response(`<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;background:black}canvas{width:100vw;height:100vh;display:block}</style></head><body><canvas id="coaxial"></canvas><script>(()=>{const Raw=WebSocket;globalThis.WebSocket=class extends Raw{constructor(url,...args){super(String(url).startsWith('wss://relay.openagents.com')?'ws://127.0.0.1:${server.port}/relay':url,...args)}}})()</script><script type="module" src="/host.js" data-app="project" data-wasm="/project.wasm" data-wgsl="/project.wgsl" data-text="${pk} coax"></script></body></html>`,{headers:{'content-type':'text/html'}})
  },websocket:{open(ws){sockets.add(ws)},message(ws,message){const request=JSON.parse(String(message));if(request[0]==='CLOSE'){(ws.data as any).subs.delete(request[1]);return}(ws.data as any).subs.set(request[1],request);queries.push(request);for(const line of answer(request,fixtureRows,{cap:37}))ws.send(line)},close(ws){sockets.delete(ws)}}})
  const profile=join(temp,'chrome');mkdirSync(profile)
  const chrome=launchChrome(profile,['--window-size=1440,900'])
  let ws:WebSocket|undefined
  try{
    let port=''
    for(let i=0;i<100;i++){try{port=readFileSync(join(profile,'DevToolsActivePort'),'utf8').split('\n')[0];break}catch{}await Bun.sleep(50)}
    expect(port).not.toBe('')
    const target:any=(await(await fetch(`http://127.0.0.1:${port}/json/list`)).json() as any[]).find(t=>t.type==='page')
    ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(resolve=>ws!.addEventListener('open',resolve,{once:true}))
    let serial=0;const pending=new Map<number,(value:any)=>void>(),browserErrors:string[]=[]
    ws.addEventListener('message',e=>{const m=JSON.parse(String(e.data));if(m.method==='Runtime.exceptionThrown')browserErrors.push(m.params.exceptionDetails.exception?.description??m.params.exceptionDetails.text);if(pending.has(m.id)){pending.get(m.id)!(m);pending.delete(m.id)}})
    const send=(method:string,params:any={})=>new Promise<any>((resolve,reject)=>{const id=++serial;const timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout '+method))},10000);pending.set(id,v=>{clearTimeout(timer);resolve(v)});ws!.send(JSON.stringify({id,method,params}))})
    await send('Runtime.enable');await send('Page.enable')
    await send('Page.navigate',{url:`http://127.0.0.1:${server.port}/?probe`})
    const expression='globalThis.projectCapacityProbe?.()??null'
    let current:any
    async function waitFor(workers:number){for(let i=0;i<200;i++){const response=await send('Runtime.evaluate',{expression,returnByValue:true});current=response.result?.result?.value;if(current?.state===3&&current?.workers===workers)return current;await Bun.sleep(50)}throw Error('browser did not catch up: '+JSON.stringify({current,browserErrors}))}
    await waitFor(1)
    const loadedAt=current.timeOrigin,initialFrames=current.frames
    const want=expected(fixtureRows);expect(current.count).toBe(263);expect(current.frames).toBeGreaterThan(0);expect(current.tables).toEqual([want.rows,want.plans,want.details,want.log,want.planInfo])
    expect(Buffer.byteLength(current.tables[1])).toBe(45*1024)
    expect(Buffer.byteLength(current.tables[1])).toBeGreaterThan(16384)
    expect(Buffer.byteLength(current.tables[1])).toBeLessThanOrEqual(65536)
    const update=sign(1111,60000,[['E',fixture.roots[4].id],['K','1621'],['P',pk],['claim','browser-worker'],['by','browser-worker']],'Taking it from the live browser')
    fixtureRows.push(update)
    for(const socket of sockets)for(const request of socket.data.subs.values()){const lines=answer(request,[update],{cap:37});if(lines.length>1)socket.send(lines[0])}
    await waitFor(2)
    const updated=expected(fixtureRows);expect(current.tables).toEqual([updated.rows,updated.plans,updated.details,updated.log,updated.planInfo]);expect(current.count).toBe(263)
    expect(current.timeOrigin).toBe(loadedAt);expect(current.frames).toBeGreaterThan(initialFrames)
    expect(browserErrors).toEqual([])
    expect(queries.filter(q=>String(q[1]).startsWith('catchup-')).length).toBeGreaterThan(10)
    const screenshot=await send('Page.captureScreenshot',{format:'png'});writeFileSync('/tmp/coax-live-catchup-browser.png',Buffer.from(screenshot.result.data,'base64'))
    writeFileSync('/tmp/coax-live-catchup-browser.json',JSON.stringify({count:current.count,workers:current.workers,state:current.state,frames:current.frames,queries:queries.length,planBytes:Buffer.byteLength(current.tables[1]),retainedFinishedPlans:fixture.planned.length,tableParity:true,reloaded:current.timeOrigin!==loadedAt},null,2)+'\n')
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
 try{ui.connect();ui.pump(rows,{cap:37})
 const captured=[...new Map(rows.map(e=>[e.id,e])).values()]
 const groups=new Map<string,number>()
 for(const e of captured){const stage=e.kind===1621?'issues':e.kind===1111?'comments':e.kind>=1630&&e.kind<=1633?'statuses':'metadata';const key=stage+':'+e.created_at;groups.set(key,(groups.get(key)??0)+1)}
 const diagnostic={at:new Date().toISOString(),relay:actual.relay,artificialReplayCap:37,verifiedIssues:authoritative.issues.length,
   capturedEvents:rows.length,uniqueEvents:captured.length,capturedBytes:Buffer.byteLength(JSON.stringify(rows)),
   native:{state:ui.info(0),incomplete:ui.info(3),pages:ui.info(4),displayedRoots:ui.info(1),storedEvents:ui.info(5),stage:ui.info(7),query:ui.info(8),boundary:ui.info(9),pageCount:ui.info(11),boundaryCount:ui.info(12),historyPeak:ui.info(13),known:ui.info(14),storedBytes:ui.info(15)},
   largestTimestampGroups:[...groups].sort((a,b)=>b[1]-a[1]).slice(0,12).map(([key,count])=>({stage:key.split(':')[0],timestamp:Number(key.split(':')[1]),count})),
   tableBytes:{items:Buffer.byteLength(t.rows),plans:Buffer.byteLength(t.plans),details:Buffer.byteLength(t.details),log:Buffer.byteLength(t.log),info:Buffer.byteLength(t.planInfo)},
   tableLimits:{items:131072,plans:65536,details:1048576,log:16384,info:16384}}
 writeFileSync(`/tmp/coax-capacity-prep/public-${authoritative.issues.length}-diagnostic.json`,JSON.stringify(diagnostic,null,2)+'\n')
 writeFileSync(`/tmp/coax-capacity-prep/public-${authoritative.issues.length}-capture.json`,JSON.stringify(rows)+'\n')
 console.log('public native capture diagnostic',JSON.stringify(diagnostic))
 expect({state:ui.info(0),incomplete:ui.info(3),roots:ui.info(1)}).toEqual({state:3,incomplete:0,roots:authoritative.issues.length})
 expect(ui.text(0)).toBe(t.rows);expect(ui.text(1)).toBe(t.plans);expect(ui.text(2)).toBe(t.details);expect(ui.text(3)).toBe(t.log);expect(ui.text(4)).toBe(t.planInfo)
 writeFileSync('/tmp/coax-live-catchup-public.json',JSON.stringify({relay:actual.relay,rows:ui.info(1),workers:ui.info(2),state:ui.info(0),pages:ui.info(4),capturedEvents:rows.length,verifiedIssues:authoritative.issues.length,tableParity:true},null,2)+'\n')
 }finally{ui.close()}
},60000)

// Replay a separately retained, signature-verified public capture without
// altering its events or the native reader's pagination/storage limits.
if(process.env.COAX_LIVE_FROZEN_PATH)test('frozen public 263-root capture retains all five tables and historical plans',()=>{
 const actual=JSON.parse(readFileSync(join(app,'projects/coax/project.json'),'utf8'))
 const rows:Event[]=JSON.parse(readFileSync(process.env.COAX_LIVE_FROZEN_PATH!,'utf8'))
 const authoritative=fold(actual.pubkey,actual,rows),want=tables(authoritative),ui=new NativeUI(actual.pubkey)
 expect(authoritative.issues.length).toBe(263)
 expect(Buffer.byteLength(want.plans)).toBeGreaterThan(16384)
 expect(Buffer.byteLength(want.plans)).toBeLessThanOrEqual(65536)
 try {
   ui.connect();ui.pump(rows,{cap:37})
   expect({state:ui.info(0),incomplete:ui.info(3),roots:ui.info(1)}).toEqual({state:3,incomplete:0,roots:263})
   expect(ui.text(0)).toBe(want.rows);expect(ui.text(1)).toBe(want.plans);expect(ui.text(2)).toBe(want.details);expect(ui.text(3)).toBe(want.log);expect(ui.text(4)).toBe(want.planInfo)
   writeFileSync('/tmp/coax-capacity-prep/public-263-frozen-green.json',JSON.stringify({capture:process.env.COAX_LIVE_FROZEN_PATH,
     roots:ui.info(1),capturedEvents:rows.length,planBytes:Buffer.byteLength(ui.text(1)),state:ui.info(0),incomplete:ui.info(3),pages:ui.info(4),tableParity:true},null,2)+'\n')
 } finally {ui.close()}
})


// The issue inventory and its signed open-task order have independent caps.
// First 262/263 roots model the current full inventory plus the R1 children
// and capacity prerequisite. The first N roots keep their IDs as N grows.
function capacityInventory(count:number) {
  if(count<112)throw Error('capacity fixture needs two hierarchy roots and 110 open tasks')
  const roots:Event[]=[], rows:Event[]=[sign(30617,1,repoTags(config)),sign(30621,2,projectTags(pk,config))]
  const note='Source ownership, declaration identity and original diagnostics remain independently inspectable. '
  for(let i=0;i<count;i++) {
    const tags=[['a',coord],['slug',`inventory-${i}`],['subject',`Inventory ${String(i).padStart(3,'0')}: preserve project history and source ownership`],['size','S']]
    if(i===0)tags.push(['t','initiative'])
    else if(i===1)tags.push(['t','milestone'],['parent',roots[0].id])
    else {
      tags.push(['parent',roots[1].id])
      if(i>2 && i<112)tags.push(['blocked-by',roots[i-1].id])
    }
    const body=`Root ${i}.\n\n${note.repeat(8)}\n\n## Acceptance\n- Preserve this root, status and parent.\n- Match all five native and TypeScript tables.`
    const issue=sign(1621,1000+i,tags,body);roots.push(issue);rows.push(issue)
    // Exactly 110 open tasks remain independently rankable. Finished tasks
    // still occupy inventory rows and retain their complete signed history.
    if(i>=112)rows.push(sign(i%2===0?1631:1632,10000+i,[['e',issue.id,'','root']]))
  }
  const onFirst=(at:number,tags:string[][],body:string)=>sign(1111,at,[['E',roots[2].id],['K','1621'],['P',pk],...tags],body)
  rows.push(onFirst(20000,[['claim','capacity-worker'],['by','capacity-worker']],'Taking the inventory task'))
  rows.push(onFirst(20001,[['plan',TASK_PLAN],['by','capacity-worker']],'- [x] read\n- [>] implement\n- [ ] verify'))
  rows.push(onFirst(20002,[['by','capacity-worker']],'The completed history remains present.'))
  const open=roots.slice(2,112)
  const order=sign(30623,30000,[['d','coax'],['a',coord],['alt','Order 110 open tasks'],...open.map((e,i)=>['item',e.id,String(i).padStart(3,'0')+'V'])])
  rows.push(order)
  return {roots,rows,order}
}

function expectCapacityView(ui:NativeUI,fixture:ReturnType<typeof capacityInventory>) {
  const count=fixture.roots.length,want=expected(fixture.rows)
  expect(want.by.size).toBe(count)
  expect({state:ui.info(0),incomplete:ui.info(3),roots:ui.info(1)}).toEqual({state:3,incomplete:0,roots:count})
  assertTables(ui,fixture.rows)
  const displayed=ui.text(0).trimEnd().split('\n').map(row=>row.split('|'))
  expect(displayed.length).toBe(count)
  expect(displayed.filter(row=>row[1]==='initiative').length).toBe(1)
  expect(displayed.filter(row=>row[1]==='milestone').length).toBe(1)
  expect(displayed.filter(row=>row[1]==='task' && row[2]==='open').length).toBe(110)
  expect(displayed.filter(row=>row[2]==='resolved' || row[2]==='closed').length).toBe(count-112)
  const detailRows=ui.text(2)
  for(const issue of fixture.roots)expect(detailRows).toContain(issue.id)
  // Keep the existing table budgets: changing inventory capacity must not
  // conceal a larger byte/resource allowance. Native parity above is real.
  const budgets=[131072,16384,1048576,16384,16384]
  for(let k=0;k<budgets.length;k++)expect(Buffer.byteLength(ui.text(k))).toBeLessThanOrEqual(budgets[k])
  expect(fixture.order.tags.filter(tag=>tag[0]==='item').length).toBe(110)
  expect(fixture.order.tags.length).toBe(113)
}

// Keep each signed event and step modest. Only the aggregate projection is
// large: 120 finished tasks retain four short steps each, plus the live plan.
// Root IDs/aliases do not depend on plan text, so byte sizing is independent
// of the native serializer and remains stable at the one-byte boundary.
function planProjectionInventory(targetBytes:number) {
  const base=capacityInventory(263),initial=expected(base.rows),planned=base.roots.slice(112,232)
  const fields=planned.flatMap((issue,i)=>Array.from({length:4},(_,step)=>({issue,plan:i,step,prefix:`history-${i}-${step} `})))
  const fixedBytes=Buffer.byteLength(initial.plans)+fields.reduce((n,field)=>n+initial.alias.get(field.issue.id)!.length+4+Buffer.byteLength(field.prefix),0)
  const padding=targetBytes-fixedBytes
  if(padding<fields.length)throw Error('plan fixture target is too small for meaningful step text')
  const each=Math.floor(padding/fields.length),remainder=padding%fields.length
  const texts=fields.map((field,index)=>field.prefix+'x'.repeat(each+(index<remainder?1:0)))
  const plans=planned.map((issue,i)=>sign(1111,50000+i,[['E',issue.id],['K','1621'],['P',pk],['plan',TASK_PLAN],['by','history-worker']],
    texts.slice(i*4,i*4+4).map(text=>'- [x] '+text).join('\n')))
  const rows=[...base.rows,...plans],oracle=expected(rows)
  const independentlySized=Buffer.byteLength(initial.plans)+fields.reduce((n,field,index)=>n+initial.alias.get(field.issue.id)!.length+4+Buffer.byteLength(texts[index]),0)
  return {...base,rows,oracle,planned,targetBytes,independentlySized,
    largestStepBytes:Math.max(...texts.map(text=>Buffer.byteLength(text))),
    largestEventBytes:Math.max(...plans.map(event=>Buffer.byteLength(JSON.stringify(event))))}
}

function expectPlanProjection(ui:NativeUI,fixture:ReturnType<typeof planProjectionInventory>) {
  expect(fixture.independentlySized).toBe(fixture.targetBytes)
  expect(Buffer.byteLength(fixture.oracle.plans)).toBe(fixture.targetBytes)
  expect(fixture.largestStepBytes).toBeLessThan(256)
  expect(fixture.largestEventBytes).toBeLessThan(2048)
  expect({state:ui.info(0),incomplete:ui.info(3),roots:ui.info(1)}).toEqual({state:3,incomplete:0,roots:263})
  assertTables(ui,fixture.rows)
  const projected=ui.text(1)
  for(const issue of fixture.planned) {
    expect(['resolved','closed']).toContain(fixture.oracle.by.get(issue.id)!.status)
    expect(projected).toContain(fixture.oracle.alias.get(issue.id)+'|x|history-')
  }
}

test('plan projection capacity retains a legitimate 45 KiB table including resolved-task plans',()=>{
  const fixture=planProjectionInventory(45*1024),ui=new NativeUI()
  try {
    ui.connect();ui.pump(fixture.rows,{cap:37})
    const diagnostic={targetBytes:fixture.targetBytes,independentlySized:fixture.independentlySized,
      oracleBytes:Buffer.byteLength(fixture.oracle.plans),plannedFinishedTasks:fixture.planned.length,
      largestStepBytes:fixture.largestStepBytes,largestEventBytes:fixture.largestEventBytes,
      native:{state:ui.info(0),incomplete:ui.info(3),roots:ui.info(1),known:ui.info(14)}}
    writeFileSync('/tmp/coax-capacity-prep/plan-projection-45k.json',JSON.stringify(diagnostic,null,2)+'\n')
    console.log('plan projection capacity diagnostic',JSON.stringify(diagnostic))
    expectPlanProjection(ui,fixture)
  } finally {ui.close()}
})

test('plan projection capacity accepts 65536 bytes and refuses 65537 atomically',()=>{
  const accepted=planProjectionInventory(65536),overflow=planProjectionInventory(65537),ui=new NativeUI()
  expect(accepted.roots.map(e=>e.id)).toEqual(overflow.roots.map(e=>e.id))
  expect(overflow.independentlySized).toBe(65537)
  expect(Buffer.byteLength(overflow.oracle.plans)).toBe(65537)
  expect(overflow.oracle.rows).toBe(accepted.oracle.rows)
  expect(overflow.oracle.details).toBe(accepted.oracle.details)
  expect(overflow.oracle.log).toBe(accepted.oracle.log)
  expect(overflow.oracle.planInfo).toBe(accepted.oracle.planInfo)
  expect(overflow.largestStepBytes).toBeLessThan(256)
  expect(overflow.largestEventBytes).toBeLessThan(2048)
  try {
    ui.connect();ui.pump(accepted.rows,{cap:37})
    expectPlanProjection(ui,accepted)
    const previous=Array.from({length:5},(_,k)=>ui.text(k))
    ui.reconnect();ui.pump(overflow.rows,{cap:37})
    expect({state:ui.info(0),incomplete:ui.info(3),roots:ui.info(1)}).toEqual({state:4,incomplete:5,roots:263})
    expect(Array.from({length:5},(_,k)=>ui.text(k))).toEqual(previous)
    expect(Buffer.byteLength(ui.text(1))).toBe(65536)
    writeFileSync('/tmp/coax-capacity-prep/plan-projection-64k-boundary.json',JSON.stringify({acceptedBytes:65536,rejectedBytes:65537,
      roots:263,retainedFinishedPlans:accepted.planned.length,largestStepBytes:overflow.largestStepBytes,largestEventBytes:overflow.largestEventBytes,
      state:ui.info(0),incomplete:ui.info(3),priorTablesPreserved:true},null,2)+'\n')
  } finally {ui.close()}
})

test('inventory capacity native scalar and string policies agree with independent row counts',()=>{
  for(const count of [-1,0,1,319,320,321])for(const maximum of [-1,0,1,319,320,321]) {
    expect(Boolean(lib.symbols.probe_count_fits(BigInt(count),BigInt(maximum)))).toBe(count>=0 && count<=maximum)
  }
  const texts=['','row','row\n','\n','\n\n','row\nnext','row\n\n','\r','row\r\nnext','\n'.repeat(320),'\n'.repeat(321)]
  for(const text of texts) {
    const count=text===''?0:(text.endsWith('\n')?text.slice(0,-1):text).split('\n').length
    const bytes=cString(text)
    for(const maximum of [-1,0,1,2,319,320,321]) {
      expect(Boolean(lib.symbols.probe_rows_fit(ptr(bytes),BigInt(bytes.length-1),BigInt(maximum)))).toBe(count<=maximum)
    }
  }
})

test('inventory capacity refuses oversized baked and live candidates before any state replacement',()=>{
  const ui=new NativeUI()
  try {
    expect(lib.symbols.probe_choose_refused(ui.state)).toBe(1)
    expect(lib.symbols.probe_live_refused(ui.state,0)).toBe(1)
    expect(lib.symbols.probe_live_refused(ui.state,1)).toBe(1)
  } finally {ui.close()}
})

test('inventory capacity keeps 262 and 263 signed roots with hierarchy and resolved history',()=>{
  for(const count of [262,263]) {
    const fixture=capacityInventory(count),ui=new NativeUI()
    try {
      ui.connect();ui.pump(fixture.rows,{cap:37,caps:[37,13,5],reverse:true})
      expectCapacityView(ui,fixture)
    } finally {ui.close()}
  }
})

test('inventory capacity accepts exactly 320 roots and refuses 321 without replacing any complete table',()=>{
  const accepted=capacityInventory(320),overflow=capacityInventory(321),ui=new NativeUI()
  expect(overflow.roots.slice(0,320).map(e=>e.id)).toEqual(accepted.roots.map(e=>e.id))
  // All 321 events are valid to the independent unbounded verified TS fold.
  expect(expected(overflow.rows).by.size).toBe(321)
  try {
    ui.connect();ui.pump(accepted.rows,{cap:37})
    expectCapacityView(ui,accepted)
    const previous=Array.from({length:5},(_,k)=>ui.text(k))
    ui.reconnect();ui.pump(overflow.rows,{cap:37})
    expect(ui.info(0)).toBe(4)
    expect(ui.info(3)).toBe(5)
    expect(ui.info(1)).toBe(320)
    expect(Array.from({length:5},(_,k)=>ui.text(k))).toEqual(previous)
    expect(ui.text(2)).not.toContain(overflow.roots[320].id)
  } finally {ui.close()}
})

test('homepage LandingReader Wasm accepts 320 known roots, refuses 321 and recovers without partial JSON',async()=>{
  const cachePath=join(app,'build/landing-reader.wasm.json'),artifactPath=join(app,'build/landing-reader.wasm')
  let previousCache:any
  try {previousCache=JSON.parse(readFileSync(cachePath,'utf8'))} catch {}
  // Import through the real producer so its ordinary source/artifact cache
  // validation rebuilds the reader when the shared Fold source changed.
  const {reader:landing,LandingReader}=await import('./landing-reader')
  expect(landing).toBeInstanceOf(LandingReader)
  const accepted=capacityInventory(320),overflow=capacityInventory(321),known=accepted.roots.map(e=>e.id)
  const authoritative=fold(pk,config,accepted.rows)
  const expectedIssues=authoritative.issues.map(i=>({id:i.id,kind:i.kind,status:i.status,title:i.title,parent:i.parent,
    rank:i.rank||'~',claimed_by:i.claimed_by,claimed_at:i.claimed_at,commit:i.commit,status_at:i.status_at})).sort((a,b)=>a.id.localeCompare(b.id))
  const selected=(result:{issues:any[]})=>[...result.issues].sort((a,b)=>a.id.localeCompare(b.id))
  const first=landing.fold(accepted.rows,pk,'coax',known)
  expect(first.issues.length).toBe(320)
  expect(selected(first)).toEqual(expectedIssues)
  expect(expected(overflow.rows).by.size).toBe(321)
  expect(()=>landing.fold(overflow.rows,pk,'coax',known)).toThrow('Coax refused complete project fold')
  expect(landing.x.nl_output_len()).toBe(0)
  expect(landing.output()).toBe('')
  const recovered=landing.fold(accepted.rows,pk,'coax',known)
  expect(recovered.issues.length).toBe(320)
  expect(selected(recovered)).toEqual(expectedIssues)
  const hash=(path:string)=>new Bun.CryptoHasher('sha256').update(readFileSync(path)).digest('hex')
  const cache=JSON.parse(readFileSync(cachePath,'utf8')),wasm=hash(artifactPath)
  expect(cache.wasm).toBe(wasm)
  writeFileSync('/tmp/coax-capacity-prep/landing-reader-320.json',JSON.stringify({accepted:320,rejected:321,known:known.length,
    recovered:recovered.issues.length,metadataParity:true,partialOutputBytes:0,normalArtifactCache:true,
    cacheSourceChanged:previousCache?.source!==cache.source,artifactSource:cache.source,wasm,
    sources:{projectFold:hash(join(app,'project-fold.coax')),landingFold:hash(join(import.meta.dir,'landing-fold.coax')),
      landingReader:hash(join(import.meta.dir,'landing-reader.ts')),compiler:hash(join(root,'coax/bin/coax'))}},null,2)+'\n')
},120000)
