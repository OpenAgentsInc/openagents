"""Run only after the parent confirms all snapshot writers have frozen.

The original worktree HEAD, index, files and refs are never changed. A separate
index constructs the complete local tree, then one unreferenced commit uses
the common base as its parent so early checkpoints are included in the delta.
"""
from pathlib import Path
import subprocess,hashlib,json,os,sys,time,signal,concurrent.futures

ROOT=Path('/Users/christopherdavid/work/openagents-aethyr-book')
TARGET=Path('/Users/christopherdavid/work/openagents-aethyr-book-main')
if len(sys.argv)!=3 or sys.argv[1]!='--all-writers-frozen':
 raise SystemExit('requires --all-writers-frozen FRESH_OUTPUT_DIRECTORY after parent freeze signal')
OUT=Path(sys.argv[2]).resolve();OUT.mkdir(exist_ok=False)
INDEX=OUT/'private.index';env=dict(os.environ,GIT_INDEX_FILE=str(INDEX),GIT_LITERAL_PATHSPECS='1');started=time.monotonic();deadline=started+810

def run(args,*,data=None,private=True):
 remaining=deadline-time.monotonic()
 if remaining<=0:raise TimeoutError('private snapshot810s deadline')
 p=subprocess.Popen(['git',*args],cwd=ROOT,env=env if private else os.environ,stdin=subprocess.PIPE if data is not None else subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
 try:out,err=p.communicate(data,timeout=min(120,remaining))
 except BaseException:
  os.killpg(p.pid,signal.SIGTERM)
  try:p.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
  raise
 if p.returncode:raise RuntimeError((args,p.returncode,err.decode(errors='replace')[-3000:]))
 return out

sha=lambda b:hashlib.sha256(b).hexdigest()
head=run(['rev-parse','HEAD']).decode().strip()
target_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=TARGET,text=True,timeout=10).strip()
assert not subprocess.check_output(['git','status','--porcelain'],cwd=TARGET,timeout=10),'integration target is not clean'
base=run(['merge-base',head,target_head]).decode().strip()
original_index=Path(run(['rev-parse','--git-path','index'],private=False).decode().strip())
if not original_index.is_absolute():original_index=(ROOT/original_index).resolve()
original_index_hash=sha(original_index.read_bytes())
run(['read-tree',head])

def zpaths(args):return {os.fsdecode(x) for x in run(args).split(b'\0') if x}
early=zpaths(['diff','--name-only','--no-renames','-z',base,head])
def selection():
 dirty=zpaths(['diff','--name-only','--no-renames','-z',head])
 new=zpaths(['ls-files','--others','--exclude-standard','-z'])
 return early|dirty|new,dirty|new

def observe(name):
 p=ROOT/name
 if p.is_symlink():
  data=os.fsencode(os.readlink(p));mode='120000';kind='symlink';stamp=p.lstat().st_mtime_ns
 elif p.is_file():
  a=p.stat();data=p.read_bytes();b=p.stat()
  if (a.st_size,a.st_mtime_ns,a.st_ino)!=(b.st_size,b.st_mtime_ns,b.st_ino):raise RuntimeError('file changed while hashing: '+name)
  mode='100755' if a.st_mode&0o111 else '100644';kind='file';stamp=a.st_mtime_ns
 else:return name,{'kind':'absent'}
 return name,{'kind':kind,'mode':mode,'bytes':len(data),'sha256':sha(data),'git_blob_oid':hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest(),'mtime_ns':stamp}

def observations(paths):
 with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:return dict(pool.map(observe,sorted(paths)))

paths,stage=selection();before=observations(paths)
(OUT/'selected-before.json').write_text(json.dumps(before,indent=2)+'\n')
pathspec=OUT/'stage-paths.nul';pathspec.write_bytes(b''.join(os.fsencode(p)+b'\0' for p in sorted(stage)))
if stage:run(['add','-A','--pathspec-from-file='+str(pathspec),'--pathspec-file-nul'])
after_paths,_=selection();after=observations(paths)
assert paths==after_paths,{'added_paths':sorted(after_paths-paths),'removed_paths':sorted(paths-after_paths)}
assert before==after, [p for p in paths if before[p]!=after[p]]
tree=run(['write-tree']).decode().strip()
entries={}
for row in run(['ls-tree','-r','-z',tree]).split(b'\0'):
 if row:
  meta,name=row.split(b'\t',1);mode,kind,oid=meta.decode().split();entries[os.fsdecode(name)]={'mode':mode,'oid':oid}
for p,d in before.items():
 if d['kind']=='absent':assert p not in entries,p
 else:assert entries.get(p)=={'mode':d['mode'],'oid':d['git_blob_oid']},(p,entries.get(p),d)
last_paths,_=selection();last=observations(paths)
assert paths==last_paths and before==last,'source changed around write-tree'
assert run(['rev-parse','HEAD']).decode().strip()==head,'original HEAD changed'
assert sha(original_index.read_bytes())==original_index_hash,'original index changed'
message=('Aethyr chapter11 private integration snapshot\n\n'
 'Preserve the complete frozen local tree as one common-base delta, including early checkpoints and final source-bound evidence. This unreferenced snapshot does not promote the chapter or publish unfinished work.\n\n'
 'Co-Authored-By: GPT-6 <gpt-6@openai.com>\n')
commit=run(['commit-tree',tree,'-p',base],data=message.encode()).decode().strip()
assert sha(original_index.read_bytes())==original_index_hash
record={'schema':'aethyr.private-integration-snapshot.v1','status':'frozen-tree-verified-unreferenced-commit','source_worktree':str(ROOT),'integration_worktree':str(TARGET),
 'original_head':head,'upstream_target':target_head,'common_base':base,'tree':tree,'commit':commit,'original_index_path':str(original_index),'original_index_sha256':original_index_hash,'original_index_unchanged':True,
 'selected_paths':len(paths),'selected_present_files':sum(v['kind']!='absent' for v in before.values()),'selected_bytes':sum(v.get('bytes',0) for v in before.values()),
 'stage_paths':len(stage),'all_selected_unchanged_before_after_tree':True,'elapsed_seconds':time.monotonic()-started,'deadline_seconds':810,
 'manifest':'selected-before.json','manifest_sha256':sha((OUT/'selected-before.json').read_bytes()),'scope':'Private index only; original files, index, HEAD and refs untouched. No cherry-pick/rebase/applied integration yet. Commit parent is common base, so its single delta includes all earlier local checkpoints.'}
(OUT/'qualification.json').write_text(json.dumps(record,indent=2)+'\n');(OUT/'commit.txt').write_text(commit+'\n');print(json.dumps(record))
