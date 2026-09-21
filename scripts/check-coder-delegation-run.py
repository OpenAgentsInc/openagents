#!/usr/bin/env python3
"""Check a captured `coder -p --json` result and its ATIF trace.

An overseeing agent runs this before it accepts a delegation batch. Given
the one JSON object a headless turn wrote and the session log the turn
recorded, it answers whether the turn selected the expected program and
ran the expected number of recorded delegations, each completed with status
`answered` and an output matching its stated expectation:

    scripts/check-coder-delegation-run.py \
        --result result.json --trace run.atif.jsonl \
        --program burn-down --expected 6

A pass is execution evidence, not artifact correctness: it says the
delegations ran and answered, and it names the worktrees the run retained
for review. Whether an answer or an edit is right is a judgment this
check does not make, and no message text is ever read as evidence.

Exit 0 when the evidence holds, 1 when any part of it is missing,
malformed, or refused, 2 for a wrong command line. A captured run whose
top-level outcome is `answered` still refuses when its delegate calls did
not complete: the exit code the run took is the turn's own word, and this
check takes the trace's.
"""
import argparse
import json
import os
from pathlib import Path
import sys

# The call names and `extra` schemas crates/coder/src/trace.rs writes.
DELEGATE_CALL = 'delegate'
DELEGATE_SCHEMA = 'openagents.delegate-call.v1'
DECISION_SCHEMA = 'openagents.decision-call.v1'
PROGRAM_CALL = 'program'

# The enums as serde spells them: the ATIF Source and Outcome, and the
# delegate Isolation. The delegate Status an accepted call reports.
SOURCES = frozenset({'System', 'User', 'Agent'})
OUTCOMES = frozenset({'Completed', 'Failed', 'Cancelled'})
ISOLATIONS = frozenset({'directory', 'worktree'})
ANSWERED = 'answered'

# The nine keys headless.rs writes into every --json report.
RESULT_KEYS = frozenset({
    'reply', 'trace', 'outcome', 'route', 'program',
    'usage', 'error', 'cause', 'refusal',
})


class Refusal(Exception):
    """The evidence cannot be read at all; the message is the reason."""


def is_u64(value):
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def load_result(path):
    """Read the one JSON object `coder -p --json` wrote."""
    try:
        text = Path(path).read_text(encoding='utf-8')
    except (OSError, UnicodeError) as error:
        raise Refusal(f'result: cannot read {path}: {error}')
    try:
        result = json.loads(text)
    except json.JSONDecodeError as error:
        raise Refusal(f'result: {path} is not one JSON object ({error})')
    if not isinstance(result, dict):
        raise Refusal(f'result: {path} is not a JSON object')
    missing = sorted(RESULT_KEYS - result.keys())
    if missing:
        raise Refusal(f'result: missing keys {", ".join(missing)}')
    return result


def valid_session(session):
    if not isinstance(session, dict):
        return False
    if not all(isinstance(session.get(key), str)
               for key in ('id', 'model', 'door', 'repository', 'version')):
        return False
    if not all(isinstance(session[key], str)
               for key in ('directive', 'state') if key in session):
        return False
    return 'seconds' not in session or is_u64(session['seconds'])


def valid_call(call):
    if not isinstance(call, dict):
        return False
    if not isinstance(call.get('id'), str):
        return False
    if not isinstance(call.get('name'), str):
        return False
    if 'arguments' not in call:
        return False
    if not isinstance(call.get('output'), str):
        return False
    if call.get('outcome') not in OUTCOMES:
        return False
    if not is_u64(call.get('milliseconds')):
        return False
    if 'purpose' in call and not isinstance(call['purpose'], str):
        return False
    return 'extra' not in call or isinstance(call['extra'], dict)


