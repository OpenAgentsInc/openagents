#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Decimal series, not the compiler's binary kernels)
"""Fixed math's full-domain accuracy, IEEE edges and evaluator/native parity.

No host transcendental functions supply reference answers. Decimal exp/ln
and a Machin pi plus Taylor series run at 420 decimal digits, enough to
reduce the largest binary64 input while retaining over 100 guard digits.
This is a sampled two-ulp qualification, not a correct-rounding proof.
"""
import ctypes
from decimal import Decimal as D, localcontext
import math
import os
from pathlib import Path
import random
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax'))


def bits(x):
    return struct.unpack('>Q', struct.pack('>d', x))[0]


def number(b):
    return struct.unpack('>d', struct.pack('>Q', b))[0]


def arctan(x):
    term = x
    total = x
    n = 1
    while True:
        term *= -x*x
        n += 2
        before = total
        total += term/n
        if total == before:
            return total


def references():
    rng = random.Random(12917)
    angles = [-0.0, 0.3, 1.0, 4.0, 411.0, 670.0, 1647099.0,
              1647099.1, 1e20, 1e100, 1e200, 1e300, number(0x7fefffffffffffff)]
    # Include every exponent band, subnormal boundaries, and near quarter turns.
    angles += [number((e << 52) | rng.getrandbits(52)) for e in range(0, 2047, 7)]
    angles += [-x for x in angles]
    with localcontext() as ctx:
        ctx.prec = 420
        pi = 16*arctan(D(1)/5)-4*arctan(D(1)/239)
        for x in angles:
            a = D(x)
            n = (a/(pi/2)).to_integral_value()
            r = a-n*(pi/2)
            s, c = r, D(1)
            st, ct = r, D(1)
            for k in range(1, 230):
                st *= -r*r/((2*k)*(2*k+1))
                ct *= -r*r/((2*k-1)*(2*k))
                s += st
                c += ct
            q = int(n) % 4
            yield 'sin', x, float((s, c, -s, -c)[q]) if x else x
            yield 'cos', x, float((c, -s, -c, s)[q])
        for x in [-746., -745.1332191019412, -745., -740., -709., -708.3964185322641,
                  -1., -0., 1., 709.5, 709.78271289338397] + [rng.uniform(-745, 709.7) for _ in range(250)]:
            yield 'exp', x, float(D(x).exp())
        for x in [number(1), number(0xfffffffffffff), number(0x10000000000000),
                  math.nextafter(1., 0.), 1., math.nextafter(1., 2.), 3., 1e-300,
                  number(0x7fefffffffffffff)] + [abs(x) for x in angles if x]:
            yield 'log', x, float(D(x).ln())


