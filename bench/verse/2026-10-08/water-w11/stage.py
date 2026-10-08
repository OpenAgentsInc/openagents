"""Stage the exact test-compiled browser application and private inputs.

Usage: stage.py CARGO_JSON SOURCE_SHA OUT_DIR PRIVATE_KIT [RELOCATED_ARTIFACT REMOTE_RECORD]
Cargo must emit JSON from the filtered wasm32 --lib --release --no-run test.
Run after compilation, outside a measurement interval. OUT_DIR is scratch.
"""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys


def module(path):
    data = path.read_bytes()
    assert data[:8] == b'\0asm\x01\0\0\0', path
    position = 8

    def integer():
        nonlocal position
        value, shift = 0, 0
        while True:
            byte = data[position]
            position += 1
            value |= (byte & 127) << shift
            if byte < 128:
                return value
            shift += 7
            assert shift < 64

    names = []
    start = None
    while position < len(data):
        section = data[position]
        position += 1
        size = integer()
        end = position + size
        if section == 7:
            for _ in range(integer()):
                length = integer()
                names.append(data[position:position + length].decode())
                position += length + 1  # Export name, then its kind.
                integer()  # Export index.
            assert position == end
        elif section == 8:
            start = integer()
            assert position == end
        position = end
    return names, start


def pin(source, prefix):
    return {'sha256':re.search(fr'pub const {prefix}_SHA256: &str = "([a-f0-9]+)"', source)[1],
            'bytes':int(re.search(fr'pub const {prefix}_BYTES: u64 = ([0-9]+)', source)[1])}


def identity(path):
    return {'sha256':hashlib.sha256(path.read_bytes()).hexdigest(), 'bytes':path.stat().st_size}


if __name__ == '__main__':
    cargo_json, source_sha, output, private_kit = sys.argv[1:5]
    relocated = sys.argv[5:]
    assert len(relocated) in [0, 2], 'Supply both the relocated artifact and original remote record'
    root = Path(__file__).resolve().parents[4]
    output, private_kit = Path(output).resolve(), Path(private_kit).resolve()
    assert subprocess.check_output(['git','rev-parse','HEAD'], cwd=root, text=True).strip() == source_sha
    scratch = Path(subprocess.check_output(['openagents','scratch'], cwd=root, text=True).strip()).resolve()
    assert output.is_relative_to(scratch), 'Stage browser inputs only in assigned scratch'
    assert not private_kit.is_relative_to(root), 'Licensed input must remain outside Git'
    rows = [json.loads(line) for line in Path(cargo_json).read_text().splitlines() if line.startswith('{')]
    artifacts = [row for row in rows if row.get('reason') == 'compiler-artifact'
                 and row['target']['name'] == 'everglade_web' and row['profile']['test']
                 and row.get('executable') and row['executable'].endswith('.wasm')]
    assert len(artifacts) == 1, artifacts
    original_path = artifacts[0]['executable']
    artifact = Path(original_path)
    if relocated:
        artifact = Path(relocated[0]).resolve()
        remote = json.loads(Path(relocated[1]).read_text())
        assert artifact.is_relative_to(scratch), 'Copy the artifact only to assigned scratch'
        assert remote['source_sha'] == source_sha and remote['original_path'] == original_path, remote
        assert hashlib.sha256(Path(cargo_json).read_bytes()).hexdigest() == remote['cargo_json_sha256']
        assert identity(artifact) == {'sha256':remote['sha256'], 'bytes':remote['bytes']}, remote
    raw_exports, raw_start = module(artifact)
    assert 'start' in raw_exports, 'Test compilation must preserve the browser start export'
    assert raw_start is None, 'Do not initialize an implicit Rust test harness entry'
    locked = re.search(r'name = "wasm-bindgen"\nversion = "([^"]+)"', (root/'Cargo.lock').read_text())[1]
    version = subprocess.check_output(['wasm-bindgen','--version'], text=True).strip().split()[-1]
    assert locked == version == '0.2.128', (locked, version)
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run(['wasm-bindgen','--target','web','--no-typescript','--out-name','everglade_web',
                    '--out-dir',str(output),str(artifact)], check=True)
    wasm = output/'everglade_web_bg.wasm'
    final_exports, final_start = module(wasm)
    glue = (output/'everglade_web.js').read_text()
    assert 'start' in final_exports and '__wbindgen_start' in final_exports, final_exports
    assert 'wasm.__wbindgen_start(' in glue, 'Normal glue must initialize the browser app'
    assert final_start is None, 'Generated web glue must own the single initialization call'
    assert not ({'main','_start','__main_void'} & set(final_exports)), 'Do not serve a Rust test harness entry'
    public = pin((root/'crates/verse-zone-everglade/src/zones/everglade_pack.rs').read_text(), 'PACK')
    kit = pin((root/'crates/verse-zone-everglade/src/zones/everglade_pack/kit.rs').read_text(), 'KIT')
    for kind, pinned, source in [('pack',public,root/f"assets/verse/everglade/{public['sha256']}.vtp"),
                                 ('kit',kit,private_kit)]:
        assert identity(source) == pinned, (kind, identity(source), pinned)
        pinned['url'] = f"/everglade/{kind}/{pinned['sha256']}.vtp"
        destination = output/pinned['url'].lstrip('/')
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    shutil.copyfile(root/'crates/everglade-web/index.html', output/'index.html')
    record = {'source_sha':source_sha,
              'compiler_artifact':dict(identity(artifact), original_path=original_path,
                                       relocated_path=str(artifact) if relocated else None),
              'wasm_bindgen':version, 'app_start_verified':True,
              'wasm':dict(identity(wasm), url='/everglade_web_bg.wasm'),
              'glue':dict(identity(output/'everglade_web.js'), url='/everglade_web.js'),
              'pack':public, 'kit':kit, 'licensed_input_storage':'scratch only',
              'raw_browser_start_export':'start', 'served_browser_start_exports':['start','__wbindgen_start']}
    (output/'water-w11-inputs.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record, indent=2))