def valid_step(step):
    if not isinstance(step, dict):
        return False
    if not is_u64(step.get('at')):
        return False
    if step.get('source') not in SOURCES:
        return False
    if not isinstance(step.get('message'), str):
        return False
    if not all(step[key] is None or isinstance(step[key], str)
               for key in ('reasoning', 'model') if key in step):
        return False
    if 'milliseconds' in step and not (
            step['milliseconds'] is None or is_u64(step['milliseconds'])):
        return False
    if 'tokens' in step and step['tokens'] is not None:
        tokens = step['tokens']
        if not (isinstance(tokens, list) and len(tokens) == 2
                and all(is_u64(each) for each in tokens)):
            return False
    if 'extensions' in step and not isinstance(step['extensions'], dict):
        return False
    return 'call' not in step or step['call'] is None or valid_call(step['call'])


def read_log(path):
    """Read one ATIF log the way `atif::log::read_whole` does: a fault on
    any line, or a missing `end` record, refuses the file rather than
    recovering a prefix."""
    try:
        raw = Path(path).read_bytes()
    except OSError as error:
        raise Refusal(f'trace: cannot read {path}: {error}')
    lines = raw.split(b'\n')
    torn = lines[-1] != b''
    if not torn:
        lines.pop()
    last = len(lines)
    faults = []
    session = None
    steps = []
    ended = False
    for number, line in enumerate(lines, 1):
        if torn and number == last:
            faults.append(f'line {number}: the last line was never finished')
            continue
        try:
            text = line.decode('utf-8')
        except UnicodeDecodeError:
            faults.append(f'line {number}: the line is not UTF-8')
            continue
        if not text.strip():
            continue
        try:
            record = json.loads(text)
        except json.JSONDecodeError:
            record = None
        if not isinstance(record, dict):
            faults.append(f'line {number}: the line is not a JSON object')
            continue
        kind = record.get('record')
        if kind not in ('session', 'step', 'end'):
            faults.append(f'line {number}: the record kind is unknown')
            continue
        if ended:
            which = 'a second end record' if kind == 'end' else 'a record after the end'
            faults.append(f'line {number}: {which}')
            continue
        if session is None and kind != 'session':
            faults.append(f'line {number}: a record before the session')
            continue
        if kind == 'session':
            if session is not None:
                faults.append(f'line {number}: a second session record')
            elif record.get('schema_version') != 'ATIF-v1.7':
                faults.append(f'line {number}: unsupported ATIF schema')
            elif not valid_session(record.get('session')):
                faults.append(f'line {number}: the session does not read')
            else:
                session = record['session']
        elif kind == 'step':
            step = record.get('step')
            if not valid_step(step):
                faults.append(f'line {number}: the step does not read')
            else:
                steps.append(step)
        else:
            if record.get('state') != 'ended' or not is_u64(record.get('at')):
                faults.append(f'line {number}: no completed end record')
            ended = True
    if faults:
        raise Refusal('trace: ' + '; '.join(faults))
    if session is None:
        raise Refusal(f'trace: {path} holds no session record')
    if not ended:
        raise Refusal(f'trace: {path} holds no end record: the session was interrupted')
    return session, steps


def check_result(result, program, trace_path):
    """What the captured report must show: the expected program selected,
    an answered outcome, and a trace path that is this trace's."""
    problems = []
    outcome = result['outcome']
    if outcome != 'answered':
        problems.append(f"the turn's outcome is {outcome!r}, not 'answered'")
    if result['program'] != program:
        problems.append(f"the turn ran program {result['program']!r}, not {program!r}")
    if result['route'] is not None:
        problems.append('a program turn takes no route, yet the result names one')
    if result['usage'] is not None:
        problems.append('a program turn reports no usage, yet the result carries some')
    for key in ('error', 'cause', 'refusal'):
        if result[key] is not None:
            problems.append(f'a finished turn carries no {key}, yet the result has one')
    if not isinstance(result['reply'], str) or not result['reply'].strip():
        problems.append('the result carries no reply')
    recorded = result['trace']
    if not isinstance(recorded, str) or not recorded:
        problems.append('the result names no trace')
    elif os.path.realpath(recorded) != os.path.realpath(trace_path):
        problems.append(f'the result names trace {recorded}, which is not {trace_path}')
    return problems


