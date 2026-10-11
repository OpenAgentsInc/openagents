"""Verify the frozen pipeline-limit probe without reading weight payloads."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parent
REPO = Path('$WORKTREE')
SOURCE_NAMES = ['shapes.metal', 'pointwise.metal', 'probe.swift', 'weights.json',
                'production-source.json', 'common.py', 'build.py', 'guard.py',
                'summarize.py', 'source_check.py', 'summary_check.py', 'PLAN.md', 'manifest.json', 'source-delta.json', 'probe-origin.patch']
DEPENDENCIES = ['mac_quiet_bench.py', 'mac_memory.py']

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def verify():
    frozen = json.loads((ROOT / 'frozen-sources.json').read_text())
    assert set(frozen['source_sha256']) == set(SOURCE_NAMES), 'unexpected source set'
    assert set(frozen['guard_dependency_sha256']) == set(DEPENDENCIES), 'unexpected dependency set'
    for name, digest in frozen['source_sha256'].items():
        assert sha(ROOT / name) == digest, 'source changed: ' + name
    for name, digest in frozen['guard_dependency_sha256'].items():
        assert sha(REPO / 'crates/psionic/fixtures/clef/tools' / name) == digest, 'guard changed: ' + name
    return frozen, json.loads((ROOT / 'manifest.json').read_text())
