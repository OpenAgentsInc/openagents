"""Stable byte-bound inputs/projection support for focused B2 checks."""
import hashlib
import os
from pathlib import Path
import re
import signal
import subprocess
ROOT=Path(__file__).resolve().parents[2]
H=ROOT/'coax/bin'  # Installed native artifacts; callers can select retained inputs explicitly.

def bounded_run(argv, *, cwd, timeout, env=None):
 """Capture an owned process group; retain partial output and reap on failure."""
 process=subprocess.Popen(list(map(str,argv)),cwd=cwd,env=env,stdin=subprocess.DEVNULL,
                          stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
 try:
  stdout,stderr=process.communicate(timeout=timeout)
  return subprocess.CompletedProcess(argv,process.returncode,stdout,stderr)
 except BaseException as exc:
  try:os.killpg(process.pid,signal.SIGKILL)
  except ProcessLookupError:pass
  try:stdout,stderr=process.communicate(timeout=5)
  except subprocess.TimeoutExpired as cleanup:
   stdout,stderr=cleanup.output or b'',cleanup.stderr or b''
   process.stdout.close();process.stderr.close();process.wait(timeout=5)
  exc.output,exc.stderr=stdout,stderr
  raise

def sha(raw):return hashlib.sha256(raw).hexdigest()
def file_sha(path):return sha(Path(path).read_bytes())
def mask(text):
 return re.sub(r'//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'',lambda m:''.join('\n' if c=='\n' else ' ' for c in m[0]),text)
def closure(entry,root=ROOT):
 seen={}
 def visit(path):
  path=path.resolve();path.relative_to(root.resolve())
  if path in seen:return
  raw=path.read_bytes();text=raw.decode();seen[path]=raw
  clean=mask(text)
  for m in re.finditer(r'(?m)^\s*#load\s+"([^"]+)"',text):
   if '#load' in clean[m.start():m.end()]:visit(path.parent/m[1])
  if re.search(r'(?m)^\s*#import_proto\b',clean):raise ValueError('descriptor import needs an explicit projection contract')
 visit(entry)
 return seen
def strip_regions(raw,flatten=False):
 text=raw.decode();clean=mask(text);regions=[]
 names='test|forall|authority'+('|load|import' if flatten else '')
 for m in re.finditer(r'(?m)^[ \t]*#('+names+r')\b',clean):
  if any(start<=m.start()<end for start,end,_ in regions):continue
  kind=m[1]
  if kind in ('test','forall'):
   begin=clean.index('{',m.end());depth=1;end=begin+1
   while depth and end<len(clean):
    if clean[end]=='{':depth+=1
    elif clean[end]=='}':depth-=1
    end+=1
   if depth:raise ValueError('unterminated check region')
  else:end=clean.index(';',m.end())+1
  regions.append((m.start(),end,kind))
 out=bytearray(raw);facts=[]
 for start,end,kind in regions:
  a=len(text[:start].encode());b=len(text[:end].encode())
  facts.append(dict(start=a,end=b,kind=kind,sha256=sha(raw[a:b])))
  for i in range(a,b):
   if out[i]!=10:out[i]=32
 projected=bytes(out)
 assert len(projected)==len(raw)
 return projected.decode(),facts
def source_inventory(files):
 return {str(p.relative_to(ROOT)):dict(bytes=len(raw),sha256=sha(raw)) for p,raw in files.items()}
