"""Pinned inputs and helpers for the unexecuted scratch-only workflow."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess

ROOT = Path(__file__).resolve().parent
WORK = ROOT / 'work'
REPO = Path('$WORKTREE')
GO = Path('/opt/homebrew/Cellar/go/1.26.3/libexec/bin/go')
GOROOT = GO.parent.parent
REVISION = 'b061384d90ff455462bc32745dd0de479de717a3'
LIBRARY_DIR = Path('/Applications/Ollama.app/Contents/Resources/mlx_metal_v4')
LIBRARY_NAMES = {'libjaccl.dylib', 'libmlx.dylib', 'libmlxc.dylib', 'libollama_xgrammar.dylib', 'mlx.metallib'}
TEST_NAMES = {
    'TestScratchZeroCacheFirstAndRepeat', 'TestScratchZeroCacheDivergent',
    'TestScratchZeroCacheLongChunks', 'TestScratchZeroCacheCancelThenFresh',
    'TestScratchZeroCacheRepeatedClose', 'TestScratchGenerationUnchanged',
    'TestScratchLibraryIdentity', 'TestPrefixCacheClose',
}
SOURCE_NAMES = {'common.py', 'stage.py', 'build.py', 'run_tests.py', 'guard.py',
                'scratch_zero_cache_test.go', 'local-inventory.json', 'PLAN.md'}


def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def save(path, value):
    with Path(path).open('x') as output:
        json.dump(value, output, indent=2, allow_nan=False)
        output.write('\n')


def freeze_check():
    frozen = json.loads((ROOT / 'frozen-sources.json').read_text())
    assert set(frozen['sources']) == SOURCE_NAMES, 'source set changed'
    for name, digest in frozen['sources'].items():
        assert sha(ROOT / name) == digest, 'source changed: ' + name
    for path, digest in frozen['dependencies'].items():
        assert sha(path) == digest, 'dependency changed: ' + path
    return frozen


def tree_hashes(root):
    result = {}
    for base, dirs, names in os.walk(root, followlinks=False):
        for name in dirs:
            assert not (Path(base) / name).is_symlink(), 'source directory symlink refused'
        for name in sorted(names):
            path = Path(base) / name
            assert path.is_file() and not path.is_symlink(), 'source special file refused'
            result[str(path.relative_to(root))] = sha(path)
    return dict(sorted(result.items()))


def libraries():
    actual = {p.name for p in LIBRARY_DIR.iterdir() if p.is_file()}
    assert actual == LIBRARY_NAMES, 'installed native payload file set changed'
    inventory = json.loads((ROOT / 'local-inventory.json').read_text())['mlx_layout']
    result = {}
    for name in sorted(LIBRARY_NAMES):
        path = LIBRARY_DIR / name
        before = path.stat()
        assert {'bytes': before.st_size, 'mtime_ns': before.st_mtime_ns, 'mode': before.st_mode} == inventory[name], 'installed library metadata changed after review'
        result[name] = {'sha256': sha(path), 'bytes': before.st_size}
        after = path.stat()
        assert (before.st_size, before.st_mtime_ns, before.st_ino) == (after.st_size, after.st_mtime_ns, after.st_ino), 'library changed while hashing'
    return result


def environment(network=False):
    # Keep the real HOME unchanged. All Go writes and temporary outputs are
    # explicitly scratch-owned; no credentials or caller preload settings pass.
    keep = ['HOME', 'USER', 'LOGNAME', 'LANG', 'LC_ALL', 'OPENAGENTS_LEASES',
            'OPENAGENTS_SCRATCH', 'DEVELOPER_DIR']
    env = {key: os.environ[key] for key in keep if key in os.environ}
    env.update(PATH=str(GO.parent) + ':/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
               GOROOT=str(GOROOT), GOENV='off', GOWORK='off', GOTOOLCHAIN='local',
               GOPATH=str(WORK / 'gopath'), GOMODCACHE=str(WORK / 'modules'),
               GOCACHE=str(WORK / 'go-build'), GOTMPDIR=str(WORK / 'tmp'), TMPDIR=str(WORK / 'tmp'),
               GOMAXPROCS='2', GOFLAGS='-p=2 -mod=readonly', GOOS='darwin', GOARCH='arm64',
               CGO_ENABLED='1', CGO_CFLAGS='-O3 -mmacosx-version-min=14.0',
               CC='/usr/bin/clang', CXX='/usr/bin/clang++',
               CGO_CXXFLAGS='-O3 -mmacosx-version-min=14.0',
               CGO_LDFLAGS='-lc++ -framework Metal -framework Foundation -framework Accelerate -mmacosx-version-min=14.0',
               GOPROXY='https://proxy.golang.org' if network else 'off',
               GOSUMDB='sum.golang.org' if network else 'off',
               GOPRIVATE='', GONOPROXY='', GONOSUMDB='', GOAUTH='off',
               GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null', GIT_TERMINAL_PROMPT='0',
               OLLAMA_LLM_LIBRARY='mlx_metal_v4', RAYON_NUM_THREADS='2', OMP_NUM_THREADS='2',
               OPENBLAS_NUM_THREADS='2', VECLIB_MAXIMUM_THREADS='2')
    return env


def run(argv, *, cwd=None, network=False, capture=False, extra=None):
    env = environment(network)
    if extra:
        env.update(extra)
    argv = [str(a) for a in argv]
    logs = WORK / 'commands'
    logs.mkdir(exist_ok=True)
    number = len(list(logs.glob('*-argv.json')))
    prefix = logs / f'{number:03d}'
    record = {'command': argv, 'cwd': str(cwd) if cwd else None, 'network_enabled': network}
    save(str(prefix) + '-argv.json', record)
    print(json.dumps(record), flush=True)
    with Path(str(prefix) + '-stdout.log').open('x') as stdout, Path(str(prefix) + '-stderr.log').open('x') as stderr:
        result = subprocess.run(argv, cwd=cwd, env=env, check=False, text=True, stdout=stdout, stderr=stderr)
    save(str(prefix) + '-result.json', {'exit': result.returncode})
    result.check_returncode()
    return Path(str(prefix) + '-stdout.log').read_text() if capture else None


def declared_modules(text):
    return re.findall(r'^\s*([A-Za-z][^\s]+)\s+(v[^\s]+)', text, re.M)


def toolchain():
    assert run([GO, 'version'], capture=True).strip() == 'go version go1.26.3 darwin/arm64'
    files = [GO, GOROOT / 'pkg/tool/darwin_arm64/compile', GOROOT / 'pkg/tool/darwin_arm64/link',
             GOROOT / 'VERSION', GOROOT / 'go.env']
    result = {str(p): {'sha256': sha(p), 'bytes': p.stat().st_size} for p in files}
    clang = Path(run(['/usr/bin/xcrun', '--find', 'clang'], capture=True).strip())
    result['clang'] = {'path': str(clang), 'sha256': sha(clang),
                       'version': run(['/usr/bin/clang', '--version'], capture=True)}
    result['sdk'] = {'path': run(['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-path'], capture=True).strip(),
                     'version': run(['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-version'], capture=True).strip()}
    return result


def prepared():
    successful_phase('stage')
    stage = json.loads((WORK / 'stage.json').read_text())
    assert stage['revision'] == REVISION
    assert stage['frozen_sources_sha256'] == sha(ROOT / 'frozen-sources.json'), 'staging descriptor changed'
    assert stage['sources'] == {v: tree_hashes(WORK / v) for v in ['stock', 'reset']}, 'staged source changed'
    assert libraries() == stage['libraries'], 'installed native payload changed'
    assert toolchain() == stage['toolchain'], 'Go toolchain changed'
    return stage


def successful_phase(phase):
    path = ROOT / (phase + '-1/receipt.json')
    receipt = json.loads(path.read_text())
    assert receipt['phase'] == phase and receipt['status'] == 'complete' and receipt['exit'] == 0, 'prerequisite did not succeed'
    assert receipt['source_freeze_sha256'] == sha(ROOT / 'frozen-sources.json'), 'prerequisite belongs to another descriptor'
    assert receipt['source_identity'] == freeze_check(), 'prerequisite source identity differs'
    for key in ['error', 'cleanup_error', 'monitor_failure', 'disk_error', 'disk_sampling_error']:
        assert not receipt.get(key), 'prerequisite failure: ' + key
    memory = receipt['memory']
    assert memory['owned_group_stopped'] is True, 'prerequisite cleanup was not confirmed'
    for key in ['memory_rejection', 'sampling_error', 'cleanup_error']:
        assert not memory.get(key), 'prerequisite memory failure: ' + key
    assert receipt['disk_after_bytes'] <= 4 * 1024**3, 'prerequisite exceeded scratch storage cap'
    assert receipt['minimum_free_disk_bytes'] >= 25 * 1024**3, 'prerequisite crossed free-disk floor'
    name = {'stage': 'stage.json', 'build': 'build.json', 'test': 'tests.json'}[phase]
    assert receipt['phase_artifact'] == {'path': 'work/' + name, 'sha256': sha(WORK / name)}, 'prerequisite artifact changed'
    artifact = json.loads((WORK / name).read_text())
    assert artifact['frozen_sources_sha256'] == sha(ROOT / 'frozen-sources.json'), 'artifact descriptor changed'
    return {'receipt_sha256': sha(path), 'artifact_sha256': sha(WORK / name), 'source_freeze_sha256': receipt['source_freeze_sha256']}
