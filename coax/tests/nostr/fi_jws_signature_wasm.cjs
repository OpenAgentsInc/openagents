const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024});const wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer),vectors=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 const modulus=Buffer.from(vectors.modulus,'hex');u.set(modulus,base+32768);
 for(const v of vectors.cases){const token=Buffer.from(v.token,'base64');u.set(token,base);let got=instance.exports.verify(base,token.length,base+32768,modulus.length);if(got!==Number(v.valid))throw Error(`verification ${got} != ${v.valid}`);}
 const r=vectors.rfc,rn=Buffer.from(r.modulus,'hex'),rs=Buffer.from(r.signature,'hex'),rm=Buffer.from(r.message,'base64'),rt=Buffer.from(r.token,'base64');u.set(rn,base+32768);u.set(rs,base+33792);u.set(rm,base+34816);u.set(rt,base);if(instance.exports.rsa(base+32768,rn.length,base+33792,rs.length,base+34816,rm.length)!==1 || instance.exports.verify(base,rt.length,base+32768,rn.length)!==0)throw Error('RFC 7515 A.2 verification/profile mismatch');
 console.log(`${vectors.cases.length} compact JWS Wasm cases match OpenSSL and closed-header oracle`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
