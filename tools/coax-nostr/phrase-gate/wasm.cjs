// Actual emitted Wasm, scalar ABI/buffer observations and public test entropy.
const fs=require('node:fs'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const binary=fs.readFileSync(process.argv[2]),moduleObject=new WebAssembly.Module(binary),imports={};
for(const item of WebAssembly.Module.imports(moduleObject)){
 assert(item.module==='wasi_snapshot_preview1'&&item.kind==='function'&&['fd_fdstat_get','fd_seek','fd_write','proc_exit'].includes(item.name));
 (imports[item.module]||={})[item.name]=()=>{throw Error('unexpected host I/O '+item.name);};
}
const x=new WebAssembly.Instance(moduleObject,imports).exports;x._initialize?.();const rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
const fields=['password','record','message','relay','recipient'];
function prepare(row){
 assert.equal(x.dispose(),0);const values=fields.map(name=>Buffer.from(row[name]));const lengths=values.map(x=>x.length),status=x.preflight(...lengths,row.operation);
 if(status)return status;
 const memory=new Uint8Array(x.memory.buffer);fields.forEach((name,i)=>memory.set(values[i],x[name]()));memory.set(Uint8Array.from({length:28},(_,i)=>i),x.entropy());
 return x.begin(...lengths,row.operation);
}
const started=performance.now(),observations=[];
for(const row of rows){
 let error=prepare(row),steps=0,previous=0;
 if(!error){assert.equal(x.output(),0);assert.equal(x.written(),0);while(x.running()){error=x.advance(1024);steps++;const completed=x.completed();assert(completed>=previous&&completed-previous<=1024&&completed<=210000);previous=completed;assert(steps<210002);if(error)break;}if(!error)error=x.error();}
 assert.equal(error,row.error,row.id);
 if(error){assert.equal(x.emit(),0);assert.equal(x.output(),0);assert.equal(x.written(),0);}
 else{const result=Buffer.from(new Uint8Array(x.memory.buffer,x.output(),x.written()));assert.equal(result.toString('hex'),row.expected,row.id);assert.equal(x.completed(),210000);}
 assert.equal(x.dispose(),0);assert.equal(x.clean(),1);observations.push({id:row.id,error,steps,completed:previous});
}
assert.equal(prepare(rows[0]),0);assert.equal(x.advance(256),0);assert.equal(x.cancel(),12);assert.equal(x.output(),0);assert.equal(x.dispose(),0);assert.equal(x.clean(),1);
assert.equal(prepare(rows[0]),0);x.test_fault(1);let status;while(x.running())status=x.advance(1024);assert.equal(status,13);assert.equal(x.emit(),0);assert.equal(x.output(),0);assert.equal(x.dispose(),13);x.test_fault(0);assert.equal(x.dispose(),0);assert.equal(x.clean(),1);
for(const budget of [0,-1,1025]){assert.equal(prepare(rows[0]),0);assert.notEqual(x.advance(budget),0);assert.equal(x.output(),0);assert.equal(x.dispose(),0);assert.equal(x.clean(),1);}
console.log(JSON.stringify({target:'wasm32-v8',node:process.version,module_sha256:crypto.createHash('sha256').update(binary).digest('hex'),records:rows.length,elapsedMs:performance.now()-started,observations}));
