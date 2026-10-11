#!/usr/bin/env python3
"""Compile only the three frozen shader sources and bounded standalone harness."""
from pathlib import Path
import hashlib,json,subprocess
root=Path(__file__).resolve().parent
commands=[['xcrun','-sdk','macosx','metal','-std=metal4.0','-c',str(root/name),'-o',str(root/(name+'.air'))] for name in ['baseline.metal','candidate.metal','support.metal']]
commands.append(['xcrun','swiftc','-O','-num-threads','2',str(root/'probe.swift'),'-o',str(root/'probe')])
for command in commands:
 print(json.dumps({'command':command}),flush=True)
 subprocess.run(command,check=True)
print(json.dumps({'compiled':True,'gpu_executed':False,'binary_sha256':hashlib.sha256((root/'probe').read_bytes()).hexdigest()}),flush=True)
