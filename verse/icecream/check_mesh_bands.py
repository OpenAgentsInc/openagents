#!/usr/bin/env python3
"""Checked Coax meshes against an explicitly repaired independent C++ oracle.

The external source tree is read only. A private copy partitions wall/trim
bands and culls covered doorway caps using actual emitted post bounds.
Original fixtures remain historical. Every child and the whole gate are bounded.
"""
import argparse, hashlib, json, os, re, shlex, signal, subprocess, sys, tempfile, time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
BAND_OLD='''      piece(s0, s1, 0, kWallHeight, colour, 0);
      piece(s0, s1, 0, 0.28f, trim, 0.05f);
      piece(s0, s1, kWallHeight - 0.22f, kWallHeight, trim, 0.05f);'''
BAND_NEW='''      const size_t bandBegin = mesh.v.size();
      piece(s0, s1, 0.28f, kWallHeight - 0.22f, colour, 0);
      const size_t lowerBegin = mesh.v.size();
      piece(s0, s1, 0, 0.28f, trim, 0.05f);
      const size_t upperBegin = mesh.v.size();
      piece(s0, s1, kWallHeight - 0.22f, kWallHeight, trim, 0.05f);
      // Independent oracle: observe existing trim geometry, not Coax seam math.
      float bottomSeam = mesh.v[lowerBegin].p.y;
      float topSeam = mesh.v[upperBegin].p.y;
      for (size_t j=lowerBegin;j<upperBegin;++j) bottomSeam=std::max(bottomSeam,mesh.v[j].p.y);
      for (size_t j=upperBegin;j<mesh.v.size();++j) topSeam=std::min(topSeam,mesh.v[j].p.y);
      for (size_t j=bandBegin;j<lowerBegin;++j)
        mesh.v[j].p.y = mesh.v[j].p.y < kWallHeight/2 ? bottomSeam : topSeam;'''

def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
CPP_POST_OLD='''      piece(mid - gap, mid + gap, kDoorHeight, kWallHeight, colour, 0);
      piece(mid - gap - 0.12f, mid - gap, 0, kDoorHeight, frame, 0.06f);
      piece(mid + gap, mid + gap + 0.12f, 0, kDoorHeight, frame, 0.06f);
      piece(mid - gap - 0.12f, mid + gap + 0.12f, kDoorHeight - 0.12f, kDoorHeight, frame, 0.06f);'''
CPP_POST_NEW='''      piece(mid - gap, mid + gap, kDoorHeight, kWallHeight, colour, 0);
      const size_t leftPostBegin = mesh.v.size();
      piece(mid - gap - 0.12f, mid - gap, 0, kDoorHeight, frame, 0.06f);
      const size_t rightPostBegin = mesh.v.size();
      piece(mid + gap, mid + gap + 0.12f, 0, kDoorHeight, frame, 0.06f);
      const size_t postsEnd = mesh.v.size();
      piece(mid - gap - 0.12f, mid + gap + 0.12f, kDoorHeight - 0.12f, kDoorHeight, frame, 0.06f);
      // Independent oracle: the original boxed primitive is a closed solid.
      // Read its actual bounds after emission, then remove only contained caps.
      auto trimCovered = [&](std::pair<size_t,size_t> band, size_t post0, size_t post1, bool atEnd) {
        Vec3 pl=mesh.v[post0].p, ph=pl;
        auto bound = [](Vec3& lo, Vec3& hi, Vec3 p) {
          lo={std::min(lo.x,p.x),std::min(lo.y,p.y),std::min(lo.z,p.z)};
          hi={std::max(hi.x,p.x),std::max(hi.y,p.y),std::max(hi.z,p.z)};
        };
        for(size_t j=post0;j<post1;++j) bound(pl,ph,mesh.v[j].p);
        auto span=[&](Vec3 p){return horizontal?p.x:p.z;};
        auto across=[&](Vec3 p){return horizontal?p.z:p.x;};
        for(size_t face=band.first;face<band.second;face+=6) {
          if(span(mesh.v[face].n)*(atEnd?1.0f:-1.0f)<=.5f) continue;
          Vec3 lo=mesh.v[face].p,hi=lo;
          for(size_t j=face;j<face+6;++j) bound(lo,hi,mesh.v[j].p);
          if(span(lo)<span(pl)||span(hi)>span(ph)||across(lo)<across(pl)||across(hi)>across(ph)||lo.y<pl.y)continue;
          if(hi.y<=ph.y){Vec3 point=mesh.v[face].p;for(size_t j=face;j<face+6;++j)mesh.v[j].p=point;}
          else if(lo.y<ph.y)for(size_t j=face;j<face+6;++j)mesh.v[j].p.y=std::max(mesh.v[j].p.y,ph.y);
        }
      };
      trimCovered(capBands[0],leftPostBegin,rightPostBegin,true);
      trimCovered(capBands[1],rightPostBegin,postsEnd,false);'''
CPP_SPANS_OLD='    std::vector<std::pair<float, float>> spans;'
CPP_SPANS_NEW='    std::vector<std::pair<size_t, size_t>> capBands;\n'+CPP_SPANS_OLD

