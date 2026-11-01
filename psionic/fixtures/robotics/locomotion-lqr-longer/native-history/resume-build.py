"""Approved transport-only casts; carry passed emissions/debug object, first guards."""
from pathlib import Path
import importlib.util,json,os,subprocess,time
ROOT=Path('/Users/christopherdavid/.codex/worktrees/robotics-foundation-release/openagents');out=Path(__file__).parent
sp=importlib.util.spec_from_file_location('qualifier',ROOT/'psionic/tests/robotics/locomotion_lqr_longer_native_reference.py');m=importlib.util.module_from_spec(sp);sp.loader.exec_module(m)
coax=Path('/Users/christopherdavid/.cache/coax-release/03bc47ea02498ddc669ad8102a41dc825562eb50ba7342a9201f18403f9ca858/coax');r=m.load(out/'fifth-attempt.record.json');m.need(r['error']=="ValueError('compile-effects-debug exit/empty stderr')" and not r['passed'] and not r['modes'],'documented compile failure before native effects');r.pop('error');start=time.monotonic();e,before=m.snapshot()
old=(out/'fifth-attempt.native.c').read_text();new=old.replace('(coax_v_t_RltIteration){p+32,count}','(coax_v_t_RltIteration){(RltIteration*)(p+32),count}').replace('(coax_v_t_RltState){p+32,count}','(coax_v_t_RltState){(RltState*)(p+32),count}').replace('(coax_v_t_RltIteration){p+48,1}','(coax_v_t_RltIteration){(RltIteration*)(p+48),1}').replace('(coax_v_t_RltState){p+48,1}','(coax_v_t_RltState){(RltState*)(p+48),1}')
m.need(new==m.NATIVE.read_text() and new!=old,'ONLY four explicit aligned view pointer casts');m.need(before['frozen']==r['frozen'] and all(h==before['source'][n] for n,h in r['source'].items() if n!=str(m.NATIVE.relative_to(ROOT))),'all other frozen/source inputs exact');m.need(m.identity(coax)==r['compiler'],'same compiler');tool=m.identity(__file__)
obj=out/'native-debug/coax.o';r['build_resume']={'tool':tool,'failed_record':m.identity(out/'fifth-attempt.record.json'),'failed_driver':m.identity(out/'fifth-attempt.driver.py'),'failed_native_C':m.identity(out/'fifth-attempt.native.c'),'new_native_C':m.identity(m.NATIVE),'carried_debug_generated_object':m.identity(obj),'scope':'Only four typed view pointer casts. Both passed C/H emissions, closure comparisons and debug generated object carried. No passed stage repeated.'};r.update(before);r['old_C_closure_manifest']=m.identity(out/'old-native-closure-tokens.json')
def save():m.write(out/'record.json',r)
def cmd(label,args,env=None):
 x={'label':label,'argv':list(map(str,args))};r['commands'].append(x);began=time.monotonic()
 try:
  z=subprocess.run(x['argv'],cwd=ROOT,env=env,capture_output=True,timeout=180);(out/(label+'.stdout')).write_bytes(z.stdout);(out/(label+'.stderr')).write_bytes(z.stderr);x.update(exit=z.returncode,seconds=time.monotonic()-began);m.need(z.returncode==0 and not z.stderr,label+' exit/empty stderr');return z.stdout
 except subprocess.TimeoutExpired as ex:
  (out/(label+'.stdout')).write_bytes(ex.stdout or b'');(out/(label+'.stderr')).write_bytes(ex.stderr or b'');x['timed_out']=True;raise
 finally:save()
save()
try:
 for mode,em in r['emissions'].items():
  d=out/mode;m.need(m.identity(d/'locomotion-lqr-longer.c')==em['C'] and m.identity(d/'locomotion-lqr-longer.h')==em['header'] and m.identity(d/'locomotion-lqr.h')==em['header_alias'],'passed emission/alias identity')
 common=['-std=c11','-Wall','-Wextra','-Werror','-ffp-contract=off','-fno-fast-math',*['-fno-builtin-'+x for x in ['exp','log','sqrt','cos','pow']]];payload=[]
 for mode,sm,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
  d=out/('native-'+mode);d.mkdir(exist_ok=mode=='debug');c=out/sm/'locomotion-lqr-longer.c';h=out/sm/'locomotion-lqr-longer.h';cc=os.environ.get('CC','cc')
  if mode!='debug':cmd('compile-generated-'+mode,[cc,*common,'-Wno-unused-parameter',*flags,'-c',c,'-o',d/'coax.o'])
  cmd(('resume-' if mode=='debug' else '')+'compile-effects-'+mode,[cc,*common,*flags,'-I'+str(h.parent),m.NATIVE,d/'coax.o','-lm','-o',d/'native']);env=os.environ.copy();env.update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');report=json.loads(cmd('execute-guards-'+mode,[d/'native','guards',d],env));m.need(report['passed'] and report['full_batches']==0,'zero full-batch guards');raw=(d/'synthetic-resume.bin').read_bytes();m.need(len(raw)==440,'440B raw state');payload.append(raw);r['modes'][mode]={'native':report,'binary':m.identity(d/'native'),'C':m.identity(c),'header':m.identity(h),'synthetic_resume':m.identity(d/'synthetic-resume.bin')};save()
 m.need(all(x==payload[0] for x in payload),'three mode bytes exact');r['all_mode_output_bytes_exact']=True
 _,after=m.snapshot();m.need(before==after and m.identity(coax)==r['compiler'] and m.identity(__file__)==tool and m.identity(obj)==r['build_resume']['carried_debug_generated_object'],'all context final identities');r['passed']=True
except BaseException as ex:r['error']=repr(ex);raise
finally:r['build_resume_seconds']=time.monotonic()-start;r['driver']=m.identity(ROOT/'psionic/tests/robotics/locomotion_lqr_longer_native_reference.py');save()
print(json.dumps({'passed':True,'record':m.identity(out/'record.json'),'modes':{n:x['native'] for n,x in r['modes'].items()}}))
