// Independent Node crypto composition, solely public negative/boundary fixtures.
import{seal,openEntry,context,phrase,pointer}from'../../../coax/tests/nostr/phrase_gate_reference.ts';
import{readFileSync}from'node:fs';
if(process.argv[2]==='verify-browser'){
 const report=JSON.parse(readFileSync(process.argv[3],'utf8'));for(const entry of report.sealed){const value=openEntry(entry,phrase,context);if(JSON.stringify(value)!==JSON.stringify(pointer))throw Error('Browser cryptogram differs from independent Node oracle');}console.log(report.sealed.length+' fresh browser packets opened by independent Node crypto');
}else{
 const rows=[];const entry=(wire:Buffer)=>JSON.stringify({v:'openagents.phrase-gate.v2',payload:wire.toString('base64')});
 const add=(id:string,wire:Buffer,error:number,expected?:Buffer,relay=pointer.relay)=>rows.push({id,message:entry(wire),error,expected:expected?.toString('hex'),relay});
 add('complete-4096',seal(Buffer.alloc(4096,32)).wire,5);add('complete-4097',seal(Buffer.alloc(4097,32)).wire,1);
 const packet=seal(Buffer.from(JSON.stringify(pointer))).wire;
 for(const n of[0,1,73,74,106])add('header-truncated-'+n,packet.subarray(0,n),1);
 const bad=Buffer.from(packet);bad[0]^=1;add('magic',bad,3);
 for(let at=0;at<32;at++){const bad=Buffer.from(packet);bad[packet.length-32+at]^=1;add('mac-position-'+at,bad,10);}
 for(const text of['0','{"id":0}','{"id":"'+pointer.id+'","\\u0069d":"'+pointer.id+'","recipient":"'+pointer.recipient+'","relay":"'+pointer.relay+'"}','{"id":"'+pointer.id+'","recipient":"'+pointer.recipient+'","relay":"wss://a/\\u0000"}'])add('semantic-'+rows.length,seal(Buffer.from(text)).wire,5);
 const relay='wss://example.com:443/p%20q?x=1&y=@',plain=Buffer.from(JSON.stringify({...pointer,relay}));add('dns-query-positive',seal(plain).wire,0,plain,relay);
 console.log(JSON.stringify(rows));
}
