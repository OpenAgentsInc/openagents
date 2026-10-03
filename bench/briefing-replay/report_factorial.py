#!/usr/bin/env python3
"""Export a registered four-arm replay panel without pooling earlier experiments."""
import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import statistics
import tempfile
import unittest
from unittest import mock

import report
import run_factorial as coordinator

SCHEMA = 'openagents.briefing.factorial-report.v1'
GATE = {'accepted_per_arm': 4, 'minimum_median_cost_reduction_fraction': 0.20, 'minimum_cheaper_blocks': 3, 'maximum_median_wall_ratio': 1.10}
COMPARISONS = {'D_vs_A': ('A', 'D'), 'C_vs_A': ('A', 'C'), 'D_vs_C': ('C', 'D'), 'B_vs_A': ('A', 'B'), 'D_vs_B': ('B', 'D')}
BRIEF_PREFIX = b'\n\nPrepared evidence follows. It is optional source evidence; mandatory instructions above remain in force. Use additional file reads when necessary.\n\n'
WARMUP_PROMPT = b'Reply only READY. This is an instruction-prefix warmup; use no tools.'
PREPARATION_V1 = 'openagents.briefing-replay.preparation.v1'
STRUCTURE_PREPARATION_V1 = 'openagents.briefing-replay.structure-preparation.v1'
MAX_BRIEF_BYTES = 16 * 1024
EXPORT_ERRORS = (OSError, ValueError, TypeError, KeyError, AttributeError)


def plan_digest(plan):
    return report.sha((json.dumps(plan, sort_keys=True, separators=(',', ':')) + '\n').encode())


def preparation_metrics(preparation, payload, source_commit):
    schema = preparation.get('schema')
    payload_sha = report.sha(payload)
    payload_bytes = preparation.get('payload_bytes')
    if type(payload_bytes) is not int or payload_bytes != len(payload) or not 0 < payload_bytes <= MAX_BRIEF_BYTES:
        raise ValueError('The preparation payload size differs or exceeds the 16 KiB bound')
    if schema == STRUCTURE_PREPARATION_V1:
        if preparation.get('status') != 'complete' or preparation.get('policy') != 'ExplicitStructureV1':
            raise ValueError('Structure preparation must be complete under ExplicitStructureV1')
        budget = preparation.get('byte_budget')
        if type(budget) is not int or not payload_bytes <= budget <= MAX_BRIEF_BYTES:
            raise ValueError('The structure preparation payload exceeds its declared budget')
        hashes = preparation.get('hashes', {})
        if preparation.get('source_commit') != source_commit or hashes.get('treatment_sha256') != payload_sha or hashes.get('focused_sha256') != payload_sha:
            raise ValueError('The preparation source or payload differs from the registered task')
        for key in ['model_calls', 'external_git_probes']:
            if type(preparation.get(key)) is not int or preparation[key] != 0:
                raise ValueError('Structure preparation requires an explicit zero ' + key)
        timings = preparation.get('timings_ms', {})
        warm_ms = report.number(timings.get('warm_preview_wall'))
        cold_ms = report.number(timings.get('cold_index_build_wall'))
        if warm_ms is None or cold_ms is None:
            raise ValueError('Structure preparation requires finite warm and cold wall measurements')
        preview, probe, cold = warm_ms / 1000, 0.0, cold_ms / 1000
        paid = 0.0
    elif schema == PREPARATION_V1:
        if preparation.get('revision') != source_commit or preparation.get('payload_sha256') != payload_sha:
            raise ValueError('The preparation source or payload differs from the registered task')
        calls = preparation.get('paid_model_calls')
        if type(calls) is not int or calls < 0:
            raise ValueError('Legacy preparation requires an explicit model-call count')
        timings = preparation.get('timings', {})
        preview = report.number(timings.get('warm_preview_s'))
        probe = report.number(timings.get('runtime_probe_s'))
        if probe is None and preparation.get('probe_triggered') is False:
            probe = 0.0
        cold = None
        paid = 0.0 if calls == 0 else None
    else:
        raise ValueError('Unsupported preparation schema')
    return {'schema': schema, 'warm_brief_wall_s': preview + probe if preview is not None and probe is not None else None, 'paid_brief_cost_usd': paid, 'payload_sha256': payload_sha, 'payload_bytes': payload_bytes, 'warm_preview_s': preview, 'runtime_probe_s': probe, 'cold_index_wall_s': cold, 'policy': 'Add this frozen measured warm preparation to each B/D endpoint. Cold indexing and machine charges are separate.'}


def cold_index_wall(prep, supplied):
    measured = prep.get('cold_index_wall_s')
    if supplied is not None and (report.number(supplied) is None or measured is not None and supplied != measured):
        raise ValueError('The supplied cold-index time differs from the preparation measurement')
    return measured if measured is not None else supplied


def verify_inputs(plan, preparation_path, protocol_path):
    if plan.get('schema') != coordinator.SCHEMA or plan.get('models') != coordinator.MODELS or plan.get('claude_cli_version') != coordinator.CLAUDE_CLI_VERSION or plan.get('effort') != 'medium' or plan.get('order') != coordinator.ORDER:
        raise ValueError('This reporter requires the registered Opus/Sonnet four-block plan')
    bound = {}
    for item in [plan['runner'], plan['instruction_guard'], plan['task_config'], *plan['input_files']]:
        path = Path(item['path']).resolve()
        if report.sha(coordinator.read(path)) != item['sha256']:
            raise ValueError('A registered input changed: ' + path.name)
        bound[str(path)] = item['sha256']
    for path in [preparation_path.resolve(), protocol_path.resolve()]:
        if bound.get(str(path)) != report.sha(coordinator.read(path)):
            raise ValueError('Preparation and prospective protocol must be bound in input_files')
    config = report.read_json(Path(plan['task_config']['path']))
    for key in ['instructions_file', 'instruction_manifest', 'instruction_block', 'task_file', 'brief_file']:
        path = Path(config[key]).resolve()
        if bound.get(str(path)) != report.sha(coordinator.read(path)):
            raise ValueError('A configured task input is not registered')
    preparation = report.read_json(preparation_path)
    prep = preparation_metrics(preparation, coordinator.read(config['brief_file']), config['source_commit'])
    prep['measurement_sha256'] = report.sha(preparation_path.read_bytes())
    return config, prep


def binding(metadata, expected):
    models = coordinator.served_models(metadata)
    valid = metadata.get('model') == expected and metadata.get('effort') == 'medium' and metadata.get('init', {}).get('model') == expected and models == [expected]
    return valid, models


