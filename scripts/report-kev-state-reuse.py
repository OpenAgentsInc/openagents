"""Count exact encoded-state reuse in the recorded request-size audit.

The input contains only state hashes and token counts. This models a four-entry
LRU above a 384-token threshold; it does not claim a measured cache speedup.
"""
import argparse
from collections import OrderedDict
import json
import math
from pathlib import Path
import statistics


def report(rows):
    cache = OrderedDict()
    seen = set()
    accesses = []
    distances = []
    hits = eligible = refused = 0
    for row in rows:
        key = row['state_sha256']
        if key in seen:
            previous = len(accesses) - 1 - accesses[::-1].index(key)
            distances.append(len(set(accesses[previous+1:])))
        seen.add(key)
        accesses.append(key)
        if not row['admitted_at_4096']:
            refused += 1
            continue
        if row['state_tokens'] < 384:
            continue
        eligible += 1
        hits += key in cache
        cache[key] = None
        cache.move_to_end(key)
        if len(cache) > 4:
            cache.popitem(last=False)
    lengths = sorted(row['state_tokens'] for row in rows)
    return {'requests': len(rows), 'unique_encoded_states': len(seen),
            'exact_repeat_requests': len(rows)-len(seen),
            'state_tokens': {'min': lengths[0], 'median': statistics.median(lengths),
                             'p95': lengths[math.ceil(.95 * len(lengths))-1], 'max': lengths[-1]},
            'refused_at_packed_limit': refused, 'eligible': eligible, 'lru_hits': hits,
            'hit_rate_all_requests': hits/len(rows),
            'hit_rate_eligible': hits/eligible if eligible else None,
            'observed_distinct_state_reuse_distances': distances}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('shapes', nargs='+', type=Path)
    args = parser.parse_args()
    all_rows, suites, tokenizers = [], {}, set()
    for path in args.shapes:
        data = json.loads(path.read_text())
        tokenizers.add((data['tokenizer_sha256'], data['option_isolation'], data['max_state']))
        rows = data['rows']
        if any(row['partition'] == 'locked' for row in rows):
            raise ValueError('Locked items are not part of this audit')
        suites[data['suite']] = {'suite_digest': data['suite_digest'],
            'question_digest': data['question_digest'], 'all': report(rows),
            'families': {family: report([r for r in rows if r['family'] == family])
                         for family in sorted({r['family'] for r in rows})}}
        all_rows.extend(rows)
    if len(tokenizers) != 1:
        raise ValueError('Tokenizer or encoding configuration changed')
    print(json.dumps({'schema': 'openagents.kev.state_reuse.v1',
        'policy': {'entries': 4, 'minimum_state_tokens': 384, 'packed_limit': 4096},
        'tokenizer_and_encoding': list(tokenizers.pop()), 'suites': suites,
        'concatenated_suites_in_argument_order': report(all_rows)}, indent=2))


if __name__ == '__main__':
    main()
