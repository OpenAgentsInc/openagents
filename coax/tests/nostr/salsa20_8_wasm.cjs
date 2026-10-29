const fs = require('fs');
const assert = require('assert');
const {WASI} = require('node:wasi');

function reference(input) {
  const x = Array.from(input);
  const groups = [[0,4,8,12],[5,9,13,1],[10,14,2,6],[15,3,7,11],
    [0,1,2,3],[5,6,7,4],[10,11,8,9],[15,12,13,14]];
  const rot = (v, n) => ((v << n) | (v >>> (32-n))) >>> 0;
  for (let round=0; round<4; round++) for (const [a,b,c,d] of groups) {
    x[b]=(x[b]^rot((x[a]+x[d])>>>0,7))>>>0;
    x[c]=(x[c]^rot((x[b]+x[a])>>>0,9))>>>0;
    x[d]=(x[d]^rot((x[c]+x[b])>>>0,13))>>>0;
    x[a]=(x[a]^rot((x[d]+x[c])>>>0,18))>>>0;
  }
  return x.map((v,i)=>(v+input[i])>>>0);
}

(async () => {
  const wasi=new WASI({version:'preview1'});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);
  const e=instance.exports;
  for (let mode=0;mode<3;mode++) assert.equal(e.salsa_refusal(mode),1);
  assert.equal(e.salsa_tail(),1);
  let seed=7914;
  for (let test=0; test<2002; test++) {
    const input=new Uint32Array(e.memory.buffer,e.salsa_input(),16);
    for (let i=0; i<16; i++) {
      seed^=seed<<13; seed^=seed>>>17; seed^=seed<<5;
      input[i]=test===0?0:test===1?0xffffffff:seed>>>0;
    }
    const expected=reference(input);
    assert.equal(e.salsa_run(),1);
    assert.deepStrictEqual(Array.from(new Uint32Array(e.memory.buffer,e.salsa_output(),16)),expected);
  }
  console.log('Wasm: 2002 independent inputs passed');
})().catch(e=>{console.error(e);process.exit(1)});
