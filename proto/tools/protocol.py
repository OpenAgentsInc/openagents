#!/usr/bin/env python3
"""Reproducible protocol recovery, module checks and offline ID migration.

This is bootstrap/build infrastructure. Coax runtime ingestion and bounded
production decoders are separate work, not simulated by these Python tools.
"""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CONFIG = json.loads((ROOT / 'modules.json').read_text())
REGISTRY = json.loads((ROOT / 'migrations/metareal-v1.json').read_text())


def canonical_id(value, kinds=('asset-profile', 'contract', 'record-format', 'bridge')):
    """Resolve metadata only. Never apply this to a signed body's bytes."""
    for row in REGISTRY['entries']:
        if row['kind'] in kinds and value in (row['legacy_id'], row['canonical_id']):
            return row['canonical_id']
    raise ValueError('Unknown or non-migratable ID')


def migrate_catalog(catalog):
    from openagents.engine.v1.assets_pb2 import AssetProfileCatalog
    if catalog.schema_version != 1 or not catalog.profiles:
        raise ValueError('Not a supported asset-profile catalog')
    migrated = AssetProfileCatalog()
    migrated.CopyFrom(catalog)  # preserve unknown fields and every required file
    seen = set()
    mappings = {e['legacy_id']: e['canonical_id'] for e in REGISTRY['entries']
                if e['kind'] == 'asset-profile' and e['automatic_metadata_migration']}
    for profile in migrated.profiles:
        profile.id = mappings.get(profile.id, profile.id)
        if not profile.id or profile.id in seen:
            raise ValueError('Profile ID collision; input is not modified')
        seen.add(profile.id)
    return migrated


def verify_recovery():
    recovered = json.loads((ROOT / 'recovery.json').read_text())
    for row in recovered['files']:
        path = ROOT.parent / row['path']
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != row['restored_sha256']:
            raise ValueError('Recovery manifest drift: ' + row['path'])
        immutable = row['path'].startswith(('proto/compatibility/', 'proto/third_party/'))
        if immutable and digest != row['source_sha256']:
            raise ValueError('Published snapshot or vendor source was altered: ' + row['path'])
    actual = {str(p.relative_to(ROOT.parent)) for p in (ROOT/'compatibility').rglob('*') if p.is_file()}
    recorded = {r['path'] for r in recovered['files'] if r['path'].startswith('proto/compatibility/')}
    if actual != recorded:
        raise ValueError('Unrecorded or missing compatibility snapshot')
    legacy = [e['legacy_id'] for e in REGISTRY['entries']]
    canonical = [e['canonical_id'] for e in REGISTRY['entries']]
    if len(set(legacy)) != len(legacy) or len(set(canonical)) != len(canonical):
        raise ValueError('Ambiguous migration registry')


def compiler():
    versions = json.loads((ROOT/'toolchain.json').read_text())
    command = os.environ.get('PROTOC') or shutil.which('protoc')
    if not command:
        raise ValueError('Install protoc ' + versions['protoc'] + '; see proto/README.md')
    actual = subprocess.check_output([command, '--version'], text=True).strip()
    if actual != 'libprotoc ' + versions['protoc']:
        raise ValueError('Set PROTOC to protoc ' + versions['protoc'] + '; got ' + actual)
    if importlib.metadata.version('protobuf') != versions['python-protobuf']:
        raise ValueError('Use proto/tools/requirements.txt with PROTO_PYTHON')
    return command


def compile_set(command, name, files, includes, output, python=False):
    target = output / (name + '.pb')
    args = [command]
    for include in includes:
        args.extend(['-I', str((ROOT / include).resolve())])
    standard = Path(command).resolve().parent.parent / 'include'
    if (standard / 'google/protobuf/descriptor.proto').exists():
        args.extend(['-I', str(standard)])
    args += ['--include_imports', '--descriptor_set_out=' + str(target)]
    if python:
        generated = output/'python'
        generated.mkdir(exist_ok=True)
        args += ['--python_out=' + str(generated)]
    args += [str((ROOT / file).resolve()) for file in files]
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode:
        raise ValueError(name + ': ' + result.stderr)
    return target


def module_paths(name, module):
    files = sorted(str(p.relative_to(ROOT)) for directory in module['roots']
                   for p in (ROOT/directory).rglob('*.proto'))
    excluded = set(module.get('exclude', []))
    if not excluded.issubset(files):
        raise ValueError('Missing excluded module schema: ' + name)
    files = [path for path in files if path not in excluded]
    if not files:
        raise ValueError('Empty module: ' + name)
    return files


