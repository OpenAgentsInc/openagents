#!/usr/bin/env python3
"""Describe retained native tool activity without executing or printing commands."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import shlex
import time
import uuid

MAX_BYTES = 128 * 1024 * 1024
MAX_LINE = 4 * 1024 * 1024
TOOLS = frozenset(('Bash', 'Read', 'Edit', 'Write', 'Glob', 'Grep', 'Task', 'Agent',
    'TaskOutput', 'TaskStop', 'WebFetch', 'WebSearch', 'TodoWrite', 'NotebookEdit',
    'Skill', 'ToolSearch', 'AskUserQuestion', 'ExitPlanMode', 'EnterPlanMode',
    'SendMessage', 'TaskCreate', 'TaskUpdate', 'TaskList', 'TaskGet',
    'ReadMcpResource', 'ListMcpResources'))
ASSIGNMENT = re.compile(r'^[A-Za-z_][A-Za-z0-9_]*=')


def object_value(value):
    return value if isinstance(value, dict) else {}


def identity(value):
    return value if isinstance(value, str) and 0 < len(value) <= 512 else None


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def shell_segments(command):
    """Split simple shell lists without treating quoted operators as syntax."""
    parts, current = [], []
    quote = None; escaped = False; comment = False
    for offset, character in enumerate(command):
        if comment:
            if character != '\n': continue
            comment = False
        if escaped:
            current.append(character); escaped = False; continue
        if character == '\\' and quote != "'":
            current.append(character); escaped = True; continue
        if quote:
            current.append(character)
            if character == quote: quote = None
        elif character in ("'", '"'):
            quote = character; current.append(character)
        elif character == '#' and (not current or current[-1].isspace()):
            comment = True
        elif character == '&' and ((current and current[-1] in '<>') or command[offset + 1:offset + 2] == '>'):
            current.append(character)
        elif character in ';|&\n':
            if current: parts.append(''.join(current)); current = []
        elif character in '()':
            raise ValueError('Unsupported shell grouping')
        else:
            current.append(character)
    if quote or escaped: raise ValueError('Incomplete shell token')
    if current: parts.append(''.join(current))
    tokens = []
    for part in parts:
        lexer = shlex.shlex(part, posix=True, punctuation_chars='<>')
        lexer.whitespace_split = True
        tokens.append(list(lexer))
    return tokens


def command_requests(command, depth=0):
    """Classify literal command positions, not shell execution or test outcomes.

    This deliberately abstains on heredocs, substitutions, loops, and eval.
    Shell wrappers are parsed only when they carry a literal -c argument.
    """
    if depth > 2 or any(s in command for s in ('<<', '$(', '`', '<(', '>(')):
        return Counter(), False
    try:
        segments = shell_segments(command)
    except ValueError:
        return Counter(), False
    counts = Counter(); complete = True
    for args in segments:
        while args and ASSIGNMENT.match(args[0]): args = args[1:]
        if not args: continue
        if args[0] in ('for', 'while', 'until', 'if', 'then', 'do', 'case', 'function', 'eval', '{', '}'):
            return Counter(), False
        # Common literal wrappers. Unknown option forms remain unclassified.
        if Path(args[0]).name == 'env':
            args = args[1:]
            while args and (ASSIGNMENT.match(args[0]) or args[0] in ('-i', '--ignore-environment', '--')):
                args = args[1:]
            if args and args[0].startswith('-'):
                complete = False; continue
        if args and Path(args[0]).name == 'timeout':
            args = args[1:]
            if args and re.fullmatch(r'[0-9]+(?:\.[0-9]+)?[smhd]?', args[0]): args = args[1:]
            else:
                complete = False; continue
        while args and args[0] in ('command', 'time'): args = args[1:]
        if not args: continue
        exe = Path(args[0]).name
        if '$' in args[0]:
            complete = False; continue
        if exe in ('bash', 'sh', 'zsh'):
            if len(args) >= 3 and args[1] in ('-c', '-lc', '-cl'):
                nested, understood = command_requests(args[2], depth + 1)
                counts.update(nested); complete &= understood
            else:
                complete = False
            continue
        counts['literal_command_positions'] += 1
        if exe == 'cargo':
            tail = args[1:]
            if tail and tail[0].startswith('+'): tail = tail[1:]
            subcommand = tail[0] if tail else ''
            name = subcommand if subcommand in ('test', 'check', 'fmt', 'clippy', 'build') else 'other'
            counts['cargo_' + name] += 1
        elif exe == 'rustfmt': counts['rustfmt'] += 1
        elif exe in ('cat', 'head', 'tail', 'less', 'wc'): counts['shell_read'] += 1
        elif exe in ('rg', 'grep', 'find', 'fd'): counts['shell_search'] += 1
        elif exe in ('apply_patch', 'patch'): counts['shell_edit'] += 1
        elif exe == 'sed':
            counts['shell_edit' if any(a == '--in-place' or a.startswith('-i') for a in args[1:]) else 'shell_read'] += 1
        elif exe in ('python', 'python3', 'perl', 'ruby', 'node') or exe.endswith('.sh'):
            counts['opaque_script'] += 1
        else: counts['other_command'] += 1
    return counts, complete


def summarize(path):
    """Read one stream; output contains no messages, commands, paths, or results."""
    started = time.monotonic(); path = Path(path)
    output = {'schema': 'openagents.jev-lifecycle.native-trace-summary.v1', 'available': False,
              'status': 'missing', 'source_sha256': None, 'bytes_scanned': 0, 'parse_errors': {}}
    if not path.exists(): return output
    if path.is_symlink() or not path.is_file():
        output['status'] = 'invalid_file'; return output
    messages, fallback, tools, results, terminal = {}, set(), {}, {}, []
    errors = Counter(); events = Counter(); assistant_records = 0; unknown_messages = 0
    stream_hash = hashlib.sha256(); consumed = 0; complete = True
    try:
        with path.open('rb') as handle:
            while raw := handle.readline(MAX_LINE + 1):
                if len(raw) > MAX_LINE or consumed + len(raw) > MAX_BYTES:
                    errors['byte_bound'] += 1; complete = False; break
                consumed += len(raw); stream_hash.update(raw)
                if not raw.endswith(b'\n'):
                    errors['unterminated_final_record'] += 1
                try:
                    event = json.loads(raw)
                    if not isinstance(event, dict): raise ValueError('Event must be an object')
                except (ValueError, UnicodeError):
                    errors['malformed_record'] += 1; continue
                kind = event.get('type')
                events[kind if kind in ('assistant', 'user', 'system', 'result', 'stream_event') else 'other'] += 1
                lane = 'child' if identity(event.get('parent_tool_use_id')) else 'root'
                if kind == 'result' and lane == 'root':
                    number = event.get('num_turns')
                    terminal.append(number if type(number) is int and number >= 0 else None)
                message = object_value(event.get('message'))
                content = message.get('content', [])
                if kind == 'assistant':
                    assistant_records += 1
                    mid = identity(message.get('id'))
                    if mid:
                        if mid in messages and messages[mid] != lane: errors['assistant_lane_conflict'] += 1
                        messages.setdefault(mid, lane)
                    elif identity(event.get('uuid')):
                        fallback.add(event['uuid'])
                        errors['assistant_message_id_missing'] += 1
                    else:
                        unknown_messages += 1; errors['assistant_identity_missing'] += 1
                    if not isinstance(content, list):
                        errors['assistant_content_invalid'] += 1; continue
                    for index, block in enumerate(content):
                        if not isinstance(block, dict) or block.get('type') != 'tool_use': continue
                        tid = identity(block.get('id'))
                        if not tid:
                            # The occurrence is visible but cannot be reliably deduplicated.
                            errors['tool_identity_missing'] += 1
                            tid = ('anonymous', mid or identity(event.get('uuid')) or assistant_records, index)
                        raw_name = block.get('name')
                        name = raw_name if isinstance(raw_name, str) and raw_name in TOOLS else 'Other'
                        arguments = object_value(block.get('input'))
                        command = arguments.get('command') if name == 'Bash' else None
                        command = command if isinstance(command, str) else None
                        row = {'name': name, 'lane': lane, 'command': command, 'input_conflict': False}
                        if tid in tools:
                            old = tools[tid]
                            if old['name'] != name or old['lane'] != lane:
                                errors['tool_identity_conflict'] += 1; old['input_conflict'] = True
                            if old['command'] is None: old['command'] = command
                            elif command is not None and old['command'] != command:
                                errors['tool_input_conflict'] += 1; old['input_conflict'] = True
                        else:
                            row['ordinal'] = len(tools) + 1; tools[tid] = row
                elif kind == 'user' and isinstance(content, list):
                    for block in content:
                        if not isinstance(block, dict) or block.get('type') != 'tool_result': continue
                        tid = identity(block.get('tool_use_id'))
                        if not tid:
                            errors['tool_result_identity_missing'] += 1; continue
                        # Omitted is_error means ordinary success in the native stream.
                        value = block.get('is_error', False)
                        if type(value) is not bool:
                            errors['tool_result_error_flag_invalid'] += 1; continue
                        results.setdefault(tid, set()).add(value)
    except OSError:
        errors['stream_read_error'] += 1; complete = False
    lanes = {}
    commands = {}; categories = Counter(); calls_by_category = Counter(); classified = 0
    for lane in ('root', 'child'):
        selected = [tool for tool in tools.values() if tool['lane'] == lane]
        names = Counter(tool['name'] for tool in selected)
        lanes[lane] = {'unique_assistant_message_ids': sum(v == lane for v in messages.values()),
                       'tool_use_occurrences': len(selected), 'tool_names': dict(sorted(names.items())),
                       'direct_read_tools': names['Read'], 'direct_search_tools': names['Glob'] + names['Grep'],
                       'direct_edit_tools': names['Edit'] + names['Write'] + names['NotebookEdit'],
                       'delegation_tools': names['Task'] + names['Agent']}
    for tid, tool in tools.items():
        if tool['name'] != 'Bash': continue
        command = tool['command']
        if command is None or tool['input_conflict']:
            errors['bash_command_unavailable'] += 1; continue
        requested, understood = command_requests(command)
        categories.update(requested); calls_by_category.update(requested.keys())
        if understood: classified += 1
        key = digest(command.encode())
        group = commands.setdefault(key, {'sha256': key, 'utf8_bytes': len(command.encode()), 'count': 0,
            'tool_ordinals': [], 'observed_error_results': 0, 'requested_categories': sorted(requested),
            'classification_complete': understood})
        group['count'] += 1; group['tool_ordinals'].append(tool['ordinal'])
        group['observed_error_results'] += results.get(tid) == {True}
    repeats = [value for value in commands.values() if value['count'] > 1]
    conflicts = sum(len(states) > 1 for states in results.values())
    if conflicts: errors['tool_result_conflict'] += conflicts
    if events['stream_event'] and not assistant_records:
        errors['only_stream_deltas_without_assistant_records'] += 1
    output.update(available=True, status='complete' if complete and not errors else 'partial',
        source_sha256=stream_hash.hexdigest() if complete else None, scanned_prefix_sha256=stream_hash.hexdigest(),
        bytes_scanned=consumed, event_counts=dict(sorted(events.items())), assistant_records=assistant_records,
        unique_assistant_message_ids=len(messages), fallback_assistant_event_uuids=len(fallback),
        assistant_records_without_identity=unknown_messages,
        assistant_count_has_all_message_ids=not unknown_messages and not fallback,
        tool_use_occurrences=len(tools), tool_counts_have_all_ids=not errors['tool_identity_missing'], lanes=lanes,
        tool_results={'observed_ids': len(results), 'matching_tool_ids': sum(t in tools for t in results),
                      'error_ids': sum(v == {True} for v in results.values()),
                      'tool_uses_without_result': sum(t not in results for t in tools)},
        bash={'calls': sum(v['name'] == 'Bash' for v in tools.values()), 'classified_calls': classified,
              'unclassified_calls': sum(v['name'] == 'Bash' for v in tools.values()) - classified,
              'literal_requested_actions': dict(sorted(categories.items())), 'calls_by_requested_category': dict(sorted(calls_by_category.items())),
              'unique_exact_payloads': len(commands), 'repeated_exact_payload_groups': sorted(repeats, key=lambda x: x['tool_ordinals'][0]),
              'repeat_occurrences_after_first': sum(v['count'] - 1 for v in repeats)},
        cli_reported_num_turns=terminal[0] if terminal and terminal[0] is not None and all(t == terminal[0] for t in terminal) else None,
        root_result_records=len(terminal), parse_errors={k: v for k, v in sorted(errors.items()) if v},
        analysis_wall_s=time.monotonic() - started,
        limits=['Completeness describes parsing, not session completion or patch acceptance.',
                'Assistant message IDs and CLI-reported turns have different definitions.',
                'Counts cover visible stream records; child activity may be absent from the parent stream.',
                'Stream deltas are not counted in addition to complete assistant records.',
                'Shell actions are literal requests, not proof of execution, completion, or passing tests. Conditional chains may stop early.',
                'One Bash call may request multiple actions; unsupported shell forms and script internals remain unclassified.',
                'Exact repeated commands can be justified by edits, retries, or changed state; they are not a waste estimate.',
                'This summary attributes no billed tokens, cost, or elapsed runtime to individual tools.'])
    return output


def build(plan_path, runs):
    """Summarize scheduled streams only, retaining missing attempts explicitly."""
    plan_path, runs = Path(plan_path), Path(runs)
    raw = plan_path.read_bytes(); plan = json.loads(raw)
    rows = []
    for entry in plan['schedule']:
        run_id = entry['run_id']
        if not isinstance(run_id, str) or str(uuid.UUID(run_id)) != run_id: raise ValueError('Invalid run UUID')
        rows.append({**{k: entry[k] for k in ('position', 'run_id', 'task_id', 'repetition', 'arm')},
                     'trace': summarize(runs / run_id / 'native/events.jsonl')})
    return {'schema': 'openagents.jev-lifecycle.native-panel-traces.v1', 'plan_sha256': digest(raw),
            'scheduled_attempts': len(rows), 'available_traces': sum(r['trace']['available'] for r in rows),
            'rows': rows, 'interpretation': 'Descriptive activity evidence. Missing or partial traces do not count as zero activity.'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--trace', type=Path)
    source.add_argument('--plan', type=Path)
    parser.add_argument('--runs', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.plan and args.runs is None: parser.error('--plan requires --runs')
    result = summarize(args.trace) if args.trace else build(args.plan, args.runs)
    with args.output.open('x') as target: target.write(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'status': result.get('status'), 'available_traces': result.get('available_traces')}))
