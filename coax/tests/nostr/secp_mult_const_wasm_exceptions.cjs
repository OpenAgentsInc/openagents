const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,peer=base+64,out=peer+320,u=new Uint8Array(memory.buffer),d=new DataView(memory.buffer);
 const n=BigInt('0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141');let cases=0;
 for(const scalar of [0n,1n,n,(1n<<256n)-1n])for(const item of [{x:1n,y:1n,z:0n,ok:1},{x:0n,y:0n,z:0n,ok:0},{x:1n,y:1n,z:1n,ok:0}]){
  for(let i=0;i<8;i++)d.setUint32(base+i*4,Number((scalar>>BigInt(i*32))&0xffffffffn),true);
  u.fill(0,peer,peer+240);d.setBigUint64(peer,item.x,true);d.setBigUint64(peer+80,item.y,true);d.setBigUint64(peer+160,item.z,true);u.fill(165,out-8,out+248);
  if(x.variable_base(base,peer,out)!==item.ok)throw Error('peer admission');
  for(let f=0;f<3;f++)for(let i=0;i<10;i++)if(d.getBigUint64(out+(f*10+i)*8,true)!==BigInt(i===0&&f<2?1:0))throw Error('canonical infinity');
  if(u.slice(out-8,out).some(n=>n!==165)||u.slice(out+240,out+248).some(n=>n!==165))throw Error('point guard');cases++;
 }
 const p=(1n<<256n)-(1n<<32n)-977n,gx=BigInt('0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798'),gy=BigInt('0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8');
 function field(v,offset){for(let i=0;i<10;i++)d.setBigUint64(offset+i*8,(v>>BigInt(i*26))&((1n<<BigInt(i===9?22:26))-1n),true);}
 for(const scalar of [0n,1n,n-1n]){
  for(let i=0;i<8;i++)d.setUint32(base+i*4,Number((scalar>>BigInt(i*32))&0xffffffffn),true);
  field(gx,peer);field(p-gy,peer+80);field(1n,peer+160);u.fill(165,out-8,out+248);
  if(x.variable_base(base,peer,out)!==1)throw Error('negative peer refusal');
  const v=[];for(let f=0;f<3;f++){let word=0n;for(let i=0;i<10;i++){const limb=d.getBigUint64(out+(f*10+i)*8,true);if(limb>((1n<<BigInt(i===9?22:26))-1n))throw Error('limb bounds');word+=limb<<BigInt(i*26);}if(word>=p)throw Error('field canonical');v.push(word);}
  if(scalar===0n){if(v[0]!==1n||v[1]!==1n||v[2]!==0n)throw Error('negative peer infinity');}
  else {const y=scalar===1n?p-gy:gy,z2=v[2]*v[2]%p;if(!v[2]||v[0]!==gx*z2%p||v[1]!==y*z2%p*v[2]%p)throw Error('negative peer mismatch');}
  if(u.slice(out-8,out).some(n=>n!==165)||u.slice(out+240,out+248).some(n=>n!==165))throw Error('point guard');cases++;
 }
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node${process.version} V8${process.versions.v8}`,exceptional_cases:cases,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
