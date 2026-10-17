#!/usr/bin/env python3
"""Count active objects and unfinished uploads in the two owned private buckets."""
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import socket
import xml.etree.ElementTree as ET
from cluster_secrets import identity, secret

PROJECT = 'openagentsgemini'
PREFIX = 'oa-space-service'


def protocol():
    spec = importlib.util.spec_from_file_location('space_storage_probe',
        Path(__file__).with_name('storage-probe.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def active_objects(store):
    cursor = None
    seen = set()
    count = size = 0
    for _ in range(100):
        query = {'list-type': 2, 'max-keys': 1000}
        if cursor:
            query['continuation-token'] = cursor
        page = ET.fromstring(store.request('GET', query=query)[1])
        rows = page.findall('{*}Contents')
        if len(rows) > 1000:
            raise RuntimeError('The object page exceeded its requested bound.')
        for row in rows:
            length = int(row.findtext('{*}Size'))
            if length < 0 or not row.findtext('{*}Key'):
                raise RuntimeError('The object inventory contains an invalid entry.')
            count += 1
            size += length
        if page.findtext('{*}IsTruncated') == 'false':
            return {'active_objects': count, 'active_bytes': size}
        cursor = page.findtext('{*}NextContinuationToken')
        if not cursor or cursor in seen:
            raise RuntimeError('The object inventory has an invalid continuation.')
        seen.add(cursor)
    raise RuntimeError('The object inventory exceeded its page budget.')


def unfinished_uploads(store):
    cursor = None
    seen = set()
    count = 0
    oldest = 0
    now = datetime.now(timezone.utc)
    for _ in range(100):
        query = {'uploads': '', 'max-uploads': 1000}
        if cursor:
            query.update({'key-marker': cursor[0], 'upload-id-marker': cursor[1]})
        page = ET.fromstring(store.request('GET', query=query)[1])
        rows = page.findall('{*}Upload')
        if len(rows) > 1000:
            raise RuntimeError('The upload page exceeded its requested bound.')
        for row in rows:
            if not row.findtext('{*}Key') or not row.findtext('{*}UploadId'):
                raise RuntimeError('The upload inventory contains an invalid entry.')
            created = datetime.fromisoformat(row.findtext('{*}Initiated').replace('Z', '+00:00'))
            oldest = max(oldest, int((now - created).total_seconds()))
            count += 1
        if page.findtext('{*}IsTruncated') == 'false':
            return {'unfinished_uploads': count, 'oldest_unfinished_upload_seconds': oldest,
                'unfinished_upload_bytes': None if count else 0}
        cursor = (page.findtext('{*}NextKeyMarker'), page.findtext('{*}NextUploadIdMarker'))
        if not all(cursor) or cursor in seen:
            raise RuntimeError('The upload inventory has an invalid continuation.')
        seen.add(cursor)
    raise RuntimeError('The upload inventory exceeded its page budget.')


def main():
    if os.getuid() != 0 or socket.gethostname().split('.')[0] != PREFIX + '-control':
        raise RuntimeError('Run storage inventory on the owned control node.')
    credential = secret('object-credentials', identity())
    client = protocol()
    buckets = []
    for purpose in ('templates', 'snapshots'):
        name = PROJECT + '-' + PREFIX + '-' + purpose
        store = client.Store(credential, name)
        buckets.append({'bucket': name, **active_objects(store), **unfinished_uploads(store)})
    print(json.dumps({'observed_at': datetime.now(timezone.utc).isoformat(),
        'buckets': buckets, 'includes_soft_deleted_objects': False,
        'mutations_performed': 0, 'credential_output': False}))


if __name__ == '__main__':
    main()
