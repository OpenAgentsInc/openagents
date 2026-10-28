#!/usr/bin/env python3
"""Optional, bounded valid-program differential testing; Python standard library only."""

import argparse
from dataclasses import asdict, dataclass, replace
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
GENERATOR = "coax-small-state-v1"
EVALUATOR_STEPS = 1000000
KINDS = ("arith", "branch", "compound", "assign", "aggregate", "defer")
OPERATORS = ("+", "-", "*", "/", "%", "&", "|", "^")
PROFILE = {"integer_arithmetic": "checked", "integer_type": "int (signed 64-bit)",
           "optimization": ["-O0", "-O2"], "emission": "coax c without --release",
           "execution": "evaluator #test and separate native main", "instrumentation": "none"}
DEFECT = "checked-multiply-as-add"


@dataclass(frozen=True)
class Step:
    kind: str
    op: str = "+"
    value: int = 1
    index: int = 0
    other: int = 0
    pivot: int = 0


@dataclass(frozen=True)
class Program:
    seed: int
    case_index: int
    steps: tuple
    inputs: tuple


class Random:
    """Pinned SplitMix64; generation does not depend on Python's random module."""
    def __init__(self, seed):
        self.state = seed & ((1 << 64) - 1)

    def integer(self, low, high):
        mask = (1 << 64) - 1
        self.state = (self.state + 0x9E3779B97F4A7C15) & mask
        value = self.state
        value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & mask
        value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & mask
        value ^= value >> 31
        return low + value % (high - low + 1)


def generate(seed, case_index, steps):
    random = Random(seed ^ ((case_index + 1) * 0xD1342543DE82EF95))
    operations = []
    for position in range(steps):
        kind = KINDS[(position + case_index) % len(KINDS)]
        op = OPERATORS[random.integer(0, len(OPERATORS) - 1)]
        value = random.integer(-7, 7)
        if op in ("/", "%") and value == 0:
            value = 1
        operations.append(Step(kind, op, value, random.integer(0, 3),
                               random.integer(0, 3), random.integer(-32, 32)))
    inputs = ((-8, -3), (0, 0), (7, 5),
              (random.integer(-12, 12), random.integer(-12, 12)))
    return Program(seed, case_index, tuple(operations), inputs)


def quotient(left, right):
    magnitude = abs(left) // abs(right)
    return -magnitude if (left < 0) != (right < 0) else magnitude


def remainder(left, right):
    return left - quotient(left, right) * right


def oracle(program, inputs):
    """Direct integer/state model, independent of emitted C and compiler evaluation."""
    a, b = inputs
    total = a * 17 + b * 3
    lanes = [0] * 4
    buffer = [a, b, -a, -b]
    counter = 0
    deferred = []

    def index(offset):
        nonlocal counter
        counter += 1
        return (counter + offset) % 4

    for step in program.steps:
        if step.kind == "arith":
            right = step.value
            if step.op == "+":
                total += right
            elif step.op == "-":
                total -= right
            elif step.op == "*":
                total *= right
            elif step.op == "/":
                total = quotient(total, right)
            elif step.op == "%":
                total = remainder(total, right)
            elif step.op == "&":
                total &= right
            elif step.op == "|":
                total |= right
            elif step.op == "^":
                total ^= right
            else:
                raise ValueError("unsupported arithmetic operator")
            total = remainder(total, 97)
        elif step.kind == "branch":
            total += step.value if total < step.pivot else -step.value
        elif step.kind == "compound":
            buffer[index(step.index)] += total
        elif step.kind == "assign":
            left = index(step.index)
            right = index(step.other)
            buffer[left] = buffer[right] + total
        elif step.kind == "aggregate":
            left = index(step.index)
            right = index(step.other)
            lanes[left] = buffer[right] + total
        elif step.kind == "defer":
            deferred.append(step)
        else:
            raise ValueError("unsupported operation")
    returned = [total, *lanes]
    for step in reversed(deferred):
        buffer[index(step.index)] += step.value
    return [*returned, 701, *buffer, -907, counter]


