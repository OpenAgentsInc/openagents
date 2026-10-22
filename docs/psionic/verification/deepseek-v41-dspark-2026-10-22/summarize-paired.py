import csv
import gzip
import hashlib
import json
import pathlib
import struct
import sys
import tarfile

# Usage: summarize-paired.py REPO RESULTS BUILD_JSON TEST_ARCHIVE [NATIVE_RECEIPT]
repo = pathlib.Path(sys.argv[1])
results = pathlib.Path(sys.argv[2])
build_path = pathlib.Path(sys.argv[3])
test_archive = pathlib.Path(sys.argv[4])
native_receipt = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else None
oracle_path = repo / 'crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/oracle/manifest.json.gz'
oracle = json.loads(gzip.decompress(oracle_path.read_bytes()))['prompts']
plain = json.loads((results / 'plain.answers.json').read_text())
draft = [json.loads(line) for line in (results / 'dspark.answers.jsonl').read_text().splitlines()]
assert len(plain) == len(oracle) == 8 and len(draft) == 9
rows = []
for i, (p, d) in enumerate(zip(plain, draft)):
    source = oracle[i % len(oracle)]
    assert p['prompt_ids'] == d['prompt_ids'] == source['prompt_token_ids']
    assert p['mode'] == d['mode'] == (source.get('thinking_mode') or 'chat')
    ids, actual = p['ids'], d['tokens']
    assert len(ids) == len(actual) == 32
    golden = source['generated_ids']
    ds = d['dspark']
    summary = p['decode_ms_per_token']
    assert summary['count'] == len(ids) - 1
    elapsed = summary['count'] * summary['mean'] / 1000
    prompt = source['prompt_token_ids']
    rows.append({
        'round': i, 'oracle_id': source['id'],
        'mode': p['mode'], 'prompt_tokens': len(prompt),
        'prompt_ids_sha256_le_u32': hashlib.sha256(struct.pack('<' + 'I' * len(prompt), *prompt)).hexdigest(),
        'plain_ids': ids, 'dspark_ids': actual, 'greedy_equal': ids == actual,
        'oracle_prefix_positions': len(golden),
        'plain_oracle_prefix_matches': sum(a == b for a, b in zip(ids, golden)),
        'plain_oracle_prefix_equal': ids[:len(golden)] == golden,
        'plain_prefill_seconds': p['prefill_ms'] / 1000,
        'dspark_prefill_seconds': d['prefill_seconds'],
        'plain_decode_seconds': elapsed, 'dspark_decode_seconds': d['decode_seconds'],
        'plain_tokens_per_second': p['decode_tokens_per_s'],
        'dspark_tokens_per_second': d['decode_tokens_per_second'],
        'dspark': ds,
    })
build = json.loads(build_path.read_text())
build_info = json.loads((results / 'build-info.json').read_text())
assert build_info['sha256'] == build['sha256']
assert build_info['bundle'] == 'verified'
input_hashes = {pathlib.Path(line.split(maxsplit=1)[1]).name: line.split()[0]
    for line in (results / 'input-sha256.txt').read_text().splitlines()}
assert input_hashes['worker'] == build['sha256']
final_build_hashes = {pathlib.Path(line.split(maxsplit=1)[1]).name: line.split()[0]
    for line in (results / 'final-build-sha256.txt').read_text().splitlines()}
