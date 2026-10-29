#!/usr/bin/env python3
"""Independent arithmetic oracle for the checked and release native core."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes
import hashlib
import random
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'coax/lib/crypto/salsa20_8.coax'
COMPILER = ROOT / 'coax/bin/coax'
MASK = (1 << 32) - 1


def reference(words):
    x = list(words)
    def quarter(indices):
        a, b, c, d = indices
        for dst, left, right, shift in ((b, a, d, 7), (c, b, a, 9), (d, c, b, 13), (a, d, c, 18)):
            value = (x[left] + x[right]) & MASK
            x[dst] ^= ((value << shift) | (value >> (32 - shift))) & MASK
    for _ in range(4):
        for indices in ((0,4,8,12),(5,9,13,1),(10,14,2,6),(15,3,7,11),
                        (0,1,2,3),(5,6,7,4),(10,11,8,9),(15,12,13,14)):
            quarter(indices)
    return [(a + b) & MASK for a, b in zip(x, words)]


class View(ctypes.Structure):
    _fields_ = [('data', ctypes.POINTER(ctypes.c_uint32)), ('count', ctypes.c_int64)]


def main():
    rng = random.Random(7914)
    print('compiler sha256:', hashlib.sha256(COMPILER.read_bytes()).hexdigest(), flush=True)
    print('source sha256:', hashlib.sha256(SOURCE.read_bytes()).hexdigest(), flush=True)
    fixtures = [[0] * 16, [MASK] * 16] + [[rng.getrandbits(32) for _ in range(16)] for _ in range(2000)]
    with tempfile.TemporaryDirectory(prefix='coax-salsa8-') as temp:
        directory = Path(temp)
        for mode, flags in [('checked', []), ('release', ['--release'])]:
            cfile, library = (directory / (mode + ext) for ext in ('.c', '.dylib'))
            header = directory / 'salsa20_8.h'
            result = subprocess.run([str(COMPILER), 'c', str(SOURCE), '--library', '--header', str(header), *flags], check=True, capture_output=True)
            cfile.write_bytes(result.stdout)
            subprocess.run(['cc', '-std=c11', '-O2', '-shared', '-fPIC', str(cfile), '-o', str(library)], check=True)
            executable = directory / (mode + '-sanitized')
            subprocess.run(['cc', '-std=c11', '-O1', '-fsanitize=address,undefined', '-I' + str(directory),
                            str(cfile), str(ROOT / 'coax/tests/nostr/salsa20_8_native.c'), '-o', str(executable)], check=True)
            subprocess.run([str(executable)], check=True)
            fn = ctypes.CDLL(str(library)).salsa20_8_core
            fn.argtypes = [View, View, ctypes.c_bool]
            fn.restype = ctypes.c_bool
            for words in fixtures:
                original = (ctypes.c_uint32 * 16)(*words)
                result = (ctypes.c_uint32 * 16)()
                assert fn(View(original, 16), View(result, 16), False)
                assert list(result) == reference(words)
            print(f'{mode}: {len(fixtures)} independent inputs passed')
            sdk = subprocess.check_output(['xcrun', '--sdk', 'iphonesimulator', '--show-sdk-path'], text=True).strip()
            subprocess.run(['xcrun', '--sdk', 'iphonesimulator', 'clang', '-arch', 'arm64', '-isysroot', sdk,
                            '-std=c11', '-O2', '-c', str(cfile), '-o', str(directory / (mode + '-ios.o'))], check=True)
            clang = '/opt/homebrew/opt/llvm/bin/clang'
            sysroot = '/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'
            resources = '/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'
            module = directory / (mode + '.wasm')
            subprocess.run([clang, '--target=wasm32-wasip1', '--sysroot=' + sysroot, '-resource-dir=' + resources,
                            '-std=c11', '-O2', '-I' + str(directory), '-mexec-model=reactor', str(cfile),
                            str(ROOT / 'coax/tests/nostr/salsa20_8_wasm.c'),
                            '-Wl,--export=salsa_input,--export=salsa_output,--export=salsa_run,--export=salsa_refusal,--export=salsa_tail', '-o', str(module)], check=True)
            print(f'{mode} C sha256: {hashlib.sha256(cfile.read_bytes()).hexdigest()}', flush=True)
            print(f'{mode} Wasm sha256: {hashlib.sha256(module.read_bytes()).hexdigest()}', flush=True)
            subprocess.run(['node', str(ROOT / 'coax/tests/nostr/salsa20_8_wasm.cjs'), str(module)], check=True)


if __name__ == '__main__':
    main()
