#!/usr/bin/env python3
"""Restore both original synthetic JSON and raw coordinate exports exactly."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=False)
root=Path(__file__).resolve().parent
for case in ['tail-1','interior-32']:
 parts=root/'coordinates'/case
 header=json.loads((parts/'header.json').read_text())
 json_sha=header.pop('original_json_sha256');raw_sha=header.pop('original_raw_sha256')
 header['objects']=[lane for obj in range(8) for lane in json.loads((parts/f'object-{obj}.json').read_text())]
 words=[0xffffffff]*65792
 for lane in header['objects']:
  offset=(lane['object']*32+lane['lane'])*257;words[offset]=lane['capacity']
  for slot in lane['slots']:
   start=offset+1+slot['slot']*4
   words[start:start+4]=slot['coordinate']+[slot['bits'],slot['valid_flag']]
 raw=struct.pack('<65792I',*words)
 encoded=json.dumps(header,sort_keys=True,separators=(',',':')).encode()
 assert hashlib.sha256(raw).hexdigest()==raw_sha
 assert hashlib.sha256(encoded).hexdigest()==json_sha
 (a.output/f'{case}.u32').write_bytes(raw)
 (a.output/f'{case}.json').write_bytes(encoded)
 print(case,'exact original bytes restored')
