#!/usr/bin/env python3
"""Compare baseline and syntax excerpts on a pinned, labeled diagnostic panel."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import time


def sha(data):
    return hashlib.sha256(data).hexdigest()


def run(command):
    start = time.perf_counter_ns()
    result = subprocess.run(command, capture_output=True, text=True, check=True)
    return (time.perf_counter_ns() - start) / 1_000_000, result.stdout


def quantiles(rows):
    ordered = sorted(rows)
    return {"count": len(rows), "p50_ms": ordered[math.ceil(len(rows) * .5) - 1],
            "p95_ms": ordered[math.ceil(len(rows) * .95) - 1],
            "max_ms": ordered[-1], "over_one_second": sum(t >= 1000 for t in rows)}


def line_bounds(raw, start, end):
    # Match Rust's newline-based source lines, including CRLF bytes.
    parts = raw.split(b'\n')
    lines = [part + b'\n' for part in parts[:-1]]
    if parts[-1]:
        lines.append(parts[-1])
    assert 1 <= start <= end <= len(lines)
    return sum(map(len, lines[:start - 1])), sum(map(len, lines[:end]))


def source(repo, rev, path, cache):
    if path not in cache:
        cache[path] = subprocess.check_output(['git', '-C', str(repo), 'show', rev + ':' + path])
    return cache[path]


def measure_evidence(brief, case, repo, rev, budget, cache):
    assert brief['commit'] == rev
    assert brief['issue']['title'] == case['issue']['title']
    assert brief['issue']['body'] == case['issue'].get('body', '')
    spans = []
    remaining = budget
    total_bytes = 0
    for item in brief['evidence']:
        raw = source(repo, rev, item['path'], cache)
        start, end = line_bounds(raw, item['start_line'], item['end_line'])
        assert raw[start:end] == item['text'].encode(), item['path']
        assert sha(raw) == item['file_sha256']
        assert sha(raw[start:end]) == item['excerpt_sha256']
        assert item['end_line'] - item['start_line'] < 64
        total_bytes += end - start
        keep = min(remaining, end - start)
        if keep:
            spans.append((item['path'], start, start + keep))
        remaining -= keep
    labels = []
    for expected in case['expected_evidence']:
        raw = source(repo, rev, expected['path'], cache)
        start, end = line_bounds(raw, expected['start_line'], expected['end_line'])
        assert sha(raw[start:end]) == expected['span_sha256'], case['id']
        blob = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', rev + ':' + expected['path']], text=True).strip()
        assert blob == expected['blob'], case['id']
        overlaps = sorted((max(start, a), min(end, b)) for path, a, b in spans
                          if path == expected['path'] and max(start, a) < min(end, b))
        covered, previous_end = 0, start
        for a, b in overlaps:
            covered += max(0, b - max(a, previous_end))
            previous_end = max(previous_end, b)
        labels.append({"path": expected['path'], "start_line": expected['start_line'],
                       "end_line": expected['end_line'], "file_selected": any(e['path'] == expected['path'] for e in brief['evidence']),
                       "file_admitted_under_budget": any(path == expected['path'] for path, _, _ in spans),
                       "span_bytes": end - start, "covered_bytes": covered,
                       "complete": covered == end - start})
    return {"labels": labels, "source_bytes_rendered": total_bytes,
            "source_bytes_admitted_under_budget": budget - remaining,
            "all_labeled_spans_present": bool(labels) and all(item['complete'] for item in labels),
            "candidate_files": brief['candidate_files'],
            "reported_no_match": any('No direct, lexical, or symbol candidates matched.' in note for note in brief['notes']),
            "evidence": [{key: item[key] for key in ['path', 'start_line', 'end_line', 'excerpt_sha256']} |
                         {"syntax_selection": item.get('syntax_selection')} for item in brief['evidence']]}


def aggregate(cases, variant):
    labels = [label for case in cases for label in case['arms'][variant]['quality']['labels']]
    elapsed = [ms for case in cases for ms in case['arms'][variant]['elapsed_ms']]
    absent = [case for case in cases if case.get('expected_absence')]
    present = [case for case in cases if case['arms'][variant]['quality']['labels']]
    return {"timing": quantiles(elapsed), "labels": len(labels),
            "necessary_files": sum(len({label['path'] for label in case['arms'][variant]['quality']['labels']}) for case in cases),
            "files_selected": sum(len({label['path'] for label in case['arms'][variant]['quality']['labels'] if label['file_selected']}) for case in cases),
            "complete_spans": sum(label['complete'] for label in labels),
            "complete_spans_untrimmed": sum(label['complete'] for case in cases for label in case['arms'][variant]['quality_untrimmed']['labels']),
            "span_byte_recall": sum(label['covered_bytes'] for label in labels) / sum(label['span_bytes'] for label in labels) if labels else None,
            "positive_cases": len(present),
            "cases_with_all_labeled_spans": sum(case['arms'][variant]['quality']['all_labeled_spans_present'] for case in present),
            "absence_cases": len(absent),
            "absence_reported_no_match": sum(case['arms'][variant]['quality']['reported_no_match'] for case in absent)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', required=True, type=Path)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--repeats', type=int, default=6)
    parser.add_argument('--source-byte-budget', type=int, default=16384)
    args = parser.parse_args()
    if args.repeats < 1 or args.source_byte_budget < 1:
        parser.error('Repeats and source-byte budget must be positive.')
    repo, output, binary = args.repo.resolve(), args.output.resolve(), str(args.binary.resolve())
    if output == repo or repo in output.parents:
        parser.error('Output must be outside the repository.')
    output.mkdir(parents=True, exist_ok=True)
    fixtures_bytes = args.fixtures.read_bytes()
    fixtures = json.loads(fixtures_bytes)
    rev = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', fixtures['source_revision'] + '^{commit}'], text=True).strip()
    paths = ['crates/briefing-lab/Cargo.toml', 'scripts/briefing-preview.sh']
    paths += sorted(str(p.relative_to(repo)) for p in (repo / 'crates/briefing-lab/src').rglob('*.rs'))
    report = {"schema": "openagents.briefing-lab.retrieval.v1", "recorded_at_utc": time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
              "source_revision": rev, "fixture_sha256": sha(fixtures_bytes),
              "evaluator_sha256": sha(Path(__file__).read_bytes()), "binary_sha256": sha(Path(binary).read_bytes()),
              "source_sha256": {path: sha((repo / path).read_bytes()) for path in paths},
              "host": {"platform": platform.platform(), "logical_cpus": os.cpu_count()},
              "method": "Fresh process, cached issues/indexes, interleaved arm order, no OS-cache flush. Matched 64-line per-excerpt cap. Quality additionally applies a common global source-byte budget in returned order; issue text stays complete. Partial last-line bytes count fractionally. Labels never enter preview inputs. Span recall is a diagnostic proxy, not accepted task outcomes.",
              "source_byte_budget": args.source_byte_budget, "indexes": {}, "cases": []}
    for variant, flags in [('baseline', []), ('syntax', ['--syntax'])]:
        path = output / (variant + '-index.json')
        ms, _ = run([binary, 'index', '--repo', str(repo), '--rev', rev, '--output', str(path)] + flags)
        index = json.loads(path.read_text())
        report['indexes'][variant] = {"elapsed_ms": ms, "bytes": path.stat().st_size, "sha256": sha(path.read_bytes()),
                                       "indexed_files": len(index['files']), "scanned_bytes": index['scanned_bytes'],
                                       "omissions": index['omissions'], "syntax": index.get('syntax')}
        if variant == 'syntax':
            parsed = [item['syntax'] for item in index['files'] if item.get('syntax')]
            report['indexes'][variant]['syntax_files'] = len(parsed)
            report['indexes'][variant]['syntax_files_with_errors'] = sum(bool(item.get('parse_has_error')) for item in parsed)
    cache = {}
    variants = ['baseline', 'syntax', 'syntax_index_control']
    for case_number, case in enumerate(fixtures['cases']):
        if not case['id'] or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789_-' for c in case['id']):
            raise ValueError('Fixture IDs must be simple lowercase file names.')
        directory = output / case['id']
        directory.mkdir(exist_ok=True)
        issue_path = directory / 'issue.json'
        issue_path.write_text(json.dumps(case['issue']) + '\n')
        record = {"id": case['id'], "split": case['split'], "kind": case['kind'], "expected_absence": case.get('expected_absence', False), "arms": {variant: {"elapsed_ms": []} for variant in variants}}
        for iteration in range(args.repeats):
            offset = (iteration + case_number) % len(variants)
            order = variants[offset:] + variants[:offset]
            for variant in order:
                target = directory / variant
                ms, _ = run([binary, 'preview', '--repo', str(repo), '--rev', rev, '--index', str(output / (('baseline' if variant == 'baseline' else 'syntax') + '-index.json')), '--issue-file', str(issue_path), '--output-dir', str(target)] + (['--syntax'] if variant == 'syntax' else []))
                record['arms'][variant]['elapsed_ms'].append(ms)
                observed = json.loads((target / 'briefing.json').read_text())
                fingerprint = sha(json.dumps(observed['evidence'], sort_keys=True).encode())
                previous = record['arms'][variant].setdefault('evidence_fingerprint', fingerprint)
                assert previous == fingerprint, 'Selection changed across repeats: ' + case['id']
        for variant in variants:
            brief = json.loads((directory / variant / 'briefing.json').read_text())
            record['arms'][variant]['quality'] = measure_evidence(brief, case, repo, rev, args.source_byte_budget, cache)
            record['arms'][variant]['quality_untrimmed'] = measure_evidence(brief, case, repo, rev, 1 << 30, cache)
            record['arms'][variant]['timing'] = quantiles(record['arms'][variant]['elapsed_ms'])
            record['arms'][variant]['last_internal_timings_ms'] = brief['timings_ms']
        # Repeated, interleaved metadata control must preserve baseline evidence.
        assert record['arms']['syntax_index_control']['quality'] == record['arms']['baseline']['quality'], case['id']
        baseline_paths = [item['path'] for item in record['arms']['baseline']['quality']['evidence']]
        syntax_paths = [item['path'] for item in record['arms']['syntax']['quality']['evidence']]
        assert baseline_paths == syntax_paths, 'File pool changed: ' + case['id']
        report['cases'].append(record)
    report['aggregate'] = {variant: aggregate(report['cases'], variant) for variant in variants}
    report['by_split'] = {split: {variant: aggregate([case for case in report['cases'] if case['split'] == split], variant) for variant in variants} for split in sorted({case['split'] for case in report['cases']})}
    (output / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({"indexes": report['indexes'], "aggregate": report['aggregate'], "cases": [{"id": case['id'], **{variant: sum(item['complete'] for item in case['arms'][variant]['quality']['labels']) for variant in variants}} for case in report['cases']]}, indent=2))


if __name__ == '__main__':
    main()