def build_sets(command, output):
    artifacts = {}
    for name, module in CONFIG['modules'].items():
        files = module_paths(name, module)
        path = compile_set(command, name, files, module['includes'], output, python=name in ('owned', 'nostr'))
        if name in ('owned', 'nostr'):
            from google.protobuf import descriptor_pb2
            descriptor = descriptor_pb2.FileDescriptorSet.FromString(path.read_bytes())
            for file in descriptor.file:
                if file.name.startswith(('openagents/', 'nostr/')):
                    expected = '.'.join(Path(file.name).parts[:-1])
                    if file.package != expected:
                        raise ValueError('Namespace/path mismatch: ' + file.name)
            artifacts['descriptors/'+name+'.pb'] = path
        print(f'{name}: compiled {len(files)} schemas')
    for name, roots in CONFIG['profiles'].items():
        path = compile_set(command, name, roots, ['.'], output)
        from google.protobuf import descriptor_pb2
        check_profile(name, descriptor_pb2.FileDescriptorSet.FromString(path.read_bytes()))
        artifacts['descriptors/' + name + '.pb'] = path
    return artifacts


def check_profile(name, descriptor):
    prefixes = CONFIG['profile_namespaces'][name]
    roots = set(CONFIG['profiles'][name])
    if not roots.issubset({file.name for file in descriptor.file}):
        raise ValueError('Missing selected profile root: ' + name)
    for file in descriptor.file:
        if file.package.startswith('google.protobuf'):
            continue
        if not any(file.package.startswith(prefix) for prefix in prefixes):
            raise ValueError('Unexpected profile dependency: ' + name + ': ' + file.package)


def check_contracts(output):
    from compatibility import check, parse
    from google.protobuf.json_format import ParseDict
    current = parse((output/'owned.pb').read_bytes())
    for path in sorted((ROOT/'compatibility').glob('*.descriptor.pb')):
        errors = check(parse(path.read_bytes()), current)
        if errors:
            raise ValueError('\n'.join(errors))
    sys.path.insert(0, str(output/'python'))
    from openagents.engine.v1.assets_pb2 import AssetProfileCatalog
    previous = AssetProfileCatalog.FromString((ROOT/'compatibility/asset-profiles-v1-20261027.pb').read_bytes())
    current_profiles = ParseDict(json.loads((ROOT/'openagents/engine/v1/profiles.json').read_text()), AssetProfileCatalog())
    expected = migrate_catalog(previous)
    if current_profiles != expected:
        raise ValueError('Profile migration changed more than the approved IDs')
    path = output/'asset-profiles.pb'
    path.write_bytes(current_profiles.SerializeToString(deterministic=True))
    # Run standalone reference/conformance tests against freshly generated types.
    import unittest
    suite = unittest.defaultTestLoader.discover(str(ROOT/'tools'), pattern='test_*.py')
    result = unittest.TextTestRunner(verbosity=1).run(suite)
    if not result.wasSuccessful():
        raise ValueError('Protocol conformance checks failed')
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='mode', required=True)
    commands.add_parser('check')
    commands.add_parser('generate')
    commands.add_parser('list')
    ids = commands.add_parser('canonical-id')
    ids.add_argument('id')
    migrate = commands.add_parser('migrate-profile-catalog')
    migrate.add_argument('input', type=Path)
    migrate.add_argument('output', type=Path)
    args = parser.parse_args()
    if args.mode == 'list':
        print(json.dumps(CONFIG, indent=2))
        return
    verify_recovery()
    from check_nostr_sources import check as check_nostr_sources
    check_nostr_sources(ROOT.parent)
    if args.mode == 'canonical-id':
        print(canonical_id(args.id))
        return
    command = compiler()
    with tempfile.TemporaryDirectory(prefix='openagents-proto-') as directory:
        output = Path(directory)
        artifacts = build_sets(command, output)
        artifacts['data/asset-profiles.pb'] = check_contracts(output)
        # Resource generation uses the same freshly compiled owned schema.
        from google.protobuf import text_format
        from openagents.metareal.home.v1alpha1.home_pb2 import HomeConfiguration
        home = text_format.Parse((ROOT.parent/'verse/home.textproto').read_text(), HomeConfiguration())
        home_path = output/'Home.pb'
        home_path.write_bytes(home.SerializeToString(deterministic=True))
        artifacts['../apps/openagents-ios/OpenAgents/Resources/Home.pb'] = home_path
        if args.mode == 'migrate-profile-catalog':
            from openagents.engine.v1.assets_pb2 import AssetProfileCatalog
            from google.protobuf.message import DecodeError
            with args.input.open('rb') as source:
                data = source.read(1024 * 1024 + 1)
            if len(data) > 1024 * 1024:
                raise ValueError('Catalog exceeds offline migration byte limit')
            try:
                catalog = migrate_catalog(AssetProfileCatalog.FromString(data))
            except DecodeError as error:
                raise ValueError('Malformed asset-profile catalog') from error
            # Never overwrite input or a pre-existing output.
            with args.output.open('xb') as file:
                file.write(catalog.SerializeToString(deterministic=True))
            print('Migrated catalog; original input preserved')
            return
        for name, source in artifacts.items():
            target = ROOT / name
            if args.mode == 'generate':
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source.read_bytes())
            elif not target.is_file() or target.read_bytes() != source.read_bytes():
                raise ValueError('Generated descriptor/catalog drift: ' + name)
        print('Published compatibility, migration, module and consumer-profile checks passed')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error))
