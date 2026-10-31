#!/usr/bin/env python3
"""Default source binding and NEW build-dispatch mocks; no SDK/NN execution."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess
ROOT=Path(__file__).resolve().parents[3]
def require(ok,msg):
    if not ok:raise RuntimeError(msg)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def legacy_source(text):
    stack=[];out=[]
    for line in text.splitlines(keepends=True):
        s=line.strip();active=all(x[1] for x in stack if x[0])
        if s=='#ifdef PANDA_OWNED_QUERY_V1':stack.append([True,False]);continue
        if s.startswith(('#if ','#ifdef ','#ifndef ')):
            if active:out.append(line)
            stack.append([False,True]);continue
        if s=='#else' and stack and stack[-1][0]:stack[-1][1]=not stack[-1][1];continue
        if s=='#endif':
            require(bool(stack),'preprocessor nesting');target=stack.pop()[0]
            if not target and active:out.append(line)
            continue
        if active:out.append(line)
    require(not stack,'preprocessor complete');return ''.join(out)
TOKEN=re.compile(r'/\*.*?\*/|//[^\n]*|\s+|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|[A-Za-z_]\w*|(?:\d|\.\d)[A-Za-z0-9_.]*|>>=|<<=|\.\.\.|==|!=|<=|>=|&&|\|\||\+\+|--|->|\+=|-=|\*=|/=|&=|\|=|\^=|<<|>>|##|.',re.S)
def tokens(s):return [x for x in TOKEN.findall(s) if not(x.isspace() or x.startswith('//') or x.startswith('/*'))]
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--base',required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--shader-cache',type=Path,required=True);p.add_argument('--actual-build',type=Path,required=True);p.add_argument('--native-record',type=Path,required=True);a=p.parse_args();out=a.output.resolve();require(not out.exists(),'preserve proof');out.mkdir(parents=True);r={'schema':'psionic.panda-query-runtime.wiring-qualification.v1','passed':False,'base_commit':a.base,'source_sha256':{},'legacy_bindings':[],'shell_mocks':[],'SDK_calls':0,'NN_calls':0};record=out/'record.json'
    def save():record.write_text(json.dumps(r,indent=2)+'\n')
    save()
    try:
        for name in ['panda_loop_effects.h','panda_loop_metal.c']:
            path=ROOT/'psionic/tests/robotics'/name;relative=str(path.relative_to(ROOT));old=subprocess.check_output(['git','show',a.base+':'+relative],cwd=ROOT).decode();current=path.read_text();before=tokens(old);after=tokens(legacy_source(current));require(before==after,('legacy C tokens',name));r['source_sha256'][relative]=sha(path);r['legacy_bindings'].append({'file':relative,'base_bytes_sha256':hashlib.sha256(old.encode()).hexdigest(),'current_bytes_sha256':sha(path),'tokens':len(before),'token_sha256':hashlib.sha256(json.dumps(before,separators=(',',':')).encode()).hexdigest(),'equal':True})
        r['legacy_scope']='Exact token streams before macro expansion, retaining all string/number literals. Whitespace/comments and disabled NEW branch are excluded. Physical source positions/expanded diagnostic __LINE__ values can differ; no executable-byte equality claim.'
        helper=out/'mock-compiler';helper.write_text('''#!/usr/bin/env python3
import json,os,pathlib,sys
args=sys.argv[1:]
with open(os.environ['PQN_MOCK_LOG'],'a')as f:f.write(json.dumps({'program':pathlib.Path(sys.argv[0]).name,'argv':args})+'\\n')
if pathlib.Path(sys.argv[0]).name=='mock-coax':
 if '--header'in args:pathlib.Path(args[args.index('--header')+1]).write_text('/* MOCK ONLY */\\n')
 print('/* MOCK ONLY */')
elif '-o'in args:pathlib.Path(args[args.index('-o')+1]).write_text('MOCK ONLY; not executable')
''');helper.chmod(0o755);coax=out/'mock-coax';cc=out/'mock-cc';coax.symlink_to(helper.name);cc.symlink_to(helper.name)
        wrapper=ROOT/'psionic/tests/robotics/run-panda-loop-metal.sh';r['source_sha256'][str(wrapper.relative_to(ROOT))]=sha(wrapper);r['source_sha256'][str(Path(__file__).resolve().relative_to(ROOT))]=sha(__file__)
        baseflags=['-std=c11','-ffp-contract=off','-fno-fast-math','-Wno-deprecated-declarations','-O1','-g','-fsanitize=address,undefined'];ownedflags=baseflags+['-DPANDA_OWNED_QUERY_V1=1','-D_DARWIN_C_SOURCE','-fno-builtin-sin','-fno-builtin-cos','-fno-sanitize-recover=all','-fno-omit-frame-pointer']
        for owned in [False,True]:
            name='owned'if owned else'legacy';log=out/(name+'.commands.jsonl');env=os.environ.copy();env.pop('PANDA_QUERY_SOURCE',None);env.update(COAX=str(coax),CC=str(cc),MUJOCO_ROOT='/unused-mock-SDK',PANDA_SHADER_CACHE=str(a.shader_cache.resolve()),PANDA_BUILD_ONLY='1',PANDA_NATIVE_MODE='sanitize',PQN_MOCK_LOG=str(log))
            if owned:env['PANDA_QUERY_SOURCE']='owned-shadow-v1'
            argv=[str(wrapper),'unused-shards','unused-pst','unused-noise','unused-pairs',str(out/name),'1','0','000'];x=subprocess.run(argv,cwd=ROOT,env=env,capture_output=True,timeout=30);(out/(name+'.stdout')).write_bytes(x.stdout);(out/(name+'.stderr')).write_bytes(x.stderr);require(x.returncode==0 and not x.stderr and b'build only'in x.stdout,'build-only dispatch');calls=[json.loads(x)for x in log.read_text().splitlines()];require(len(calls)==4,'two Coax emissions/two C invocations only');aggregate='panda-query-runtime.coax'if owned else'panda-clock-session.coax';require(calls[0]['program']=='mock-coax'and calls[0]['argv'][:2]==['c','psionic/tests/robotics/'+aggregate],'source dispatch');require(calls[1]['argv'][:2]==['c','psionic/lib/robotics/panda-policy.coax'],'policy unchanged');last=calls[-1]['argv'];end=next(i for i,s in enumerate(last)if s.startswith('-I'));actual=last[:end];require(actual==(ownedflags if owned else baseflags),'exact numeric flags');r['shell_mocks'].append({'owned':owned,'aggregate':aggregate,'flags':actual,'calls':calls,'result_stdout_sha256':hashlib.sha256(x.stdout).hexdigest(),'model_or_SDK_or_binary_execution':False})
        native=json.loads(a.native_record.read_text());release=next(x for x in native['modes']if x['mode']=='release');build=a.actual_build/'build';require(sha(build/'panda-session.c')==release['C_sha256']and sha(build/'panda-session.h')==release['header_sha256'],'actual linked session exact to independently qualified releaseC/H');require((build/'panda-loop').is_file(),'actual linked binary');r['real_owned_build']={'directory':str(a.actual_build),'C_sha256':sha(build/'panda-session.c'),'header_sha256':sha(build/'panda-session.h'),'binary_sha256':sha(build/'panda-loop'),'session_matches_qualified_release':True,'binary_executed':False};r['passed']=True
    except BaseException as e:r['failure']=repr(e);raise
    finally:save()
    print(json.dumps({'passed':True,'record':str(record)}))
if __name__=='__main__':main()
