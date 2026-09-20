#!/usr/bin/env python3
"""Validate an explicitly assembled two-attempt choice sweep without inference."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

spec = importlib.util.spec_from_file_location('sweep', Path(__file__).with_name('analyze-choice-state-sweep.py'))
s = importlib.util.module_from_spec(spec)
spec.loader.exec_module(s)
ORIGINAL_SHA = '8fe1d847aafb3d46433636b66d94c91b0f13045f82b185722d25115b051a28aa'


def boundary(original, composite, metadata):
    s.require(len(original) == len(composite) == 704, 'Both retained attempts must contain 704 rows')
    s.require(composite[:384] == original[:384], 'Retained prefix bytes differ')
    s.require(metadata.get('original_rows_sha256') == ORIGINAL_SHA, 'Original SHA differs')
    s.require(metadata.get('retained_rows_before') == 384, 'Recovery boundary differs')
    s.require(metadata.get('recovery_rungs') == list(s.RUNGS[6:]), 'Recovery rung selection differs')
    prefix_sha = hashlib.sha256(b''.join(original[:384])).hexdigest()
    s.require(metadata.get('seeded_prefix_sha256') == prefix_sha, 'Seeded prefix SHA differs')
    p = metadata.get('composite_provenance', {})
    s.require(p.get('original_rows_sha256') == ORIGINAL_SHA and p.get('original_row_count') == 704,
              'Composite original provenance differs')
    s.require(p.get('original_retained_line_range') == [1, 384] and
              p.get('retained_rungs') == list(s.RUNGS[:6]) and
              p.get('rerun_rungs') == list(s.RUNGS[6:]) and p.get('rerun_request_count') == 80,
              'Composite selection boundary differs')
    s.require(p.get('original_controller_exit') == 'unknown; terminal evidence was suppressed by cleanup failure',
              'Original controller outcome must remain unknown')
    s.require(p.get('retained_timeout_and_413') is True and p.get('original_failed_rows_preserved') is True,
              'Original failure retention is not declared')
    for index, line in enumerate(composite):
        s.require(json.loads(line)['rung'] == s.RUNGS[index // 64], 'Composite rung blocks changed')
    return prefix_sha


def card_check(path, manifest):
    cards = json.loads(path.read_text())['models']
    s.require(len(cards) == 1, 'Unexpected model inventory')
    card = cards[0]
    s.require(card.get('adapter') == 'lev-adapted@1' and card.get('name') == 'lev-adapted' and
              card.get('samples') == 8 and card.get('seed_base') == 0 and card.get('pool_width') == 4 and
              card.get('calibration') == 'none', 'Choice release or estimator differs')
    s.require(card.get('manifest', {}).get('release') == 'lev-adapted@1' and
              card.get('base_model_signature') == manifest['base']['signature'] and
              card['manifest'].get('artifact_sha256') == manifest['artifact']['sha256'],
              'Card artifact or base identity differs')
    return card


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('reference-dir', 'original-rows', 'choice-rows', 'measurement-result',
                 'controller-result', 'model-card', 'original-model-card', 'output-dir'):
        parser.add_argument('--' + name, type=Path, required=True)
    a = parser.parse_args()
    jev, base = s.references(a.reference_dir)
    s.require(s.digest(a.original_rows) == ORIGINAL_SHA, 'Retained original attempt changed')
    original = s.validate(s.read_rows(a.original_rows), 'lev-adapted@1')
    choice = s.validate(s.read_rows(a.choice_rows), 'lev-adapted@1')
    for label, rows in [('original', original), ('composite', choice)]:
        s.pair(base, rows, label + ' versus base')
        s.pair(jev, rows, label + ' versus Jev')
    measurement = json.loads(a.measurement_result.read_text())
    terminal = json.loads(a.controller_result.read_text())
    for label, m in [('measurement', measurement), ('terminal', terminal)]:
        s.require(m.get('exit_code') == 0 and m.get('observed_runner_returncode_before_cleanup') == 0 and
                  m.get('validated_complete_paired_sweep') is True, label + ': runner completion not verified')
        s.require(m.get('rows') == 704 and m.get('rows_sha256') == s.digest(a.choice_rows),
                  label + ': rows or SHA differ')
        boundary(a.original_rows.read_bytes().splitlines(keepends=True),
                 a.choice_rows.read_bytes().splitlines(keepends=True), m)
    s.require(not measurement.get('failure'), 'Measurement phase failed')
    # Cleanup must be reported independently, never recast as a model result.
    s.require(not terminal.get('failure') and not terminal.get('cleanup_errors'),
              'Recovery cleanup did not complete; measurement evidence remains retained, but publication is deferred')
    card_check(a.model_card, measurement['manifest'])
    card_check(a.original_model_card, measurement['manifest'])
    s.require(terminal['manifest'] == measurement['manifest'], 'Terminal manifest changed')
    summary = s.summarize(choice, base, jev)
    output = {
        'classification': 'Two-attempt composite; first384 original rows plus whole320-row recovery suffix',
        'original_controller_exit': 'unknown', 'original_sha256': ORIGINAL_SHA,
        'original_failed_attempt_rungs': s.summarize(original, base, jev),
        'rows': 704, 'requests': 176, 'recovery_requests': 80,
        'choice_sha256': s.digest(a.choice_rows), 'paired_reference_sha256': s.PINNED,
        'measurement_result_sha256': s.digest(a.measurement_result),
        'controller_result_sha256': s.digest(a.controller_result),
        'model_card_sha256': s.digest(a.model_card),
        'original_model_card_sha256': s.digest(a.original_model_card),
        'composite_provenance': measurement['composite_provenance'],
        'recovery_source': measurement.get('source'), 'cleanup_errors': terminal.get('cleanup_errors', []),
        'rungs': summary,
    }
    s.require(not a.output_dir.exists(), 'Output directory exists; refusing overwrite')
    a.output_dir.mkdir(parents=True)
    (a.output_dir / 'choice-state-summary.json').write_text(json.dumps(output, indent=2) + '\n')
    (a.output_dir / 'choice-state-tables.md').write_text(s.tables(summary, base, jev))
    print('Validated explicit two-attempt704-row composite; original outcome remains unknown.')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, json.JSONDecodeError) as error:
        print('Validation failed: ' + str(error), file=sys.stderr)
        sys.exit(2)