def export_transport_retries(metadata, row):
    """Copy only bounded numeric and enum telemetry; never transport contents."""
    if 'transport_retries' not in metadata:
        return
    records = metadata['transport_retries']
    row['transport_retries'] = []
    malformed = not isinstance(records, list) or len(records) > 4096
    omitted = 0
    if isinstance(records, list):
        omitted = max(0, len(records) - 4096)
        fields = {'method', 'operation', 'attempts', 'http_statuses', 'scheduled_backoff_s', 'outcome', 'elapsed_s'}
        for record in records[:4096]:
            if not isinstance(record, dict):
                malformed, omitted = True, omitted + 1
                continue
            statuses, delays = record.get('http_statuses'), record.get('scheduled_backoff_s')
            valid = (record.get('method') == 'GET'
                     and record.get('operation') in ('process_status', 'file_read', 'other_get')
                     and type(record.get('attempts')) is int and 1 <= record['attempts'] <= 3
                     and isinstance(statuses, list) and 1 <= len(statuses) <= record['attempts']
                     and all(type(code) is int and 100 <= code <= 599 for code in statuses)
                     and statuses[0] in (502, 503, 504)
                     and isinstance(delays, list) and delays in ([], [1], [1, 2])
                     and all(type(delay) is int for delay in delays)
                     and record.get('outcome') in ('succeeded', 'exhausted', 'failed')
                     and report.number(record.get('elapsed_s')) is not None)
            if not valid:
                malformed, omitted = True, omitted + 1
                continue
            if set(record) != fields:
                malformed = True
            row['transport_retries'].append({key: record[key] for key in sorted(fields)})
    row['transport_retry_records_omitted'] = omitted
    if malformed:
        row.setdefault('artifact_errors', []).append('Transport retry telemetry contains malformed, unsupported, or excess fields or records; only valid whitelisted fields are exported.')


def recover_run(run_dir, label, config, runs_root, output, builder, prep, condition, error):
    """Retain known cumulative usage when malformed metadata prevents normal export."""
    scrub = report.Scrubber(config, runs_root)
    row = {'label': label, 'registered': condition is not None, 'condition': condition,
           'status': 'export_error', 'final_result_available': (run_dir / 'result.json').exists(),
           'accepted': False, 'completed': False, 'cost_complete': False,
           'input_consistency_errors': ['export_error'], 'artifact_errors': [],
           'export_error': scrub(str(error)[:1000]), 'attempts': [], 'artifacts': {}}
    records, estimates, sequences = [], [], []
    for name in ['result.json', 'progress.json', 'request.json']:
        path = run_dir / name
        if not path.exists():
            continue
        try:
            record = report.read_json(path)
            if not isinstance(record, dict):
                raise ValueError('Expected an object')
            records.append((name, record))
            value = report.number(record.get('cost_usd_list_estimate'))
            if value is not None:
                estimates.append(value)
            attempts = record.get('attempts', [])
            if isinstance(attempts, list):
                events = [a['result'] for a in attempts if isinstance(a, dict) and isinstance(a.get('result'), dict)]
                if events:
                    sequences.append((name, events))
        except EXPORT_ERRORS as failure:
            row['artifact_errors'].append(name + ': ' + scrub(str(failure)[:1000]))
    try:
        counts, events, scan = report.scan_events(run_dir / 'events.jsonl')
        row.update(tool_counts=dict(sorted(counts.items())), tool_calls=sum(counts.values()), event_scan=scan)
        if events:
            sequences.append(('events.jsonl', events))
    except EXPORT_ERRORS as failure:
        row['artifact_errors'].append('events.jsonl: ' + scrub(str(failure)[:1000]))
    for _, events in sequences:
        estimates.extend(value for event in events if (value := report.number(event.get('total_cost_usd'))) is not None)
    # Different artifacts repeat cumulative observations. Never sum their charges.
    cost = max(estimates) if estimates else None
    row.update(cost_usd_list_estimate=cost, cost_status='incomplete_lower_bound' if cost is not None else 'unknown',
               recovery_sources=[name for name, _ in records],
               recovery_note='Highest observed cumulative estimate, counted once. Malformed metadata prevents complete accounting or acceptance.')
    for _, metadata in records:
        if 'transport_retries' in metadata:
            export_transport_retries(metadata, row)
            break
    if sequences:
        name, events = max(sequences, key=lambda item: (max((report.number(e.get('total_cost_usd')) or 0 for e in item[1]), default=0), len(item[1])))
        row['recovered_usage_source'] = name
        previous = 0.0
        for i, event in enumerate(events, 1):
            value = report.number(event.get('total_cost_usd'))
            item = {'attempt': i, 'cumulative_cost_usd_list_estimate': value,
                    'incremental_cost_usd_list_estimate': value - previous if value is not None and previous is not None and value >= previous else None,
                    'reported_check_passed': None}
            try:
                item['usage_per_input'] = report.public_turn_usage(event)
            except EXPORT_ERRORS:
                row['artifact_errors'].append('Attempt ' + str(i) + ' usage is malformed.')
            row['attempts'].append(item)
            previous = value
        try:
            row['final_model_usage_cumulative'] = report.public_model_usage(events[-1])
        except EXPORT_ERRORS:
            row['artifact_errors'].append('Final model usage is malformed.')
    row['attempt_count'] = len(row['attempts'])
    row['cost_with_preparation_usd_list_estimate'] = None
    row['total_wall_with_preparation_s'] = None
    # Candidate exports remain useful even when the final record is truncated.
    try:
        paths = sorted(run_dir.iterdir())
    except OSError as failure:
        paths = []
        row['artifact_errors'].append('Candidate listing: ' + scrub(str(failure)[:1000]))
    for path in paths:
        if not report.re.fullmatch(r'(?:candidate-\d+|normalized-candidate-\d+|final-candidate)\.json', path.name):
            continue
        try:
            patch, binary = builder.build(report.read_json(path))
            name = label + '/' + path.stem + '.patch'
            row['artifacts'][name] = dict(report.export_text(output / name, patch, scrub), binary_files_omitted=binary, candidate_json_sha256=report.sha(path.read_bytes()))
        except EXPORT_ERRORS as failure:
            row['artifact_errors'].append(path.name + ': ' + scrub(str(failure)[:1000]))
    return row


def export_observed(run_dir, label, config, runs_root, output, builder, prep, condition=None):
    try:
        row = report.export_run(run_dir, label, config, runs_root, output, builder, prep, condition)
        for name in ['result.json', 'progress.json', 'request.json']:
            path = run_dir / name
            if path.exists():
                export_transport_retries(report.read_json(path), row)
                break
        return row
    except EXPORT_ERRORS as error:
        return recover_run(run_dir, label, config, runs_root, output, builder, prep, condition, error)


