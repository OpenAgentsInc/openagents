#!/usr/bin/env python3
"""Pins and comparisons for the Elephant original-weight slices.

The slices are small parts of the decision path run with the original
weights: the embedding gather, linear layer 0, full-attention layer 3 and
rows 0..4095 of the vocabulary readout, over five tokens. The references were
recorded once from the Psionic CPU path; the tolerances in slices.json were
fixed before that run. No weights are committed: the checker authenticates
the bytes it uses (safetensors header and every tensor range) against pins.

  elephant_slices.py pins DIR                 check the pinned ranges of DIR/model.safetensors
  elephant_slices.py pins DIR --generate      hash the whole file against the manifest, then write pins
  elephant_slices.py compare OUT DIR          compare harness outputs in OUT with the references
  elephant_slices.py record OUT               store OUT as the references (generation only)
  elephant_slices.py inputs OUT               write the recorded inputs for layer 3 and the readout
  elephant_slices.py tokens OUT               write the slice tokens for the harness
"""
import hashlib
import json
import math
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / 'psionic/fixtures/models/elephant'
SLICES = FIXTURES / 'slices'
SPEC = SLICES / 'slices.json'
D = 4096
LAYERS = (0, 3)
REFERENCES = ('linear0.f32', 'full3.f32', 'readout.f32')


def spec():
    return json.loads(SPEC.read_text())


def header(path):
    with open(path, 'rb') as f:
        n = struct.unpack('<Q', f.read(8))[0]
        if not 2 <= n <= 1048576:
            raise ValueError('header size')
        raw = f.read(n)
    return 8 + n, raw, json.loads(raw)


def ranges(s, path):
    """(label, begin, end) for every byte range the slices read."""
    base, _, tensors = header(path)
    out = []
    names = sorted(n for n in tensors if n != '__metadata__' and (
        any(n.startswith(f'model.language_model.layers.{i}.') for i in LAYERS) or n == 'model.language_model.norm.weight'))
    for name in names:
        a, b = tensors[name]['data_offsets']
        out.append((name, base + a, base + b))
    a, _ = tensors['model.language_model.embed_tokens.weight']['data_offsets']
    for t in s['tokens']:
        out.append((f'embed row {t}', base + a + t * D * 2, base + a + (t + 1) * D * 2))
    a, _ = tensors['lm_head.weight']['data_offsets']
    out.append((f'lm_head rows 0..{s["readout_rows"]}', base + a, base + a + s['readout_rows'] * D * 2))
    return out


def sha_range(f, begin, end):
    digest = hashlib.sha256()
    f.seek(begin)
    left = end - begin
    while left:
        chunk = f.read(min(left, 16 << 20))
        if not chunk:
            raise ValueError('short read')
        digest.update(chunk)
        left -= len(chunk)
    return digest.hexdigest()


def pins(directory, generate):
    s = spec()
    path = Path(directory) / s['file']
    if path.stat().st_size != s['bytes']:
        raise SystemExit('slices: the checkpoint is not the pinned size')
    size, raw, _ = header(path)
    if generate:
        whole = hashlib.sha256()
        with open(path, 'rb') as f:
            for chunk in iter(lambda: f.read(64 << 20), b''):
                whole.update(chunk)
        if whole.hexdigest() != s['sha256']:
            raise SystemExit('slices: the checkpoint is not the pinned original')
        with open(path, 'rb') as f:
            s['pins'] = {'header': hashlib.sha256(struct.pack('<Q', size - 8) + raw).hexdigest(),
                         'ranges': [{'name': n, 'begin': a, 'end': b, 'sha256': sha_range(f, a, b)}
                                    for n, a, b in ranges(s, path)]}
        SPEC.write_text(json.dumps(s, indent=2) + '\n')
        print(f"slices: pinned {len(s['pins']['ranges'])} ranges of the authenticated original")
        return
    p = s['pins']
    if hashlib.sha256(struct.pack('<Q', size - 8) + raw).hexdigest() != p['header']:
        raise SystemExit('slices: safetensors header differs from the pin')
    expected = [(r['name'], r['begin'], r['end']) for r in p['ranges']]
    if expected != ranges(s, path):
        raise SystemExit('slices: tensor ranges differ from the pins')
    total = 0
    with open(path, 'rb') as f:
        for r in p['ranges']:
            if sha_range(f, r['begin'], r['end']) != r['sha256']:
                raise SystemExit(f"slices: {r['name']} differs from its pin")
            total += r['end'] - r['begin']
    print(f"slices: {len(p['ranges'])} pinned ranges ({total / 2**20:.0f} MiB) match the original")


