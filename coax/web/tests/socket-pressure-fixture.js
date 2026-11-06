// Firm reason: 1 (browser glue: real WebSocket/host effects and raw lease observations; Coax admits the receipt).
import {openSocketRetention,makeSocketQueues} from '../socket-queue-host.js';
import {workerRoomSocket} from '../room-worker.js';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(test,label,ms=5000){const end=performance.now()+ms;while(!test()){if(performance.now()>end)throw Error(`socket pressure:${label}`);await sleep(5);}}
export async function socketLocalFixture(){
 const rows=()=>Array.from({length:Number(coaxial.hostProbe(-1,2))},(_,i)=>Array.from({length:8},(_,f)=>Number(coaxial.hostProbe(i,f)))),info=()=>coaxial.socketRetention().active;
 coaxial.hostReset();coaxial.send(910,'local:/api/room/relay/everglade/local-1');await wait(()=>rows().some(r=>r[0]===4&&r[1]===256),'actual room worker open',10000);
 const before=coaxial.roomHost.acknowledgedFrames||0;coaxial.holdSocketSending(true);for(let i=0;i<64;i++)coaxial.send(911,'0 ');const held=info();coaxial.holdSocketSending(false);
 await wait(()=>info().output[0]===0,'real room ACK drain');const active=info(),acked=(coaxial.roomHost.acknowledgedFrames||0)-before,failed=coaxial.roomHost.failed.length;coaxial.send(912,'0');await wait(()=>rows().some(r=>r[0]===4&&r[1]===258),'actual room close');
 return [...held.output.slice(0,2),acked,...active.output.slice(0,2),Number(!coaxial.roomHost.native),failed,info().output[0]];
}
export async function socketPressureFixture(){
 const out={},rows=()=>Array.from({length:Number(coaxial.hostProbe(-1,2))},(_,i)=>Array.from({length:8},(_,f)=>Number(coaxial.hostProbe(i,f)))),info=()=>coaxial.socketRetention().active;
 const callbacks=()=>rows().filter(r=>r[0]===4),count=kind=>callbacks().filter(r=>r[1]===kind).length;
 async function open(path){coaxial.holdSocketDelivery(false);coaxial.holdSocketSending(false);coaxial.hostReset();coaxial.send(910,`ws://127.0.0.1:9185/${path}`);await wait(()=>count(256)===1,'open');coaxial.hostReset();}
 for(const path of ['events','bytes']){
  await open(path);coaxial.holdSocketDelivery(true);const base=info().overflow;coaxial.send(911,'0 go');
  if(path==='events'){const at=performance.now();while(performance.now()-at<150){}out.stall_ms=Math.floor(performance.now()-at);}
  await wait(()=>info().terminals===1,`${path} full`);const held=info();coaxial.holdSocketDelivery(false);await wait(()=>count(258)===1,`${path} terminal`);const end=info();
  out[path]=[...held.input.slice(0,2),held.events,held.terminals,held.overflow-base,count(257),count(259),count(258),end.input[0],end.output[0],end.terminals,Number(coaxial.hostProbe(-1,3))];
  if(path==='bytes')out.byteHashes=callbacks().filter(r=>r[1]===257).map(r=>r[7]);
 }
 await open('unicode');coaxial.send(911,'0 go');await wait(()=>count(259)===1&&count(257)===1,'unicode after');const u=callbacks().filter(r=>r[1]>=256&&r[1]<272),f=u.find(r=>r[1]===257),e=u.find(r=>r[1]===259);
 out.unicode=[u.length,e[6],count(259),count(257),f[6],f[7],u.filter(r=>r[1]===257&&r[6]!==5).length,info().input[0]];
 coaxial.send(912,'0');await wait(()=>count(258)===1,'unicode close');
 await open('idle');coaxial.holdSocketSending(true);const baseline=info().overflow;for(let i=0;i<64;i++)coaxial.send(911,'0 ');const full=info();coaxial.send(911,'0 ');const failed=info();await wait(()=>count(258)===1,'command close');
 out.commands=[...full.output.slice(0,2),full.commands,failed.terminals,failed.overflow-baseline,failed.output[0],info().input[0],count(259),count(258)];coaxial.holdSocketSending(false);
 await open('hold');coaxial.holdSocketDelivery(true);coaxial.send(911,'0 go');await wait(()=>info().input[0]===4,'retained before replacement');const held=info();await coaxial.reloadEngine();await wait(()=>coaxial.ready&&!!info(),'replacement ready');coaxial.holdSocketDelivery(false);await sleep(200);const retired=coaxial.socketRetention().retired;
 out.retirement=[...held.input.slice(0,2),held.events,held.terminals,held.output[0],Number(retired.retired),retired.input[0],retired.output[0],retired.events,info().input[0],info().output[0],count(257)];
 const retained=await openSocketRetention(128),ws=new WebSocket('ws://127.0.0.1:9185/buffer');let sending=false,closed=false,delivered=0;
 const q=makeSocketQueues(retained,{current:()=>true,closed(){},deliver(){delivered++;},schedule(){},canSend:()=>sending});
 ws.onmessage=m=>{if(typeof m.data==='string')q.receive(ws,0,1,m.data);};ws.onclose=()=>{closed=true;q.receive(ws,0,2);};ws.onerror=()=>q.receive(ws,0,3);
 try{
  await new Promise((yes,no)=>{ws.onopen=yes;ws.onerror=()=>no(Error('physical buffer peer opening'));});const memory=new WebAssembly.Memory({initial:129});new Uint8Array(memory.buffer).fill(65);
  for(let i=0;i<4;i++)q.send(ws,0,memory,8,8388608);const before=q.info();sending=true;await wait(()=>q.info().buffered.length===4&&q.info().buffered.every(r=>r.sent)&&ws.bufferedAmount>0,'actual SDK buffered bytes');const buffered=q.info();
  out.sdk={bufferedAmount:ws.bufferedAmount,rows:buffered.buffered};q.reset();ws.close();await wait(()=>closed,'physical peer closed',6000);q.drain();const final=q.info();
  out.wire=[...before.output.slice(0,2),buffered.buffered.length,Number(out.sdk.bufferedAmount>0),final.output[0],final.output[1],Number(closed),delivered];out.sdk.closedBufferedAmount=ws.bufferedAmount;
 }finally{q.reset();ws.close();}
 // Real Worker effects with an explicit stalled consumer; no room simulation oracle.
 const code="onmessage=e=>{const m=e.data;if(m.t==='open')postMessage({t:'open'});if(m.t==='frame')setTimeout(()=>postMessage({t:'sent',ack:m.ack}),500);}";
 const url=URL.createObjectURL(new Blob([code],{type:'text/javascript'})),w=new Worker(url),s=workerRoomSocket(w,0,'/fixture',()=>{});let opened=false;
 w.onmessage=e=>{if(e.data.t==='open'){s.readyState=1;opened=true;}if(e.data.t==='sent')s.acknowledge(e.data.ack);};
 const owned=await openSocketRetention(128),queue=makeSocketQueues(owned,{current:()=>true,closed(){},deliver(){},schedule(){}});
 try{await wait(()=>opened,'worker open');const mem=new WebAssembly.Memory({initial:1});for(let i=0;i<64;i++)queue.send(s,0,mem,0,0);await wait(()=>s.bufferedCommands===64,'worker pending empties');const held=queue.info();await wait(()=>queue.info().output[0]===0,'actual worker replies');out.worker=[held.output[0],held.output[1],held.buffered[0].bufferedCommands,queue.info().output[0],s.bufferedCommands,s.bufferedAmount];}
 finally{queue.reset();s.close();w.terminate();URL.revokeObjectURL(url);}
 return out;
}
