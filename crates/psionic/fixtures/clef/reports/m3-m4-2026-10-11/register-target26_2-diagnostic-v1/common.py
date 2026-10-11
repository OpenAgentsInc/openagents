"""Verify frozen sources and unchanged inputs for the fixed deployment diagnostic."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parent
SDK_ROOT = Path('/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk')
SDK_HEADERS = SDK_ROOT / 'System/Library/Frameworks/MetalPerformancePrimitives.framework/Versions/A/Headers'
REPO = Path('<REPO>')
SOURCE_NAMES = ['candidate.metal', 'exporter-template.swift', 'deployment_probe.metal',
                'common.py', 'build.py', 'guard.py', 'analyze.py', 'source_check.py', 'PLAN.md', 'manifest.json']
DEPENDENCIES = ['mac_quiet_bench.py', 'mac_memory.py']

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def verify():
    frozen = json.loads((ROOT / 'frozen-sources.json').read_text())
    assert set(frozen['source_sha256']) == set(SOURCE_NAMES), 'unexpected source set'
    assert set(frozen['guard_dependency_sha256']) == set(DEPENDENCIES), 'unexpected dependency set'
    for name, digest in frozen['source_sha256'].items():
        assert sha(ROOT / name) == digest, f'source changed: {name}'
    for name, digest in frozen['guard_dependency_sha256'].items():
        assert sha(REPO / 'crates/psionic/fixtures/clef/tools' / name) == digest, f'guard changed: {name}'
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    for name, digest in manifest['sdk_header_sha256'].items():
        assert sha(SDK_HEADERS / name) == digest, f'SDK header changed: {name}'
    for entry in manifest['unchanged_inputs']:
        path = (ROOT / entry['path']).resolve()
        assert path.parent in [ROOT.parent / 'gemm-register-input-probe', ROOT.parent / 'register-coordinate-diagnostic'], 'unexpected input directory'
        assert sha(path) == entry['sha256'], f'original input changed: {path}'
        if entry.get('copy'):
            assert (ROOT / entry['copy']).read_bytes() == path.read_bytes(), 'source copy differs'
    return frozen, manifest
