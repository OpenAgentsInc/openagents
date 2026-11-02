#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""Bounded native SoOwnedCall snapshots/tickets; scripted effects, no live service."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import time
from b2_support import ROOT, closure, source_inventory, bounded_run

HERE = Path(__file__).resolve().parent

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--compiler', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--reuse', type=Path, help='retain exact prior emission/native passes after runner-only correction')
    args = p.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    compiler = args.compiler.resolve()
    cc_name = shutil.which(os.environ.get('COAX_CC', 'cc'))
    if not cc_name:
        raise RuntimeError('no C compiler')
    cc = Path(cc_name).resolve()
    entry = HERE / 'systemone-owner.coax'
    files = closure(entry)
    files.update({path: path.read_bytes() for path in [HERE/'systemone-owner.c', Path(__file__), HERE/'b2_support.py']})
    before = source_inventory(files)
    tools = {str(path): sha(path) for path in [compiler, cc]}
    artifacts, runs = {}, []
    deadline = time.monotonic() + 800
    proof = dict(schema=1, scope=__doc__, recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                 source_before=before, tools_before=tools, runs=runs, status='running', profiles={})
    def save():
        proof['source_after'] = {path: dict(bytes=(ROOT/path).stat().st_size, sha256=sha(ROOT/path)) for path in before}
        proof['inputs_unchanged'] = before == proof['source_after']
        proof['tools_unchanged'] = all(sha(path) == identity for path,identity in tools.items())
        proof['artifacts'] = artifacts
        proof['artifacts_unchanged'] = all(sha(out/path) == identity for path,identity in artifacts.items())
        (out/'qualification.json').write_text(json.dumps(proof, indent=2)+'\n')
    def run(label, argv):
        save()
        if not all(proof[field] for field in ['inputs_unchanged','tools_unchanged','artifacts_unchanged']):
            raise RuntimeError('input, tool or artifact drift')
        start = time.monotonic()
        remaining = deadline-start
        if remaining <= 0:
            raise RuntimeError('800-second whole-check deadline exhausted')
        row = dict(label=label, argv=list(map(str,argv)))
        try:
            result = bounded_run(argv, cwd=ROOT, timeout=min(180,remaining),
                                 env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1', 'UBSAN_OPTIONS':'halt_on_error=1'})
        except BaseException as exc:
            for stream,data in [('stdout',getattr(exc,'output',b'')),('stderr',getattr(exc,'stderr',b''))]:
                target=out/(label+'.'+stream);target.write_bytes(data or b'');artifacts[target.name]=sha(target)
            row.update(exit_status=None,error=type(exc).__name__,seconds=time.monotonic()-start)
            runs.append(row);save();raise
        for stream,data in [('stdout',result.stdout),('stderr',result.stderr)]:
            target=out/(label+'.'+stream);target.write_bytes(data);artifacts[target.name]=sha(target)
        row.update(exit_status=result.returncode,seconds=time.monotonic()-start)
        runs.append(row);save()
        if result.returncode:
            raise RuntimeError(label+': '+result.stderr.decode(errors='replace')[-2000:])
        return result.stdout
    try:
        source,header=out/'systemone-owner.c',out/'systemone-owner.h'
        retained = {}
        if args.reuse:
            previous = args.reuse.resolve()
            receipt = previous/'qualification.json'
            prior = json.loads(receipt.read_text())
            current_inputs = {key:value for key,value in before.items() if key != str(Path(__file__).relative_to(ROOT))}
            prior_inputs = {key:value for key,value in prior['source_before'].items() if key != str(Path(__file__).relative_to(ROOT))}
            if current_inputs != prior_inputs or tools != prior['tools_before']:
                raise RuntimeError('reuse changes more than the runner')
            if not all(prior.get(field) for field in ['inputs_unchanged','tools_unchanged','artifacts_unchanged']):
                raise RuntimeError('prior run drifted')
            for name,identity in prior['artifacts'].items():
                if sha(previous/name) != identity:
                    raise RuntimeError('prior artifact changed: '+name)
            if not any(row['label']=='emit' and row['exit_status']==0 for row in prior['runs']):
                raise RuntimeError('no successful prior emission')
            for artifact in [source,header]:
                artifact.write_bytes((previous/artifact.name).read_bytes())
            for name in ['O0','O2','sanitized']:
                if any(row['label']==name+'-run' and row['exit_status']==0 for row in prior['runs']):
                    observed=json.loads((previous/(name+'-run.stdout')).read_text())
                    if observed['groups'] != 4 or observed['observations'] != 150:
                        raise RuntimeError('prior native oracle incomplete')
                    retained[name]=observed
            proof['retained_results']=dict(receipt=str(receipt),receipt_sha256=sha(receipt),
                                           runner_only_correction=True,profiles=retained)
        else:
            source.write_bytes(run('emit',[compiler,'c',entry,'--library','--header',header]))
        artifacts[source.name],artifacts[header.name]=sha(source),sha(header)
        for name,flags in [('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            if name in retained:
                proof['profiles'][name] = retained[name]
                continue
            binary=out/name
            run(name+'-compile',[cc,'-std=c11',*flags,'-ffp-contract=off','-I',out,source,HERE/'systemone-owner.c','-lm','-o',binary])
            artifacts[name]=sha(binary)
            observed=json.loads(run(name+'-run',[binary]))
            if observed['groups'] != 4 or observed['observations'] != 150:
                raise RuntimeError('incomplete native oracle report')
            proof['profiles'][name]=observed
        proof['status']='passed'
    except BaseException as exc:
        proof.update(status='failed',error=str(exc));raise
    finally:
        save()
        if not all(proof[field] for field in ['inputs_unchanged','tools_unchanged','artifacts_unchanged']):
            raise RuntimeError('final input, tool or artifact drift')
        print(json.dumps(dict(status=proof['status'],receipt=str(out/'qualification.json'))))

if __name__ == '__main__':
    main()
