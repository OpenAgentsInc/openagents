// Independent fixtures, real Wasm32/V8/WASI execution and output guards.
const fs=require('node:fs');const{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:512,maximum:1024});const wasi=new WASI({version:'preview1',args:[],env:{}});
 const{instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,a=Number(x.__heap_base.value)+1024,b=a+1048576+64,c=b+1048576+64,out=c+1024+64;const bytes=new Uint8Array(memory.buffer);
 const data=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 function put(p,hex){const v=Buffer.from(hex,'hex');bytes.set(v,p);return v.length;}
 function check(status,expected){if(status!==0)throw new Error(`Wasm refused (${status})`);const value=Buffer.from(expected,'hex');if(!Buffer.from(bytes.slice(out,out+value.length)).equals(value))throw new Error('Wasm digest/KDF disagreement');if(bytes.slice(out+value.length,out+value.length+8).some(x=>x!==165))throw new Error('Wasm output guard changed');}
 for(const v of data.hmac){const kn=put(a,v.key),mn=put(b,v.input);bytes.fill(165,out,out+8192);check(x.mac(a,kn,b,mn,out),v.digest);if(!v.digest.startsWith(v.published))throw new Error('RFC4231 fixture mismatch');}
 for(const v of data.hkdf){const sn=put(a,v.salt),kn=put(b,v.ikm);bytes.fill(165,out,out+8192);check(x.extract(a,sn,b,kn,out),v.prk);const pn=put(a,v.prk),n=put(b,v.info);bytes.fill(165,out,out+8192);check(x.expand(a,pn,b,n,out,v.okm.length/2),v.okm);}
 for(const v of data.expand){const pn=put(a,v.prk),n=put(b,v.info);bytes.fill(165,out,out+8192);check(x.expand(a,pn,b,n,out,v.okm.length/2),v.okm);}
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node ${process.version} V8 ${process.versions.v8}`,hmac:data.hmac.length,hkdf:data.hkdf.length,expand:data.expand.length}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
