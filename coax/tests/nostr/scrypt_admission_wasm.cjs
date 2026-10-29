const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{
  const wasi=new WASI({version:'preview1'});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);
  const cases=JSON.parse(fs.readFileSync(process.argv[3]));
  for(const [q,want,want32] of cases.start){const a=q.map(BigInt);assert.equal(instance.exports.sa_start(...a),want);assert.equal(instance.exports.sa_start32(...a),want32);}
  for(const [q,want,want32] of cases.output){const a=q.map(BigInt);assert.equal(instance.exports.sa_output(...a),want);assert.equal(instance.exports.sa_output32(...a),want32);}
})().catch(e=>{console.error(e);process.exit(1)});
