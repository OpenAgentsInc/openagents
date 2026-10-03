#!/usr/bin/env python3
"""Export bounded public evidence and compare a registered replay panel."""
import argparse
import base64
from collections import Counter
import difflib
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import re
import statistics
import subprocess
import tempfile
import unittest

MAX_JSON = 32 * 1024 * 1024
MAX_LINE = 8 * 1024 * 1024
MAX_EVENTS = 256 * 1024 * 1024
TOOLS = {'Read', 'Edit', 'Write', 'Glob', 'Grep'}
MODEL_COUNTERS = {'inputTokens', 'outputTokens', 'cacheReadInputTokens', 'cacheCreationInputTokens', 'thinkingTokens', 'webSearchRequests', 'costUSD', 'contextWindow', 'maxOutputTokens'}
TURN_COUNTERS = {'input_tokens', 'output_tokens', 'cache_read_input_tokens', 'cache_creation_input_tokens'}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def number(value):
    return value if type(value) in (int, float) and math.isfinite(value) and value >= 0 else None


def read_json(path, maximum=MAX_JSON):
    with path.open('rb') as handle:
        data = handle.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError('JSON artifact exceeds its byte bound')
    return json.loads(data)


def write_json(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, allow_nan=False) + '\n')


class Scrubber:
    """Remove owned paths, run identities, and credential-shaped strings."""
    def __init__(self, config, runs_root, run_id=''):
        names = {'remote_root': '<VERIFICATION_ROOT>', 'remote_source_repo': '<REMOTE_SOURCE>', 'source_repo': '<SOURCE_REPO>', 'workspace_parent': '<WORKSPACES>', 'target_dir': '<TARGET>', 'remote_checker': '<CHECKER>', 'remote_verifier': '<VERIFIER>'}
        self.replacements = [(str(runs_root), '<PRIVATE_RUNS>')]
        self.replacements += [(config[key], label) for key, label in names.items() if config.get(key)]
        if run_id:
            self.replacements.append((run_id, '<RUN_ID>'))
        self.replacements.sort(key=lambda row: len(row[0]), reverse=True)

    def __call__(self, text):
        for old, new in self.replacements:
            text = text.replace(old, new)
        text = re.sub(r'\b[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}\b|\b[0-9a-f]{32}\b', '<RUN_ID>', text, flags=re.I)
        text = re.sub(r'/(?:Users|home|tmp|private/(?:tmp|var)|var/(?:folders|tmp))/[^\s\x00"<>\]\)]+', '<LOCAL_PATH>', text)
        text = re.sub(r'(?i)\bBearer\s+[^\s"\']+', 'Bearer <REDACTED>', text)
        text = re.sub(r'\b(?:sk-ant-|sk-|oak_)[A-Za-z0-9_.-]{16,}', '<REDACTED_CREDENTIAL>', text)
        return re.sub(r'(https?://)[^/\s:@]+:[^/\s@]+@', r'\1<REDACTED>@', text)


def public_model_usage(event):
    result = {}
    for model, values in event.get('modelUsage', {}).items():
        if not isinstance(values, dict) or not re.fullmatch(r'[A-Za-z0-9._:-]{1,128}', model):
            continue
        result[model] = {key: values[key] for key in sorted(MODEL_COUNTERS) if number(values.get(key)) is not None}
    return result


def public_turn_usage(event):
    usage = event.get('usage', {})
    result = {key: usage[key] for key in sorted(TURN_COUNTERS) if number(usage.get(key)) is not None}
    for group, keys in [('cache_creation', ['ephemeral_1h_input_tokens', 'ephemeral_5m_input_tokens']), ('output_tokens_details', ['thinking_tokens']), ('server_tool_use', ['web_search_requests', 'web_fetch_requests'])]:
        if isinstance(usage.get(group), dict):
            result[group] = {key: usage[group][key] for key in keys if number(usage[group].get(key)) is not None}
    return result


def scan_events(path):
    counts = Counter()
    seen = set()
    results = []
    errors = 0
    truncated_tail = False
    consumed = 0
    if not path.exists():
        return counts, results, {'available': False}
    with path.open('rb') as handle:
        while True:
            line = handle.readline(MAX_LINE + 1)
            if not line:
                break
            consumed += len(line)
            if consumed > MAX_EVENTS or len(line) > MAX_LINE:
                raise ValueError('Event stream exceeds its byte bound')
            if not line.endswith(b'\n'):
                truncated_tail = True
                break
            try:
                event = json.loads(line)
            except (ValueError, UnicodeDecodeError):
                errors += 1
                continue
            if event.get('type') == 'result':
                results.append(event)
            if event.get('type') != 'assistant':
                continue
            content = event.get('message', {}).get('content', [])
            if not isinstance(content, list):
                continue
            for item in content:
                if not isinstance(item, dict) or item.get('type') != 'tool_use':
                    continue
                identity = item.get('id')
                if identity and identity in seen:
                    continue
                if identity:
                    seen.add(identity)
                name = item.get('name')
                counts[name if name in TOOLS else 'Other'] += 1
    return counts, results, {'available': True, 'bytes_scanned': consumed, 'malformed_lines': errors, 'truncated_tail': truncated_tail}


