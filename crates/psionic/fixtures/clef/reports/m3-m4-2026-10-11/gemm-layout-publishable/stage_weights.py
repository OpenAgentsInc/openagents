#!/usr/bin/env python3
"""Stage bit-preserving persistent transposes of the earlier Flash half files."""
from pathlib import Path
import os,hashlib,json,time
import numpy as np
root=Path(__file__).resolve().parent
source=Path(os.environ['CLEF_HALF_SOURCE']).resolve()
original=json.loads((source/'weights.json').read_text())
manifest={'source_weights_manifest_sha256':hashlib.sha256((source/'weights.json').read_bytes()).hexdigest(),'source':original,'tensors':[],'gpu_executed':False,'staging':[]}
for item in original['tensors']:
 path=source/item['half_file'];digest=hashlib.sha256()
 with path.open('rb') as f:
  for block in iter(lambda:f.read(1024**2),b''):digest.update(block)
 assert digest.hexdigest()==item['half_sha256']
 # A task-local symlink reuses the exact earlier file; no duplicate NT payload.
 (root/path.name).symlink_to(path)
 manifest['tensors'].append(item)
 k,m=item['shape'];name=path.stem+'_nn.f16';out=root/name
 assert not out.exists();began=time.monotonic()
 values=np.memmap(path,dtype='<u2',mode='r',shape=(m,k))
 transposed=np.ascontiguousarray(values.T)
 assert transposed.shape==(k,m)
 # Verify every half bit in both orientations before writing.
 assert np.array_equal(transposed.T,values)
 transposed.tofile(out)
 payload_sha=hashlib.sha256(memoryview(transposed).cast('B')).hexdigest()
 manifest['tensors'].append({**item,'half_file':name,'half_sha256':payload_sha,'layout':'row-major [inputK,outputM]','original_half_sha256':item['half_sha256']})
 manifest['staging'].append({'file':name,'seconds':time.monotonic()-began,'bytes':out.stat().st_size,'every_half_bit_preserved':True})
 del values,transposed
(root/'weights.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest),flush=True)
