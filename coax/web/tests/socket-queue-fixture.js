// Firm reason: 1 (browser SDK fixture: raw copies, stalled effects and retirement observations; Coax decides the verdict).
import {openSocketRetention,makeSocketQueues} from '../socket-queue-host.js';
export async function socketQueueFixture(){
 const results={};
 async function fixture(capacity){
  const retained=await openSocketRetention(capacity),sockets=new Array(retained.workers).fill(null),events=[],sent=[];
  let delivery=false,sending=false;
  const make=id=>(sockets[id]={readyState:1,bufferedAmount:0,closes:0,send(text){sent.push(new TextEncoder().encode(text));},close(){this.closes++;this.readyState=3;}});
  const queue=makeSocketQueues(retained,{current:(ws,id)=>sockets[id]===ws,closed:(ws,id)=>{if(sockets[id]===ws)sockets[id]=null;},deliver:(ws,id,event,bytes)=>events.push({id,event,bytes:bytes.slice()}),schedule(){},canDeliver:()=>delivery,canSend:()=>sending});
  return {retained,queue,events,sent,make,setDelivery:v=>{delivery=v;},setSending:v=>{sending=v;}};
 }
 {
  const f=await fixture(8),ws=f.make(0),q=f.queue;
  results.plan=[f.retained.workers,f.retained.events,...f.retained.info(false).slice(0,4),...f.retained.info(true).slice(0,4)];
  q.receive(ws,0,1,'é🙂');q.receive(ws,0,1,'two');const before=q.info();q.drain();const held=q.info();
  f.setDelivery(true);q.drain();const after=q.info();results.copy=[...before.input.slice(0,2),before.events,...held.input.slice(0,2),...after.input.slice(0,2),f.events.length,...f.events.flatMap(e=>[...e.bytes])];
  q.receive(ws,0,1,'é'.repeat(5));const oversize=q.info();q.drain();const e=f.events.at(-1);results.oversize=[...oversize.input.slice(0,2),e.event,e.bytes.length,...q.info().input.slice(0,2)];q.reset();
 }
 {
  const f=await fixture(8),ws=f.make(0),q=f.queue;for(let i=0;i<4;i++)q.receive(ws,0,1,'12345678');const full=q.info();q.receive(ws,0,1,'x');const failed=q.info();f.setDelivery(true);q.drain();
  results.bytes=[...full.input.slice(0,2),failed.terminals,ws.closes,...failed.input.slice(0,2),...q.info().input.slice(0,2),f.events.length,...f.events.slice(-2).map(e=>e.event)];q.reset();
 }
 {
  const f=await fixture(0),ws=f.make(0),q=f.queue;for(let i=0;i<f.retained.events;i++)q.receive(ws,0,1,'');const full=q.info();q.receive(ws,0,1,'');const failed=q.info();q.reset();
  results.events=[...full.input.slice(0,2),failed.terminals,ws.closes,...q.info().input.slice(0,2),q.info().terminals,f.events.length];
 }
 {
  const f=await fixture(8),ws=f.make(0),q=f.queue,memory=new WebAssembly.Memory({initial:1}),storage=new Uint8Array(memory.buffer),encoder=new TextEncoder();
  storage.set(encoder.encode('first'),8);q.send(ws,0,memory,8,5);storage.fill(120,8,13);storage.set(encoder.encode('é🙂'),16);q.send(ws,0,memory,16,6);const held=q.info();f.setSending(true);
  const end=performance.now()+2000;while(q.info().output[0]&&performance.now()<end)await new Promise(r=>setTimeout(r,5));
  results.sendCopy=[...held.output.slice(0,2),f.sent.length,...f.sent.flatMap(b=>[b.length,...b]),...q.info().output.slice(0,2)];q.reset();
 }
 {
  const f=await fixture(8),ws=f.make(0),q=f.queue,memory=new WebAssembly.Memory({initial:1});for(let i=0;i<f.retained.events;i++)q.send(ws,0,memory,0,0);const full=q.info();q.send(ws,0,memory,0,0);const failed=q.info();
  results.sendFull=[...full.output.slice(0,2),...failed.output.slice(0,2),failed.terminals,ws.closes,f.sent.length];q.reset();
 }
 {
  const f=await fixture(8),ws=f.make(0),q=f.queue,memory=new WebAssembly.Memory({initial:1});q.receive(ws,0,1,'abc');q.send(ws,0,memory,8,4);const held=q.info();q.reset();const closed=q.info();
  results.retire=[...held.input.slice(0,2),...held.output.slice(0,2),...closed.input.slice(0,2),...closed.output.slice(0,2),Number(closed.retired),Number(!q.receive(ws,0,1,'x'))];
 }
 {
  const retention=await openSocketRetention(8),ws={},inside=[];let q;
  q=makeSocketQueues(retention,{current:()=>true,closed(){},schedule(){},canDeliver:()=>true,deliver(){q.reset();const held=q.info();inside.push(...held.input.slice(0,2),held.delivering);}});
  q.receive(ws,0,1,'x');q.drain();const closed=q.info();results.borrow=[...inside,...closed.input.slice(0,2),closed.delivering,Number(closed.retired)];
 }
 {
  const f=await fixture(8),peers=Array.from({length:f.retained.workers},(_,id)=>f.make(id)),q=f.queue;for(let i=0;i<f.retained.events;i++)q.receive(peers[0],0,1,'');for(let id=0;id<peers.length;id++)q.receive(peers[id],id,1,'');
  for(let i=0;i<100;i++)q.receive(peers[0],0,1,'ignored');const full=q.info();q.reset();const closed=q.info();results.terminals=[...full.input.slice(0,2),full.terminals,peers.reduce((n,p)=>n+p.closes,0),...closed.input.slice(0,2),closed.terminals];
 }
 {
  const f=await fixture(8),ws=f.make(0),q=f.queue,extent=f.retained.info(true)[2],memory=new WebAssembly.Memory({initial:Math.ceil((extent+8)/65536)});
  for(let i=0;i<4;i++)q.send(ws,0,memory,8,extent);const full=q.info();q.send(ws,0,memory,8,extent);const failed=q.info();
  results.outBytes=[...full.output.slice(0,2),full.allocations,...failed.output.slice(0,2),failed.terminals,ws.closes];q.reset();
 }
 {
  const f=await fixture(8),ws=f.make(0),q=f.queue,memory=new WebAssembly.Memory({initial:1});q.send(ws,0,memory,65535,2);const failed=q.info();results.span=[...failed.output.slice(0,2),failed.allocations,failed.terminals,ws.closes];q.reset();
 }
 return results;
}
