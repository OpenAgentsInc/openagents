#!/usr/bin/env python3
# Firm reason: 2 (independent nominal-domain, diagnostic and generated-C ABI oracle).
"""Run the same ID semantics through evaluation and native debug/release output."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
COAX = Path(os.environ.get('COAX', ROOT / 'coax/bin/coax')).resolve()


def invoke(*args):
    return subprocess.run([str(x) for x in args], cwd=ROOT, text=True,
                          capture_output=True, timeout=120)


class NominalIds(unittest.TestCase):
    def test_evaluator_and_native_profiles(self):
        source = ROOT / 'coax/tests/pass/nominal_ids.coax'
        checks = invoke(COAX, 'test', source)
        self.assertEqual(checks.returncode, 0, checks.stderr + checks.stdout)
        for mode in [[], ['--release']]:
            result = invoke(COAX, 'run', source, *mode)
            self.assertEqual((result.returncode, result.stdout, result.stderr),
                             (0, 'nominal IDs: ok\n', ''), result.stderr)

    def test_refusals_keep_stable_codes(self):
        for source in sorted((ROOT / 'coax/tests/fail').glob('nominal_id_*.coax')):
            with self.subTest(source=source.name):
                expected = source.read_text().splitlines()[0].split(': ', 1)[1]
                result = invoke(COAX, 'check', source)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('error[' + expected + ']', result.stderr)
                if source.stem in ['nominal_id_argument', 'nominal_id_compare']:
                    self.assertIn('A', result.stderr)
                    self.assertIn('B', result.stderr)

    def test_all_integer_widths_and_codec_edges(self):
        with tempfile.TemporaryDirectory(prefix='coax-id-widths-') as temp:
            source = Path(temp) / 'widths.coax'
            declarations, checks = [], []
            for kind, low, high in [('s8', -128, 127), ('u8', 0, 255),
                                    ('s16', -32768, 32767), ('u16', 0, 65535),
                                    ('s32', -2147483648, 2147483647), ('u32', 0, 4294967295),
                                    ('s64', -9223372036854775808, 9223372036854775807),
                                    ('u64', 0, 18446744073709551615)]:
                name = 'Id_' + kind
                declarations.append(f'{name} :: struct @id(ordered) {{value:{kind};}}')
                checks.append(f'a_{kind}:={name}.{{value={low}}};b_{kind}:={name}.{{value={high}}};'
                              f'#assert a_{kind}<b_{kind} && a_{kind}.value=={low} && b_{kind}.value=={high};'
                              f'#assert size_of({name})==size_of({kind});')
            source.write_text('\n'.join(declarations) + '\nf :: () {\n' + '\n'.join(checks) +
                              '\n}\n#test "widths" { f(); }\nX :: #run g();\n'
                              'g :: () -> int {f();return 0;}\nmain :: () {f();}\n')
            for command, flags in [('test', []), ('run', []), ('run', ['--release'])]:
                result = invoke(COAX, command, source, *flags)
                self.assertEqual(result.returncode, 0, result.stderr + result.stdout)

    def test_generated_c_preserves_nominal_types_and_layout(self):
        with tempfile.TemporaryDirectory(prefix='coax-id-abi-') as temp:
            base = Path(temp)
            source = base / 'ids.coax'
            source.write_text('SeatId :: struct @id {value:u32;}\n'
                              'EntityId :: struct @id {value:u32;}\n'
                              'pass_seat :: (id:SeatId)->SeatId #export_c {return id;}\n'
                              'pass_entity :: (id:EntityId)->EntityId #export_c {return id;}\n')
            result = invoke(COAX, 'c', source, '--library', '--header', base / 'ids.h')
            self.assertEqual(result.returncode, 0, result.stderr)
            (base / 'ids.c').write_text(result.stdout)
            host = '#include "ids.h"\n#include <stdint.h>\n' \
                   '_Static_assert(sizeof(coax_t_SeatId)==sizeof(uint32_t),"size");\n' \
                   '_Static_assert(_Alignof(coax_t_SeatId)==_Alignof(uint32_t),"alignment");\n' \
                   'int main(void){coax_t_SeatId s={UINT32_MAX};' \
                   'return pass_seat(s).value!=UINT32_MAX;}\n'
            (base / 'host.c').write_text(host)
            for mode, flags in [('debug', ['-O0']), ('release', ['-O2']),
                                ('sanitized', ['-O1', '-fsanitize=address,undefined'])]:
                compile_result = invoke(os.environ.get('CC', 'cc'), '-std=c11', *flags,
                                        base / 'ids.c', base / 'host.c', '-lm', '-o', base / mode)
                self.assertEqual(compile_result.returncode, 0, compile_result.stderr)
                run = invoke(base / mode)
                self.assertEqual(run.returncode, 0, run.stderr)
            (base / 'host.c').write_text(host.replace('pass_seat(s)', 'pass_entity(s)'))
            wrong = invoke(os.environ.get('CC', 'cc'), '-std=c11', '-c', base / 'host.c', '-o', base / 'bad.o')
            self.assertNotEqual(wrong.returncode, 0, 'C must refuse swapping wrapper domains')

    def test_device_output_refuses_before_emission(self):
        with tempfile.TemporaryDirectory(prefix='coax-id-device-') as temp:
            source = Path(temp) / 'device.coax'
            source.write_text('SeatId :: struct @id {value:int;}\n')
            result = invoke(COAX, 'metal', source)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('error[E0962]', result.stderr)

    def test_header_contract_includes_identity_and_ordering(self):
        with tempfile.TemporaryDirectory(prefix='coax-id-headers-') as temp:
            base = Path(temp)
            for name, annotation in [('equal', '@id'), ('ordered', '@id(ordered)'), ('plain', '')]:
                source = base / (name + '.coax')
                source.write_text('Key :: struct ' + annotation + ' {value:int;}\n'
                                  'take_' + name + ' :: (id:Key)->Key #export_c {return id;}\n')
                result = invoke(COAX, 'header', source, '-o', base / (name + '.h'))
                self.assertEqual(result.returncode, 0, result.stderr)
            for second in ['ordered', 'plain']:
                for first, last in [('equal', second), (second, 'equal')]:
                    host = base / 'host.c'
                    host.write_text('#include "' + first + '.h"\n#include "' + last + '.h"\n')
                    result = invoke(os.environ.get('CC', 'cc'), '-std=c11', '-c', host, '-o', base / 'host.o')
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn('incompatible Coax C type', result.stderr)


if __name__ == '__main__':
    unittest.main()
