// Test-only parser inputs; successful extraction never grants authority.
const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:48,maximum:128}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,input=base,u0=base+262200,m0=u0+1040,p0=m0+32,r0=p0+80,mem=new Uint8Array(memory.buffer);
 const long='[]'+' '.repeat(262142),many='['+Array(2000).fill('[]').join(',')+']';
 const cases=[
  {text:'[]',valid:true,counts:[0,0,0],values:['','','']},
  {text:'[["u","url"],["method","POST"],["payload","abc"]]',valid:true,counts:[1,1,1],values:['url','POST','abc']},
  {text:'[["u","x"],["u","last"]]',valid:true,counts:[2,0,0],values:['last','','']},
  {text:'[["\\u0075","\\u0061"]]',valid:true,counts:[1,0,0],values:['a','','']},
  {text:'[["unknown","\\ud800"]]',valid:true,counts:[0,0,0],values:['','','']},
  {text:'[["\\ud800","x"]]',valid:true,counts:[0,0,0],values:['','','']},
  {text:'[["u","\\ud800"]]',valid:false},
  {text:'[["u","x","extra"]]',valid:false},
  {text:'[["unknown",3]]',valid:false},
  {text:'[] true',valid:false},
  {text:long,valid:true,counts:[0,0,0],values:['','','']},
  {text:long+' ',valid:false},
  {text:many,valid:true,counts:[0,0,0],values:['','','']},
  {text:'['+Array(2001).fill('[]').join(',')+']',valid:false},
  {text:'[]',valid:false,overlap:true},
 ];
 for(const item of cases){
  const b=Buffer.from(item.text,'utf8');mem.fill(0xa5,input-8,input+b.length+8);mem.set(b,input);
  for(const [p,n] of [[u0,1024],[m0,16],[p0,64]])mem.fill(0xa5,p-8,p+n+8);
  const got=instance.exports.tags(input,b.length,u0,1024,m0,16,p0,64,item.overlap?1:0,r0),r=new Int32Array(memory.buffer,r0,7);
  if(Boolean(got)!==item.valid||Boolean(r[0])!==item.valid)throw Error('valid: '+item.text.slice(0,32));
  if(item.valid){
   for(let i=0;i<3;i++)if(r[i+1]!==item.counts[i])throw Error('count '+i);
   for(let i=0;i<3;i++){const p=[u0,m0,p0][i],x=Buffer.from(mem.slice(p,p+r[i+4])).toString('utf8');if(x!==item.values[i])throw Error('value '+i);}
  }
  if(Buffer.compare(Buffer.from(mem.slice(input,input+b.length)),b))throw Error('source changed');
  for(const [p,n] of [[u0,1024],[m0,16],[p0,64]])if(mem.slice(p-8,p).some(x=>x!==0xa5)||mem.slice(p+n,p+n+8).some(x=>x!==0xa5))throw Error('guard changed');
 }
 process.stdout.write(JSON.stringify({cases:cases.length,failures:0,target:'wasm32-wasip1'}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
