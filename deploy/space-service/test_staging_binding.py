#!/usr/bin/env python3
"""Check the staging SPACE binding keeps Boat as default and admits one account."""
import hashlib
import unittest
import staging_binding


class StagingBinding(unittest.TestCase):
    def test_boat_stays_default_and_space_admits_only_the_test_account(self):
        config = staging_binding.registry('acct_4570d993d9897bdd', True)
        self.assertEqual(config['default'], 'oa-boat-primary')
        boat, space = config['deployments']
        self.assertTrue(boat['legacy'])
        self.assertNotIn('rollout_accounts', boat)
        digest = 'account:' + hashlib.sha256(b'openagents.web.chat.account.v1\0acct_4570d993d9897bdd').hexdigest()
        self.assertEqual(space['rollout_accounts'], [digest])
        self.assertTrue(space['admit_new'])
        self.assertFalse(staging_binding.registry('acct_4570d993d9897bdd', False)['deployments'][1]['admit_new'])

    def test_only_one_exact_account_is_accepted(self):
        for bad in ('chris@openagents.com', 'acct_*', 'acct_4570d993d9897bd'):
            with self.assertRaises(RuntimeError):
                staging_binding.owner(bad)


if __name__ == '__main__':
    unittest.main()
