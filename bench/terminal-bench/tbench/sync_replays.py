"""Mirror trace records from a benchmark host for offline Gym replay.

Usage: uv run python -m tbench.sync_replays HOST
Reads existing files over SSH. Does not start or stop any benchmark work.
The private local cache is outside git; configs contain identity fields only.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import tarfile

# Only metadata and transcript files are included, never task workspaces,
# environment secrets, credentials, settings, or collected project artifacts.
REMOTE = r'''
import io,json,tarfile
from pathlib import Path
root=Path.home()/'.openagents/terminal-bench/jobs'
with tarfile.open(fileobj=__import__('sys').stdout.buffer,mode='w|gz') as tar:
 def emit(name,data):
  info=tarfile.TarInfo(name);info.size=len(data);info.mode=0o600
  tar.addfile(info,io.BytesIO(data))
 for config in sorted(root.glob('*/*/config.json')):
  trial=config.parent;prefix=str(trial.relative_to(root))
  try:c=json.loads(config.read_text())
  except (OSError,ValueError):continue
  agent=c.get('agent') or {}
  clean={'agent':{k:agent[k] for k in ['name','import_path','model_name'] if k in agent},'task':{'path':(c.get('task') or {}).get('path')}}
  emit(prefix+'/config.json',json.dumps(clean).encode())
  result=trial/'result.json'
  if result.exists():
   try:
    r=json.loads(result.read_text())
    keys=['trial_name','task_name','started_at','finished_at','agent_execution','agent_result','verifier_result','exception_info','agent_info']
    emit(prefix+'/result.json',json.dumps({k:r[k] for k in keys if k in r}).encode())
   except (OSError,ValueError):pass
  paths=[]
  for name in ['agent/trajectory.json','agent/claude-code.txt','agent/codex.txt','agent/live/episode.atif.jsonl','verifier/ctrf.json']:
   paths.append(trial/name)
  episode=trial/'agent/episode'
  for name in ['episode.atif.jsonl','trajectory.atif.json','manifest.json','usage.json','evaluation/usage.json','artifacts/composition.json']:
   paths.append(episode/name)
  paths.extend(sorted((episode/'artifacts').glob('*.stream.jsonl')))
  for path in paths:
   if path.is_file() and not path.is_symlink():
    try:emit(str(path.relative_to(root)),path.read_bytes())
    except OSError:pass
'''


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("host")
    parser.add_argument("--output", type=Path, default=Path.home()/".openagents/terminal-bench/replay-jobs")
    args = parser.parse_args()
    if args.host.startswith("-"):
        parser.error("invalid SSH host")
    args.output.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(args.output, 0o700)
    process = subprocess.Popen(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", args.host, "python3", "-"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    assert process.stdin is not None and process.stdout is not None
    process.stdin.write(REMOTE.encode()); process.stdin.close()
    count = 0
    total = 0
    with tarfile.open(fileobj=process.stdout, mode="r|gz") as archive:
        for member in archive:
            if not member.isfile() or member.name.startswith("/") or ".." in Path(member.name).parts:
                raise ValueError("invalid replay archive member")
            dest = args.output/member.name
            dest.parent.mkdir(parents=True, exist_ok=True)
            data = archive.extractfile(member)
            assert data is not None
            temporary = dest.with_suffix(dest.suffix+".pending")
            with temporary.open("wb") as output:
                while chunk := data.read(1 << 20): output.write(chunk)
            os.chmod(temporary, 0o600)
            temporary.replace(dest)
            count += 1; total += member.size
            if count % 500 == 0: print(f"{count} files mirrored", flush=True)
    if process.wait() != 0: raise SystemExit("SSH trace collection failed")
    record = {"synced_at":datetime.now(timezone.utc).isoformat(),"host":args.host,"files":count,"bytes":total,"trials":len(list(args.output.glob('*/*/config.json'))),"snapshot":True}
    (args.output/"sync.json").write_text(json.dumps(record,indent=2)+"\n")
    print(json.dumps(record))


if __name__ == "__main__":
    main()
