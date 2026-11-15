#!/usr/bin/env python3
# Firm reason: 2 (independent stdlib binary-layout and SHA-256 oracle; never a runtime dependency).
"""Pin the owned two-joint RXE1 record before implementing its Coax codec."""
import hashlib
import json
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[3]
PROFILE = ROOT / 'proto/psionic/robotics-experiment-v1.json'
VECTOR = ROOT / 'psionic/lib/robotics/experiment-wire-golden.coax'
TAG = b'psionic.robotics.experiment.v1'

def frozen():
    joints = [(0, 1, 1, 1, 1, .2, .4, -.1, .7),
              (1, 0, -1, 1, 1, -.125, .25, -.25, .125)]
    observations = [(j, signal, signal, 1, 0., 1.) for j in range(2) for signal in (1, 2)]
    body = b''.join(struct.pack('<IIiII4d', *row) for row in joints)
    body += b''.join(struct.pack('<4I2d', *row) for row in observations)
    length = 240 + len(body)
    header = b'RXE1' + struct.pack('<11I', length, 2, 4, 1, 1, 2_000_000, 20_000_000, 4, 8, 256, 7)
    data = header + b''.join(bytes([i]) * 32 for i in range(1, 7)) + body
    assert len(data) == length == 472
    tag_hash = hashlib.sha256(TAG).digest()
    digest = hashlib.sha256(tag_hash + tag_hash + data).digest()
    profile = {
        'id': TAG.decode(), 'version': 1,
        'specification': 'docs/protocol/robotics-experiment-v1.md',
        'encoding': 'RXE1, little-endian u32/i32/IEEE-binary64; exact active records; no padding',
        'header_bytes': 240, 'joint_bytes': 52, 'observation_bytes': 32, 'maximum_bytes': 2096,
        'joint_fields': ['joint', 'motor', 'sign', 'unit', 'mode', 'home', 'scale', 'low', 'high'],
        'observation_fields': ['joint', 'signal', 'unit', 'frame', 'offset', 'scale'],
        'golden': {'name': 'owned two-joint experiment', 'bytes': length,
                   'joints': joints, 'observations': observations, 'hex': data.hex(),
                   'sha256': hashlib.sha256(data).hexdigest(), 'tagged_sha256': digest.hex()},
    }
    def array(name, values):
        rows = [','.join(str(x) for x in values[k:k + 16]) for k in range(0, len(values), 16)]
        return name + ' :: [' + str(len(values)) + ']u8.[\n  ' + ',\n  '.join(rows) + '\n];\n'
    vector = '// Generated independent RXE1 oracle vectors; regenerate only before publishing a new version.\n'
    vector += array('RXW_GOLDEN', data) + array('RXW_GOLDEN_DIGEST', digest)
    return json.dumps(profile, indent=2) + '\n', vector

def main():
    assert sys.argv[1:] in ([], ['--write']), 'usage: experiment_wire_oracle.py [--write]'
    profile, vector = frozen()
    for path, text in [(PROFILE, profile), (VECTOR, vector)]:
        if not path.exists() and sys.argv[1:] == ['--write']:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        assert path.read_text() == text, f'independent RXE1 golden changed: {path}'
    print('RXE1 independent oracle: 472 bytes, all fields and tagged SHA-256 match')

if __name__ == '__main__':
    main()
