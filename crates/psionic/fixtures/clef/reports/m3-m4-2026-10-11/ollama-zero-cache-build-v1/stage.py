"""Fetch only pinned public inputs after a separate staging handoff."""
import json
import re
import shutil
from pathlib import Path
from common import ROOT, WORK, REVISION, GO, LIBRARY_DIR, declared_modules, freeze_check, libraries, run, save, sha, toolchain, tree_hashes

frozen = freeze_check()
assert not WORK.exists(), 'preserve the previous staging attempt'
WORK.mkdir()
for name in ['tmp', 'modules', 'go-build', 'gopath']:
    (WORK / name).mkdir()
compiler = toolchain()
native = libraries()
source = WORK / 'public-git'
run(['git', 'init', source])
git = ['git', '-c', 'credential.helper=', '-c', 'core.hooksPath=/dev/null', '-c', 'core.fsmonitor=false', '-c', 'protocol.file.allow=never']
run(git + ['fetch', '--depth=1', 'https://github.com/ollama/ollama.git', REVISION], cwd=source, network=True)
actual = run(git + ['rev-parse', 'FETCH_HEAD^{commit}'], cwd=source, capture=True).strip()
assert actual == REVISION, 'fetched a different commit'
run(git + ['checkout', '--detach', REVISION], cwd=source)
run(git + ['diff', '--exit-code', REVISION, '--'], cwd=source)
tracked = run(git + ['ls-files', '-z'], cwd=source, capture=True).split('\0')
tracked = [name for name in tracked if name]
assert tracked and all(not Path(name).is_absolute() and '..' not in Path(name).parts for name in tracked)
for variant in ['stock', 'reset']:
    dest = WORK / variant
    dest.mkdir()
    for name in tracked:
        src, target = source / name, dest / name
        assert src.is_file() and not src.is_symlink(), 'tracked symlink or missing submodule is outside this staging plan'
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, target)
    assert sha(dest / 'go.mod') == sha(ROOT.parent / 'stock-runner/go.mod')
    test = dest / 'mlxrunner/scratch_zero_cache_test.go'
    assert not test.exists(), 'upstream test name collided'
    shutil.copyfile(ROOT / 'scratch_zero_cache_test.go', test)
old_prefix = ROOT.parent.parent / 'attention-qk128-key32-flash-latency/ollama-source-review/mlxrunner-prefix_cache.go'
assert sha(WORK / 'stock/mlxrunner/prefix_cache.go') == sha(old_prefix)
assert sha(WORK / 'stock/mlxrunner/prefix_cache_test.go') == sha(ROOT.parent / 'stock-runner/mlxrunner-prefix_cache_test.go')
patch = ROOT.parent / 'candidate.patch'
run(git + ['apply', '--check', patch], cwd=WORK / 'reset')
run(git + ['apply', patch], cwd=WORK / 'reset')
source_hashes = {v: tree_hashes(WORK / v) for v in ['stock', 'reset']}
changed = [name for name in source_hashes['stock'] if source_hashes['stock'][name] != source_hashes['reset'][name]]
assert set(source_hashes['stock']) == set(source_hashes['reset']) and changed == ['mlxrunner/prefix_cache.go']

# Reuse only declared public cache entries. Never point Go at the user's cache
# for writes, and verify both archive content and the pinned go.sum afterward.
cache = Path('$HOME/go/pkg/mod/cache/download')
seeded = []
def escape(value):
    return ''.join('!' + char.lower() if char.isupper() else char for char in value)
modules = declared_modules((WORK / 'stock/go.mod').read_text())
for module, version in modules:
    relative = Path(escape(module)) / '@v'
    base = escape(version)
    for suffix in ['.zip', '.mod', '.info', '.ziphash']:
        src = cache / relative / (base + suffix)
        if src.is_file() and not src.is_symlink():
            dst = WORK / 'modules/cache/download' / relative / (base + suffix)
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, dst)
            seeded.append({'relative': str(relative / dst.name), 'sha256': sha(dst), 'bytes': dst.stat().st_size})
output = run([GO, 'mod', 'download', '-json'], cwd=WORK / 'stock', network=True, capture=True)
(WORK / 'module-download.jsonl').write_text(output)
decoder, rest, downloaded = json.JSONDecoder(), output.strip(), []
while rest:
    item, end = decoder.raw_decode(rest)
    downloaded.append(item)
    rest = rest[end:].lstrip()
expected_sums = {}
for line in (WORK / 'stock/go.sum').read_text().splitlines():
    module, version, digest = line.split()
    expected_sums[(module, version)] = digest
assert {(r['Path'], r['Version']) for r in downloaded} == set(modules), 'module download coverage differs'
for row in downloaded:
    assert not row.get('Error'), 'module download failed'
    for key, version in [('Sum', row['Version']), ('GoModSum', row['Version'] + '/go.mod')]:
        assert row[key] == expected_sums[(row['Path'], version)], 'module sum differs from pinned source'
run([GO, 'mod', 'verify'], cwd=WORK / 'stock')
assert source_hashes == {v: tree_hashes(WORK / v) for v in ['stock', 'reset']}, 'module staging changed sources'
for variant in ['stock', 'reset']:
    binary_dir = WORK / 'bin' / variant
    link = binary_dir / 'lib/ollama/mlx_metal_v4'
    link.parent.mkdir(parents=True)
    link.symlink_to(LIBRARY_DIR, target_is_directory=True)
assert libraries() == native and toolchain() == compiler
assert freeze_check() == frozen
save(WORK / 'stage.json', {'revision': REVISION, 'sources': source_hashes, 'toolchain': compiler,
                          'frozen_sources_sha256': sha(ROOT / 'frozen-sources.json'),
                          'libraries': native, 'seeded_cache_files': seeded,
                          'downloaded_module_count': len(downloaded),
                          'changed_product_files': changed, 'patch_sha256': sha(patch),
                          'model_or_gpu_executed': False})
