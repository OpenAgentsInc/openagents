#!/usr/bin/env python3
"""Reconstruct one split receipt and verify its complete byte identity."""
import argparse
import hashlib
import json
from pathlib import Path

def digest(raw):return hashlib.sha256(raw).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('bundle',type=Path);p.add_argument('index');p.add_argument('output',type=Path);p.add_argument('--original-path-map',type=Path);a=p.parse_args()
 root=a.bundle.resolve()
 def read(name):
  path=(root/name).resolve()
  if root not in path.parents:raise ValueError('bundle path escapes root')
  return path.read_bytes()
 index=json.loads(read(a.index));data={}
 for key in index['original_keys']:
  if key not in index['arrays']:data[key]=index['metadata'][key];continue
  values=[]
  for part in index['arrays'][key]:
   raw=read(part['path'])
   if digest(raw)!=part['sha256'] or len(raw)!=part['bytes']:raise ValueError('split part hash or size differs')
   values.append(json.loads(raw))
  data[key]=values
 raw=(json.dumps(data,indent=2,allow_nan=False)+'\n').encode()
 if digest(raw)!=index['sanitized_original_sha256'] or len(raw)!=index['sanitized_original_bytes']:raise ValueError('sanitized receipt reconstruction differs')
 kind='sanitized'
 if a.original_path_map:
  mapping=json.loads(a.original_path_map.read_text());manifest=json.loads(read('manifest.json'))
  if set(mapping)!=set(manifest['path_tokens']):raise ValueError('path token set differs')
  text=raw.decode()
  for token,value in mapping.items():text=text.replace(token,value)
  raw=text.encode();kind='original'
  if digest(raw)!=index['original_sha256'] or len(raw)!=index['original_bytes']:raise ValueError('original receipt reconstruction differs')
 with a.output.open('xb') as f:f.write(raw)
 print(json.dumps({'kind':kind,'bytes':len(raw),'sha256':digest(raw)}))

if __name__=='__main__':main()
