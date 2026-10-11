"""Run only the eight reviewed scalar lifecycle/identity tests in each build."""
import json
import re
from common import ROOT, WORK, GO, LIBRARY_DIR, TEST_NAMES, freeze_check, prepared, run, save, sha, successful_phase

frozen = freeze_check()
stage = prepared()
successful_phase('build')
built = json.loads((WORK / 'build.json').read_text())
assert built['stage_sha256'] == sha(WORK / 'stage.json')
assert built['frozen_sources_sha256'] == sha(ROOT / 'frozen-sources.json')
assert not (WORK / 'tests.json').exists(), 'preserve the previous test attempt'
pattern = '^(' + '|'.join(sorted(TEST_NAMES)) + ')$'
results = {}
for variant in ['stock', 'reset']:
    path = WORK / 'bin' / variant / 'lifecycle.test'
    assert sha(path) == built['artifacts'][variant]['test_sha256']
    env = {'CLEF_SCRATCH_EXPECT_RESET': variant,
           'CLEF_SCRATCH_EXPECT_LIBRARY': str((LIBRARY_DIR / 'libmlxc.dylib').resolve())}
    listing = run([path, '-test.list', pattern], capture=True, extra=env)
    names = listing.strip().splitlines()
    assert len(names) == len(TEST_NAMES) and set(names) == TEST_NAMES, 'selected test list differs'
    raw = run([GO, 'tool', 'test2json', '-p', 'github.com/ollama/ollama/mlxrunner', '-t', path,
               '-test.v=test2json', '-test.run', pattern, '-test.count=1', '-test.parallel=1', '-test.timeout=45s'],
              capture=True, extra=env)
    (WORK / (variant + '-tests.jsonl')).write_text(raw)
    events = [json.loads(line) for line in raw.splitlines()]
    assert not any(e['Action'] in ['fail', 'skip'] for e in events), 'a selected test failed or skipped'
    completed = [e['Test'] for e in events if e['Action'] == 'pass' and 'Test' in e and '/' not in e['Test']]
    assert len(completed) == len(TEST_NAMES) and set(completed) == TEST_NAMES, 'incomplete top-level tests'
    assert any(e['Action'] == 'pass' and 'Test' not in e for e in events), 'package completion missing'
    results[variant] = {'top_level_tests': completed, 'test_sha256': sha(path),
                        'events_sha256': sha(WORK / (variant + '-tests.jsonl'))}
assert prepared() == stage and freeze_check() == frozen
save(WORK / 'tests.json', {'results': results, 'frozen_sources_sha256': sha(ROOT / 'frozen-sources.json'),
                          'stage_sha256': sha(WORK / 'stage.json'), 'build_sha256': sha(WORK / 'build.json'),
                          'model_loaded': False, 'arrays_created': False,
                          'gpu_dispatched': False, 'real_tensor_cache_cleanup_proved': False})
