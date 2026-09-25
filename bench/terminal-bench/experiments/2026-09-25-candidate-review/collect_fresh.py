#!/usr/bin/env python3
"""Retain all fresh agent and candidate files without reading verifier grades."""
from pathlib import Path
import argparse,hashlib,json,gzip,tarfile,io
parser=argparse.ArgumentParser(description="Collect label-free fresh traces and candidates as deduplicated blobs.")
parser.add_argument('--records',type=Path,required=True)
parser.add_argument('--jobs',type=Path,required=True)
args=parser.parse_args()
home=Path.home();root=args.records;jobs=args.jobs;entries=[];blobs={};secret=set()
def gather(v):
 if isinstance(v,str) and len(v)>24:secret.add(v.encode())
 elif isinstance(v,dict):
  for x in v.values():gather(x)
 elif isinstance(v,list):
  for x in v:gather(x)
for p in [home/'.codex/auth.json',home/'.openagents/jev.json']:
 if p.exists():gather(json.loads(p.read_text()))
p=home/'.openagents/bearer'
if p.exists():secret.add(p.read_bytes().strip())
for row in json.loads((root/'prospective-manifest.json').read_text()):
 trial=jobs/row['job']/row['trial'];files=[]
 for base in [trial/'agent',trial/'artifacts']:
  files.extend(p for p in base.rglob('*') if p.is_file() and not p.is_symlink())
 files.extend(p for p in [trial/'config.json',trial.parent/'config.json'] if p.exists())
 for p in sorted(set(files)):
  data=p.read_bytes()
  if any(s and s in data for s in secret):raise RuntimeError('Credential content in '+str(p))
  sha=hashlib.sha256(data).hexdigest(); name=str(p.relative_to(jobs));entries.append(dict(path=name,bytes=len(data),sha256=sha));blobs.setdefault(sha,p)
manifest=dict(schema='openagents.content-addressed-traces.v1',contains_grades=False,files=entries,blobs=len(blobs),bytes=sum(p.stat().st_size for p in blobs.values()))
(root/'prospective-trace-files.json').write_text(json.dumps(manifest,indent=2)+'\n')
archive=root/'prospective-traces.tar.gz'
with archive.open('wb') as dest:
 with gzip.GzipFile(filename='',mode='wb',fileobj=dest,mtime=0) as compressed:
  with tarfile.open(fileobj=compressed,mode='w|') as tar:
   for sha,p in sorted(blobs.items()):
    data=p.read_bytes()
    if hashlib.sha256(data).hexdigest()!=sha:raise RuntimeError('Trace changed')
    info=tarfile.TarInfo('blobs/'+sha);info.size=len(data);info.mode=0o644;tar.addfile(info,io.BytesIO(data))
print(json.dumps(dict(files=len(entries),blobs=len(blobs),archive_bytes=archive.stat().st_size,archive_sha256=hashlib.sha256(archive.read_bytes()).hexdigest(),credential_matches=0)))
