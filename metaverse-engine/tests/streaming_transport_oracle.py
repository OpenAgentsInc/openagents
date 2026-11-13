# Firm reason: 2 (independent reconstruction of whole-container HTP1 metadata, source spans, page footprints and native/Wasm parity vectors).
import hashlib
import json
from pathlib import Path
import struct
import sys
from streaming_assets_oracle import pack,words

MODEL = b'HTP1/whole-vpk1-vpk2-1/raw-exact-1/hpk-initial8m-page64k-1'


def inspect(scene_dir, output, hosts=None, planned=None):
    scene_raw = (scene_dir / 'scene.json').read_bytes();scene = json.loads(scene_raw)
    descriptor_raw = (output / 'transport.json').read_bytes();descriptor = json.loads(descriptor_raw)
    assert descriptor['schema'] == 'aethyr.stream-transport-index.v1' and descriptor['containerLimit'] == 65536
    assert descriptor['sceneSHA256'] == hashlib.sha256(scene_raw).hexdigest()
    assert descriptor['modelSHA256'] == hashlib.sha256(MODEL).hexdigest() and len(descriptor['companions']) == 4
    result = [];all_containers = set();payloads = set()
    for source,companion in zip(scene['manifests'],descriptor['companions']):
        assert companion['manifestSHA256'] == source['sha256'] and companion['tier'] == source['tier'] and companion['backend'] == source['backend']
        manifest = (scene_dir / (source['sha256'] + '.rcm')).read_bytes()
        assert hashlib.sha256(manifest).hexdigest() == source['sha256'] and manifest[:4] == b'RCM1' and struct.unpack_from('<I',manifest,8)[0] == 64
        raw = (output / (companion['sha256'] + '.htp')).read_bytes()
        expected = struct.pack('<4sIII',b'HTP1',1,64,96) + words(descriptor['sceneSHA256']) + words(source['sha256']) + words(descriptor['modelSHA256'])
        costs = [];raw_count = 0
        for row,r in enumerate(scene['resources']):
            container = (scene_dir / (r['container_sha256'] + ('.raw' if r['raw'] else '.vpk'))).read_bytes()
            assert r['row'] == row and hashlib.sha256(container).hexdigest() == r['container_sha256'] and 0 < len(container) <= 65536
            source_bytes = container[r['offset']:r['offset'] + r['bytes']]
            assert len(source_bytes) == r['bytes'] and hashlib.sha256(source_bytes).hexdigest() == r['sha256']
            at = 48 + row * 440
            assert manifest[at+12:at+44] == words(r['sha256']) and struct.unpack_from('<q',manifest,at+44)[0] == len(source_bytes)
            if r['raw']:
                assert r['offset'] == 0 and source_bytes == container;raw_count += 1
            else:
                entries = pack(container)
                assert any(offset == r['offset'] and len(data) >= r['bytes'] for kind,offset,data in entries)
            expected += struct.pack('<II',int(r['raw']),0) + words(r['container_sha256']) + words(r['sha256']) + struct.pack('<qqq',len(container),r['offset'],r['bytes'])
            all_containers.add(r['container_sha256']);payloads.add(r['sha256'])
            # Four independently reconstructed layout observations, including
            # a heap at the initial extent which forces a real page boundary.
            for base in (0,16,8388592,8388608):
                end = ((base+15)//16)*16 + len(container)
                reactor = 8388608 if r['raw'] else max(8388608,((end+65535)//65536)*65536)
                external = len(container) if r['raw'] else 0
                reserved = reactor + ((external+65535)//65536)*65536
                costs.append([row,base,reactor,external,reserved])
        assert raw == expected and len(raw) == companion['bytes'] == 6256 and hashlib.sha256(raw).hexdigest() == companion['sha256']
        result.append({'tier':source['tier'],'backend':source['backend'],'sha256':companion['sha256'],'raw_rows':raw_count,'packed_rows':64-raw_count,'costs':costs})
    if hosts is not None:
        observed = list(map(int,(hosts / 'native.txt').read_text().splitlines()))
        source = bytes((1,0,0,0));source_sha = hashlib.sha256(source).hexdigest()
        encoded = struct.pack('<4sIII',b'HTP1',1,1,96) + words(hashlib.sha256(b'transport-parity-scene').hexdigest()) + words(hashlib.sha256(b'transport-parity-rcm').hexdigest()) + words(hashlib.sha256(MODEL).hexdigest())
        encoded += struct.pack('<II',1,0) + words(source_sha)*2 + struct.pack('<qqq',4,0,4)
        root = list(struct.unpack('>8I',hashlib.sha256(encoded).digest()));model = list(struct.unpack('>8I',hashlib.sha256(MODEL).digest()))
        expected = [0,512,488,12,8388608,0,8388608,0,8454144,0,8454144,1,8388608,4,8454144,0,8388608,67108864,75497472,0,1,0,0,2,4,6,*root,*model,208,96,32,112,96,6256]
        expected += [0,2,129,2,0,1,1,1,2,1,8454144,1,1,1,0,0]
        assert observed == expected and len(observed) == 64
    if planned is not None:
        initial = 128*65536
        def held(public,raw):
            return [public+raw,1+public+2*raw,initial*(1+public+raw)+raw*65536]
        expected = [512,5,0,2,initial*2,*held(0,0),4,0,3,initial*2+65536,1,*held(0,0),
                    *held(0,2),1,*held(0,2),*held(0,1),*held(0,0),1,*held(0,0),
                    *held(30,0),1,0,0,*held(30,0),*held(0,0),1,0,0,1,0,0,1,0,0,
                    1,3,initial*2+65536,1]
        assert json.loads(planned.read_bytes()) == expected and len(expected) == 58
    return {'schema':'aethyr.stream-transport-metadata-proof.v1','index_sha256':hashlib.sha256(descriptor_raw).hexdigest(),'companions':result,'unique_containers':len(all_containers),'unique_payloads':len(payloads),'rows_checked':256,'native_wasm_vector_words':64 if hosts else 0,'planned_browser_words':58 if planned else 0,'world_companion_adoption':False}


if __name__ == '__main__':
    args=list(map(Path,sys.argv[1:]));print(json.dumps(inspect(*args),sort_keys=True))