def export_registered(row, run_dir, plan, config, runs_root, output, builder, prep):
    exported = export_observed(run_dir, row['label'], config, runs_root, output, builder, prep, row['condition'])
    exported.update(arm=row['arm'], block=row['block'], ordinal=row['ordinal'], expected_model=row['model'])
    exported['runner_reported_accepted'] = exported['accepted']
    exported['model_binding_ok'] = False
    exported['expected_claude_cli_version'] = coordinator.CLAUDE_CLI_VERSION
    exported['claude_cli_version'] = None
    exported['cli_version_binding_ok'] = False
    exported['arm_record_valid'] = False
    exported['served_models_from_cumulative_usage'] = []
    if not run_dir.is_dir():
        return exported
    final = run_dir / 'result.json'
    arm_path = run_dir / 'arm-result.json'
    if final.exists() and exported['status'] != 'export_error':
        metadata = report.read_json(final)
        valid, served = binding(metadata, row['model'])
        version_ok, version = coordinator.cli_version_binding(metadata)
        exported.update(model_binding_ok=valid, served_models_from_cumulative_usage=served, cli_version_binding_ok=version_ok, claude_cli_version=version)
        errors = exported.setdefault('input_consistency_errors', [])
        instructions = coordinator.read(config['instructions_file'])
        task = coordinator.read(config['task_file'])
        suffix = BRIEF_PREFIX + coordinator.read(config['brief_file']) if row['condition'] == 'treatment' else b''
        expected_inputs = {'instructions_sha256': report.sha(instructions), 'common_input_sha256': report.sha(instructions + b'\0' + task), 'prompt_sha256': report.sha(task + suffix), 'suffix_sha256': report.sha(suffix)}
        for key, value in expected_inputs.items():
            if metadata.get(key) != value:
                errors.append('registered_' + key)
        if not valid:
            errors.append('served_model_binding')
        if not version_ok:
            errors.append('claude_cli_version_binding')
        if arm_path.exists():
            try:
                arm = report.read_json(arm_path)
                wanted = {'plan_sha256': plan_digest(plan), 'runner_result_sha256': report.sha(final.read_bytes()), 'ordinal': row['ordinal'], 'block': row['block'], 'arm': row['arm'], 'model': row['model'], 'condition': row['condition'], 'model_binding_ok': valid, 'served_models_from_cumulative_usage': served, 'expected_claude_cli_version': coordinator.CLAUDE_CLI_VERSION, 'claude_cli_version': version, 'cli_version_binding_ok': version_ok, 'accepted': bool(metadata.get('accepted') and valid and version_ok), 'completed': bool(metadata.get('completed')), 'cost_complete': bool(metadata.get('cost_complete')), 'cost_usd_list_estimate': metadata.get('cost_usd_list_estimate')}
                if all(arm.get(key) == value for key, value in wanted.items()):
                    exported['arm_record_valid'] = True
                else:
                    errors.append('arm_record_identity')
            except EXPORT_ERRORS as error:
                errors.append('malformed_arm_record')
                exported['artifact_errors'].append('arm-result.json: ' + report.Scrubber(config, runs_root)(str(error)[:1000]))
        else:
            errors.append('missing_arm_record')
    exported['accepted'] = bool(exported['runner_reported_accepted'] and exported['model_binding_ok'] and exported['cli_version_binding_ok'] and exported['arm_record_valid'])
    exported['first_attempt_accepted'] = bool(exported['accepted'] and exported.get('attempt_count') == 1)
    return exported


def common_input_errors(rows):
    errors = []
    registered = [row for row in rows if row['registered']]
    expected = coordinator.schedule()
    if len(registered) != 16 or {row.get('label') for row in registered} != {row['label'] for row in expected}:
        errors.append('The report must contain exactly the 16 registered run labels.')
    by_label = {row.get('label'): row for row in registered}
    for wanted in expected:
        row = by_label.get(wanted['label'], {})
        for key in ['arm', 'block', 'ordinal', 'condition', 'expected_model']:
            value = wanted['model'] if key == 'expected_model' else wanted[key]
            if row.get(key) != value:
                errors.append('A registered schedule field is missing or changed.')
                break
    available = [row for row in registered if row.get('final_result_available')]
    for key in ['common_input_sha256', 'instructions_sha256', 'source_commit']:
        values = [row.get(key) for row in available]
        if values and (None in values or len(set(values)) != 1):
            errors.append('Registered rows disagree on ' + key + '.')
    for condition in ['control', 'treatment']:
        group = [row for row in available if row.get('condition') == condition]
        for key in ['prompt_sha256', 'suffix_sha256']:
            values = [row.get(key) for row in group]
            if values and (None in values or len(set(values)) != 1):
                errors.append('Model arms with the same briefing condition disagree on ' + key + '.')
    return sorted(set(errors))


def compare(rows, reference, candidate, panel_ready):
    registered = [row for row in rows if row['registered']]
    a = sorted([row for row in registered if row.get('arm') == reference], key=lambda row: row['block'])
    b = sorted([row for row in registered if row.get('arm') == candidate], key=lambda row: row['block'])
    result = {'reference': reference, 'candidate': candidate, 'gate': GATE, 'verdict': 'pending', 'blocks': [], 'accepted': {reference: sum(bool(row['accepted']) for row in a), candidate: sum(bool(row['accepted']) for row in b)}}
    for block in range(1, 5):
        left = next((row for row in a if row['block'] == block), {})
        right = next((row for row in b if row['block'] == block), {})
        lc, rc = left.get('cost_with_preparation_usd_list_estimate'), right.get('cost_with_preparation_usd_list_estimate')
        result['blocks'].append({'block': block, 'reference_run': left.get('label'), 'candidate_run': right.get('label'), 'reference_accepted': bool(left.get('accepted')), 'candidate_accepted': bool(right.get('accepted')), 'candidate_cost_ratio': rc / lc if lc and rc is not None else None})
    result['same_acceptance'] = result['accepted'][reference] == result['accepted'][candidate]
    if not panel_ready:
        result['pending_reason'] = 'All registered arms, input identities, model bindings, and costs must be complete.'
        return result
    costs_a = [row['cost_with_preparation_usd_list_estimate'] for row in a]
    costs_b = [row['cost_with_preparation_usd_list_estimate'] for row in b]
    walls_a = [row['total_wall_with_preparation_s'] for row in a]
    walls_b = [row['total_wall_with_preparation_s'] for row in b]
    median_a, median_b = statistics.median(costs_a), statistics.median(costs_b)
    wall_a, wall_b = statistics.median(walls_a), statistics.median(walls_b)
    result['medians'] = {reference: {'cost_usd_list_estimate': median_a, 'total_wall_s': wall_a}, candidate: {'cost_usd_list_estimate': median_b, 'total_wall_s': wall_b}}
    if median_a <= 0 or wall_a <= 0:
        result['pending_reason'] = 'Positive reference medians are required for the registered ratios.'
        return result
    cost_ratio, wall_ratio = median_b / median_a, wall_b / wall_a
    cheaper = sum(row['candidate_cost_ratio'] is not None and row['candidate_cost_ratio'] < 1 for row in result['blocks'])
    both_accepted = result['accepted'][reference] == result['accepted'][candidate] == 4
    passed = both_accepted and cost_ratio <= 1 - GATE['minimum_median_cost_reduction_fraction'] and cheaper >= GATE['minimum_cheaper_blocks'] and wall_ratio <= GATE['maximum_median_wall_ratio']
    result.update(median_cost_ratio=cost_ratio, median_cost_reduction_fraction=1 - cost_ratio, median_wall_ratio=wall_ratio, cheaper_blocks=cheaper, both_arms_accept_all=both_accepted, verdict='cost_gate_passed' if passed else 'cost_gate_not_met')
    if not both_accepted:
        result['interpretation'] = 'Report the acceptance counts and fixed-endpoint costs. This comparison does not establish a cost through acceptance or an effectiveness win.'
    return result


