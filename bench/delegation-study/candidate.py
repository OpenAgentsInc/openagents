"""Bind every candidate effect, including deletions, to its retained payload."""
import hashlib
import gzip
import json
from pathlib import Path, PurePosixPath
import tarfile

from seed_manifest import digest
from capture_limits import Limits, Reader

MAX_PAYLOAD_BYTES=512*1024*1024



def container_limits(deadline,config):
    limits=Limits(config)
    limits.config['max_file_bytes']=limits.config['max_total_bytes']
    if deadline is not None:limits.deadline=deadline
    return limits


def bounded_digest(path,deadline,config):
    limits=container_limits(deadline,config)
    limits.entry(Path(path).stat().st_size)
    value=hashlib.sha256()
    with Path(path).open('rb') as source:
        reader=Reader(source,limits)
        while block:=reader.read(1024*1024):value.update(block)
    return value.hexdigest()


def write_manifest(output,source_commit,source_archive_sha256,changes,deadline=None,limits_config=None):
    output=Path(output)
    if (output/'candidate.tar.gz').stat().st_size>MAX_PAYLOAD_BYTES:raise ValueError('The candidate payload is too large')
    value={'schema':'openagents.delegation.candidate.v1','source_commit':source_commit,'source_archive_sha256':source_archive_sha256,'changes':changes,'payload_sha256':bounded_digest(output/'candidate.tar.gz',deadline,limits_config)}
    path=output/'candidate-manifest.json'
    path.write_text(json.dumps(value,sort_keys=True,separators=(',',':'))+'\n')
    identity=bounded_digest(path,deadline,limits_config)
    validate(output,identity,source_commit,source_archive_sha256,deadline,limits_config)
    return identity,value['payload_sha256']


def validate(output,expected_digest,source_commit=None,source_archive_sha256=None,deadline=None,limits_config=None):
    output=Path(output)
    path=output/'candidate-manifest.json'
    if path.is_symlink() or path.stat().st_size>32*1024*1024 or bounded_digest(path,deadline,limits_config)!=expected_digest:
        raise ValueError('The candidate manifest changed')
    value=json.loads(path.read_text())
    if value.get('schema')!='openagents.delegation.candidate.v1':raise ValueError('Unknown candidate schema')
    if source_commit is not None and value.get('source_commit')!=source_commit:raise ValueError('The candidate source commit changed')
    if source_archive_sha256 is not None and value.get('source_archive_sha256')!=source_archive_sha256:raise ValueError('The candidate source archive changed')
    changes=value['changes']
    if not isinstance(changes,dict) or len(changes)>100_000:raise ValueError('The candidate change set is invalid')
    for name in changes:
        relative=PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts or relative.as_posix()!=name or '.git' in relative.parts:
            raise ValueError('The candidate path is invalid')
    if json.loads((output/'changes.json').read_text())!=changes:raise ValueError('The candidate change sidecar differs from its identity')
    payload=output/'candidate.tar.gz'
    if payload.is_symlink() or payload.stat().st_size>MAX_PAYLOAD_BYTES or bounded_digest(payload,deadline,limits_config)!=value['payload_sha256']:raise ValueError('The candidate payload changed')
    expected={name:change['after'] for name,change in changes.items() if change['after'] is not None}
    stream_limits=container_limits(deadline,limits_config)
    entries=Limits(limits_config)
    entries.deadline=stream_limits.deadline
    seen=set()
    with gzip.open(payload,'rb') as source, tarfile.open(fileobj=Reader(source,stream_limits),mode='r|') as archive:
        for member in archive:
            entries.entry(member.size)
            if member.name in seen:raise ValueError('The candidate payload has duplicate entries')
            seen.add(member.name)
            after=expected.get(member.name)
            if after is None or member.mode!=after['mode']:raise ValueError('The candidate payload metadata changed')
            if after['kind']=='symlink':
                if not member.issym() or member.linkname!=after['target']:raise ValueError('The candidate symlink changed')
            elif after['kind']=='file':
                if not member.isfile() or member.size!=after['bytes']:raise ValueError('The candidate file changed')
                content=archive.extractfile(member);hasher=hashlib.sha256()
                while block:=content.read(1024*1024):
                    entries.check();hasher.update(block)
                if hasher.hexdigest()!=after['sha256']:raise ValueError('The candidate file contents changed')
            else:raise ValueError('The candidate has an unsupported file kind')
    if seen!=set(expected):raise ValueError('The candidate payload has extra or missing entries')
    return value