def valid_boundary(boundary):
    if not isinstance(boundary, dict):
        return False
    if not isinstance(boundary.get('backend'), str) or not boundary['backend']:
        return False
    if boundary.get('checkout') is not None and not isinstance(boundary['checkout'], str):
        return False
    return all(
        isinstance(boundary.get(key), list)
        and all(isinstance(path, str) for path in boundary[key])
        for key in ('writable', 'protected', 'sealed'))


def valid_relayed(relayed):
    if not isinstance(relayed, dict):
        return False
    if not all(isinstance(relayed.get(key), str)
               for key in ('relay', 'worker', 'request')):
        return False
    if relayed.get('model') is not None and not isinstance(relayed['model'], str):
        return False
    return is_u64(relayed.get('feedback'))


def delegation_problems(call):
    """What one recorded delegation must show: a completed call, an
    answered status, a nonempty output, no failed verdict, and a record of
    where it ran."""
    ident = call.get('id') or '?'
    problems = []
    extra = call['extra']
    for key in ('capability', 'executor_path', 'workdir'):
        if not isinstance(extra.get(key), str) or not extra[key]:
            problems.append(f'{ident}: {key} is missing or empty')
    concurrent = extra.get('concurrent_max')
    if not is_u64(concurrent) or not concurrent:
        problems.append(f'{ident}: concurrent_max is missing')
    # `wrote` records as null by contract: what a delegate changed is the
    # workspace snapshot's answer, not this call's.
    if extra.get('wrote', 'absent') is not None:
        problems.append(f'{ident}: wrote must be recorded as null')
    status = extra.get('status')
    if not isinstance(status, str):
        problems.append(f'{ident}: status is missing')
        status = ''
    arguments = call.get('arguments')
    if not (isinstance(arguments, dict)
            and isinstance(arguments.get('agent'), str)
            and arguments.get('isolation') in ISOLATIONS
            and isinstance(arguments.get('prompt'), str)
            and isinstance(arguments.get('bounds'), dict)):
        problems.append(f'{ident}: the delegation arguments do not read')
    for key in ('reads', 'expected', 'transcript', 'refusal'):
        if key in extra and not isinstance(extra[key], str):
            problems.append(f'{ident}: {key} is not a string')
    if 'exit_code' in extra and not (
            isinstance(extra['exit_code'], int)
            and not isinstance(extra['exit_code'], bool)):
        problems.append(f'{ident}: exit_code is not an integer')
    if 'retained' in extra and not (
            isinstance(extra['retained'], str) and extra['retained']):
        problems.append(f'{ident}: retained is not a path')
    if call['outcome'] != 'Completed':
        problems.append(f"{ident}: outcome is {call['outcome']}, not Completed")
    if status != ANSWERED:
        problems.append(f'{ident}: status is {status!r}, not {ANSWERED!r}')
    if not call['output'].strip():
        problems.append(f'{ident}: the output is empty')
    if status == ANSWERED and ('refusal' in extra or 'exit_code' in extra):
        problems.append(f'{ident}: an answered delegation carries a refusal or exit code')
    if ('expected' in extra) != ('correct' in extra):
        problems.append(f'{ident}: expected and correct must appear together')
    expected = extra.get('expected')
    if not isinstance(expected, str) or not expected.strip():
        problems.append(f'{ident}: unverifiable — no nonempty expectation')
    elif ' '.join(call['output'].split()).lower() != ' '.join(expected.split()).lower():
        problems.append(f'{ident}: output does not match the stated expectation')
    if 'correct' in extra:
        if not isinstance(extra['correct'], bool):
            problems.append(f'{ident}: correct is not a boolean')
        elif not extra['correct']:
            problems.append(f'{ident}: the completion verdict failed')
    ran = call['outcome'] == 'Completed' and status == ANSWERED
    boundary, relayed = extra.get('boundary'), extra.get('relayed')
    if ran and boundary is None and relayed is None:
        problems.append(f'{ident}: unverifiable — no boundary and no relay record')
    if boundary is not None and not valid_boundary(boundary):
        problems.append(f'{ident}: the boundary record does not read')
    if relayed is not None and not valid_relayed(relayed):
        problems.append(f'{ident}: the relay record does not read')
    return problems


