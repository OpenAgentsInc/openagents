#!/usr/bin/env python3
"""Compare original and new descriptor generators; optional path for new hashes.

Uses protoc's descriptor model to count each affected API, verifies the old
frozen declaration fingerprint and applies only the reviewed precedence edit.
Not an error-semantics oracle: execute error_precedence.coax separately.
"""
import hashlib, json, pathlib, re, shutil, subprocess, sys
if not __debug__: raise SystemExit('checks require assertions')
if len(sys.argv) not in (3,4): raise SystemExit('usage: error_precedence_compare.py OLD NEW [OUTPUT]')
R=pathlib.Path(__file__).resolve().parents[3]; old,new=map(pathlib.Path,sys.argv[1:3]); protoc=shutil.which('protoc')
if not protoc: raise SystemExit('protoc is required')
def cmd(*args,input=None):return subprocess.run(list(map(str,args)),input=input,capture_output=True,check=True).stdout
def descriptor(path):
 text=cmd(protoc,'-I/opt/homebrew/include','-I'+str(R/'proto'),'--decode=google.protobuf.FileDescriptorSet','google/protobuf/descriptor.proto','openagents/options/v1alpha1/options.proto',input=path.read_bytes()).decode()
 root={};stack=[root]
 for raw in text.splitlines():
  line=raw.strip()
  if line=='}':stack.pop();continue
  if line.endswith(' {'):
   key=line[:-2];node={};stack[-1].setdefault(key,[]).append(node);stack.append(node);continue
  key,sep,value=line.partition(': ')
  if not sep:raise ValueError('unexpected descriptor line '+line)
  stack[-1].setdefault(key,[]).append(json.loads(value) if value.startswith('"') else value)
 assert len(stack)==1;types={}
 def add(m,prefix):
  full=prefix+'.'+m['name'][0];types[full]=m
  for c in m.get('nested_type',[]):add(c,full)
 for f in root.get('file',[]):
  pkg=f.get('package',[''])[0];prefix='.'+pkg if pkg else ''
  for m in f.get('message_type',[]):add(m,prefix)
 return types
from precedence_transform import transform
old_hashes=json.loads((R/'coax/tests/proto/message-limit-baselines.json').read_text())['profiles']
profiles=json.loads((R/'coax/tests/proto/import-baselines.json').read_text())['profiles'];assert set(profiles)==set(old_hashes)
results={}
for name,entry in profiles.items():
 path=R/entry.get('descriptor_fixture',name);assert hashlib.sha256(path.read_bytes()).hexdigest()==entry['descriptor_sha256']
 args=['proto',path,'--prefix','N'];a=cmd(old,*args).decode().split('// @proto-descriptor-declarations\n',1)[1];b=cmd(new,*args).decode().split('// @proto-descriptor-declarations\n',1)[1]
 assert hashlib.sha256(a.encode()).hexdigest()==old_hashes[name],name
 expected=transform(a,descriptor(path))
 if expected!=b:
  import difflib;print('\n'.join(list(difflib.unified_diff(expected.splitlines(),b.splitlines()))[:25]));raise ValueError(name+' changed more than the reviewed precedence edit')
 # A corrupted source and a malformed target must never compare equal.
 assert expected!=b.replace('if msg.error!=0','if msg.error==0',1)
 assert expected!=b.replace('error=checked.error','error=msg.error',1)
 results[name]=hashlib.sha256(b.encode()).hexdigest();print(name,'matches')
if len(sys.argv)==4:pathlib.Path(sys.argv[3]).write_text(json.dumps({'description':'Declarations after ordinal carried-first precedence. Prior message-limit fingerprints are unchanged.','reference_prefix':'N','profiles':results},indent=2)+'\n')
print(len(results),'profiles matched; independent descriptor counts and negative controls passed')
