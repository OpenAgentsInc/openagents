#!/usr/bin/env python3
"""Prepare a focused source pack and optional fixed, synthetic runtime evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'repo', 'issue', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--rev', required=True)
    parser.add_argument('--index', type=Path)
    parser.add_argument('--facts', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    index = args.index or args.output / 'index.json'
    timings = {}
    if not index.exists():
        start = time.monotonic()
        subprocess.run([str(args.binary), 'index', '--repo', str(args.repo), '--rev', args.rev, '--output', str(index)], check=True)
        timings['cold_index_s'] = time.monotonic() - start
    start = time.monotonic()
    subprocess.run([str(args.binary), 'preview', '--repo', str(args.repo), '--rev', args.rev, '--index', str(index), '--issue-file', str(args.issue), '--output-dir', str(args.output / 'preview'), '--focused', '--no-lexical', '--no-symbols'], check=True)
    timings['warm_preview_s'] = time.monotonic() - start
    payload = (args.output / 'preview/focused.md').read_text()
    issue = json.loads(args.issue.read_text())
    task = (issue['title'] + '\n' + issue.get('body', '')).lower()
    probe = Path(__file__).with_name('probe_git.py')
    triggered = args.facts and all(term in task for term in ('git', 'ignored', 'worktree'))
    if triggered:
        start = time.monotonic()
        result = subprocess.check_output([sys.executable, str(probe)])
        timings['runtime_probe_s'] = time.monotonic() - start
        (args.output / 'probe.json').write_bytes(result)
        payload += '\n\n## Fixed synthetic Git observation\n\nThis probe ran in a temporary repository on the verification host. It observes Git behavior; it is not a result for your candidate.\n\n```json\n' + result.decode().rstrip() + '\n```\n'
    target = args.output / 'treatment.md'
    target.write_text(payload)
    record = {'schema': 'openagents.briefing-replay.preparation.v1', 'variant': 'explicit-scope-with-conditional-git-probe-v2', 'revision': args.rev, 'binary_sha256': sha(args.binary), 'issue_sha256': sha(args.issue), 'index_sha256': sha(index), 'prepare_sha256': sha(Path(__file__)), 'probe_sha256': sha(probe), 'facts_requested': args.facts, 'probe_triggered': triggered, 'payload_bytes': target.stat().st_size, 'payload_sha256': sha(target), 'timings': timings, 'paid_model_calls': 0}
    (args.output / 'preparation.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record), flush=True)


if __name__ == '__main__':
    main()
