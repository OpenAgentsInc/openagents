const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,out=base+4096,u=new Uint8Array(memory.buffer),d=new DataView(memory.buffer),p=(1n<<256n)-(1n<<32n)-977n;
 const fixtures=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 const scalar=(at,n)=>{n=BigInt('0x'+n);for(let i=0;i<8;i++)d.setUint32(at+i*4,Number((n>>BigInt(i*32))&0xffffffffn),true);};
 for(const row of fixtures.operations){
  scalar(base,row.a);scalar(base+32,row.b);row.point.forEach((n,i)=>d.setBigUint64(base+64+i*8,BigInt(n),true));u.fill(165,out,out+248);
  const status=x.multiply(base,base+64,base+32,out);if(status!==(row.ok?0:1))throw Error('multiply status');
  const f=[];for(let field=0;field<3;field++){let n=0n;for(let i=0;i<10;i++){const limb=d.getBigUint64(out+(field*10+i)*8,true),mask=(1n<<BigInt(i===9?22:26))-1n;if(limb>mask)throw Error('limb bounds');n+=limb<<BigInt(i*26);}if(n>=p)throw Error('field canonical');f.push(n);}
  if(!row.ok||row.expected===null){if(f[0]!==1n||f[1]!==1n||f[2]!==0n)throw Error('infinity');}
  else {const ex=BigInt('0x'+row.expected[0]),ey=BigInt('0x'+row.expected[1]),zz=f[2]*f[2]%p;if(!f[2]||f[0]!==ex*zz%p||f[1]!==ey*zz%p*f[2]%p)throw Error('affine mismatch');}
  if(u.slice(out+240,out+248).some(n=>n!==165))throw Error('point guard');
 }
 for(const row of fixtures.recodings){scalar(base,row.scalar);u.fill(165,out,out+2064);const length=x.wnaf(base,row.width,out);if(length!==row.length)throw Error('wnaf length');for(let i=0;i<257;i++)if(d.getBigInt64(out+i*8,true)!==BigInt(row.digits[i]))throw Error('wnaf digit');if(u.slice(out+2056,out+2064).some(n=>n!==165))throw Error('wnaf guard');}
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node${process.version} V8${process.versions.v8}`,operations:fixtures.operations.length,recodings:fixtures.recodings.length,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
