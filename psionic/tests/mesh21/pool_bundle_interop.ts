// Independent protoc oracle. Test tooling only; no product protocol logic.
import { mkdir } from 'node:fs/promises';
import assert from 'node:assert/strict';
const [mode,root,out]=process.argv.slice(2);if(!root||!out)throw Error('prepare|check ROOT OUT');
const descriptor=root+'/proto/descriptors/mesh-pool-inputs.pb';const name='openagents.psionic.mesh.v1alpha1.PublicPoolInputs';
async function protoc(args:string[],input:Uint8Array|string){const p=Bun.spawn([process.env.PROTOC||'protoc','--descriptor_set_in='+descriptor,...args],{stdin:'pipe',stdout:'pipe',stderr:'pipe'});p.stdin.write(input);p.stdin.end();const data=new Uint8Array(await new Response(p.stdout).arrayBuffer());if(await p.exited!==0)throw Error(await new Response(p.stderr).text());return data;}
const source=(policy:string,aggregate:string,events:string[])=>'version:"openagents.psionic.mesh-public-inputs.v1" policy_json:'+JSON.stringify(policy)+' aggregate_event_json:'+JSON.stringify(aggregate)+' '+events.map(e=>'event_json:'+JSON.stringify(e)).join(' ');
await mkdir(out,{recursive:true});
if(mode==='prepare'){
 const v='openagents.psionic.mesh-public-inputs.v1';
 const vectorInputs={GOOD:source('policy','aggregate',['event0','event1']),EMPTY:source('p','a',[]),EMPTY_POLICY:source('','a',[]),EMPTY_AGGREGATE:source('p','',[]),EMPTY_EVENT:source('p','a',['']),WRONG_VERSION:source('p','a',[]).replace(v,'openagents.psionic.mesh-witness.v1')};
 const vectors:Record<string,Uint8Array>={};for(const [key,input]of Object.entries(vectorInputs))vectors[key]=await protoc(['--encode='+name],input);
 vectors.UNKNOWN=new Uint8Array([...vectors.GOOD,42,1,88]);vectors.DUPLICATE=new Uint8Array([...vectors.GOOD,18,1,88]);vectors.NONMINIMAL=new Uint8Array([138,0,...vectors.GOOD.slice(1)]);
 let fixture='// Public synthetic protobuf fixtures encoded independently by protoc.\n';for(const[key,bytes]of Object.entries(vectors))fixture+=`MBPF_${key} :: [${bytes.length}]u8.[${Array.from(bytes).join(',')}];\n`;
 assert.equal(await Bun.file(root+'/psionic/tests/mesh21/pool_bundle_vectors.coax').text(),fixture,'committed vectors must match independent protoc');
 const p=source(' {"b":2, "a":1} ',' {"id":"synthetic","content":"\\u03c0"} ',[' {"kind":30200} ','{"kind":3201}']);
 await Bun.write(out+'/originals.pb',await protoc(['--encode='+name],p));
 await Bun.write(out+'/schema-valid-over-codec-limit.pb',await protoc(['--encode='+name],source('p','a',Array.from({length:70},()=> 'e'.repeat(16384)))));
}else if(mode==='check'){
 for(const file of ['coax.pb','exact.pb']){const original=new Uint8Array(await Bun.file(out+'/'+file).arrayBuffer());const text=await protoc(['--decode='+name],original);const again=await protoc(['--encode='+name],text);assert.deepEqual(again,original);}
 assert.deepEqual(new Uint8Array(await Bun.file(out+'/coax.pb').arrayBuffer()),new Uint8Array(await Bun.file(out+'/originals.pb').arrayBuffer()));
 assert.equal((await Bun.file(out+'/exact.pb').arrayBuffer()).byteLength,1048576);console.log('PASS independent protoc: exact original JSON bytes and exact1MiB codec boundary roundtrip');
}else throw Error('mode');
