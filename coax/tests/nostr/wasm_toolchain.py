"""The Wasm toolchain for the crypto harnesses, found the way coax/web/toolchain.sh finds it.

COAXIAL_CLANG, COAXIAL_SYSROOT and COAXIAL_RESOURCES win when set. Otherwise
toolchain.sh picks Homebrew's llvm, lld, wasi-libc and wasi-runtimes on macOS,
or the nixpkgs toolchain linked into ~/.cache/coax-wasm-toolchain on Linux.
Importing this module exports the three variables and puts wasm-ld on PATH,
so every clang and node subprocess the harness starts sees the same tools.
"""
import os
import platform
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def _defaults():
    if platform.system() == 'Darwin':
        brew = '/opt/homebrew/opt/'
        return (brew + 'llvm/bin/clang', brew + 'wasi-libc/share/wasi-sysroot',
                brew + 'wasi-runtimes/share/wasi-runtimes', brew + 'lld/bin')
    tc = Path(os.environ.get('COAXIAL_TOOLCHAIN', Path.home() / '.cache/coax-wasm-toolchain'))
    return (str(tc / 'bin/clang'), str(tc / 'sysroot'), str(tc / 'resource'), str(tc / 'bin'))


def _resolve():
    script = 'source coax/web/toolchain.sh; printf "%s\\0" "$COAXIAL_CLANG" "$COAXIAL_SYSROOT" "$COAXIAL_RESOURCES" "$PATH"'
    try:
        out = subprocess.run(['bash', '-c', script], cwd=ROOT, capture_output=True, check=True).stdout.split(b'\0')
        if len(out) == 5 and out[-1] == b'':
            return [part.decode() for part in out[:4]]
    except (OSError, subprocess.CalledProcessError):
        pass
    clang, sysroot, resources, bindir = _defaults()
    return [os.environ.get('COAXIAL_CLANG', clang), os.environ.get('COAXIAL_SYSROOT', sysroot),
            os.environ.get('COAXIAL_RESOURCES', resources), bindir + os.pathsep + os.environ.get('PATH', '')]


WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES, _path = _resolve()
os.environ.update(COAXIAL_CLANG=WASM_CLANG, COAXIAL_SYSROOT=WASM_SYSROOT,
                  COAXIAL_RESOURCES=WASM_RESOURCES, PATH=_path)
