# Firm reason: 2 (independent hashlib/GLB oracle for Coax's registry and Blender exports).
"""Validate the published catalog against source bytes, dependency closure and GLB structure."""
import hashlib, json, struct, sys
from pathlib import Path

registry = Path('assets/registry')
store = Path(sys.argv[1]).expanduser() if len(sys.argv) > 1 else Path.home()/'.openagents/assets'
def digest(path):
    with path.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()
def glb(path):
    with path.open('rb') as f:
        magic, version, size = struct.unpack('<4sII', f.read(12))
        assert magic == b'glTF' and version == 2 and size == path.stat().st_size
        n, kind = struct.unpack('<II', f.read(8)); assert kind == 0x4e4f534a
        data = json.loads(f.read(n)); binlen, binkind = struct.unpack('<II', f.read(8)); start=f.tell()
        assert binkind == 0x004e4942 and start+binlen == size
        for v in data.get('bufferViews', []):
            assert v.get('buffer', 0) == 0 and v.get('byteOffset', 0)+v['byteLength'] <= binlen
        assert data.get('meshes')
        for mesh in data['meshes']:
            for p in mesh['primitives']:
                assert 'POSITION' in p['attributes']
                for key, i in p['attributes'].items(): assert data['accessors'][i]['count'] > 0
        for image in data.get('images', []): assert 'bufferView' in image, 'GLB image not self-contained'
        for skin in data.get('skins', []): assert skin['joints'] and max(skin['joints']) < len(data['nodes'])
        return data

packages = {}
source_files = 0
for lockpath in sorted((registry/'packages').glob('*.json')):
    if lockpath.name.endswith('.files.json'): continue
    lock = json.loads(lockpath.read_text()); assert lock['schema']=='oa-package-lock-v1'
    snapshot = store/'originals'/lock['revision']; inv = snapshot/'inventory.json'
    assert digest(inv)==lock['revision']
    portable=registry/'packages'/(lock['package']+'.files.json')
    assert portable.read_bytes()==inv.read_bytes()
    inventory=json.loads(inv.read_text()); assert inventory['package']==lock['package']
    files={r['path']: r for r in inventory['files']}; assert len(files)==len(inventory['files'])
    for name, row in files.items():
        path=snapshot/'source'/name
        assert not path.is_symlink() and path.resolve().is_relative_to((snapshot/'source').resolve())
        assert path.stat().st_size==row['bytes'] and digest(path)==row['sha256'], name
    assert inventory['license_evidence'] in files
    assert 'CC0 1.0' in (snapshot/'source'/inventory['license_evidence']).read_text()
    packages[lock['package']] = (lock, snapshot, files); source_files += len(files)

records = {}
seen_artifacts = set()
for p in sorted((registry/'assets').glob('*.jsonl')):
    for line in p.read_bytes().splitlines():
        row=json.loads(line); definition=row['definition']; aid=row['id']
        assert aid not in records and definition['id']==aid
        # Preserve the exact raw definition serialization; do not share Coax's canonicalizer.
        raw=line.split(b'"definition":',1)[1][:-1]
        assert hashlib.sha256(raw).hexdigest()==row['revision'], aid
        assert (store/'objects'/row['revision']).read_bytes()==raw
        lock, snapshot, files = packages[definition['package']]
        assert definition['package_revision']==lock['revision']
        obs=definition['source']
        if obs.get('format')=='gltf':
            assert obs['path'] in files
            model=snapshot/'source'/obs['path']; data=json.loads(model.read_text())
            dependencies=[model.name]+[x['uri'] for key in ('buffers','images') for x in data.get(key,[]) if not x['uri'].startswith('data:application/octet-stream;base64,')]
            if 'animations' in obs: assert obs['animations']==[x['name'] for x in data.get('animations',[])]
            assert [x['source_name'] for x in definition['artifacts']]==dependencies
            for artifact, name in zip(definition['artifacts'],dependencies):
                actual=model.parent/name
                assert actual.resolve().is_relative_to((snapshot/'source').resolve())
                assert digest(actual)==artifact['sha256'] and actual.stat().st_size==artifact['bytes']
                obj=store/'objects'/artifact['sha256']
                assert digest(obj)==artifact['sha256'] and obj.stat().st_size==artifact['bytes']
            records[aid]=row
            continue
        if definition['kind']=='animation':
            assert obs['path'] in files and obs['motion']=='in-place'
            library=snapshot/'source'/obs['path']
            data=glb(library)
            assert data['animations'][obs['animation_index']]['name']==definition['name']
            assert definition['artifacts'][0]['sha256']==files[obs['path']]['sha256']
            for artifact in definition['artifacts']:
                obj=store/'objects'/artifact['sha256']
                if artifact['sha256'] not in seen_artifacts:
                    assert digest(obj)==artifact['sha256'] and obj.stat().st_size==artifact['bytes']
                    seen_artifacts.add(artifact['sha256'])
            records[aid]=row
            continue
        assert obs['source'] in files and obs['objects']
        for image in obs['images']:
            if image['packed']: continue
            imagepath=image['path']
            assert imagepath.startswith('//'), (aid, imagepath)
            actual=(snapshot/'source'/obs['source']).parent/imagepath[2:]
            assert actual.resolve().is_relative_to((snapshot/'source').resolve()) and actual.is_file(), (aid, imagepath)
        assert (store/'objects'/definition['recipe_sha256']).is_file(), aid
        for artifact in definition['artifacts']:
            obj=store/'objects'/artifact['sha256']
            if artifact['sha256'] not in seen_artifacts:
                assert digest(obj)==artifact['sha256'] and obj.stat().st_size==artifact['bytes']
                data=glb(obj); seen_artifacts.add(artifact['sha256'])
                if definition['package']=='village' and definition['kind']=='static-mesh':
                    # Standalone village pieces must lose the source display scene's grid offset.
                    roots=[data['nodes'][i] for s in data['scenes'] for i in s.get('nodes', [])]
                    assert all(max(map(abs,n.get('translation',[0,0,0]))) < .001 for n in roots), aid
        records[aid]=row
for aid, row in records.items():
    d=row['definition']
    if d['kind']=='collision': assert d['collision_owner'] in records, aid
    for parent in d.get('parents', []): assert parent['id'] in records and records[parent['id']]['revision']==parent['revision']
for p in (registry/'collections').glob('*.lock.json'):
    collection=json.loads(p.read_text())
    for pin in collection['assets']: assert records[pin['id']]['revision']==pin['revision'], (p, pin)
    for preview in collection.get('previews', []): assert digest(Path(preview['path']))==preview['sha256']
for p in (registry/'packages').glob('*.representations.jsonl'):
    pkg=p.name.removesuffix('.representations.jsonl'); files=packages[pkg][2]
    for line in p.read_text().splitlines():
        row=json.loads(line); assert files[row['path']]['sha256']==row['sha256']
        if 'candidate_asset' in row: assert row['candidate_asset'] in records
print(json.dumps({'source_packages':len(packages),'source_files':source_files,'assets':len(records),
                  'collision_parts':sum(r['definition']['kind']=='collision' for r in records.values()),
                  'unique_glb_artifacts':len(seen_artifacts),'checks':'source hashes, revisions, dependency closure, GLB extents, origin normalization, collision owners, parents and collection pins'},indent=2))
