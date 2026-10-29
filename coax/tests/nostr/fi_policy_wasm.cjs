const fs=require('node:fs');
(async()=>{
 const denied=()=>{throw Error('Unexpected FI policy host effect');};
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:{fd_fdstat_get:denied,fd_seek:denied,fd_write:denied,proc_exit:denied}});
 const x=instance.exports;x._initialize?.();
 const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const row of cases.addresses){const bytes=Buffer.from(row.hex,'hex');for(let i=0;i<bytes.length;i++)x.put(i,bytes[i]);if(x.address(bytes.length,row.transport)!==Number(row.expected))throw Error('Address policy mismatch');}
 for(let i=0;i<cases.scenarios.length;i++)if(x.scenario(i)!==cases.scenarios[i])throw Error(`Scenario mismatch ${i}`);
 process.stdout.write(JSON.stringify({engine:'Bun'+process.versions.bun,addresses:cases.addresses.length,scenarios:cases.scenarios.length,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
