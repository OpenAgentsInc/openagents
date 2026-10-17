#!/usr/bin/env python3
"""Count GCS object generations without reading or changing their contents."""
from datetime import datetime, timezone
import hashlib
import json
from urllib.parse import urlencode


def unsigned(value, *, positive=False):
    if (not isinstance(value, str) or len(value) > 20
            or not value.isascii() or not value.isdecimal()):
        raise RuntimeError('The object inventory contains an invalid number.')
    number = int(value)
    if number > 2**64 - 1 or (positive and number == 0):
        raise RuntimeError('The object inventory contains an invalid number.')
    return number


def timestamp(value):
    if not isinstance(value, str):
        raise RuntimeError('The object inventory contains an invalid date.')
    try:
        parsed = datetime.fromisoformat(value.replace('Z', '+00:00'))
    except ValueError:
        raise RuntimeError('The object inventory contains an invalid date.') from None
    if parsed.tzinfo is None:
        raise RuntimeError('The object inventory contains an invalid date.')
    return parsed


def inventory_objects(fetch, bucket, *, page_budget=100):
    started = datetime.now(timezone.utc).isoformat()
    totals = {kind + suffix: 0 for kind in ('active', 'noncurrent', 'soft_deleted')
              for suffix in ('_objects', '_bytes')}
    digest = hashlib.sha256()
    digest.update(json.dumps(['gcs-object-inventory-v1', bucket]).encode() + b'\n')
    identities = set()
    prefixes = set()
    latest_expiry = None
    pages = 0
    # GCS forbids combining versions=true with softDeleted=true.
    for soft_deleted in (False, True):
        cursor = None
        seen = set()
        for _ in range(page_budget):
            query = {'maxResults': 1000,
                     'fields': 'nextPageToken,items(name,generation,size,timeDeleted,softDeleteTime,hardDeleteTime,restoreToken)',
                     'softDeleted' if soft_deleted else 'versions': 'true'}
            if cursor:
                query['pageToken'] = cursor
            page = fetch('GET', 'b/' + bucket + '/o?' + urlencode(query))
            if not isinstance(page, dict):
                raise RuntimeError('The object inventory contains an invalid page.')
            rows = page.get('items', [])
            if not isinstance(rows, list) or len(rows) > 1000:
                raise RuntimeError('The object page exceeded its requested bound.')
            pages += 1
            for row in rows:
                if not isinstance(row, dict) or not isinstance(row.get('name'), str) or not row['name']:
                    raise RuntimeError('The object inventory contains an invalid entry.')
                generation = unsigned(row.get('generation'), positive=True)
                size = unsigned(row.get('size'))
                restore = row.get('restoreToken', '')
                if not isinstance(restore, str):
                    raise RuntimeError('The object inventory contains an invalid entry.')
                identity = (row['name'], generation, restore)
                if identity in identities:
                    raise RuntimeError('The object inventory repeated a generation; collect it again.')
                identities.add(identity)
                kind = 'active'
                if soft_deleted:
                    deleted = timestamp(row.get('softDeleteTime'))
                    expiry = timestamp(row.get('hardDeleteTime'))
                    if expiry <= deleted:
                        raise RuntimeError('The object inventory contains an invalid recovery period.')
                    latest_expiry = max(latest_expiry, expiry) if latest_expiry else expiry
                    kind = 'soft_deleted'
                elif row.get('softDeleteTime') is not None:
                    raise RuntimeError('The object inventory returned an unexpected deleted generation.')
                elif row.get('timeDeleted') is not None:
                    timestamp(row['timeDeleted'])
                    kind = 'noncurrent'
                totals[kind + '_objects'] += 1
                totals[kind + '_bytes'] += size
                if kind == 'active':
                    prefixes.add(row['name'].split('/', 1)[0])
                digest.update(json.dumps([kind, *identity, size, row.get('timeDeleted'),
                    row.get('softDeleteTime'), row.get('hardDeleteTime')],
                    separators=(',', ':'), ensure_ascii=True).encode() + b'\n')
            cursor = page.get('nextPageToken')
            if cursor is None:
                break
            if not isinstance(cursor, str) or not cursor or cursor in seen:
                raise RuntimeError('The object inventory has an invalid continuation.')
            seen.add(cursor)
        else:
            raise RuntimeError('The object inventory exceeded its page budget; collect it again.')
    return {**totals, 'top_level_prefixes': sorted(prefixes), 'metadata_sha256': digest.hexdigest(),
            'listing_started_at': started, 'listing_finished_at': datetime.now(timezone.utc).isoformat(),
            'listing_pages': pages, 'object_metadata_complete': True,
            'latest_soft_delete_expiry': latest_expiry.isoformat() if latest_expiry else None,
            'includes_noncurrent_versions': True, 'includes_soft_deleted_objects': True,
            'includes_unfinished_uploads': False,
            'storage_cleanup_proof': False}
