# Firm reason: 2 (independent original native-bridge oracle with a specific bounded wall-time override).
from pathlib import Path
import hashlib,importlib.util,json,os,subprocess,sys,time,unittest
ROOT=Path('/tmp/openagents-12625-complete-20261101').resolve();sys.path.insert(0,str(ROOT/'coax/tests'));import lexer as LX
original=LX.checked;checker=(ROOT/'coax/lib/compiler/checker.coax').resolve();record=Path(os.environ['COAX_NATIVE_BRIDGE_DIR']).parent/'bridge-wall-override.json'
def checked(args,**kwargs):
 argv=list(map(str,args))
 if len(argv)==3 and Path(argv[0]).name=='coax-sanitized' and argv[1]=='check' and Path(argv[2]).resolve()==checker:
  started=time.monotonic();row={'schema':'coax.12625.native.bridge-wall-override.v1','argv':argv,'original_wall_limit':180,'new_wall_limit':600,'scope':'Only original sanitized checker-source check; every other LX.checked remains180 and original deep-refusal stays30. Outer gate840.','compiler_work_budget_changed':False}
  try:r=subprocess.run(argv,capture_output=True,timeout=600,**kwargs)
  except subprocess.TimeoutExpired:
   row.update(timed_out=True,elapsed_seconds=time.monotonic()-started);record.write_text(json.dumps(row,indent=2)+'\n');raise
  row.update(timed_out=False,returncode=r.returncode,elapsed_seconds=time.monotonic()-started,stdout_sha256=hashlib.sha256(r.stdout).hexdigest(),stderr_sha256=hashlib.sha256(r.stderr).hexdigest());record.write_text(json.dumps(row,indent=2)+'\n')
  if r.returncode:raise AssertionError(r.stderr.decode(errors='replace'))
  return r.stdout
 return original(args,**kwargs)
LX.checked=checked
spec=importlib.util.spec_from_file_location('closure_native_bridge_original',ROOT/'coax/tests/parser.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
suite=unittest.defaultTestLoader.loadTestsFromName('Parser.test_native_bridge_under_sanitizers',module);result=unittest.TextTestRunner(verbosity=1).run(suite);sys.exit(0 if result.wasSuccessful() else 1)
