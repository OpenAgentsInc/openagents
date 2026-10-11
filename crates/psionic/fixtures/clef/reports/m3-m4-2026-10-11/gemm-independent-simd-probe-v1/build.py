#!/usr/bin/env python3
"""Compile frozen scratch sources without reading weight payloads."""
from pathlib import Path
import hashlib,json,subprocess
root=Path(__file__).resolve().parent
names=['baseline.metal', 'candidate.metal', 'production-shapes.metal', 'production-fixed-extract.metal', 'production-silu-extract.metal', 'production-planner.rs', 'probe.swift', 'weights.json', 'build.py', 'guard.py', 'manifest.json', 'summarize.py', 'source_check.py', 'README.md', 'half_ulp_check.py', 'half-ulp-fixtures.json', 'criterion-revision-v2.json']
def hashes(): return {name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in names}
before=hashes()
commands=[]
for name in ['baseline.metal','candidate.metal']:
 flags=['-std=metal4.0']
 commands.append(['xcrun','-sdk','macosx','metal']+flags+['-c',str(root/name),'-o',str(root/(name+'.air'))])
commands.append(['xcrun','-sdk','macosx','metallib']+[str(root/(name+'.air')) for name in ['baseline.metal','candidate.metal']]+['-o',str(root/'kernels.metallib')])
commands.append(['xcrun','swiftc','-O','-num-threads','2',str(root/'probe.swift'),'-o',str(root/'probe')])
for command in commands:
 print(json.dumps({'command':command}),flush=True);subprocess.run(command,check=True)
if hashes()!=before: raise SystemExit('sources changed during build')
record={'source_sha256':before,'artifacts':{name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in ['probe','kernels.metallib']},'gpu_executed':False}
(root/'build-artifacts.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record),flush=True)
