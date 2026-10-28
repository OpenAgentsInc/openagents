#!/usr/bin/env python3
"""value_barrier (spec §9): a masked select compiles without a conditional branch."""
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = ROOT / 'bin/coax'
SOURCE = '''
ct_select :: (bit: u64, a: u64, b: u64) -> u64 #export_c {
  mask := value_barrier(sub_wrap(cast(u64) 0, bit));
  return b ^ ((a ^ b) & mask);
}
ct_select32 :: (bit: u32, a: u32, b: u32) -> u32 #export_c {
  mask := value_barrier(sub_wrap(cast(u32) 0, bit));
  return b ^ ((a ^ b) & mask);
}
#assert #run value_barrier(cast(u64) 7) == 7;
'''
# conditional branches: arm64 b.cond/cbz/cbnz/tbz/tbnz, x86 jcc (not jmp)
BRANCH = re.compile(r'\t(b\.[a-z]{2}|cbn?z|tbn?z|j(?!mp)[a-z]{1,3})\s')

def disassembly(obj, name):
    tool = shutil.which('objdump') or shutil.which('llvm-objdump')
    symbol = ('_' if platform.system() == 'Darwin' else '') + name   # Mach-O prefixes C names
    out = subprocess.run([tool, '-d', f'--disassemble-symbols={symbol}', str(obj)], capture_output=True, text=True, check=True).stdout
    m = re.search(r'<' + symbol + r'>:\n(.*?)(?:\n\n|\Z)', out, re.S)
    return m.group(1) if m else ''

class ConstantTime(unittest.TestCase):
    def build(self, d, *mode):
        src = d/'ct.coax'; src.write_text(SOURCE)
        r = subprocess.run([str(COMPILER), 'c', str(src), '--library', '--header', str(d/'ct.h'), *mode], capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        (d/'ct.c').write_text(r.stdout)
        return r.stdout

    def test_masked_select_has_no_conditional_branch(self):
        with tempfile.TemporaryDirectory() as directory:
            d = Path(directory)
            c = self.build(d, '--release')
            self.assertIn('__asm__("" : "+r"(x))', c)
            cc = shutil.which('clang') or os.environ.get('CC', 'cc')
            r = subprocess.run([cc, '-std=c11', '-pedantic-errors', '-O2', '-c', str(d/'ct.c'), '-o', str(d/'ct.o')], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            for name in ('ct_select', 'ct_select32'):
                body = disassembly(d/'ct.o', name)
                self.assertTrue(body.strip(), f'{name} not found in the object')
                self.assertIsNone(BRANCH.search(body), f'{name} branches:\n{body}')
            # and it still selects
            (d/'host.c').write_text('#include "ct.h"\nint main(void){return ct_select(1,5,9)==5 && ct_select(0,5,9)==9 && ct_select32(1,5,9)==5 && ct_select32(0,5,9)==9 ? 0 : 1;}')
            r = subprocess.run([cc, '-std=c11', '-O2', str(d/'ct.c'), str(d/'host.c'), '-o', str(d/'host')], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(subprocess.run([str(d/'host')]).returncode, 0)

    def test_checked_masked_select_does_not_underflow(self):
        with tempfile.TemporaryDirectory() as directory:
            d = Path(directory)
            self.build(d)
            (d/'host.c').write_text('#include "ct.h"\nint main(void){return ct_select(1,5,9)==5 && ct_select(0,5,9)==9 && ct_select32(1,5,9)==5 && ct_select32(0,5,9)==9 ? 0 : 1;}')
            subprocess.run(['clang', '-std=c11', '-O2', str(d/'ct.c'), str(d/'host.c'), '-o', str(d/'host')], check=True, capture_output=True)
            self.assertEqual(subprocess.run([str(d/'host')]).returncode, 0)

    def test_wasm32_calls_an_identity_that_is_never_inlined(self):
        # the toolchain coax/web/wasm.sh uses
        clang = Path(os.environ.get('COAXIAL_CLANG', '/opt/homebrew/opt/llvm/bin/clang'))
        sysroot = Path(os.environ.get('COAXIAL_SYSROOT', '/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'))
        resources = Path(os.environ.get('COAXIAL_RESOURCES', '/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'))
        objdump = clang.parent/'llvm-objdump'
        if not (clang.exists() and sysroot.exists() and objdump.exists()): self.skipTest('no wasm32 toolchain (see coax/web/wasm.sh)')
        with tempfile.TemporaryDirectory() as directory:
            d = Path(directory)
            self.build(d, '--release')
            r = subprocess.run([str(clang), '--target=wasm32-wasip1', f'--sysroot={sysroot}', f'-resource-dir={resources}', '-std=c11', '-O2',
                                '-c', str(d/'ct.c'), '-o', str(d/'ct.o')], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            out = subprocess.run([str(objdump), '-d', str(d/'ct.o')], capture_output=True, text=True, check=True).stdout
            body = re.search(r'<ct_select>:\n(.*?)(?:\n\n|\Z)', out, re.S).group(1)
            self.assertIn('call', body)
            self.assertNotRegex(body, r'\b(if|br_if|select)\b')

    def test_value_barrier_takes_u32_or_u64(self):
        with tempfile.TemporaryDirectory() as directory:
            src = Path(directory)/'bad.coax'
            for bad in ('value_barrier(1.0f32)', 'value_barrier(cast(u8) 1)', 'value_barrier()', 'value_barrier(cast(u64) 1, cast(u64) 2)'):
                with self.subTest(bad=bad):
                    src.write_text('main :: () { x := ' + bad + '; }')
                    r = subprocess.run([str(COMPILER), 'c', str(src)], capture_output=True, text=True)
                    self.assertNotEqual(r.returncode, 0); self.assertIn('E0378', r.stderr)

if __name__ == '__main__':
    unittest.main()
