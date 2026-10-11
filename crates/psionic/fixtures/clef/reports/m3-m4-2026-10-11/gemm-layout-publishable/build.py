#!/usr/bin/env python3
"""Build the bounded probe and stage its three Flash tensors."""
from pathlib import Path
import hashlib,json,subprocess,os,sys
root=Path(__file__).resolve().parent
python=os.environ.get('CLEF_NUMPY_PYTHON',sys.executable)
commands=[['xcrun','-sdk','macosx','metal','-std=metal4.0','-c',str(root/'tiles.metal'),'-o',str(root/'tiles.air')],['xcrun','swiftc','-O','-num-threads','2',str(root/'probe.swift'),'-o',str(root/'probe')],[python,str(root/'stage_weights.py')]]
for command in commands:
 print(json.dumps({'command':command}),flush=True);subprocess.run(command,check=True)
print(json.dumps({'compiled':True,'gpu_executed':False,'binary_sha256':hashlib.sha256((root/'probe').read_bytes()).hexdigest()}),flush=True)
