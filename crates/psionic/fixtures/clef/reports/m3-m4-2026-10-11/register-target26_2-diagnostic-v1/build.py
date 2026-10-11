#!/usr/bin/env python3
"""Prove the deployment branch, then build one fixed-target coordinate exporter."""
import difflib
import json
import os
import platform
import re
import subprocess
from common import ROOT, SDK_ROOT, sha, verify

frozen, manifest = verify()
outputs = ['default.macros', 'default.stderr', 'target26_2.macros', 'target26_2.stderr',
           'preprocessor-proof.json', 'candidate-target26_2.air', 'target26_2.metallib',
           'exporter-target26_2.swift', 'host-library-hash.patch', 'exporter-target26_2', 'build-artifacts.json']
assert not any((ROOT / name).exists() for name in outputs), 'preserve prior attempt; no automatic rebuild'
version = tuple(int(value) for value in platform.mac_ver()[0].split('.'))
assert version >= (26, 2), 'fixed deployment target exceeds host OS'
proof = {'status':'running', 'host_os':platform.mac_ver()[0], 'variant':'one fixed macOS26.2 deployment minimum',
         'candidate_source_sha256':sha(ROOT / 'candidate.metal'), 'cases':[], 'gemm_executed':False,
         'sdk_header_sha256':manifest['sdk_header_sha256'],
         'deployment_environment':{key:os.environ.get(key) for key in ['DEVELOPER_DIR','SDKROOT','MACOSX_DEPLOYMENT_TARGET','TOOLCHAINS']}}
def save():
    (ROOT / 'preprocessor-proof.json').write_text(json.dumps(proof, indent=2) + '\n')
def run(command):
    print(json.dumps({'command':command}), flush=True)
    subprocess.run(command, check=True)
def definitions(text):
    return dict(re.findall(r'^#define\s+(\S+)\s+(.+)$', text, flags=re.MULTILINE))
try:
    save()
    sdk_command = ['xcrun','-sdk','macosx','--show-sdk-path']
    print(json.dumps({'command':sdk_command}), flush=True)
    sdk_result = subprocess.run(sdk_command, capture_output=True, text=True)
    proof['sdk_resolution'] = {'command':sdk_command, 'exit':sdk_result.returncode, 'stdout':sdk_result.stdout, 'stderr':sdk_result.stderr}; save()
    assert sdk_result.returncode == 0 and SDK_ROOT.resolve() == type(SDK_ROOT)(sdk_result.stdout.strip()).resolve(), 'selected SDK differs from inspected headers'
    for label, target in [('default', []), ('target26_2', ['-mmacosx-version-min=26.2'])]:
        command = ['xcrun','-sdk','macosx','metal','-std=metal4.0'] + target + ['-E','-dM',str(ROOT/'deployment_probe.metal')]
        print(json.dumps({'command':command}), flush=True)
        with (ROOT / (label + '.macros')).open('xb') as out, (ROOT / (label + '.stderr')).open('xb') as err:
            result = subprocess.run(command, stdout=out, stderr=err)
        defs = definitions((ROOT / (label + '.macros')).read_text())
        row = {'label':label, 'command':command, 'exit':result.returncode,
               'macros_sha256':sha(ROOT / (label+'.macros')), 'stderr_sha256':sha(ROOT / (label+'.stderr')),
               'selected':{key:defs.get(key) for key in ['__ENVIRONMENT_OS_VERSION_MIN_REQUIRED__',
                    'CLEF_COORDINATE_OPERAND_AWARE','__METAL_VERSION__','__clang_version__',
                    '__TENSOR_OPS_SUPPORT_DEPLOYMENT_TARGET_26_2']}}
        proof['cases'].append(row); save()
        assert result.returncode == 0, 'preprocessor failed; stop without target alternatives'
        raw = defs.get('__ENVIRONMENT_OS_VERSION_MIN_REQUIRED__', '')
        match = re.fullmatch(r'\(?([0-9]+)(?:L)?\)?', raw)
        assert match is not None, 'unexpected deployment macro representation'
        minimum = int(match.group(1)); aware = defs.get('CLEF_COORDINATE_OPERAND_AWARE')
        row.update(effective_minimum=minimum, operand_aware=aware); save()
    default, targeted = proof['cases']
    assert default['effective_minimum'] < 260200 and default['operand_aware'] == '0', 'original deployment prediction failed'
    assert targeted['effective_minimum'] == 260200 and targeted['operand_aware'] == '1', 'fixed deployment prediction failed'
    assert verify() == (frozen, manifest), 'inputs changed during preprocessing'
    proof.update(status='complete', predicted_branch_change_confirmed=True); save()
except BaseException as error:
    proof.update(status='failed', error_type=type(error).__name__, error=str(error)); save(); raise

commands = [
 ['xcrun','-sdk','macosx','metal','-std=metal4.0','-mmacosx-version-min=26.2','-c',str(ROOT/'candidate.metal'),'-o',str(ROOT/'candidate-target26_2.air')],
 ['xcrun','-sdk','macosx','metallib',str(ROOT/'candidate-target26_2.air'),'-o',str(ROOT/'target26_2.metallib')]]
for command in commands:
    run(command)
library_hash = sha(ROOT / 'target26_2.metallib')
source = (ROOT / 'exporter-template.swift').read_text()
old = manifest['prior_library_sha256']
assert source.count(old) == 1, 'expected one pinned host library hash'
generated = source.replace(old, library_hash)
assert len(source.splitlines()) == len(generated.splitlines())
changed = [(a,b) for a,b in zip(source.splitlines(), generated.splitlines()) if a != b]
assert len(changed) == 1 and changed[0] == (f'let pinnedLibraryHash = "{old}"', f'let pinnedLibraryHash = "{library_hash}"'), 'host changed outside library identity'
with (ROOT/'exporter-target26_2.swift').open('x') as output:
    output.write(generated)
with (ROOT/'host-library-hash.patch').open('x') as output:
    output.writelines(difflib.unified_diff(source.splitlines(True), generated.splitlines(True), fromfile='exporter-template.swift', tofile='exporter-target26_2.swift'))
run(['xcrun','swiftc','-O','-num-threads','2',str(ROOT/'exporter-target26_2.swift'),'-o',str(ROOT/'exporter-target26_2')])
assert verify() == (frozen, manifest), 'inputs changed during target build'
result = {'source_sha256':frozen['source_sha256'], 'frozen_sources_sha256':sha(ROOT/'frozen-sources.json'),
          'library_sha256':library_hash, 'sdk_header_sha256':manifest['sdk_header_sha256'], 'artifacts':{name:sha(ROOT/name) for name in ['target26_2.metallib','exporter-target26_2.swift','host-library-hash.patch','exporter-target26_2','preprocessor-proof.json']},
          'candidate_source_unchanged':True, 'host_change':'One pinned library hash literal only', 'gemm_executed':False}
with (ROOT/'build-artifacts.json').open('x') as output:
    json.dump(result, output, indent=2); output.write('\n')
print(json.dumps(result), flush=True)
