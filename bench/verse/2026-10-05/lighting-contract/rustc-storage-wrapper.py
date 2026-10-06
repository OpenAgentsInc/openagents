#!/usr/bin/env python3
import os,sys,pathlib,subprocess,hashlib,json,shutil
args=sys.argv[1:]
name=args[args.index('--crate-name')+1] if '--crate-name' in args else ''
args=[('incremental=/run/verse-audit-build-agent1/incremental' if x=='incremental=/home/christopherdavid/work/openagents-target-agent1/debug/incremental' else x) for x in args]
if '--out-dir' not in args:os.execv(args[0],args)
i=args.index('--out-dir')+1
original=pathlib.Path(args[i])
if str(original)!='/home/christopherdavid/work/openagents-target-agent1/debug/deps':os.execv(args[0],args)
suffix=next((x.split('=',1)[1] for x in args if x.startswith('extra-filename=')),name)
output=pathlib.Path('/run/verse-audit-build-agent1/outputs')/name/suffix
output.mkdir(parents=True,exist_ok=True);args[i]=str(output)
result=subprocess.run(args)
if result.returncode==0:
 for p in output.iterdir():
  if not p.is_file():continue
  dest=original/p.name
  if dest.is_symlink() and dest.resolve()==p:continue
  if dest.exists() or dest.is_symlink():
   if dest.is_symlink():dest.unlink()
   else:
    old=pathlib.Path('/run/verse-audit-build-agent1/previous')/(dest.name+'-'+hashlib.file_digest(dest.open('rb'),'sha256').hexdigest())
    assert not old.exists();shutil.move(str(dest),old)
  dest.symlink_to(p)
 with pathlib.Path('/run/verse-audit-build-agent1/compiler-storage.jsonl').open('a') as receipt:receipt.write(json.dumps({'crate':name,'target':str(original),'physical':str(output),'argv':args,'exit_code':0})+'\n')
sys.exit(result.returncode)
