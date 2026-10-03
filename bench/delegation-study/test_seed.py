import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import seed
from seed_manifest import digest,inventory,validate_seed


class SeedTests(unittest.TestCase):
    def test_only_reported_baseline_graph_enters_seed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();workspace=root/'workspace';workspace.mkdir()
            (workspace/'Cargo.toml').write_text('[package]\nname="base"\nversion="0.1.0"\n')
            target=root/'target';target.mkdir()
            exact=['debug/deps/libbase-0123456789abcdef.rlib','debug/deps/base-0123456789abcdef.d','debug/.fingerprint/base-0123456789abcdef/lib-base','debug/.fingerprint/base-0123456789abcdef/lib-base.json','debug/.fingerprint/base-0123456789abcdef/dep-lib-base','debug/.fingerprint/base-0123456789abcdef/invoked.timestamp']
            forbidden=['debug/deps/libfuture_checker-1111111111111111.rlib','debug/.fingerprint/base-0123456789abcdef/output-lib-base','debug/.fingerprint/base-0123456789abcdef/future-checker','debug/build/base-2222222222222222/out/future-checker.rs','debug/incremental/private/source.rs']
            for relative in exact+forbidden:
                path=target/relative;path.parent.mkdir(parents=True,exist_ok=True);path.write_text('future/checker sentinel' if relative in forbidden else 'baseline')
            rows=[{'reason':'compiler-artifact','manifest_path':'/workspace/Cargo.toml','target':{'name':'base','kind':['lib']},'profile':{'test':False},'filenames':[seed.TARGET+'/'+exact[0]]},{'reason':'build-script-executed','out_dir':seed.TARGET+'/debug/build/base-2222222222222222/out'}]
            executable=target/'debug/deps/base-3333333333333333';executable.write_text('large linked test')
            rows.append({'reason':'compiler-artifact','manifest_path':'/workspace/Cargo.toml','target':{'name':'base','kind':['test']},'profile':{'test':True},'filenames':[seed.TARGET+'/debug/deps/'+executable.name]})
            selected=seed.collect(rows,target,workspace,{})
            self.assertEqual(set(selected),set(exact))
            self.assertTrue(all('future/checker sentinel' not in (target/name).read_text() for name in selected))

    def test_copy_budget_rejects_oversize_and_low_disk_before_copy(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);(root/'base.rlib').write_bytes(b'1234')
            with patch.object(seed,'MAX_SEED_BYTES',3):
                with self.assertRaisesRegex(ValueError,'export bound'):seed.copy_budget(root,['base.rlib'],root)
            with patch.object(seed.shutil,'disk_usage') as usage:
                usage.return_value.free=seed.MIN_COPY_HEADROOM_BYTES+3
                with self.assertRaisesRegex(ValueError,'headroom'):seed.copy_budget(root,['base.rlib'],root)
                usage.return_value.free+=1
                self.assertEqual(seed.copy_budget(root,['base.rlib'],root),4)

    def test_failed_result_replacement_keeps_prior_build_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);prior={'status':'incomplete','exit_code':0}
            seed.save_result(root,prior)
            with patch.object(seed.os,'fsync',side_effect=OSError('disk full')):
                with self.assertRaises(OSError):seed.save_result(root,{'status':'complete'})
            self.assertEqual(json.loads((root/'result.json').read_text()),prior)

    def test_missing_or_relative_rustdoc_fails_before_source_export(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);target=root/'target';target.mkdir()
            for i,environment in enumerate(({}, {'RUSTDOC':'rustdoc'})):
                config={'source_commit':'1'*40,'packages':['base'],'shared_target':str(target),'toolchain':{'environment':environment}}
                with patch.object(seed.subprocess,'run') as command:
                    result=seed.build(config,root/str(i))
                self.assertEqual(result['status'],'infrastructure_error')
                self.assertIn('absolute pinned',result['error'])
                command.assert_not_called()

    def make_seed(self,root):
        target=root/'target';target.mkdir()
        (target/'baseline.rlib').write_text('base-only')
        manifest={'schema':'openagents.delegation.cargo-seed.v1','seed_policy':seed.SEED_POLICY,'source_commit':'1'*40,'files':inventory(target)}
        (root/'seed-manifest.json').write_text(json.dumps(manifest))
        return digest(root/'seed-manifest.json')

    def test_validation_hashes_actual_contents_and_rejects_extra_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);expected=self.make_seed(root)
            validate_seed(root,expected,'1'*40)
            (root/'target/baseline.rlib').write_text('future-code')
            with self.assertRaises(ValueError):validate_seed(root,expected,'1'*40)
            (root/'target/baseline.rlib').write_text('base-only')
            (root/'target/checker').write_text('hidden checker')
            with self.assertRaises(ValueError):validate_seed(root,expected,'1'*40)

    def test_validation_rejects_wrong_commit_or_symlink(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);expected=self.make_seed(root)
            with self.assertRaises(ValueError):validate_seed(root,expected,'2'*40)
            with self.assertRaisesRegex(ValueError,'environment'):validate_seed(root,expected,'1'*40,build_environment={'CARGO_PROFILE_TEST_DEBUG':'0'})
            (root/'target/baseline.rlib').unlink();(root/'target/baseline.rlib').symlink_to('../seed-manifest.json')
            with self.assertRaises(ValueError):validate_seed(root,expected,'1'*40)

    def test_explicit_rustdoc_identity_is_bound_to_actual_executable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);self.make_seed(root)
            rustdoc=root/'rustdoc';rustdoc.write_bytes(b'pinned rustdoc')
            manifest=root/'seed-manifest.json';value=json.loads(manifest.read_text())
            environment={'RUSTDOC':str(rustdoc)}
            value.update(build_environment=environment,rustdoc_path=str(rustdoc),rustdoc_sha256=digest(rustdoc),toolchain={'rustdoc':'rustdoc pinned version'})
            manifest.write_text(json.dumps(value));expected=digest(manifest)
            validate_seed(root,expected,build_environment=environment)
            rustdoc.write_bytes(b'replaced rustdoc')
            with self.assertRaisesRegex(ValueError,'executable changed'):validate_seed(root,expected,build_environment=environment)


if __name__=='__main__':unittest.main()
