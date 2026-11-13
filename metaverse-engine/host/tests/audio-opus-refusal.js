// Firm reason: 1 (browser SDK effects: actual admission refusals and a held real decode completion across retirement).
globalThis.hostOpusRefusal=async()=>{
 const {openCoaxAudio}=await import('/coaxial/audio-host.js'),{openHostServices,openPcmMemory}=await import('/coaxial/services-host.js');
 const services=await openHostServices(),memory=openPcmMemory(services),context=new AudioContext({sampleRate:48000});await context.suspend();
 let mode='full',calls=0,entered,release;const began=new Promise(resolve=>entered=resolve),gate=new Promise(resolve=>release=resolve),decode=context.decodeAudioData.bind(context);
 context.decodeAudioData=async bytes=>{calls++;const buffer=await decode(bytes);if(mode==='shape')return {length:buffer.length-1,numberOfChannels:buffer.numberOfChannels,sampleRate:buffer.sampleRate};if(mode==='held'){entered();await gate;}return buffer;};
 const bad=W.files['input.apc'].slice();bad[64]^=1;
 const adapter=await openCoaxAudio(context,context.destination,name=>name==='fire-hit-e2'?{opus:W.files['canonical.ogg'],companion:bad}:{opus:mode==='source'?Uint8Array.of(1):W.files['sdk.ogg']},{services,memory});await adapter.ready;
 const full=memory.claim(8388608,1),rows=[await adapter.play('!fetch fire-cast-e1',false)];let info=await adapter.info();rows.push(info.memoryWords,info.words,info.allocationRefusals,services.audioQueueInfo()[0]);full.release();
 mode='companion';rows.push(await adapter.play('!fetch fire-hit-e2',false));info=await adapter.info();rows.push(info.memoryWords,info.words);
 mode='source';rows.push(await adapter.play('!fetch fire-cast-e1',false));info=await adapter.info();rows.push(info.memoryWords,info.words);
 mode='shape';rows.push(await adapter.play('!fetch fire-cast-e1',false));info=await adapter.info();rows.push(info.memoryWords,info.words,info.allocationRefusals);
 mode='held';const command=adapter.play('!fetch fire-cast-e1',false);await began;rows.push(calls,memory.total());let settled=false;const retirement=adapter.close().then(value=>{settled=true;return value;});await new Promise(resolve=>setTimeout(resolve,75));rows.push(memory.total(),Number(settled));release();rows.push(await command);const closed=await retirement;rows.push(closed.memoryAfter,closed.nodeAfter,services.audioQueueInfo()[0]);await context.close();rows.push(Number(context.state==='closed'));return rows;
};
'ready'
