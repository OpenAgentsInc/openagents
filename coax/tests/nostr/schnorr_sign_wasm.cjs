// Test-only Wasm runner for coax/tests/nostr/schnorr_sign.py: runs the
// signing driver as a WASI command, stdin and stdout passed through.
const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const wasi=new WASI({version:'preview1',args:['driver'],env:{},stdin:0,stdout:1,stderr:2});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
 process.exitCode=wasi.start(instance);
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
