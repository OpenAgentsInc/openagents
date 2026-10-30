#!/usr/bin/env python3
"""Measure signer secret flow under two native build profiles; never approve crypto.

Tooling only. The signer and all cryptographic decisions remain in Coax.
Requires Linux, a Coax compiler, GCC and Valgrind with its development headers.
"""
import argparse
import csv
import hashlib
import json
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
VECTORS = ROOT/'coax/tests/vectors/upstream/bip340/test-vectors.csv'
PROFILES = [('diagnostic', ['-O1', '-fno-inline']), ('release-optimization', ['-O2'])]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def verify_sources(hashes):
    if not isinstance(hashes, dict) or not hashes:
        raise ValueError('missing compiled source identities')
    for path, expected in hashes.items():
        if sha(path) != expected:
            raise ValueError('compiled source changed: '+path)


def fixtures():
    with VECTORS.open(newline='') as stream:
        rows = [r for r in csv.DictReader(stream) if r['secret key']]
    cases = [dict(name='vector-'+r['index'], mode='valid', key=r['secret key'],
                  aux=r['aux_rand'], message=r['message'], signature=r['signature']) for r in rows]
    if [r['name'] for r in cases] != ['vector-0', 'vector-1', 'vector-2', 'vector-3',
                                    'vector-15', 'vector-16', 'vector-17', 'vector-18']:
        raise ValueError('unexpected signing fixture inventory')
    for mode in ['key-zero', 'key-order', 'key-max', 'short-key', 'overlap',
                 'message-change', 'candidate-change', 'positive-control']:
        cases.append(dict(cases[0], name=mode, mode=mode))
    return cases


def observation(code, xml, stdout, control):
    try:
        root = ET.fromstring(xml)
        result = json.loads(stdout)
    except (ET.ParseError, ValueError) as error:
        raise ValueError('incomplete measurement output') from error
    if root.tag != 'valgrindoutput' or root.findtext('protocolversion') not in ('4', '6') or root.findtext('protocoltool') != 'memcheck' or root.findtext('status[last()]/state') != 'FINISHED':
        raise ValueError('missing completed memcheck report')
    if not isinstance(result, dict) or result.get('ok') is not True:
        raise ValueError('fixture semantics failed')
    errors = []
    for item in root.findall('error'):
        kind = item.findtext('kind')
        if kind not in ('UninitCondition', 'UninitValue'):
            raise ValueError('unexpected memcheck error: '+str(kind))
        frames = []
        for frame in item.findall('stack/frame'):
            row = {key: frame.findtext(key) for key in ['ip', 'obj', 'fn', 'dir', 'file', 'line'] if frame.findtext(key) is not None}
            if 'line' in row:
                row['line'] = int(row['line'])
            frames.append(row)
        errors.append(dict(kind=kind, what=item.findtext('what'), frames=frames))
    if code != (9 if errors else 0) or (control and not errors):
        raise ValueError('memcheck exit or positive-control mismatch')
    return dict(secret_flow_contexts=len(errors), errors=errors, fixture=result)


def command(args, **kwargs):
    return subprocess.run([str(x) for x in args], check=True, timeout=600, **kwargs)