assert final_build_hashes['worker'] == build['sha256']
assert final_build_hashes['BUILD.json'] == hashlib.sha256(build_path.read_bytes()).hexdigest()
def caches(value):
    found = []
    if isinstance(value, dict):
        if 'expert_cache_at_load' in value:
            found.append(value['expert_cache_at_load'])
        for child in value.values():
            found.extend(caches(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(caches(child))
    return found
plain_report = json.loads((results / 'plain.report.json').read_text())
draft_report = json.loads((results / 'dspark.report.json').read_text())
assert plain_report['max_context'] == 32768
assert plain_report['max_step_tokens'] == 4096
assert plain_report['sampling']['temperature'] == 0
plain_cache = caches(plain_report)[0]
draft_cache = caches(draft_report)[0]
keys = ['slots_per_layer', 'layers', 'device_bytes']
assert {k: plain_cache[k] for k in keys} == {k: draft_cache[k] for k in keys}
library_hashes = {pathlib.Path(line.split(maxsplit=1)[1]).name: line.split()[0]
    for line in (results / 'library-sha256.txt').read_text().splitlines()}
assert library_hashes == build['libraries']
final_library_hashes = {pathlib.Path(line.split(maxsplit=1)[1]).name: line.split()[0]
    for line in (results / 'final-library-sha256.txt').read_text().splitlines()}
assert final_library_hashes == library_hashes
with tarfile.open(test_archive) as archive:
    tests = json.loads(archive.extractfile('compact/executables.json').read())
memory = list(csv.DictReader((results / 'memory.csv').read_text().splitlines()))
peak_mib = max(int(next(v for k, v in row.items() if k.lstrip().startswith('memory.used')).strip().split()[0]) for row in memory)
def aggregate(selected):
    plain_count = sum(max(0, len(r['plain_ids']) - 1) for r in selected)
    draft_count = sum(max(0, len(r['dspark_ids']) - 1) for r in selected)
    plain_seconds = sum(r['plain_decode_seconds'] for r in selected)
    draft_seconds = sum(r['dspark_decode_seconds'] for r in selected)
    accepted = sum(r['dspark']['accepted'] for r in selected)
    proposed = sum(r['dspark']['proposed'] for r in selected)
    prefix_histogram = [0] * 6
    for row in selected:
        for length in row['dspark']['accepted_lengths']:
            prefix_histogram[length] += 1
    return {'round_count': len(selected), 'plain_decode_tokens': plain_count,
        'dspark_decode_tokens': draft_count, 'plain_decode_seconds': plain_seconds,
        'dspark_decode_seconds': draft_seconds,
        'plain_tokens_per_second': plain_count / plain_seconds,
        'dspark_tokens_per_second': draft_count / draft_seconds,
        'accepted': accepted, 'proposed': proposed,
        'acceptance_rate': accepted / proposed if proposed else 0,
        'accepted_prefix_histogram': prefix_histogram}
report = {
    'schema': 'openagents.deepseek_v41.dspark_verification.v1',
    'source_commit': build['commit'], 'source_tree': build['tree'],
    'worker_sha256': build['sha256'],
    'build_sha256': hashlib.sha256(build_path.read_bytes()).hexdigest(),
    'test_archive_sha256': hashlib.sha256(test_archive.read_bytes()).hexdigest(),
    'native_receipt_sha256': hashlib.sha256(native_receipt.read_bytes()).hexdigest() if native_receipt else None,
    'native_receipt_uri': 'gs://openagents-psionic-models/verification/deepseek-v41-dspark/2026-10-22/3f32d9abb6/native-pair-3f32d9abb6.tar.gz',
    'input_hashes': input_hashes,
    'run_times': {p.stem: p.read_text().strip() for p in sorted(results.glob('*-*.txt'))
        if p.stem.endswith(('-started', '-finished'))},
    'raw_file_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(results.iterdir()) if p.is_file() and p.name != 'summary.json'},
    'libraries': build['libraries'], 'test_executables': tests,
    'checkpoint': json.loads((results / 'checkpoint-metadata.json').read_text()),
    'oracle_path': str(oracle_path.relative_to(repo)),
    'oracle_manifest_sha256': hashlib.sha256(oracle_path.read_bytes()).hexdigest(),
    'gpu': (results / 'gpu.csv').read_text().strip(), 'peak_device_memory_mib': peak_mib,
    'memory_sampling_interval_seconds': 2,
    'host_cpu': json.loads((results / 'host-cpu.json').read_text()),
    'host_memory': (results / 'host-memory.txt').read_text().strip(),
    'host_kernel': (results / 'host-kernel.txt').read_text().strip(),
    'vm': {'name': 'oa-maya-12465-q3', 'type': 'g4-standard-48',
        'instance_id': '6024340802794656199', 'zone': 'asia-northeast1-b',
        'project': 'openagentsgemini'},
    'expert_cache_at_load': {k: draft_cache[k] for k in keys},
    'staging_bytes': {'plain': plain_cache['staging_bytes'], 'dspark': draft_cache['staging_bytes']},
    'profiles': {'ordinary_runtime_label': plain_report['runtime'], 'dspark': draft_report['profile']},
    'configuration': {'max_context': 32768, 'max_step_tokens': 4096, 'prefill_chunk': 4096,
        'expert_cache_argument': '40', 'experts': 'file', 'engram': 'file',
        'prefill': 'layer-major', 'kv': 'packed', 'miss_lane': 'auto',
        'sampling': plain_report['sampling'], 'max_tokens': 32, 'repeat_answer_argument': 9},
    'all_greedy_equal': all(r['greedy_equal'] for r in rows),
    'trial_order': 'DSpark first (eight required cases plus one short Engram repeat), then ordinary (eight required cases).',
    'repeat_note': 'The ordinary chain ignores --repeat-answer when --prompts-jsonl is nonempty. No ordinary warm-repeat measurement exists; the DSpark-only repeat is separate from the paired aggregate.',
    'decode_token_denominator': 'Generated tokens after the first prefill prediction in each round: 248 per mode for the eight required paired cases. The separate DSpark-only repeat adds 31 decode tokens.',
    'required_cases_aggregate': aggregate(rows),
    'rounds': rows,
}
repeat = draft[8]
assert repeat['prompt_ids'] == oracle[0]['prompt_token_ids']
assert repeat['mode'] == (oracle[0].get('thinking_mode') or 'chat')
report['dspark_only_repeat'] = {
    'round': 8, 'oracle_id': oracle[0]['id'],
    'prompt_tokens': len(repeat['prompt_ids']), 'tokens': repeat['tokens'],
    'equal_to_first_dspark': repeat['tokens'] == draft[0]['tokens'],
    'equal_to_first_plain': repeat['tokens'] == plain[0]['ids'],
    'plain_warm_repeat_available': False,
    'prefill_seconds': repeat['prefill_seconds'],
    'decode_seconds': repeat['decode_seconds'],
    'decode_tokens': len(repeat['tokens']) - 1,
    'tokens_per_second': repeat['decode_tokens_per_second'],
    'dspark': repeat['dspark'],
}
paired = report['required_cases_aggregate']
paired['dspark_to_plain_throughput_ratio'] = paired['dspark_tokens_per_second'] / paired['plain_tokens_per_second']
report['interpretation'] = 'All eight required prompts preserve ordinary greedy output. Drafting is slower in this single paired trial; these results establish neither a speedup nor qualification against the reference checkpoint.'
(results / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({k: report[k] for k in ['all_greedy_equal', 'required_cases_aggregate', 'dspark_only_repeat', 'peak_device_memory_mib']}, indent=2))
sys.exit(0 if report['all_greedy_equal'] and report['dspark_only_repeat']['equal_to_first_dspark'] else 1)
