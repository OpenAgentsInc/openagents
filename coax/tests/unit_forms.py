#!/usr/bin/env python3
"""Independent sparse-dictionary oracle; never calls Coax to form its answers."""
from collections import Counter
from pathlib import Path
import hashlib,itertools,json,os,platform,random,struct,subprocess,tempfile,time
ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'coax/lib/compiler/unit-forms.coax';NATIVE=ROOT/'coax/tests/unit-forms-native.c'
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
MAX=(1<<31)-1;SEED=0x55464f524d533032;EMPTY=(0,)*17
NAMES=['one','atom','valid','equal','product','quotient']
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def form(items):
    ordered=sorted((k,v) for k,v in dict(items).items() if v)
    assert len(ordered)<=8
    return (len(ordered),*(k for k,v in ordered),*([0]*(8-len(ordered))),*(v for k,v in ordered),*([0]*(8-len(ordered))))
def valid(a):
    n=a[0]
    return (0<=n<=8 and all(1<=k<=MAX for k in a[1:1+n]) and
            list(a[1:1+n])==sorted(set(a[1:1+n])) and
            all(0<abs(p)<=MAX for p in a[9:9+n]) and not any(a[1+n:9]) and not any(a[9+n:17]))
def expected(op,a,b):
    if op==0:return (0,*EMPTY,0)
    if op==1:
        return (0,*form({a[1]:1}),0) if 1<=a[1]<=MAX else (1,*EMPTY,0)
    if op==2:return (0,*EMPTY,int(valid(a)))
    if op==3:return (0,*EMPTY,int(valid(a) and valid(b) and a==b))
    if not valid(a) or not valid(b):return (1,*EMPTY,0)
    # Python dictionary addition, not a second sorted-merge implementation.
    d={a[1+i]:a[9+i] for i in range(a[0])}
    for i in range(b[0]):d[b[1+i]]=d.get(b[1+i],0)+(b[9+i] if op==4 else -b[9+i])
    d={k:v for k,v in d.items() if v}
    if any(abs(p)>MAX for p in d.values()):return (2,*EMPTY,0)
    if len(d)>8:return (3,*EMPTY,0)
    return (0,*form(d),0)
def cases():
    rows=[]
    def add(op,a=EMPTY,b=EMPTY):rows.append((op,tuple(a),tuple(b)))
    add(0)
    for atom in [-(1<<63),-1,0,1,2,65536,MAX,MAX+1,(1<<63)-1]:
        a=list(EMPTY);a[1]=atom;add(1,a)
    small=[form(dict(zip([1,2,3],powers))) for powers in itertools.product([-2,-1,0,1,2],repeat=3)]
    for a in small:add(2,a)
    for a,b in itertools.product(small,repeat=2):
        for op in [3,4,5]:add(op,a,b)
    for x,y in itertools.product([-MAX,-MAX+1,-65536,-1,1,65536,MAX-1,MAX],repeat=2):
        for op in [3,4,5]:add(op,form({MAX:x}),form({MAX:y}))
    for na,nb,p in itertools.product(range(9),range(9),[1,-1,MAX,-MAX]):
        a=form({2*i+1:p for i in range(na)});b=form({2*i+2:p for i in range(nb)})
        for op in [3,4,5]:add(op,a,b)
    base=form({10*(i+1):i+1 for i in range(8)})
    for lane,mode in itertools.product(range(8),range(9)):
        a=list(base)
        if mode==0:a[1+lane]=0
        if mode==1:a[1+lane]=MAX+1
        if mode==2:a[9+lane]=0
        if mode==3:a[9+lane]=MAX+1
        if mode==4:a[9+lane]=-MAX-1
        if mode==5:a[1+lane]=-(1<<63)
        if mode==6:a[9+lane]=(1<<63)-1
        if mode==7:a=list(EMPTY);a[1+lane]=1
        if mode==8:a=list(EMPTY);a[9+lane]=1
        add(2,a)
        for op in [3,4,5]:add(op,a,EMPTY);add(op,EMPTY,a)
    for n in [-1,9,-(1<<63),(1<<63)-1]:
        a=list(EMPTY);a[0]=n;add(2,a)
        for op in [3,4,5]:add(op,a,EMPTY);add(op,EMPTY,a)
    for lane,duplicate in itertools.product(range(1,8),[False,True]):
        a=list(base);a[1+lane]=a[lane]-(0 if duplicate else 1);add(2,a)
        for op in [3,4,5]:add(op,a,EMPTY);add(op,EMPTY,a)
    for count,field in itertools.product(range(8),[1,9]):
        a=list(form({10*(i+1):1 for i in range(count)}));a[field+count]=1;add(2,a)
        for op in [3,4,5]:add(op,a,EMPTY);add(op,EMPTY,a)
    for mode in [0,1]:
        a=form({2*i+1:(MAX if i==7 else 1) for i in range(8)})
        b=form({**{2*i+2:1 for i in range(7)},15:(1 if mode==0 else -1)})
        for op in [4,5]:add(op,a,b)
        b=list(b);b[0]=9
        for op in [4,5]:add(op,a,b)
    for lane in range(8):
        a=list(base);a[1+lane]+=1;add(3,base,a);add(3,a,base)
        a=list(base);a[9+lane]*=-1;add(3,base,a);add(3,a,base)
    rng=random.Random(SEED)
    for _ in range(12000):
        def random_form():
            ids=rng.sample([*range(1,31),65535,65536,MAX-1,MAX],rng.randrange(9))
            return form({k:rng.choice([-MAX,-MAX+1,-65536,-3,-1,1,3,65536,MAX-1,MAX]) for k in ids})
        a=random_form();b=random_form();negative=form({a[i+1]:-a[i+9] for i in range(a[0])})
        for op in [3,4,5]:add(op,a,b)
        add(4,a,negative);add(5,a,a);add(2,a);add(3,a,a)
    return rows
