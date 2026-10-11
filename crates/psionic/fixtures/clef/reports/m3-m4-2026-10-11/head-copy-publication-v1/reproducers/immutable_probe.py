"""Byte comparisons for bounded, scratch-only MPS diagnostics."""
import hashlib
import json
from pathlib import Path


def raw(torch, tensor):
    cpu = tensor.detach().cpu().clone().contiguous()
    return cpu.reshape(-1).view(torch.uint8).numpy().tobytes()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def compare(torch, tensors, immutable):
    result = {}
    for name, tensor in tensors.items():
        actual = raw(torch, tensor)
        expected = immutable[name]
        row = {'exact': actual == expected, 'sha256': digest(actual)}
        if actual != expected:
            changed = [i for i, (a, b) in enumerate(zip(expected, actual)) if a != b]
            row.update(changed_bytes=len(changed), first_changed_byte_offsets=changed[:32])
            if tensor.dtype == torch.int64:
                import struct
                left = struct.unpack('<' + 'q' * (len(expected)//8), expected)
                right = struct.unpack('<' + 'q' * (len(actual)//8), actual)
                row['changed_values'] = [{'index': i, 'before': a, 'after': b}
                                         for i, (a, b) in enumerate(zip(left, right)) if a != b]
        result[name] = row
    return result


def write(path, report):
    Path(path).write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
