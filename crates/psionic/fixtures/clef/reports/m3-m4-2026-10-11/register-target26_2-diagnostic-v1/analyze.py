#!/usr/bin/env python3
"""Describe recorded coordinates without accepting or repairing a GEMM mapping."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

parser = argparse.ArgumentParser()
parser.add_argument('output', type=Path)
args = parser.parse_args()
root = args.output.resolve()
sha = lambda b: hashlib.sha256(b).hexdigest()
result = {'identity': 'register-target26_2-diagnostic-v1', 'mapping_repaired': False,
          'gemm_validated': False, 'speed_claim': False, 'cases': [], 'failures': []}

def require(condition, reason):
    if not condition:
        raise ValueError(reason)

def hypothesis(entries, object_id, side, right_logical):
    width, height = ((16, 16) if object_id < 4 else
                     ((32, 16) if object_id >= 6 or right_logical else (16, 32)))
    seen = Counter()
    outside = mismatches = 0
    examples = []
    for row in entries:
        x, y = row['coordinate']; bits = row['bits']; seen[x, y] += 1
        if not (0 <= x < width and 0 <= y < height):
            outside += 1
            if len(examples) < 8:
                examples.append(dict(row, reason='outside proposed shape'))
            continue
        if object_id < 4:
            r, k = (object_id // 2) * 16 + y, (object_id % 2) * 16 + x
            expected = 0x2000 + r * side + k if r < side and k < side else 0
        elif object_id < 6:
            r, k = (x, (object_id - 4) * 16 + y) if right_logical else (y, (object_id - 4) * 16 + x)
            expected = 0x3000 + r * side + k if r < side and k < side else 0
        else:
            r, c = (object_id - 6) * 16 + y, x
            expected = 0x3f000000 + r * side + c if r < side and c < side else 0
        if bits != expected:
            mismatches += 1
            if len(examples) < 8:
                examples.append(dict(row, expected_bits=expected, reason='bits differ under proposed mapping'))
    duplicates = sum(count - 1 for count in seen.values())
    coverage = set(seen) == {(x, y) for x in range(width) for y in range(height)}
    return {'proposed_shape': [width, height], 'outside_shape': outside, 'duplicate_coordinates': duplicates,
            'complete_unique_coverage': coverage and duplicates == 0, 'bits_mismatches_inside_shape': mismatches,
            'matches_shape_coverage_and_bits': coverage and duplicates == 0 and outside == 0 and mismatches == 0,
            'first_differences': examples}

try:
    receipt = json.loads((root / 'receipt.json').read_text())
    require(receipt['status'] == 'complete' and receipt['exit'] == 0, 'guarded exporter did not complete')
    require(receipt['memory']['owned_group_stopped'] is True, 'owned group cleanup was not confirmed')
    require(all(receipt['memory'][name] is None for name in ['memory_rejection', 'sampling_error', 'cleanup_error']),
            'guard recorded a resource, sampler, or cleanup failure')
    events = [json.loads(line) for line in (root / 'stdout.jsonl').read_text().splitlines() if line.strip()]
    require(not any(e['kind'] == 'failure' for e in events), 'exporter emitted a failure')
    complete = [e for e in events if e['kind'] == 'complete']
    require(len(complete) == 1 and complete[0]['commands'] == 2 and complete[0]['dispatches'] == 2,
            'expected exactly two dispatches')
    require(complete[0]['explicit_peak_bytes'] == 273408 and complete[0]['explicit_live_bytes'] == 0,
            'explicit allocation accounting differs')
    for side, name in [(1, 'tail-1'), (32, 'interior-32')]:
        done = [e for e in events if e['kind'] == 'case_complete' and e['case'] == name]
        require(len(done) == 1 and done[0]['input_redzones_holes_passed'] is True, f'case did not pass export checks: {name}')
        raw = (root / f'{name}.u32').read_bytes()
        doc = json.loads((root / f'{name}.json').read_text())
        require(len(raw) == 263168 and sha(raw) == doc['raw_sha256'], f'raw export hash or length differs: {name}')
        require(doc['dimensions'] == [side] * 3 and doc['base'] == [0, 0, 0], 'case coordinates differ')
        require(doc['library_sha256'] == receipt['library_sha256'], 'library identity differs')
        words = struct.unpack('<65792I', raw)
        require(len(doc['objects']) == 256, 'object/lane entry count differs')
        objects = [[] for _ in range(8)]
        seen_lanes = set()
        for lane in doc['objects']:
            obj, index = lane['object'], lane['lane']
            require((obj, index) not in seen_lanes and 0 <= obj < 8 and 0 <= index < 32, 'duplicate or invalid object/lane')
            seen_lanes.add((obj, index))
            offset = (obj * 32 + index) * 257
            require(lane['capacity'] == words[offset] and 0 < words[offset] <= 64, 'capacity differs')
            expected_rows, holes = [], []
            for slot in range(64):
                x, y, bits, flag = words[offset + 1 + slot * 4:offset + 5 + slot * 4]
                if (x, y, bits, flag) == (0xffffffff,) * 4:
                    holes.append(slot)
                else:
                    require(flag == 1 and slot < words[offset], 'malformed or partial slot')
                    expected_rows.append({'slot': slot, 'coordinate': [x, y], 'bits': bits, 'valid_flag': flag})
                    objects[obj].append({'lane': index, 'slot': slot, 'coordinate': [x, y], 'bits': bits})
            require(lane['slots'] == expected_rows and lane['sentinel_holes'] == holes, 'JSON dump differs from raw slots')
        rows = []
        for obj, entries in enumerate(objects):
            require(entries, f'object has no records: {obj}')
            row = {'object': obj, 'slots': len(entries),
                   'minimum_coordinate': [min(e['coordinate'][i] for e in entries) for i in range(2)],
                   'maximum_coordinate': [max(e['coordinate'][i] for e in entries) for i in range(2)]}
            if 4 <= obj < 6:
                row['physical_K_N_hypothesis'] = hypothesis(entries, obj, side, False)
                row['logical_N_K_hypothesis'] = hypothesis(entries, obj, side, True)
            else:
                row['existing_left_or_destination_hypothesis'] = hypothesis(entries, obj, side, False)
            rows.append(row)
        result['cases'].append({'case': name, 'raw_sha256': sha(raw), 'objects': rows})
except Exception as error:
    result['failures'].append(f'{type(error).__name__}: {error}')
result['export_complete'] = len(result['cases']) == 2 and not result['failures']
with (root / 'analysis.json').open('x') as out:
    json.dump(result, out, indent=2); out.write('\n')
print(json.dumps(result, indent=2))
raise SystemExit(0 if result['export_complete'] else 1)