def render(program):
    lines = ['#import "Basic";',
             'DiffState :: struct { total: int; lanes: [4] int; }',
             'index :: (counter: [] int, offset: int) -> int {',
             '  counter[0] += 1; return (counter[0] + offset) % 4;', '}',
             'probe :: (a: int, b: int, out: [] int, counter: [] int) -> DiffState {',
             '  state: DiffState; state.total = a * 17 + b * 3;']
    for step in program.steps:
        left = f'index(counter, {step.index})'
        right = f'index(counter, {step.other})'
        if step.kind == 'arith':
            lines.append(f'  state.total = (state.total {step.op} ({step.value})) % 97;')
        elif step.kind == 'branch':
            lines.append(f'  if state.total < {step.pivot} {{ state.total += {step.value}; }}'
                         f' else {{ state.total -= {step.value}; }}')
        elif step.kind == 'compound':
            lines.append(f'  out[{left}] += state.total;')
        elif step.kind == 'assign':
            lines.append(f'  out[{left}] = out[{right}] + state.total;')
        elif step.kind == 'aggregate':
            lines.append(f'  state.lanes[{left}] = out[{right}] + state.total;')
        elif step.kind == 'defer':
            lines.append(f'  defer {{ out[{left}] += {step.value}; }}')
        else:
            raise ValueError('unsupported operation')
    lines.extend(['  return state;', '}'])

    def invocation(number, inputs):
        a, b = inputs
        return [f'  buffer{number} := [6] int.[701, {a}, {b}, {-a}, {-b}, -907];',
                f'  counter{number}: [1] int;',
                f'  result{number} := probe({a}, {b}, buffer{number}[1..<5], counter{number}[..]);']

    def observations(number):
        return [f'result{number}.total', *[f'result{number}.lanes[{i}]' for i in range(4)],
                *[f'buffer{number}[{i}]' for i in range(6)], f'counter{number}[0]']

    for number, inputs in enumerate(program.inputs):
        lines.append(f'#test "oracle {number}" {{')
        lines.extend(invocation(number, inputs))
        for expression, expected in zip(observations(number), oracle(program, inputs)):
            lines.append(f'  #assert {expression} == {expected};')
        lines.append('}')
    lines.append('main :: () {')
    for number, inputs in enumerate(program.inputs):
        lines.extend(invocation(number, inputs))
        lines.append('  print("' + ' '.join(['%'] * 12) + '\\n", '
                     + ', '.join(observations(number)) + ');')
    return '\n'.join([*lines, '}', ''])


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def execute(command, timeout):
    record = {"command": [str(item) for item in command]}
    try:
        result = subprocess.run(record["command"], capture_output=True, text=True, timeout=timeout)
        record.update(returncode=result.returncode, stdout=result.stdout, stderr=result.stderr,
                      outcome="passed" if result.returncode == 0 else "infrastructure_error")
    except subprocess.TimeoutExpired as error:
        def decoded(value):
            return value.decode(errors="replace") if isinstance(value, bytes) else value or ""
        record.update(returncode=None, stdout=decoded(error.stdout), stderr=decoded(error.stderr),
                      outcome="inconclusive", reason="execution timeout", timeout_seconds=timeout)
    except OSError as error:
        record.update(returncode=None, stdout="", stderr=str(error), outcome="infrastructure_error")
    return record


def combined_outcome(outcomes):
    # A concrete mismatch remains evidence even if a separate profile could not run.
    for outcome in ("counterexample", "infrastructure_error", "inconclusive", "not_checked"):
        if outcome in outcomes:
            return outcome
    return "passed"


def seeded_c(source):
    before = "if (__builtin_mul_overflow(a, b, &r))"
    after = "if (__builtin_add_overflow(a, b, &r))"
    if source.count(before) != 1:
        raise ValueError("seeded defect requires exactly one checked multiply helper template")
    return source.replace(before, after, 1)


