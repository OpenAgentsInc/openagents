#!/usr/bin/env python3
"""The machine-wide build-worker budget (ax-job-budget, coax/src/jobs.cpp).

Every case runs on its own token directory ($COAX_JOBS_DIR) with a small
budget and no load backoff, so it is the same on a busy machine:
  - `coax status` keeps the release catalogue on stdout and adds the budget
    on stderr; `coax status --jobs [--json]` prints the budget alone;
  - three compiles at once never hold more tokens than the budget, each runs
    at least one worker, and a compile that waits says so once;
  - a compile killed while it holds tokens frees them at once;
  - `coax c` output and the `coax mutate` report are byte-identical at 1, 4
    and all cores;
  - without a usable token directory a compile still runs, and status says so.

  python3 coax/tests/job_budget.py
"""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax'))
CORES = os.cpu_count() or 1

# Eight #authority checks of about a fifth of a second each.
SLOW = '#authority;\nspin :: (n: int, k: int) -> int { s := 0; for i: 0..<n { s += (i * k) % 7; } return s; }\n' + ''.join(
    f'#test "spin {k}" #steps 400000000 {{ #assert spin(4000000, {k}) >= 0; }}\n' for k in range(1, 9)) + 'main :: () { }\n'

MUTATE = '''
f :: (x:int)->int { return x+1; }
g :: (x:int)->int { return x*2; }
h :: (x:int)->int { if x>2 { return x-1; } return x+1; }
#test "first" { #assert f(2)==3; }
#test "second" { #assert g(3)==6; }
#forall "range" x:0..<5 { #assert f(x)>x; }
#test "branch" { #assert h(1)==2 && h(2)==3 && h(3)==2; }
main :: () {}
'''


