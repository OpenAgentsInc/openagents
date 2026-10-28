// Independent ECMAScript UTF-16 ordering and actual emitted Wasm. Public data.
const fs=require('node:fs'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
function canonical(v){
 if(v===null||typeof v==='boolean'||typeof v==='string')return JSON.stringify(v);
 if(typeof v==='number'){assert(Number.isSafeInteger(v));return String(v);}
 if(Array.isArray(v))return '['+v.map(canonical).join(',')+']';
 return '{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+canonical(v[k])).join(',')+'}';
}
if(process.argv[2]==='oracle'){
 for(const c of cases)if(!c.error)assert.equal(canonical(JSON.parse(Buffer.from(c.input,'hex').toString('utf8'))),Buffer.from(c.output,'hex').toString('utf8'));
 fs.writeFileSync(1,JSON.stringify({node:process.version,records:cases.filter(c=>!c.error).length})+'\n');process.exit(0);
}
const moduleBytes=fs.readFileSync(process.argv[2]),moduleObject=new WebAssembly.Module(moduleBytes),memory=new WebAssembly.Memory({initial:128,maximum:1024}),imports={env:{memory}};
for(const i of WebAssembly.Module.imports(moduleObject))if(i.kind==='function'){
 assert(i.module==='wasi_snapshot_preview1'&&['fd_fdstat_get','fd_seek','fd_write'].includes(i.name));
 (imports[i.module]||={})[i.name]=()=>{throw Error('unexpected libc path '+i.name);};
}
const x=new WebAssembly.Instance(moduleObject,imports).exports;x._initialize?.();
const input=Number(x.__heap_base.value)+64,out=input+524320,view=new Uint8Array(memory.buffer);
for(const c of cases){
 const data=Buffer.from(c.input,'hex'),expected=Buffer.from(c.output,'hex');view.set(data,input);const before=Buffer.from(view.subarray(input,input+data.length));view.fill(165,out,out+524304);
 assert.equal(x.canonicalize(input,data.length,out,c.error?524288:expected.length),c.error,c.id);assert.equal(Number(x.written()),c.error?0:expected.length,c.id);
 if(c.error)assert(view.subarray(out,out+524304).every(b=>b===165),c.id);
 else{
  assert.equal(Buffer.from(view.subarray(out,out+expected.length)).toString('hex'),c.output,c.id);assert(view.subarray(out+expected.length,out+expected.length+8).every(b=>b===165));
  view.fill(165,out,out+524304);assert.equal(x.canonicalize(input,data.length,out,expected.length-1),5,c.id);assert.equal(Number(x.written()),0);assert(view.subarray(out,out+524304).every(b=>b===165));
 }
 assert(Buffer.from(view.subarray(input,input+data.length)).equals(before),c.id);
}
view.set(Buffer.from('{"a":1}'),input);const before=Buffer.from(view.subarray(input,input+32));
assert.equal(x.canonicalize(input,7,input,7),6);assert.equal(x.canonicalize(input,7,input+1,7),6);assert(Buffer.from(view.subarray(input,input+32)).equals(before));
assert.equal(x.canonicalize(input,-1,out,100),1);assert.equal(x.canonicalize(input,7,out,-1),5);assert.equal(x.canonicalize(input,7,out,524289),5);
console.log(JSON.stringify({target:'wasm32-v8',node:process.version,records:cases.length,module_sha256:crypto.createHash('sha256').update(moduleBytes).digest('hex')}));
