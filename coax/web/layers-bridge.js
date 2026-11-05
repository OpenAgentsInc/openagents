// Firm reason: 1 (browser SDK: Wasm storage and copies of admitted canonical image bytes).
export class LayerBridge {
  constructor(module) {
    const memory=this.memory=new WebAssembly.Memory({initial:256,maximum:1024});
    const wasi={proc_exit:code=>{throw Error(`image reactor exited ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
      clock_time_get(id,precision,at){const milliseconds=id===0?Date.now():performance.now();new DataView(memory.buffer).setBigUint64(at,BigInt(Math.floor(milliseconds*1e6)),true);return 0;},
      fd_write(fd,iov,count,written){const v=new DataView(memory.buffer);let n=0;for(let i=0;i<count;i++)n+=v.getUint32(iov+8*i+4,true);v.setUint32(written,n,true);return 0;}};
    this.x=new WebAssembly.Instance(module,{env:{memory},wasi_snapshot_preview1:wasi}).exports;this.x._initialize?.();
  }
  side(profile){return Number(this.x.layers_profile_side(BigInt(profile)));}
  decode(bytes,side){
    const x=this.x;if(bytes.length>x.layers_source_capacity())throw Error('canonical image exceeds physical source storage');
    new Uint8Array(this.memory.buffer,x.layers_source_ptr(),bytes.length).set(bytes);
    const status=Number(x.layers_plan(bytes.length,BigInt(side)));if(status!==0)throw Error(`Coax refused canonical layer plan: ${status}`);
    const value={side:Number(x.layers_info(0)),width:Number(x.layers_info(1)),height:Number(x.layers_info(2)),levels:Number(x.layers_info(3)),bytes:Number(x.layers_info(4)),work:Number(x.layers_info(5))};
    const decoded=Number(x.layers_decode());if(decoded!==0)throw Error(`Coax refused canonical layer decode: ${decoded}`);
    value.data=new Uint8Array(this.memory.buffer,x.layers_output_ptr(),value.bytes).slice();return value;
  }
  close(){this.x.layers_close();}
}
