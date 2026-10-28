#!/usr/bin/env python3
"""Regression checks for source bytes, full inventory and local index policy."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
import nips

class PinTests(unittest.TestCase):
    def test_full_tree_preserves_fixtures_and_nested_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            repo=Path(directory)
            subprocess.run(['git','init','-q',str(repo)],check=True)
            for path,data in {'docs/nips/NIP-X.md':b'spec\r\n',
                              'docs/nips/NIP-X.fixtures.json':b'{"case":1}\n',
                              'docs/nips/data/vector.bin':b'\0\xff\n',
                              'unrelated.md':b'not a NIP'}.items():
                f=repo/path;f.parent.mkdir(parents=True,exist_ok=True);f.write_bytes(data)
            subprocess.run(['git','-C',str(repo),'add','.'],check=True)
            subprocess.run(['git','-C',str(repo),'-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','fixture'],check=True)
            revision=nips.git(repo,'rev-parse','HEAD').decode().strip()
            files=nips.read_tree(repo,revision,'docs/nips')
            self.assertEqual(set(files),{'NIP-X.md','NIP-X.fixtures.json','data/vector.bin'})
            self.assertEqual(files['NIP-X.md'],b'spec\r\n')
            self.assertEqual(files['data/vector.bin'],b'\0\xff\n')
    def test_missing_changed_and_stray_files_fail(self):
        expected={'01.md':b'spec','vectors.json':b'{}'}
        for actual in [{}, {'01.md':b'changed','vectors.json':b'{}'},
                       {**expected,'stray.txt':b'extra'}]:
            with self.subTest(actual=actual),self.assertRaises(ValueError): nips.compare(actual,expected,'official')
        nips.compare(expected,expected,'official')
    def test_local_readme_is_only_allowed_in_block(self):
        nips.compare({'NIP-X.md':b'spec','README.md':b'local'},{'NIP-X.md':b'spec'},'block')
        with self.assertRaises(ValueError):nips.compare({'01.md':b'spec','README.md':b'local'},{'01.md':b'spec'},'official')
        with self.assertRaises(ValueError):nips.compare({'README.md':b'local'},{'README.md':b'upstream'},'block')
    def test_replace_removes_stale_files_and_upstream_readme_wins(self):
        with tempfile.TemporaryDirectory() as directory:
            lane=Path(directory)/'block';lane.mkdir();(lane/'README.md').write_bytes(b'local');(lane/'old.md').write_bytes(b'old')
            nips.write_lane(lane,{'NIP-X.md':b'new'},'block')
            self.assertEqual(nips.lane_files(lane),{'NIP-X.md':b'new','README.md':b'local'})
            nips.write_lane(lane,{'README.md':b'upstream','NIP-Y.md':b'next'},'block')
            self.assertEqual(nips.lane_files(lane),{'README.md':b'upstream','NIP-Y.md':b'next'})
    def test_symlinks_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            lane=Path(directory);(lane/'link').symlink_to('/tmp')
            with self.assertRaises(ValueError):nips.lane_files(lane)
    def test_nonexact_revision_fails(self):
        with self.assertRaises(ValueError):nips.read_tree(Path('.'),'HEAD','.')

if __name__=='__main__':unittest.main()
