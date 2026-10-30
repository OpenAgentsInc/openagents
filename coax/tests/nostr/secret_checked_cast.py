#!/usr/bin/env python3
"""Real-source audit and checked-cast behavior on native and WASI targets."""
import hashlib
import json
import shutil
import signal
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
SOURCE = HERE / 'secret_checked_cast_fixture.coax'
COMPILER = ROOT / 'coax/bin/coax'
CLANG = '/opt/homebrew/opt/llvm/bin/clang'
SYSROOT = '/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'
RESOURCE = '/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'

def run(*args):
    return subprocess.run([str(x) for x in args], capture_output=True, text=True, timeout=120)

def require(result, args):
    if result.returncode:
        raise AssertionError((args, result.returncode, result.stdout, result.stderr))
    return result.stdout

def main():
    profiles = ['secret-check', 'public-view-check', 'large-source-audit']
    entries = {'checked_u':'unsupported', 'trunc_u':'accepted', 'checked_s':'type',
               'checked_us':'unsupported', 'checked_wide':'unsupported'}
    results = []
    sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
    compiler_sha = sha(COMPILER)
    tracked = [SOURCE, HERE/'secret_checked_cast_fixture.c', HERE/'secret_checked_cast_wasm.cjs',
               *(ROOT/'tools/coax-nostr'/p/'aggregate.coax' for p in profiles)]
    initial = {str(p.resolve()):sha(p) for p in tracked}
    with tempfile.TemporaryDirectory(prefix='coax-checked-casts-') as td:
        d = Path(td)
        for profile in profiles:
            for entry, expected in entries.items():
                # Large profile v2 admits a range-contained checked cast (u8 -> u64).
                if profile == 'large-source-audit' and entry == 'checked_wide': expected = 'accepted'
                report = d / 'audit.json'
                tool = ROOT / 'tools/coax-nostr' / profile / 'aggregate.py'
                result = run(sys.executable, tool, SOURCE, '--entry', entry, '--report', report)
                data = json.loads(report.read_text())
                accepted = expected == 'accepted'
                if (result.returncode != (0 if accepted else 1) or data['accepted'] is not accepted or
                    data['error_name'] != expected or data['compiler_sha256'] != compiler_sha or
                    data['source_sha256'] != {str(SOURCE.resolve()): sha(SOURCE)} or
                    data['audit_input_sha256'].get(str((ROOT/'tools/coax-nostr'/profile/'aggregate.coax').resolve())) !=
                      initial[str((ROOT/'tools/coax-nostr'/profile/'aggregate.coax').resolve())] or
                    (accepted and data['return_dependency_mask'] != 1)):
                    raise AssertionError((profile, entry, result.returncode, data))
                results.append([profile, entry, data['accepted'], data['error_name']])
            # A checked cast remains admissible for an explicitly public operand.
            rows = [{'name':'x', 'type':'u64', 'classification':'public'}]
            if profile == 'large-source-audit':
                process = subprocess.Popen([sys.executable, str(tool), str(SOURCE), '--entry', 'checked_u',
                                            '--classification-stdio'], stdin=subprocess.PIPE,
                                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                hello = json.loads(process.stdout.readline())
                if hello['sources'] != {str(SOURCE.resolve()):sha(SOURCE)} or hello['compiler_sha256'] != compiler_sha:
                    raise AssertionError(('handshake identity', hello))
                manifest = {k:hello[k] for k in ('entry','sources','compiler_sha256','audit_inputs',
                                                 'audit_artifact_sha256','profile')}
                manifest.update(schema='coax.secret-large-classification-views.v3', views_disjoint=True,
                                live_aligned_exclusive=True, public_only=True,
                                parameters=[dict(rows[0], view=None)])
                stdout, stderr = process.communicate(json.dumps(manifest)+'\n', timeout=120)
                if process.returncode: raise AssertionError((profile, stdout, stderr))
                public = json.loads(stdout)
            else:
                manifest = {'schema':'coax.secret-classification.v1','entry':'checked_u',
                            'sources':data['source_sha256'],'parameters':rows}
                classification = d / 'classification.json'; classification.write_text(json.dumps(manifest))
                result = run(sys.executable, tool, SOURCE, '--entry', 'checked_u',
                             '--classification', classification, '--report', report)
                if result.returncode: raise AssertionError((profile, result.stderr, report.read_text()))
                public = json.loads(report.read_text())
            if (not public['accepted'] or public['return_dependency_mask'] != 0 or
                public['source_sha256'] != {str(SOURCE.resolve()):sha(SOURCE)} or
                public['compiler_sha256'] != compiler_sha):
                raise AssertionError((profile, public))
            results.append([profile, 'public-checked', True, 'public-only'])
        for mode in ('checked', 'release'):
            flags = [] if mode == 'checked' else ['--release']
            c = d / 'fixture.c'; h = d / 'fixture.h'; mapping = d / 'map.json'
            c.write_text(require(run(COMPILER, 'c', SOURCE, '--library', '--header', h,
                                     '--source-map', mapping, *flags), mode))
            if json.loads(mapping.read_text())['source_hashes'] != {str(SOURCE.resolve()): sha(SOURCE)}:
                raise AssertionError('compiled source identity changed')
            native = d / 'native'; wasm = d / 'fixture.wasm'
            require(run('cc', '-std=c11', '-O2', '-I'+str(d), c,
                        HERE/'secret_checked_cast_fixture.c', '-lm', '-o', native), 'native')
            require(run(CLANG, '--target=wasm32-wasip1', '--sysroot='+SYSROOT,
                        '-resource-dir='+RESOURCE, '-std=c11', '-O2', '-I'+str(d), c,
                        HERE/'secret_checked_cast_fixture.c', '-lm', '-o', wasm), 'wasm')
            for target in ('native', 'wasm'):
                for case in range(8):
                    args = [native, case] if target == 'native' else ['node', HERE/'secret_checked_cast_wasm.cjs', wasm, case]
                    result = run(*args)
                    trap = case in (1, 4, 5, 7)
                    if trap:
                        if result.returncode == 0 or 'cast out of range' not in result.stderr:
                            raise AssertionError((mode, target, case, result.returncode, result.stderr))
                        if target == 'native' and result.returncode != -signal.SIGABRT:
                            raise AssertionError((mode, target, case, result.returncode))
                    elif result.returncode:
                        raise AssertionError((mode, target, case, result.returncode, result.stderr))
                results.append([mode, target, 8, 'passed'])
            results.append([mode, 'artifact_sha256', {'c':sha(c),'native':sha(native),'wasm':sha(wasm),
                                                       'source_map':sha(mapping)}])
        if sha(COMPILER) != compiler_sha or any(sha(path) != digest for path,digest in initial.items()):
            raise AssertionError('fixture, policy or compiler changed during test')
        print(json.dumps({'source_sha256':sha(SOURCE),'compiler_sha256':compiler_sha,
                          'host_cc_sha256':sha(shutil.which('cc')),'wasm_clang_sha256':sha(CLANG),
                          'node_sha256':sha(shutil.which('node')),'results':results}, indent=2))

if __name__ == '__main__':
    main()
