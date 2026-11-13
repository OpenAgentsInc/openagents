// Firm reason: 1 (browser SDK effects: actual bounded Worklet allocation and deferred decode/cleanup acknowledgements).
globalThis.workletMemoryFixture=async()=>{
 globalThis.workletMemoryStage='imports';
 const {openHostServices,openPcmMemory}=await import('/coaxial/services-host.js');
 const {openCoaxAudio}=await import('/coaxial/audio-host.js');
 const services=await openHostServices(),memory=openPcmMemory(services),context=new AudioContext({sampleRate:48000});globalThis.workletMemoryStage='suspend';await context.suspend();
 const wav=Uint8Array.of(82,73,70,70,52,0,0,0,87,65,86,69,102,109,116,32,16,0,0,0,1,0,1,0,128,187,0,0,0,119,1,0,2,0,16,0,100,97,116,97,16,0,0,0,255,127,0,0,0,0,0,0,0,0,0,0,0,0,0,0);
 const held=memory.claim(8388608,1);globalThis.workletMemoryStage='open';const adapter=await openCoaxAudio(context,context.destination,()=>({wav}),{services,memory});globalThis.workletMemoryStage='ready';await adapter.ready;
 globalThis.workletMemoryStage='refusal';
 const refused=await adapter.play('!fetch pulse',false),full=await adapter.info(),rows=[refused,full.memoryWords,full.words,full.allocationRefusals,services.audioQueueInfo()[0]];
 held.release();globalThis.workletMemoryStage='install';rows.push(await adapter.play('!fetch pulse',false));const installed=await adapter.info();rows.push(installed.memoryWords,installed.words,services.audioQueueInfo()[0]);
 globalThis.workletMemoryStage='close';const closed=await adapter.close();rows.push(closed.memoryBefore,closed.memoryAfter,closed.nodeBefore,closed.nodeAfter);
 await context.close();const context2=new AudioContext({sampleRate:48000});await context2.suspend();
 const services2=await openHostServices(),memory2=openPcmMemory(services2);let release,entered;
 const gate=new Promise(resolve=>release=resolve),began=new Promise(resolve=>entered=resolve);
 globalThis.workletMemoryStage='open next';const next=await openCoaxAudio(context2,context2.destination,async()=>{entered();await gate;return {wav};},{services:services2,memory:memory2});await next.ready;
 globalThis.workletMemoryStage='deferred';const command=next.play('!fetch deferred',false);await began;rows.push(services2.audioQueueInfo()[0]);globalThis.workletMemoryStage='close next';const retirement=await next.close();rows.push(retirement.memoryAfter,retirement.nodeAfter);
 release();rows.push(await command,services2.audioQueueInfo()[0]);let refusedAfter=false;try{await next.info();}catch{refusedAfter=true;}rows.push(Number(refusedAfter));
 await context2.close();rows.push(Number(context.state==='closed'&&context2.state==='closed'));return rows;
};
'ready'