def relative(value):
    path = PurePosixPath(value)
    if not value or value == '.' or path.is_absolute() or '..' in path.parts or str(path) != value:
        raise ValueError('Candidate paths must be normalized repository-relative paths')
    return value


class PatchBuilder:
    def __init__(self, repository, revision, roots):
        if not re.fullmatch(r'[0-9a-f]{40}|[0-9a-f]{64}', revision):
            raise ValueError('Supply a full pinned source revision')
        self.repository, self.revision, self.roots = repository, revision, roots
        resolved = subprocess.check_output(['git', 'rev-parse', '--verify', revision + '^{commit}'], cwd=repository, text=True).strip()
        if resolved != revision:
            raise ValueError('The report source does not resolve to its pinned commit')
        self.cache = {}

    def original(self, path):
        if path not in self.cache:
            result = subprocess.run(['git', 'cat-file', 'blob', self.revision + ':' + path], cwd=self.repository, capture_output=True)
            if result.returncode:
                exists = subprocess.run(['git', 'cat-file', '-e', self.revision + ':' + path], cwd=self.repository, capture_output=True)
                if exists.returncode == 0:
                    raise ValueError('Candidate baseline is not a regular blob')
                self.cache[path] = None
            else:
                if len(result.stdout) > MAX_JSON:
                    raise ValueError('Candidate source file exceeds its byte bound')
                self.cache[path] = result.stdout
        return self.cache[path]

    def build(self, changes):
        pieces = []
        binary = []
        seen = set()
        if not isinstance(changes, list) or len(changes) > 4096:
            raise ValueError('Invalid candidate file count')
        for row in changes:
            path = relative(row['path'])
            if path in seen or not any(path == root or path.startswith(root.rstrip('/') + '/') for root in self.roots):
                raise ValueError('Duplicate or out-of-scope candidate path')
            seen.add(path)
            before = self.original(path)
            after = None if row['content'] is None else base64.b64decode(row['content'], validate=True)
            if after is not None and len(after) > MAX_JSON:
                raise ValueError('Candidate content exceeds its byte bound')
            if before == after:
                continue
            a = 'a/' + path
            b = 'b/' + path
            quote = lambda value: json.dumps(value) if re.search(r'[^A-Za-z0-9_./+-]', value) else value
            pieces.append('diff --git ' + quote(a) + ' ' + quote(b) + '\n')
            if before is None:
                pieces.append('new file mode 100644\n')
            if after is None:
                pieces.append('deleted file mode 100644\n')
            try:
                old = (before or b'').decode('utf-8')
                new = (after or b'').decode('utf-8')
                if '\0' in old or '\0' in new:
                    raise UnicodeError('Binary candidate')
            except UnicodeError:
                binary.append(path)
                pieces.append('Binary content omitted from this public text patch.\n')
                continue
            lines = difflib.unified_diff(old.splitlines(keepends=True), new.splitlines(keepends=True), fromfile='/dev/null' if before is None else quote(a), tofile='/dev/null' if after is None else quote(b))
            for line in lines:
                pieces.append(line if line.endswith('\n') else line + '\n\\ No newline at end of file\n')
        return ''.join(pieces), binary


def export_text(destination, text, scrub):
    public = scrub(text)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(public)
    return {'sha256': sha(public.encode()), 'original_sha256': sha(text.encode()), 'redacted': public != text, 'bytes': len(public.encode())}


