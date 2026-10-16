#!/usr/bin/env python3
"""Check extraction, component namespacing, and public-front exclusions."""
import copy
import json
import unittest
import yaml
import systemone_openapi as source
import public_reference


class SystemOne(unittest.TestCase):
    def setUp(self):
        self.contract = yaml.load(source.SOURCE.read_text(), Loader=source.UniqueLoader)

    def test_public_slice_has_no_other_door_routes_or_external_references(self):
        document = source.generate(self.contract)
        self.assertEqual(list(document['paths']), ['/v1/systemone'])
        self.assertNotIn('SystemOneJobStatus', document['components']['schemas'])
        self.assertNotIn('SystemOneClassificationReport', document['components']['schemas'])
        self.assertEqual(document['paths']['/v1/systemone']['post']['operationId'], 'answerSystemOne')
        # The complete front includes both independent, unchanged qualified slices.
        app = json.loads((source.ROOT / 'docs/sdk/openapi.app.json').read_text())
        expected = sum(len(item) for item in app['paths'].values()) + 1
        for section, values in document['components'].items():
            app['components'].setdefault(section, {}).update(values)
        app['paths'].update(document['paths'])
        self.assertEqual(public_reference.check(app), expected)

    def test_referenced_owner_changes_propagate_without_copying_other_operations(self):
        changed = copy.deepcopy(self.contract)
        changed['components']['schemas']['Call']['description'] = 'Owner contract update'
        document = source.generate(changed)
        self.assertEqual(document['components']['schemas']['SystemOneCall']['description'], 'Owner contract update')
        self.assertNotIn('/v1/classify', document['paths'])
        self.assertNotIn('/v1/jobs', document['paths'])

    def test_unknown_external_refs_and_duplicate_keys_are_rejected(self):
        changed = copy.deepcopy(self.contract)
        changed['components']['schemas']['Call'] = {'$ref': 'private-or-unqualified.json'}
        with self.assertRaisesRegex(ValueError, 'local component'):
            source.generate(changed)
        with self.assertRaisesRegex(ValueError, 'Duplicate YAML'):
            yaml.load('field: first\nfield: second\n', Loader=source.UniqueLoader)

    def test_retry_and_response_metadata_match_the_owner(self):
        document = source.generate(self.contract)
        operation = document['paths']['/v1/systemone']['post']
        parameters = {p['name'] for p in operation['parameters']}
        self.assertEqual(parameters, {'X-Workspace-Id', 'Idempotency-Key', 'X-Attempt'})
        self.assertIn('Retry-After', operation['responses']['429']['headers'])
        self.assertEqual(set(operation['responses']['200']['headers']), {'x-request-id', 'x-attempt', 'x-outcome', 'x-receipt'})
        self.assertEqual(operation['security'], [{'SystemOneBearer': []}, {}])
        self.assertIn('idempotency_conflict', document['components']['schemas']['SystemOneTypedError']['properties']['error']['properties']['code']['examples'])


if __name__ == '__main__':
    unittest.main()
