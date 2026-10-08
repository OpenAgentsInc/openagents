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


def unsigned(data, position):
    value, shift = 0, 0
    while True:
        byte = data[position]
        position += 1
        value |= (byte & 127) << shift
        if byte < 128:
            return value, position
        shift += 7
        assert shift < 64


def encoded(value):
    result = bytearray()
    while True:
        byte, value = value & 127, value >> 7
        result.append(byte | (128 if value else 0))
        if not value:
            return result


def sections(data):
    assert data[:8] == b'\0asm\x01\0\0\0'
    position = 8
    while position < len(data):
        kind = data[position]
        length, position = unsigned(data, position + 1)
        end = position + length
        yield kind, data[position:end]
        position = end


def inspect_module(path):
    data = path.read_bytes()
    exports, imports = {}, []
    start = None
    startup_body = None
    for kind, body in sections(data):
        if kind in [2, 7]:
            count, position = unsigned(body, 0)
            for _ in range(count):
                names = []
                for _ in range(2 if kind == 2 else 1):
                    length, position = unsigned(body, position)
                    names.append(body[position:position + length].decode())
                    position += length
                entry_kind = body[position]
                position += 1
                index, position = unsigned(body, position)
                if kind == 2:
                    # This application imports functions only. A changed
                    # import contract requires review before staging.
                    assert entry_kind == 0, 'Review non-function WASM imports'
                    imports.append(names)
                else:
                    exports[names[0]] = (entry_kind, index)
            assert position == len(body)
        elif kind == 8:
            start, position = unsigned(body, 0)
            assert position == len(body)
        elif kind == 10 and '__wbindgen_start' in exports:
            count, position = unsigned(body, 0)
            target = exports['__wbindgen_start'][1] - len(imports)
            for number in range(count):
                length, position = unsigned(body, position)
                if number == target:
                    startup_body = body[position:position + length]
                position += length
    return {'exports':exports, 'implicit_start':start,
            'imports':imports, 'startup_body':startup_body}


def module(path):
    inspected = inspect_module(path)
    return list(inspected['exports']), inspected['implicit_start']


def remove_test_exports(source, destination):
    removed = []
    output = bytearray(b'\0asm\x01\0\0\0')
    for kind, body in sections(source.read_bytes()):
        if kind == 7:
            count, position = unsigned(body, 0)
            entries = []
            for _ in range(count):
                beginning = position
                length, position = unsigned(body, position)
                name = body[position:position + length].decode()
                position += length + 1  # Name and export kind.
                _, position = unsigned(body, position)
                if name in ['main', '_start', '__main_void']:
                    removed.append(name)
                else:
                    entries.append(body[beginning:position])
            body = encoded(len(entries)) + b''.join(entries)
        output.append(kind)
        output.extend(encoded(len(body)))
        output.extend(body)
    destination.write_bytes(output)
    return removed


def verify_startup(path, glue):
    inspected = inspect_module(path)
    exports = inspected['exports']
    assert exports.get('start', (None,))[0] == 0, 'Browser app start must be a function'
    assert exports.get('__wbindgen_start', (None,))[0] == 0, 'Missing glue startup function'
    assert inspected['implicit_start'] is None, 'Web glue must own initialization'
    assert not ({'main', '_start', '__main_void'} & set(exports)), 'Do not serve a Rust test entry'
    assert re.search(r'export function start\(\)\s*\{\s*wasm\.start\(\);\s*\}', glue)
    assert re.search(r'function __wbg_finalize_init\([^)]*\)\s*\{[^}]*wasm\.__wbindgen_start\(\);', glue)
    app = exports['start'][1]
    startup = exports['__wbindgen_start'][1]
    calls = []
    if app != startup:
        body = inspected['startup_body']
        assert body is not None, 'Startup must be a defined app function or wrapper'
        locals_count, position = unsigned(body, 0)
        assert locals_count == 0, 'Review changed startup wrapper locals'
        while body[position] == 0x10:  # Direct call.
            function, position = unsigned(body, position + 1)
            calls.append(function)
        assert body[position:] == b'\x0b', 'Review changed startup wrapper instructions'
        assert calls.count(app) == 1 and calls[-1] == app, 'Startup must call the browser app once'
        for function in calls[:-1]:
            assert function < len(inspected['imports'])
            assert inspected['imports'][function][1] == '__wbindgen_init_externref_table'
    return {'app_function':app, 'startup_function':startup,
            'wrapper_calls':calls, 'implicit_start':None, 'test_entry_exports':[]}


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
    assert any(row.get('reason') == 'build-finished' and row['success'] for row in rows)
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
        assert remote['compiler_exit'] == 0, 'Compilation must succeed before artifact validation'
    raw_exports, raw_start = module(artifact)
    raw_starts = [name for name in raw_exports
                  if re.fullmatch(r'start(?:_[a-f0-9]+)?', name)
                  and '__wbindgen_describe_' + name in raw_exports]
    assert len(raw_starts) == 1, 'Review the descriptor-backed browser start export'
    assert raw_start is None, 'Do not initialize an implicit Rust test harness entry'
    locked = re.search(r'name = "wasm-bindgen"\nversion = "([^"]+)"', (root/'Cargo.lock').read_text())[1]
    version = subprocess.check_output(['wasm-bindgen','--version'], text=True).strip().split()[-1]
    assert locked == version == '0.2.128', (locked, version)
    output.mkdir(parents=True, exist_ok=True)
    prepared = output/'app-input.wasm'
    removed = remove_test_exports(artifact, prepared)
    subprocess.run(['wasm-bindgen','--target','web','--no-typescript','--out-name','everglade_web',
                    '--out-dir',str(output),str(prepared)], check=True)
    wasm = output/'everglade_web_bg.wasm'
    glue = (output/'everglade_web.js').read_text()
    startup = verify_startup(wasm, glue)
    prepared_identity = identity(prepared)
    prepared.unlink()  # Serve only normal glue and its pruned app module.
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
              'artifact_kind':'test-compiled benchmark; export-only derivative',
              'compiler_exit':0, 'staging_validation_exit':0,
              'cargo_json':identity(Path(cargo_json)),
              'compiler_profile':artifacts[0]['profile'],
              'compiler_artifact':dict(identity(artifact), original_path=original_path,
                                       relocated_path=str(artifact) if relocated else None),
              'prepared_artifact':dict(prepared_identity, removed_test_exports=removed),
              'transformation':'Remove only named Rust test-entry exports before normal wasm-bindgen pruning',
              'wasm_bindgen':version, 'app_start_verified':True,
              'wasm':dict(identity(wasm), url='/everglade_web_bg.wasm'),
              'glue':dict(identity(output/'everglade_web.js'), url='/everglade_web.js'),
              'pack':public, 'kit':kit, 'licensed_input_storage':'scratch only',
              'raw_browser_start_export':raw_starts[0],
              'served_browser_start_exports':['start','__wbindgen_start'], 'startup':startup}
    (output/'water-w11-inputs.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record, indent=2))