def export_run(run_dir, label, config, runs_root, destination, builder, preparation, expected_condition=None):
    row = {'label': label, 'registered': expected_condition is not None, 'condition': expected_condition, 'final_result_available': False, 'status': 'missing', 'accepted': False, 'completed': False, 'attempts': [], 'artifacts': {}}
    if not run_dir.is_dir():
        return row
    final = run_dir / 'result.json'
    partial = run_dir / 'progress.json'
    source = final if final.exists() else partial if partial.exists() else run_dir / 'request.json'
    metadata = read_json(source) if source.exists() else {}
    scrub = Scrubber(config, runs_root, metadata.get('run_id', ''))
    row.update(condition=metadata.get('condition', expected_condition), final_result_available=final.exists(), status='complete' if final.exists() and metadata.get('completed') else 'failed' if final.exists() else 'in_progress_or_interrupted', accepted=bool(final.exists() and metadata.get('accepted')), completed=bool(final.exists() and metadata.get('completed')))
    row['input_consistency_errors'] = []
    row['artifact_errors'] = []
    if metadata.get('source_commit') != config['source_commit']:
        row['input_consistency_errors'].append('source_commit')
    if expected_condition and metadata.get('condition') != expected_condition:
        row['input_consistency_errors'].append('condition')
    for key in ['source_commit', 'instructions_sha256', 'common_input_sha256', 'prompt_sha256', 'suffix_sha256']:
        value = metadata.get(key)
        if isinstance(value, str) and re.fullmatch(r'[0-9a-f]{40}|[0-9a-f]{64}', value):
            row[key] = value
    row['model'] = metadata.get('model') if re.fullmatch(r'[A-Za-z0-9._:-]{1,128}', str(metadata.get('model', ''))) else None
    row['effort'] = metadata.get('effort') if metadata.get('effort') in ['low', 'medium', 'high'] else None
    for key in ['error', 'cleanup_error', 'scope_failure']:
        if metadata.get(key):
            row[key] = scrub(json.dumps(metadata[key], ensure_ascii=False)[:4000])
    row['timeout'] = bool(metadata.get('timeout'))
    try:
        counts, result_events, stream = scan_events(run_dir / 'events.jsonl')
    except (OSError, ValueError, TypeError, KeyError) as error:
        counts, result_events, stream = Counter(), [], {'available': False, 'export_error': scrub(str(error)[:1000])}
        row['artifact_errors'].append('Event counters could not be exported.')
    row['tool_counts'] = dict(sorted(counts.items()))
    row['tool_calls'] = sum(counts.values())
    row['event_scan'] = stream
    attempts = metadata.get('attempts', [])
    events = [attempt.get('result', {}) for attempt in attempts]
    if len(result_events) > len(events):
        events = result_events
    cumulative = [number(event.get('total_cost_usd')) for event in events]
    previous = 0.0
    for i, event in enumerate(events, 1):
        attempt = attempts[i - 1] if i <= len(attempts) else {}
        checks_path = run_dir / ('checks-' + str(i) + '.json')
        verification = attempt.get('verification', {})
        if checks_path.exists():
            try:
                verification = read_json(checks_path)
            except (OSError, ValueError) as error:
                row['artifact_errors'].append('Attempt ' + str(i) + ' checks: ' + scrub(str(error)[:1000]))
        value = cumulative[i - 1]
        item = {'attempt': i, 'cumulative_cost_usd_list_estimate': value, 'incremental_cost_usd_list_estimate': value - previous if value is not None and previous is not None and value >= previous else None, 'model_success': event.get('subtype') == 'success' and event.get('is_error') is False, 'reported_check_passed': verification.get('passed'), 'num_turns_per_input': number(event.get('num_turns')), 'duration_ms_per_input': number(event.get('duration_ms')), 'duration_api_ms_cumulative': number(event.get('duration_api_ms')), 'usage_per_input': public_turn_usage(event), 'checks': []}
        previous = value
        for key in ['scope_failure', 'verification_error']:
            if verification.get(key) or attempt.get(key):
                item[key] = scrub(json.dumps(verification.get(key, attempt.get(key)))[:4000])
        for index, check in enumerate(verification.get('checks', []), 1):
            name = check.get('name', 'unknown')
            if not re.fullmatch(r'[A-Za-z0-9_-]{1,64}', name):
                name = 'check-' + str(index)
            relative_log = label + '/attempt-' + str(i) + '-' + name + '.log'
            log = str(check.get('log', ''))
            row['artifacts'][relative_log] = export_text(destination / relative_log, log, scrub)
            summaries = [{'passed': int(a), 'failed': int(b), 'ignored': int(c)} for a, b, c in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)]
            failed_names = re.findall(r'^test ([A-Za-z0-9_:.-]+) \.\.\. FAILED$', log, flags=re.M)
            item['checks'].append({'name': name, 'exit_code': check.get('exit_code'), 'timed_out': bool(check.get('timed_out')), 'wall_s': number(check.get('wall_s')), 'command': [scrub(str(value)) for value in check.get('command', [])], 'test_summaries': summaries, 'failed_test_names': failed_names, 'log': relative_log})
        row['attempts'].append(item)
    row['attempt_count'] = len(events)
    row['first_attempt_check_passed'] = row['attempts'][0]['reported_check_passed'] if row['attempts'] else None
    row['final_model_usage_cumulative'] = public_model_usage(events[-1]) if events else {}
    final_cost = number(metadata.get('cost_usd_list_estimate'))
    if final_cost is None:
        known = [value for value in cumulative if value is not None]
        final_cost = known[-1] if known else None
    if final.exists() and cumulative and cumulative[-1] is not None and final_cost != cumulative[-1]:
        row['input_consistency_errors'].append('cumulative_cost_mismatch')
    row['cost_usd_list_estimate'] = final_cost
    row['cost_complete'] = bool(final.exists() and metadata.get('cost_complete'))
    row['cost_status'] = metadata.get('cost_status') if final.exists() else 'partial_observation'
    row['repair_cost_usd_list_estimate'] = cumulative[-1] - cumulative[0] if len(cumulative) > 1 and None not in cumulative and cumulative[-1] >= cumulative[0] else None
    prep_cost = preparation['paid_brief_cost_usd'] if row['condition'] == 'treatment' else 0.0
    prep_wall = preparation['warm_brief_wall_s'] if row['condition'] == 'treatment' else 0.0
    row['paid_brief_cost_usd'] = prep_cost
    row['cost_with_preparation_usd_list_estimate'] = final_cost + prep_cost if final_cost is not None and prep_cost is not None else None
    row['warm_brief_wall_s'] = prep_wall
    for key in ['agent_wall_s', 'checks_wall_s', 'preparation_wall_s', 'wall_s']:
        row[key] = number(metadata.get(key))
    pieces = [row['wall_s'], row['preparation_wall_s'], prep_wall]
    row['total_wall_with_preparation_s'] = sum(pieces) if None not in pieces else None
    for path in sorted(run_dir.iterdir()):
        try:
            if re.fullmatch(r'(?:candidate-\d+|normalized-candidate-\d+|final-candidate)\.json', path.name):
                patch, binary = builder.build(read_json(path))
                name = label + '/' + path.stem + '.patch'
                row['artifacts'][name] = dict(export_text(destination / name, patch, scrub), binary_files_omitted=binary, candidate_json_sha256=sha(path.read_bytes()))
            elif re.fullmatch(r'verifier-error-\d+\.json', path.name):
                error = read_json(path)
                text = '\n'.join(str(error.get(key, '')) for key in ['stdout', 'stderr'])
                name = label + '/' + path.stem + '.log'
                row['artifacts'][name] = export_text(destination / name, text, scrub)
        except (OSError, ValueError, TypeError, KeyError) as error:
            row['artifact_errors'].append(path.name + ': ' + scrub(str(error)[:1000]))
    return row