def run_case(program, directory, compiler, cc, timeout, seed_defect=False):
    directory.mkdir(parents=True, exist_ok=True)
    source = directory / 'source.coax'
    source.write_text(render(program))
    write_json(directory / 'program.json', {"schema_version": SCHEMA_VERSION,
                                           "generator": GENERATOR, "program": asdict(program)})
    expected = [oracle(program, inputs) for inputs in program.inputs]
    evaluator = execute([compiler, 'test', source, '--json', '--steps', str(EVALUATOR_STEPS)], timeout)
    if evaluator['returncode'] == 1 and '"E0406"' in evaluator['stderr']:
        evaluator['outcome'] = 'counterexample'
    elif evaluator['returncode'] == 1 and '"E0400"' in evaluator['stderr']:
        evaluator['outcome'] = 'inconclusive'
    emission = execute([compiler, 'c', source], timeout)
    result = {"schema_version": SCHEMA_VERSION, "expected": expected,
              "source_sha256": digest(source), "evaluator": evaluator,
              "emission": emission, "native": [], "seeded_defect": DEFECT if seed_defect else None}
    if emission['outcome'] == 'passed':
        emitted = directory / 'emitted.c'
        emitted.write_text(emission.pop('stdout'))
        result['emitted_sha256'] = digest(emitted)
        if seed_defect:
            try:
                broken = seeded_c(emitted.read_text())
                emitted = directory / 'emitted-seeded.c'
                emitted.write_text(broken)
                result['seeded_emitted_sha256'] = digest(emitted)
            except ValueError as error:
                emission.update(outcome='infrastructure_error', reason=str(error))
        if emission['outcome'] == 'passed':
            for optimization in ('-O0', '-O2'):
                binary = directory / ('native-' + optimization[1:])
                compilation = execute([*cc, '-std=c11', '-pedantic-errors', optimization,
                                       emitted, '-lm', '-o', binary], timeout)
                native = {"optimization": optimization, "compilation": compilation,
                          "outcome": compilation['outcome']}
                if compilation['outcome'] == 'passed':
                    native['binary_sha256'] = digest(binary)
                    execution = execute([binary], timeout)
                    native['execution'] = execution
                    native['outcome'] = execution['outcome']
                    if execution['returncode'] is not None:
                        if execution['returncode'] != 0:
                            native.update(outcome='counterexample', mismatch='nonzero_exit')
                        else:
                            try:
                                observed = [[int(word) for word in row.split()]
                                            for row in execution['stdout'].splitlines()]
                            except ValueError:
                                observed = None
                            native['observed'] = observed
                            if observed != expected:
                                native.update(outcome='counterexample', mismatch='values')
                result['native'].append(native)
    if not result['native']:
        result['native'] = [{"optimization": opt, "outcome": "not_checked",
                             "reason": "C emission unavailable"} for opt in ('-O0', '-O2')]
    result['outcome'] = combined_outcome([evaluator['outcome'], emission['outcome'],
                                        *[native['outcome'] for native in result['native']]])
    write_json(directory / 'result.json', result)
    return result


def mismatch_signature(result):
    if result['evaluator']['outcome'] == 'counterexample':
        return ('evaluator', 'assertion')
    for native in result['native']:
        if native['outcome'] == 'counterexample':
            return (native['optimization'], native['mismatch'])
    return None


def minimize(program, predicate, budget):
    """Grammar-preserving deletion and input shrinking, with a strict execution budget."""
    attempts = 0
    exhausted = False

    def accepts(candidate):
        nonlocal attempts, exhausted
        if attempts >= budget:
            exhausted = True
            return False
        attempts += 1
        return predicate(candidate)

    original_steps, original_inputs = len(program.steps), len(program.inputs)
    for field, minimum in (('steps', 0), ('inputs', 1)):
        chunk = max(1, len(getattr(program, field)))
        while chunk:
            position = 0
            while position < len(getattr(program, field)) and not exhausted:
                values = getattr(program, field)
                candidate_values = values[:position] + values[position + chunk:]
                if len(candidate_values) >= minimum:
                    candidate = replace(program, **{field: candidate_values})
                    if accepts(candidate):
                        program = candidate
                        continue
                position += chunk
            if exhausted:
                break
            chunk = chunk // 2
    # Simpler scalar values cannot enlarge the generated arithmetic domain.
    for row in range(len(program.inputs)):
        for column in range(2):
            value = program.inputs[row][column]
            for simpler in (0, 1 if value > 0 else -1):
                if abs(simpler) >= abs(value) or exhausted:
                    continue
                inputs = list(program.inputs)
                pair = list(inputs[row])
                pair[column] = simpler
                inputs[row] = tuple(pair)
                candidate = replace(program, inputs=tuple(inputs))
                if accepts(candidate):
                    program, value = candidate, simpler
    return program, {"attempts": attempts, "budget": budget, "budget_exhausted": exhausted,
                     "original_steps": original_steps, "remaining_steps": len(program.steps),
                     "original_inputs": original_inputs, "remaining_inputs": len(program.inputs),
                     "claim": "bounded grammar reduction; global minimality is not claimed"}


