#!/usr/bin/env python3
"""Exercise read-only storage accounting with synthetic provider responses."""
import copy
from contextlib import nullcontext
import json
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlsplit
from object_inventory import inventory_objects


def object_row(**overrides):
    return {'name': 'fixture/private-object', 'generation': '1', 'size': '7', **overrides}


class Pages:
    def __init__(self, *pages):
        self.pages = list(pages)
        self.queries = []

    def __call__(self, method, path):
        assert method == 'GET'
        assert path.startswith('b/owned-fixture/o?')
        query = parse_qs(urlsplit(path).query)
        assert query['maxResults'] == ['1000']
        assert not ('versions' in query and 'softDeleted' in query)
        self.queries.append(query)
        return self.pages.pop(0)


class ObjectInventoryTests(unittest.TestCase):
    def test_paged_versions_and_recoverable_bytes_are_counted_separately(self):
        pages = Pages(
            {'items': [object_row()], 'nextPageToken': 'next /?&'},
            {'items': [object_row(generation='2', size='11', timeDeleted='2026-10-17T01:00:00Z')]},
            {'items': [object_row(generation='3', size='13',
                softDeleteTime='2026-10-17T01:00:00Z', hardDeleteTime='2026-10-24T01:00:00Z')]},
        )
        result = inventory_objects(pages, 'owned-fixture')
        for kind, size in [('active', 7), ('noncurrent', 11), ('soft_deleted', 13)]:
            self.assertEqual(result[kind + '_objects'], 1)
            self.assertEqual(result[kind + '_bytes'], size)
        self.assertEqual(pages.queries[0]['versions'], ['true'])
        self.assertEqual(pages.queries[1]['pageToken'], ['next /?&'])
        self.assertEqual(pages.queries[2]['softDeleted'], ['true'])
        self.assertEqual(result['listing_pages'], 3)
        self.assertTrue(result['object_metadata_complete'])
        self.assertFalse(result['includes_unfinished_uploads'])
        self.assertFalse(result['storage_cleanup_proof'])
        self.assertNotIn('private-object', json.dumps(result))
        self.assertEqual(result['latest_soft_delete_expiry'], '2026-10-24T01:00:00+00:00')

    def test_empty_pages_can_continue_and_empty_buckets_still_need_both_listings(self):
        pages = Pages({'nextPageToken': 'empty'}, {}, {})
        result = inventory_objects(pages, 'owned-fixture')
        self.assertEqual(len(pages.queries), 3)
        self.assertEqual(result['soft_deleted_bytes'], 0)
        self.assertTrue(result['includes_noncurrent_versions'])
        self.assertTrue(result['includes_soft_deleted_objects'])

    def test_digest_binds_the_bucket_and_exact_generation_metadata(self):
        def collect(bucket, rows):
            pages = iter([{'items': rows}, {}])
            return inventory_objects(lambda *_: next(pages), bucket)['metadata_sha256']
        first = collect('first', [object_row()])
        self.assertEqual(first, collect('first', [object_row()]))
        self.assertNotEqual(first, collect('second', [object_row()]))
        self.assertNotEqual(first, collect('first', [object_row(size='8')]))
        self.assertNotEqual(first, collect('first', [object_row(generation='2')]))

    def test_duplicate_generations_in_a_page_or_across_listings_are_refused(self):
        for pages in [Pages({'items': [object_row(), object_row()]}),
                      Pages({'items': [object_row()]}, {'items': [object_row()]})]:
            with self.subTest(pages=pages), self.assertRaises(RuntimeError):
                inventory_objects(pages, 'owned-fixture')

    def test_cycling_empty_or_invalid_continuations_are_refused(self):
        for pages in [Pages({'nextPageToken': 'cycle'}, {'nextPageToken': 'cycle'}),
                      Pages({'nextPageToken': ''}), Pages({'nextPageToken': 1})]:
            with self.subTest(pages=pages), self.assertRaises(RuntimeError):
                inventory_objects(pages, 'owned-fixture')

    def test_page_budget_and_response_bounds_are_enforced(self):
        for pages, budget in [(Pages({'nextPageToken': 'more'}), 1),
                              (Pages({'items': [object_row()] * 1001}), 100)]:
            with self.subTest(budget=budget), self.assertRaises(RuntimeError):
                inventory_objects(pages, 'owned-fixture', page_budget=budget)

    def test_invalid_metadata_never_becomes_zero_usage(self):
        variants = [{}, {'name': ''}, {'generation': '0'}, {'size': '-1'},
                    {'size': True}, {'size': '1.5'}, {'size': str(2**64)},
                    {'size': '9' * 5000}, {'generation': None}, {'restoreToken': []},
                    {'timeDeleted': '2026-10-17'}, {'softDeleteTime': '2026-10-17T00:00:00Z'}]
        for variant in variants:
            row = object_row(**variant) if variant else {'name': 'fixture/private-object'}
            with self.subTest(variant=list(variant)), self.assertRaises(RuntimeError) as error:
                inventory_objects(Pages({'items': [row]}, {}), 'owned-fixture')
            self.assertNotIn('private-object', str(error.exception))

    def test_deleted_generations_require_valid_recovery_dates(self):
        base = object_row(softDeleteTime='2026-10-17T00:00:00Z', hardDeleteTime='2026-10-24T00:00:00Z')
        for change in [{'softDeleteTime': None}, {'hardDeleteTime': None},
                       {'hardDeleteTime': '2026-10-16T00:00:00Z'},
                       {'hardDeleteTime': '2026-10-24T00:00:00'}]:
            row = copy.deepcopy(base)
            row.update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                inventory_objects(Pages({}, {'items': [row]}), 'owned-fixture')

    def test_malformed_pages_are_refused(self):
        for page in [None, [], {'items': None}, {'items': {}}, {'items': [None]}]:
            with self.subTest(page=page), self.assertRaises(RuntimeError):
                inventory_objects(Pages(page), 'owned-fixture')

    def test_provider_failure_is_not_reported_as_an_empty_bucket(self):
        calls = 0
        def fetch(*_):
            nonlocal calls
            calls += 1
            if calls == 2:
                raise RuntimeError('Synthetic provider refusal.')
            return {}
        with self.assertRaisesRegex(RuntimeError, 'Synthetic provider refusal'):
            inventory_objects(fetch, 'owned-fixture')

    def test_foreign_bucket_is_refused_before_listing_and_private_port_is_cleared(self):
        import inventory
        for metadata in [{'name': 'foreign', 'labels': inventory.storage.LABELS},
                         {'name': inventory.PROJECT + '-' + inventory.PREFIX + '-templates', 'labels': {}}]:
            with self.subTest(metadata=metadata), \
                 patch.object(inventory.storage, 'private_tunnel', return_value=nullcontext(12345)), \
                 patch.object(inventory.storage, 'api', return_value=metadata) as api:
                with self.assertRaisesRegex(RuntimeError, 'ownership differs'):
                    inventory.objects()
                self.assertEqual(api.call_count, 1)
                self.assertIsNone(inventory.storage.PRIVATE_PORT)


if __name__ == '__main__':
    unittest.main()
