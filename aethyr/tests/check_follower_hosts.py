#!/usr/bin/env python3
"""Qualify the actual Garden/Villa clock adapters with identical supplied terrain.

Uses production C units separately, as their real faces do. Python supplies
recorded frame data and independent integer-clock/draw oracles; C owns no
motion or timing policy. Normal admitted C can be reused from a passing
page build. Native optimization modes do not imply Coax --release.
"""
import argparse
import json
import math
import os
from pathlib import Path
import shutil
import struct
import time

import check_native_gameplay as gate

ROOT = Path(__file__).resolve().parents[2]
DRIVER = ROOT / 'aethyr/tests/follower-host.c'
WASM_DRIVER = ROOT / 'aethyr/tests/follower-host.js'
PHYSICAL = 40


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def value(word):
    return struct.unpack('<f', struct.pack('<I', word))[0]


def frame_ns(frame):
    return math.floor(f32(frame) * 1_000_000_000 + 0.5)


def cadence(fps):
    """Nominal cadence, with a final fractional tail closing one exact second."""
    nominal = f32(1 / fps)
    prefix = [nominal] * (fps - 1)
    need = 1_000_000_000 - sum(map(frame_ns, prefix))
    bits = struct.unpack('<I', struct.pack('<f', need / 1_000_000_000))[0]
    while frame_ns(value(bits)) > need: bits -= 1
    result = prefix + [value(bits)]
    missing = 1_000_000_000 - sum(map(frame_ns, result))
    if missing: result.append(f32(missing / 1_000_000_000))
    assert sum(map(frame_ns, result)) == 1_000_000_000
    return result


def admitted_page_ok(admission, compiler_hash):
    return (admission.get('exit_code') == 0 and admission.get('source_changes') == []
            and all(admission.get(key) is True for key in ['compiler_unchanged', 'wrapper_unchanged',
                'clang_wrapper_unchanged', 'actual_clang_unchanged'])
            and admission.get('compiler_sha256') == compiler_hash
            and admission.get('normal_coax_c_flags') == ['--library', '--header', '<unit>.h'])


def reusable_artifact(metadata, identity, artifact):
    return (metadata.get('exit_code') == 0 and metadata.get('identity') == identity
            and artifact.is_file() and metadata.get('sha256') == gate.digest(artifact.read_bytes()))


