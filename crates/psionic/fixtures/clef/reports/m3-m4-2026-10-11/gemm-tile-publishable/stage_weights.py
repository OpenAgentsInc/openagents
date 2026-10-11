#!/usr/bin/env python3
"""Stage three bounded Flash FFN tensors, without loading a model."""
from pathlib import Path
import sys, os, hashlib, json
import numpy as np
ROOT=Path(__file__).resolve().parent
BASE=ROOT.parent
sys.path.insert(0,os.environ['CLEF_Q4_HELPERS'])
import compare_weights as helper
helper.GGUF=Path(os.environ['CLEF_FLASH_GGUF'])
assert helper.GGUF.stat().st_size==6486448288
before=helper.GGUF.stat()
tensors,start=helper.gguf_header()
decoder,identity=helper.upstream_decoder()
manifest={'kind':'bounded_actual_flash_ffn_weights','artifact':'Clef-Flash-Q4_K_M.gguf','gguf_sha256_from_prior_verified_artifact':'fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c','decoder':identity,'layer':0,'tensors':[],'full_model_loaded':False,'gpu_executed':False}
for op,shape in [('gate',[4096,12288]),('up',[4096,12288]),('down',[12288,4096])]:
 name=f'blk.0.ffn_{op}.weight'; item=tensors[name]; assert item['shape']==shape and item['type']==12
 path=ROOT/f'{op}.f16'; assert not path.exists()
 packed_hash=hashlib.sha256(); half_hash=hashlib.sha256(); count=shape[0]*shape[1]; packed_bytes=count//256*144
 assert packed_bytes<=32*1024**2
 with helper.GGUF.open('rb') as src, path.open('xb') as dst:
  src.seek(start+item['offset'])
  remaining=packed_bytes
  while remaining:
   raw=src.read(min(remaining,144*2048)); assert raw and len(raw)%144==0
   packed_hash.update(raw); remaining-=len(raw)
   values=decoder.dequantize_blocks(np.frombuffer(raw,dtype=np.uint8).reshape(-1,144)).reshape(-1)
   assert np.isfinite(values).all()
   half=values.astype('<f2'); assert np.isfinite(half).all()
   payload=half.tobytes(); half_hash.update(payload); dst.write(payload)
 assert path.stat().st_size==count*2
 manifest['tensors'].append({'name':name,**item,'data_start':start,'packed_bytes':packed_bytes,'packed_sha256':packed_hash.hexdigest(),'half_file':path.name,'half_bytes':count*2,'half_sha256':half_hash.hexdigest(),'note':'Independent pinned Q4_K decoder followed by half conversion; not a full native decoder comparison.'})
after=helper.GGUF.stat(); assert (before.st_size,before.st_mtime_ns,before.st_ino)==(after.st_size,after.st_mtime_ns,after.st_ino)
(ROOT/'weights.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest),flush=True)