def comparison(rows, registration, panel, development=None, safety=None):
    registered = [row for row in rows if row['registered']]
    result = {'verdict': 'pending', 'registered_runs': len(registered), 'final_results': sum(row['final_result_available'] for row in registered), 'pairs': []}
    for i in range(0, len(registered), 2):
        pair = {row['condition']: row for row in registered[i:i + 2]}
        a, b = pair.get('control', {}), pair.get('treatment', {})
        ac, bc = a.get('cost_with_preparation_usd_list_estimate'), b.get('cost_with_preparation_usd_list_estimate')
        result['pairs'].append({'pair': i // 2 + 1, 'control': a.get('label'), 'treatment': b.get('label'), 'both_final': bool(a.get('final_result_available') and b.get('final_result_available')), 'control_accepted': bool(a.get('accepted')), 'treatment_accepted': bool(b.get('accepted')), 'treatment_cost_ratio': bc / ac if ac and bc is not None else None, 'same_common_input': a.get('common_input_sha256') == b.get('common_input_sha256') if a.get('common_input_sha256') and b.get('common_input_sha256') else None})
    result['conditions'] = {}
    for condition in ['control', 'treatment']:
        group = [row for row in registered if row['condition'] == condition]
        costs = [row['cost_with_preparation_usd_list_estimate'] for row in group if row.get('final_result_available') and row.get('cost_complete') and row.get('cost_with_preparation_usd_list_estimate') is not None]
        walls = [row['total_wall_with_preparation_s'] for row in group if row.get('final_result_available') and row.get('total_wall_with_preparation_s') is not None]
        result['conditions'][condition] = {'accepted': sum(row['accepted'] for row in group), 'final_results': sum(row['final_result_available'] for row in group), 'complete_cost_observations': len(costs), 'median_cost_usd_list_estimate': statistics.median(costs) if costs else None, 'median_total_wall_s': statistics.median(walls) if walls else None}
    first_pair = registered[:2]
    result['early_futility_eligible'] = bool(panel == 'development' and len(first_pair) == 2 and all(row['final_result_available'] and row['completed'] for row in first_pair) and any(row['condition'] == 'treatment' and not row['accepted'] for row in first_pair))
    if len(registered) != 8 or not all(row['final_result_available'] for row in registered):
        result['pending_reason'] = 'The registered four-pair panel is incomplete.'
        return result
    if any(row.get('input_consistency_errors') or row.get('artifact_errors') for row in registered) or any(pair['same_common_input'] is not True for pair in result['pairs']):
        result['pending_reason'] = 'Input identity or common-input mismatch requires review.'
        return result
    if not all(row.get('cost_complete') and row.get('cost_with_preparation_usd_list_estimate') is not None and row.get('total_wall_with_preparation_s') is not None for row in registered):
        result['pending_reason'] = 'Some costs or endpoint durations remain incomplete.'
        return result
    control, treatment = result['conditions']['control'], result['conditions']['treatment']
    result['development_gate_passed'] = treatment['accepted'] == 4 if panel == 'development' else None
    if panel == 'development':
        result['verdict'] = 'advance_to_heldout' if result['development_gate_passed'] else 'development_gate_failed'
        return result
    if not development or development.get('comparison', {}).get('development_gate_passed') is not True:
        result['pending_reason'] = 'A passing development panel is required.'
        return result
    if not control['median_cost_usd_list_estimate'] or not control['median_total_wall_s']:
        result['pending_reason'] = 'The registered ratios require positive control medians.'
        return result
    cost_ratio = treatment['median_cost_usd_list_estimate'] / control['median_cost_usd_list_estimate']
    wall_ratio = treatment['median_total_wall_s'] / control['median_total_wall_s']
    cheaper = sum(pair['treatment_cost_ratio'] is not None and pair['treatment_cost_ratio'] < 1 for pair in result['pairs'])
    result.update(median_cost_ratio=cost_ratio, median_wall_ratio=wall_ratio, cheaper_pairs=cheaper)
    gate = registration['efficiency_gate']
    efficiency = control['accepted'] == gate['heldout_control_accepted'] and treatment['accepted'] == gate['heldout_treatment_accepted'] and cost_ratio <= 1 - gate['minimum_median_cost_reduction_fraction'] and cheaper >= gate['minimum_cheaper_pairs'] and wall_ratio <= gate['maximum_median_wall_ratio']
    gate = registration['correctness_gate']
    correctness_numeric = control['accepted'] <= gate['heldout_control_accepted_max'] and treatment['accepted'] >= gate['heldout_treatment_accepted_min'] and cost_ratio <= gate['maximum_median_cost_ratio'] and wall_ratio <= gate['maximum_median_wall_ratio']
    labels = sorted(row['label'] for row in registered if row['condition'] == 'treatment')
    safety_confirmed = bool(safety and safety.get('no_violations') is True and sorted(safety.get('reviewed_run_labels', [])) == labels)
    observed_scope_failure = any(row.get('scope_failure') or any(attempt.get('scope_failure') for attempt in row.get('attempts', [])) for row in registered if row['condition'] == 'treatment')
    result.update(efficiency_gate_passed=efficiency, correctness_numeric_gate_passed=correctness_numeric, correctness_safety_review_confirmed=safety_confirmed and not observed_scope_failure)
    result['verdict'] = 'efficiency_gate_passed' if efficiency else 'correctness_gate_passed' if correctness_numeric and safety_confirmed and not observed_scope_failure else 'pending' if correctness_numeric and not safety_confirmed else 'no_clear_win'
    if result['verdict'] == 'pending':
        result['pending_reason'] = 'The correctness route requires a review of every treatment candidate for data-preservation and instruction violations.'
    return result


def render(report):
    comparison = report['comparison']
    lines = ['# ' + report['panel'].capitalize() + ' replay panel', '', 'Verdict: **' + comparison['verdict'].replace('_', ' ') + '**.', '', comparison.get('pending_reason', 'This is the registered engineering gate on four pairs, not a population-level significance claim.'), '', '| Run | Status | Accepted | Attempts | CLI cost ($) | Agent (s) | Checks (s) | Recorded endpoint (s) | Tools |', '| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |']
    def fmt(value):
        return '—' if value is None else f'{value:.4f}'
    for row in report['runs']:
        lines.append('| ' + ' | '.join([row['label'], row['status'], ('—' if row['status'] in ('missing', 'in_progress_or_interrupted') else str(row['accepted']).lower()), str(row.get('attempt_count', 0)), fmt(row.get('cost_usd_list_estimate')), fmt(row.get('agent_wall_s')), fmt(row.get('checks_wall_s')), fmt(row.get('total_wall_with_preparation_s')), str(row.get('tool_calls', 0))]) + ' |')
    lines += ['', 'Costs use the last cumulative CLI list-price estimate once, including any repair. They are not verified subscription charges. Incomplete observations remain visible and cannot pass a gate. These are fixed-endpoint costs; failed runs are not a cost through acceptance.', '', 'Recorded endpoint wall time is wall_s + preparation_wall_s + warm_brief_wall_s, with warm briefing preparation added only for treatment. It includes source export, instruction checks, prompt setup, the model session, external checks, and executor-process shutdown. The frozen runner records wall_s before final candidate/artifact capture, scratch-workspace deletion, and final result serialization. Those later steps are excluded, so this metric does not measure the entire harness elapsed time. The registered formula and gate remain unchanged.', '', 'Paid briefing cost is zero only when preparation records zero model calls. Shared instruction warmup uses the same timing boundary, and cold indexing is reported separately in metrics.json; missing machine costs are not treated as zero.', '', 'The JSON includes per-attempt checks, final cumulative model/cache counters, tool counts, and artifact digests. Patches describe file-byte changes; the frozen runner does not record chmod-only changes. Redacted or binary-omitting patches are labeled and are not exact replay artifacts. Raw model text, thinking, tool inputs/results, streams, and init/account metadata are excluded.', '']
    return '\n'.join(lines)


def build_report(args):
    registration = read_json(args.registration)
    config = read_json(args.config)
    preparation = read_json(args.preparation)
    order = registration['order_per_panel']
    if len(order) != 8 or any(sorted(order[i:i + 2]) != ['control', 'treatment'] for i in range(0, 8, 2)):
        raise ValueError('Expected the registered four balanced pairs')
    if registration['source_pins'][args.panel] != config['source_commit'] or preparation['revision'] != config['source_commit']:
        raise ValueError('Registration, configuration, and preparation source pins differ')
    for path in [args.config] + [Path(config[key]) for key in ['instructions_file', 'instruction_manifest', 'instruction_block', 'task_file', 'brief_file']]:
        key = 'private-input/' + path.resolve().relative_to(args.runs_root.resolve().parent).as_posix()
        if registration['files_sha256'].get(key) != sha(path.read_bytes()):
            raise ValueError('A required panel input differs from its registration: ' + path.name)
    if sha(Path(config['brief_file']).read_bytes()) != preparation['payload_sha256']:
        raise ValueError('Preparation does not bind the configured brief')
    times = [number(preparation.get('timings', {}).get(key)) for key in ['warm_preview_s', 'runtime_probe_s']]
    prep = {'warm_brief_wall_s': sum(times) if None not in times else None, 'paid_brief_cost_usd': 0.0 if preparation.get('paid_model_calls') == 0 else None, 'measurement_sha256': sha(args.preparation.read_bytes()), 'payload_sha256': preparation['payload_sha256'], 'payload_bytes': preparation.get('payload_bytes'), 'warm_preview_s': times[0], 'runtime_probe_s': times[1], 'policy': 'Apply this frozen measured warm preparation to each treatment endpoint; machine charges are not measured.'}
    args.output.mkdir(parents=True, exist_ok=True)
    builder = PatchBuilder(config['source_repo'], config['source_commit'], config['allowed_roots'])
    rows = []
    expected = [str(i) + '-' + condition for i, condition in enumerate(order, 1)]
    panel_dir = args.runs_root / args.panel
    names = expected + sorted(path.name for path in panel_dir.iterdir() if path.is_dir() and path.name not in expected) if panel_dir.exists() else expected
    for i, name in enumerate(names):
        label = args.panel + '-' + name if name in expected else args.panel + '-unregistered-' + str(i + 1)
        condition = order[i] if i < len(order) else None
        try:
            row = export_run(panel_dir / name, label, config, args.runs_root, args.output, builder, prep, condition)
        except (OSError, ValueError, TypeError, KeyError) as error:
            row = {'label': label, 'condition': condition, 'registered': condition is not None, 'status': 'export_error', 'final_result_available': (panel_dir / name / 'result.json').exists(), 'accepted': False, 'completed': False, 'attempts': [], 'input_consistency_errors': ['export_error'], 'export_error': Scrubber(config, args.runs_root)(str(error)[:1000])}
        rows.append(row)
    warmup = args.runs_root / ('warmup-' + args.panel)
    shared = {'cold_index_wall_s': args.cold_index_seconds, 'cold_index_cost_usd': None, 'machine_cost_usd': None, 'engineering_cost_usd': None, 'warmup': export_run(warmup, 'warmup-' + args.panel, config, args.runs_root, args.output, builder, dict(prep, warm_brief_wall_s=0.0, paid_brief_cost_usd=0.0))}
    development = read_json(args.development_metrics) if args.development_metrics else None
    if development and (development.get('panel') != 'development' or development.get('registration_sha256') != sha(args.registration.read_bytes()) or development.get('source_commit') != registration['source_pins']['development']):
        raise ValueError('Development metrics belong to another registration')
    safety = read_json(args.safety_review, 65536) if args.safety_review else None
    report = {'schema': 'openagents.briefing-replay.public-panel.v1', 'panel': args.panel, 'registration_sha256': sha(args.registration.read_bytes()), 'source_commit': config['source_commit'], 'preparation': prep, 'shared_setup': shared, 'runs': rows, 'comparison': comparison(rows, registration, args.panel, development, safety), 'privacy': 'Only selected counters, sanitized checker logs, and candidate patches are exported. No raw streams, model prose, thinking, tool input/results, or init/account metadata.'}
    observed = [row for row in rows + [shared['warmup']] if row['status'] != 'missing']
    report['cost_accounting'] = {
        'registered_runs_known_cli_cost_usd_list_estimate': sum(row.get('cost_usd_list_estimate') or 0 for row in rows if row['registered']),
        'unregistered_runs_known_cli_cost_usd_list_estimate': sum(row.get('cost_usd_list_estimate') or 0 for row in rows if not row['registered']),
        'shared_warmup_known_cli_cost_usd_list_estimate': shared['warmup'].get('cost_usd_list_estimate'),
        'all_observed_known_cli_cost_usd_list_estimate': sum(row.get('cost_usd_list_estimate') or 0 for row in observed),
        'complete_for_observed_runs': all(row.get('cost_complete') for row in observed),
        'note': 'Known cumulative reports are counted once per run, including failures. Missing or interrupted usage can make this total a lower bound. Machine and engineering costs are not measured.',
    }
    if args.safety_review:
        report['safety_review_sha256'] = sha(args.safety_review.read_bytes())
    write_json(args.output / 'metrics.json', report)
    (args.output / 'README.md').write_text(render(report))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs-root', type=Path)
    parser.add_argument('--panel', choices=['development', 'heldout'])
    parser.add_argument('--registration', type=Path)
    parser.add_argument('--config', type=Path)
    parser.add_argument('--preparation', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--cold-index-seconds', type=float)
    parser.add_argument('--development-metrics', type=Path)
    parser.add_argument('--safety-review', type=Path, help='JSON with no_violations and reviewed_run_labels; independent review only.')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner().run(unittest.defaultTestLoader.loadTestsFromTestCase(ReportTests))
        raise SystemExit(0 if result.wasSuccessful() else 1)
    if any(getattr(args, key) is None for key in ['runs_root', 'panel', 'registration', 'config', 'preparation', 'output']):
        parser.error('Supply all panel input paths and --output')
    if args.cold_index_seconds is not None and number(args.cold_index_seconds) is None:
        parser.error('--cold-index-seconds must be finite and nonnegative')
    report = build_report(args)
    print(json.dumps({'panel': args.panel, 'verdict': report['comparison']['verdict'], 'final_results': report['comparison']['final_results']}))


class ReportTests(unittest.TestCase):
    def test_scrubber_removes_owned_paths_and_identity(self):
        scrub = Scrubber({'remote_root': '/tmp/private-root'}, Path('/Users/person/private'), 'a' * 32)
        text = '/tmp/private-root/checks/' + 'a' * 32 + '/1 /Users/person/private/account /home/person/key Bearer secret-token'
        public = scrub(text)
        for private in ['/tmp/private-root', '/Users/person', '/home/person', 'a' * 32, 'secret-token']:
            self.assertNotIn(private, public)
        self.assertIn('<VERIFICATION_ROOT>', public)

    def test_stream_counts_unique_tools_without_exporting_content(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'events.jsonl'
            tool = {'type': 'assistant', 'message': {'content': [{'type': 'thinking', 'thinking': 'PRIVATE'}, {'type': 'tool_use', 'id': 'one', 'name': 'Read', 'input': {'path': 'PRIVATE'}}]}}
            path.write_text(json.dumps(tool) + '\n' + json.dumps(tool) + '\n' + '{')
            counts, results, state = scan_events(path)
            self.assertEqual(counts, {'Read': 1})
            self.assertEqual(results, [])
            self.assertTrue(state['truncated_tail'])
            self.assertNotIn('PRIVATE', json.dumps([counts, results, state]))

    def test_final_cumulative_cost_and_usage_are_not_summed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run = root / 'run'
            run.mkdir()
            events = [{'type': 'result', 'subtype': 'success', 'is_error': False, 'total_cost_usd': cost, 'modelUsage': {'claude-test': {'cacheReadInputTokens': cache, 'account': 'PRIVATE'}}} for cost, cache in [(0.25, 100), (0.4, 250)]]
            metadata = {'run_id': 'b' * 32, 'source_commit': 'a' * 40, 'condition': 'control', 'completed': True, 'accepted': True, 'cost_complete': True, 'cost_usd_list_estimate': 0.4, 'attempts': [{'result': event, 'verification': {'passed': i == 1, 'checks': []}} for i, event in enumerate(events)], 'wall_s': 10, 'agent_wall_s': 7, 'checks_wall_s': 3, 'preparation_wall_s': 2}
            write_json(run / 'result.json', metadata)
            row = export_run(run, 'test', {'source_commit': 'a' * 40}, root, root/'public', None, {'warm_brief_wall_s': 0.3, 'paid_brief_cost_usd': 0}, 'control')
            self.assertEqual(row['cost_usd_list_estimate'], 0.4)
            self.assertAlmostEqual(row['repair_cost_usd_list_estimate'], 0.15)
            self.assertEqual(row['final_model_usage_cumulative']['claude-test']['cacheReadInputTokens'], 250)
            self.assertEqual(row['total_wall_with_preparation_s'], 12)
            self.assertNotIn('PRIVATE', json.dumps(row))
            (run/'result.json').rename(run/'progress.json')
            partial = export_run(run, 'partial', {'source_commit': 'a' * 40}, root, root/'public', None, {'warm_brief_wall_s': 0.3, 'paid_brief_cost_usd': 0}, 'control')
            self.assertFalse(partial['accepted'])
            self.assertFalse(partial['cost_complete'])
            self.assertEqual(partial['cost_usd_list_estimate'], 0.4)
            (run/'final-candidate.json').write_text('{broken')
            retained = export_run(run, 'retained', {'source_commit': 'a' * 40}, root, root/'public', PatchBuilder.__new__(PatchBuilder), {'warm_brief_wall_s': 0.3, 'paid_brief_cost_usd': 0}, 'control')
            self.assertEqual(retained['cost_usd_list_estimate'], 0.4)
            self.assertTrue(retained['artifact_errors'])

    def test_missing_or_failed_runs_cannot_be_dropped(self):
        rows = [{'label': str(i), 'condition': 'control' if i % 2 == 0 else 'treatment', 'registered': True, 'final_result_available': False, 'accepted': False, 'completed': False} for i in range(8)]
        value = comparison(rows, {}, 'development')
        self.assertEqual(value['verdict'], 'pending')
        self.assertEqual(value['registered_runs'], 8)
        self.assertEqual(len(value['pairs']), 4)

    def test_four_pair_gate_uses_all_final_costs_and_acceptance(self):
        registration = {'efficiency_gate': {'heldout_control_accepted': 4, 'heldout_treatment_accepted': 4, 'minimum_median_cost_reduction_fraction': 0.2, 'minimum_cheaper_pairs': 3, 'maximum_median_wall_ratio': 1.1}, 'correctness_gate': {'heldout_control_accepted_max': 1, 'heldout_treatment_accepted_min': 3, 'maximum_median_cost_ratio': 1.1, 'maximum_median_wall_ratio': 1.1}}
        order = ['control', 'treatment', 'treatment', 'control', 'control', 'treatment', 'treatment', 'control']
        rows = [{'label': str(i), 'condition': condition, 'registered': True, 'final_result_available': True, 'completed': True, 'accepted': True, 'cost_complete': True, 'cost_with_preparation_usd_list_estimate': 1.0 if condition == 'control' else 0.7, 'total_wall_with_preparation_s': 100 if condition == 'control' else 105, 'common_input_sha256': 'a' * 64} for i, condition in enumerate(order)]
        development = {'comparison': {'development_gate_passed': True}}
        value = comparison(rows, registration, 'heldout', development)
        self.assertEqual(value['verdict'], 'efficiency_gate_passed')
        self.assertEqual(value['cheaper_pairs'], 4)
        self.assertAlmostEqual(value['median_cost_ratio'], 0.7)
        rows[1]['accepted'] = False
        self.assertEqual(comparison(rows, registration, 'heldout', development)['verdict'], 'no_clear_win')
        rows[1]['cost_complete'] = False
        self.assertEqual(comparison(rows, registration, 'heldout', development)['verdict'], 'pending')
        rows[1]['cost_complete'] = True
        kept_control = False
        for row in rows:
            if row['condition'] == 'control':
                row['accepted'] = not kept_control
                kept_control = True
        self.assertEqual(comparison(rows, registration, 'heldout', development)['verdict'], 'pending')
        safety = {'no_violations': True, 'reviewed_run_labels': [row['label'] for row in rows if row['condition'] == 'treatment']}
        self.assertEqual(comparison(rows, registration, 'heldout', development, safety)['verdict'], 'correctness_gate_passed')
        rows[1]['scope_failure'] = 'Instruction file changed.'
        self.assertEqual(comparison(rows, registration, 'heldout', development, safety)['verdict'], 'no_clear_win')

    def test_patch_records_missing_terminal_newline_and_empty_addition(self):
        builder = PatchBuilder.__new__(PatchBuilder)
        builder.roots = ['src']
        builder.cache = {'src/old.rs': b'old', 'src/new.rs': None}
        patch, binary = builder.build([{'path': 'src/old.rs', 'content': base64.b64encode(b'new').decode()}, {'path': 'src/new.rs', 'content': ''}])
        self.assertEqual(patch.count('\\ No newline at end of file'), 2)
        self.assertIn('new file mode 100644', patch)
        self.assertFalse(binary)


if __name__ == '__main__':
    main()
