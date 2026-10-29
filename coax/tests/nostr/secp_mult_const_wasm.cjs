const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,peer=base+64,out=peer+320,u=new Uint8Array(memory.buffer),d=new DataView(memory.buffer),p=(1n<<256n)-(1n<<32n)-977n;
 const inputs=fs.readFileSync(process.argv[3]),expected=fs.readFileSync(process.argv[4]);
 if(inputs.length%96||expected.length/65!==inputs.length/96)throw Error('fixture lengths');
 function number(buf,start){let n=0n;for(let i=0;i<32;i++)n=(n<<8n)|BigInt(buf[start+i]);return n;}
 function field(value,offset){for(let i=0;i<10;i++)d.setBigUint64(offset+i*8,(value>>BigInt(i*26))&((1n<<BigInt(i===9?22:26))-1n),true);}
 for(let row=0;row<inputs.length/96;row++){
  const at=row*96,k=number(inputs,at),px=number(inputs,at+32),py=number(inputs,at+64),e=row*65;
  for(let i=0;i<8;i++)d.setUint32(base+i*4,Number((k>>BigInt(i*32))&0xffffffffn),true);
  field(px,peer);field(py,peer+80);field(1n,peer+160);u.fill(165,out-8,out+248);
  if(x.variable_base(base,peer,out)!==1)throw Error('valid peer refusal');
  const f=[];for(let z=0;z<3;z++){let n=0n;for(let i=0;i<10;i++){let limb=d.getBigUint64(out+(z*10+i)*8,true);if(limb>((1n<<BigInt(i===9?22:26))-1n))throw Error('limb bounds');n+=limb<<BigInt(i*26);}if(n>=p)throw Error('field canonical');f.push(n);}
  if(!expected[e]){if(f[0]!==1n||f[1]!==1n||f[2]!==0n)throw Error('infinity');}
  else {const ex=number(expected,e+1),ey=number(expected,e+33),zz=f[2]*f[2]%p;if(!f[2]||f[0]!==ex*zz%p||f[1]!==ey*zz%p*f[2]%p)throw Error('affine mismatch row '+row);}
  if(u.slice(out-8,out).some(n=>n!==165)||u.slice(out+240,out+248).some(n=>n!==165))throw Error('point guard');
 }
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node${process.version} V8${process.versions.v8}`,pairs:inputs.length/96,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