def main():
    refs = list(references())
    with tempfile.TemporaryDirectory(prefix='coax-strict-math-') as temp:
        d = Path(temp)
        source = d/'math.coax'
        source.write_text('\n'.join(f'{name}64 :: (x:f64)->f64 #export_c {{return {name}(x);}}\n'
                                    f'{name}32 :: (x:f32)->f32 #export_c {{return {name}(x);}}'
                                    for name in ('sin', 'cos', 'exp', 'log')))
        c = d/'math.c'
        c.write_bytes(subprocess.check_output([str(COAX), 'c', str(source), '--library']))
        assert 'coax_sm_sin' in c.read_text()
        assert 'Copyright (C) 1993 by Sun Microsystems' in c.read_text()
        assert 'static double coax_sm_sin' in c.read_text()
        previous = None
        for opt in ('-O0', '-O2'):
            lib = d/('math'+opt+'.dylib')
            subprocess.run([os.environ.get('CC', 'cc'), '-std=c11', opt, '-ffp-contract=off',
                            '-shared', '-fPIC', str(c), '-lm', '-o', str(lib)], check=True)
            dll = ctypes.CDLL(str(lib))
            answers = []
            for name in ('sin', 'cos', 'exp', 'log'):
                f = getattr(dll, name+'64'); f.argtypes = [ctypes.c_double]; f.restype = ctypes.c_double
                g = getattr(dll, name+'32'); g.argtypes = [ctypes.c_float]; g.restype = ctypes.c_float
            for name, x, expected in refs:
                actual = getattr(dll, name+'64')(x)
                assert (actual < 0) == (expected < 0), (name, x, actual, expected)
                assert abs(bits(abs(actual))-bits(abs(expected))) <= 2, (name, x, actual, expected)
                answers.append(bits(actual))
            # Native special values; compile-time nonfinite results still refuse E0412.
            assert bits(dll.sin64(-0.0)) == 0x8000000000000000
            assert bits(dll.log64(1.0)) == 0
            assert dll.exp64(math.inf) == math.inf and dll.exp64(-math.inf) == 0
            assert dll.log64(0.0) == -math.inf and dll.log64(-0.0) == -math.inf
            assert dll.log64(math.inf) == math.inf and dll.exp64(710.) == math.inf
            for name in ('sin', 'cos', 'exp', 'log'):
                assert math.isnan(getattr(dll, name+'64')(math.nan))
            assert math.isnan(dll.sin64(math.inf)) and math.isnan(dll.cos64(-math.inf))
            assert math.isnan(dll.log64(-1.0))
            for x in (0.0, -0.0, 4.0, 1e20, 3.4028234663852886e38):
                for name in ('sin', 'cos'):
                    xf = ctypes.c_float(x).value
                    assert getattr(dll, name+'32')(xf) == ctypes.c_float(getattr(dll, name+'64')(xf)).value
            assert dll.exp32(100.0) == math.inf
            if previous is not None:
                assert previous == answers, 'optimization changed fixed math bits'
            previous = answers
        # Exercise the actual generated large-angle limb indexes, casts and
        # subnormal paths in a separate process under ASan/UBSan.
        driver = d/'sanitized.c'
        driver.write_text('#include <stdint.h>\n#include <string.h>\n'
                          + ''.join(f'double {name}64(double);\n' for name in ('sin', 'cos', 'exp', 'log'))
                          + 'int main(void) { uint64_t b, got; double x, y;\n'
                          + ''.join(f'b=UINT64_C({bits(x)});memcpy(&x,&b,8);y={name}64(x);memcpy(&got,&y,8);'
                                    f'if(got!=UINT64_C({answer}))return 1;\n'
                                    for (name, x, _), answer in zip(refs, previous))
                          + 'return 0;}\n')
        exe = d/'sanitized'
        subprocess.run([os.environ.get('CC', 'cc'), '-std=c11', '-O1', '-ffp-contract=off',
                        '-fsanitize=address,undefined', str(c), str(driver), '-lm', '-o', str(exe)], check=True)
        subprocess.run([str(exe)], check=True, env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0'})
        source.write_text('#test "fixed intrinsics" {\n'
                          '#assert sin(4.0)==-0.7568024953079282;\n'
                          '#assert cos(4.0)==-0.65364362086361194;\n'
                          '#assert exp(1.0)==2.7182818284590455;\n'
                          '#assert log(3.0)==1.0986122886681096;\n}\n')
        checks = d/'checks.c'
        checks.write_bytes(subprocess.check_output([str(COAX), 'checks-c', str(source),
                                                   '--library', '--verification-abi', '2'])
                           + b'\nint main(void) {int64_t n; const Coax_VerificationCheck_v2 *p=coax_verification_checks_v2(&n);'
                             b'return n!=1 || p[0].run_values(NULL,0)!=0;}\n')
        subprocess.run([os.environ.get('CC', 'cc'), '-std=c11', '-O2', '-ffp-contract=off',
                        str(checks), '-lm', '-o', str(exe)], check=True)
        subprocess.run([str(exe)], check=True)
        for mode in ([], ['--release']):
            result = subprocess.run([str(COAX), 'run', str(ROOT/'tests/strict-math.coax'), *mode],
                                    capture_output=True, text=True, timeout=60)
            assert result.returncode == 0, result.stderr
            assert result.stdout == 'fixed math: compile-time and native bits agree\n'
        for expression in ('log(0.0)', 'log(-1.0)', 'exp(710.0)', 'exp(100.0f32)'):
            source.write_text(f'X :: #run {expression}; main :: () {{}}')
            result = subprocess.run([str(COAX), 'c', str(source)], capture_output=True, text=True)
            assert result.returncode != 0 and 'E0412' in result.stderr, result.stderr
    print(f'fixed math: {len(refs)} independent references, both optimization modes, ASan/UBSan, IEEE edges and evaluator/native parity pass')


if __name__ == '__main__':
    main()
