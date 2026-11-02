#!/usr/bin/env python3
"""Compare the extracted walker with a git-pinned controller and an old C client."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
FROZEN = 'bd73544a7e'
ADAPTER = Path('aethyr/tests/walk-native-fixtures.coax')
DRIVER = ROOT / 'aethyr/tests/walk-native.c'
TUNED = Path('aethyr/tests/walk-tuned-native.coax')
TUNED_DRIVER = ROOT / 'aethyr/tests/walk-tuned-native.c'
FAULTS = ['queued-jump', 'clock-carry', 'counter-high', 'previous-pose',
          'residual', 'query-status', 'camera-status']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def closure(entry, base):
    seen = {}

    def visit(path):
        path = path.resolve()
        key = str(path.relative_to(base))
        if key in seen:
            return
        data = path.read_bytes()
        seen[key] = sha(data)
        for name in re.findall(rb'^\s*#load\s+"([^"\n]+)"', data, re.M):
            visit(path.parent / name.decode())

    visit(entry)
    return dict(sorted(seen.items()))


def frozen_tree(work, commit):
    mirror = (work / 'frozen').resolve()
    seen = set()

    def materialize(relative):
        destination = (mirror / relative).resolve()
        name = str(destination.relative_to(mirror))
        if name in seen:
            return
        seen.add(name)
        data = subprocess.check_output(['git', 'show', f'{commit}:{name}'], cwd=ROOT, timeout=15)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        for dependency in re.findall(rb'^\s*#load\s+"([^"\n]+)"', data, re.M):
            materialize(str((destination.parent / dependency.decode()).resolve().relative_to(mirror)))

    materialize('aethyr/player.coax')
    target = mirror / ADAPTER
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes((ROOT / ADAPTER).read_bytes())
    # Adapter imports are explicit even when the old Player reached the same files.
    for name in re.findall(rb'^\s*#load\s+"([^"\n]+)"', target.read_bytes(), re.M):
        materialize(str((target.parent / name.decode()).resolve().relative_to(mirror)))
    return mirror


def old_scalar_fields(mirror):
    """Inventory every declared old Player field independently of the C serializer."""
    owners = {'Player':'aethyr/player.coax', 'Vec3':'aethyr/player.coax',
              'InputPress':'aethyr/input.coax', 'FrameClock':'aethyr/loop.coax'}

    def fields(type_name, prefix=''):
        source = re.sub(r'//[^\n]*', '', (mirror/owners[type_name]).read_text())
        marker = re.search(r'\b'+type_name+r'\s*::\s*struct\s*\{',source)
        if marker is None:
            raise RuntimeError('missing frozen declared type '+type_name)
        first=marker.end();at=first;depth=1
        while depth:
            if source[at]=='{':depth+=1
            if source[at]=='}':depth-=1
            at+=1
        body=source[first:at-1]
        result=[]
        for name,child in re.findall(r'(?:^|;)\s*(\w+)\s*:\s*(\w+)',body):
            path=prefix+name
            result.extend(fields(child,path+'.') if child in owners else [path])
        return result

    return fields('Player')


def qualify(args):
    work = Path(args.work_dir).resolve() if args.work_dir else Path(tempfile.mkdtemp(prefix='aethyr-walk-'))
    work.mkdir(parents=True, exist_ok=True)
    began = time.monotonic()
    deadline = began + 830
    commands = []
    compiler = Path(args.coax).resolve()
    compiler_hash = sha(compiler.read_bytes())
    commit = subprocess.check_output(['git', 'rev-parse', FROZEN], cwd=ROOT, text=True, timeout=15).strip()
    mirror = frozen_tree(work, commit)
    inputs = closure(ROOT / ADAPTER, ROOT)
    tuned_inputs = closure(ROOT / TUNED, ROOT)
    frozen_inputs = closure(mirror / ADAPTER, mirror)
    support_files = [DRIVER, TUNED_DRIVER, Path(__file__).resolve()]
    support = {str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in support_files}
    receipt = {'schema': 'aethyr.walk-extraction-native-parity.v1', 'mode': args.mode,
               'compiler': str(compiler), 'compiler_sha256': compiler_hash,
               'frozen_commit': commit, 'frozen_source_sha256': frozen_inputs,
               'source_sha256': inputs, 'tuned_source_sha256': tuned_inputs, 'support_sha256': support,
               'platform': platform.platform(), 'commands': commands,
               'scope': 'Default-profile native Player arithmetic and all old observable scalar fields; independent pinned implementation and old generated-header/current-library ABI client. Normal Coax authority checks remain enabled.'}

    def save(status):
        receipt.update(status=status, elapsed_seconds=time.monotonic()-began)
        if args.receipt:
            Path(args.receipt).parent.mkdir(parents=True, exist_ok=True)
            Path(args.receipt).write_text(json.dumps(receipt, indent=2)+'\n')

    def run(argv, label, *, output=None, assertion=False):
        remaining = min(180, deadline-time.monotonic())
        if remaining <= 0:
            raise RuntimeError('native parity command reached its 830-second deadline')
        log = work / (args.mode+'-'+label+'.log')
        started = time.monotonic()
        with log.open('wb') as error:
            destination = output.open('wb') if output else subprocess.PIPE
            try:
                process = subprocess.Popen(list(map(str, argv)), cwd=ROOT, stdout=destination,
                    stderr=error, start_new_session=True, env=dict(os.environ,
                    ASAN_OPTIONS='halt_on_error=1', UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1'))
                try:
                    stdout, _ = process.communicate(timeout=remaining)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL); stdout, _ = process.communicate()
                    process.returncode = 124
            finally:
                if output:
                    destination.close()
        stderr = log.read_text(errors='replace')
        commands.append({'argv':list(map(str,argv)), 'exit_status':process.returncode,
                         'duration_seconds':time.monotonic()-started, 'stderr_sha256':sha(log.read_bytes()),
                         'assertion_probe':assertion})
        if assertion:
            if process.returncode in (0, 99, 124) or 'assertion failed' not in stderr:
                raise RuntimeError('runtime Coax assertion was not detected: '+stderr[-2000:])
        elif process.returncode:
            raise RuntimeError(f'{label} exited {process.returncode}\n{stderr[-4000:]}')
        return stdout or b''

    try:
        release = ['--release'] if args.mode == 'release' else []
        emissions = {}
        for name, base, sources, entry in [('frozen', mirror, frozen_inputs, ADAPTER),
                ('current', ROOT, inputs, ADAPTER), ('tuned', ROOT, tuned_inputs, TUNED)]:
            folder = work / (name+('-release' if release else '-debug'))
            folder.mkdir(exist_ok=True)
            code, header, metadata = folder/'player-native.c', folder/'player-native.h', folder/'emission.json'
            identity = {'compiler_sha256':compiler_hash, 'source_sha256':sources, 'coax_flags':release}
            prior = json.loads(metadata.read_text()) if metadata.exists() else {}
            reusable = prior.get('identity')==identity and code.exists() and header.exists() and \
                prior.get('c_sha256')==sha(code.read_bytes()) and prior.get('header_sha256')==sha(header.read_bytes())
            if not reusable:
                run([compiler,'c',base/entry,'--library',*release,'--header',header], name+'-emit',output=code)
                prior = {'identity':identity,'c_sha256':sha(code.read_bytes()),'header_sha256':sha(header.read_bytes())}
                metadata.write_text(json.dumps(prior,indent=2)+'\n')
            emissions[name] = {'code':code,'header':header,'proof':prior,'reused_emission':reusable}
        public = lambda header: set(re.findall(r'\b(verse_player_\w+|verse_runs|coax_default_t_\w+)\s*\(',header.read_text()))
        old_symbols = public(emissions['frozen']['header']); new_symbols = public(emissions['current']['header'])
        if not old_symbols or old_symbols-new_symbols:
            raise RuntimeError('legacy C declarations missing: '+str(sorted(old_symbols-new_symbols)))
        flags = {'debug':['-O0','-g'],'release':['-O2'],
                 'sanitized':['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']}[args.mode]
        binaries = {}
        for name in ['frozen','current','old-header-current','tuned']:
            implementation = emissions['current' if name=='old-header-current' else name]
            client = emissions['frozen' if name=='old-header-current' else name]
            binary = work/(args.mode+'-'+name)
            run([os.environ.get('CC','cc'),'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off',
                 '-I'+str(client['header'].parent),implementation['code'],
                 TUNED_DRIVER if name=='tuned' else DRIVER,'-lm','-o',binary], name+'-compile')
            binaries[name]=binary
        expected = run([binaries['frozen']], 'frozen-run')
        for name in ['current','old-header-current']:
            actual=run([binaries[name]],name+'-run')
            if actual!=expected:
                before=expected.decode().splitlines();after=actual.decode().splitlines()
                mismatch=next((i for i,(a,b) in enumerate(zip(before,after)) if a!=b),min(len(before),len(after)))
                raise RuntimeError(f'{name} first differing trace line {mismatch}:\n'+str(before[mismatch:mismatch+1])+'\n'+str(after[mismatch:mismatch+1]))
        run([binaries['current'],'--assert-probe'],'assert-probe',assertion=True)
        tuned_output=run([binaries['tuned']],'custom-tuning-and-trace')
        detected=[]
        for fault in FAULTS:
            actual=run([binaries['current'],fault],'fault-'+fault)
            if actual==expected:
                raise RuntimeError('producer loss escaped detection: '+fault)
            detected.append(fault)
        nm=run(['nm','-g',binaries['current']],'current-symbols').decode()
        raw_walk_exports=set(re.findall(r'\b_?(walk_\w+)\s*$',nm,re.M))
        if raw_walk_exports-{'walk_parity_frame','walk_parity_assert_probe'}:
            raise RuntimeError('shared Walk unexpectedly exports C symbols: '+str(sorted(raw_walk_exports)))
        after=closure(ROOT/ADAPTER,ROOT)
        if inputs!=after or tuned_inputs!=closure(ROOT/TUNED,ROOT) or \
                support!={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in support_files}:
            raise RuntimeError('source closure changed during qualification')
        lines=expected.decode().splitlines(); fields=lines[1].split()[1:]
        declared=old_scalar_fields(mirror)
        if len(fields)!=len(set(fields)) or set(fields)!=set(declared):
            raise RuntimeError('serializer is not the complete declared old Player: missing '+
                str(sorted(set(declared)-set(fields)))+'; extra '+str(sorted(set(fields)-set(declared))))
        receipt.update(observable_scalar_fields=len(fields),observable_field_names=fields,
            independent_old_declared_leaf_fields=declared,
            compared_states=len(lines)-2, trace_sha256=sha(expected),
            legacy_c_symbols=sorted(old_symbols),additional_c_symbols=sorted(new_symbols-old_symbols),
            c_header_abi='frozen header/current implementation output, every field offset, size and alignment match frozen binary',
            emissions={name:{k:str(v) if isinstance(v,Path) else v for k,v in data.items()} for name,data in emissions.items()},
            producer_loss_controls_detected=detected, runtime_coax_assertion='observed native trap',
            custom_tuning_and_trace=tuned_output.decode().strip(),
            limits=['Native scalar profile on the recorded host; no Wasm or raster qualification.',
                    'Default tuning parity plus hand-derived nondefault acceleration/jump and fixed-schedule trace witnesses; not every admitted tuning combination.',
                    'Every old scalar Player field is serialized; object padding and nominal type IDs are excluded.'])
        save('passed')
        print(f'{args.mode}: {len(lines)-2} states x {len(fields)} observable fields match pinned controller; old-header ABI and {len(detected)} producer-loss controls pass')
    except Exception:
        save('failed')
        raise


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--coax',default=str(ROOT/'coax/bin/coax'))
    parser.add_argument('--mode',choices=['debug','release','sanitized'],default='debug')
    parser.add_argument('--work-dir')
    parser.add_argument('--receipt')
    qualify(parser.parse_args())
