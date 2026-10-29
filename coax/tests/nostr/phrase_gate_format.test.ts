import{test}from'node:test';import assert from'node:assert/strict';import{readFileSync}from'node:fs';
import{match}from'../../web/unlock/gate.ts';
import{PATH,fixture,open,openEntry,decodeBase64,normalizeV2,context,phrase,pointer,seal}from'./phrase_gate_reference.ts';
const pinned=JSON.parse(readFileSync(PATH,'utf8'));
test('draft bytes and actual legacy AES-GCM reader agree',async()=>{
 assert.deepEqual(fixture(),pinned);assert.deepEqual(await match(pinned.legacy.phrase,[pinned.legacy.entry],210000,true),pinned.legacy.pointer);
 assert.equal(await match('incorrectphrase',[pinned.legacy.entry],210000,true),null);
});
test('new public fixture opens exact canonical pointer only after its full MAC',()=>{
 let calls=0;assert.deepEqual(openEntry(pinned.v2.entry,phrase,context,()=>calls++),pointer);assert.equal(calls,1);
 for(const entry of [{...pinned.v2.entry,v:'openagents.phrase-gate.v3'},{...pinned.v2.entry,flags:0},{payload:pinned.v2.entry.payload},null])assert.throws(()=>openEntry(entry,phrase,context));
});
test('wrong phrase and every malformed field refuse at the declared boundary',()=>{
 for(const item of pinned.invalid){let calls=0;assert.throws(()=>open(Buffer.from(item.wireHex,'hex'),phrase,context,()=>calls++),undefined,item.case);assert.equal(calls,item.beforeDecrypt?0:1,item.case);}
 let calls=0;assert.throws(()=>open(Buffer.from(pinned.v2.wireHex,'hex'),'wrongdemophrase2026',context,()=>calls++));assert.equal(calls,0);
 assert.throws(()=>open(Buffer.from(pinned.v2.wireHex,'hex'),phrase,Buffer.alloc(0)));
});
test('v2 input is ASCII-only with explicit separator and length limits',()=>{
 assert.equal(normalizeV2('Coax demo-phrase2026'),phrase);assert.equal(normalizeV2('a'.repeat(16)),'a'.repeat(16));assert.equal(normalizeV2('a'.repeat(40)),'a'.repeat(40));
 for(const s of ['a'.repeat(15),'a'.repeat(41),'a'.repeat(16)+'_','a'.repeat(16)+'\0','a'.repeat(16)+'\t','K'.repeat(16),'a'.repeat(16)+'\u00a0',' '.repeat(113)+'a'.repeat(16)])assert.throws(()=>normalizeV2(s));
 assert.equal(normalizeV2(' '.repeat(112)+'a'.repeat(16)),'a'.repeat(16));
});
test('base64 spelling and untrusted zero/oversize work never enter decryption',()=>{
 for(const text of [pinned.v2.entry.payload+'\n',pinned.v2.entry.payload.slice(0,-1),'AR==','A'.repeat(5608),'!!!!'])assert.throws(()=>decodeBase64(text));
 for(const n of [0,4097]){const wire=Buffer.from(pinned.v2.wireHex,'hex');wire.writeUInt32BE(n,10);let calls=0;assert.throws(()=>open(wire,phrase,context,()=>calls++));assert.equal(calls,0);}
 for(const length of [0,1,73,74]){let calls=0;assert.throws(()=>open(Buffer.from(pinned.v2.wireHex,'hex').subarray(0,length),phrase,context,()=>calls++));assert.equal(calls,0);}
 const magic=Buffer.from(pinned.v2.wireHex,'hex');magic[0]^=1;let magicCalls=0;assert.throws(()=>open(magic,phrase,context,()=>magicCalls++));assert.equal(magicCalls,0);
 let calls=0;const maximum=seal(Buffer.from('{}'+' '.repeat(4094))).wire;assert.equal(maximum.length,4202);assert.equal(maximum.toString('base64').length,5604);assert.deepEqual(decodeBase64(maximum.toString('base64')),maximum);assert.throws(()=>open(maximum,phrase,context,()=>calls++));assert.equal(calls,1); // admitted bytes; refused noncanonical shape
 const over=seal(Buffer.from('{}'+' '.repeat(4095))).wire;calls=0;assert.equal(over.length,4203);assert.throws(()=>open(over,phrase,context,()=>calls++));assert.equal(calls,0);
});
