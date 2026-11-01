#!/usr/bin/env python3
"""Copy the compact, already-audited longer-LQR capture. No numeric execution.

The immutable original top record retains historical private stream references.
Large iteration/coefficient streams are HASHED and extent-checked, never copied
or decoded. Data publishes the independently passed audit receipts separately.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import shutil

FINAL_RECORD_SHA256 = '6890299496be8725d91ee9db83310310042ae1c29caf004e66a0b6aee8ceca8e'
GROUPS = ('imports', 'chunks', 'replays', 'evaluations')
COMPACT = {
    'imports': {'checkpoint.bin', 'stdout', 'stderr'},
    'chunks': {'checkpoint.bin', 'progress.jsonl', 'stdout', 'stderr'},
    'replays': {'checkpoint.bin', 'progress.jsonl', 'stdout', 'stderr'},
    'evaluations': {'evaluations.bin', 'quality.bin', 'stdout', 'stderr'},
}
BIG = {'iterations.bin': 62696, 'coefficients.bin': 88}


def need(ok, message):
    if not ok:
        raise ValueError(message)


def identity(path):
    h = hashlib.sha256()
    size = 0
    with Path(path).open('rb') as f:
        while block := f.read(1024 * 1024):
            size += len(block)
            h.update(block)
    return {'bytes': size, 'sha256': h.hexdigest()}


def load(path):
    return json.loads(Path(path).read_text())


def relative_path(value):
    p = Path(value)
    need(not p.is_absolute() and p.parts and '..' not in p.parts, 'safe recorded relative path')
    return p


def publish(run, output, repo):
    need(not output.exists(), 'fresh publication destination; preserve any first attempt')
    before = identity(run / 'record.json')
    need(before['sha256'] == FINAL_RECORD_SHA256, 'exact immutable final81-action record')
    top = load(run / 'record.json')
    descriptor = load(run / 'descriptor.json')
    prepared = load(run / 'prepared.json')
    need(top['schema'] == 'psionic.locomotion-lqr-longer.learning.v1' and top['passed'] is True
         and top['lifecycle_state'] == 'complete', 'completed first continuation')
    need(tuple(len(top[g]) for g in GROUPS) == (3, 72, 3, 3), 'exact3/72/3/3 action captures')
    need(len(top['completed_actions']) == 81 and top['completed_actions'] == descriptor['actions'],
         'exact81 frozen completed actions')
    e = load(repo / 'psionic/fixtures/robotics/locomotion-lqr-longer/experiment.json')
    need(descriptor['actions'] == e['actions'] and descriptor['experiment'] == identity(
         repo / 'psionic/fixtures/robotics/locomotion-lqr-longer/experiment.json'), 'sealed81-action profile')
    need(identity(run / 'descriptor.json') == top['descriptor'] == prepared['descriptor'], 'exact descriptor')
    need(identity(run / 'prepared.json') == top['prepared'] and prepared['passed'] is True
         and prepared['effect_calls'] == 0, 'exact pure preparation')
    need(top['source'] == prepared['source'] == descriptor['source'], 'same frozen source inventory')
    for name, expected in top['source'].items():
        need(identity(repo / relative_path(name)) == expected, 'frozen source ' + name)
    need(identity(run / 'context.bin') == prepared['context'] and
         (run / 'context.bin').read_bytes() == bytes.fromhex(top['descriptor']['sha256']), 'exact new context')
    need(identity(run / 'original-context.bin') == prepared['original_context'] and
         (run / 'original-context.bin').read_bytes() == bytes.fromhex(descriptor['original_descriptor']['sha256']),
         'exact original import context')
    need((top['canonical_steps'], top['canonical_adam_updates'], top['replay_steps'],
          top['replay_adam_updates'], top['deterministic_eval_steps'], top['old_prefix_and_baseline_reruns'])
         == (589824, 4608, 1536, 12, 2304, 0), 'frozen new workload; no old prefix/baseline rerun')

    # Preflight identities BEFORE creating the publication directory.
    copy_jobs = []
    omitted = {}
    audit_bindings = []
    eval_results = {}
    for name in ('record.json', 'descriptor.json', 'prepared.json', 'context.bin', 'original-context.bin'):
        copy_jobs.append((run / name, Path(name), identity(run / name)))
    copy_jobs.append((Path(__file__), Path('publication-tool.py'), identity(__file__)))
    for group in GROUPS:
        for item in top[group]:
            directory = relative_path(item['directory'])
            expected_names = COMPACT[group] | (set(BIG) if group in ('chunks', 'replays') else set())
            need(set(item['artifacts']) == expected_names and item['native']['passed'] is True,
                 'exact compact/stream artifact inventory ' + str(directory))
            ap = run / relative_path(item['audit']['directory']) / 'record.json'
            need(identity(ap) == item['audit']['record'], 'immutable independently passed receipt')
            audit = load(ap)
            need(audit['schema'] == 'psionic.locomotion-lqr-longer.learning-audit.v1'
                 and audit['passed'] is True and audit['auditor'] == descriptor['live_auditor']
                 and audit['inputs']['descriptor'] == top['descriptor']
                 and audit['result']['seed'] == item['seed'], 'same-source/context/seed passed audit')
            label = ('import-seed-' + str(item['seed']) if group == 'imports' else
                     'canonical-seed-' + str(item['seed']) + '-batch-' + f"{item['first_batch']:03d}" if group == 'chunks' else
                     'replay-seed-' + str(item['seed']) if group == 'replays' else
                     'evaluation-seed-' + str(item['seed']))
            need(identity(output.parent / 'independent-learning/audits' / (label + '.json')) == item['audit']['record'],
                 'exact Data-owned public audit copy')
            audit_bindings.append({'historical_private_relative': str(ap.relative_to(run)),
                                   'identity': item['audit']['record'], 'copied_by_this_helper': False,
                                   'Data_public_relative': '../independent-learning/audits/' + label + '.json'})
            for name, expected in item['artifacts'].items():
                source = run / directory / name
                need(identity(source) == expected, 'immutable captured artifact ' + str(source))
                if name in BIG:
                    width = BIG[name]
                    count = item['batches'] * (2048 if name == 'coefficients.bin' else 1)
                    need(expected['bytes'] == 48 + count * width, 'complete private stream extent')
                    omitted[str(directory / name)] = {
                        **expected, 'private_path': str(source), 'header_bytes': 48,
                        'record_bytes': width, 'complete_records': count,
                        'trailing_bytes': 0, 'copied': False, 'decoded_or_recomputed': False,
                    }
                else:
                    copy_jobs.append((source, directory / name, expected))
            if group == 'evaluations':
                result = audit['result']
                need(result['baseline'] is False and result['steps'] == 768, 'only final six-start evaluation')
                eval_results[str(item['seed'])] = {
                    'promoted': result['promoted'], 'source_gain_ok': result['source_gain_ok'],
                    'source_gap_ok': result['source_gap_ok'], 'real_goals_supported': result['real_goals_supported'],
                    'independent_audit': item['audit']['record'],
                }

    aliases = {}
    for seed in (0, 1, 7):
        for first, completed, filename in ((496, 512, f'final-{seed}.bin'), (240, 256, f'after256-{seed}.bin')):
            matches = [x for x in top['chunks'] if x['seed'] == seed and x['first_batch'] == first]
            need(len(matches) == 1, 'unique authenticated checkpoint alias source')
            item = matches[0]
            need(item['batches'] == 16 and item['native']['last_batch'] == completed,
                 'alias is actual completed endpoint')
            source = run / relative_path(item['directory']) / 'checkpoint.bin'
            expected = item['artifacts']['checkpoint.bin']
            ap = run / relative_path(item['audit']['directory']) / 'record.json'
            audit = load(ap)
            need(audit['inputs']['after'] == expected and
                 (audit['result']['seed'], audit['result']['first_batch'], audit['result']['batches']) == (seed, first, 16),
                 'alias independently audited exact endpoint')
            if completed == 256:
                replay = next(x for x in top['replays'] if x['seed'] == seed)
                ra = load(run / relative_path(replay['audit']['directory']) / 'record.json')
                need(ra['inputs']['before'] == expected, 'after256 alias is the actual replay input')
            relative = Path('checkpoints') / filename
            copy_jobs.append((source, relative, expected))
            aliases[str(relative)] = {'source_relative': str(source.relative_to(run)),
                                     'identity': expected, 'seed': seed, 'completed_batch': completed,
                                     'independent_audit': item['audit']['record']}
    need(sorted(eval_results) == ['0', '1', '7'], 'all3 final seed outcomes')
    need(identity(run / 'record.json') == before, 'final record unchanged during preflight')
    output.mkdir(parents=True)
    files = {}
    for source, relative, expected in copy_jobs:
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        need(not target.exists(), 'no overwrite in compact publication')
        shutil.copyfile(source, target)
        need(identity(source) == identity(target) == expected, 'copied bytes exact ' + str(relative))
        files[str(relative)] = expected
    need(identity(run / 'record.json') == before and identity(output / 'record.json') == before,
         'original final record preserved byte-exact')
    need(not list(output.rglob('iterations.bin')) and not list(output.rglob('coefficients.bin')),
         'large streams intentionally excluded')
    publication = {
        'schema': 'psionic.locomotion-lqr-longer.core-compact-publication.v1', 'passed': True,
        'helper': identity(__file__), 'original_private_run': str(run), 'original_final_record': before,
        'execution_integrity_passed': True,
        'all_prescribed_final_goals_met': all(x['promoted'] for x in eval_results.values()),
        'final_seed_flags_carried': eval_results, 'files': files, 'checkpoint_aliases': aliases,
        'private_large_streams': omitted, 'independent_audit_bindings': audit_bindings,
        'copied_bytes': sum(x['bytes'] for x in files.values()),
        'private_stream_bytes_not_copied': sum(x['bytes'] for x in omitted.values()),
        'numeric_decoder_or_oracle_calls': 0, 'training_optimizer_RNG_tape_model_SDK_calls': 0,
        'scope': 'Cached SHA/extent verification and byte copying only. Original top record retains historical private stream paths. Independent audit receipts are Data-owned; no audit rerun, numerical decoding or recomputation. Original128 outcomes and all qualified sources are unchanged.',
    }
    (output / 'publication.json').write_text(json.dumps(publication, indent=2, allow_nan=False) + '\n')
    print(json.dumps({'passed': True, 'publication': identity(output / 'publication.json'),
                      'copied_files': len(files), 'copied_bytes': publication['copied_bytes'],
                      'private_stream_bytes_not_copied': publication['private_stream_bytes_not_copied']}))


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--run', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    p.add_argument('--repo', required=True, type=Path)
    a = p.parse_args()
    publish(a.run.resolve(), a.output.resolve(), a.repo.resolve())
