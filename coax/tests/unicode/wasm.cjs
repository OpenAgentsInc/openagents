const fs=require('node:fs'),assert=require('node:assert/strict'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=(Number(x.__heap_base.value)+1039)&~15,input=base,scratch=base+2048,output=scratch+18432*4+32,result=output+73728+32;
 const bytes=new Uint8Array(memory.buffer),words=new DataView(memory.buffer),cases=fs.readFileSync(process.argv[3]);let at=0;const timings=[];
 function word(){const v=cases.readUInt32LE(at);at+=4;return v;}
 assert.equal(word(),0x314b464e);const count=word();
 for(let row=0;row<count;row++){
   const n=word(),m=word();assert(n<=1024&&m<=73728);
   bytes.fill(165,input-8,input+n+8);bytes.fill(165,output-8,output+m+8);bytes.fill(165,scratch-8,scratch);bytes.fill(165,scratch+18432*4,scratch+18432*4+8);bytes.fill(165,result-8,result+16);
   const original=cases.subarray(at,at+n);at+=n;const expected=cases.subarray(at,at+m);at+=m;bytes.set(original,input);
   const begun=row>=count-3?performance.now():0,error=x.normalize(input,n,scratch,18432,output,m,0,result);
   if(row>=count-3)timings.push({case:row-(count-3),milliseconds:performance.now()-begun});
   assert.equal(error,0,'error row '+row);assert.equal(words.getInt32(result+4,true),m,'length row '+row);
   assert.equal(Buffer.compare(bytes.subarray(output,output+m),expected),0,'bytes row '+row);assert.equal(Buffer.compare(bytes.subarray(input,input+n),original),0,'original');
   for(const[a,b]of[[input-8,input],[input+n,input+n+8],[output-8,output],[output+m,output+m+8],[scratch-8,scratch],[scratch+18432*4,scratch+18432*4+8],[result-8,result],[result+8,result+16]])assert(bytes.subarray(a,b).every(v=>v===165),'guards');
 }
 assert.equal(at,cases.length);
 const bad=[[-1,1,1,0,2],[1025,1,1,0,2],[1,-1,1,0,2],[1,18433,1,0,2],[1,1,-1,0,2],[1,1,73729,0,2],[1,1,1,1,4]];
 bytes[input]=65;for(const[n,p,c,overlap,error]of bad){bytes.fill(165,output,output+16);assert.equal(x.normalize(input,n,scratch,p,output,c,overlap,result),error);assert.equal(words.getInt32(result+4,true),0);assert(bytes.subarray(output,output+16).every(v=>v===165));}
 process.stdout.write(JSON.stringify({cases:count,boundary_cases:bad.length,guard_bytes:true,boundary_timings:timings,engine:`Node${process.version} V8${process.versions.v8}`}));
})().catch(e=>{console.error(e);process.exitCode=1;});
