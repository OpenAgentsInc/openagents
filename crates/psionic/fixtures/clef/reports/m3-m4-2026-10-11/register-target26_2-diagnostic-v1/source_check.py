#!/usr/bin/env python3
"""Validate only frozen source structure and parser fixtures; run no compiler/GPU."""
import ast
import json
import re
from common import ROOT, verify

frozen, manifest = verify()
for name in frozen['source_sha256']:
    if name.endswith('.py'):
        ast.parse((ROOT / name).read_text(), filename=name)
build=(ROOT/'build.py').read_text();guard=(ROOT/'guard.py').read_text();swift=(ROOT/'exporter-template.swift').read_text()
assert build.index("proof.update(status='complete'") < build.index('commands = [')
assert "'-mmacosx-version-min=26.2'" in build and "-D__TENSOR" not in build
assert "for label, target in [('default', []), ('target26_2', ['-mmacosx-version-min=26.2'])]" in build
assert "assert len(changed) == 1" in build
assert "'swiftc','-O','-num-threads','2'" in build
assert "if guard.failure:" in guard
assert "for side in [1, 32]" in swift and swift.count('dispatchThreadgroups(')==1
assert "gpuStartTime" not in swift and "gpuEndTime" not in swift
assert 32*32*8 + 8*32*257*4 + 4*512 == 273408
for value, expected in [('260000',260000),('260200',260200),('(260200)',260200),('260200L',260200)]:
 match=re.fullmatch(r'\(?([0-9]+)(?:L)?\)?',value)
 assert match and int(match.group(1))==expected
for value in ['', '260200 + 1','undefined']:
 assert re.fullmatch(r'\(?([0-9]+)(?:L)?\)?',value) is None
assert manifest['candidate_math_or_layout_changes']==[]
print(json.dumps({'status':'source checks passed','compiler_or_gpu_executed':False,
                  'candidate_source_byte_identical':True,'host_template_byte_identical':True,
                  'commands':2,'dispatches':2,'explicit_peak_bytes':273408,
                  'source_sha256':frozen['source_sha256']},indent=2))
