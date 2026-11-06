#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python's unbounded integers, not the compiler's helpers)
"""Checked, saturating and wrapping integer operations (#12927, spec section 9).

Near-boundary operands (min, min+1, -1, 0, 1, max-1, max and a few in between)
for add, sub and mul on all eight integer widths. One oracle: the exact result
in Python, then none (checked), the nearest bound (saturating) or the low bits
(wrapping). Each profile must agree with it: the compile-time evaluator in a
debug and a release build, the emitted C of both builds compiled at -O2 and
called, the same C under ASan/UBSan, and actual wasm32. A checked refusal
leaves a counter as it was.
"""
from pathlib import Path
import ctypes
import json
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', str(ROOT / 'bin/coax')))
CC = os.environ.get('CC') or shutil.which('cc') or 'clang'
if os.uname().sysname == 'Darwin':
    WASM = ('/opt/homebrew/opt/llvm/bin/clang', '/opt/homebrew/opt/wasi-libc/share/wasi-sysroot',
            '/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes', '/opt/homebrew/opt/lld/bin')
else:
    _tc = Path(os.environ.get('COAXIAL_TOOLCHAIN', Path.home() / '.cache/coax-wasm-toolchain'))
    WASM = (str(_tc / 'bin/clang'), str(_tc / 'sysroot'), str(_tc / 'resource'), str(_tc / 'bin'))

WIDTHS = [(s, b) for s in (True, False) for b in (8, 16, 32, 64)]
OPS = ('add', 'sub', 'mul')
CTYPE = {(True, 8): ctypes.c_int8, (True, 16): ctypes.c_int16, (True, 32): ctypes.c_int32, (True, 64): ctypes.c_int64,
         (False, 8): ctypes.c_uint8, (False, 16): ctypes.c_uint16, (False, 32): ctypes.c_uint32, (False, 64): ctypes.c_uint64}


def name(signed, bits):
    return ('s' if signed else 'u') + str(bits)


def bounds(signed, bits):
    return (-(1 << (bits - 1)), (1 << (bits - 1)) - 1) if signed else (0, (1 << bits) - 1)


