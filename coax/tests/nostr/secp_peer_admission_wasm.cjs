const fs = require('node:fs'), {WASI} = require('node:wasi');
(async () => {
  async function load(path) {
    const memory = new WebAssembly.Memory({initial:64,maximum:1024});
    const wasi = new WASI({version:'preview1',args:[],env:{}});
    const {instance} = await WebAssembly.instantiate(fs.readFileSync(path),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});
    wasi.initialize(instance); return {memory, x:instance.exports};
  }
  const current=await load(process.argv[2]), prior=await load(process.argv[3]);
  const cases=JSON.parse(fs.readFileSync(process.argv[4],'utf8'));
  function call(mod,item) {
    const base=Number(mod.x.__heap_base.value)+1024, peer=base+64, out=peer+320;
    const u=new Uint8Array(mod.memory.buffer), d=new DataView(mod.memory.buffer);
    u.fill(0,base,base+32); d.setUint32(base,item.scalar,true);
    for(let i=0;i<30;i++)d.setBigUint64(peer+i*8,BigInt(item.limbs[i]),true);
    const before=u.slice(peer,peer+240); u.fill(165,out-8,out+248);
    const ok=mod.x.variable_base(base,peer,out), value=u.slice(out,out+240);
    if(!before.every((v,i)=>v===u[peer+i]))throw Error('peer modified');
    if(u.slice(out-8,out).some(v=>v!==165)||u.slice(out+240,out+248).some(v=>v!==165))throw Error('output guard');
    return {ok,value};
  }
  let bounded=0,refused=0;
  for(const item of cases) {
    const got=call(current,item);
    if(item.bounded) {
      const before=call(prior,item);
      if(got.ok!==before.ok || !got.value.every((v,i)=>v===before.value[i]))throw Error(`bounded behavior: ${item.name}`);
      bounded++;
    } else {
      if(got.ok!==0)throw Error(`oversized accepted: ${item.name}`);
      const d=new DataView(got.value.buffer,got.value.byteOffset,got.value.byteLength);
      for(let i=0;i<30;i++)if(d.getBigUint64(i*8,true)!==BigInt(i===0||i===10?1:0))throw Error(`infinity: ${item.name}`);
      refused++;
    }
  }
  process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node ${process.version} V8 ${process.versions.v8}`,bounded,refused,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
