// Firm reason: 1 (browser SDK: Wasm storage and copies of admitted natural image bytes).
export class MediaBridge {
  constructor(module) {
    const memory=this.memory=new WebAssembly.Memory({initial:256,maximum:1024});
    const wasi={proc_exit:code=>{throw Error(`media image reactor exited ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
      clock_time_get(id,precision,at){const milliseconds=id===0?Date.now():performance.now();new DataView(memory.buffer).setBigUint64(at,BigInt(Math.floor(milliseconds*1e6)),true);return 0;},
      fd_write(fd,iov,count,written){const v=new DataView(memory.buffer);let n=0;for(let i=0;i<count;i++)n+=v.getUint32(iov+8*i+4,true);v.setUint32(written,n,true);return 0;}};
    this.x=new WebAssembly.Instance(module,{env:{memory},wasi_snapshot_preview1:wasi}).exports;this.x._initialize?.();
  }
  decode(bytes){
    const x=this.x;if(bytes.length>x.media_source_capacity())throw Error('canonical media exceeds physical source storage');
    new Uint8Array(this.memory.buffer,x.media_source_ptr(),bytes.length).set(bytes);
    const status=Number(x.media_plan(bytes.length));if(status!==0)throw Error(`Coax refused canonical media plan: ${status}`);
    const value={width:Number(x.media_info(0)),height:Number(x.media_info(1)),bytes:Number(x.media_info(2)),work:Number(x.media_info(3)),source_bytes:Number(x.media_info(4))};
    const decoded=Number(x.media_decode());if(decoded!==0)throw Error(`Coax refused canonical media decode: ${decoded}`);
    value.data=new Uint8Array(this.memory.buffer,x.media_output_ptr(),value.bytes).slice();return value;
  }
  notice(rows,capacity){
    const x=this.x,encode=new TextEncoder();let at=0;x.media_notice_reset();
    for(const [index,row] of rows.entries()){
      const url=encode.encode(row.url);
      if(x.media_notice_row(BigInt(index),BigInt(at),BigInt(url.length),BigInt(row.ready??false),BigInt(row.failed??false),BigInt(row.width??0),BigInt(row.height??0))!==1)throw Error('Coax refused media notice storage');
      new Uint8Array(this.memory.buffer,x.media_source_ptr()+at,url.length).set(url);at+=url.length;
    }
    const bytes=Number(x.media_notice_format(BigInt(rows.length),BigInt(capacity)));
    if(bytes<0)throw Error('Coax refused complete media notice');
    return {kind:Number(x.media_notice_kind()),text:new TextDecoder().decode(new Uint8Array(this.memory.buffer,x.media_notice_ptr(),bytes))};
  }
  close(){this.x.media_close();}
}
