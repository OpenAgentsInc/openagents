const fs=require('node:fs');
(async()=>{
 const denied=()=>{throw Error('Unexpected QR host effect');};
 const{instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:{fd_fdstat_get:denied,fd_seek:denied,fd_write:denied,proc_exit:denied}});
 const x=instance.exports;x._initialize?.();const memory=new Uint8Array(x.memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8')).rows;
 for(const row of rows){
  memory.set(row.bytes,x.input());memory.fill(165,x.output(),x.output()+5930);
  if(x.encode(row.bytes.length,5930,-1)!==0||x.size()!==row.size||x.mask()!==row.mask||x.penalty()!==row.penalty)throw Error('Wasm metadata mismatch');
  const expected=row.rows.join('');for(let i=0;i<expected.length;i++)if(memory[x.output()+i]!==Number(expected[i]))throw Error('Wasm module mismatch');
  for(let i=expected.length;i<5930;i++)if(memory[x.output()+i]!==165)throw Error('Wasm guard changed');
 }
 if(x.encode(413,5930,-1)!==1||x.encode(70,1368,-1)!==2||x.encode(70,5930,8)!==3)throw Error('Wasm boundary refusal failed');
 process.stdout.write(JSON.stringify({engine:'Bun'+process.versions.bun+' V8-compatible Wasm API; not a browser timing report',grids:rows.length,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