def operands(signed, bits):
    lo, hi = bounds(signed, bits)
    values = {lo, lo + 1, 0, 1, 2, hi - 1, hi, hi // 2, hi // 2 + 1, 1 << (bits // 2)}
    if signed:
        values |= {-1, -2, lo // 2, -(1 << (bits // 2))}
    return sorted(v for v in values if lo <= v <= hi)


def exact(op, a, b):
    return a + b if op == 'add' else a - b if op == 'sub' else a * b


def oracle(form, op, a, b, signed, bits):
    lo, hi = bounds(signed, bits)
    v = exact(op, a, b)
    if form == 'checked':
        return v if lo <= v <= hi else None
    if form == 'sat':
        return min(max(v, lo), hi)
    m = v & ((1 << bits) - 1)
    return m - (1 << bits) if signed and m > hi else m


def literal(v, signed, bits):
    # cast(T) of the literal; the most negative s64 is not a literal, so it is spelled as an expression
    t = name(signed, bits)
    if signed and bits == 64 and v == -(1 << 63):
        return f'(cast(s64) -9223372036854775807 - 1)'
    return f'cast({t}) {v}' if v >= 0 else f'cast({t}) -{-v}'


def source():
    lines = []
    for signed, bits in WIDTHS:
        t = name(signed, bits)
        for op in OPS:
            lines.append(f'c_{op}_{t} :: (a:{t},b:{t},none:{t})->{t} #export_c #effects(all) {{return {op}_checked(a,b) or_else none;}}')
            lines.append(f'h_{op}_{t} :: (a:{t},b:{t})->bool #export_c #effects(all) {{r:={op}_checked(a,b);switch r {{case none {{return false;}} case some(v) {{return true;}}}}}}')
            lines.append(f's_{op}_{t} :: (a:{t},b:{t})->{t} #export_c #effects(all) {{return {op}_sat(a,b);}}')
            lines.append(f'w_{op}_{t} :: (a:{t},b:{t})->{t} #export_c #effects(all) {{return {op}_wrap(a,b);}}')
    lines.append('''
// A counter's refusal leaves it as it was (#12927): the change happens only when the result fits.
Bank :: struct { n: u8 #counter; changes: int; }
bump :: (b: [] Bank, k: u8) -> bool {
  r := add_checked(b[0].n, k);
  switch r { case none { return false; } case some(v) { b[0].n = v; b[0].changes += 1; return true; } }
}
bank_after :: (start: u8, k: u8) -> u8 #export_c #effects(all) { b: [1] Bank; b[0].n = start; ok := bump(b[..], k); return b[0].n; }
bank_ok :: (start: u8, k: u8) -> bool #export_c #effects(all) { b: [1] Bank; b[0].n = start; return bump(b[..], k); }
bank_changes :: (start: u8, k: u8) -> int #export_c #effects(all) { b: [1] Bank; b[0].n = start; ok := bump(b[..], k); return b[0].changes; }
// Operands run once, left to right.
next :: (s: [] s32) -> s32 { v := s[0]; s[0] = v + 1; return v; }
order :: () -> s32 #export_c #effects(all) { s: [1] s32; s[0] = 5; return sub_sat(next(s[..]), next(s[..])) * 10 + s[0]; }
''')
    return '\n'.join(lines)


def cases():
    for signed, bits in WIDTHS:
        vals = operands(signed, bits)
        for op in OPS:
            for a in vals:
                for b in vals:
                    yield signed, bits, op, a, b


class IntegerOps(unittest.TestCase):
    def compile_library(self, d, mode, opt='-O2', extra=()):
        src = d / 'ops.coax'
        src.write_text(source())
        p = subprocess.run([str(COMPILER), 'c', str(src), '--library', '--header', str(d / 'ops.h'), *mode], capture_output=True, text=True)
        self.assertEqual(p.returncode, 0, p.stderr)
        c = d / ('release.c' if mode else 'debug.c')
        c.write_text(p.stdout)
        return c

    def test_emitted_c_at_o2_matches_the_oracle_in_debug_and_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            seen = 0
            for mode in ([], ['--release']):
                c = self.compile_library(d, mode)
                lib_path = d / (c.stem + '.dylib')
                subprocess.run([CC, '-std=c11', '-O2', '-shared', '-fPIC', str(c), '-lm', '-o', str(lib_path)], check=True, capture_output=True)
                lib = ctypes.CDLL(str(lib_path))
                for signed, bits, op, a, b in cases():
                    t = name(signed, bits)
                    ct = CTYPE[(signed, bits)]
                    fns = {f: getattr(lib, f'{f}_{op}_{t}') for f in 'chsw'}
                    for f in 'csw':
                        fns[f].restype = ct
                    fns['c'].argtypes = [ct, ct, ct]
                    fns['h'].argtypes = [ct, ct]
                    fns['h'].restype = ctypes.c_bool
                    fns['s'].argtypes = fns['w'].argtypes = [ct, ct]
                    want = oracle('checked', op, a, b, signed, bits)
                    fallback = 0 if want != 0 else 1
                    got_has = fns['h'](a, b)
                    self.assertEqual(got_has, want is not None, (mode, op, t, a, b))
                    self.assertEqual(fns['c'](a, b, fallback), fallback if want is None else want, (mode, op, t, a, b))
                    self.assertEqual(fns['s'](a, b), oracle('sat', op, a, b, signed, bits), (mode, 'sat', op, t, a, b))
                    self.assertEqual(fns['w'](a, b), oracle('wrap', op, a, b, signed, bits), (mode, 'wrap', op, t, a, b))
                    seen += 1
                lib.bank_after.argtypes = lib.bank_ok.argtypes = lib.bank_changes.argtypes = [ctypes.c_uint8, ctypes.c_uint8]
                lib.bank_after.restype = ctypes.c_uint8
                lib.bank_ok.restype = ctypes.c_bool
                lib.bank_changes.restype = ctypes.c_int64
                for start, k in ((254, 1), (255, 0), (255, 1), (200, 100), (0, 255)):
                    fits = start + k <= 255
                    self.assertEqual(lib.bank_ok(start, k), fits, (mode, start, k))
                    self.assertEqual(lib.bank_after(start, k), start + k if fits else start, (mode, start, k))
                    self.assertEqual(lib.bank_changes(start, k), 1 if fits else 0, (mode, start, k))
                lib.order.restype = ctypes.c_int32
                self.assertEqual(lib.order(), -10 + 7)
            self.assertGreater(seen, 2000)

    def test_compile_time_evaluator_matches_the_oracle_in_debug_and_release(self):
        lines = []
        for signed, bits, op, a, b in cases():
            x, y = literal(a, signed, bits), literal(b, signed, bits)
            want = oracle('checked', op, a, b, signed, bits)
            t = name(signed, bits)
            if want is None:
                lines.append(f'#assert #run ({op}_checked({x}, {y}) == null);')
            else:
                lines.append(f'#assert #run (({op}_checked({x}, {y}) or_else {literal(0 if want else 1, signed, bits)}) == {literal(want, signed, bits)});')
            lines.append(f'#assert #run {op}_sat({x}, {y}) == {literal(oracle("sat", op, a, b, signed, bits), signed, bits)};')
            lines.append(f'#assert #run {op}_wrap({x}, {y}) == {literal(oracle("wrap", op, a, b, signed, bits), signed, bits)};')
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / 'eval.coax'
            src.write_text('#run_steps 100000000;\n' + '\n'.join(lines) + '\nmain :: () {}\n')
            for mode in ([], ['--release']):
                p = subprocess.run([str(COMPILER), 'c', str(src), *mode], capture_output=True, text=True)
                self.assertEqual(p.returncode, 0, (mode, p.stderr[-3000:]))
            # an assertion the oracle contradicts does fail: the checks above are live
            src.write_text('#assert #run add_sat(cast(u8) 255, cast(u8) 1) == 0;\nmain :: () {}\n')
            p = subprocess.run([str(COMPILER), 'c', str(src)], capture_output=True, text=True)
            self.assertNotEqual(p.returncode, 0)

    def test_emitted_c_under_sanitizers(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            for mode in ([], ['--release']):
                c = self.compile_library(d, mode)
                host = ['#include <stdint.h>', '#include <stdbool.h>', '#include "ops.h"', 'int main(void){']
                for signed, bits, op, a, b in cases():
                    t = name(signed, bits)
                    ctype = ('int' if signed else 'uint') + f'{bits}_t'
                    def lit(v):
                        if signed and bits == 64 and v == -(1 << 63):
                            return 'INT64_MIN'
                        return f'(({ctype}){v}{"LL" if signed else "ULL"})' if bits == 64 else f'(({ctype}){v})'
                    want = oracle('checked', op, a, b, signed, bits)
                    host.append(f'if(h_{op}_{t}({lit(a)},{lit(b)})!={"true" if want is not None else "false"})return 1;')
                    if want is not None:
                        host.append(f'if(c_{op}_{t}({lit(a)},{lit(b)},{lit(0 if want else 1)})!={lit(want)})return 2;')
                    host.append(f'if(s_{op}_{t}({lit(a)},{lit(b)})!={lit(oracle("sat", op, a, b, signed, bits))})return 3;')
                    host.append(f'if(w_{op}_{t}({lit(a)},{lit(b)})!={lit(oracle("wrap", op, a, b, signed, bits))})return 4;')
                host.append('return bank_after(255,1)!=255 || bank_after(254,1)!=255;}')
                (d / 'host.c').write_text('\n'.join(host))
                exe = d / 'host'
                subprocess.run([CC, '-std=c11', '-O1', '-g', '-fsanitize=address,undefined', '-fno-sanitize-recover=all', str(c), str(d / 'host.c'), '-lm', '-o', str(exe)], check=True, capture_output=True)
                p = subprocess.run([str(exe)], capture_output=True, text=True)
                self.assertEqual(p.returncode, 0, (mode, p.stderr[-2000:]))

    def test_actual_wasm_matches_the_oracle(self):
        clang = Path(os.environ.get('COAXIAL_CLANG', WASM[0]))
        sysroot = Path(os.environ.get('COAXIAL_SYSROOT', WASM[1]))
        resources = Path(os.environ.get('COAXIAL_RESOURCES', WASM[2]))
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(), 'required wasm32 toolchain unavailable')
        self.assertIsNotNone(shutil.which('bun'), 'required Wasm executor unavailable')
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            for mode in ([], ['--release']):
                c = self.compile_library(d, mode)
                exports = [f'-Wl,--export={f}_{op}_{name(s, b)}' for s, b in WIDTHS for op in OPS for f in 'chsw'] + ['-Wl,--export=bank_after']
                wasm = d / (c.stem + '.wasm')
                subprocess.run([str(clang), '--target=wasm32-wasip1', f'--sysroot={sysroot}', f'-resource-dir={resources}', '-std=c11', '-O2',
                                '-nostartfiles', '-Wl,--no-entry', *exports, str(c), '-lm', '-o', str(wasm)], check=True, capture_output=True,
                               env=dict(os.environ, PATH=WASM[3] + os.pathsep + os.environ.get('PATH', '')))
                rows = []
                for signed, bits, op, a, b in cases():
                    t = name(signed, bits)
                    want = oracle('checked', op, a, b, signed, bits)
                    rows.append([f'h_{op}_{t}', signed, bits, [a, b], 1 if want is not None else 0])
                    if want is not None:
                        rows.append([f'c_{op}_{t}', signed, bits, [a, b, 0 if want else 1], want])
                    rows.append([f's_{op}_{t}', signed, bits, [a, b], oracle('sat', op, a, b, signed, bits)])
                    rows.append([f'w_{op}_{t}', signed, bits, [a, b], oracle('wrap', op, a, b, signed, bits)])
                rows.append(['bank_after', False, 8, [255, 1], 255])
                rows = [[f, s, b, [str(x) for x in args], str(w)] for f, s, b, args, w in rows]
                (d / 'cases.json').write_text(json.dumps(rows))
                (d / 'run.mjs').write_text("""import {readFileSync} from 'node:fs';
const {instance}=await WebAssembly.instantiate(readFileSync(process.argv[2]),{});
const rows=JSON.parse(readFileSync(process.argv[3],'utf8'));
for(const [fn,signed,bits,args,want] of rows){
 const v=instance.exports[fn](...args.map(a=>bits===64?BigInt(a):Number(a)));
 let got;
 if(fn.startsWith('h_'))got=String(v&1);
 else if(bits===64)got=(signed?BigInt.asIntN(64,v):BigInt.asUintN(64,v)).toString();
 else got=String(signed?((v<<(32-bits))>>(32-bits)):((v>>>0)&(bits===32?0xffffffff:(1<<bits)-1))>>>0);
 if(got!==want)throw new Error(`${fn} ${args}: ${got} != ${want}`);
}
console.log(rows.length);
""")
                p = subprocess.run(['bun', str(d / 'run.mjs'), str(wasm), str(d / 'cases.json')], capture_output=True, text=True)
                self.assertEqual(p.returncode, 0, (mode, p.stderr[-2000:]))
                self.assertEqual(p.stdout.strip(), str(len(rows)))

    def test_refuses_mixed_types_floats_and_plain_arithmetic_on_a_counter(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / 'bad.coax'
            for form in ('checked', 'sat', 'wrap'):
                for op in OPS:
                    for args in ('cast(u8)1,cast(s8)2', '1.0,2.0', 'cast(u32)1', 'cast(u32)1,cast(u32)2,cast(u32)3', 'true,false'):
                        src.write_text(f'main :: () {{x:={op}_{form}({args});}}')
                        p = subprocess.run([str(COMPILER), 'c', str(src)], capture_output=True, text=True)
                        self.assertNotEqual(p.returncode, 0, (form, op, args))
                        self.assertTrue('E0378' in p.stderr or 'E0350' in p.stderr, (form, op, args, p.stderr))
            for body, code in (('c.n = c.n + 1;', 'E0840'), ('c.n += 1;', 'E0840'), ('c.n = c.n * 2;', 'E0840'),
                               ('c.k[0] -= 1;', 'E0840'), ('x := cast(int) c.n - 1;', 'E0840'),
                               ('c.m = c.m + 1;', None), ('x := c.n / 2;', None), ('ok := c.n < 3;', None),
                               ('c.n = add_checked(c.n, 1) or_else c.n;', None), ('c.n = add_sat(c.n, 1);', None)):
                src.write_text(f'C :: struct {{ n: u32 #counter; k: [2] s16 #counter; m: int; }}\nmain :: () {{ c: C; {body} }}')
                p = subprocess.run([str(COMPILER), 'c', str(src)], capture_output=True, text=True)
                if code:
                    self.assertNotEqual(p.returncode, 0, body)
                    self.assertIn(f'error[{code}]', p.stderr, (body, p.stderr))
                else:
                    self.assertEqual(p.returncode, 0, (body, p.stderr))


if __name__ == '__main__':
    unittest.main()
