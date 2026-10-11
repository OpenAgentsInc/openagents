#!/usr/bin/env python3
"""Stage only the three verified layer-0 Q4_K projections, never a full model."""
import argparse, hashlib, json, os
from pathlib import Path
import numpy as np

parser = argparse.ArgumentParser()
parser.add_argument('--gguf', type=Path, required=True)
parser.add_argument('--half-directory', type=Path, required=True)
parser.add_argument('--manifest', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
source = json.loads(args.manifest.read_text())
source_rows = source.get('source', source)['tensors']
result = {'schema': 'clef.q4-tensor-probe.weights.v1', 'source_manifest_sha256': hashlib.sha256(args.manifest.read_bytes()).hexdigest(),
          'full_model_read': False, 'numpy_version': np.__version__, 'tensors': []}
for name in ['gate', 'up', 'down']:
    row = next(r for r in source_rows if r['half_file'] == name+'.f16')
    m, k = (4096, 12288) if name == 'down' else (12288, 4096)
    assert row['type'] == 12 and row['packed_bytes'] == m*k//256*144
    with args.gguf.open('rb') as handle:
        handle.seek(row['data_start']+row['offset'])
        data = handle.read(row['packed_bytes'])
    assert len(data) == row['packed_bytes'] and hashlib.sha256(data).hexdigest() == row['packed_sha256']
    # Half values come from the previously pinned independent Q4_K decoder.
    half_path = args.half_directory / row['half_file']
    with half_path.open('rb') as handle:
        half_sha = hashlib.file_digest(handle, 'sha256').hexdigest()
    assert half_path.stat().st_size == m*k*2 and half_sha == row['half_sha256']
    blocks = np.frombuffer(data, dtype=np.uint8).reshape(-1,144)
    d = blocks[:,:2].copy().view('<f2').reshape(-1).astype(np.float32)
    dm = blocks[:,2:4].copy().view('<f2').reshape(-1).astype(np.float32)
    scale_bytes = blocks[:,4:16]
    scales = np.empty((len(blocks),8),dtype='<f4'); offsets = np.empty_like(scales)
    q = np.empty((len(blocks),256),dtype=np.uint8)
    for group in range(8):
        sc = scale_bytes[:,group]&63 if group<4 else (scale_bytes[:,group+4]&15)|((scale_bytes[:,group-4]>>6)<<4)
        mn = scale_bytes[:,group+4]&63 if group<4 else (scale_bytes[:,group+4]>>4)|((scale_bytes[:,group]>>6)<<4)
        scales[:,group] = d*sc.astype(np.float32)
        offsets[:,group] = -(dm*mn.astype(np.float32))
        src = blocks[:,16+(group//2)*32:16+(group//2+1)*32]
        q[:,group*32:(group+1)*32] = src>>4 if group%2 else src&15
    packed = (q[:,0::2] | (q[:,1::2]<<4)).astype(np.uint8)
    # Every source nibble is retained; no additional quantization occurs.
    assert np.array_equal(packed&15,q[:,0::2]) and np.array_equal(packed>>4,q[:,1::2])
    files = {}
    for suffix, payload in [('q4',data),('uint4',packed.tobytes()),('scales.f32',scales.tobytes()),('offsets.f32',offsets.tobytes())]:
        file = args.output/(name+'.'+suffix)
        with file.open('xb') as out: out.write(payload)
        files[suffix] = {'file':file.name,'bytes':len(payload),'sha256':hashlib.sha256(payload).hexdigest()}
    os.symlink(half_path.resolve(), args.output/(name+'.f16'))
    files['f16'] = {'file':name+'.f16','bytes':m*k*2,'sha256':half_sha}
    result['tensors'].append({'name':name,'m':m,'k':k,'files':files,'nibble_identity':True})
    (args.output/'manifest.json').write_text(json.dumps(result,indent=2)+'\n')
    del blocks, data, d, dm, scales, offsets, q, packed
print(json.dumps({'complete':True,'projections':len(result['tensors']),'output':str(args.output)}))