def repaired_cpp(text):
    patches=[(BAND_OLD,BAND_NEW+'\n      capBands.emplace_back(bandBegin,mesh.v.size());'),(CPP_SPANS_OLD,CPP_SPANS_NEW),(CPP_POST_OLD,CPP_POST_NEW)]
    for old,new in patches:
        if text.count(old)!=1:raise ValueError('independent C++ wall source drift: expected exactly one original patch anchor')
        text=text.replace(old,new)
    return text

def admit_reused_checks(path, expected_sources, compiler_hash, expected_command):
    checks=json.loads(Path(path).read_text())
    if checks.get('schema')!='aethyr.icecream.mesh.checks.v1' or checks.get('exit')!=0:
        raise ValueError('full-check receipt identity or exit refused')
    if checks.get('compiler_sha256')!=compiler_hash or checks.get('source_sha256')!=expected_sources:
        raise ValueError('full-check receipt compiler or complete source closure refused')
    command=checks.get('command',[])
    def canonical(argv):
        if len(argv)!=len(expected_command):return None
        result=list(argv)
        result[0]=str((ROOT/result[0]).resolve());result[2]=str((ROOT/result[2]).resolve())
        return result
    if canonical(command)!=canonical(expected_command):
        raise ValueError('full-check receipt command must be the unsliced mesh check')
    log=Path(checks.get('log_file',''))
    if not log.is_file() or sha(log)!=checks.get('log_sha256'):
        raise ValueError('full-check receipt log identity refused')
    lines=log.read_text().strip().splitlines()
    match=re.fullmatch(r'(\d+) checks, (\d+) passed, 0 failed',lines[-1] if lines else '')
    if not match or int(match[1])<1 or match[1]!=match[2] or any(line.startswith('FAIL ')for line in lines):
        raise ValueError('full-check receipt is not a complete passing check log')
    return checks

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work-dir',type=Path)
    parser.add_argument('--demo',type=Path,default=Path(os.environ.get('AETHYR_DEMO',str(Path.home()/'work/aethyr-demo'))))
    parser.add_argument('--reuse-checks',type=Path)
    parser.add_argument('--table',action='store_true',help='retain the independently repaired table in the work directory; never rewrite historical fixtures')
    a=parser.parse_args();work=(a.work_dir or Path(tempfile.mkdtemp(prefix='icecream-mesh-bands-'))).resolve();work.mkdir(parents=True,exist_ok=True)
    coax=Path(os.environ.get('COAX',str(ROOT/'coax/bin/coax'))).resolve();cc=os.environ.get('CC','/usr/bin/clang');cxx=os.environ.get('CXX','/usr/bin/clang++')
    started=time.monotonic();commands=[];active=None;before={};receipt=dict(schema='aethyr.icecream.mesh-bands.native.v1',status='failed',scope='Independent repaired C++ oracle copy versus actual normal generated-Coax meshes; original C++ fixtures stay historical.',commands=commands,whole_deadline_seconds=820,child_deadline_seconds=600,compiler_sha256=sha(coax))
    def interrupted(sig,frame): raise KeyboardInterrupt(f'signal {sig}')
    previous={sig:signal.signal(sig,interrupted)for sig in (signal.SIGINT,signal.SIGTERM)}
    def run(args,name,limit=180):
        nonlocal active
        remaining=820-(time.monotonic()-started)
        if remaining<=0: raise TimeoutError('whole geometry gate deadline')
        path=work/name;begin=time.monotonic()
        with path.open('wb')as out:
            active=subprocess.Popen(list(map(str,args)),cwd=ROOT,stdout=out,stderr=subprocess.STDOUT,start_new_session=True)
            try: code=active.wait(timeout=min(limit,remaining))
            finally:
                failure=sys.exc_info()[0]
                if active.poll()is None:
                    try:os.killpg(active.pid,signal.SIGKILL)
                    except ProcessLookupError:pass
                    active.wait()
                commands.append(dict(args=list(map(str,args)),exit=active.returncode,failure=failure.__name__ if failure else None,elapsed_seconds=round(time.monotonic()-begin,6),log=path.name,log_sha256=sha(path)))
                active=None
        if code: raise RuntimeError(f'{name}: exit {code}')
        return path
    try:
        deps=run([coax,'deps',HERE/'mesh-band-native.coax'],'deps.json')
        paths=[Path(p)for p in json.loads(deps.read_text())['sources']]
        paths += [HERE/'mesh-band-native.c',HERE/'mesh-check.c',HERE/'mesh-oracle.cpp',Path(__file__).resolve(),HERE/'test-mesh.sh']
        before={str(p):sha(p)for p in paths};receipt['source_sha256']=before
        if a.reuse_checks:
            check_deps=run([coax,'deps',HERE/'mesh.coax'],'mesh-check-deps.json')
            check_sources={str(Path(p).resolve()):sha(p)for p in json.loads(check_deps.read_text())['sources']}
            checks=admit_reused_checks(a.reuse_checks,check_sources,sha(coax),[str(coax),'test',str(HERE/'mesh.coax'),'--library','--steps','4000000000'])
            receipt['checks_reused_from']=str(a.reuse_checks.resolve());receipt['checks_receipt_sha256']=sha(a.reuse_checks)
        else:run([coax,'test',HERE/'mesh.coax','--library','--steps','4000000000'],'coax-checks.log',600)
        # Snapshot every actual user-header dependency before compiling anything.
        src=a.demo.resolve()/'src'
        if not (src/'icecream/mesh.cpp').is_file():raise FileNotFoundError('independent local C++ oracle source is required')
        cpp_inputs=set()
        for k,unit in enumerate([src/'icecream/rules.cpp',src/'icecream/mesh.cpp',HERE/'mesh-oracle.cpp']):
            dep=run([cxx,'-std=c++20','-I',src,'-MM',unit],f'cpp-deps-{k}.txt')
            cpp_inputs.update(Path(p).resolve()for p in shlex.split(dep.read_text().replace('\\\n',' ').split(':',1)[1]))
        cpp_before={str(p):sha(p)for p in cpp_inputs};receipt['cpp_original_sha256']=cpp_before
        copied=work/'cpp-source';copied.mkdir(exist_ok=True)
        for p in cpp_inputs:
            dest=copied/p.relative_to(src)if p.is_relative_to(src)else copied/'oracle-harness.cpp'
            dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(p.read_bytes());assert sha(dest)==cpp_before[str(p)]
        original=copied/'icecream/mesh.cpp';(work/'oracle-mesh-original.cpp').write_bytes(original.read_bytes())
        patched=copied/'icecream/mesh-bands.cpp';patched.write_text(repaired_cpp(original.read_text()))
        receipt['cpp_repair_sha256']=sha(patched)
        run([cxx,'-std=c++20','-O2','-ffp-contract=off','-I',copied,'-I',copied/'icecream',copied/'icecream/rules.cpp',patched,copied/'oracle-harness.cpp','-Wl,-undefined,dynamic_lookup','-o',work/'oracle'],'oracle-compile.log')
        run([work/'oracle'],'oracle.bin');receipt['oracle_sha256']=sha(work/'oracle.bin')
        run([coax,'c',HERE/'mesh-band-native.coax','--library','--header',work/'IceCream.h'],'IceCream.c',600)
        modes=[]
        for mode,flags in [('debug',['-O0','-g']),('checked-o2',['-O2']),('sanitized-o1',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            programs=[]
            for label,driver in [('mesh',HERE/'mesh-check.c'),('bands',HERE/'mesh-band-native.c')]:
                exe=work/f'{label}-{mode}'
                run([cc,'-std=c11',*flags,'-ffp-contract=off','-Wno-unused-value','-I',work,work/'IceCream.c',driver,'-lm','-o',exe],f'{label}-{mode}-compile.log')
                args=[exe,work/'oracle.bin']if label=='mesh'else[exe]
                if label=='mesh'and mode=='debug':args+=['--table',work/'repaired-oracle-table.coax']
                log=run(args,f'{label}-{mode}.log',240)
                programs.append(dict(kind=label,executable_sha256=sha(exe),log=log.name,output=log.read_text().strip()))
            modes.append(dict(mode=mode,flags=flags,sanitizers=['address','undefined']if mode=='sanitized-o1'else[],programs=programs))
        receipt['profiles']=modes
        actual=re.findall(r'OracleHouse\.\{[^\n]+?hash = (0x[0-9a-f]+)',(work/'repaired-oracle-table.coax').read_text())
        declared=re.findall(r'0x[0-9a-f]+',(HERE/'fixtures/mesh-band-oracle.coax').read_text())
        assert len(actual)==10 and actual==declared,'repaired fixture hash drift'
        receipt['source_changes']=[p for p,h in before.items()if sha(p)!=h]
        receipt['compiler_changed']=sha(coax)!=receipt['compiler_sha256']
        if receipt['source_changes']or receipt['compiler_changed']:raise RuntimeError('source/compiler changed during qualification')
        receipt['generated_c_sha256']=sha(work/'IceCream.c');receipt['header_sha256']=sha(work/'IceCream.h');receipt['status']='pass'
    except BaseException as e:
        receipt['error']=f'{type(e).__name__}: {e}'
        raise
    finally:
        if active is not None and active.poll()is None:
            try:os.killpg(active.pid,signal.SIGKILL)
            except ProcessLookupError:pass
            active.wait()
        for sig,old in previous.items():signal.signal(sig,old)
        receipt['elapsed_seconds']=round(time.monotonic()-started,6)
        (work/'qualification.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps(dict(status=receipt['status'],work_dir=str(work),profiles=[m['mode']for m in modes])))

if __name__=='__main__':
    os.environ['ASAN_OPTIONS']='detect_leaks=0:halt_on_error=1'
    os.environ['UBSAN_OPTIONS']='halt_on_error=1'
    main()
