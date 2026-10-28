#!/usr/bin/env python3
"""Original Coax source through generated C, native diagnostics and real LLDB."""
from pathlib import Path
import hashlib, json, os, shutil, subprocess, tempfile, unittest
ROOT=Path(__file__).resolve().parents[1]
COAX=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'bin/coax')).resolve()
class Debugging(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory(prefix='coax-debug-');self.addCleanup(self.tmp.cleanup);self.d=Path(self.tmp.name)
 def call(self,*args):return subprocess.run([str(COAX),*map(str,args)],capture_output=True,text=True)
 def test_default_output_stays_reproducible_and_opt_in_records_source(self):
  source=self.d/'entry.coax';source.write_text('main :: () {n:=41;n+=1;#assert n==42;}')
  plain=self.call('c',source);self.assertEqual(plain.returncode,0,plain.stderr);self.assertNotIn('#line ',plain.stdout)
  mapping=self.d/'map.json';timing=self.d/'timing.json'
  debug=self.call('c',source,'--debug-info','--source-map',mapping,'--timings',timing)
  self.assertEqual(debug.returncode,0,debug.stderr);self.assertIn('#line 1 "'+str(source)+'"',debug.stdout)
  data=json.loads(mapping.read_text());self.assertEqual(data['schema_version'],1)
  self.assertEqual(data['source_hashes'][str(source.resolve())],hashlib.sha256(source.read_bytes()).hexdigest())
  f=next(x for x in data['functions'] if x['name']=='main')
  self.assertEqual(f['file'],str(source));self.assertIn({'name':'n','c_name':'l_n','type':'int','role':'local','line':1},f['variables'])
  t=json.loads(timing.read_text());self.assertTrue(t['success']);self.assertGreater(t['generated_bytes'],0)
  for phase in ['load','lex','imports','parse','check','evaluate','authority','emit']:
   self.assertIn(phase,t['seconds']);self.assertGreaterEqual(t['seconds'][phase],0)
 def test_loaded_function_source_step_values_and_stack(self):
  source=self.d/'main.coax';helper=self.d/'helper.coax';mapping=self.d/'map.json';exe=self.d/'app'
  helper.write_text('twice :: (value:int)->int {\n total:=value*2;\n total+=2;\n return total;\n}\n')
  source.write_text('#load "helper.coax";\nmain :: () {answer:=twice(20);#assert answer==42;}\n')
  result=self.call('build',source,'-o',exe,'--source-map',mapping,'--timings',self.d/'time.json')
  self.assertEqual(result.returncode,0,result.stderr)
  t=json.loads((self.d/'time.json').read_text());self.assertIn('native_compile',t['seconds']);self.assertIn('link',t['seconds'])
  commands=self.d/'lldb.txt'
  commands.write_text('settings set target.disable-aslr false\ncommand script import '+str(ROOT/'tools/coax_lldb.py')+'\ncoax-map '+str(mapping)+'\nbreakpoint set --file helper.coax --line 3\nrun\ncoax-vars\ncoax-bt\nthread step-over\ncoax-vars\ncontinue\n')
  run=subprocess.run(['xcrun','lldb','--batch','--source',str(commands),str(exe)],capture_output=True,text=True)
  self.assertEqual(run.returncode,0,run.stdout+run.stderr)
  for text in ['helper.coax:3','helper.coax:4','value = 20','total = 40','total = 42','twice at','main at']:
   self.assertIn(text,run.stdout)
 def test_lowered_assignment_steps_once_with_the_completed_value(self):
  src=self.d/'steps.coax';src.write_text('work :: (value:int)->int {\n values:=[1]int.[value*2];\n values[0]+=increment();\n total:=values[0];\n return total;\n}\nincrement :: ()->int {return 2;}\nmain :: () {answer:=work(20);#assert answer==42;}')
  built=self.call('build',src,'-o',self.d/'app');self.assertEqual(built.returncode,0,built.stderr)
  commands=self.d/'lldb.txt';commands.write_text('settings set target.disable-aslr false\nbreakpoint set --file steps.coax --line 3\nrun\nframe variable l_values.e[0]\nthread step-over\nframe variable l_values.e[0]\ncontinue\n')
  run=subprocess.run(['xcrun','lldb','--batch','--source',str(commands),str(self.d/'app')],capture_output=True,text=True)
  self.assertEqual(run.returncode,0,run.stdout+run.stderr)
  self.assertIn('l_values.e[0] = 40',run.stdout);self.assertIn('l_values.e[0] = 42',run.stdout);self.assertIn('steps.coax:4',run.stdout)
 def test_native_diagnostic_retains_loaded_source(self):
  helper=self.d/'helper.coax';helper.write_text('missing :: (n:int)->int #foreign;\nleaf :: ()->int {return missing(1);}\n')
  source=self.d/'entry.coax';source.write_text('#load "helper.coax";\nmain :: () {answer:=leaf();}\n')
  code=self.call('c',source,'--debug-info');self.assertEqual(code.returncode,0,code.stderr)
  c=self.d/'program.c';c.write_text(code.stdout.replace("return missing(","return undeclared_native_symbol("))
  compile=subprocess.run(['cc','-std=c11','-Werror=unused-variable','-c',str(c),'-o',str(self.d/'bad.o')],capture_output=True,text=True)
  self.assertNotEqual(compile.returncode,0);self.assertIn(str(helper)+':2:',compile.stderr)
 def test_metadata_paths_escape_quotes_and_backslashes(self):
  source=self.d/'quote"back\\name.coax';source.write_text('main :: () {n:=1;#assert n==1;}')
  result=self.call('build',source,'-o',self.d/'app','--source-map',self.d/'map.json')
  self.assertEqual(result.returncode,0,result.stderr)
  self.assertEqual(json.loads((self.d/'map.json').read_text())['functions'][0]['file'],str(source))
 def test_sanitizer_abort_stack_has_original_coax_frame(self):
  helper=self.d/'leaf.coax';helper.write_text('leaf :: (index:int)->int #export_c {\n values:=[1]int.[7];\n return values[index];\n}\n')
  src=self.d/'entry.coax';src.write_text('#load "leaf.coax";\n')
  emitted=self.call('c',src,'--library','--debug-info','--header',self.d/'lib.h');self.assertEqual(emitted.returncode,0,emitted.stderr)
  c=self.d/'lib.c';c.write_text(emitted.stdout)
  host=self.d/'host.c';host.write_text('#include "lib.h"\nint main(void){return (int)leaf(1);}')
  built=subprocess.run(['cc','-std=c11','-O0','-g','-fsanitize=address,undefined',str(c),str(host),'-o',str(self.d/'fault')],capture_output=True,text=True)
  self.assertEqual(built.returncode,0,built.stderr)
  run=subprocess.run([str(self.d/'fault')],capture_output=True,text=True,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:handle_abort=1'})
  self.assertNotEqual(run.returncode,0);self.assertIn('AddressSanitizer',run.stderr)
  self.assertIn('leaf.coax:3',run.stderr);self.assertIn('leaf',run.stderr)
 def test_debugger_rejects_stale_source(self):
  src=self.d/'entry.coax';src.write_text('main :: () {}')
  built=self.call('build',src,'-o',self.d/'app','--source-map',self.d/'map.json');self.assertEqual(built.returncode,0,built.stderr)
  src.write_text('main :: () {} // changed')
  commands=self.d/'lldb.txt';commands.write_text('command script import '+str(ROOT/'tools/coax_lldb.py')+'\ncoax-map '+str(self.d/'map.json')+'\n')
  run=subprocess.run(['xcrun','lldb','--batch','--source',str(commands),str(self.d/'app')],capture_output=True,text=True)
  self.assertNotEqual(run.returncode,0);self.assertIn('source changed since compilation',run.stdout+run.stderr)
 def test_private_names_from_two_libraries_use_their_source_owner(self):
  for letter,name in [('a','first'),('b','second')]:
   src=self.d/(letter+'.coax');src.write_text('adjust :: ('+name+':int)->int {\n total:='+name+'+1;\n return total;\n}\nf'+letter+' :: ()->int #export_c {return adjust(20);}')
   built=self.call('lib',src,'-o',self.d/(letter+'.o'),'--source-map',self.d/(letter+'.json'));self.assertEqual(built.returncode,0,built.stderr)
  host=self.d/'host.c';host.write_text('long long fa(void);long long fb(void);int main(void){return fa()+fb()==42?0:1;}')
  built=subprocess.run(['cc','-g',str(host),str(self.d/'a.o'),str(self.d/'b.o'),'-o',str(self.d/'app')],capture_output=True,text=True);self.assertEqual(built.returncode,0,built.stderr)
  commands=self.d/'lldb.txt';commands.write_text('settings set target.disable-aslr false\ncommand script import '+str(ROOT/'tools/coax_lldb.py')+'\ncoax-map '+str(self.d/'a.json')+'\ncoax-map '+str(self.d/'b.json')+'\nbreakpoint set --file a.coax --line 3\nbreakpoint set --file b.coax --line 3\nrun\ncoax-vars\ncontinue\ncoax-vars\ncontinue\n')
  run=subprocess.run(['xcrun','lldb','--batch','--source',str(commands),str(self.d/'app')],capture_output=True,text=True)
  self.assertEqual(run.returncode,0,run.stdout+run.stderr);self.assertIn('first = 20',run.stdout);self.assertIn('second = 20',run.stdout)
 def test_output_failure_is_not_a_successful_profile(self):
  source=self.d/'entry.coax';source.write_text('main :: () {}')
  timing=self.d/'time.json';r=self.call('c',source,'--source-map',self.d/'missing/map.json','--timings',timing)
  self.assertNotEqual(r.returncode,0);self.assertFalse(json.loads(timing.read_text())['success'])
if __name__=='__main__':unittest.main()
