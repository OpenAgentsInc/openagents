#!/usr/bin/env python3
"""Extract layer zero and one embedding row for an independent Linux oracle.

Run only after authenticating the pinned original checkpoint. This owned
fixture tool copies bytes; it does not execute checkpoint content.
"""
import json,struct,sys
from pathlib import Path

def main():
 if len(sys.argv)!=3:raise SystemExit('usage: elephant_extract.py ORIGINAL_MODEL OUTPUT_DIRECTORY')
 src=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
 if src.stat().st_size!=18819722392:raise ValueError('checkpoint size')
 with src.open('rb') as f:
  n=struct.unpack('<Q',f.read(8))[0];header=json.loads(f.read(n));base=8+n
  embed=header['model.language_model.embed_tokens.weight']
  assert embed['shape']==[248320,4096] and embed['dtype']=='BF16'
  f.seek(base+embed['data_offsets'][0]+32*4096*2);row=f.read(4096*2);assert len(row)==4096*2
  (out/'token32.bf16').write_bytes(row)
  prefix='model.language_model.layers.0.';selected=[(name,entry) for name,entry in header.items() if name.startswith(prefix)]
  assert len(selected)==14 and all(entry['dtype']=='BF16' for _,entry in selected)
  offset=0;h={}
  for name,entry in selected:
   begin,end=entry['data_offsets'];h[name[len(prefix):]]={'dtype':'BF16','shape':entry['shape'],'data_offsets':[offset,offset+end-begin]};offset+=end-begin
  raw=json.dumps(h,separators=(',',':')).encode();raw+=b' '*((-len(raw))%8)
  with (out/'layer0.safetensors').open('wb') as target:
   target.write(struct.pack('<Q',len(raw)));target.write(raw)
   for _,entry in selected:
    begin,end=entry['data_offsets'];f.seek(base+begin);remaining=end-begin
    while remaining:
     chunk=f.read(min(8*1024*1024,remaining))
     if not chunk:raise ValueError('truncated source')
     target.write(chunk);remaining-=len(chunk)
 print('extracted 14 original layer tensors and token 32 embedding')
if __name__=='__main__':main()
