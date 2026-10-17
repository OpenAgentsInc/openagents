#!/usr/bin/env python3
"""Review legacy CAD objects without writing to the source or copying designs.

Uses the active gcloud credential. Evidence contains counts and digests only.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import urllib.parse
import urllib.request


class Objects:
    def __init__(self, bucket, maximum):
        self.base = 'https://storage.googleapis.com/storage/v1/b/' + urllib.parse.quote(bucket, safe='') + '/o'
        self.token = subprocess.check_output(['gcloud', 'auth', 'print-access-token'], text=True).strip()
        self.maximum = maximum
        self.bytes = 0

    def get(self, suffix='', query=None, maximum=2 * 1024 * 1024):
        url = self.base + suffix + '?' + urllib.parse.urlencode(query or {})
        request = urllib.request.Request(url, headers={'Authorization': 'Bearer ' + self.token})
        with urllib.request.urlopen(request, timeout=30) as response:
            data = response.read(maximum + 1)
        if len(data) > maximum:
            raise ValueError('An object exceeds the review limit')
        return data

    def listing(self, prefix, folders=False):
        query = {'prefix': prefix, 'maxResults': 1000,
                 'fields': 'prefixes,items(name,generation,size),nextPageToken'}
        if folders:
            query['delimiter'] = '/'
        results = []
        seen = set()
        while True:
            page = json.loads(self.get(query=query))
            results.extend(page.get('prefixes' if folders else 'items', []))
            if len(results) > 100_000:
                raise ValueError('Too many objects for one review')
            cursor = page.get('nextPageToken')
            if not cursor:
                return results
            if cursor in seen:
                raise ValueError('The object cursor did not advance')
            seen.add(cursor)
            query['pageToken'] = cursor

    def read(self, item):
        size = int(item['size'])
        self.bytes += size
        if size > 15 * 1024 * 1024 or self.bytes > self.maximum:
            raise ValueError('The CAD review exceeds its byte limit')
        data = self.get('/' + urllib.parse.quote(item['name'], safe=''),
                        {'alt': 'media', 'generation': item['generation']}, maximum=size)
        if len(data) != size:
            raise ValueError('The object size changed')
        return data

    def current_generation(self, name):
        return json.loads(self.get('/' + urllib.parse.quote(name, safe=''), {'fields': 'generation'}))['generation']


def review(objects, prefix):
    scopes = documents = versions = files = total = 0
    rows = []
    checked_indexes = []
    reviews = []
    # Delimited listings avoid reading chats, messages, or their attachments.
    owners = objects.listing(prefix.rstrip('/') + '/', folders=True)
    for owner in owners:
        if not re.fullmatch(re.escape(prefix.rstrip('/')) + r'/[0-9a-f]{64}/', owner):
            continue
        spaces = objects.listing(owner + 'cad/', folders=True)
        for space in spaces:
            if not re.fullmatch(re.escape(owner) + r'cad/(me|p-[A-Za-z0-9_-]{1,64})/', space):
                continue
            entries = objects.listing(space)
            by_name = {item['name']: item for item in entries}
            index_name = space + 'index.json'
            if index_name not in by_name:
                continue
            contents = {}
            scope_files = []
            for name, item in sorted(by_name.items()):
                data = objects.read(item)
                rows.append([name, item['generation'], len(data), hashlib.sha256(data).hexdigest()])
                scope_files.append({'key': name[len(prefix.rstrip('/')) + 1:], 'bytes': len(data),
                                    'sha256': hashlib.sha256(data).hexdigest()})
                files += 1
                total += len(data)
                if name.endswith('/doc.json') or name.endswith('/version.json') or name == index_name:
                    if len(data) > 2 * 1024 * 1024:
                        raise ValueError('A CAD record exceeds its limit')
                    contents[name] = json.loads(data)
            index = contents[index_name]
            if index.get('schema') != 'openagents.web.cad-index.v1' or len(index.get('docs', {})) > 100:
                raise ValueError('Unsupported CAD index')
            for identifier, doc in index['docs'].items():
                if not re.fullmatch(r'[a-z0-9][a-z0-9_-]{0,47}', identifier):
                    raise ValueError('Invalid CAD document name')
                count = doc.get('versions')
                if type(count) is not int or not 1 <= count <= 300:
                    raise ValueError('Invalid CAD version count')
                for number in range(1, count + 1):
                    at = space + identifier + '/v' + str(number)
                    content = contents.get(at + '/doc.json', {})
                    record = contents.get(at + '/version.json', {})
                    if content.get('kind') not in ('part.v1', 'assembly.v1') or record.get('n') != number:
                        raise ValueError('An indexed CAD version is missing or invalid')
                    versions += 1
                documents += 1
            scopes += 1
            report = {'scope': space[len(prefix.rstrip('/')) + 1:].rstrip('/'),
                      'documents': len(index['docs']), 'versions': sum(d['versions'] for d in index['docs'].values()),
                      'files': scope_files, 'total_bytes': sum(f['bytes'] for f in scope_files)}
            report_bytes = json.dumps(report, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
            reviews.append({'scope_sha256': hashlib.sha256(report['scope'].encode()).hexdigest(),
                            'review_sha256': hashlib.sha256(report_bytes).hexdigest(),
                            'documents': report['documents'], 'versions': report['versions'],
                            'files': len(scope_files), 'bytes': report['total_bytes']})
            checked_indexes.append((index_name, by_name[index_name]['generation']))
    for name, generation in checked_indexes:
        if objects.current_generation(name) != generation:
            raise ValueError('A CAD index changed during review; run it again')
    manifest = json.dumps(sorted(rows), separators=(',', ':'), ensure_ascii=False).encode()
    return {'scopes': scopes, 'documents': documents, 'versions': versions, 'files': files,
            'bytes': total, 'manifest_sha256': hashlib.sha256(manifest).hexdigest(),
            'reviews': reviews,
            'source_unchanged': True, 'copy_performed': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bucket', required=True)
    parser.add_argument('--prefix', default='conversations')
    parser.add_argument('--maximum-bytes', type=int, default=512 * 1024 * 1024)
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'[a-z0-9][a-z0-9._-]+', args.bucket) or not re.fullmatch(r'[A-Za-z0-9_-]+', args.prefix):
        parser.error('Choose an explicit bucket and object prefix')
    try:
        result = review(Objects(args.bucket, args.maximum_bytes), args.prefix)
    except Exception as error:
        raise SystemExit('CAD review failed (' + type(error).__name__ + '); no data changed.') from None
    evidence = {'schema': 'openagents.labs-cad-migration-review.v1',
                'checked_at': datetime.datetime.now(datetime.timezone.utc).isoformat(), **result}
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps(evidence, indent=2))


if __name__ == '__main__':
    main()
