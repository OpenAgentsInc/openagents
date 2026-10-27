#!/usr/bin/env python3
"""Generate C/Python bindings, descriptors, and a shared binary fixture."""
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from compatibility import check, parse

ROOT = Path(__file__).resolve().parents[1]
SCHEMAS = ROOT.parents[1] / 'proto'
MODE = sys.argv[1] if len(sys.argv) > 1 else 'check'
VERSIONS = json.loads((ROOT / 'toolchain.json').read_text())
PROTO = 'openagents/engine/v1/scene.proto'
PROTOS = sorted(str(p.relative_to(SCHEMAS)) for p in SCHEMAS.rglob('*.proto'))


def main():
    if MODE not in ('generate', 'check'):
        raise SystemExit('Choose generate or check')
    protoc = os.environ.get('PROTOC') or shutil.which('protoc')
    if not protoc or subprocess.check_output([protoc, '--version'], text=True).strip() != 'libprotoc ' + VERSIONS['protoc']:
        raise SystemExit('Set PROTOC to protoc ' + VERSIONS['protoc'])
    for package, key in [('nanopb', 'nanopb'), ('protobuf', 'python-protobuf')]:
        if importlib.metadata.version(package) != VERSIONS[key]:
            raise SystemExit('Install tools/requirements.txt in the selected Python environment')
    plugin = Path(sys.executable).parent / 'protoc-gen-nanopb'
    if not plugin.is_file():
        raise SystemExit('The selected Python environment needs protoc-gen-nanopb')
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory)
        c = output / 'generated/c'
        python = output / 'generated/python'
        c.mkdir(parents=True)
        python.mkdir(parents=True)
        env = {**os.environ, 'PATH': str(Path(sys.executable).parent) + os.pathsep + os.environ.get('PATH', '')}
        subprocess.run([protoc, '-I', str(SCHEMAS), '--plugin=protoc-gen-nanopb=' + str(plugin),
                        '--nanopb_out=-I' + str(SCHEMAS) + ':' + str(c), '--python_out=' + str(python),
                        '--descriptor_set_out=' + str(output / 'scene.descriptor.pb'), *PROTOS], check=True, env=env)
        descriptor = (output / 'scene.descriptor.pb').read_bytes()
        from contracts import generate as generate_contracts
        generate_contracts(descriptor, SCHEMAS, output)
        baseline = ROOT / 'compatibility/scene-v1.descriptor.pb'
        if not baseline.is_file():
            raise SystemExit('The published v1 descriptor is missing; do not regenerate the baseline')
        errors = [error for published in (ROOT / 'compatibility').glob('*.descriptor.pb')
                  for error in check(parse(published.read_bytes()), parse(descriptor))]
        if errors:
            raise SystemExit('\n'.join(errors))
        encoded = subprocess.check_output([protoc, '-I', str(SCHEMAS), '--encode=openagents.engine.v1.SceneMetadata', PROTO],
                                          input=(ROOT / 'fixtures/scene.textproto').read_bytes())
        (output / 'fixtures').mkdir()
        (output / 'fixtures/scene.pb').write_bytes(encoded)
        sys.path.insert(0, str(python))
        from openagents.engine.v1.scene_pb2 import SceneMetadata
        from google.protobuf.json_format import ParseDict
        (output / 'data').mkdir()
        grid = ParseDict(json.loads((ROOT / 'data/grid.json').read_text()), SceneMetadata())
        (output / 'data/grid.pb').write_bytes(grid.SerializeToString(deterministic=True))
        message = SceneMetadata.FromString(encoded)
        cases = {'scene.pb': True, 'scene-unknown.pb': True}
        (output / 'fixtures/scene-unknown.pb').write_bytes(encoded + b'\x98\x06\x01')
        def invalid(name, mutate):
            value = SceneMetadata()
            value.CopyFrom(message)
            mutate(value)
            filename = 'invalid-' + name + '.pb'
            (output / 'fixtures' / filename).write_bytes(value.SerializeToString(deterministic=True))
            cases[filename] = False
        invalid('version', lambda v: setattr(v, 'schema_version', 2))
        invalid('coordinates', lambda v: setattr(v, 'coordinates', 77))
        invalid('nan', lambda v: setattr(v.landmarks[0].position, 'x', float('nan')))
        invalid('missing-position', lambda v: v.landmarks[0].ClearField('position'))
        invalid('duplicate-id', lambda v: setattr(v.landmarks[1], 'id', v.landmarks[0].id))
        invalid('range', lambda v: setattr(v.landmarks[0].interaction, 'leave_m', 1))
        invalid('bounds', lambda v: setattr(v.colliders[0].max, 'x', -10))
        invalid('label-nul', lambda v: setattr(v.landmarks[0], 'label', 'PYLON\0'))
        invalid('long-label', lambda v: setattr(v.landmarks[0], 'label', 'x' * 65))
        def too_many(v):
            for n in range(15):
                landmark = v.landmarks.add()
                landmark.CopyFrom(message.landmarks[1])
                landmark.id = 'extra-' + str(n)
        invalid('count', too_many)
        at_limits = SceneMetadata()
        at_limits.CopyFrom(message)
        while len(at_limits.landmarks) < 16:
            item = at_limits.landmarks.add()
            item.CopyFrom(message.landmarks[0])
            item.id = 'landmark-' + str(len(at_limits.landmarks))
            item.label = 'x' * 64
        while len(at_limits.colliders) < 64:
            item = at_limits.colliders.add()
            item.CopyFrom(message.colliders[0])
            item.id = 'collider-' + str(len(at_limits.colliders))
        (output / 'fixtures/scene-at-limits.pb').write_bytes(at_limits.SerializeToString(deterministic=True))
        cases['scene-at-limits.pb'] = True
        for name, data in [('invalid-truncated.pb', encoded[:-1]), ('invalid-utf8.pb', encoded.replace(b'ORIGIN', b'\xffRIGIN')), ('invalid-size.pb', b'\0' * 16385)]:
            (output / 'fixtures' / name).write_bytes(data)
            cases[name] = False
        (output / 'fixtures/cases.json').write_text(json.dumps(cases, indent=2) + '\n')
        from portable_fixtures import generate
        generate(output)
        from state_fixtures import generate as generate_state
        generate_state(output)
        from profiles import generate as generate_profiles
        generate_profiles(SCHEMAS, output)
        files = [p for p in output.rglob('*') if p.is_file() and '__pycache__' not in p.parts]
        expected = {str(p.relative_to(output)) for p in files}
        checked = {str(p.relative_to(ROOT)) for folder in (ROOT / 'generated/c', ROOT / 'generated/python') if folder.exists() for p in folder.rglob('*') if p.is_file() and '__pycache__' not in p.parts}
        if MODE == 'check' and not checked.issubset(expected):
            raise SystemExit('Unexpected generated files: ' + str(checked - expected))
        for file in files:
            target = ROOT / file.relative_to(output)
            if MODE == 'generate':
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(file.read_bytes())
            elif not target.is_file() or target.read_bytes() != file.read_bytes():
                raise SystemExit('Generated file drift: ' + str(target.relative_to(ROOT)))
    print('Engine C/Python bindings, descriptor compatibility, and fixture: ' + MODE)


if __name__ == '__main__':
    main()
