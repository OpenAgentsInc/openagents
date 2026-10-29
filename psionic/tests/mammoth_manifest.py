#!/usr/bin/env python3
"""Build/check Mammoth's complete pinned original two-shard inventory."""
import hashlib
import json
import math
import struct
import sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
TARGET=ROOT/'psionic/fixtures/models/mammoth/manifest.json'
REV='61cec44c281c693992e3d6ebeb44dd3402dfa186'
SHARDS=[('model-00001-of-00002.safetensors',49825162976,'29701f9a7b131bedb3fe1d565dad69b912d9128c6fc27e3d8fb0aedc47ae0e06'),('model-00002-of-00002.safetensors',4888445168,'4da0ff5476813bbd9898454f0d7f7ba3c82914c42e32e2b87577c6d69daae671')]
INDEX='0424520f63f04816eee4d229a4487dc9744866ceb46513e591dd987e401b1663'
MANIFEST_SHA='4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019'

def digest(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for chunk in iter(lambda:f.read(8*1024*1024),b''):h.update(chunk)
 return h.hexdigest()

def make(directory):
 index=directory/'model.safetensors.index.json'
 if digest(index)!=INDEX:raise ValueError('index digest')
 data=json.loads(index.read_bytes());mapping=data['weight_map'];total=0;tensors=[]
 for shard,(name,size,sha) in enumerate(SHARDS):
  file=directory/name
  if file.stat().st_size!=size or digest(file)!=sha:raise ValueError('shard digest or size')
  with file.open('rb') as f:
   n=struct.unpack('<Q',f.read(8))[0]
   if not 2<=n<=1048576:raise ValueError('header size')
   pairs=json.loads(f.read(n),object_pairs_hook=lambda x:x)
  seen=set();spans=[]
  for key,properties in pairs:
   if key=='__metadata__':continue
   if key in seen:raise ValueError('duplicate name')
   seen.add(key);p=dict(properties)
   if len(p)!=len(properties) or p.keys()!={'dtype','shape','data_offsets'}:raise ValueError('fields')
   if p['dtype']!='BF16' or not 0<=len(p['shape'])<=5 or any(not isinstance(x,int) or x<=0 for x in p['shape']):raise ValueError('shape')
   begin,end=p['data_offsets'];span=2*math.prod(p['shape'])
   if begin<0 or end!=begin+span or end>size-n-8 or mapping.get(key)!=name:raise ValueError('range or index')
   spans.append((begin,end));tensors.append({'name':key,'dtype':'BF16','shape':p['shape'],'shard':shard})
  spans.sort();position=0
  for begin,end in spans:
   if begin!=position:raise ValueError('overlap or gap')
   position=end
  if position!=size-n-8:raise ValueError('coverage')
  total+=position
 if len(tensors)!=1184 or len({x['name'] for x in tensors})!=1184 or len(mapping)!=1184 or data['metadata']['total_size']!=total or data['metadata']['total_parameters']!=sum(math.prod(x['shape']) for x in tensors):raise ValueError('inventory')
 tensors.sort(key=lambda x:x['name'])
 return {'model':'baselabs/mammoth-1','revision':REV,'index_sha256':INDEX,'shards':[{'file':name,'bytes':size,'sha256':sha} for name,size,sha in SHARDS],'tensors':tensors}

def header():
 raw=TARGET.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=MANIFEST_SHA:raise ValueError('pinned inventory fixture changed')
 manifest=json.loads(raw)
 print('/* Generated from the pinned Mammoth inventory. */')
 print('static const struct {const char *name; int64_t rank; int64_t dims[5]; int64_t shard;} mammoth_specs[] = {')
 for tensor in manifest['tensors']:
  shape=tensor['shape'];dims=shape+[0]*(5-len(shape))
  print('  {'+json.dumps(tensor['name'])+','+str(len(shape))+',{'+','.join(map(str,dims))+'},'+str(tensor['shard'])+'},')
 print('};')

def main():
 if sys.argv[1:]==['--header']:
  header();return
 if len(sys.argv)!=3 or sys.argv[1] not in ('--write','--check'):raise SystemExit('usage: mammoth_manifest.py --header OR --write|--check ORIGINAL_DIRECTORY')
 value=json.dumps(make(Path(sys.argv[2])),indent=2)+'\n'
 if sys.argv[1]=='--write':TARGET.write_text(value)
 elif TARGET.read_text()!=value:raise SystemExit('inventory differs from pinned originals')
 print('pinned Mammoth shards, index and 1184-tensor inventory match')
if __name__=='__main__':main()
