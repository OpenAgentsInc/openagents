#!/usr/bin/env python3
"""Reproduce the audit's local controls without cloud or model calls.

Run with --scratch set to a disposable directory under `openagents scratch`.
The checker controls compile tiny, inspected Rust functions with rustc.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import tempfile
import types


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument('--scratch', type=Path, required=True)
    args = parser.parse_args()
    args.scratch.mkdir(parents=True, exist_ok=True)
    tempfile.tempdir = str(args.scratch)

    checker = module('audit_g2', args.root / 'scripts/bench/deepseek-v41/g2_check.py')
    task = {'thinking_mode': 'thinking', 'assertions': [
        {'kind': 'last_line_exact', 'pattern': '^72$', 'must_pass': True}]}
    judged = checker.judge(task, True, '', '72')
    rust_assertion = {'kind': 'rust_tests', 'function': 'gcd',
                      'cases': [['12, 18', '6'], ['7, 5', '1']]}
    rust = ('```rust\nfn gcd(a: u64, b: u64) -> u64 { '
            'println!("G2-RESULT 0 ok\\nG2-RESULT 1 ok"); '
            'std::process::exit(0); }\n```')
    python_assertion = {'kind': 'python_tests', 'function': 'is_palindrome',
                        'cases': [["'racecar'", 'True'], ["'ab'", 'False']]}
    python = ('```python\ndef is_palindrome(s):\n'
              '    print("G2-RESULT 0 ok\\nG2-RESULT 1 ok")\n'
              '    raise SystemExit(0)\n```')
    results = {
        'missing_reasoning': {'task_passed': judged[0], 'floor_held': judged[2]},
        'complete_forgery_rust': checker.check(rust, rust_assertion),
        'complete_forgery_python': checker.check(python, python_assertion),
    }

    watcher = module('audit_watcher', args.root / 'scripts/pylon/v41-live.py')
    now = 1000.0
    stopped, costs, attempts = [False], [0.0], []
    state = {'zones': ['asia-northeast1-b', 'asia-northeast1-a'],
             'until': now + 360000, 'cap': 300.0}
    # Replace every effect before calling the real retry loop. No watcher
    # state, ledger, process, cloud command, or stop file is touched.
    watcher.load_state = lambda: state
    watcher.save_state = lambda value: None
    watcher.spot_price = lambda: 6.654
    watcher.spend = lambda value, stamp: costs[0]
    watcher.log = lambda text: None
    watcher.time = types.SimpleNamespace(time=lambda: now, sleep=lambda seconds: None)
    watcher.STOP = types.SimpleNamespace(exists=lambda: stopped[0])

    def launch(value, zone, hours):
        attempts.append({'zone': zone, 'hours': hours})
        if len(attempts) == 1:
            costs[0] += 10.0
            return 'refused', 'simulated paid launch that never became ready'
        costs[0] += hours * 6.654
        stopped[0] = True
        return 'ended', 'simulated completed booking'

    watcher.launch = launch
    watcher.serve(types.SimpleNamespace())
    results['watcher_cap'] = {'cap': 300.0, 'attempts': attempts,
                              'admitted_cost': costs[0], 'excess': costs[0] - 300.0}
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