def executable(value):
    path = shutil.which(str(value))
    if path is None:
        raise ValueError('executable unavailable: '+str(value))
    return Path(path).resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT/'coax/bin/coax')
    parser.add_argument('--cc', default='gcc')
    parser.add_argument('--valgrind', default='valgrind')
    parser.add_argument('--valgrind-include', type=Path, required=True)
    parser.add_argument('--source-revision', required=True, help='caller label; source hashes are authoritative')
    parser.add_argument('--profile', action='append', choices=[p for p, _ in PROFILES], help='default: both profiles')
    parser.add_argument('--out', type=Path, required=True, help='new directory, never overwritten')
    args = parser.parse_args()
    out = args.out.resolve(); out.mkdir(parents=True, exist_ok=False)
    compiler, cc, valgrind = [executable(v) for v in [args.compiler, args.cc, args.valgrind]]
    identities = {str(p): sha(p) for p in [compiler, cc, valgrind, HERE/'probe.c', Path(__file__), VECTORS]}
    record = dict(schema='coax.signer-native-comparison.v1', source_revision=args.source_revision,
                  source_revision_is_caller_label=True, measurement_complete=False,
                  production_crypto_approved=False, object_timing_approved=False,
                  report_scope='distinct memcheck secret-flow contexts, not leak severity or execution counts',
                  compiler=command([compiler, '--version'], capture_output=True, text=True).stdout.strip(),
                  cc=command([cc, '--version'], capture_output=True, text=True).stdout.splitlines()[0],
                  valgrind=command([valgrind, '--version'], capture_output=True, text=True).stdout.strip(),
                  input_sha256=identities, cases=[], runs=[])
    try:
        c = out/'release.c'; header = out/'es.h'; source_map = out/'source-map.json'
        with c.open('wb') as stream:
            command([compiler, 'c', ROOT/'coax/lib/nostr/event_sign.coax', '--library', '--header', header,
                     '--source-map', source_map, '--run-steps', '250000000', '--release'], stdout=stream)
        metadata = json.loads(source_map.read_text())
        verify_sources(metadata['source_hashes'])
        record.update(source_sha256=metadata['source_hashes'], generated_c_sha256=sha(c),
                      generated_header_sha256=sha(header), source_map_sha256=sha(source_map))
        cases = fixtures()
        for case in cases:
            request = ' '.join(case[k] or '-' for k in ['key', 'aux', 'message', 'signature'])+'\n'
            record['cases'].append(dict(name=case['name'], mode=case['mode'], request_sha256=hashlib.sha256(request.encode()).hexdigest()))
        for profile, flags in PROFILES:
            if args.profile and profile not in args.profile:
                continue
            binary = out/profile
            build_args = [cc, '-std=c11', *flags, '-g', '-I'+str(out), '-I'+str(args.valgrind_include.resolve()), c, HERE/'probe.c', '-o', binary]
            command(build_args)
            binary_sha = sha(binary)
            # A runner that merely returns success on a wrong signature must
            # not turn into a green measurement. This is a public fixture fault.
            wrong = dict(cases[0], signature='00'*64)
            request = ' '.join(wrong[k] or '-' for k in ['key', 'aux', 'message', 'signature'])+'\n'
            negative = subprocess.run([str(binary), 'valid'], input=request, capture_output=True, text=True, timeout=600)
            if negative.returncode != 3 or json.loads(negative.stdout).get('ok') is not False:
                raise ValueError('signature mismatch was not detected by the harness')
            record.setdefault('harness_negative', []).append(dict(profile=profile, exit_code=negative.returncode,
                                                                  fixture=json.loads(negative.stdout)))
            for case in cases:
                request = ' '.join(case[k] or '-' for k in ['key', 'aux', 'message', 'signature'])+'\n'
                prefix = profile+'-'+case['name']
                xml = out/(prefix+'.xml'); stderr = out/(prefix+'.stderr'); stdout = out/(prefix+'.stdout')
                start = time.monotonic()
                run_args = [valgrind, '--tool=memcheck', '--xml=yes', '--xml-file='+str(xml), '--error-exitcode=9',
                            '--track-origins=no', '--num-callers=4', '--error-limit=no', binary, case['mode']]
                run = subprocess.run([str(x) for x in run_args], input=request, capture_output=True, text=True, timeout=600)
                stdout.write_text(run.stdout); stderr.write_text(run.stderr)
                measured = observation(run.returncode, xml.read_text(), run.stdout, case['mode']=='positive-control')
                row = dict(profile=profile, flags=flags, case=case['name'], exit_code=run.returncode,
                           valgrind_flags=['--tool=memcheck', '--xml=yes', '--error-exitcode=9',
                                           '--track-origins=no', '--num-callers=4', '--error-limit=no'],
                           seconds=round(time.monotonic()-start, 3), binary_sha256=binary_sha,
                           xml_sha256=sha(xml), stdout_sha256=sha(stdout), stderr_sha256=sha(stderr), **measured)
                record['runs'].append(row)
                print(profile, case['name'], measured['secret_flow_contexts'], 'contexts; fixture ok', flush=True)
            if sha(binary) != binary_sha:
                raise ValueError('measured binary changed during measurement')
        verify_sources(metadata['source_hashes']); verify_sources(identities)
        if sha(c) != record['generated_c_sha256'] or sha(header) != record['generated_header_sha256']:
            raise ValueError('generated artifact changed during measurement')
        record['measurement_complete'] = True
    except Exception as error:
        record['failure'] = type(error).__name__+': '+str(error)
        raise
    finally:
        (out/'comparison.json').write_text(json.dumps(record, indent=2)+'\n')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
