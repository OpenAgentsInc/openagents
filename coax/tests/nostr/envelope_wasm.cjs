// Independent ECMAScript JSON/hash oracle and actual emitted Wasm; public data.
const fs=require('node:fs'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
const bytes=h=>Buffer.from(h,'hex'),string=h=>bytes(h).toString('utf8');
function canonical(v){
 if(v===null||typeof v==='boolean'||typeof v==='string')return JSON.stringify(v);
 if(typeof v==='number'){assert(Number.isSafeInteger(v));return String(v);}
 if(Array.isArray(v))return '['+v.map(canonical).join(',')+']';
 return '{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+canonical(v[k])).join(',')+'}';
}
if(process.argv[2]==='oracle'){
 let records=0;
 for(const c of cases)if(!c.error){
  const body=JSON.parse(c.op?string(c.input):string(c.expected)),inline=canonical(body.inline),raw=string(c.input);
  assert.equal(Buffer.byteLength(inline),c.canonical,c.id);assert.equal(body.artifact.size,c.canonical,c.id);
  assert.equal(body.artifact.digest,'sha256:'+crypto.createHash('sha256').update(inline).digest('hex'),c.id);
  if(!c.op)assert.deepEqual(body.inline,JSON.parse(raw),c.id);records++;
 }
 console.log(JSON.stringify({node:process.version,records}));process.exit(0);
}
const moduleBytes=fs.readFileSync(process.argv[2]),moduleObject=new WebAssembly.Module(moduleBytes),memory=new WebAssembly.Memory({initial:160,maximum:1024}),imports={env:{memory}};
for(const i of WebAssembly.Module.imports(moduleObject))if(i.kind==='function'){
 assert(i.module==='wasi_snapshot_preview1'&&['fd_fdstat_get','fd_seek','fd_write'].includes(i.name));
 (imports[i.module]||={})[i.name]=()=>{throw Error('unexpected libc '+i.name);};
}
const x=new WebAssembly.Instance(moduleObject,imports).exports;x._initialize?.();
const p=Number(x.__heap_base.value)+64,s=p+524320,scratch=s+65568,out=scratch+524320,view=new Uint8Array(memory.buffer);
function failedInfo(c){for(const getter of ['from','to','size','issued','retain','wire'])assert.equal(x[getter](),0n,c.id+' '+getter);}
function untouched(){assert(view.subarray(out,out+524304).every(b=>b===165));}
for(const c of cases){
 const input=bytes(c.input),schema=bytes(c.schema),expected=bytes(c.expected);view.set(input,p);view.set(schema,s);view.fill(90,scratch,scratch+524304);view.fill(165,out,out+524304);
 if(!c.op){
  assert.equal(x.build(p,input.length,s,schema.length,BigInt(c.issued),BigInt(c.retain),scratch,524288,out,c.error?524288:expected.length),c.error,c.id);
  assert.equal(x.written(),BigInt(c.error?0:expected.length),c.id);assert.equal(x.build_size(),BigInt(c.error?0:c.canonical),c.id);
  if(c.error)untouched();else{
   assert.equal(Buffer.from(view.subarray(out,out+expected.length)).toString('hex'),c.expected,c.id);assert(view.subarray(out+expected.length,out+expected.length+8).every(b=>b===165));assert(view.subarray(scratch,scratch+c.canonical).every(b=>b===0));
   assert.equal(x.check(out,expected.length,s,schema.length,1048576n,scratch,524288),0,c.id+' closure');assert.equal(x.size(),BigInt(c.canonical));assert.equal(x.wire(),1048576n);
   view.fill(165,out,out+524304);assert.equal(x.build(p,input.length,s,schema.length,BigInt(c.issued),BigInt(c.retain),scratch,524288,out,expected.length-1),10,c.id+' short');assert.equal(x.written(),0n);assert.equal(x.build_size(),0n);untouched();
  }
 }else{
  assert.equal(x.check(p,input.length,s,schema.length,BigInt(c.wire),scratch,524288),c.error,c.id);
  if(c.error){failedInfo(c);if(c.error===7)assert(view.subarray(scratch,scratch+2).every(b=>b===0));}else{
   assert.equal(x.size(),BigInt(c.canonical),c.id);assert.equal(x.issued(),BigInt(c.issued),c.id);assert.equal(x.retain(),BigInt(c.retain),c.id);assert.equal(x.wire(),BigInt(c.wire===-1?0:c.wire),c.id);
   assert.equal(Buffer.from(view.subarray(p+Number(x.from()),p+Number(x.to()))).toString('hex'),c.expected,c.id);assert(view.subarray(scratch,scratch+c.canonical).every(b=>b===0));
  }
 }
 assert.equal(Buffer.from(view.subarray(p,p+input.length)).toString('hex'),c.input,c.id+' original');assert.equal(Buffer.from(view.subarray(s,s+schema.length)).toString('hex'),c.schema,c.id+' schema');
}
view.set(Buffer.from('{}'),p);view.set(Buffer.from('x'),s);view.fill(165,out,out+524304);
for(const [work,dest] of [[scratch,p],[p,out],[s,out],[scratch,scratch+1]]){
 assert.equal(x.build(p,2,s,1,0n,1n,work,524288,dest,524288),11);assert.equal(x.written(),0n);untouched();
}
assert.equal(x.check(p,2,s,1,-1n,p,524288),11);assert.equal(x.check(p,2,s,1,-1n,s,524288),11);
assert.equal(x.build(p,-1,s,1,0n,1n,scratch,524288,out,100),1);assert.equal(x.build(p,2,s,1,0n,1n,scratch,-1,out,100),10);assert.equal(x.build(p,2,s,1,0n,1n,scratch,1,out,100),10);
assert.equal(x.build(p,2,s,1,0n,1n,scratch,524288,out,524288),0);assert.equal(x.check(out,Number(x.written()),s,1,-1n,scratch,1),10);failedInfo({id:'short-check-workspace'});
console.log(JSON.stringify({target:'wasm32-v8',node:process.version,records:cases.length,module_sha256:crypto.createHash('sha256').update(moduleBytes).digest('hex')}));
