import {test,expect} from 'bun:test'
import {makeHttpRequests} from './http-requests.js'

function deferred(){let resolve:any,reject:any;const promise=new Promise((a,b)=>{resolve=a;reject=b});return {promise,resolve,reject}}
const bytes=(s:string)=>new TextEncoder().encode(s)
async function flush(){for(let i=0;i<18;i++)await Promise.resolve()}
function harness(capacity=64){
 const calls:any[]=[],events:any[]=[],timers=new Map<number,()=>void>();let next=0
 const http=makeHttpRequests({origin:'https://example.test',capacity:()=>capacity,player:()=>({key:'player-test'}),
  fetch:(url:string,init:any)=>{const d=deferred();calls.push({url,init,...d});return d.promise},
  emit:(id:number,text:string)=>events.push({id,text}),
  setTimer:(fn:()=>void,_ms:number)=>{timers.set(++next,fn);return next},clearTimer:(id:number)=>timers.delete(id)})
 return {http,calls,events,timers}
}

test('legacy requests preserve same-origin cookies, body bytes, player and explicit auth headers',async()=>{
 const h=harness();const body=bytes('Nostr abc\n{}');expect(h.http.request('/api/example',body,1|4|8|16)).toBe(0);body.fill(0)
 await flush();expect(h.calls[0].init.credentials).toBe('same-origin');expect(h.calls[0].init.method).toBe('POST')
 expect(h.calls[0].init.headers).toEqual({'Content-Type':'application/json','X-Verse-Player':'player-test',Authorization:'Nostr abc'})
 expect(new TextDecoder().decode(h.calls[0].init.body)).toBe('{}')
 h.calls[0].resolve(new Response('ok',{status:201}));await flush();expect(h.events).toEqual([{id:0,text:'201\nok'}]);expect(h.timers.size).toBe(0)
})
test('legacy GET, fire-and-forget and successful beacon keep their existing shapes',async()=>{
 const h=harness();h.http.request('/read',null,4);h.http.request('/post',bytes('{}'),1)
 await flush();expect(h.calls[0].init.method).toBe('GET');expect(h.calls[0].init.body).toBeUndefined();expect(h.calls[1].init.keepalive).toBe(true)
 h.calls[0].resolve(new Response('yes'));h.calls[1].resolve(new Response('ignored'));await flush();expect(h.events).toEqual([{id:0,text:'200\nyes'}])
 const seen:any[]=[];const beacon=makeHttpRequests({origin:'https://example.test',fetch:()=>{throw Error('beacon must not fetch')},sendBeacon:(url:string,body:Blob)=>{seen.push({url,body});return true},emit:()=>{throw Error('beacon must not answer')}})
 expect(beacon.request('/beacon',bytes('x'),3)).toBe(0);expect(await seen[0].body.text()).toBe('x');expect(seen[0].url).toBe('/beacon')
})
test('eight waiting slots are shared between old and bounded requests and released exactly once',async()=>{
 const h=harness();for(let i=0;i<8;i++)expect(h.http.request('/r',null,4,i%2?100:undefined)).toBe(i)
 expect(h.http.request('/full',null,4,100)).toBe(-1);await flush()
 h.calls[3].resolve(new Response('done'));await flush();expect(h.events).toEqual([{id:3,text:'200\ndone'}])
 expect(h.http.request('/next',null,4,100)).toBe(3);h.http.reset()
})
test('cancel invalidates before abort and late completion cannot enter its reused slot',async()=>{
 const h=harness();expect(h.http.request('/old',null,4,100)).toBe(0);await flush();const oldTimer=[...h.timers.values()][0]
 expect(h.http.cancel(0)).toBe(1);expect(h.calls[0].init.signal.aborted).toBe(true);expect(h.http.cancel(0)).toBe(0)
 expect(h.http.request('/new',null,4,100)).toBe(0);await flush();oldTimer()
 h.calls[0].resolve(new Response('OLD'));await flush();expect(h.events).toEqual([])
 h.calls[1].resolve(new Response('NEW'));await flush();expect(h.events).toEqual([{id:0,text:'200\nNEW'}]);expect(h.timers.size).toBe(0)
})
test('cancel and reset before the async fetch starts perform no stale network effect',async()=>{
 const h=harness();h.http.request('/old',null,4,100);expect(h.http.cancel(0)).toBe(1)
 h.http.request('/another',null,4,100);h.http.reset();await flush();expect(h.calls).toEqual([]);expect(h.events).toEqual([])
})
test('timeout emits one terminal failure, aborts and fences a late completion',async()=>{
 const h=harness();h.http.request('/slow',null,4,100);await flush();const timer=[...h.timers.values()][0];timer();timer()
 expect(h.events).toEqual([{id:0,text:'0\n'}]);expect(h.calls[0].init.signal.aborted).toBe(true)
 h.calls[0].reject(Error('late abort'));await flush();expect(h.events).toHaveLength(1);expect(h.http.request('/reuse',null,4,100)).toBe(0);h.http.reset()
})
test('cancellation while reading the body cannot deliver to a successor',async()=>{
 const h=harness();const body=deferred();h.http.request('/old',null,4,100);await flush()
 h.calls[0].resolve({status:200,body:{getReader:()=>({read:()=>body.promise,cancel:()=>Promise.resolve()})}});await flush()
 h.http.cancel(0);h.http.request('/new',null,4,100);await flush();body.resolve({done:false,value:bytes('old')});await flush();expect(h.events).toEqual([])
 h.calls[1].resolve(new Response('new'));await flush();expect(h.events).toEqual([{id:0,text:'200\nnew'}])
})
test('engine or page reset fences both legacy and bounded replies',async()=>{
 const h=harness();h.http.request('/legacy',null,4);h.http.request('/bounded',null,4,100);await flush();h.http.reset()
 expect(h.calls.every(c=>c.init.signal.aborted)).toBe(true);expect(h.timers.size).toBe(0)
 h.calls[0].resolve(new Response('old'));h.calls[1].resolve(new Response('old'));await flush();expect(h.events).toEqual([])
 expect(h.http.request('/new-engine',null,4,100)).toBe(0);h.http.reset()
})
test('a legacy response body already being read is fenced by cancellation and slot reuse',async()=>{
 const h=harness();const body=deferred();h.http.request('/old',null,4);await flush()
 h.calls[0].resolve({status:200,text:()=>body.promise});await flush();h.http.cancel(0);h.http.request('/new',null,4,100);await flush()
 body.resolve('old');await flush();expect(h.events).toEqual([])
 h.calls[1].resolve(new Response('new'));await flush();expect(h.events).toEqual([{id:0,text:'200\nnew'}])
})
test('a refused beacon falls back to the legacy keepalive fetch',async()=>{
 const calls:any[]=[];const h=makeHttpRequests({origin:'https://example.test',sendBeacon:()=>false,
  fetch:async(url:string,init:any)=>{calls.push({url,init});return new Response('ignored')},emit:()=>{throw Error('no answer wanted')}})
 expect(h.request('/beacon-fallback',bytes('{}'),3)).toBe(0);await flush();expect(calls).toHaveLength(1);expect(calls[0].init.keepalive).toBe(true)
})
test('bounded streaming counts UTF-8 bytes plus the status header and refuses overflow',async()=>{
 const h=harness(8);h.http.request('/fits',null,4,100);await flush();h.calls[0].resolve(new Response('éé'));await flush()
 expect(h.events).toEqual([{id:0,text:'200\néé'}])
 h.http.request('/too-big',null,4,100);await flush();h.calls[1].resolve(new Response('ééx'));await flush()
 expect(h.events[1]).toEqual({id:0,text:'0\n'});expect(h.calls[1].init.signal.aborted).toBe(true)
 const chunks=harness(10);chunks.http.request('/chunks',null,4,100);await flush()
 chunks.calls[0].resolve({status:200,body:new ReadableStream({start(c){c.enqueue(bytes('abcd'));c.enqueue(bytes('efg'));c.close()}})});await flush()
 expect(chunks.events).toEqual([{id:0,text:'0\n'}])
})
test('network errors and body errors finish once without keeping a timer or slot',async()=>{
 const h=harness();h.http.request('/fail',null,4,100);await flush();h.calls[0].reject(Error('offline'));await flush();expect(h.events).toEqual([{id:0,text:'0\n'}]);expect(h.timers.size).toBe(0)
 h.http.request('/body-fail',null,4,100);await flush();h.calls[1].resolve({status:200,body:{getReader:()=>({read:()=>Promise.reject(Error('read failed')),cancel:()=>Promise.resolve()})}});await flush()
 expect(h.events).toEqual([{id:0,text:'0\n'},{id:0,text:'0\n'}]);expect(h.http.cancel(0)).toBe(0)
})
test('bounded text refuses malformed UTF-8 rather than expanding it beyond the byte cap',async()=>{
 const h=harness(5);h.http.request('/invalid-text',null,4,100);await flush();h.calls[0].resolve(new Response(new Uint8Array([255])));await flush()
 expect(h.events).toEqual([{id:0,text:'0\n'}]);expect(h.calls[0].init.signal.aborted).toBe(true)
})
test('same-origin and bounded request effects refuse malformed arguments without reserving slots',async()=>{
 const h=harness();for(const url of ['https://elsewhere.test/x','//elsewhere.test/x','/\\elsewhere.test/x','/\n/elsewhere.test/x','relative'])expect(h.http.request(url,null,4,100)).toBe(-1)
 for(const timeout of [0,-1,NaN,Infinity,2147483648])expect(h.http.request('/x',null,4,timeout)).toBe(-1)
 expect(h.http.request('/x',null,0,100)).toBe(-1);expect(h.http.request('/x',null,6,100)).toBe(-1)
 expect(h.http.request('/x',bytes('no-newline'),20,100)).toBe(-1)
 expect(h.http.cancel(-1)).toBe(0);expect(h.http.cancel(8)).toBe(0);expect(h.http.cancel(0.5)).toBe(0)
 expect(h.http.request('/ok',null,4,1)).toBe(0);h.http.reset()
})