def check_trace(steps, program, expected):
    """What the trace must show: the program selected, exactly the expected
    number of real delegate calls, and each one completed and answered."""
    problems = []
    retained = []
    calls = [step['call'] for step in steps if step.get('call') is not None]
    if len({call['id'] for call in calls}) != len(calls):
        problems.append('the trace repeats a call ID')
    choices = []
    program_calls = 0
    for call in calls:
        if call['name'] != PROGRAM_CALL:
            continue
        program_calls += 1
        if call['outcome'] != 'Completed':
            problems.append('the program selection call did not complete')
        extra = call.get('extra', {})
        if extra.get('schema') != DECISION_SCHEMA:
            problems.append(
                f"{call['id']}: named {PROGRAM_CALL} but carries no {DECISION_SCHEMA} record")
            continue
        answers = extra.get('answers')
        answer = answers.get('program') if isinstance(answers, dict) else None
        if isinstance(answer, dict) and answer.get('type') == 'choice':
            choices.append(answer.get('choice'))
    if program_calls != 1:
        problems.append('the trace must hold exactly one program selection')
    if not choices:
        problems.append('the trace records no program selection')
    elif program not in choices:
        problems.append(f'the trace selected {choices[-1]!r}, not {program!r}')
    real = []
    for call in calls:
        if call['name'] != DELEGATE_CALL:
            continue
        extra = call.get('extra')
        if isinstance(extra, dict) and extra.get('schema') == DELEGATE_SCHEMA:
            real.append(call)
        else:
            problems.append(
                f"{call['id']}: named {DELEGATE_CALL} but carries no {DELEGATE_SCHEMA} record")
    if len(real) != expected:
        problems.append(f'expected {expected} delegate calls; the trace holds {len(real)}')
    for call in real:
        problems.extend(delegation_problems(call))
        kept = call['extra'].get('retained')
        if isinstance(kept, str) and kept:
            retained.append(kept)
    return problems, retained


def main(argv=None):
    parser = argparse.ArgumentParser(
        description='Check a captured coder -p --json result and its ATIF trace '
                    'before accepting a delegation batch.')
    parser.add_argument('--result', required=True,
                        help='the file the --json report was captured in')
    parser.add_argument('--trace', required=True,
                        help='the ATIF session log the turn wrote')
    parser.add_argument('--program', required=True,
                        help='the program slug the run had to select')
    parser.add_argument('--expected', type=int, required=True,
                        help='the number of delegate calls the run had to complete')
    args = parser.parse_args(argv)
    if args.expected < 1:
        parser.error('--expected must be at least 1')

    problems = []
    result = steps = None
    try:
        result = load_result(args.result)
    except Refusal as refused:
        problems.append(str(refused))
    try:
        _, steps = read_log(args.trace)
    except Refusal as refused:
        problems.append(str(refused))
    if result is not None:
        problems.extend(check_result(result, args.program, args.trace))
    retained = []
    if steps is not None:
        found, retained = check_trace(steps, args.program, args.expected)
        problems.extend(found)
    if problems:
        for problem in problems:
            print(f'refused: {problem}', file=sys.stderr)
        return 1
    print(f'pass: outcome=answered program={args.program} '
          f'delegates={args.expected} trace={args.trace}')
    for path in retained:
        print(f'retained worktree: {path}')
    print('scope: execution evidence only — the run selected the program and '
          'every delegation completed with an answer; whether the answers or '
          'the retained worktrees are correct is a separate judgment')
    return 0


if __name__ == '__main__':
    sys.exit(main())
