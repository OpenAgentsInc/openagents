#!/usr/bin/env python3
"""Validate the complete choice sweep and produce paired publication artifacts.

This is offline analysis: no client, server, or model invocation occurs. Normal
publication requires a successful terminal controller result and all 704 rows.
"""
import argparse
import collections
import copy
import hashlib
import json
import math
from pathlib import Path
import sys

FAMILIES = ('action', 'needs_code', 'progress', 'risk')
RUNGS = ('unbudgeted', 'output 512', 'output 256', 'commands 3', 'turns 8',
         'turns 6', 'turns 4', 'turns 6, message 1024',
         'production: turns 6, message 768', 'turns 6, message 512',
         'turns 4, message 512')
PINNED = {
    '2026-09-20-state-budget.jsonl': '1cb8c1fb20ca9291f8cee6c10cab398e046444da2cf45ddecbe6e4025abd4dda',
    '2026-09-20-state-budget-lev.jsonl': '22535d570d20436d99c7f0c192e5b253cd7b5ce02d8f10a93ae357de26bb8e8a',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def key(row):
    return row['rung'], row['state'], row['family']


def read_rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def validate(rows, door):
    require(len(rows) == 704, f'{door}: expected 704 rows, found {len(rows)}; no partial result is publishable')
    indexed = {key(row): row for row in rows}
    require(len(indexed) == 704, f'{door}: duplicate keys')
    require(tuple(dict.fromkeys(row['rung'] for row in rows)) == RUNGS,
            f'{door}: rung labels or their retained order changed')
    states = {row['state'] for row in rows}
    require(len(states) == 16, f'{door}: expected 16 states')
    expected = {(rung, state, family) for rung in RUNGS for state in states for family in FAMILIES}
    require(set(indexed) == expected, f'{door}: missing or unexpected paired keys')
    for row in rows:
        context = f'{door} {key(row)}'
        require(row['schema'] == 'openagents.coder.state_sweep_row.v1', context + ': wrong schema')
        require(row['suite'] == 'coder-turns-v1' and row['partition'] == 'development', context + ': wrong suite or partition')
        require(row['door'] == door, context + ': wrong door')
        require(type(row['correct']) is bool, context + ': correctness is not boolean')
        require(row['correct'] == (row['chosen'] == row['truth']), context + ': correctness disagrees with selected answer')
        ms = row['latency_ms']
        require(type(ms) in (int, float) and math.isfinite(ms) and ms >= 0, context + ': invalid timing')
        require(type(row['state_bytes']) is int and row['state_bytes'] > 0, context + ': invalid state byte count')
        if row['refusal'] is not None:
            require(isinstance(row['refusal'], str) and row['refusal'], context + ': empty failure')
            require(row['chosen'] is None and row['model'] is None and not row['correct'], context + ': failed request carries an answer')
        elif door == 'lev-adapted@1':
            require(row['model'] == 'lev-adapted', context + ': response model differs from choice release')
    for rung in RUNGS:
        caps = [indexed[(rung, state, FAMILIES[0])]['caps'] for state in states]
        require(all(value == caps[0] for value in caps), f'{door} {rung}: caps vary within rung')
        for state in states:
            request = [indexed[(rung, state, family)] for family in FAMILIES]
            for field in ('caps', 'state_bytes', 'latency_ms', 'refusal', 'model'):
                require(all(row[field] == request[0][field] for row in request), f'{door} {rung}/{state}: inconsistent request field {field}')
    return indexed


def pair(reference, candidate, name):
    require(set(reference) == set(candidate), name + ': paired keys differ')
    for k, row in candidate.items():
        for field in ('suite', 'partition', 'truth', 'caps', 'state_bytes'):
            require(row[field] == reference[k][field], f'{name} {k}: mismatched {field}')


def tally(rows):
    return {'correct': sum(r['correct'] for r in rows), 'rows': len(rows),
            'failed_rows': sum(r['refusal'] is not None for r in rows),
            'missing_answers': sum(r['refusal'] is None and r['chosen'] is None for r in rows)}


def paired_counts(rows, reference):
    cells = collections.Counter((r['correct'], reference[key(r)]['correct']) for r in rows)
    return {'choice_only_correct': cells[(True, False)], 'reference_only_correct': cells[(False, True)],
            'both_correct': cells[(True, True)], 'neither_correct': cells[(False, False)],
            'correct_count_difference': cells[(True, False)] - cells[(False, True)]}


def summarize(choice, base, jev):
    result = []
    for rung in RUNGS:
        rows = [r for r in choice.values() if r['rung'] == rung]
        requests = {r['state']: r for r in rows}
        sizes = sorted(r['state_bytes'] for r in requests.values())
        failures = collections.Counter(r['refusal'] for r in requests.values() if r['refusal'] is not None)
        families = {}
        for family in FAMILIES:
            selected = [r for r in rows if r['family'] == family]
            families[family] = dict(tally(selected), versus_base=paired_counts(selected, base), versus_jev=paired_counts(selected, jev))
        result.append(dict(rung=rung, median_bytes=sizes[len(sizes)//2], maximum_bytes=max(sizes),
                           requests=len(requests), failed_requests=sum(failures.values()),
                           request_failure_categories=dict(sorted(failures.items())), pooled=tally(rows),
                           families=families, versus_base=paired_counts(rows, base), versus_jev=paired_counts(rows, jev)))
    return result


def tables(summary, base, jev):
    lines = ['| Rung | Median B | Largest B | Pooled | `action` | `needs_code` | `progress` | `risk` | Refused requests |',
             '| --- | --- | --- | --- | --- | --- | --- | --- | --- |']
    for r in summary:
        label = '**' + r['rung'] + '**' if r['rung'].startswith('production:') else r['rung']
        values = [label, f"{r['median_bytes']:,}", f"{r['maximum_bytes']:,}", f"{r['pooled']['correct']}/64"]
        values += [f"{r['families'][family]['correct']}/16" for family in FAMILIES]
        values += [str(r['failed_requests'])]
        lines.append('| ' + ' | '.join(values) + ' |')
    lines += ['', 'Refused requests counts SDK request failures, each contributing four incorrect',
              'rows. Exact error categories are retained in the JSON summary; an SDK `Other`',
              'category does not identify a more specific server cause.', '',
              '| Rung | Choice | Base | Jev | Choice − base | Choice − Jev |',
              '| --- | --- | --- | --- | --- | --- |']
    for r in summary:
        b = sum(x['correct'] for x in base.values() if x['rung'] == r['rung'])
        j = sum(x['correct'] for x in jev.values() if x['rung'] == r['rung'])
        c = r['pooled']['correct']
        lines.append(f"| {r['rung']} | {c}/64 | {b}/64 | {j}/64 | {c-b:+d}/64 | {c-j:+d}/64 |")
    return '\n'.join(lines) + '\n'


def references(directory):
    paths = [directory / name for name in PINNED]
    for p in paths:
        require(digest(p) == PINNED[p.name], str(p) + ': retained reference bytes changed')
    jev = validate(read_rows(paths[0]), 'jev (hosted)')
    base = validate(read_rows(paths[1]), 'lev-base')
    pair(jev, base, 'base versus Jev')
    return jev, base


def self_test(directory):
    jev, base = references(directory)
    synthetic = copy.deepcopy(list(base.values()))
    for row in synthetic:
        row['door'] = 'lev-adapted@1'
        if row['refusal'] is None:
            row['model'] = 'lev-adapted'
    candidate = validate(synthetic, 'lev-adapted@1')
    pair(jev, candidate, 'synthetic')
    require(len(summarize(candidate, base, jev)) == 11, 'summary size')
    cases = [synthetic[:-1], synthetic[:-1] + [synthetic[0]]]
    for field, value in [('truth', 'different'), ('state_bytes', 1), ('caps', {}), ('rung', 'renamed')]:
        changed = copy.deepcopy(synthetic)
        changed[0][field] = value
        cases.append(changed)
    for changed in cases:
        try:
            candidate = validate(changed, 'lev-adapted@1')
            pair(jev, candidate, 'synthetic mutation')
        except (ValueError, KeyError):
            continue
        raise ValueError('invalid synthetic fixture unexpectedly passed')
    print('Self-test passed: complete synthetic fixture validated; incomplete, duplicate, truth, byte, cap, and rung mutations rejected. No measured output written.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-dir', type=Path, required=True)
    parser.add_argument('--choice-rows', type=Path)
    parser.add_argument('--controller-result', type=Path)
    parser.add_argument('--model-card', type=Path)
    parser.add_argument('--output-dir', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test(args.reference_dir)
        return
    for name in ('choice_rows', 'controller_result', 'model_card', 'output_dir'):
        require(getattr(args, name) is not None, '--' + name.replace('_', '-') + ' is required')
    jev, base = references(args.reference_dir)
    choice = validate(read_rows(args.choice_rows), 'lev-adapted@1')
    pair(jev, choice, 'choice versus Jev')
    pair(base, choice, 'choice versus base')
    controller = json.loads(args.controller_result.read_text())
    require(not controller.get('failure') and controller.get('exit_code') == 0 and controller.get('validated_complete_paired_sweep') is True,
            'controller did not confirm successful terminal completion')
    require(controller.get('rows') == 704 and controller.get('rows_sha256') == digest(args.choice_rows), 'controller row digest or count differs')
    cards = json.loads(args.model_card.read_text())['models']
    require(len(cards) == 1, 'unexpected model inventory')
    card = cards[0]
    require(card.get('adapter') == 'lev-adapted@1' and card.get('name') == 'lev-adapted' and
            card.get('samples') == 8 and card.get('seed_base') == 0 and card.get('pool_width') == 4,
            'model card does not identify the requested release and estimator')
    require(card.get('manifest', {}).get('release') == 'lev-adapted@1', 'manifest release differs')
    manifest = controller['manifest']
    require(card.get('base_model_signature') == manifest['base']['signature'] and
            card['manifest'].get('artifact_sha256') == manifest['artifact']['sha256'],
            'model card artifact or base differs from the controller manifest')
    require(card.get('calibration') == 'none', 'unexpected loaded calibration; update method before publication')
    summary = summarize(choice, base, jev)
    output = {'rows': 704, 'requests': 176, 'complete_rungs': 11, 'paired_reference_sha256': PINNED,
              'choice_sha256': digest(args.choice_rows), 'controller_sha256': digest(args.controller_result),
              'model_card_sha256': digest(args.model_card), 'rungs': summary}
    require(not args.output_dir.exists(), 'output directory already exists; refusing to overwrite analysis')
    args.output_dir.mkdir(parents=True)
    (args.output_dir/'choice-state-summary.json').write_text(json.dumps(output, indent=2)+'\n')
    (args.output_dir/'choice-state-tables.md').write_text(tables(summary, base, jev))
    print('Validated 704 paired rows, 176 requests, and all 11 unchanged rungs. Analysis written to ' + str(args.output_dir))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, json.JSONDecodeError) as error:
        print('Not publishable: ' + str(error), file=sys.stderr)
        sys.exit(2)
