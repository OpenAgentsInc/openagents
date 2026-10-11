#!/usr/bin/env python3
"""Two CPU requests with the same GGUF, head, and executable as the Metal run."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import threading
import time

ROOT = Path('repo')
BASE = Path('run-artifacts')
OUT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'crates/psionic/fixtures/clef/tools'))
import image_parity as parity
from mac_quiet_bench import OwnedProcessGuard, available_memory, file_sha, stop_owned_group
GIB = 1024 ** 3
EXPECTED_BINARY = 'bb097ec7fb4c18134377b3877b0d66d2c6fc68812436c3bf10ecd9240b08a0d2'


def compare(cpu, other):
    rows = {}
    for key, answer in cpu['answers'].items():
        ref = other['answers'][key]
        differences = ([abs(answer['noul'] - ref['noul'])] if answer['type'] == 'noul' else
                       [abs(p - ref['probabilities'][option]) for option, p in answer['probabilities'].items()])
        rows[key] = {'type': answer['type'], 'cpu_top': parity.top_answer(answer),
                     'other_top': parity.top_answer(ref),
                     'top_matches': parity.top_answer(answer) == parity.top_answer(ref),
                     'cpu_top_two_margin': parity.top_two_margin(answer),
                     'other_top_two_margin': parity.top_two_margin(ref),
                     'maximum_probability_difference': max(differences)}
    return {'tokens_match': cpu['usage']['input_tokens'] == other['usage']['input_tokens'],
            'top_matches': sum(row['top_matches'] for row in rows.values()), 'questions': len(rows),
            'maximum_probability_difference': max(row['maximum_probability_difference'] for row in rows.values()),
            'question_comparisons': rows}


def run():
    if 'memory' not in os.environ.get('OPENAGENTS_LEASES', '').split(','):
        raise ValueError('CPU diagnostic requires a memory lease')
    if (OUT / 'run.json').exists():
        raise ValueError('output exists; preserve it and choose a new run directory')
    original = json.loads((BASE / 'native27b-image-text-parity/run.json').read_text())
    metal = {row['id']: row for row in json.loads((BASE / 'native27b-image-text-parity/1-text.json').read_text())['cases']}
    reference = {row['id']: row for row in json.loads((BASE / 'reference27b/text-mps-bf16.json').read_text())['cases']}
    binary = OUT / 'psionic-openai-server-bb097ec7'
    gguf = Path(original['config']['gguf'])
    head = Path(original['config']['head'])
    request_path = ROOT / 'crates/psionic/fixtures/clef/e2e/requests.jsonl'
    record = {'kind': 'same_gguf_cpu_diagnostic', 'status': 'preflight', 'cases': [],
              'conditions': 'shared machine; correctness diagnostic, not latency evidence',
              'selected_cases': ['text-014', 'text-034'], 'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
              'limits': {'memory_lease_gib': 32, 'max_owned_group_gib': 32,
                         'minimum_start_available_gib': 44, 'minimum_available_gib': 12,
                         'deadline_seconds': 1800, 'startup_timeout_seconds': 180,
                         'first_request_timeout_seconds': 600, 'chunk': 512, 'max_tokens': 2048},
              'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'source_is_dirty': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT)),
              'script_sha256': file_sha(Path(__file__)),
              'harnesses': {name: file_sha(ROOT / 'crates/psionic/fixtures/clef/tools' / name)
                            for name in ['image_parity.py', 'mac_quiet_bench.py', 'mac_memory.py']},
              'inputs': {str(path): file_sha(path) for path in [request_path,
                         BASE / 'native27b-image-text-parity/run.json',
                         BASE / 'native27b-image-text-parity/1-text.json',
                         BASE / 'reference27b/text-mps-bf16.json']}}
    record['artifacts'] = {}
    for name, path in [('binary', binary), ('gguf', gguf), ('head', head)]:
        identity = {'path': str(path), 'bytes': path.stat().st_size, 'sha256': file_sha(path)}
        wanted = EXPECTED_BINARY if name == 'binary' else original['artifacts'][name]['sha256']
        if identity['sha256'] != wanted:
            raise ValueError(f'{name} differs from the Metal comparison artifact')
        record['artifacts'][name] = identity
    environment = {key: value for key, value in os.environ.items() if not key.startswith('PSIONIC_CLEF_')}
    flags = {'RAYON_NUM_THREADS': '2', 'TOKIO_WORKER_THREADS': '1', 'OMP_NUM_THREADS': '2',
             'OPENBLAS_NUM_THREADS': '2', 'MKL_NUM_THREADS': '2', 'VECLIB_MAXIMUM_THREADS': '2',
             'PSIONIC_CLEF_PREFIX_CACHE_BYTES': '0'}
    environment.update(flags)
    record['effective_environment'] = flags
    record['preflight'] = available_memory()
    record['load_average_before'] = os.getloadavg()
    parity.write(OUT / 'run.json', record)
    if record['preflight']['available_memory_bytes'] < 44 * GIB:
        raise ValueError('less than 44 GiB available before CPU load')
    command = [str(binary), '--decision-model', str(gguf), '--clef-head', str(head),
               '--decision-device', 'cpu', '--decision-chunk', '512', '--decision-max-tokens', '2048',
               '--host', '127.0.0.1', '--port', '0']
    record['command'] = command
    cases = parity.load_suite(argparse.Namespace(text_suite=request_path, model='clef'))
    chosen = [(case, request) for case, request, _ in cases if case['id'] in record['selected_cases']]
    with (OUT / 'requests.jsonl').open('w', encoding='utf-8') as sink:
        for case, _ in chosen:
            sink.write(case['request_body'] + '\n')
    process = guard = timer = None
    expired = threading.Event()
    began = time.monotonic()
    old_signals = {}
    def interrupt(signum, _frame):
        raise KeyboardInterrupt(f'signal {signum}')
    try:
        for signum in [signal.SIGINT, signal.SIGTERM]:
            old_signals[signum] = signal.signal(signum, interrupt)
        with (OUT / 'server.log').open('wb') as log:
            process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL,
                                       stdout=log, stderr=log, start_new_session=True)
            guard = OwnedProcessGuard(process, 32 * GIB, OUT / 'memory.jsonl',
                                      minimum_available_memory_bytes=12 * GIB)
            print(json.dumps({'model_active': True, 'pid': process.pid, 'device': 'cpu'}), flush=True)
            def deadline():
                expired.set()
                guard.stop_process(grace=1)
            timer = threading.Timer(1800, deadline)
            timer.start()
            endpoint = None
            while endpoint is None:
                guard.guard()
                if process.poll() is not None:
                    raise RuntimeError('owned CPU server exited before readiness')
                if time.monotonic() - began > 180:
                    raise TimeoutError('CPU startup exceeded 180 seconds')
                match = re.search(r'listening on (http://127\.0\.0\.1:\d+)', (OUT / 'server.log').read_text(errors='replace'))
                if match:
                    with parity.HTTP.open(match[1] + '/health', timeout=2) as health:
                        if health.status == 200:
                            endpoint = match[1]
                time.sleep(.2)
            record.update(status='running', endpoint=endpoint, startup_seconds=time.monotonic()-began)
            parity.write(OUT / 'run.json', record)
            for index, (case, request) in enumerate(chosen):
                guard.guard()
                remaining = 1800 - (time.monotonic() - began)
                if remaining <= 1:
                    raise TimeoutError('no time remains for another CPU request')
                timeout = min(600 if index == 0 else 1790, remaining)
                print(json.dumps({'request_started': case['id'], 'timeout_seconds': timeout}), flush=True)
                started = time.monotonic()
                status, response = parity.post(endpoint, request, timeout, case['request_body'])
                row = {'id': case['id'], 'http_status': status, 'response': response,
                       'seconds': time.monotonic()-started, 'load_average': os.getloadavg(),
                       'source_request_sha256': case['source_request_sha256']}
                if status == 200:
                    parity.validate_response(response, request)
                    row['versus_metal'] = compare(response, metal[case['id']]['response'])
                    row['versus_official_bf16'] = compare(response, reference[case['id']]['response'])
                record['cases'].append(row)
                parity.write(OUT / (case['id'] + '.json'), row)
                parity.write(OUT / 'run.json', record)
                print(json.dumps({key: value for key, value in row.items() if key != 'response'}), flush=True)
                if status != 200:
                    raise RuntimeError(f'CPU request failed: HTTP {status}')
                guard.guard()
            record['status'] = 'complete'
    except BaseException as error:
        record.update(status='failed', error_type=type(error).__name__, error=str(error))
    finally:
        if timer is not None:
            timer.cancel()
            timer.join()
        try:
            if guard is not None:
                try:
                    guard.close()
                finally:
                    record['memory'] = guard.receipt()
                if guard.failure:
                    record['status'] = 'failed'
            elif process is not None:
                stop_owned_group(process)
        except BaseException as error:
            record.update(status='failed', cleanup_error=str(error))
        for signum, prior in old_signals.items():
            signal.signal(signum, prior)
        if expired.is_set():
            record.update(status='failed', error='CPU diagnostic exceeded 1800 seconds')
        record['elapsed_seconds'] = time.monotonic()-began
        record['load_average_after'] = os.getloadavg()
        parity.write(OUT / 'run.json', record)
        print(json.dumps({'model_active': False, 'status': record['status'], 'completed_cases':len(record['cases']),
                          'owned_group_stopped':record.get('memory',{}).get('owned_group_stopped')}), flush=True)
    return 0 if record['status'] == 'complete' else 1

if __name__ == '__main__':
    raise SystemExit(run())
