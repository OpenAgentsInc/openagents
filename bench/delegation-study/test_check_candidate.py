import hashlib
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import time
import unittest
import uuid
from unittest import mock

import candidate
import check_candidate as checks
from seed_manifest import digest, inventory


class Fixture:
    def __init__(self, root):
        self.root = root
        self.base = root / 'base'
        (self.base / 'crate/src').mkdir(parents=True)
        (self.base / 'Cargo.toml').write_text('[workspace]\nmembers=["crate"]\n')
        (self.base / 'crate/Cargo.toml').write_text('[package]\nname="fixture"\nversion="0.1.0"\nedition="2021"\n')
        (self.base / 'crate/src/lib.rs').write_text('pub fn value() -> u8 { 0 }\n')
        (self.base / 'crate/src/old.rs').write_text('pub const OLD: bool = true;\n')
        self.archive = root / 'source.tar'
        with tarfile.open(self.archive, 'w') as archive:
            for path in self.base.rglob('*'):
                archive.add(path, arcname=path.relative_to(self.base), recursive=False)
        self.commit = '1' * 40
        self.native = root / 'native'
        self.native.mkdir()
        self.body = b'pub fn value() -> u8 { 1 }\n'
        self.new_body = b'pub const ADDED: bool = true;\n'
        self.changes = {
            'crate/src/lib.rs': {'before': checks.descriptor(self.base / 'crate/src/lib.rs'),
                                 'after': {'kind': 'file', 'mode': 0o755, 'bytes': len(self.body), 'sha256': hashlib.sha256(self.body).hexdigest()}},
            'crate/src/new.rs': {'before': None, 'after': {'kind': 'file', 'mode': 0o644, 'bytes': len(self.new_body), 'sha256': hashlib.sha256(self.new_body).hexdigest()}},
            'crate/src/old.rs': {'before': checks.descriptor(self.base / 'crate/src/old.rs'), 'after': None},
        }
        with tarfile.open(self.native / 'candidate.tar.gz', 'w:gz') as archive:
            info = tarfile.TarInfo('crate/src/lib.rs')
            info.mode, info.size = 0o755, len(self.body)
            archive.addfile(info, io.BytesIO(self.body))
            info = tarfile.TarInfo('crate/src/new.rs')
            info.mode, info.size = 0o644, len(self.new_body)
            archive.addfile(info, io.BytesIO(self.new_body))
        self.bind_candidate()
        self.checker = root / 'private-oracle.rs'
        self.checker.write_text('#[test]\nfn behavior() { assert_eq!(fixture::value(), 1); }\n')
        self.seed = root / 'seed'
        (self.seed / 'target/debug').mkdir(parents=True)
        (self.seed / 'target/debug/base.rlib').write_bytes(b'baseline-only')
        self.versions = {'cargo': 'cargo fixture-version', 'rustc': 'rustc fixture-version'}
        value = {'schema': 'openagents.delegation.cargo-seed.v1', 'source_commit': self.commit,
                 'source_archive_sha256': digest(self.archive), 'packages': ['fixture'],
                 'seed_policy': 'cargo-reported-libraries-v1', 'build_environment': {},
                 'toolchain': self.versions, 'files': inventory(self.seed / 'target')}
        (self.seed / 'seed-manifest.json').write_text(json.dumps(value))
        self.config = {'run_id': str(uuid.uuid4()), 'source_commit': self.commit,
                       'source_archive': str(self.archive), 'source_archive_sha256': digest(self.archive),
                       'candidate_dir': str(self.native), 'candidate_manifest_sha256': self.identity,
                       'allowed_paths': ['crate'], 'packages': ['fixture'],
                       'target_seed': str(self.seed), 'target_seed_manifest_sha256': digest(self.seed / 'seed-manifest.json'),
                       'toolchain': {'read_only_mounts': [], 'environment': {}}, 'total_timeout_s': 30,
                       'checker': {'path': str(self.checker), 'sha256': digest(self.checker),
                                   'injection': 'crate/tests/frozen_check.rs', 'target': 'frozen_check',
                                   'package': 'fixture', 'timeout_s': 2}}
        self.calls = []

    def bind_candidate(self):
        (self.native / 'changes.json').write_text(json.dumps(self.changes))
        self.identity, _ = candidate.write_manifest(self.native, self.commit, digest(self.archive), self.changes)

    def execute(self, argv, output, phase, timeout):
        self.calls.append((argv, phase, timeout))
        if phase.startswith('version-'):
            (output / (phase + '.stdout')).write_text(self.versions[phase.removeprefix('version-')] + '\n')
        return {'passed': True, 'status': 'passed', 'exit_code': 0, 'wall_s': .01}

    def run(self, name='checked', executor=None):
        output = self.root / name
        output.mkdir()
        return checks.check(self.config, output, executor or self.execute), output


class CheckCandidateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.fixture = Fixture(self.root)

    def tearDown(self):
        self.temporary.cleanup()

    def test_full_flow_reconstructs_deletions_modes_and_readonly_oracle(self):
        fixture = self.fixture
        seed_before = inventory(fixture.seed / 'target')
        row, output = fixture.run()
        self.assertTrue(row['completed'])
        self.assertTrue(row['accepted'])
        self.assertEqual(row['candidate_manifest_sha256'], fixture.identity)
        self.assertTrue((output / 'workspace/.git').is_dir())
        self.assertEqual(len(row['snapshot_commit']), 40)
        self.assertFalse((output / 'workspace/crate/src/old.rs').exists())
        library = output / 'workspace/crate/src/lib.rs'
        self.assertEqual(library.read_bytes(), fixture.body)
        self.assertEqual(library.stat().st_mode & 0o777, 0o755)
        self.assertEqual(library.stat().st_mtime_ns, row['candidate_mtimes_refreshed_unix_ns'])
        with tarfile.open(fixture.archive) as archive:
            original_mtime = archive.getmember('crate/Cargo.toml').mtime
        unchanged = output / 'workspace/crate/Cargo.toml'
        self.assertAlmostEqual(unchanged.stat().st_mtime, original_mtime, places=5)
        self.assertGreater(library.stat().st_mtime_ns, unchanged.stat().st_mtime_ns)
        self.assertEqual((output / 'checker.rs').stat().st_mtime_ns, library.stat().st_mtime_ns)
        self.assertEqual((output / 'workspace/crate/src/new.rs').stat().st_mtime_ns, library.stat().st_mtime_ns)
        self.assertEqual(row['source_mtime_policy'], 'changed_and_injected_only_verified_base_seed')
        self.assertEqual(inventory(fixture.seed / 'target'), seed_before)
        self.assertEqual((output / 'home/target/debug/base.rlib').read_bytes(), b'baseline-only')
        self.assertEqual([phase for _, phase, _ in fixture.calls], ['version-cargo', 'version-rustc', 'format', 'ordinary-compile', 'ordinary-test', 'independent-compile', 'independent-test'])
        for argv, phase, timeout in fixture.calls:
            at = argv.index(str(output / 'workspace'))
            self.assertEqual(argv[at - 1], '--ro-bind')
            self.assertIn('--unshare-all', argv)
            self.assertNotIn('/run/provider.sock', argv)
            self.assertGreater(timeout, 0)
            if phase == 'format':
                self.assertEqual(argv[-2:], ['--', '--check'])
                self.assertNotIn('--fix', argv)
            if phase.startswith('independent-'):
                at = argv.index(str(output / 'checker.rs'))
                self.assertEqual(argv[at - 1], '--ro-bind')
                self.assertEqual(argv[at + 1], '/workspace/crate/tests/frozen_check.rs')
                self.assertLessEqual(timeout, fixture.config['total_timeout_s'])
            else:
                self.assertNotIn(str(output / 'checker.rs'), argv)
        self.assertNotIn(str(fixture.checker), json.dumps(row))
        self.assertNotIn('assert_eq!', json.dumps(row))
        self.assertEqual(json.loads((output / 'checks.json').read_text()), row)

    def test_expected_native_snapshot_must_match_acceptance_base(self):
        self.fixture.config['expected_snapshot_commit'] = '0' * 40
        row, _ = self.fixture.run()
        self.assertFalse(row['completed'])
        self.assertEqual(row['error_code'], 'snapshot_commit_changed')
        self.assertEqual(self.fixture.calls, [])

    def test_cache_copy_does_not_share_writable_baseline_files(self):
        row, output = self.fixture.run()
        self.assertTrue(row['completed'])
        (output / 'home/target/debug/base.rlib').write_bytes(b'executor-modified')
        self.assertEqual((self.fixture.seed / 'target/debug/base.rlib').read_bytes(), b'baseline-only')

    def test_linux_copy_requests_reflink_with_copy_fallback(self):
        source = self.fixture.seed / 'target'
        destination = self.root / 'cloned'
        destination.mkdir()
        (destination / 'debug').mkdir()
        (destination / 'debug/base.rlib').write_bytes(b'baseline-only')
        with mock.patch.object(checks.sys, 'platform', 'linux'), mock.patch.object(checks.subprocess, 'run') as command:
            checks.copy_seed(source, destination, ['debug/base.rlib'])
        self.assertEqual(command.call_args.args[0], ['cp', '-a', '--reflink=auto', str(source), str(destination)])

    def test_changed_seed_or_checker_never_runs_commands(self):
        for kind in ('seed', 'checker'):
            with self.subTest(kind=kind):
                fixture = self.fixture
                path = fixture.seed / 'target/debug/base.rlib' if kind == 'seed' else fixture.checker
                original = path.read_bytes()
                path.write_bytes(b'tampered')
                row, _ = fixture.run('checked-' + kind)
                self.assertFalse(row['completed'])
                self.assertEqual(row['status'], 'infrastructure_error')
                self.assertEqual(fixture.calls, [])
                path.write_bytes(original)

    def test_seed_environment_must_match_acceptance_environment(self):
        self.fixture.config['toolchain']['environment'] = {'CARGO_PROFILE_DEV_DEBUG': '1'}
        row, _ = self.fixture.run()
        self.assertFalse(row['completed'])
        self.assertEqual(row['status'], 'infrastructure_error')
        self.assertEqual(self.fixture.calls, [])

    def test_explicit_features_reach_both_compile_and_test_commands(self):
        features=['fixture/blocking']
        self.fixture.config['cargo_features']=features
        path=self.fixture.seed/'seed-manifest.json';value=json.loads(path.read_text())
        value['cargo_features']=features;path.write_text(json.dumps(value))
        self.fixture.config['target_seed_manifest_sha256']=digest(path)
        row,_=self.fixture.run()
        self.assertTrue(row['accepted'],row)
        self.assertEqual(row['cargo_features'],features)
        for argv,phase,_ in self.fixture.calls:
            if phase.startswith(('ordinary-','independent-')):
                self.assertEqual(argv[argv.index('--features')+1],'fixture/blocking')
            else:
                self.assertNotIn('--features',argv)

    def test_mismatched_seed_features_refuse_before_commands(self):
        self.fixture.config['cargo_features']=['fixture/blocking']
        row,_=self.fixture.run()
        self.assertFalse(row['completed'])
        self.assertEqual(row['status'],'infrastructure_error')
        self.assertEqual(self.fixture.calls,[])

    def test_wildcard_or_unselected_features_refuse_before_commands(self):
        for index,features in enumerate((['fixture/*'],['--all-features'],['other/blocking'])):
            self.fixture.config['cargo_features']=features
            row,_=self.fixture.run('invalid-features-'+str(index))
            self.assertFalse(row['completed'])
            self.assertEqual(self.fixture.calls,[])

    def test_candidate_preimage_must_match_clean_base(self):
        fixture = self.fixture
        fixture.changes['crate/src/lib.rs']['before']['sha256'] = '0' * 64
        fixture.bind_candidate()
        fixture.config['candidate_manifest_sha256'] = fixture.identity
        row, _ = fixture.run()
        self.assertFalse(row['completed'])
        self.assertEqual(row['error_code'], 'candidate_preimage_mismatch')
        self.assertEqual(fixture.calls, [])

    def test_scope_uses_path_components_and_preserves_failed_receipt(self):
        self.fixture.config['allowed_paths'] = ['crat']
        row, _ = self.fixture.run()
        self.assertTrue(row['completed'])
        self.assertFalse(row['accepted'])
        self.assertFalse(row['scope']['passed'])
        self.assertEqual(row['scope']['status'], 'prohibited_scope')
        self.assertTrue(all(type(row[p]['passed']) is bool for p in checks.PHASES))
        self.assertEqual(self.fixture.calls, [])
        self.assertFalse(checks.scoped({'crate/tests/frozen_check.rs': {'before': None, 'after': {'kind': 'file'}}}, ['crate'], 'crate/tests/frozen_check.rs'))

    def test_readonly_source_does_not_admit_symlink_candidates(self):
        self.assertFalse(checks.scoped({'crate/link': {'before': None, 'after': {'kind': 'symlink', 'target': '/private'}}}, ['crate'], 'crate/tests/check.rs'))
        source = self.root / 'source'
        source.mkdir()
        (source / 'link').symlink_to(self.root)
        with self.assertRaises(checks.Invalid):
            checks.safe_path(source, 'link/outside')

    def test_failed_format_still_runs_ordinary_and_independent_without_repair(self):
        def execute(argv, output, phase, timeout):
            row = self.fixture.execute(argv, output, phase, timeout)
            if phase == 'format':
                row.update(passed=False, status='failed', exit_code=1)
            return row
        row, _ = self.fixture.run(executor=execute)
        self.assertTrue(row['completed'])
        self.assertFalse(row['accepted'])
        self.assertTrue(row['ordinary']['passed'])
        self.assertTrue(row['independent']['passed'])

    def test_phase_timeout_is_nonacceptance_after_identity_validation(self):
        def execute(argv, output, phase, timeout):
            row = self.fixture.execute(argv, output, phase, timeout)
            if phase == 'ordinary-test':
                row.update(passed=False, status='timeout', exit_code=-9)
            return row
        row, _ = self.fixture.run(executor=execute)
        self.assertTrue(row['completed'])
        self.assertFalse(row['accepted'])
        self.assertEqual(row['ordinary']['status'], 'timeout')
        self.assertEqual(row['independent'], {'passed': False, 'status': 'not_run_after_deadline'})

    def test_compile_failure_keeps_elapsed_and_does_not_execute_that_test(self):
        def execute(argv, output, phase, timeout):
            result = self.fixture.execute(argv, output, phase, timeout)
            if phase == 'ordinary-compile':
                result.update(passed=False, status='failed', exit_code=101)
            return result
        row, _ = self.fixture.run(executor=execute)
        self.assertTrue(row['completed'])
        self.assertFalse(row['accepted'])
        self.assertEqual(row['ordinary']['compile']['wall_s'], .01)
        self.assertEqual(row['ordinary']['test']['status'], 'not_run_after_compile_failure')
        self.assertTrue(row['independent']['passed'])
        self.assertNotIn('ordinary-test', [phase for _, phase, _ in self.fixture.calls])

    def test_watchdog_failure_keeps_completed_subphase_measurements(self):
        row = checks.initial(self.fixture.config)
        row.update(identity_validated=True, scope_validated=True,
                   active_check={'phase': 'ordinary', 'part': 'test'})
        row['scope'] = {'passed': True, 'status': 'passed'}
        row['ordinary']['compile'] = {'passed': True, 'status': 'passed', 'wall_s': 3.5}
        checks.budget_failure(row, 'total_deadline')
        self.assertEqual(row['ordinary']['compile']['wall_s'], 3.5)
        self.assertEqual(row['ordinary']['test']['status'], 'total_deadline')
        self.assertTrue(row['completed'])

    def test_declared_rustdoc_uses_pinned_path_and_version(self):
        rustdoc = self.root / 'pinned-rustdoc'
        rustdoc.write_bytes(b'synthetic executable identity')
        rustdoc.chmod(0o755)
        self.fixture.versions['rustdoc'] = 'rustdoc fixture-version'
        environment = {'RUSTDOC': str(rustdoc)}
        self.fixture.config['toolchain']['environment'] = environment
        manifest = self.fixture.seed / 'seed-manifest.json'
        value = json.loads(manifest.read_text())
        value.update(build_environment=environment, toolchain=self.fixture.versions,
                     rustdoc_path=str(rustdoc), rustdoc_sha256=digest(rustdoc))
        manifest.write_text(json.dumps(value))
        self.fixture.config['target_seed_manifest_sha256'] = digest(manifest)
        row, _ = self.fixture.run()
        self.assertTrue(row['completed'])
        command = next(argv for argv, phase, _ in self.fixture.calls if phase == 'version-rustdoc')
        self.assertEqual(command[-2:], [str(rustdoc), '-Vv'])
        self.assertEqual(row['toolchain']['rustdoc'], self.fixture.versions['rustdoc'])

    def test_wrong_toolchain_fails_before_check_execution(self):
        def execute(argv, output, phase, timeout):
            row = self.fixture.execute(argv, output, phase, timeout)
            if phase == 'version-rustc':
                (output / (phase + '.stdout')).write_text('other version')
            return row
        row, _ = self.fixture.run(executor=execute)
        self.assertFalse(row['completed'])
        self.assertEqual(row['error_code'], 'toolchain_changed')
        self.assertEqual(len(self.fixture.calls), 2)

    def test_existing_output_is_never_overwritten(self):
        output = self.root / 'existing'
        output.mkdir()
        (output / 'sentinel').write_text('retain')
        with self.assertRaises(FileExistsError):
            checks.run(self.fixture.config, output)
        self.assertEqual((output / 'sentinel').read_text(), 'retain')

    def test_invalid_source_is_retained_by_actual_watchdog_without_cargo(self):
        config = dict(self.fixture.config, source_archive_sha256='0' * 64)
        output = self.root / 'watchdog'
        row = checks.run(config, output)
        self.assertFalse(row['completed'])
        self.assertEqual(row['error_code'], 'source_archive_changed')
        self.assertEqual(json.loads((output / 'checks.json').read_text()), row)
        self.assertGreater(row['total_wall_s'], 0)
        self.assertTrue(row['execution_closed'])

    def test_real_cancellation_preserves_checkpoint_and_stops_child(self):
        worker = self.root / 'synthetic-worker.py'
        worker.write_text("""import json,subprocess,sys,time
from pathlib import Path
import check_candidate as c
config=json.loads(Path(sys.argv[2]).read_text());out=Path(sys.argv[3])
row=c.initial(config);row.update(identity_validated=True,scope_validated=True)
row['scope']={'passed':True,'status':'passed'}
row['format']={'passed':True,'status':'passed','wall_s':3.25}
row['ordinary']['compile']={'passed':True,'status':'passed','wall_s':4.5}
c.durable_json(out/'checks.json',row)
child=subprocess.Popen([sys.executable,'-c',"from pathlib import Path;import sys,time;time.sleep(.8);Path(sys.argv[1]).write_text('late')",str(out/'late')],start_new_session=True)
c.durable_json(out/'active-process.json',{'pid':child.pid})
(out/'ready').write_text('ready')
time.sleep(60)
""")
        config = self.root / 'config.json'
        config.write_text(json.dumps(self.fixture.config))
        env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1', PYTHONPATH=str(Path(checks.__file__).parent))
        for number in (checks.signal.SIGTERM, checks.signal.SIGINT):
            with self.subTest(signal=number):
                output = self.root / ('signal-' + str(number))
                script = 'import json,sys;from pathlib import Path;import check_candidate as c;c.__file__=sys.argv[1];c.run(json.loads(Path(sys.argv[2]).read_text()),Path(sys.argv[3]))'
                process = checks.subprocess.Popen([checks.sys.executable, '-c', script, str(worker), str(config), str(output)], env=env,
                                                   stdout=checks.subprocess.PIPE, stderr=checks.subprocess.PIPE)
                try:
                    deadline = time.monotonic() + 5
                    while time.monotonic() < deadline and not (output / 'ready').exists():
                        if process.poll() is not None:
                            self.fail('Synthetic worker exited before the cancellation probe')
                        time.sleep(.01)
                    self.assertTrue((output / 'ready').exists())
                    process.send_signal(number)
                    stdout, stderr = process.communicate(timeout=5)
                    self.assertEqual(process.returncode, 0, stderr.decode())
                    row = json.loads((output / 'checks.json').read_text())
                    self.assertFalse(row['completed'])
                    self.assertTrue(row['execution_closed'])
                    self.assertEqual(row['status'], 'interrupted')
                    self.assertEqual(row['format']['wall_s'], 3.25)
                    self.assertEqual(row['ordinary']['compile']['wall_s'], 4.5)
                    time.sleep(.85)
                    self.assertFalse((output / 'late').exists())
                finally:
                    if process.poll() is None:
                        process.kill();process.wait()

    def test_finished_worker_still_closes_recorded_namespace_group(self):
        output = self.root / 'cleanup'
        output.mkdir()
        checks.durable_json(output / 'active-process.json', {'pid': 12345678})
        process = mock.Mock(pid=87654321)
        process.poll.return_value = 1
        with mock.patch.object(checks.os, 'killpg') as kill:
            checks.stop_worker(process, output)
        kill.assert_called_once_with(12345678, checks.signal.SIGKILL)
        self.assertFalse((output / 'active-process.json').exists())
        process.wait.assert_called_once()

    def test_total_deadline_preserves_known_scope_and_kills_worker(self):
        class Process:
            pid = 99999999
            returncode = None
            def wait(self, timeout=None):
                if timeout is not None:
                    raise checks.subprocess.TimeoutExpired('synthetic', timeout)
                self.returncode = -9
                return -9
            def poll(self):
                return self.returncode
        process = Process()
        output = self.root / 'deadline'
        def launch(*args, **kwargs):
            row = checks.initial(self.fixture.config)
            row.update(identity_validated=True, scope_validated=True)
            row['scope'] = {'passed': True, 'status': 'passed'}
            checks.durable_json(output / 'checks.json', row)
            return process
        with mock.patch.object(checks.subprocess, 'Popen', side_effect=launch), mock.patch.object(checks.os, 'killpg') as kill:
            row = checks.run(self.fixture.config, output)
        kill.assert_called_once_with(process.pid, checks.signal.SIGKILL)
        self.assertTrue(row['completed'])
        self.assertFalse(row['accepted'])
        self.assertTrue(row['scope']['passed'])
        self.assertEqual(row['independent']['status'], 'total_deadline')


if __name__ == '__main__':
    unittest.main()
