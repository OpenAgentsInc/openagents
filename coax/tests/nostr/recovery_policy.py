#!/usr/bin/env python3
"""Finite mock of recovery policy; this is not a real checkpoint provider."""
if not __debug__: raise SystemExit('tests require Python assertions')
import hashlib
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
ADAPTER = ROOT / 'coax/tests/nostr/recovery_policy_adapter.coax'
FIXTURE = ROOT / 'coax/tests/nostr/recovery_policy_fixture.c'
COMPILER = ROOT / 'coax/bin/coax'
LLVM = '/opt/homebrew/opt/llvm/bin/clang'
SYSROOT = '/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'
RESOURCE = '/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'

def run(*args, **kw):
    return subprocess.run([str(x) for x in args], check=True, **kw)

with tempfile.TemporaryDirectory(prefix='coax-recovery-policy-') as temp:
    d = Path(temp)
    for name, flags in [('checked', []), ('release', ['--release'])]:
        source = d / (name + '.c')
        header = d / 'recovery-policy.h'
        with source.open('wb') as out:
            run(COMPILER, 'c', ADAPTER, '--library', '--header', header, *flags, stdout=out)
        native = d / (name + '-asan')
        run('cc', '-std=c11', '-O1', '-fsanitize=address,undefined', '-DNATIVE_MAIN',
            '-I' + str(d), source, FIXTURE, '-o', native)
        run(native)
        sdk = subprocess.check_output(['xcrun', '--sdk', 'iphonesimulator', '--show-sdk-path'], text=True).strip()
        run('xcrun', '--sdk', 'iphonesimulator', 'clang', '-arch', 'arm64', '-isysroot', sdk,
            '-std=c11', '-O2', '-c', source, '-o', d / (name + '-ios.o'))
        wasm = d / (name + '.wasm')
        run(LLVM, '--target=wasm32-wasip1', '--sysroot=' + SYSROOT, '-resource-dir=' + RESOURCE,
            '-std=c11', '-O2', '-mexec-model=reactor', '-I' + str(d), source, FIXTURE,
            '-Wl,--export=recovery_policy_fixture', '-o', wasm)
        js = '''const fs=require("fs"),{WASI}=require("node:wasi");
(async()=>{const w=new WASI({version:"preview1"});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]),{wasi_snapshot_preview1:w.wasiImport});w.initialize(instance);if(instance.exports.recovery_policy_fixture()!==0)process.exit(1)})().catch(e=>{console.error(e);process.exit(1)})'''
        run('node', '-e', js, wasm)
        print(name, 'native sanitizer and Wasm finite mock passed; iOS simulator compile;',
              'C', hashlib.sha256(source.read_bytes()).hexdigest(),
              'Wasm', hashlib.sha256(wasm.read_bytes()).hexdigest(), flush=True)
