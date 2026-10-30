#!/usr/bin/env python3
"""Operational wrapper tests; the unchanged cryptographic oracle is not retested."""
import json,os,pathlib,signal,subprocess,sys,tempfile,time,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
SCRIPT=ROOT/'scripts/probe-confidential-pylon.sh'
FIXTURE=ROOT/'docs/verification/data/oct30-confidential/round.json'
class Probe(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory(prefix='confidential-probe-');self.root=pathlib.Path(self.tmp.name);self.report=json.loads(FIXTURE.read_text());self.fake=self.root/'oracle with spaces'
  self.fake.write_text('#!'+sys.executable+'\nimport json,os,pathlib,sys,time\npathlib.Path(os.environ["ARGS"]).write_text(json.dumps(sys.argv[1:]))\ntime.sleep(float(os.environ.get("DELAY","0")))\nprint(pathlib.Path(os.environ["REPORT"]).read_text())\nsys.exit(int(os.environ.get("EXIT","0")))\n');self.fake.chmod(0o755)
  self.env={**os.environ,'REPORT':str(self.root/'input.json'),'ARGS':str(self.root/'args.json')}
 def tearDown(self):self.tmp.cleanup()
 def run_probe(self,success=True,extra=()):
  pathlib.Path(self.env['REPORT']).write_text(json.dumps(self.report));r=subprocess.run(['bash',str(SCRIPT),'--oracle',str(self.fake),'--evidence-dir',str(self.root/'proofs'),'--timeout-seconds','2',*extra],env=self.env,capture_output=True,text=True,timeout=8)
  if success:self.assertEqual(r.returncode,0,r.stdout+r.stderr)
  else:self.assertNotEqual(r.returncode,0,r.stdout+r.stderr)
  return json.loads(r.stdout)
 def test_success_keeps_public_proof_and_fresh_state_without_keys(self):
  a=self.run_probe();args=json.loads(pathlib.Path(self.env['ARGS']).read_text());state=args[args.index('--state')+1];self.assertEqual(args[0],'round');self.assertNotIn('--key',args);self.assertIn('sunny',state);self.assertTrue(a['ok']);self.assertEqual(a['provider'],'eadd886db9e84b3fa069e84282fec1943388d84baf1e3ad534254b524ed0dfc0');self.assertEqual(a['inference_ms'],7882)
  self.assertEqual(json.loads(pathlib.Path(a['proof']).read_text()),self.report)
  b=self.run_probe();args2=json.loads(pathlib.Path(self.env['ARGS']).read_text());self.assertNotEqual(state,args2[args2.index('--state')+1]);self.assertNotEqual(a['proof'],b['proof'])
 def test_top_level_refused_and_missing_receipt_are_not_success(self):
  self.report['refused']={'step':'measure','reason':'fixture refusal'};self.assertFalse(self.run_probe(False)['ok']);self.report.pop('refused');self.report['steps']=[s for s in self.report['steps'] if s['step']!='receipt'];self.assertFalse(self.run_probe(False)['ok'])
 def test_bad_receipt_and_response_binding_are_not_success(self):
  for corrupt in ('receipt','provider','score','latency'):
   self.report=json.loads(FIXTURE.read_text());steps={s['step']:s for s in self.report['steps']}
   if corrupt=='receipt':steps['receipt']['checked']['receipt_digest']='not a digest'
   elif corrupt=='provider':steps['answer']['response']['service']['provider']='0'*64
   elif corrupt=='score':steps['answer']['response']['answers']['answer']['noul']=True
   else:steps['answer']['response']['psionic']['latency_ms']=0
   self.assertFalse(self.run_probe(False)['ok'])
 def test_nonzero_oracle_and_timeout_are_explicit_failures(self):
  self.env['EXIT']='9';r=self.run_probe(False);self.assertEqual(r['oracle_exit'],9)
  self.env['EXIT']='0';self.env['DELAY']='10';t=time.monotonic();r=self.run_probe(False);self.assertLess(time.monotonic()-t,6);self.assertIn(r['oracle_exit'],(124,137));self.assertFalse(r['ok'])
 def test_cancellation_terminates_its_owned_oracle(self):
  pathlib.Path(self.env['REPORT']).write_text(json.dumps(self.report));self.env['DELAY']='10'
  p=subprocess.Popen(['bash',str(SCRIPT),'--oracle',str(self.fake),'--evidence-dir',str(self.root/'proofs')],env=self.env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   deadline=time.monotonic()+4
   while not pathlib.Path(self.env['ARGS']).exists() and time.monotonic()<deadline:time.sleep(.02)
   self.assertTrue(pathlib.Path(self.env['ARGS']).exists());start=time.monotonic();p.send_signal(signal.SIGTERM);p.communicate(timeout=3);self.assertLess(time.monotonic()-start,3);self.assertNotEqual(p.returncode,0)
  finally:
   if p.poll() is None:p.kill();p.wait()
 def test_bad_timeout_never_runs_oracle(self):
  self.assertFalse(self.run_probe(False,('--timeout-seconds','0'))['ok']);self.assertFalse(pathlib.Path(self.env['ARGS']).exists())
if __name__=='__main__':unittest.main()
