#!/usr/bin/env python3
"""Four-byte load vectors (#092b): checked and release, native and Wasm.

The oracle is Python's int.from_bytes, independent of the Coax source. Each
vector fills a 64-byte buffer, loads from a borrowed view of the first n
bytes at an offset, and requires the exact value or a zero-valued refusal.
The buffer must be unchanged afterward. The 64-bit loads run alongside with
exact values, to show they keep their old behavior.

Wasm needs clang with wasm32-wasip1, wasm-ld and wasi-libc: COAXIAL_CLANG,
COAXIAL_SYSROOT and COAXIAL_RESOURCES (defaults are the Mac's Homebrew paths),
and Node for WASI.
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'coax/bin/coax'))
SOURCE = ROOT / 'coax/lib/crypto/bytes.coax'
CLANG = os.environ.get('COAXIAL_CLANG', '/opt/homebrew/opt/llvm/bin/clang')
SYSROOT = os.environ.get('COAXIAL_SYSROOT', '/opt/homebrew/opt/wasi-libc/share/wasi-sysroot')
RESOURCES = os.environ.get('COAXIAL_RESOURCES', '/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes')
DRIVER = r'''
#include <stdint.h>
#include "bytes.h"
static uint8_t buffer[64];
uint8_t *t_buffer(void) { return buffer; }
static uint64_t value;
uint64_t *t_value(void) { return &value; }
/* returns ok and stores the exact loaded value, so a nonzero refusal value is visible */
int64_t t_load(int64_t n, int64_t at, int64_t width, int64_t big) {
  coax_v_u8 view = {buffer, n};
  if (width == 4) {
    coax_t_CbU32 r = big ? cb_load_be32(view, at) : cb_load_le32(view, at);
    value = r.value;return r.ok;
  }
  coax_t_CbU64 r = big ? cb_load_be64(view, at) : cb_load_le64(view, at);
  value = r.value;return r.ok;
}
'''
RUNNER = r'''
const fs=require('fs'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
wasi.initialize(instance);const e=instance.exports;let bad=0,count=0;
for(const c of JSON.parse(fs.readFileSync(process.argv[3],'utf8'))){
 const mem=new Uint8Array(e.memory.buffer,e.t_buffer(),64);mem.set(c.buffer);
 const ok=e.t_load(BigInt(c.n),BigInt(c.at),BigInt(c.width),BigInt(c.big));count++;
 const v=new BigUint64Array(e.memory.buffer,e.t_value(),1)[0];
 if(ok!==BigInt(c.ok)||v!==BigInt(c.want)){bad++;if(bad<5)console.log('mismatch',JSON.stringify(c),String(ok),String(v));}
 for(let i=0;i<64;i++)if(mem[i]!==c.buffer[i]){bad++;console.log('buffer changed',i);break;}}
console.log(count,'cases',bad,'failures');process.exit(bad?1:0);})().catch(e=>{console.error(e);process.exit(1)});
'''

def run(*args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode:raise RuntimeError(f'{args[0]} failed ({r.returncode}): {r.stdout} {r.stderr}')
    return r

def expected(buffer, n, at, width, big):
    if n < 0 or n > 1048576 or at < 0 or at > n or width > n - at:return 0, 0
    return 1, int.from_bytes(bytes(buffer[at:at + width]), 'big' if big else 'little')

def vectors():
    rng = random.Random(0x092B);cases = []
    patterns = [[0] * 64, [255] * 64, list(range(64)), [rng.getrandbits(8) for _ in range(64)], [128, 0, 0, 1] * 16]
    for buffer in patterns:
        for n in list(range(0, 13)) + [63, 64]:
            for at in [-2, -1, -2**63] + list(range(0, n + 2)) + [2**62, 2**63 - 1]:
                for width in [4, 8]:
                    for big in [0, 1]:
                        ok, want = expected(buffer, n, at, width, big)
                        cases.append({'buffer': buffer, 'n': n, 'at': at, 'width': width, 'big': big, 'ok': ok, 'want': want})
    # Malformed borrowed extents (negative, or above the 1 MiB span bound) are
    # refused by the span check before any byte is read.
    for n in [-1, -2**63, 1048577, 2**63 - 1]:
        for width in [4, 8]:
            for big in [0, 1]:cases.append({'buffer': patterns[2], 'n': n, 'at': 0, 'width': width, 'big': big, 'ok': 0, 'want': 0})
    return cases

class Load32(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='coax-load32-');cls.dir = Path(cls.temp.name)
        cls.cases = vectors();cls.path = cls.dir / 'cases.json';cls.path.write_text(json.dumps([{**c, 'want': str(c['want']), 'n': str(c['n']), 'at': str(c['at'])} for c in cls.cases]))  # strings keep 64-bit values exact in JS
        (cls.dir / 'driver.c').write_text(DRIVER);(cls.dir / 'runner.cjs').write_text(RUNNER)
        cls.record = {'schema': 'coax.load32-vectors.v1', 'cases_per_target': len(cls.cases),
                      'source_sha256': hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
                      'compiler': run(COMPILER, '--version').stdout.strip(), 'compiler_sha256': hashlib.sha256(COMPILER.read_bytes()).hexdigest(),
                      'oracle': 'Python int.from_bytes', 'targets': {}}
        cls.c = {}
        for mode, flags in [('checked', []), ('release', ['--release'])]:
            d = cls.dir / mode;d.mkdir()
            (d / 'bytes.c').write_text(run(COMPILER, 'c', SOURCE, '--library', '--header', d / 'bytes.h', *flags).stdout)
            cls.c[mode] = (d / 'bytes.c').read_text()
    @classmethod
    def tearDownClass(cls):
        Path(os.environ.get('COAX_LOAD32_REPORT', '/tmp/coax-load32-vectors.json')).write_text(json.dumps(cls.record, indent=2) + '\n')
        cls.temp.cleanup()
    def check_native(self, mode):
        d = self.dir / mode;lib = d / 'load.so'
        run('cc', '-std=c11', '-O2', '-shared', '-fPIC', '-I' + str(d), d / 'bytes.c', self.dir / 'driver.c', '-lm', '-o', lib)
        obj = C.CDLL(str(lib));obj.t_buffer.restype = C.POINTER(C.c_uint8);obj.t_value.restype = C.POINTER(C.c_uint64)
        obj.t_load.argtypes = [C.c_int64] * 4;obj.t_load.restype = C.c_int64
        for case in self.cases:
            buf = bytes(case['buffer']);C.memmove(obj.t_buffer(), buf, 64)
            self.assertEqual(obj.t_load(case['n'], case['at'], case['width'], case['big']), case['ok'], case)
            self.assertEqual(obj.t_value()[0], case['want'], case)
            self.assertEqual(bytes(obj.t_buffer()[:64]), buf, case)
        self.record['targets'][f'native-{mode}'] = {'passed': len(self.cases), 'library_sha256': hashlib.sha256(lib.read_bytes()).hexdigest()}
    def check_wasm(self, mode):
        d = self.dir / mode;wasm = d / 'load.wasm'
        run(CLANG, '--target=wasm32-wasip1', '--sysroot=' + SYSROOT, '-resource-dir=' + RESOURCES, '-std=c11', '-O2', '-mexec-model=reactor',
            '-I' + str(d), d / 'bytes.c', self.dir / 'driver.c', '-Wl,--export=t_buffer,--export=t_value,--export=t_load', '-o', wasm)
        out = run('node', self.dir / 'runner.cjs', wasm, self.path).stdout
        self.assertIn(f'{len(self.cases)} cases 0 failures', out)
        self.record['targets'][f'wasm-{mode}'] = {'passed': len(self.cases), 'wasm_sha256': hashlib.sha256(wasm.read_bytes()).hexdigest()}
    def test_native_checked(self):self.check_native('checked')
    def test_native_release(self):self.check_native('release')
    def test_wasm_checked(self):self.check_wasm('checked')
    def test_wasm_release(self):self.check_wasm('release')
    def test_emitted_four_byte_load_has_no_checked_cast(self):
        for mode, c in self.c.items():
            body = re.search(r'^static coax_t_CbU32 coax_p_cb_load_word32\(.*?\{\n(.*?)^\}', c, re.S | re.M)
            self.assertIsNotNone(body, mode);body = body.group(1)
            self.assertNotIn('coax_cast_', body, mode)
            for wrapper in ['cb_load_be32', 'cb_load_le32']:
                w = re.search(r'^coax_t_CbU32 ' + wrapper + r'\(coax_v_u8 l_data, int64_t l_at\) \{\n(.*?)^\}', c, re.S | re.M).group(1)
                self.assertNotIn('coax_cast_', w, (mode, wrapper));self.assertIn('coax_p_cb_load_word32', w)
        self.record['emitted_c'] = 'cb_load_word32 and its two wrappers call no checked-cast helper in checked or release C'

if __name__ == '__main__':unittest.main()