def floats(path, n):
    raw = Path(path).read_bytes()
    if len(raw) != 4 * n:
        raise SystemExit(f'slices: {path} has {len(raw)} bytes, expected {4 * n}')
    values = struct.unpack(f'<{n}f', raw)
    if not all(math.isfinite(v) for v in values):
        raise SystemExit(f'slices: {path} is not finite')
    return values


def within(name, actual, reference, tol):
    worst = 0.0
    failures = 0
    for a, r in zip(actual, reference):
        d = abs(a - r)
        worst = max(worst, d)
        if d > tol['atol'] + tol['rtol'] * abs(r):
            failures += 1
    status = 'ok  ' if failures == 0 else 'FAIL'
    print(f"{status} {name}: {len(actual)} values, max |difference| {worst:.9g} "
          f"(tolerance {tol['atol']:g} + {tol['rtol']:g}|ref|){'' if not failures else f', {failures} outside'}")
    return failures == 0


def bf16_rows(path, s):
    """Independent decode of the embedding rows: BF16 is the top half of binary32."""
    base, _, tensors = header(path)
    a, _ = tensors['model.language_model.embed_tokens.weight']['data_offsets']
    out = []
    with open(path, 'rb') as f:
        for t in s['tokens']:
            f.seek(base + a + t * D * 2)
            halves = struct.unpack(f'<{D}H', f.read(D * 2))
            out.extend(struct.unpack(f'<{D}f', struct.pack(f'<{D}I', *(h << 16 for h in halves))))
    return out


def compare(out, directory):
    s = spec()
    tol = s['tolerances']
    n = len(s['tokens']) * D
    out = Path(out)
    ok = True
    embed = floats(out / 'embed.f32', n)
    ok &= within('embed', embed, bf16_rows(Path(directory) / s['file'], s), tol['embed'])
    for name, count in (('linear0', n), ('full3', n), ('readout', s['readout_rows'])):
        reference = floats(SLICES / f'{name}.f32', count)
        ok &= within(name, floats(out / f'{name}.f32', count), reference, tol[name])
    if s['tokens'][0] == 32:
        capture = floats(FIXTURES / 'layer0-token32-transformers.f32', D)
        ok &= within('linear0 token 0 vs Transformers', floats(out / 'linear0.f32', n)[:D], capture,
                     tol['linear0_token0_transformers'])
    if not ok:
        raise SystemExit('slices: outside the frozen tolerances')
    print('Elephant original-weight slices match the recorded CPU references')


def record(out):
    s = spec()
    if 'pins' not in s:
        raise SystemExit('slices: generate the pins first')
    n = len(s['tokens']) * D
    hashes = {}
    for name, count in (('linear0.f32', n), ('full3.f32', n), ('readout.f32', s['readout_rows'])):
        raw = (Path(out) / name).read_bytes()
        floats(Path(out) / name, count)
        (SLICES / name).write_bytes(raw)
        hashes[name] = hashlib.sha256(raw).hexdigest()
    s['references'] = hashes
    SPEC.write_text(json.dumps(s, indent=2) + '\n')
    print('slices: recorded', ', '.join(hashes))


def inputs(out):
    """Layer 3 reads the recorded linear0 output; the readout its last full3 row."""
    s = spec()
    for name, digest in s['references'].items():
        if hashlib.sha256((SLICES / name).read_bytes()).hexdigest() != digest:
            raise SystemExit(f'slices: reference {name} differs from its pin')
    n = len(s['tokens']) * D
    (Path(out) / 'full-input.f32').write_bytes((SLICES / 'linear0.f32').read_bytes())
    full = (SLICES / 'full3.f32').read_bytes()
    (Path(out) / 'readout-input.f32').write_bytes(full[4 * (n - D):])


def tokens(out):
    (Path(out) / 'tokens.txt').write_text(' '.join(map(str, spec()['tokens'])) + '\n')


def main():
    args = sys.argv[1:]
    if len(args) >= 2 and args[0] == 'pins':
        pins(args[1], '--generate' in args[2:])
    elif len(args) == 3 and args[0] == 'compare':
        compare(args[1], args[2])
    elif len(args) == 2 and args[0] == 'record':
        record(args[1])
    elif len(args) == 2 and args[0] == 'inputs':
        inputs(args[1])
    elif len(args) == 2 and args[0] == 'tokens':
        tokens(args[1])
    else:
        raise SystemExit(__doc__)


if __name__ == '__main__':
    main()
