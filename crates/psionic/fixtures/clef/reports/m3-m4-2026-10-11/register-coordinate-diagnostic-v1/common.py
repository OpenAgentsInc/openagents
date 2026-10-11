"""Verify this diagnostic and its unchanged compiled shader before launch."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parent
REPO = Path('<REPO>')
SOURCE_NAMES = ['exporter.swift', 'common.py', 'build.py', 'guard.py', 'analyze.py',
                'source_check.py', 'PLAN.md', 'manifest.json']
DEPENDENCIES = ['mac_quiet_bench.py', 'mac_memory.py']

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def verify():
    frozen = json.loads((ROOT / 'frozen-sources.json').read_text())
    assert set(frozen['source_sha256']) == set(SOURCE_NAMES), 'unexpected source set'
    assert set(frozen['guard_dependency_sha256']) == set(DEPENDENCIES), 'unexpected guard dependency set'
    for name, digest in frozen['source_sha256'].items():
        assert sha(ROOT / name) == digest, f'source changed: {name}'
    for name, digest in frozen['guard_dependency_sha256'].items():
        assert sha(REPO / 'crates/psionic/fixtures/clef/tools' / name) == digest, f'guard changed: {name}'
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    for entry in manifest['unchanged_inputs']:
        path = (ROOT / entry['path']).resolve()
        assert path.parent == ROOT.parent / 'gemm-register-input-probe', 'unexpected compiled-input directory'
        assert sha(path) == entry['sha256'], f'compiled input changed: {path.name}'
    return frozen, manifest
