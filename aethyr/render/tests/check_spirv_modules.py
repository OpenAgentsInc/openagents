#!/usr/bin/env python3
"""Validate concrete package programs on the native host, without drawing.

Shader bytes travel as data. The remote effects adapter runs the installed
SPIR-V validator, including bad-magic and type-zero controls, and returns JSON.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import signal
import struct
import subprocess
import tarfile
import time

HERE = Path(__file__).resolve().parent
REMOTE = '''import hashlib,json,shutil,subprocess
from pathlib import Path
tool=shutil.which('spirv-val');assert tool
source=json.loads(Path('inputs.json').read_text());observations=[]
for row in source:
 data=Path(row['file']).read_bytes();assert hashlib.sha256(data).hexdigest()==row['sha256']
 r=subprocess.run([tool,'--target-env','vulkan1.1',row['file']],capture_output=True,timeout=30)
 observations.append(dict(row,exit_status=r.returncode,stdout=r.stdout.decode(errors='replace'),stderr=r.stderr.decode(errors='replace')))
positive=[r for r in observations if r['kind']=='program'];controls=[r for r in observations if r['kind']=='control']
ok=all(r['exit_status']==0 for r in positive) and len(controls)==2 and all(r['exit_status']!=0 and r['stderr'] for r in controls)
version=subprocess.run([tool,'--version'],capture_output=True,check=True,timeout=10).stdout.decode()
print(json.dumps(dict(status='pass' if ok else 'fail',validator=tool,validator_sha256=hashlib.sha256(Path(tool).read_bytes()).hexdigest(),version=version,observations=observations)))
'''


def sha(data):
    return hashlib.sha256(data).hexdigest()


class Stopped(RuntimeError):
    pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--packages", type=Path, required=True)
    parser.add_argument("--host", default="coderos-4080")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    start = time.monotonic()
    def interrupted(signum, frame):
        raise Stopped("native validator interrupted")
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    work = args.work_dir.resolve(); work.mkdir(parents=True, exist_ok=True)
    snapshot = HERE / "catalog-6445ee0050.json"
    inputs = {str(path): sha(path.read_bytes()) for path in (Path(__file__).resolve(), snapshot)}
    programs, payloads = {}, {}
    declared = 0
    for profile in json.loads(snapshot.read_text())["profiles"]:
        directory = args.packages.resolve() / profile["slug"]
        manifest = directory / "render.json"; inputs[str(manifest)] = sha(manifest.read_bytes())
        package = json.loads(manifest.read_text())
        assert package["schema"] == "aethyr.render.package.v1" and package["slug"] == profile["slug"]
        for row in package["entries"]:
            assert Path(row["spirv_file"]).name == row["spirv_file"]
            path = directory / row["spirv_file"]; data = path.read_bytes(); digest = sha(data)
            assert digest == row["spirv_sha256"]; inputs[str(path)] = digest
            if digest not in programs:
                name = digest + ".spv"; payloads[name] = data
                programs[digest] = dict(file=name, sha256=digest, kind="program", owners=[])
            programs[digest]["owners"].append(dict(profile=profile["slug"], entry_index=row["entry_index"]))
            declared += 1
    first = next(iter(payloads.values()))
    bad = bytearray(first); struct.pack_into("<I", bad, 0, 0)
    payloads["bad-magic.spv"] = bytes(bad)
    words = list(struct.unpack("<" + str(len(first) // 4) + "I", first)); at = 5
    while at < len(words):
        count, opcode = words[at] >> 16, words[at] & 65535
        assert count > 0
        if opcode == 80:
            words[at + 1] = 0; break
        at += count
    assert at < len(words), "control needs an actual composite construct"
    payloads["bad-result-type.spv"] = struct.pack("<" + str(len(words)) + "I", *words)
    rows = list(programs.values()) + [dict(file=name, sha256=sha(payloads[name]), kind="control") for name in ("bad-magic.spv", "bad-result-type.spv")]
    payloads["inputs.json"] = json.dumps(rows).encode(); payloads["validate.py"] = REMOTE.encode()
    for name, data in payloads.items(): (work / name).write_bytes(data)
    archive = io.BytesIO()
    with tarfile.open(fileobj=archive, mode="w") as tar:
        for name, data in payloads.items():
            info = tarfile.TarInfo(name); info.size = len(data); tar.addfile(info, io.BytesIO(data))
    remote = '''set -eu
d=$(mktemp -d)
trap 'rm -rf "$d"' EXIT
tar -xf - -C "$d"
cd "$d"
timeout 600 nix-shell -p spirv-tools python3 --run 'python3 validate.py'
'''
    command = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2", args.host, remote]
    child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = child.communicate(archive.getvalue(), timeout=800)
    except BaseException:
        try: os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError: pass
        try: child.communicate(timeout=3)
        except BaseException:
            try: os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError: pass
            child.communicate()
        raise
    (work / "remote.stdout").write_bytes(stdout); (work / "remote.stderr").write_bytes(stderr)
    assert child.returncode == 0, stderr.decode(errors="replace")[-4000:]
    result = json.loads(stdout)
    assert len(result["observations"]) == len(rows)
    for original, observed in zip(rows, result["observations"]):
        assert all(observed[key] == value for key, value in original.items())
    changes = [path for path, digest in inputs.items() if sha(Path(path).read_bytes()) != digest]
    receipt = dict(schema="aethyr.render.spirv-validation.v1", status=result["status"] if not changes else "inputs-changed",
                   host=args.host, declared_programs=declared, unique_programs=len(programs), command=command,
                   source_inputs=inputs, source_changes=changes, remote_script_sha256=sha(REMOTE.encode()),
                   elapsed_seconds=round(time.monotonic() - start, 3), result=result,
                   scope="Exact actual package programs validated for Vulkan1.1 by the named installed validator, with two structural refusal controls. No pipeline creation, GPU execution or numerical-image claim.")
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({key: receipt[key] for key in ("status", "declared_programs", "unique_programs", "elapsed_seconds")}))
    assert receipt["status"] == "pass"


if __name__ == "__main__":
    main()
