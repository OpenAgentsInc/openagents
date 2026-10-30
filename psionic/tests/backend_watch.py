#!/usr/bin/env python3
"""Real generated policy and ASan/UBSan face with isolated platform effects."""
import json,os,pathlib,subprocess,sys,tempfile,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
BIN=pathlib.Path(os.environ.get('BACKEND_WATCH_BIN','/tmp/oct30-backend-watch-build/backend-watch'))
SCRIPT=ROOT/'psionic/platform/backend-watch-once.sh'
BOOT='11111111-2222-3333-4444-555555555555'
class Watch(unittest.TestCase):
 def setUp(self):
  self.t=tempfile.TemporaryDirectory(prefix='backend-watch-');self.p=pathlib.Path(self.t.name);self.state=self.p/'state';self.mock=self.p/'mock.json';self.log=self.p/'effects';self.boot=self.p/'boot';self.boot.write_text(BOOT)
  self.data={'wall':100,'mono':1000,'start':100000000,'inv':'0123456789abcdef0123456789abcdef','status':'503','body':'{}','confirm_ready':False};self.mock.write_text(json.dumps(self.data))
  tools=self.p/'tools';tools.mkdir();fake=tools/'effect'
  fake.write_text('#!'+sys.executable+'\n'+'''import json,os,pathlib,sys
p=pathlib.Path(os.environ['BW_MOCK']);d=json.loads(p.read_text());kind=pathlib.Path(sys.argv[0]).name
with open(os.environ['BW_LOG'],'a') as f:f.write(json.dumps([kind,*sys.argv[1:]])+'\\n')
if kind=='systemctl':
 if sys.argv[1]=='show':
  prop=[a.split('=',1)[1] for a in sys.argv if a.startswith('--property=')][0]
  print({'InvocationID':d['inv'],'ExecMainStartTimestampMonotonic':d['start'],'LoadState':'loaded'}[prop])
elif kind=='curl':
 unit=sys.argv[-1].split(':')[-1].split('/')[0];count=d.setdefault('counts',{}).get(unit,0);d['counts'][unit]=count+1
 ready=d.get('confirm_ready') and count>=3
 pathlib.Path(sys.argv[sys.argv.index('--output')+1]).write_text(('{'+'"status":"ok"'+'}' if unit=='8795' else '{'+'"schema":"psionic-health-v1","ready":true'+'}') if ready else d['body'])
 print('200' if ready else d['status'],end='');p.write_text(json.dumps(d))
else:
 if sys.argv[1]=='clock':print(json.dumps({'wall':d['wall'],'mono':d['mono']}))
 else:os.execv(os.environ['BW_REAL'],[os.environ['BW_REAL'],*sys.argv[1:]])
''');fake.chmod(0o755)
  for name in ('systemctl','curl','backend-watch'):(tools/name).symlink_to(fake)
  self.env={**os.environ,'PATH':str(tools)+os.pathsep+os.environ['PATH'],'BW_MOCK':str(self.mock),'BW_LOG':str(self.log),'BW_REAL':str(BIN),'OA_BACKEND_WATCH_BIN':str(tools/'backend-watch'),'OA_BACKEND_WATCH_STATE':str(self.state),'OA_BACKEND_WATCH_BOOT_FILE':str(self.boot),'OA_BACKEND_WATCH_START':'100','OA_BACKEND_WATCH_END':'21700'}
 def tearDown(self):self.t.cleanup()
 def set(self,**changes):
  d=json.loads(self.mock.read_text());d.update(changes);self.mock.write_text(json.dumps(d))
 def run_once(self):
  r=subprocess.run(['bash',str(SCRIPT)],env=self.env,capture_output=True,text=True,timeout=15);self.assertEqual(r.returncode,0,r.stdout+r.stderr);return r.stdout
 def effects(self):return [json.loads(x) for x in self.log.read_text().splitlines()] if self.log.exists() else []
 def restarts(self):return [e for e in self.effects() if e[:2]==['systemctl','restart']]
 def test_three_failures_restart_only_the_exact_three_units(self):
  for mono in (1000,1120,1240):self.set(mono=mono);self.run_once()
  self.assertEqual([x[-1] for x in self.restarts()],['psionic-elephant-rtx.service','psionic-mammoth.service','psionic-gptoss20b.service'])
  self.assertTrue(all(json.loads(p.read_text())['failures']==0 for p in self.state.glob('*.json')))
 def test_health_busy_and_warmup_never_restart(self):
  for status,body in [('429','{}'),('200','{"schema":"psionic-health-v1","ready":true}'),('404','{}')]:
   self.set(status=status,body=body)
   for mono in (1000,1120,1240):self.set(mono=mono);self.run_once()
  self.set(status='503',body='{}',start=1240000000)
  for mono in (1240,1360,1480):self.set(mono=mono);self.run_once()
  self.assertEqual(self.restarts(),[])
 def test_recovered_confirmation_cancels_a_planned_restart(self):
  self.set(confirm_ready=True)
  for mono in (1000,1120,1240):self.set(mono=mono);self.run_once()
  self.assertEqual(self.restarts(),[])
 def test_outside_window_or_process_restart_cannot_carry_failures(self):
  self.set(wall=21700);self.run_once();self.assertEqual(self.effects(),[['backend-watch','clock'],['backend-watch','window','21700','100','21700']])
  self.set(wall=100)
  for mono in (1000,1120):self.set(mono=mono);self.run_once()
  self.set(mono=1240,start=1240000000,inv='f'*32);self.run_once();self.assertEqual(self.restarts(),[])
 def test_binary_and_state_paths_with_spaces_are_transparent(self):
  tools=self.p/'tools with spaces';tools.mkdir();effect=tools/'effect';effect.write_text((self.p/'tools/effect').read_text());effect.chmod(0o755)
  for name in ('systemctl','curl','backend-watch'):(tools/name).symlink_to(effect)
  self.env['PATH']=str(tools)+os.pathsep+self.env['PATH'];self.env['OA_BACKEND_WATCH_BIN']=str(tools/'backend-watch');self.env['OA_BACKEND_WATCH_STATE']=str(self.p/'state with spaces');self.set(status='429');self.run_once();self.assertEqual(self.restarts(),[]);self.assertEqual(len(list((self.p/'state with spaces').glob('*.json'))),3)
 def test_native_face_rejects_oversized_files_and_bad_arguments(self):
  body=self.p/'body';body.write_text('x'*8193);state=self.p/'absent'
  args=['observe','0','100','1000','100','21700',BOOT,'0'*32,'100000000','loaded','503',str(body),str(state)]
  self.assertNotEqual(subprocess.run([str(BIN),*args],capture_output=True).returncode,0)
  self.assertNotEqual(subprocess.run([str(BIN),'unknown'],capture_output=True).returncode,0)
if __name__=='__main__':unittest.main()
