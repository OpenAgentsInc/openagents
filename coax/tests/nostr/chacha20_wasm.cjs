// Independent OpenSSL oracle and actual emitted Wasm tests. Public fixtures only.
const fs=require('node:fs'),crypto=require('node:crypto'),assert=require('node:assert/strict');
function oracle(c){const iv=Buffer.alloc(16);iv.writeUInt32LE(c.counter,0);Buffer.from(c.nonce,'hex').copy(iv,4);const cipher=crypto.createCipheriv('chacha20',Buffer.from(c.key,'hex'),iv);return Buffer.concat([cipher.update(Buffer.from(c.input,'hex')),cipher.final()]).toString('hex');}
const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
if(process.argv[2]==='oracle'){
  fs.writeFileSync(1,JSON.stringify({node:process.version,openssl:process.versions.openssl,outputs:cases.map(oracle)})+'\n');process.exit(0);
}
const moduleBytes=fs.readFileSync(process.argv[2]),moduleObject=new WebAssembly.Module(moduleBytes);
const memory=new WebAssembly.Memory({initial:128,maximum:1024}),imports={env:{memory}};
for(const i of WebAssembly.Module.imports(moduleObject))if(i.kind==='function'){
  assert(i.module==='wasi_snapshot_preview1'&&['fd_fdstat_get','fd_seek','fd_write'].includes(i.name),'unexpected import');
  (imports[i.module]||={})[i.name]=()=>{throw Error('unexpected libc path: '+i.name);};
}
const x=new WebAssembly.Instance(moduleObject,imports).exports;x._initialize?.();
const base=Number(x.__heap_base.value)+64,key=base,nonce=key+64,input=nonce+64,out=input+1048592,back=out+1048592;
const view=new Uint8Array(memory.buffer),put=(p,h)=>view.set(Buffer.from(h,'hex'),p);
let records=0;
for(const c of cases){
  const n=c.input.length/2,cap=Math.min(1048576,n+8);put(key,c.key);put(nonce,c.nonce);put(input,c.input);view.fill(165,out,out+n+8);
  const status=c.block?x.cc_block(key,32,nonce,12,c.counter,out,cap):x.cc_xor(key,32,nonce,12,c.counter,input,n,out,cap);
  assert.equal(status,0);assert.equal(Number(x.cc_written()),n);
  assert.equal(Buffer.from(view.subarray(out,out+n)).toString('hex'),c.output);assert(view.subarray(out+n,out+n+8).every(b=>b===165));
  if(!c.block){assert.equal(x.cc_xor(key,32,nonce,12,c.counter,out,n,back,n),0);assert.equal(Buffer.from(view.subarray(back,back+n)).toString('hex'),c.input);}
  records++;
}
view.fill(165,out,out+1048584);view.fill(165,key,key+128);
const bad=[
  [key,31,nonce,12,0,input,1,out,1,1],[key,32,nonce,11,0,input,1,out,1,2],
  [key,32,nonce,12,0,input,-1,out,64,3],[key,32,nonce,12,0,input,1048577,out,1048576,3],
  [key,32,nonce,12,0xffffffff,input,65,out,65,5],[key,32,nonce,12,0,input,65,out,64,4],
  [key,32,nonce,12,0,input,65,input,65,6],[key,32,nonce,12,0,input,1,key,64,6],
  [key,32,nonce,12,0,input,1,nonce,12,6],[key,32,nonce,12,0,input,1,out,-1,4],
  [key,32,nonce,12,0xffffc001,input,1048576,out,1048576,5]
];
for(const args of bad){const wanted=args.pop();assert.equal(x.cc_xor(...args),wanted);assert.equal(Number(x.cc_written()),0);}
assert(view.subarray(out,out+1048584).every(b=>b===165));assert(view.subarray(key,key+128).every(b=>b===165));
put(key,'00'.repeat(32));put(nonce,'00'.repeat(12));for(let i=0;i<1048576;i++)view[input+i]=i%256;
for(let i=0;i<5;i++)assert.equal(x.cc_xor(key,32,nonce,12,7,input,1048576,out,1048576),0);
const rates=[];let sink=0;
for(let sample=0;sample<7;sample++){
  const start=process.hrtime.bigint();
  for(let i=0;i<32;i++){view[input]=i;assert.equal(x.cc_xor(key,32,nonce,12,7,input,1048576,out,1048576),0);sink^=view[out+(i%64)];}
  rates.push((32*1048576/1e6)/(Number(process.hrtime.bigint()-start)/1e9));
}
console.log(JSON.stringify({target:'wasm32-v8',node:process.version,openssl:process.versions.openssl,records,refusal_cases:bad.length,payload_bytes:1048576,iterations:32,samples:7,mb_per_second:rates,median_mb_per_second:[...rates].sort((a,b)=>a-b)[3],sink,module_sha256:crypto.createHash('sha256').update(moduleBytes).digest('hex')}));