def qualify(args):
    work = Path(args.work).resolve(); work.mkdir(parents=True, exist_ok=True)
    record = Path(args.record).resolve()
    began = time.monotonic()
    compiler = Path(args.coax).resolve()
    entries = [ROOT / f'apps/openagents-web/{args.realm}.coax', ROOT / 'aethyr/player.coax']
    sources = gate.snapshot([*entries, DRIVER, WASM_DRIVER, Path(__file__), Path(gate.__file__),
        Path(__file__).with_name('test_follower_hosts_tooling.py')])
    expected = {name: (ROOT / name, digest) for name, digest in sources.items()}
    expected['Coax compiler'] = (compiler, gate.digest(compiler.read_bytes()))
    receipt = {'status': 'running', 'realm': args.realm, 'native_mode': args.mode,
               'coax_codegen': 'normal C, no --release', 'compiler_sha256': expected['Coax compiler'][1],
               'sources': sources, 'commands': [], 'profiles': [], 'negative_controls': []}

    def run(command, label, *, output=None, limit=120, environment=None):
        command = list(map(str, command)); stamp = time.monotonic()
        log = work / (label + '.stderr.log'); stdout = output or work / (label + '.stdout')
        remaining = 830 - (stamp - began)
        if remaining <= 0: raise RuntimeError('qualification exceeded 830 seconds')
        with Path(stdout).open('wb') as out, log.open('wb') as err:
            code = gate.run_process_group(command, cwd=ROOT, stdout=out, stderr=err,
                environment=environment or os.environ.copy(), timeout=min(limit, remaining))
        receipt['commands'].append({'label': label, 'command': command, 'exit_code': code,
            'seconds': time.monotonic() - stamp, 'stderr': str(log), 'stdout': str(stdout)})
        if code: raise RuntimeError(label + ': ' + log.read_text(errors='replace')[-3000:])
        return Path(stdout).read_bytes()

    try:
        if args.admission:
            admission_path = Path(args.admission).resolve(); admission = json.loads(admission_path.read_text())
            if not admitted_page_ok(admission, expected['Coax compiler'][1]):
                raise RuntimeError('page admission did not pass unchanged')
            for name, digest in admission['sources'].items():
                if gate.digest((ROOT / name).read_bytes()) != digest:
                    raise RuntimeError('admitted source changed: ' + name)
            code_dir = Path(admission['admitted_directory'])
            for name in (args.realm + '.c', args.realm + '.h', 'Player.c', 'Player.h'):
                path = code_dir / name
                if gate.digest(path.read_bytes()) != admission['admitted_files'][name]:
                    raise RuntimeError('admitted C/header changed: ' + name)
                expected['admitted ' + name] = (path, admission['admitted_files'][name])
            expected['page admission'] = (admission_path, gate.digest(admission_path.read_bytes()))
            receipt['reused_page_admission'] = str(admission_path)
        else:
            code_dir = work / 'admitted'; code_dir.mkdir(exist_ok=True)
            for entry, name in zip(entries, [args.realm, 'Player']):
                run([compiler, 'c', entry.relative_to(ROOT), '--library', '--header', code_dir / (name + '.h')],
                    name + '-emit', output=code_dir / (name + '.c'), limit=780)
            for path in code_dir.iterdir(): expected['generated ' + path.name] = (path, gate.digest(path.read_bytes()))
        flags = {'debug': ['-O0', '-g'], 'optimized': ['-O2'],
                 'sanitized': ['-O1', '-g', '-fsanitize=address,undefined', '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']}[args.mode]
        define = ['-DFOLLOWER_GARDEN'] if args.realm == 'garden' else []
        native = work / ('native-' + args.mode)
        units = [code_dir / (args.realm + '.c'), code_dir / 'Player.c', DRIVER]
        cc_path = Path(shutil.which(args.cc) or args.cc).resolve()
        expected['native C compiler'] = (cc_path, gate.digest(cc_path.read_bytes()))
        native_command = [args.cc, '-std=c11', *flags, '-fno-fast-math', '-ffp-contract=off', *define, '-I' + str(code_dir), *units, '-lm', '-o', native]
        native_identity = {'command': list(map(str, native_command)), 'cc_sha256': expected['native C compiler'][1],
            'units': {str(path): gate.digest(path.read_bytes()) for path in units},
            'headers': {name: gate.digest((code_dir / name).read_bytes()) for name in [args.realm + '.h', 'Player.h']}}
        native_meta = work / ('native-emission-' + args.mode + '.json')
        native_prior = json.loads(native_meta.read_text()) if native_meta.exists() else {}
        if reusable_artifact(native_prior, native_identity, native):
            receipt['reused_native'] = True
        else:
            run(native_command, 'native-compile')
            native_meta.write_text(json.dumps({'identity': native_identity, 'sha256': gate.digest(native.read_bytes()), 'exit_code': 0}, indent=2) + '\n')
        expected['native binary'] = (native, gate.digest(native.read_bytes()))
        receipt['native_emission'] = json.loads(native_meta.read_text())
        # Use the repository's exact target discovery, with no new toolchain.
        env_bytes = run(['bash', '-c', 'source coax/web/toolchain.sh; printf "%s\\0" "$COAXIAL_CLANG" "$COAXIAL_SYSROOT" "$COAXIAL_RESOURCES" "$PATH"'], 'wasm-toolchain')
        paths = env_bytes.split(b'\0')
        if len(paths) != 5 or paths[-1] != b'': raise RuntimeError('invalid Wasm toolchain path record')
        tool_env = os.environ.copy()
        tool_env.update(zip(['COAXIAL_CLANG', 'COAXIAL_SYSROOT', 'COAXIAL_RESOURCES', 'PATH'], (part.decode() for part in paths[:4])))
        clang = Path(tool_env['COAXIAL_CLANG']).resolve()
        expected['Wasm clang'] = (clang, gate.digest(clang.read_bytes()))
        wasm = work / 'follower.wasm'
        wasm_command = [clang, '--target=wasm32-wasip1', '--sysroot=' + tool_env['COAXIAL_SYSROOT'], '-resource-dir=' + tool_env['COAXIAL_RESOURCES'],
            '-std=c11', '-O2', '-fno-fast-math', '-ffp-contract=off', *define, '-I' + str(code_dir), *units,
            '-mexec-model=reactor', '-Wl,--import-memory,--max-memory=67108864,-z,stack-size=1048576,--export=__heap_base,--gc-sections,--strip-debug',
            '-o', wasm]
        wasm_identity = {'c': {path.name: gate.digest(path.read_bytes()) for path in units},
                         'headers': {name: gate.digest((code_dir / name).read_bytes()) for name in [args.realm + '.h', 'Player.h']},
                         'clang': expected['Wasm clang'][1], 'command': list(map(str, wasm_command))}
        wasm_meta = work / 'wasm-emission.json'
        prior = json.loads(wasm_meta.read_text()) if wasm_meta.exists() else {}
        if not reusable_artifact(prior, wasm_identity, wasm):
            run(wasm_command, 'wasm-compile', environment=tool_env)
            wasm_meta.write_text(json.dumps({'identity': wasm_identity, 'sha256': gate.digest(wasm.read_bytes()), 'exit_code': 0}, indent=2) + '\n')
        else: receipt['reused_wasm'] = True
        expected['Wasm artifact'] = (wasm, gate.digest(wasm.read_bytes()))
        receipt['wasm_emission'] = json.loads(wasm_meta.read_text())

        def simulate(operations, name, *, wasm_too=True):
            path = work / (name + '.operations')
            path.write_bytes(b''.join(struct.pack('<ifi', op, frame, fault) for op, frame, fault in operations))
            environment = os.environ.copy(); environment['ASAN_OPTIONS'] = 'halt_on_error=1:detect_leaks=0'
            environment['UBSAN_OPTIONS'] = 'halt_on_error=1:print_stacktrace=1'
            data = run([native, path], name + '-native', environment=environment)
            rows = [list(map(int, line.split())) for line in data.decode().splitlines()]
            if len(rows) != len(operations) or any(len(row) != 61 for row in rows): raise RuntimeError('incomplete snapshot stream')
            if wasm_too:
                browser = json.loads(run([args.node, WASM_DRIVER, wasm, path], name + '-wasm'))
                if browser['rows'] != rows:
                    first = next(i for i, pair in enumerate(zip(rows, browser['rows'])) if pair[0] != pair[1])
                    changed = [i for i, pair in enumerate(zip(rows[first], browser['rows'][first])) if pair[0] != pair[1]]
                    raise RuntimeError(f'{name}: native/Wasm row {first}, words {changed}')
                receipt['wasm_imports'] = browser['imports']
            for (operation, _, fault), row in zip(operations, rows):
                if operation in (0, 1) and fault == 0:
                    alpha = row[43] / 15_625_000
                    for current, previous, drawn in [(10, 30, 57), (12, 32, 59)]:
                        wanted = value(row[previous]) + (value(row[current]) - value(row[previous])) * alpha
                        if abs(value(row[drawn]) - wanted) > 0.00002: raise RuntimeError(name + ': drawn feet do not use previous pose and alpha')
                    if row[60] >= 48000: raise RuntimeError('clipped HUD fixture')
            return rows

        traces = {}
        for fps in [30, 60, 120, 144]:
            frames = cadence(fps)
            rows = simulate([(0, frame, 0) for frame in frames], 'fps-' + str(fps))
            if rows[-1][40] != 64 or rows[-1][49] != 64 or any(row[52] for row in rows): raise RuntimeError('realm did not consume all64 assigned steps')
            if value(rows[-1][11]) != 2.5 or value(rows[-1][19]) != (3.75 if args.realm == 'garden' else f32(3.65)):
                raise RuntimeError('supplied terrain origin was omitted or duplicated')
            traces[fps] = rows
            receipt['profiles'].append({'fps': fps, 'frames': len(frames), 'frame_ns_sum': sum(map(frame_ns, frames)),
                'paid_ticks': rows[-1][40], 'physical_word_count': PHYSICAL,
                'trace_sha256': gate.digest(json.dumps(rows, separators=(',', ':')).encode())})
        by_tick = {row[40]: row[:PHYSICAL] for row in traces[144]}
        if set(range(65)) - by_tick.keys(): raise RuntimeError('reference cadence did not observe every tick')
        for fps, rows in traces.items():
            for row in rows:
                if row[:PHYSICAL] != by_tick[row[40]]: raise RuntimeError(f'physics differs at {fps} FPS, tick {row[40]}')
        for fault, label in [(1, 'repeat final target'), (2, 'legacy variable-time stepping'), (3, 'drop floor origin in producer')]:
            rows = simulate([(0, frame, fault) for frame in cadence(30)], 'fault-' + str(fault), wasm_too=False)
            if rows[-1][9:29] == traces[30][-1][9:29]: raise RuntimeError('planted control survived: ' + label)
            receipt['negative_controls'].append(label)
        special_ops = [(0, .010, 0), (0, .006, 0), (1, 0, 0), (0, 0, 0), (0, .03125, 0),
                       (0, .4, 0), (0, float('nan'), 0), (0, -1, 0), (2, 0, 0), (0, .03125, 0), (3, 0, 0), (0, .015625, 0)]
        special = simulate(special_ops, 'carry-multi-stall-refusal-reset')
        if [special[i][51] for i in [0, 1, 2, 3, 4, 5, 6, 7, 9, 11]] != [0, 1, 0, 0, 2, 8, 0, 0, 2, 1]: raise RuntimeError('actual realm schedule mismatch')
        if special[1][:PHYSICAL] != special[2][:PHYSICAL] or special[1][57:60] != special[2][57:60]: raise RuntimeError('redrawing moved the follower')
        if special[5][48] != 125_000_000 or special[7][45] != 2: raise RuntimeError('stall/refused-frame diagnostics differ')
        if special[9][56] == 0 or special[9][49] != 13 or special[9][:3] != special[8][:3]: raise RuntimeError('refused Player steps did not retain contemporaneous repeated targets')
        if special[11][50] != 1 or special[11][49] != 1: raise RuntimeError('fresh Player clock did not reset follower cursor')
        action_ops = [(0, .015625, 0), (4, 0, 0), (0, .015625, 0), (5, 0, 0), (0, .015625, 0)]
        action = simulate(action_ops, 'queued-action-refusal')
        if action[2][9:29] != action[1][9:29] or action[2][54] != 1 or action[2][52] != 1: raise RuntimeError('refusal changed fetch physics or consumed queued action')
        if action[4][54] != 0 or action[4][25] != 0xffffffff or action[4][26] != 1: raise RuntimeError('retained action did not execute once')
        if args.realm == 'villa' and (action[2][55] != action[1][55] or action[4][55] != action[1][55] + 1): raise RuntimeError('refused action published pet events')
        receipt['special_scenarios'] = ['carry10ms+6ms', 'redraw', 'zero frame', 'two targets', 'eight-step stall/drop',
            'NaN/negative frame', 'refused physical Player step', 'fresh clock after place', 'queued throw rollback/retry']
        receipt['status'] = 'passed'
    except BaseException as error:
        receipt['status'] = 'failed'; receipt['error'] = str(error)
        raise
    finally:
        receipt['seconds'] = time.monotonic() - began
        gate.finalize_receipt(record, receipt, expected)
    if json.loads(record.read_text())['status'] != 'passed': raise RuntimeError('qualification inputs changed')
    print(args.realm + ' ' + args.mode + ': four cadence traces, native/Wasm, drawn alpha, nine scenarios, three detected producer controls pass')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--realm', choices=['garden', 'villa'], required=True)
    parser.add_argument('--mode', choices=['debug', 'optimized', 'sanitized'], default='debug')
    parser.add_argument('--admission', help='passing page-build JSON with preserved normal Coax C and headers')
    parser.add_argument('--work', required=True)
    parser.add_argument('--record', required=True)
    parser.add_argument('--coax', default=str(ROOT / 'coax/bin/coax'))
    parser.add_argument('--cc', default=os.environ.get('CC', 'cc'))
    parser.add_argument('--node', default=shutil.which('node') or 'node')
    qualify(parser.parse_args())


if __name__ == '__main__': main()
