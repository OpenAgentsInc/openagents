#!/usr/bin/env python3
"""Regression checks for public schema exposure and compatibility drift."""
import copy
import json
import unittest
import public_reference as reference


class PublicReference(unittest.TestCase):
    def setUp(self):
        self.document = json.loads((reference.ROOT / 'docs/sdk/openapi.app.json').read_text())
        systemone = json.loads((reference.ROOT / 'docs/sdk/openapi.systemone.json').read_text())
        self.document['paths'].update(systemone['paths'])
        for section, values in systemone['components'].items():
            self.document['components'].setdefault(section, {}).update(values)

    def test_qualified_fixture_and_public_front_patterns(self):
        self.assertGreater(reference.check(self.document), 0)
        self.assertTrue(reference.forwarded('/v1/workspaces/ws/keys', 'get'))
        self.assertTrue(reference.forwarded('/v1/workspaces/ws/keys/key', 'delete'))
        self.assertTrue(reference.forwarded('/v1/models', 'head'))
        self.assertFalse(reference.forwarded('/v1/models', 'post'))
        self.assertFalse(reference.forwarded('/v1/sessions/device/decide', 'post'))
        self.assertFalse(reference.forwarded('/v1/accounts', 'post'))

    def test_internal_and_unmounted_operations_cannot_be_published(self):
        for path in ['/v1/admin/inference/status', '/v1/sessions/device/decide', '/v1/not-implemented']:
            document = copy.deepcopy(self.document)
            document['paths'][path] = {'post': {'operationId': 'injected'}}
            with self.assertRaisesRegex(AssertionError, 'not served'):
                reference.check(document)

    def test_missing_duplicate_and_wrong_base_are_detected(self):
        document = copy.deepcopy(self.document)
        del document['paths']['/v1/device/code']
        with self.assertRaisesRegex(AssertionError, 'drift'):
            reference.check(document)
        document = copy.deepcopy(self.document)
        document['paths']['/v1/models'] = {'get': {'operationId': 'getAccount'}}
        with self.assertRaisesRegex(AssertionError, 'duplicate'):
            reference.check(document)
        document = copy.deepcopy(self.document)
        document['servers'][0]['url'] += '/v1'
        with self.assertRaisesRegex(AssertionError, 'already contain'):
            reference.check(document)
        document = copy.deepcopy(self.document)
        document['paths']['/models'] = {'get': {'operationId': 'listModels'}}
        with self.assertRaisesRegex(AssertionError, 'Mixed path'):
            reference.check(document)

    def test_broken_references_and_app_contract_changes_are_detected(self):
        document = copy.deepcopy(self.document)
        del document['components']['schemas']['AppAccount']
        with self.assertRaisesRegex(AssertionError, 'component drift'):
            reference.check(document)
        document = copy.deepcopy(self.document)
        document['paths']['/v1/device/code']['post']['responses'] = {}
        with self.assertRaisesRegex(AssertionError, 'drift'):
            reference.check(document)

    def test_key_management_authority_and_shapes_cannot_drift(self):
        self.assertEqual(self.document['paths']['/v1/keys']['post']['security'],
                         [{'appKeyManagementBearer': []}])
        self.assertEqual(self.document['components']['schemas']['AppManagedKeyApiListed']
                         ['properties']['created']['type'], ['string', 'null'])
        document = copy.deepcopy(self.document)
        document['paths']['/v1/keys']['post']['security'] = [{'appBearer': []}]
        with self.assertRaisesRegex(AssertionError, 'drift'):
            reference.check(document)
        document = copy.deepcopy(self.document)
        document['components']['schemas']['AppManagedKeyApiListed']['properties']['created'] = {'type': 'integer'}
        with self.assertRaisesRegex(AssertionError, 'component drift'):
            reference.check(document)


if __name__ == '__main__':
    unittest.main()
