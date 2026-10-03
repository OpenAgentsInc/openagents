"""Validate the exact files admitted into an executor's baseline Cargo cache."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat

SEED_POLICY='cargo-reported-libraries-v1'


def digest(path):
    value=hashlib.sha256()
    with Path(path).open('rb') as source:
        while block:=source.read(1024*1024):value.update(block)
    return value.hexdigest()


def inventory(root):
    files={}
    for path in sorted(root.rglob('*')):
        mode=path.lstat().st_mode
        if stat.S_ISDIR(mode):continue
        if not stat.S_ISREG(mode):raise ValueError('A seed entry is not a regular file')
        files[path.relative_to(root).as_posix()]={'sha256':digest(path),'bytes':path.stat().st_size,'mode':stat.S_IMODE(mode)}
    return files


def validate_seed(root,expected_digest,source_commit=None,source_archive_sha256=None,build_environment=None):
    root=Path(root)
    manifest=root/'seed-manifest.json'
    if manifest.is_symlink() or digest(manifest)!=expected_digest:
        raise ValueError('The target seed manifest changed')
    if manifest.stat().st_size>16*1024*1024:raise ValueError('The seed manifest is too large')
    value=json.loads(manifest.read_text())
    if value.get('schema')!='openagents.delegation.cargo-seed.v1':raise ValueError('Unknown seed schema')
    if value.get('seed_policy')!=SEED_POLICY:raise ValueError('The seed cache policy does not match')
    if build_environment is not None and value.get('build_environment')!=build_environment:
        raise ValueError('The seed build environment does not match')
    rustdoc=value.get('build_environment',{}).get('RUSTDOC')
    if rustdoc:
        if value.get('rustdoc_path')!=rustdoc or not re.fullmatch(r'[0-9a-f]{64}',value.get('rustdoc_sha256','')):
            raise ValueError('The seed has no matching rustdoc identity')
        if not isinstance(value.get('toolchain',{}).get('rustdoc'),str) or not value['toolchain']['rustdoc']:
            raise ValueError('The seed has no rustdoc version')
        if digest(Path(rustdoc))!=value['rustdoc_sha256']:
            raise ValueError('The rustdoc executable changed')
    if source_commit is not None and value.get('source_commit')!=source_commit:
        raise ValueError('The target seed belongs to a different source commit')
    if source_archive_sha256 is not None and value.get('source_archive_sha256')!=source_archive_sha256:
        raise ValueError('The target seed belongs to a different source archive')
    if not re.fullmatch(r'[0-9a-f]{40}',value.get('source_commit','')):
        raise ValueError('Invalid seed source commit')
    if not isinstance(value.get('files'),dict) or not value['files'] or len(value['files'])>100_000:
        raise ValueError('The seed has no bounded artifact file set')
    if not (root/'target').is_dir():raise ValueError('The seed target directory is missing')
    for name in value['files']:
        path=PurePosixPath(name)
        if path.is_absolute() or '..' in path.parts or path.as_posix()!=name:
            raise ValueError('The seed manifest contains an invalid path')
    if (root/'target').is_symlink() or inventory(root/'target')!=value.get('files'):
        raise ValueError('The target seed file set or content changed')
    return value
