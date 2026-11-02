#!/usr/bin/env python3
"""Offline real desktop factory/defaults checks; never opens the app or a service.

Use an existing compiler with --coax or COAX. --generated-dir reuses only an
explicit emission record whose compiler and complete source inventory match.
No compiler build, release gate or mutation run is performed.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

from zeron_source_extract import emit_subset, mask, declarations

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'apps/openagents-desktop/zeron/settings.coax'
HARNESS = ROOT / 'apps/openagents-desktop/tests/zeron-defaults.c'
PROCESS = ROOT / 'coax/platform/process-native.c'
FACE = ROOT / 'apps/openagents-desktop/zeron-face.c'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inventory():
    return declarations(ROOT)[1]


def host_projection(out, face_source):
    text = face_source.read_text()
    clean = mask(text)
    parts, records = [], []
    for name in ['at', 'zr_new', 'zr_quit']:
        hit = re.search(r'^(?:static )?[^\n;{}]*\b'+name+r'\s*\([^;{}]*\)\s*\{', clean, re.M)
        if not hit:
            raise RuntimeError('missing exact production C function '+name)
        end = hit.end(); depth = 1
        while depth:
            if clean[end] == '{': depth += 1
            if clean[end] == '}': depth -= 1
            end += 1
        raw = text[hit.start():end]
        parts.append(raw)
        records.append(dict(name=name, path=str(face_source), start=hit.start(), end=end,
                            sha256=hashlib.sha256(raw.encode()).hexdigest()))
    (out/'factory-face.inc').write_text('\n\n'.join(parts)+'\n')
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--coax', default=os.environ.get('COAX', str(ROOT / 'coax/bin/coax')))
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--generated-dir', type=Path)
    parser.add_argument('--face-source', type=Path, default=FACE, help='Retained actual pre-fix source for check-first replay; defaults to production face')
    parser.add_argument('--profile', action='append', choices=['debug', 'release', 'sanitized'])
    args = parser.parse_args()
    requested_out = args.out.absolute()
    if requested_out.exists() or requested_out.is_symlink():
        parser.error('--out must be a new directory; existing observations are immutable')
    out = requested_out.resolve()
    out.mkdir(parents=True)
    face_source = args.face_source.resolve()
    compiler = Path(args.coax).resolve()
    if not compiler.is_file():
        parser.error('supply an existing Coax executable; this check never builds it')
    cc = os.environ.get('COAX_CC', 'cc')
    env = dict(os.environ, ASAN_OPTIONS='detect_leaks=0:halt_on_error=1', UBSAN_OPTIONS='halt_on_error=1')
    runs = []
    deadline = time.monotonic() + 830

    def run(label, command, stdout=None):
        remaining = deadline - time.monotonic()
        if remaining <= 1:
            raise RuntimeError('830-second whole-command bound reached')
        started = time.monotonic()
        log = out / (label + '.log')
        with log.open('wb') as errors:
            if stdout is None:
                result = subprocess.run(command, cwd=ROOT, env=env, stdout=errors, stderr=subprocess.STDOUT, timeout=min(300, remaining))
            else:
                with stdout.open('wb') as output:
                    result = subprocess.run(command, cwd=ROOT, env=env, stdout=output, stderr=errors, timeout=min(300, remaining))
        runs.append(dict(label=label, command=list(map(str, command)), exit_status=result.returncode, seconds=time.monotonic()-started, log=str(log)))
        return result.returncode

    before = inventory()
    owned = [FACE, HARNESS, Path(__file__), PROCESS, Path(__file__).with_name('zeron_source_extract.py')]
    if face_source not in owned: owned.append(face_source)
    def pin_name(path):
        return str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)
    cc_path = Path(shutil.which(cc) or cc).resolve()
    cc = str(cc_path)
    projection_producer = {str(p.relative_to(ROOT)):digest(p) for p in [Path(__file__),Path(__file__).with_name('zeron_source_extract.py')]}
    tool_before = dict(compiler_sha256=digest(compiler), cc_path=str(cc_path), cc_sha256=digest(cc_path),
                       cc_version=subprocess.check_output([cc,'--version'],text=True))
    owned_before = {pin_name(p):digest(p) for p in owned}
    generated = out / 'generated'
    generated.mkdir()
    emission_record = generated / 'emission.json'
    reused = None
    if args.generated_dir:
        previous = args.generated_dir.resolve()
        previous_record = previous / 'emission.json'
        emission = json.loads(previous_record.read_text())
        if emission.get('projection_producer') != projection_producer:
            raise RuntimeError('emission driver/extractor producer differs; emit again')
        if emission['compiler_sha256'] != digest(compiler) or emission['sources'] != before:
            raise RuntimeError('emission compiler/source identity differs; emit again rather than reusing it')
        for name in ['zeron.c', 'zeron.h']:
            if emission['artifacts'][name] != digest(previous / name):
                raise RuntimeError('generated artifact hash differs')
        reused = dict(path=str(previous_record),sha256=digest(previous_record))
        for name in ['zeron.c','zeron.h','emission.json','extraction.json','subset.coax']:
            shutil.copyfile(previous/name,generated/name)
        if emission['projection']['subset_sha256'] != digest(generated/'subset.coax'):
            raise RuntimeError('projected source hash differs')
    else:
        projection = emit_subset(ROOT, generated, ['ZeronState','coaxial_zeron_quit','lab_attach','ps_init','gs_init'])
        # Test-only owner-ready projection, distinct from the blocked full app.
        index, _ = declarations(ROOT)
        attachment = index['coaxial_zeron_attach']
        statements = ['e := s[0].engine[..];', 'e[0].p[0].handle = -1; e[0].probe = -1; e[0].pend = -1;',
                      'e[0].on = true;', 'p := s[0].psionic[..];', 'ps_init(p);', 'gs_init(s[0].gpu[..]);']
        for statement in statements:
            if statement not in attachment['text']:
                raise RuntimeError('actual attachment ownership statement changed: '+statement)
        helper = '\nzdf_attach :: (s:[]ZeronState,body:[]u8,out:[]u8,err:[]u8) #export_c {lab_attach(s[0].lab[..],body,out,err);'+' '.join(statements)+'}\n'
        projection['attachment_statements'] = dict(path=attachment['path'], line=attachment['line'],
                                                   source_sha256=attachment['sha256'], statements=statements)
        with (generated/'subset.coax').open('a') as target: target.write(helper)
        projection['test_helper'] = helper
        projection['subset_sha256'] = digest(generated/'subset.coax')
        (generated/'extraction.json').write_text(json.dumps(projection,indent=2)+'\n')
        status = run('emit', [str(compiler), 'c', str(generated/'subset.coax'), '--library', '--header', str(generated / 'zeron.h')], generated / 'zeron.c')
        if status or before != inventory():
            raise RuntimeError('desktop emission refused or its input closure changed; no native test ran')
        header = (generated / 'zeron.h').read_text()
        if 'void coax_default_t_ZeronState(coax_t_ZeronState *p);' not in header:
            raise RuntimeError('actual generated desktop header lacks the declared default helper')
        emission = dict(compiler=str(compiler), compiler_sha256=digest(compiler), sources=before, projection=projection, projection_producer=projection_producer,
                        artifacts={name:digest(generated/name) for name in ['zeron.c','zeron.h']},
                        command=runs[-1]['command'], exit_status=0)
        emission_record.write_text(json.dumps(emission, indent=2)+'\n')
    host_functions = host_projection(generated, face_source)
    artifact_before = {name:digest(generated/name) for name in ['zeron.c','zeron.h','subset.coax','extraction.json','emission.json','factory-face.inc']}
    passed = True
    for profile in args.profile or ['debug', 'release', 'sanitized']:
        folder = out / profile
        folder.mkdir(exist_ok=True)
        flags = ['-std=c11', '-O2' if profile == 'release' else '-O0', '-g', '-ffp-contract=off', '-fno-fast-math', '-ffunction-sections', '-fdata-sections', '-Wno-unused-function', '-Wno-unused-variable', '-Wno-unused-but-set-variable']
        if profile == 'sanitized':
            flags += ['-fsanitize=address,undefined', '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
        substitutions = ['-Dcxproc_spawn=zdf_real_spawn', '-Dcxproc_getenv=zdf_real_getenv', '-Dcxproc_getenv_byte=zdf_real_getenv_byte', '-Dcxproc_release=zdf_real_release']
        commands = [
            ('process', [cc, *flags, *substitutions, '-c', str(PROCESS), '-o', str(folder/'process.o')]),
            ('generated', [cc, *flags, '-c', str(generated/'zeron.c'), '-o', str(folder/'zeron.o')]),
            ('link', [cc, *flags, '-I'+str(generated), '-I'+str(ROOT/'apps/openagents-desktop'), str(HARNESS), str(folder/'zeron.o'), str(folder/'process.o'), '-Wl,-dead_strip', '-lm', '-o', str(folder/'check')])]
        compiled = True
        for label, command in commands:
            if run(profile+'-'+label, command):
                compiled = False
                passed = False
                break
        if compiled:
            for case in ['defaults', 'attachment']:
                if run(profile+'-'+case, [str(folder/'check'), case]):
                    passed = False
    owned_after = {pin_name(p):digest(p) for p in owned}
    tool_after = dict(compiler_sha256=digest(compiler), cc_path=str(cc_path), cc_sha256=digest(cc_path),
                      cc_version=subprocess.check_output([cc,'--version'],text=True))
    artifact_after = {name:digest(generated/name) for name in artifact_before}
    stable = before == inventory() and owned_before == owned_after and tool_before == tool_after and artifact_before == artifact_after
    passed = passed and stable
    receipt = dict(schema_version=1, recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(), passed=passed,
                   compiler=dict(path=str(compiler), sha256=digest(compiler)), emission=emission, reused_emission=reused,
                   native_toolchain=tool_before, input_stability=dict(passed=stable,owned_before=owned_before,owned_after=owned_after,artifacts_before=artifact_before,artifacts_after=artifact_after,tools_before=tool_before,tools_after=tool_after),
                   sources=owned_before, tested_face_source=str(face_source), host_functions=host_functions,
                   environment={key:env[key] for key in ['ASAN_OPTIONS','UBSAN_OPTIONS']}, runs=runs,
                   native_artifacts={str(p):digest(p) for p in sorted(out.glob('*/check'))},
                   limits=['Exact-declaration subset: actual complete ZeronState/nested defaults, production factory/quit wrappers, and actual Coax lab_attach/quit/close functions. Full desktop emission and full zr_attach are not established. No request is sent.',
                           'Actual process-pool allocation/release is linked. Process spawn and configuration environment reads are stubbed offline; the selected close paths need only release/kill/wait.',
                           'Guarded caller buffers stay live for the owner-ready projection. Production app-life buffer reclamation is not changed or qualified.',
                           'No general lifetime, copied-owner, active-request, compiler qualification, app build, UI or network claim.'])
    (out/'receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(json.dumps(dict(passed=passed, profiles=args.profile or ['debug','release','sanitized'], runs=len(runs), receipt=str(out/'receipt.json'))))
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
