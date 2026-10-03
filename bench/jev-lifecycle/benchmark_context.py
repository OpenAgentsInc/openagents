#!/usr/bin/env python3
"""Compare source-read implementations without changing context selection."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def without_time(value):
    return {k: v for k, v in value.items() if k != 'wall_s'}


def run(repo, inputs, output):
    here = Path(__file__).parent
    paths = {'original': here / 'context-at-round-one.py', 'batched': here / 'context.py'}
    modules = {name: load_module(name, path) for name, path in paths.items()}
    rows = []
    for item in inputs:
        task = next(t for t in json.loads(Path(item['manifest']).read_text())['tasks']
                    if t['id'] == item['id'])
        index = json.loads(Path(item['index']).read_text())
        response = json.loads(Path(item['recorded_response']).read_text())
        scores = {key.removeprefix('rank_'): answer['score']
                  for key, answer in response['answers'].items() if key.startswith('rank_')}
        observations = {name: [] for name in modules}
        expected = None
        packed_hashes = None
        warmups = {}
        for repetition in range(6):
            order = ['original', 'batched'] if repetition % 2 == 0 else ['batched', 'original']
            for name in order:
                module = modules[name]
                ctx = module.assemble(repo, task['source_commit'], index, task)
                baseline, ranked = module.pack(ctx), module.pack(ctx, scores=scores)
                actual = [without_time(ctx), without_time(baseline), without_time(ranked)]
                if expected is None:
                    expected = actual
                    packed_hashes = {'deterministic': baseline['sha256'], 'recorded_jev': ranked['sha256']}
                if actual != expected:
                    raise ValueError(f'Context or packed output changed: {item["id"]} {name}')
                timing = {'assembly_s': ctx['wall_s'], 'deterministic_pack_s': baseline['wall_s'],
                          'recorded_jev_pack_s': ranked['wall_s']}
                if repetition == 0:
                    warmups[name] = timing
                else:
                    observations[name].append(timing)
        medians = {n: statistics.median(r['assembly_s'] for r in obs) for n, obs in observations.items()}
        rows.append({'id': item['id'], 'source_commit': task['source_commit'],
                     'index_sha256': ctx['index_sha256'], 'candidate_pool_sha256': ctx['candidate_pool_sha256'],
                     'manifest_sha256': sha(Path(item['manifest'])), 'recorded_response_sha256': sha(Path(item['recorded_response'])),
                     'pack_sha256': packed_hashes, 'all_non_timing_fields_equal': True,
                     'warmup': warmups, 'observations': observations, 'median_assembly_s': medians,
                     'median_reduction_fraction': 1 - medians['batched'] / medians['original']})
    result = {'schema': 'openagents.jev-lifecycle.source-read-timing.v1', 'model_calls': 0,
              'source_hashes': {path.name: sha(path) for path in paths.values()},
              'benchmark_sha256': sha(Path(__file__)), 'warm_observations_per_implementation': 5,
              'timing_boundary': 'assemble only: includes source I/O, validation, selection and canonical index hashing; excludes process startup, input JSON loading, packing and output writes',
              'order': 'one warmup per implementation, then five pairs alternating original-first and batched-first',
              'tasks': rows}
    output.write_text(json.dumps(result, indent=2) + '\n')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = run(args.repo, json.loads(args.inputs.read_text()), args.output)
    print(json.dumps([{k: row[k] for k in ('id', 'median_assembly_s', 'median_reduction_fraction')}
                      for row in result['tasks']], indent=2))