def load_program(path):
    record = json.loads(path.read_text())
    if record.get('schema_version') != SCHEMA_VERSION or record.get('generator') != GENERATOR:
        raise ValueError('unsupported program schema or generator')
    value = record['program']
    steps = tuple(Step(**step) for step in value['steps'])
    inputs = tuple(tuple(row) for row in value['inputs'])
    if not 0 <= len(steps) <= 64 or not 1 <= len(inputs) <= 4:
        raise ValueError('program exceeds bounded subset')
    for row in inputs:
        if len(row) != 2 or any(type(item) is not int or abs(item) > 12 for item in row):
            raise ValueError('invalid input vector')
    for step in steps:
        if step.kind not in KINDS or step.op not in OPERATORS:
            raise ValueError('unsupported operation')
        if any(type(item) is not int for item in (step.value, step.index, step.other, step.pivot)):
            raise ValueError('invalid operation fields')
        if abs(step.value) > 7 or not 0 <= step.index <= 3 or not 0 <= step.other <= 3 or abs(step.pivot) > 32:
            raise ValueError('operation exceeds bounded subset')
        if step.kind == 'arith' and step.op in ('/', '%') and step.value == 0:
            raise ValueError('zero divisor')
    return Program(value['seed'], value['case_index'], steps, inputs)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--coax', type=Path, default=ROOT / 'bin/coax')
    parser.add_argument('--cc', default=os.environ.get('CC', 'cc'), help='C compiler command')
    parser.add_argument('--output', type=Path, required=True, help='new directory for retained evidence')
    parser.add_argument('--seed', type=lambda value: int(value, 0), default=20261028)
    parser.add_argument('--cases', type=int, default=16)
    parser.add_argument('--steps', type=int, default=12)
    parser.add_argument('--timeout', type=float, default=30)
    parser.add_argument('--reduce-budget', type=int, default=64)
    parser.add_argument('--seed-defect', action='store_true', help='deliberately corrupt emitted checked multiply')
    parser.add_argument('--replay', type=Path, help='retained program.json instead of generation')
    args = parser.parse_args(argv)
    if not (0 <= args.seed < 1 << 64 and 1 <= args.cases <= 256 and 1 <= args.steps <= 64
            and 0 < args.timeout <= 120 and 0 <= args.reduce_budget <= 512):
        parser.error('seed/cases/steps/timeout/reduction budget outside documented bounds')
    if args.output.exists():
        parser.error('output directory already exists; choose a new path to preserve evidence')
    compiler = args.coax.resolve()
    try:
        cc = shlex.split(args.cc)
    except ValueError as error:
        parser.error(str(error))
    if not cc:
        parser.error('empty C compiler command')
    cc[0] = shutil.which(cc[0]) or cc[0]
    try:
        programs = [load_program(args.replay)] if args.replay else [
            generate(args.seed, index, args.steps) for index in range(args.cases)]
    except (KeyError, TypeError, ValueError, OSError) as error:
        parser.error(str(error))
    args.output.mkdir(parents=True)
    output = args.output.resolve()
    version = execute([compiler, 'version'], args.timeout)
    toolchain = execute([*cc, '--version'], args.timeout)
    target = execute([*cc, '-dumpmachine'], args.timeout)
    report = {"schema_version": SCHEMA_VERSION, "generator": GENERATOR,
              "tool_sha256": digest(__file__), "python_version": platform.python_version(),
              "compiler": {"path": str(compiler), "version": version,
                           "sha256": digest(compiler) if compiler.is_file() else None},
              "toolchain": {"version": toolchain, "target": target,
                            "driver_sha256": digest(cc[0]) if Path(cc[0]).is_file() else None},
              "host": {"system": platform.system(), "machine": platform.machine()},
              "semantic_profile": PROFILE, "source_dependencies": ['Basic (built into compiler)'],
              "bounds": {"cases": len(programs),
                         "steps_per_case": max(len(program.steps) for program in programs),
                         "inputs_per_case": max(len(program.inputs) for program in programs),
                         "input_magnitude": 12,
                         "evaluator_steps_per_check": EVALUATOR_STEPS,
                         "timeout_seconds_per_process": args.timeout,
                         "reduction_attempts_per_mismatch": args.reduce_budget},
              "seeded_defect": DEFECT if args.seed_defect else None, "cases": []}
    for number, program in enumerate(programs):
        relative = Path(f'case-{number:04d}')
        case = run_case(program, output / relative / 'original', compiler, cc,
                        args.timeout, args.seed_defect)
        case['directory'] = str(relative / 'original')
        signature = mismatch_signature(case)
        if signature and args.reduce_budget:
            with tempfile.TemporaryDirectory(prefix='reduction-', dir=output / relative) as temporary:
                def preserves(candidate):
                    observed = run_case(candidate, Path(temporary), compiler, cc,
                                        args.timeout, args.seed_defect)
                    return mismatch_signature(observed) == signature
                reduced, reduction = minimize(program, preserves, args.reduce_budget)
            retained = run_case(reduced, output / relative / 'minimized', compiler, cc,
                                args.timeout, args.seed_defect)
            case['minimized'] = {"directory": str(relative / 'minimized'), **reduction,
                                 "outcome": retained['outcome'],
                                 "preserves_signature": mismatch_signature(retained) == signature}
        report['cases'].append(case)
        print(f'case {number}: {case["outcome"]}', flush=True)
    report['outcome'] = combined_outcome([version['outcome'], toolchain['outcome'], target['outcome'],
                                        *[case['outcome'] for case in report['cases']]])
    write_json(output / 'results.json', report)
    print(f'{report["outcome"]}: {output / "results.json"}')
    return 0 if report['outcome'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