class JobBudget(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='coax-job-budget-')
        self.dir = Path(self.tmp.name)
        self.pool = self.dir / 'pool'
        (self.dir / 'slow.coax').write_text(SLOW)
        (self.dir / 'mutate.coax').write_text(MUTATE)
        self.env = {k: v for k, v in os.environ.items() if not k.startswith('COAX_JOBS') and k not in ('COAX_CHECK_JOBS', 'COAX_MUTATE_JOBS')}
        self.env.update(COAX_JOBS_DIR=str(self.pool), COAX_JOBS_BUDGET='3', COAX_JOBS_LOAD='0')
        self.caches = 0

    def tearDown(self):
        self.tmp.cleanup()

    def cache(self):
        self.caches += 1
        return str(self.dir / f'cache-{self.caches}')

    def run_coax(self, *args, **env):
        e = dict(self.env, COAX_CHECK_CACHE=self.cache(), **env)
        return subprocess.run([str(COMPILER), *args], capture_output=True, text=True, env=e, cwd=self.dir, timeout=600)

    def status(self):
        r = subprocess.run([str(COMPILER), 'status', '--jobs', '--json'], capture_output=True, text=True, env=self.env, timeout=60)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def test_status_shows_the_budget(self):
        s = self.status()
        self.assertEqual((s['budget'], s['held'], s['shared'], s['load_limit'], s['holders']), (3, 0, True, 0, []))
        self.assertGreaterEqual(s['performance_cores'], 1)
        text = subprocess.run([str(COMPILER), 'status', '--jobs'], capture_output=True, text=True, env=self.env).stdout
        self.assertIn('Build workers: 0 of 3 in use by 0 coax processes', text)
        self.assertIn('budget 3 from $COAX_JOBS_BUDGET', text)
        default = dict(self.env)
        del default['COAX_JOBS_BUDGET']
        d = json.loads(subprocess.run([str(COMPILER), 'status', '--jobs', '--json'], capture_output=True, text=True, env=default).stdout)
        self.assertEqual(d['budget'], d['performance_cores'] - d['reserve'])
        self.assertEqual(d['reserve'], max(1, d['performance_cores'] // 3) if d['performance_cores'] > 1 else 0)
        r = subprocess.run([str(COMPILER), 'status', '--jobs', '--json'], capture_output=True, text=True,
                           env=dict(default, COAX_JOBS_RESERVE='2'))
        self.assertEqual(json.loads(r.stdout)['reserve'], min(2, d['performance_cores'] - 1))
        # The catalogue alone on stdout, as the release check compares it; the budget on stderr.
        plain = subprocess.run([str(COMPILER), 'status'], capture_output=True, text=True, env=self.env)
        self.assertEqual(plain.stdout, (ROOT / 'STATUS.md').read_text())
        self.assertIn('Build workers:', plain.stderr)
        js = subprocess.run([str(COMPILER), 'status', '--json'], capture_output=True, text=True, env=self.env)
        self.assertEqual(js.stdout, (ROOT / 'release-status.json').read_text())

    def test_concurrent_compiles_share_the_budget(self):
        procs = [subprocess.Popen([str(COMPILER), 'check', 'slow.coax'], cwd=self.dir, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  text=True, env=dict(self.env, COAX_CHECK_CACHE=self.cache())) for _ in range(3)]
        most = most_over = 0
        while any(p.poll() is None for p in procs):
            s = self.status()
            most = max(most, s['held'])
            most_over = max(most_over, s['over_budget'])
            self.assertLessEqual(s['held'], 3, s)
            time.sleep(0.05)
        outs = [p.communicate() for p in procs]
        for p, (out, err) in zip(procs, outs):
            self.assertEqual((p.returncode, out), (0, ''), err)
        self.assertEqual(most, 3)
        self.assertLessEqual(most_over, 3)
        waits = [err.count('coax jobs: waiting for ') for _, err in outs]
        self.assertTrue(all(w <= 1 for w in waits), waits)
        self.assertGreaterEqual(sum(waits), 1, outs)
        self.assertRegex(''.join(err for _, err in outs), r'waiting for \d of 3 build workers held by \d other coax process')
        self.assertEqual(self.status()['held'], 0)

    def test_a_killed_compile_frees_its_tokens(self):
        p = subprocess.Popen([str(COMPILER), 'check', 'slow.coax'], cwd=self.dir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                             env=dict(self.env, COAX_CHECK_CACHE=self.cache()), start_new_session=True)
        try:
            deadline = time.monotonic() + 30
            while self.status()['held'] < 3 and time.monotonic() < deadline: time.sleep(0.02)
            self.assertEqual(self.status()['held'], 3)
            os.kill(p.pid, signal.SIGKILL)
            p.wait()
            self.assertEqual(self.status()['held'], 0)
            # A new compile gets every token at once.
            r = self.run_coax('check', 'slow.coax', '--timings', str(self.dir / 't.json'))
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertNotIn('waiting', r.stderr)
        finally:
            try: os.killpg(p.pid, signal.SIGKILL)  # its orphaned check children: this test's own process group
            except ProcessLookupError: pass

    def test_results_are_the_same_for_every_worker_count(self):
        everything = dict(COAX_JOBS_BUDGET=str(CORES))
        c = {j: self.run_coax('c', 'slow.coax', '--timings', str(self.dir / f'c{j}.json'), COAX_CHECK_JOBS=str(j), **everything) for j in (1, 4, CORES)}
        for j, r in c.items():
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(r.stdout, c[1].stdout, f'coax c with {j} workers')
        way = json.loads((self.dir / f'c{CORES}.json').read_text())['parallel_checks']
        self.assertTrue(way.startswith('parallel' if CORES > 1 else 'sequential'), way)
        m = {j: self.run_coax('mutate', 'mutate.coax', '--jobs', str(j), **everything) for j in (1, 4, CORES)}
        for j, r in m.items():
            self.assertEqual((r.returncode, r.stdout), (m[1].returncode, m[1].stdout), f'coax mutate --jobs {j}')
        self.assertIn('mutants,', m[1].stdout)
        # $COAX_JOBS and $COAX_MUTATE_JOBS give the same report, and so does a budget of one.
        for env in (dict(COAX_JOBS='4'), dict(COAX_MUTATE_JOBS='4'), dict(COAX_JOBS_BUDGET='1')):
            r = self.run_coax('mutate', 'mutate.coax', **env)
            self.assertEqual((r.returncode, r.stdout), (m[1].returncode, m[1].stdout), env)
        r = self.run_coax('c', 'slow.coax', '--jobs', '2', COAX_JOBS_BUDGET='1')
        self.assertEqual((r.returncode, r.stdout), (0, c[1].stdout), r.stderr)

    def test_no_token_directory(self):
        env = dict(COAX_JOBS_DIR='/dev/null/coax-jobs')
        s = json.loads(self.run_coax('status', '--jobs', '--json', **env).stdout)
        self.assertFalse(s['shared'])
        self.assertIn('no shared pool', self.run_coax('status', '--jobs', **env).stdout)
        r = self.run_coax('c', 'slow.coax', COAX_CHECK_JOBS='3', **env)
        self.assertEqual(r.returncode, 0, r.stderr)


if __name__ == '__main__':
    unittest.main()