def analyze(rows, setup_errors=()):
    registered = [row for row in rows if row['registered']]
    errors = common_input_errors(rows) + list(setup_errors)
    final_count = sum(row.get('final_result_available', False) for row in registered)
    ready = len(registered) == 16 and final_count == 16 and not errors
    ready = ready and all(row.get('cost_complete') and row.get('model_binding_ok') and row.get('cli_version_binding_ok') and row.get('arm_record_valid') and not row.get('input_consistency_errors') and not row.get('artifact_errors') and row.get('cost_with_preparation_usd_list_estimate') is not None and row.get('total_wall_with_preparation_s') is not None for row in registered)
    comparisons = {name: compare(rows, a, b, ready) for name, (a, b) in COMPARISONS.items()}
    result = {'registered_runs': len(registered), 'final_results': final_count, 'panel_ready': ready, 'input_errors': errors, 'comparisons': comparisons, 'primary_verdict': comparisons['D_vs_A']['verdict'], 'attribution': 'Pending: the complete registered panel is required.'}
    if not ready:
        return result
    bundle = comparisons['D_vs_A']['verdict'] == 'cost_gate_passed'
    routing = comparisons['C_vs_A']['verdict'] == 'cost_gate_passed'
    packing = comparisons['D_vs_C']['verdict'] == 'cost_gate_passed'
    if bundle and routing and not packing:
        result['attribution'] = 'The combined policy and model-only control pass their cost gates. The brief has not demonstrated an additional Sonnet cost win; attribute the demonstrated saving to model selection.'
    elif bundle and packing:
        result['attribution'] = 'The combined policy and Sonnet briefing comparison pass their separate cost gates on this task. This does not establish a population-level interaction.'
    elif bundle:
        result['attribution'] = 'The combined policy passes its cost gate. The separate comparisons do not establish a Sonnet briefing cost win; report their acceptance counts without attributing the bundle saving to the brief alone.'
    elif routing:
        result['attribution'] = 'The model-only control passes its cost gate; the primary combined-policy gate is not met.'
    else:
        result['attribution'] = 'The primary combined-policy gate is not met. Report every arm and each prespecified comparison; do not pool prior Opus runs or expand the sample.'
    # Descriptive ratios only; no post hoc interaction threshold is introduced.
    by_block = []
    for block in range(1, 5):
        ab = comparisons['B_vs_A']['blocks'][block - 1]['candidate_cost_ratio']
        cd = comparisons['D_vs_C']['blocks'][block - 1]['candidate_cost_ratio']
        by_block.append({'block': block, 'sonnet_brief_ratio_over_opus_brief_ratio': cd / ab if ab and cd is not None else None})
    result['descriptive_cost_interaction'] = {'blocks': by_block, 'note': 'A ratio below one means the within-block briefing cost ratio is smaller on Sonnet. No significance or interaction gate was registered; failed-arm costs remain fixed-endpoint observations.'}
    return result


def render(document):
    analysis = document['analysis']
    lines = ['# Model and briefing factorial panel', '', 'Primary verdict: **' + analysis['primary_verdict'].replace('_', ' ') + '**.', '', analysis['attribution'], '', 'A is Opus control; B is Opus with a brief; C is Sonnet control; D is Sonnet with the same brief.', '', '| Arm | Final results | Accepted | First-attempt accepted |', '| --- | ---: | ---: | ---: |']
    for arm in 'ABCD':
        group = [row for row in document['runs'] if row['registered'] and row.get('arm') == arm]
        lines.append(f"| {arm} | {sum(row.get('final_result_available', False) for row in group)}/4 | {sum(row['accepted'] for row in group)}/4 | {sum(row.get('first_attempt_accepted', False) for row in group)}/4 |")
    lines += ['', '| Comparison | Verdict | Median cost ratio | Cheaper blocks | Median recorded endpoint ratio |', '| --- | --- | ---: | ---: | ---: |']
    for name, item in analysis['comparisons'].items():
        cost = '—' if item.get('median_cost_ratio') is None else f"{item['median_cost_ratio']:.4f}"
        wall = '—' if item.get('median_wall_ratio') is None else f"{item['median_wall_ratio']:.4f}"
        lines.append(f"| {name.replace('_vs_', ' versus ')} | {item['verdict'].replace('_', ' ')} | {cost} | {item.get('cheaper_blocks', '—')}/4 | {wall} |")
    lines += ['', 'Ratios compare candidate to reference. Each cost uses the final cumulative CLI list-price estimate once, including repair, plus paid briefing preparation. These estimates are not verified charges. Unknown or incomplete costs do not pass a gate.', '', 'Recorded endpoint wall time is wall_s + preparation_wall_s + warm_brief_wall_s, with warm briefing preparation added only for B and D. It includes source export, instruction checks, prompt setup, the model session, external checks, and executor-process shutdown. The frozen runner records wall_s before final candidate/artifact capture, scratch-workspace deletion, and final result serialization. Those later steps are excluded, so this metric does not measure the entire harness elapsed time. The registered formula and gate remain unchanged. Shared warmups use the same timing boundary and remain separate from scored runs; cold indexing is reported separately.', '', 'Acceptance differences remain explicit. A failed arm’s cost is a fixed-endpoint observation, not a cost through acceptance or an effectiveness win. The five comparisons were planned separately; old Opus runs and unregistered attempts are retained outside their calculations.', '', 'All registered rows, sanitized check logs, raw and normalized candidate patches, model/cache counters, and artifact digests are in metrics.json. Raw transcripts, thinking, tool inputs/results, and account metadata are excluded. Redacted or binary-omitting patches are labeled. Four blocks on one task do not establish broad superiority.', '']
    return '\n'.join(lines)


