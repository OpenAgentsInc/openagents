const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
  const memory=new WebAssembly.Memory({initial:96,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
  const x=instance.exports,base=Number(x.__heap_base.value)+1024,input=base,out=base+524352,result=base+1048704,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
  for(const row of rows){
    const text=Buffer.from(row.text,'hex'),capacity=row.capacity??262144;u.fill(165,input-8,input+text.length+8);u.set(text,input);
    u.fill(165,out-8,out+262144+8);u.fill(165,result-8,result+72);
    const error=x.event(input,row.input_count??text.length,out,capacity,row.overlap?1:0,row.verify?1:0,result),r=new Int32Array(memory.buffer,result,8);
    if(error!==row.error||r[0]!==row.error||Boolean(r[2])!==Boolean(row.verified))throw Error(`event result:${row.name}:${error}`);
    if(row.preimage!==undefined){const expected=Buffer.from(row.preimage,'hex');if(r[1]!==expected.length||Buffer.from(u.slice(out,out+r[1])).compare(expected))throw Error('preimage:'+row.name);}
    if(row.id!==undefined&&Buffer.from(u.slice(result+32,result+64)).toString('hex')!==row.id)throw Error('computed ID:'+row.name);
    if(row.error&&r[1]!==0)throw Error('published failed output');
    if(Buffer.from(u.slice(input,input+text.length)).compare(text)||u.slice(input-8,input).some(v=>v!==165)||u.slice(input+text.length,input+text.length+8).some(v=>v!==165))throw Error('input changed');
    const limit=capacity>=0&&capacity<=262144?capacity:262144;
    if(u.slice(out-8,out).some(v=>v!==165)||u.slice(out+limit,out+262144+8).some(v=>v!==165)||u.slice(result-8,result).some(v=>v!==165)||u.slice(result+64,result+72).some(v=>v!==165))throw Error('output guard changed');
  }
  process.stdout.write(JSON.stringify({cases:rows.length,failures:0,target:'wasm32-wasip1',engine:`Node${process.version} V8${process.versions.v8}`}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
