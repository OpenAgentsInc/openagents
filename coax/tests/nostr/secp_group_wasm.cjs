// Real wasm32 plus independent BigInt Jacobian projection. Fixtures are public.
const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,out=base+65536,u=new Uint8Array(memory.buffer),d=new DataView(memory.buffer);
 const p=(1n<<256n)-(1n<<32n)-977n,fixtures=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 const pow=(a,n)=>{let r=1n;while(n){if(n&1n)r=r*a%p;a=a*a%p;n>>=1n;}return r;};
 const set=(at,words)=>words.forEach((v,i)=>d.setBigUint64(at+i*8,BigInt(v),true));
 function check(status,wanted,ok=true){
  if(status!==(ok?0:1))throw Error('Wasm point status mismatch');const f=[];
  for(let field=0;field<3;field++){let value=0n;for(let i=0;i<10;i++){const n=d.getBigUint64(out+(field*10+i)*8,true),mask=(1n<<BigInt(i===9?22:26))-1n;if(n>mask)throw Error('noncanonical limb');value+=n<<BigInt(i*26);}if(value>=p)throw Error('noncanonical field');f.push(value);}
  if(!ok||wanted===null){if(f[0]!==1n||f[1]!==1n||f[2]!==0n)throw Error('noncanonical infinity');}
  else{if(!f[2])throw Error('unexpected infinity');const zi=pow(f[2],p-2n);if(f[0]*zi*zi%p!==BigInt('0x'+wanted[0])||f[1]*zi*zi*zi%p!==BigInt('0x'+wanted[1]))throw Error('Wasm affine mismatch');}
  if(u.slice(out+240,out+248).some(b=>b!==165))throw Error('point guard changed');
 }
 for(const v of fixtures.operations){set(base,v.a);set(base+256,v.b);u.fill(165,out,out+248);check(x.group(v.op,base,base+256,out),v.expected,v.ok);}
 for(const v of fixtures.decodes){u.set(Buffer.from(v.bytes,'hex'),base);u.fill(165,out,out+248);check(x.decode(base,v.bytes.length/2,v.lift?1:0,out),v.expected,v.ok);}
 for(const v of fixtures.picks){v.table.forEach((a,i)=>set(base+i*240,a));u.fill(165,out,out+248);check(x.pick(base,v.table.length,BigInt(v.which),out),v.expected,v.ok);}
 for(const v of fixtures.encodes){set(base,v.point);u.fill(165,out,out+40);const n=x.encode(base,out);if(n!==v.length)throw Error('encode length/status');const wanted=Buffer.from(v.bytes,'hex');if(Buffer.from(u.slice(out,out+wanted.length)).compare(wanted))throw Error('encode bytes');if(u.slice(out+wanted.length,out+33).some(b=>b!==0)||u.slice(out+33,out+40).some(b=>b!==165))throw Error('encode refusal/guard bytes');}
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node${process.version} V8${process.versions.v8}`,operations:fixtures.operations.length,decodes:fixtures.decodes.length,picks:fixtures.picks.length,encodes:fixtures.encodes.length,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
