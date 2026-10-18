"""Contract drift and deterministic-generation checks; no network or credentials."""
import copy
import json
import unittest

import check_computers_public as public
import computers_codegen as codegen


class ComputersContract(unittest.TestCase):
    def setUp(self):
        self.raw = codegen.SPEC.read_bytes()
        self.spec = json.loads(self.raw)

    def test_identical_public_contract_is_accepted(self):
        operations = public.compare_contract(self.spec, self.spec, 'https://api.openagents.com')
        self.assertEqual(len(operations), 21)

    def test_changed_runtime_operation_is_refused(self):
        changed = copy.deepcopy(self.spec)
        changed['paths']['/v1/computers/{id}/files']['put']['parameters'] = []
        with self.assertRaisesRegex(AssertionError, 'Operation drift'):
            public.compare_contract(changed, self.spec, 'https://api.openagents.com')

    def test_changed_runtime_component_is_refused(self):
        changed = copy.deepcopy(self.spec)
        changed['components']['schemas']['AppComputer']['required'].append('unknown_required_field')
        with self.assertRaisesRegex(AssertionError, 'Component drift'):
            public.compare_contract(changed, self.spec, 'https://api.openagents.com')

    def test_duplicate_operation_and_missing_reference_are_refused(self):
        changed = copy.deepcopy(self.spec)
        changed['paths']['/v1/computers']['post']['operationId'] = 'listComputers'
        with self.assertRaisesRegex(AssertionError, 'Duplicate operation'):
            codegen.validate(changed)
        changed = copy.deepcopy(self.spec)
        changed['components']['schemas']['AppCommandOutput']['properties']['command']['$ref'] = '#/components/schemas/Missing'
        with self.assertRaisesRegex(AssertionError, 'Missing component'):
            codegen.validate(changed)

    def test_internal_routes_and_wrong_auth_are_refused(self):
        changed = copy.deepcopy(self.spec)
        changed['paths']['/v1/accounts'] = changed['paths'].pop('/v1/account')
        with self.assertRaisesRegex(AssertionError, 'Internal or unrelated'):
            codegen.validate(changed)
        changed = copy.deepcopy(self.spec)
        changed['paths']['/v1/computers']['post']['security'] = []
        with self.assertRaisesRegex(AssertionError, 'credential boundary'):
            codegen.validate(changed)

    def test_generation_is_deterministic_and_binary_transport_is_explicit(self):
        first = codegen.generate(self.raw)
        self.assertEqual(first, codegen.generate(self.raw))
        index = json.loads(first[codegen.INDEX])
        upload = next(op for op in index['operations'] if op['operation_id'] == 'writeComputerFile')
        download = next(op for op in index['operations'] if op['operation_id'] == 'readComputerFile')
        self.assertTrue(upload['binary_request'])
        self.assertTrue(download['binary_response'])
        self.assertIn('RequestBody::Binary(self.body)', first[codegen.OUT])
        self.assertIn('pub state: String', first[codegen.OUT])


if __name__ == '__main__':
    unittest.main()