def build_report(args):
    plan = report.read_json(args.plan)
    config, prep = verify_inputs(plan, args.preparation, args.protocol)
    saved_plan = args.study_root / 'plan.json'
    if saved_plan.exists() and plan_digest(report.read_json(saved_plan)) != plan_digest(plan):
        raise ValueError('The study directory belongs to another registration')
    saved_schedule = args.study_root / 'schedule.json'
    if saved_schedule.exists():
        actual = report.read_json(saved_schedule)
        if actual != {'plan_sha256': plan_digest(plan), 'runs': coordinator.schedule()}:
            raise ValueError('The saved schedule differs from the registration')
    args.output.mkdir(parents=True, exist_ok=True)
    runs_root = args.study_root / 'runs'
    builder = report.PatchBuilder(config['source_repo'], config['source_commit'], config['allowed_roots'])
    rows = []
    for row in coordinator.schedule():
        try:
            exported = export_registered(row, runs_root / row['label'], plan, config, runs_root, args.output, builder, prep)
        except EXPORT_ERRORS as error:
            exported = recover_run(runs_root / row['label'], row['label'], config, runs_root, args.output, builder, prep, row['condition'], error)
            exported.update(arm=row['arm'], block=row['block'], ordinal=row['ordinal'], expected_model=row['model'])
        rows.append(exported)
    expected = {row['label'] for row in coordinator.schedule()}
    if runs_root.exists():
        for i, extra in enumerate(sorted(path for path in runs_root.iterdir() if path.is_dir() and path.name not in expected), 1):
            label = 'unregistered-' + str(i)
            rows.append(export_observed(extra, label, config, runs_root, args.output, builder, prep))
    warmups = []
    setup_errors = []
    for key, filename in [('runner', 'replay.py'), ('instruction_guard', 'instruction_guard.py')]:
        path = args.study_root / 'harness' / filename
        if not path.exists() or report.sha(coordinator.read(path)) != plan[key]['sha256']:
            setup_errors.append('The retained ' + filename + ' differs from its registered source.')
    warmup_root = args.warmups_root or args.study_root / 'warmups'
    for family, model in coordinator.MODELS.items():
        path = warmup_root / family
        row = export_observed(path, 'warmup-' + family, config, warmup_root, args.output, builder, dict(prep, warm_brief_wall_s=0.0, paid_brief_cost_usd=0.0))
        valid = False
        row.update(expected_claude_cli_version=coordinator.CLAUDE_CLI_VERSION, claude_cli_version=None, cli_version_binding_ok=False)
        if (path / 'result.json').exists() and row['status'] != 'export_error':
            try:
                metadata = report.read_json(path / 'result.json')
                valid, served = binding(metadata, model)
                version_ok, version = coordinator.cli_version_binding(metadata)
                row.update(claude_cli_version=version, cli_version_binding_ok=version_ok)
                instructions = coordinator.read(config['instructions_file'])
                expected_inputs = {'source_commit': config['source_commit'], 'instructions_sha256': report.sha(instructions), 'common_input_sha256': report.sha(instructions + b'\0' + WARMUP_PROMPT), 'prompt_sha256': report.sha(WARMUP_PROMPT), 'suffix_sha256': report.sha(b'')}
                errors = row.setdefault('input_consistency_errors', [])
                for key, value in expected_inputs.items():
                    if metadata.get(key) != value:
                        errors.append('registered_warmup_' + key)
                valid = valid and metadata.get('warmup') is True and metadata.get('input_turns_sent') == metadata.get('session_results') == 1 and row.get('tool_calls') == 0 and not errors and not row.get('artifact_errors')
                row['served_models_from_cumulative_usage'] = served
            except EXPORT_ERRORS as error:
                valid = False
                row.setdefault('input_consistency_errors', []).append('malformed_warmup_binding')
                row.setdefault('artifact_errors', []).append(report.Scrubber(config, warmup_root)(str(error)[:1000]))
        row['model_binding_ok'] = valid
        if not row.get('completed') or not row.get('cost_complete') or not valid or not row['cli_version_binding_ok']:
            setup_errors.append('The ' + family + ' shared warmup is missing, incomplete, or differs from its registered inputs, model, CLI version, or no-tool policy.')
        warmups.append(row)
    observed = [row for row in rows + warmups if row['status'] != 'missing']
    document = {'schema': SCHEMA, 'plan_sha256': plan_digest(plan), 'protocol_sha256': report.sha(args.protocol.read_bytes()), 'source_commit': config['source_commit'], 'preparation': prep, 'shared_setup': {'warmups': warmups, 'cold_index_wall_s': cold_index_wall(prep, args.cold_index_seconds), 'machine_cost_usd': None, 'engineering_cost_usd': None}, 'runs': rows, 'analysis': analyze(rows, setup_errors), 'cost_accounting': {'registered_known_cli_cost_usd_list_estimate': sum(row.get('cost_usd_list_estimate') or 0 for row in rows if row['registered']), 'unregistered_known_cli_cost_usd_list_estimate': sum(row.get('cost_usd_list_estimate') or 0 for row in rows if not row['registered']), 'shared_warmup_known_cli_cost_usd_list_estimate': sum(row.get('cost_usd_list_estimate') or 0 for row in warmups), 'all_observed_known_cli_cost_usd_list_estimate': sum(row.get('cost_usd_list_estimate') or 0 for row in observed), 'complete_for_observed_runs': all(row.get('cost_complete') for row in observed), 'note': 'Each cumulative CLI report is counted once. Unknown usage can make these sums lower bounds. No costs from previous task panels are pooled.'}}
    report.write_json(args.output / 'metrics.json', document)
    (args.output / 'README.md').write_text(render(document))
    return document


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--study-root', type=Path)
    parser.add_argument('--plan', type=Path)
    parser.add_argument('--preparation', type=Path)
    parser.add_argument('--protocol', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--warmups-root', type=Path)
    parser.add_argument('--cold-index-seconds', type=float)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner().run(unittest.defaultTestLoader.loadTestsFromTestCase(FactorialTests))
        raise SystemExit(0 if result.wasSuccessful() else 1)
    if any(getattr(args, key) is None for key in ['study_root', 'plan', 'preparation', 'protocol', 'output']):
        parser.error('Supply the study, registered plan, preparation, prospective protocol, and output paths')
    if args.cold_index_seconds is not None and report.number(args.cold_index_seconds) is None:
        parser.error('--cold-index-seconds must be finite and nonnegative')
    document = build_report(args)
    print(json.dumps({'final_results': document['analysis']['final_results'], 'primary_verdict': document['analysis']['primary_verdict']}))


class FactorialTests(unittest.TestCase):
    def structure_preparation(self, payload=b'Synthetic complete evidence.\n'):
        return {'schema': STRUCTURE_PREPARATION_V1, 'policy': 'ExplicitStructureV1', 'status': 'complete', 'source_commit': 'c' * 40, 'model_calls': 0, 'external_git_probes': 0, 'byte_budget': MAX_BRIEF_BYTES, 'payload_bytes': len(payload), 'hashes': {'treatment_sha256': report.sha(payload), 'focused_sha256': report.sha(payload)}, 'timings_ms': {'warm_preview_wall': 231.668202, 'cold_index_build_wall': 1692.404944, 'preview_stages': {'assembly': 30}, 'v2_comparison_wall': 211}}

    def test_structure_preparation_converts_wall_time_and_separates_cold_index(self):
        payload = b'Synthetic complete evidence.\n'
        prep = preparation_metrics(self.structure_preparation(payload), payload, 'c' * 40)
        self.assertAlmostEqual(prep['warm_brief_wall_s'], 0.231668202)
        self.assertEqual(prep['paid_brief_cost_usd'], 0.0)
        self.assertEqual(prep['runtime_probe_s'], 0.0)
        self.assertAlmostEqual(cold_index_wall(prep, None), 1.692404944)
        self.assertEqual(cold_index_wall(prep, prep['cold_index_wall_s']), prep['cold_index_wall_s'])
        with self.assertRaises(ValueError):
            cold_index_wall(prep, 99)

    def test_structure_preparation_rejects_inconsistent_or_unmeasured_inputs(self):
        payload = b'Synthetic complete evidence.\n'
        changes = [('schema', 'unknown'), ('status', 'failed'), ('policy', 'unknown'), ('source_commit', 'd' * 40), ('payload_bytes', 1), ('byte_budget', len(payload) - 1), ('model_calls', None), ('model_calls', False), ('model_calls', 1), ('external_git_probes', None), ('external_git_probes', 1), ('hashes', {'treatment_sha256': '0' * 64, 'focused_sha256': report.sha(payload)}), ('hashes', {'treatment_sha256': report.sha(payload), 'focused_sha256': '0' * 64}), ('timings_ms', {'warm_preview_wall': float('nan'), 'cold_index_build_wall': 1})]
        for key, value in changes:
            with self.subTest(key=key, value=value):
                record = self.structure_preparation(payload)
                record[key] = value
                with self.assertRaises(ValueError):
                    preparation_metrics(record, payload, 'c' * 40)
        oversized = b'x' * (MAX_BRIEF_BYTES + 1)
        with self.assertRaises(ValueError):
            preparation_metrics(self.structure_preparation(oversized), oversized, 'c' * 40)

    def test_legacy_preparation_requires_explicit_schema_and_model_call_count(self):
        payload = b'Synthetic complete evidence.\n'
        record = {'schema': PREPARATION_V1, 'revision': 'c' * 40, 'payload_sha256': report.sha(payload), 'payload_bytes': len(payload), 'paid_model_calls': 0, 'probe_triggered': True, 'timings': {'warm_preview_s': 0.25, 'runtime_probe_s': 0.05}}
        prep = preparation_metrics(record, payload, 'c' * 40)
        self.assertAlmostEqual(prep['warm_brief_wall_s'], 0.30)
        self.assertIsNone(prep['cold_index_wall_s'])
        self.assertEqual(cold_index_wall(prep, 1.5), 1.5)
        del record['paid_model_calls']
        with self.assertRaises(ValueError):
            preparation_metrics(record, payload, 'c' * 40)

    def rows(self):
        costs = {'A': 1.0, 'B': 0.9, 'C': 0.6, 'D': 0.55}
        rows = []
        for item in coordinator.schedule():
            rows.append({**item, 'registered': True, 'expected_model': item['model'], 'final_result_available': True, 'accepted': True, 'first_attempt_accepted': True, 'cost_complete': True, 'model_binding_ok': True, 'cli_version_binding_ok': True, 'arm_record_valid': True, 'input_consistency_errors': [], 'artifact_errors': [], 'cost_with_preparation_usd_list_estimate': costs[item['arm']], 'total_wall_with_preparation_s': 100, 'common_input_sha256': 'a' * 64, 'instructions_sha256': 'b' * 64, 'source_commit': 'c' * 40, 'prompt_sha256': ('d' if item['condition'] == 'control' else 'e') * 64, 'suffix_sha256': ('f' if item['condition'] == 'control' else '0') * 64})
        return rows

    def test_model_control_prevents_misattributed_briefing_benefit(self):
        value = analyze(self.rows())
        self.assertEqual(value['primary_verdict'], 'cost_gate_passed')
        self.assertEqual(value['comparisons']['C_vs_A']['verdict'], 'cost_gate_passed')
        self.assertEqual(value['comparisons']['D_vs_C']['verdict'], 'cost_gate_not_met')
        self.assertIn('model selection', value['attribution'])

    def test_missing_invalid_or_incomplete_registered_rows_stay_pending(self):
        for key, invalid in [('final_result_available', False), ('cost_complete', False), ('model_binding_ok', False), ('cli_version_binding_ok', False), ('arm_record_valid', False), ('common_input_sha256', 'changed'), ('prompt_sha256', 'changed')]:
            with self.subTest(key=key):
                rows = self.rows()
                rows[0][key] = invalid
                value = analyze(rows)
                self.assertEqual(value['primary_verdict'], 'pending')
                self.assertTrue(all(item['verdict'] == 'pending' for item in value['comparisons'].values()))

    def test_failed_cheap_arm_is_not_an_effectiveness_or_accepted_cost_win(self):
        rows = self.rows()
        row = next(row for row in rows if row['arm'] == 'D')
        row['accepted'] = False
        row['cost_with_preparation_usd_list_estimate'] = 0.01
        value = analyze(rows)['comparisons']['D_vs_A']
        self.assertEqual(value['verdict'], 'cost_gate_not_met')
        self.assertFalse(value['same_acceptance'])
        self.assertIn('does not establish', value['interpretation'])

    def test_three_cheaper_blocks_and_wall_threshold_are_independent_gates(self):
        rows = self.rows()
        for row in rows:
            if row['arm'] == 'A':
                row['cost_with_preparation_usd_list_estimate'] = [10, 10, 1, 1][row['block'] - 1]
            if row['arm'] == 'D':
                row['cost_with_preparation_usd_list_estimate'] = 2
        item = analyze(rows)['comparisons']['D_vs_A']
        self.assertLess(item['median_cost_ratio'], 0.8)
        self.assertEqual(item['cheaper_blocks'], 2)
        self.assertEqual(item['verdict'], 'cost_gate_not_met')
        rows = self.rows()
        for row in rows:
            if row['arm'] == 'D':
                row['total_wall_with_preparation_s'] = 111
        self.assertEqual(analyze(rows)['primary_verdict'], 'cost_gate_not_met')

    def test_unregistered_or_previous_opus_results_are_not_pooled(self):
        rows = self.rows()
        old = dict(rows[0], registered=False, label='previous-opus-run', cost_with_preparation_usd_list_estimate=10000)
        with_old = analyze(rows + [old])
        original = analyze(rows)
        self.assertEqual(with_old, original)

    def test_final_model_binding_requires_exact_model_and_medium_effort(self):
        model = coordinator.MODELS['sonnet']
        metadata = {'model': model, 'effort': 'medium', 'init': {'model': model}, 'attempts': [{'result': {'modelUsage': {model: {}}}}]}
        self.assertTrue(binding(metadata, model)[0])
        metadata['attempts'][0]['result']['modelUsage'][coordinator.MODELS['opus']] = {}
        self.assertFalse(binding(metadata, model)[0])

    def test_malformed_arm_record_preserves_valid_cost_attempts_and_artifacts(self):
        for malformed in ['{', '[]']:
            with self.subTest(malformed=malformed), self.synthetic_study() as (args, builder):
                run = args.study_root / 'runs' / coordinator.schedule()[0]['label']
                (run / 'arm-result.json').write_text(malformed)
                with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                    document = build_report(args)
                row = document['runs'][0]
                self.assertEqual(document['analysis']['primary_verdict'], 'pending')
                self.assertEqual(row['cost_usd_list_estimate'], 1.0)
                self.assertEqual(row['attempt_count'], 2)
                self.assertTrue(row['artifacts'])
                self.assertFalse(row['accepted'])
                self.assertIn('malformed_arm_record', row['input_consistency_errors'])
                self.assertAlmostEqual(document['cost_accounting']['all_observed_known_cli_cost_usd_list_estimate'], 12.22)

    def test_truncated_final_records_recover_usage_without_passing_or_double_counting(self):
        for warmup in [False, True]:
            with self.subTest(warmup=warmup), self.synthetic_study() as (args, builder):
                run = args.study_root / ('warmups/sonnet' if warmup else 'runs/' + coordinator.schedule()[0]['label'])
                original = report.read_json(run / 'result.json')
                progress = dict(original, attempts=original['attempts'][:1])
                progress['cost_usd_list_estimate'] = progress['attempts'][0]['result']['total_cost_usd']
                report.write_json(run / 'progress.json', progress)
                (run / 'events.jsonl').write_text(''.join(json.dumps(a['result']) + '\n' for a in original['attempts']))
                (run / 'result.json').write_text('{')
                with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                    document = build_report(args)
                row = document['shared_setup']['warmups'][1] if warmup else document['runs'][0]
                self.assertTrue((args.output / 'metrics.json').is_file())
                self.assertEqual(len(document['runs']), 16)
                self.assertEqual(document['analysis']['primary_verdict'], 'pending')
                self.assertEqual(row['cost_usd_list_estimate'], original['cost_usd_list_estimate'])
                self.assertEqual(row['attempt_count'], len(original['attempts']))
                self.assertFalse(row['cost_complete'])
                self.assertFalse(row['accepted'])
                self.assertEqual(row['cost_status'], 'incomplete_lower_bound')
                self.assertTrue(row['final_model_usage_cumulative'])
                if not warmup:
                    self.assertTrue(row['artifacts'])
                self.assertAlmostEqual(document['cost_accounting']['all_observed_known_cli_cost_usd_list_estimate'], 12.22)
                self.assertFalse(document['cost_accounting']['complete_for_observed_runs'])
                public = (args.output / 'metrics.json').read_text()
                self.assertNotIn('PRIVATE_RESPONSE', public)
                self.assertNotIn('PRIVATE_ACCOUNT', public)

    def test_unrecoverable_cost_stays_unknown_and_other_rows_remain_visible(self):
        with self.synthetic_study() as (args, builder):
            run = args.study_root / 'runs' / coordinator.schedule()[0]['label']
            (run / 'result.json').write_text('{')
            with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                document = build_report(args)
            row = document['runs'][0]
            self.assertIsNone(row['cost_usd_list_estimate'])
            self.assertEqual(row['cost_status'], 'unknown')
            self.assertFalse(row['cost_complete'])
            self.assertEqual(len(document['runs']), 16)
            self.assertEqual(document['analysis']['primary_verdict'], 'pending')
            self.assertAlmostEqual(document['cost_accounting']['all_observed_known_cli_cost_usd_list_estimate'], 11.22)
            self.assertFalse(document['cost_accounting']['complete_for_observed_runs'])

    def test_malformed_scored_binding_retains_otherwise_valid_usage(self):
        with self.synthetic_study() as (args, builder):
            path = args.study_root / 'runs' / coordinator.schedule()[0]['label'] / 'result.json'
            metadata = report.read_json(path)
            metadata['init'] = []
            report.write_json(path, metadata)
            with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                document = build_report(args)
            row = document['runs'][0]
            self.assertEqual(row['cost_usd_list_estimate'], 1.0)
            self.assertEqual(row['attempt_count'], 2)
            self.assertFalse(row['cost_complete'])
            self.assertEqual(document['analysis']['primary_verdict'], 'pending')
            self.assertAlmostEqual(document['cost_accounting']['all_observed_known_cli_cost_usd_list_estimate'], 12.22)

    def test_warmup_input_mismatches_block_the_gate_and_keep_the_charge(self):
        for key in ['source_commit', 'instructions_sha256', 'common_input_sha256', 'prompt_sha256', 'suffix_sha256']:
            with self.subTest(key=key), self.synthetic_study() as (args, builder):
                path = args.study_root / 'warmups/sonnet/result.json'
                metadata = report.read_json(path)
                metadata[key] = 'd' * (40 if key == 'source_commit' else 64)
                report.write_json(path, metadata)
                with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                    document = build_report(args)
                warmup = document['shared_setup']['warmups'][1]
                self.assertFalse(warmup['model_binding_ok'])
                self.assertIn('registered_warmup_' + key, warmup['input_consistency_errors'])
                self.assertEqual(warmup['cost_usd_list_estimate'], 0.01)
                self.assertEqual(document['analysis']['primary_verdict'], 'pending')
                self.assertAlmostEqual(document['cost_accounting']['all_observed_known_cli_cost_usd_list_estimate'], 12.22)

    def retry_record(self):
        return {'method': 'GET', 'operation': 'process_status', 'attempts': 3,
                'http_statuses': [502, 503], 'scheduled_backoff_s': [1, 2],
                'outcome': 'succeeded', 'elapsed_s': 3.25}

    def test_transport_retry_whitelist_retains_scored_and_warmup_records_without_secrets(self):
        with self.synthetic_study() as (args, builder):
            scored = args.study_root / 'runs' / coordinator.schedule()[0]['label']
            warmup = args.study_root / 'warmups/sonnet'
            def update(run, records):
                metadata = report.read_json(run / 'result.json')
                metadata['transport_retries'] = records
                report.write_json(run / 'result.json', metadata)
                if (run / 'arm-result.json').exists():
                    arm = report.read_json(run / 'arm-result.json')
                    arm['runner_result_sha256'] = report.sha((run / 'result.json').read_bytes())
                    report.write_json(run / 'arm-result.json', arm)
            for run in [scored, warmup]:
                update(run, [self.retry_record()])
            with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                document = build_report(args)
            self.assertEqual(document['analysis']['primary_verdict'], 'cost_gate_passed')
            for row in [document['runs'][0], document['shared_setup']['warmups'][1]]:
                self.assertEqual(row['transport_retries'], [self.retry_record()])
                self.assertFalse(row['artifact_errors'])
            private = dict(self.retry_record(), url='https://invalid.test/PRIVATE_URL',
                           path='PRIVATE_PATH', headers={'Authorization': 'PRIVATE_HEADER'},
                           response_body='PRIVATE_BODY')
            malformed = dict(self.retry_record(), operation='PRIVATE_OPERATION')
            for run in [scored, warmup]:
                update(run, [private, malformed])
            with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                document = build_report(args)
            self.assertEqual(document['analysis']['primary_verdict'], 'pending')
            for row in [document['runs'][0], document['shared_setup']['warmups'][1]]:
                self.assertEqual(row['transport_retries'], [self.retry_record()])
                self.assertEqual(row['transport_retry_records_omitted'], 1)
                self.assertTrue(row['artifact_errors'])
            self.assertAlmostEqual(document['cost_accounting']['all_observed_known_cli_cost_usd_list_estimate'], 12.22)
            self.assertNotIn('PRIVATE_', (args.output / 'metrics.json').read_text())

    def test_malformed_present_retry_telemetry_is_visible_and_legacy_absence_is_allowed(self):
        row = {}
        export_transport_retries({}, row)
        self.assertEqual(row, {})
        for invalid in [None, 'PRIVATE_VALUE', {}, [None], [dict(self.retry_record(), attempts=True)],
                        [dict(self.retry_record(), elapsed_s=float('nan'))]]:
            with self.subTest(invalid=type(invalid).__name__):
                row = {}
                export_transport_retries({'transport_retries': invalid}, row)
                self.assertEqual(row['transport_retries'], [])
                self.assertTrue(row['artifact_errors'])
                self.assertNotIn('PRIVATE_VALUE', json.dumps(row))

    def test_retry_telemetry_survives_recovery_from_truncated_final_metadata(self):
        with self.synthetic_study() as (args, builder):
            run = args.study_root / 'runs' / coordinator.schedule()[0]['label']
            progress = report.read_json(run / 'result.json')
            progress['transport_retries'] = [dict(self.retry_record(), outcome='exhausted', http_statuses=[502, 503, 504])]
            report.write_json(run / 'progress.json', progress)
            (run / 'result.json').write_text('{')
            with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                document = build_report(args)
            row = document['runs'][0]
            self.assertEqual(row['transport_retries'], progress['transport_retries'])
            self.assertEqual(row['cost_usd_list_estimate'], 1.0)
            self.assertFalse(row['cost_complete'])
            self.assertEqual(document['analysis']['primary_verdict'], 'pending')

    @contextmanager
    def synthetic_study(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = []
            config = {'source_commit': 'c' * 40, 'source_repo': str(root / 'source'), 'allowed_roots': ['src']}
            def bound(path):
                return {'path': str(path), 'sha256': report.sha(path.read_bytes())}
            for key in ['instructions_file', 'instruction_manifest', 'instruction_block', 'task_file', 'brief_file']:
                path = root / key
                path.write_text(key + '\n')
                config[key] = str(path)
                inputs.append(bound(path))
            config_path = root / 'config.json'
            report.write_json(config_path, config)
            preparation_path = root / 'preparation.json'
            report.write_json(preparation_path, {'schema': PREPARATION_V1, 'payload_bytes': Path(config['brief_file']).stat().st_size, 'revision': config['source_commit'], 'payload_sha256': report.sha(Path(config['brief_file']).read_bytes()), 'paid_model_calls': 0, 'probe_triggered': False, 'timings': {'warm_preview_s': 0.25}})
            protocol_path = root / 'protocol.md'
            protocol_path.write_text('Synthetic prospective protocol.\n')
            inputs += [bound(preparation_path), bound(protocol_path)]
            study = root / 'study'
            (study / 'harness').mkdir(parents=True)
            for name in ['replay.py', 'instruction_guard.py']:
                (study / 'harness' / name).write_text('Synthetic source; never executed.\n')
            plan = {'schema': coordinator.SCHEMA, 'models': coordinator.MODELS, 'claude_cli_version': coordinator.CLAUDE_CLI_VERSION, 'effort': 'medium', 'order': coordinator.ORDER, 'runner': bound(study / 'harness/replay.py'), 'instruction_guard': bound(study / 'harness/instruction_guard.py'), 'task_config': bound(config_path), 'input_files': inputs}
            plan_path = study / 'plan.json'
            report.write_json(plan_path, plan)
            report.write_json(study / 'schedule.json', {'plan_sha256': plan_digest(plan), 'runs': coordinator.schedule()})
            def metadata(model, condition, cost, warmup=False):
                instructions = Path(config['instructions_file']).read_bytes()
                task = WARMUP_PROMPT if warmup else Path(config['task_file']).read_bytes()
                suffix = BRIEF_PREFIX + Path(config['brief_file']).read_bytes() if condition == 'treatment' else b''
                costs = [cost] if warmup else [cost * 0.8, cost]
                attempts = [{'result': {'type': 'result', 'subtype': 'success', 'is_error': False, 'result': 'PRIVATE_RESPONSE', 'total_cost_usd': value, 'modelUsage': {model: {'cacheReadInputTokens': 500 if i == 0 else 900, 'account': 'PRIVATE_ACCOUNT'}}}, 'verification': {'passed': i == len(costs) - 1, 'checks': [{'name': 'acceptance', 'exit_code': 0 if i == len(costs) - 1 else 1, 'log': '/Users/private-person/account\n'}]}} for i, value in enumerate(costs)]
                return {'condition': condition, 'source_commit': config['source_commit'], 'model': model, 'effort': 'medium', 'init': {'model': model, 'claude_code_version': coordinator.CLAUDE_CLI_VERSION, 'account': 'PRIVATE_ACCOUNT'}, 'warmup': warmup, 'input_turns_sent': len(costs), 'session_results': len(costs), 'completed': True, 'accepted': not warmup, 'cost_complete': True, 'cost_status': 'complete_cli_report', 'cost_usd_list_estimate': cost, 'wall_s': 100, 'agent_wall_s': 80, 'checks_wall_s': 20, 'preparation_wall_s': 2, 'attempts': attempts, 'instructions_sha256': report.sha(instructions), 'common_input_sha256': report.sha(instructions + b'\0' + task), 'prompt_sha256': report.sha(task + suffix), 'suffix_sha256': report.sha(suffix)}
            for row in coordinator.schedule():
                run = study / 'runs' / row['label']
                value = metadata(row['model'], row['condition'], {'A': 1.0, 'B': 0.9, 'C': 0.6, 'D': 0.55}[row['arm']])
                report.write_json(run / 'result.json', value)
                report.write_json(run / 'candidate-1.json', [])
                report.write_json(run / 'normalized-candidate-2.json', [])
                report.write_json(run / 'arm-result.json', {**row, 'plan_sha256': plan_digest(plan), 'runner_result_sha256': report.sha((run / 'result.json').read_bytes()), 'served_models_from_cumulative_usage': [row['model']], 'model_binding_ok': True, 'expected_claude_cli_version': coordinator.CLAUDE_CLI_VERSION, 'claude_cli_version': coordinator.CLAUDE_CLI_VERSION, 'cli_version_binding_ok': True, 'accepted': True, 'completed': True, 'cost_complete': True, 'cost_usd_list_estimate': value['cost_usd_list_estimate']})
            for family, model in coordinator.MODELS.items():
                report.write_json(study / 'warmups' / family / 'result.json', metadata(model, 'control', 0.01, True))
            args = argparse.Namespace(plan=plan_path, study_root=study, preparation=preparation_path, protocol=protocol_path, output=root / 'public', warmups_root=None, cold_index_seconds=None)
            builder = mock.Mock()
            builder.build.return_value = ('', [])
            yield args, builder

    def test_export_retains_repairs_and_sanitizes_without_pooling_cumulative_rows(self):
        with self.synthetic_study() as (args, builder):
            study = args.study_root
            with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                document = build_report(args)
            self.assertEqual(document['analysis']['primary_verdict'], 'cost_gate_passed')
            self.assertEqual(len(document['runs']), 16)
            self.assertEqual(document['runs'][0]['cost_usd_list_estimate'], 1.0)
            self.assertAlmostEqual(document['runs'][0]['repair_cost_usd_list_estimate'], 0.2)
            self.assertEqual(document['runs'][0]['final_model_usage_cumulative'][coordinator.MODELS['opus']]['cacheReadInputTokens'], 900)
            self.assertAlmostEqual(document['cost_accounting']['all_observed_known_cli_cost_usd_list_estimate'], 12.22)
            public = '\n'.join(path.read_text() for path in args.output.rglob('*') if path.is_file())
            for private in ['PRIVATE_RESPONSE', 'PRIVATE_ACCOUNT', '/Users/private-person']:
                self.assertNotIn(private, public)
            warmup_path = study / 'warmups/sonnet/result.json'
            warmup = report.read_json(warmup_path)
            warmup['init']['claude_code_version'] = '2.1.288'
            report.write_json(warmup_path, warmup)
            with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                mismatch = build_report(args)
            self.assertEqual(mismatch['analysis']['primary_verdict'], 'pending')
            self.assertFalse(mismatch['shared_setup']['warmups'][1]['cli_version_binding_ok'])
            self.assertEqual(mismatch['shared_setup']['warmups'][1]['cost_usd_list_estimate'], 0.01)
            first_run = study / 'runs' / coordinator.schedule()[0]['label']
            first = report.read_json(first_run / 'result.json')
            first['init']['claude_code_version'] = '2.1.288'
            report.write_json(first_run / 'result.json', first)
            with mock.patch.object(report, 'PatchBuilder', return_value=builder):
                mismatch = build_report(args)
            self.assertEqual(mismatch['analysis']['primary_verdict'], 'pending')
            self.assertTrue(mismatch['runs'][0]['runner_reported_accepted'])
            self.assertFalse(mismatch['runs'][0]['accepted'])
            self.assertFalse(mismatch['runs'][0]['cli_version_binding_ok'])
            self.assertEqual(mismatch['runs'][0]['claude_cli_version'], '2.1.288')
            self.assertEqual(mismatch['runs'][0]['cost_usd_list_estimate'], 1.0)
            self.assertAlmostEqual(mismatch['cost_accounting']['all_observed_known_cli_cost_usd_list_estimate'], 12.22)


if __name__ == '__main__':
    main()
