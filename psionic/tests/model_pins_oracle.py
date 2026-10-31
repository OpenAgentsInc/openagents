"""Independent check of psionic/fixtures/models/pins against the other retained evidence.

Tooling only: it reads committed files, never the network, and never runs
upstream code. The Coax library writes the manifests; this re-derives what it
can from the separate tensor inventories, the bpe-train source record and the
synthetic toy file, so a pin typed wrong in one place is caught in another.
"""
import hashlib
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
PINS = ROOT / 'psionic/fixtures/models/pins'
SHA = re.compile(r'^[0-9a-f]{64}$')
REFUSALS = ['psionic.pin.' + n for n in ('target', 'path', 'bytes', 'digest', 'scope', 'length', 'options', 'score')]
ROLES = {'weights', 'index', 'config', 'tokenizer', 'readout', 'renderer', 'reference', 'card', 'license'}
UNITS = {'model-io/weights', 'ops', 'qwen-block', 'qwen-matvec', 'delta', 'qwen-rope', 'qwen-attention',
         'qwen-decision', 'qwen-prefix', 'readout', 'bpe-train'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fail(msg):
    print('model pins oracle: ' + msg, file=sys.stderr)
    sys.exit(1)


def common(m, name):
    if m.get('schema') != 'psionic.model-pin.v1' or m.get('target') != name:
        fail(f'{name}: schema or target')
    if m['refusals'] != REFUSALS:
        fail(f'{name}: refusal identities')
    paths = [f['path'] for f in m['files']]
    if len(set(paths)) != len(paths) or paths != sorted(paths):
        fail(f'{name}: file paths must be unique and sorted')
    for f in m['files']:
        if f['role'] not in ROLES or not SHA.match(f['sha256']) or f['bytes'] <= 0:
            fail(f'{name}: {f["path"]}')
        want = 'read-only; never executed' if f['role'] == 'reference' else 'notice' if f['role'] in ('card', 'license') else 'data'
        if f['use'] != want:
            fail(f'{name}: {f["path"]} use')
    for o in m['operations']:
        if o['unit'] not in UNITS and not o['unit'].startswith('planned:'):
            fail(f'{name}: operation {o["name"]} names no unit')
        if o['unit'] in UNITS and not (ROOT / 'psionic/lib' / (o['unit'] + '.coax')).is_file():
            fail(f'{name}: unit {o["unit"]} is missing')
    return {f['path']: f for f in m['files']}


def decision(name, folder, layers_linear, layers_full):
    m = json.loads((PINS / f'{name}.json').read_text())
    files = common(m, name)
    inv_path = ROOT / m['inventory']
    if m['inventory'] != f'psionic/fixtures/models/{folder}/manifest.json' or sha(inv_path) != m['inventory_sha256']:
        fail(f'{name}: tensor inventory digest')
    inv = json.loads(inv_path.read_text())
    if inv['model'] != m['origin'] or inv['revision'] != m['revision']:
        fail(f'{name}: inventory revision')
    tensors = inv['tensors']
    b = m['bounds']
    if len(tensors) != b['tensors'] or any(t['dtype'] != 'BF16' for t in tensors) or m['storage'] != 'BF16':
        fail(f'{name}: original storage must be all-BF16 with the pinned tensor count')
    names = [t['name'] for t in tensors]
    linear = sum(n.endswith('linear_attn.A_log') for n in names)
    full = sum(n.endswith('self_attn.q_proj.weight') and 'visual' not in n for n in names)
    if (linear, full) != (layers_linear, layers_full) or (b['linear_layers'], b['full_layers']) != (linear, full):
        fail(f'{name}: layer kinds {linear}/{full}')
    if b['layers'] != linear + full:
        fail(f'{name}: layer total')
    vision = sum(n.startswith('model.visual.') for n in names)
    if vision != b['vision_tensors'] or m['scope'] != 'text':
        fail(f'{name}: vision tensors are metadata in a text-only profile ({vision})')
    embed = next(t for t in tensors if t['name'] == 'model.language_model.embed_tokens.weight')
    if embed['shape'] != [b['vocab'], b['width']]:
        fail(f'{name}: embedding shape')
    gate = next(t for t in tensors if t['name'] == 'model.language_model.layers.0.mlp.gate_proj.weight')
    if gate['shape'] != [b['ffn'], b['width']]:
        fail(f'{name}: FFN shape')
    weights = sorted(p for p, f in files.items() if f['role'] == 'weights')
    shards = inv.get('shards')
    if shards is None:
        shards = [{'file': inv['file'], 'bytes': inv['bytes'], 'sha256': inv['sha256']}]
    else:
        if files['model.safetensors.index.json']['sha256'] != inv['index_sha256']:
            fail(f'{name}: shard index digest')
    if weights != [s['file'] for s in shards]:
        fail(f'{name}: weight files')
    for s in shards:
        f = files[s['file']]
        if (f['bytes'], f['sha256']) != (s['bytes'], s['sha256']):
            fail(f'{name}: {s["file"]} pin differs from the inventory')
    if m['license_metadata'] != 'mit' or m['license_text'] is not None or 'LICENSE' in files:
        fail(f'{name}: card metadata is not license text')
    if files['readout.json']['bytes'] != 63 or b['slots'] != 256 or b['max_tokens'] != 16384:
        fail(f'{name}: readout and trained length')
    return files


def main():
    e = decision('elephant-1', 'elephant', 24, 8)
    mm = decision('mammoth-1', 'mammoth', 48, 16)
    for shared in ('tokenizer.json', 'tokenizer_config.json', 'readout.json', 'serve.patch', 'engine.py', 'chat_template.jinja'):
        if e[shared]['sha256'] != mm[shared]['sha256']:
            fail(f'{shared} differs between the two Base Labs revisions')
    if e['config.json']['sha256'] == mm['config.json']['sha256']:
        fail('the two configs must differ')

    n = json.loads((PINS / 'nanochat.json').read_text())
    files = common(n, 'nanochat')
    ref = json.loads((ROOT / 'psionic/fixtures/bpe-train/reference.json').read_text())['source']
    if ref['repo'] != n['origin'] or ref['revision'] != n['revision']:
        fail('nanochat: revision differs from the bpe-train source record')
    for path, rec in ref['files'].items():
        if (files[path]['bytes'], files[path]['sha256']) != (rec['bytes'], rec['sha256']):
            fail(f'nanochat: {path} differs from the bpe-train source record')
    if n['license_text'] != 'LICENSE' or files['LICENSE']['role'] != 'license' or n['license_metadata'] is not None:
        fail('nanochat: the actual LICENSE text is pinned')
    if n['revision'] != '92d63d4e8bb4df75c3b71618f31ddde2378b2bcd':
        fail('nanochat: revision')

    t = json.loads((PINS / 'psionic-pin-toy.json').read_text())
    tf = common(t, 'psionic-pin-toy')['readout.json']
    raw = (PINS / 'toy/readout.json').read_bytes()
    if (tf['bytes'], tf['sha256']) != (len(raw), hashlib.sha256(raw).hexdigest()):
        fail('toy: fixture digest')
    print(f'model pins oracle: elephant-1 {len(e)} files, mammoth-1 {len(mm)}, nanochat {len(files)}, toy 1; inventories, shards, license and bpe-train record agree')


main()
