"""Build the two frozen Go controls offline; do not run a server or test."""
from common import ROOT, WORK, GO, freeze_check, prepared, run, save, sha

frozen = freeze_check()
stage = prepared()
assert not (WORK / 'build.json').exists(), 'preserve the previous build attempt'
artifacts = {}
ldflags = '-s -w -X=github.com/ollama/ollama/version.Version=0.40.2 -X=github.com/ollama/ollama/server.mode=release'
for variant in ['stock', 'reset']:
    source = WORK / variant
    dest = WORK / 'bin' / variant
    binary, tests = dest / 'ollama', dest / 'lifecycle.test'
    assert not binary.exists() and not tests.exists(), 'existing output refused'
    run([GO, 'build', '-trimpath', '-buildvcs=false', '-ldflags', ldflags, '-o', binary, '.'], cwd=source)
    run([GO, 'test', '-c', '-trimpath', '-buildvcs=false', '-o', tests, './mlxrunner'], cwd=source)
    metadata = run([GO, 'version', '-m', binary], capture=True)
    artifacts[variant] = {'binary_sha256': sha(binary), 'binary_bytes': binary.stat().st_size,
                          'test_sha256': sha(tests), 'test_bytes': tests.stat().st_size,
                          'go_build_metadata': metadata}
assert prepared() == stage and freeze_check() == frozen
save(WORK / 'build.json', {'artifacts': artifacts, 'stage_sha256': sha(WORK / 'stage.json'),
                          'frozen_sources_sha256': sha(ROOT / 'frozen-sources.json'),
                          'network_allowed': False, 'tests_executed': False, 'model_or_gpu_executed': False})
