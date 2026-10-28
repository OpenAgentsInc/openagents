// Real wasm32 execution; expected bytes come from the independent bigint oracle.
const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024});
 const wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});
 wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,out=base+512;
 const u=new Uint8Array(memory.buffer),data=new DataView(memory.buffer);
 const fixtures=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 function check(status,expected){
  if(status!==0)throw Error(`Wasm refused fixture: ${status}`);
  const actual=Buffer.from(u.slice(out,out+32)).toString('hex');
  if(actual!==expected)throw Error(`Wasm mismatch ${actual} != ${expected}`);
  if(u.slice(out+32,out+40).some(b=>b!==165))throw Error('Wasm output guard changed');
 }
 for(const v of fixtures.operations){
  u.set(Buffer.from(v.a,'hex'),base);u.set(Buffer.from(v.b,'hex'),base+64);u.fill(165,out,out+40);
  check(x.field(['normalize','add','sub','negate','mul','square'].indexOf(v.op),base,base+64,out),v.expected);
 }
 for(const v of fixtures.raw){
  v.limbs.forEach((n,i)=>data.setBigUint64(base+i*8,BigInt(n),true));u.fill(165,out,out+40);
  check(x.raw(base,v.magnitude,out),v.expected);
 }
 for(const v of fixtures.raw_products||[]){
  v.a.forEach((n,i)=>data.setBigUint64(base+i*8,BigInt(n),true));v.b.forEach((n,i)=>data.setBigUint64(base+128+i*8,BigInt(n),true));u.fill(165,out,out+40);
  check(x.raw_product(base,base+128,out),v.expected);
 }
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node${process.version} V8${process.versions.v8}`,operations:fixtures.operations.length,raw_cases:fixtures.raw.length,raw_products:fixtures.raw_products?.length||0,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
