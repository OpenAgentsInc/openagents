// Firm reason: 1 (browser SDK effects: real authored offers, shared Worklet ownership, muted device pause/resume/close).
globalThis.hostOpusDemand=async()=>{
 const {openCoaxAudio}=await import('/coaxial/audio-host.js'),{openHostServices,openPcmMemory}=await import('/coaxial/services-host.js');
 const services=await openHostServices(),memory=openPcmMemory(services),context=new AudioContext({sampleRate:48000});await context.suspend();
 const adapter=await openCoaxAudio(context,context.destination,(name,wav,asset)=>name==='room'?wav(W.files['room.wav']):({url:asset.url}),{services,memory});await adapter.ready;
 const plan=JSON.parse(new TextDecoder().decode(W.files['demand.json']));let count=0;const results=[];
 for(const row of plan.rows){globalThis.hostOpusDemandStage=row.line;const result=await adapter.play(row.line,false);results.push({line:row.line,result});if(result>=0)count++;}
 const before=await adapter.info(),rows=[before.assetCount,before.memoryWords,before.words,before.allocationRefusals,services.audioQueueInfo()[0],before.deviceFrames,before.frames,before.roomPartitions];
 globalThis.hostOpusDemandStage='gesture';await new Promise((ok,no)=>addEventListener('keydown',()=>context.resume().then(ok,no),{once:true}));let running;const until=performance.now()+10000;do{running=await Promise.race([adapter.info(),new Promise((_,reject)=>setTimeout(()=>reject(Error('audio clock observation timeout')),1000))]);if(running.frames>0&&running.callbacks>0)break;await new Promise(resolve=>setTimeout(resolve,25));}while(performance.now()<until);
 await context.suspend();const paused=await adapter.info();await new Promise(resolve=>setTimeout(resolve,100));const held=await adapter.info();
 const closed=await adapter.close();await context.close();rows.push(closed.memoryAfter,closed.nodeAfter);
 return {rows,clock:[running.frames,running.callbacks,paused.frames,paused.callbacks,held.frames,held.callbacks,closed.memoryAfter,closed.nodeAfter,Number(context.state==='closed')],acceptedOffers:count,before,running,paused,held,closed,results};
};
'ready'
