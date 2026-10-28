// Execute real wasm32 bytes in V8/WASI, checking independent supplied digests.
const fs = require('node:fs');
const { WASI } = require('node:wasi');
(async () => {
  const memory = new WebAssembly.Memory({initial:512,maximum:1024});
  const wasi = new WASI({version:'preview1',args:[],env:{}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);
  const x=instance.exports, input=Number(x.__heap_base.value)+1024, output=input+1048576+64;
  const fixtures=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
  function check(v,op) {
    const source=Buffer.from(v.input,'hex'), bytes=new Uint8Array(memory.buffer);
    bytes.set(source,input);bytes.fill(165,output,output+40);
    const status=op(source.length);
    if(status!==0) throw new Error(`Wasm refused valid fixture (${status})`);
    const actual=Buffer.from(bytes.slice(output,output+32)).toString('hex');
    if(actual!==v.digest) throw new Error(`Wasm digest mismatch: ${actual} != ${v.digest}`);
    if(bytes.slice(output+32,output+40).some(b=>b!==165)) throw new Error('Wasm output guard changed');
  }
  for(const v of fixtures.vectors)check(v,n=>x.sha_hash(input,n,output));
  for(const v of fixtures.streams)check(v,n=>x.sha_stream(input,n,v.chunk,output));
  for(const v of fixtures.tags)check(v,n=>x.sha_tag(v.id,input,n,output));
  const bytes=new Uint8Array(memory.buffer);for(let i=0;i<1048576;i++)bytes[input+i]=i&255;
  for(let chunk=1;chunk<=129;chunk++) {
    if(x.sha_stream(input,1048576,chunk,output)!==0)throw new Error('Wasm large stream failed');
    if(Buffer.from(bytes.slice(output,output+32)).toString('hex')!==fixtures.large_digest)throw new Error(`Wasm large stream disagrees at chunk${chunk}`);
  }
  const rates=[];let sink=0;
  for(let sample=0;sample<7;sample++) {
    const start=performance.now();
    for(let i=0;i<8;i++) {
      bytes[input]=i;
      if(x.sha_hash(input,1048576,output)!==0)throw new Error('Wasm throughput call failed');
      sink^=bytes[output+(i&31)];
    }
    rates.push((8*1048576/1e6)/((performance.now()-start)/1000));
  }
  process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node ${process.version} V8 ${process.versions.v8}`,nist_rows:fixtures.vectors.length,streams:fixtures.streams.length,tags:fixtures.tags.length,large_stream_chunk_sizes:129,payload_bytes:1048576,mb_per_second:rates,digest_sink:sink}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