def main():
    work=Path(os.environ.get('COAX_UNIT_FORMS_RECORD_DIR',tempfile.mkdtemp(prefix='coax-unit-forms-')));work.mkdir(parents=True,exist_ok=True)
    rows=cases();answers=[expected(*r) for r in rows]
    assert len(rows)==132956,len(rows)
    assert set(r[0] for r in rows)==set(range(6))
    assert set(r[0] for r in answers)=={0,1,2,3}
    (work/'inputs.bin').write_bytes(struct.pack('<Q',len(rows))+b''.join(struct.pack('<35q',op,*a,*b) for op,a,b in rows))
    truth=b''.join(struct.pack('<19q',*r) for r in answers);(work/'expected.bin').write_bytes(truth)
    record={'schema':'coax-unit-forms-qualification-v1','seed':SEED,'rows':len(rows),'operation_counts':dict(Counter(NAMES[r[0]] for r in rows)),
            'status_counts':dict(Counter(['ok','metadata','power','capacity'][r[0]] for r in answers)),
            'oracle':'Independent Python arbitrary-precision sparse dictionary accumulation followed by sorting; no sorted-merge producer calls.',
            'profile':'Canonical <=8 sorted nominal atoms, IDs1..2147483647, nonzero powers within symmetric ±2147483647, zero inactive lanes; metadata before power before capacity.',
            'scope':'Separate producer; no compiler integration, physical metadata, registry ownership, source operator or full R12 qualification.',
            'input_sha256':sha(work/'inputs.bin'),'expected_sha256':sha(work/'expected.bin'),'platform':platform.platform(),'machine':platform.machine(),
            'source_sha256':{str(p.relative_to(ROOT)):sha(p) for p in [SOURCE,NATIVE,Path(__file__).resolve()]},'compiler_sha256':sha(COMPILER),'commands':[],'modes':[]}
    def command(name,args):
        start=time.monotonic();p=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,timeout=180)
        (work/(name+'.stdout')).write_bytes(p.stdout);(work/(name+'.stderr')).write_bytes(p.stderr)
        record['commands'].append({'name':name,'arguments':[str(x) for x in args],'exit':p.returncode,'elapsed_seconds':time.monotonic()-start})
        (work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
        assert p.returncode==0,(name,p.stderr.decode(errors='replace'))
        if name.startswith('run-'):assert not p.stderr,(name,p.stderr.decode(errors='replace'))
        return p
    record['compiler_version']=command('compiler-version',[COMPILER,'version']).stdout.decode().strip()
    cc=os.environ.get('CC','cc');record['c_compiler_version']=command('cc-version',[cc,'--version']).stdout.decode()
    generated=command('generate-c',[COMPILER,'c',SOURCE,'--library','--header',work/'unit-forms.h']);(work/'unit-forms.c').write_bytes(generated.stdout)
    record['generated_c_sha256']=sha(work/'unit-forms.c');record['generated_header_sha256']=sha(work/'unit-forms.h')
    for mode,flags in [('debug',['-O0','-g']),('release',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
        command('compile-'+mode,[cc,'-std=c11',*flags,'-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-variable','-Wno-unused-parameter','-Wno-misleading-indentation','-I'+str(work),work/'unit-forms.c',NATIVE,'-lm','-o',work/mode])
        command('run-'+mode,[work/mode,work/'inputs.bin',work/(mode+'.bin')]);have=(work/(mode+'.bin')).read_bytes()
        assert len(have)==len(truth),(mode,len(have),len(truth))
        if have!=truth:
            for i,(actual,want) in enumerate(zip(struct.iter_unpack('<19q',have),answers)):assert actual==want,(mode,i,rows[i],actual,want)
            raise AssertionError('transcript mismatch')
        record['modes'].append({'mode':mode,'passed':True,'exact_results':len(rows),'exact_words':len(rows)*19,'stderr_bytes':0,'output_sha256':sha(work/(mode+'.bin'))})
    record['passed']=True;(work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'passed':True,'rows':len(rows),'words_per_mode':len(rows)*19,'record':str(work/'receipt.json')}))
if __name__=='__main__':main()
