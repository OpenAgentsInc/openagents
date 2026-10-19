"""Regression checks for the relay's Cloud Build inputs and caches."""
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class RelayBuildTests(unittest.TestCase):
    def test_relay_upload_includes_workspace_inputs(self):
        files = set(subprocess.check_output(
            ['gcloud', 'meta', 'list-files-for-upload'], cwd=ROOT, text=True
        ).splitlines())
        required = ['Cargo.toml', 'Cargo.lock', 'apps/openagents/Cargo.toml',
                    'scripts/deploy/cargo-skeleton.sh',
                    'scripts/deploy/cloud-buildx.sh', 'scripts/deploy/oa-kache.sh']
        required += [str(p.relative_to(ROOT)) for p in
                     (ROOT / 'apps/openagents/crates').rglob('Cargo.toml')]
        self.assertTrue(set(required) <= files, set(required) - files)
        self.assertNotIn('apps/openagents/Cargo.lock', files)
        self.assertNotIn('apps/openagents/ARCHITECTURE.md', files)

    def test_relay_uses_dependency_and_remote_caches(self):
        dockerfile = (ROOT / 'Dockerfile').read_text()
        command = 'cargo build --locked --release -p nostr-relay --bin nostr-relay'
        self.assertEqual(dockerfile.count(command), 2)
        self.assertEqual(dockerfile.count('oa-kache run ' + command), 2)
        self.assertIn('FROM scratch AS deps', dockerfile)
        self.assertIn('FROM builder-deps AS builder', dockerfile)
        config = (ROOT / 'deploy/nostr-relay.cloudbuild.yaml').read_text()
        self.assertIn('scripts/deploy/cloud-buildx.sh', config)
        self.assertNotIn("args: ['build'", config)


if __name__ == '__main__':
    unittest.main()
