#!/usr/bin/env python3
"""Independent Fraction oracle for the separate exact unit metadata producer.

No third-party packages, no compiler-policy imports, and no runtime dependency.
Generated inputs and all native transcripts stay in an explicit temporary dir.
"""
from collections import Counter
from fractions import Fraction
from pathlib import Path
import hashlib,itertools,json,os,platform,random,struct,subprocess,sys,tempfile,time

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'coax/lib/compiler/unit-algebra.coax'
NATIVE=ROOT/'coax/tests/unit-algebra-native.c'
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
LIMIT=1<<20
SEED=0x55414C4745425241
ONE=(0,0,0,0,1,1)
ZERO=(0,)*6

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def valid(u):return all(-8<=x<=8 for x in u[:4]) and all(1<=x<=LIMIT for x in u[4:])
def expected(op,a,b):
    # Fraction performs its own arbitrary-precision canonicalization.
    if not valid(a) or (op!=0 and not valid(b)):return (1,*ZERO)
    if op==0:dimensions=a[:4];scale=Fraction(a[4],a[5])
    else:
        if op==3 and a[:4]!=b[:4]:return (2,*ZERO)
        dimensions=tuple(x+y if op==1 else x-y for x,y in zip(a[:4],b[:4]))
        scale=Fraction(a[4],a[5])*Fraction(b[4],b[5]) if op==1 else Fraction(a[4],a[5])/Fraction(b[4],b[5])
        if any(abs(x)>8 for x in dimensions):return (2,*ZERO)
    if max(scale.numerator,scale.denominator)>LIMIT:return (3,*ZERO)
    return (0,*dimensions,scale.numerator,scale.denominator)

def cases():
    out=[]
    def add(op,a,b=ONE):out.append((op,tuple(a),tuple(b)))
    edge=[1,2,3,5,7,13,17,89,144,233,377,61681,514229,524288,832040,LIMIT-1,LIMIT]
    for n,d in itertools.product(edge,repeat=2):add(0,(2,-3,4,-5,n,d))
    for axis,a,b in itertools.product(range(4),range(-8,9),range(-8,9)):
        x=[0,0,0,0,6,10];y=[0,0,0,0,15,21];x[axis]=a;y[axis]=b
        for op in [1,2,3]:add(op,x,y)
    # Every small rational pair, including cancellation in both directions.
    for an,ad,bn,bd in itertools.product(range(1,13),repeat=4):
        a=(1,-2,3,-4,an,ad);b=(1,-2,3,-4,bn,bd)
        for op in [1,2,3]:add(op,a,b)
    for field in range(6):
        bad=[-9,9,-(1<<63),(1<<63)-1] if field<4 else [0,-1,LIMIT+1,-(1<<63),(1<<63)-1]
        for value in bad:
            a=list(ONE);a[field]=value
            add(0,a)
            for op in [1,2,3]:add(op,a,ONE);add(op,ONE,a)
    # Exact final scale boundaries and transient 2^40 products.
    specials=[(LIMIT,1,LIMIT,1),(1,LIMIT,1,LIMIT),(LIMIT,LIMIT-1,LIMIT-1,LIMIT),
              (17,1,61681,1),(1,17,1,61681),(LIMIT,1,1,1),(1,LIMIT,1,1),
              (LIMIT,2,2,1),(832040,514229,514229,832040)]
    for an,ad,bn,bd in specials:
        for op in [1,2,3]:add(op,(0,0,0,0,an,ad),(0,0,0,0,bn,bd))
    # Simultaneous failures freeze input -> dimension -> scale precedence.
    for field in range(6):
        a=[8,8,8,8,LIMIT,1];b=[8,8,8,8,LIMIT,1];b[field]=9 if field<4 else 0
        for op in [1,2,3]:add(op,a,b)
    rng=random.Random(SEED)
    for _ in range(12000):
        a=tuple(rng.randrange(-8,9) for _ in range(4))+tuple(rng.randrange(1,LIMIT+1) for _ in range(2))
        b=tuple(rng.randrange(-8,9) for _ in range(4))+tuple(rng.randrange(1,LIMIT+1) for _ in range(2))
        add(rng.randrange(4),a,b)
        # Successful high-factor products and ratios are not left to chance.
        add(1,a,tuple(-x for x in a[:4])+(a[5],a[4]))
        add(2,a,a);add(3,a,a)
    return out

def main():
    work=Path(os.environ.get('COAX_UNIT_ALGEBRA_RECORD_DIR',tempfile.mkdtemp(prefix='coax-unit-algebra-')));work.mkdir(parents=True,exist_ok=True)
    rows=cases();want=[expected(*r) for r in rows]
    assert len(rows)==114192,len(rows)
    assert set(r[0] for r in rows)=={0,1,2,3}
    assert set(r[0] for r in want)=={0,1,2,3}
    payload=struct.pack('<Q',len(rows))+b''.join(struct.pack('<13q',op,*a,*b) for op,a,b in rows)
    truth=b''.join(struct.pack('<7q',*row) for row in want)
    (work/'inputs.bin').write_bytes(payload);(work/'expected.bin').write_bytes(truth)
    record={'schema':'coax-unit-algebra-qualification-v1','seed':SEED,'profile':'exact signed int64 metadata, inputs <=2^20 scales, intermediates <=2^40; Python arbitrary-precision Fraction oracle',
            'scope':'separate producer; no compiler/type-interner integration or complete R12',
            'rows':len(rows),'operation_counts':dict(Counter(['normalize','product','quotient','ratio'][r[0]] for r in rows)),
            'expected_status_counts':dict(Counter(['ok','metadata','dimension','scale'][r[0]] for r in want)),
            'input_sha256':sha(work/'inputs.bin'),'expected_sha256':sha(work/'expected.bin'),
            'platform':platform.platform(),'machine':platform.machine(),'compiler_sha256':sha(COMPILER),
            'source_sha256':{str(p.relative_to(ROOT)):sha(p) for p in [SOURCE,NATIVE,Path(__file__).resolve(),ROOT/'coax/lib/compiler/quantity.coax']},'commands':[],'modes':[]}
    def command(name,args):
        start=time.monotonic();r=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,timeout=180)
        (work/(name+'.stdout')).write_bytes(r.stdout);(work/(name+'.stderr')).write_bytes(r.stderr)
        record['commands'].append({'name':name,'arguments':[str(x) for x in args],'elapsed_seconds':time.monotonic()-start,'exit':r.returncode})
        (work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
        assert r.returncode==0,(name,r.stderr.decode(errors='replace'),r.stdout.decode(errors='replace'))
        if name.startswith('run-'):assert not r.stderr,(name,r.stderr.decode(errors='replace'))
        return r
    record['compiler_version']=command('compiler-version',[COMPILER,'version']).stdout.decode().strip()
    cc=os.environ.get('CC','cc');record['c_compiler_version']=command('cc-version',[cc,'--version']).stdout.decode()
    emitted=command('generate-c',[COMPILER,'c',SOURCE,'--library','--header',work/'unit-algebra.h'])
    (work/'unit-algebra.c').write_bytes(emitted.stdout)
    record['generated_c_sha256']=sha(work/'unit-algebra.c');record['generated_header_sha256']=sha(work/'unit-algebra.h')
    for mode,flags in [('debug',['-O0','-g']),('release',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
        command('compile-'+mode,[cc,'-std=c11',*flags,'-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-variable','-Wno-unused-parameter','-Wno-misleading-indentation','-Wno-unknown-pragmas','-I'+str(work),work/'unit-algebra.c',NATIVE,'-lm','-o',work/mode])
        command('run-'+mode,[work/mode,work/'inputs.bin',work/(mode+'.bin')])
        actual=(work/(mode+'.bin')).read_bytes();assert len(actual)==len(truth),(mode,len(actual),len(truth))
        if actual!=truth:
            for i,(have,needed) in enumerate(zip(struct.iter_unpack('<7q',actual),want)):
                assert have==needed,(mode,i,rows[i],have,needed)
            raise AssertionError('transcript mismatch')
        record['modes'].append({'mode':mode,'passed':True,'exact_results':len(rows),'exact_words':len(rows)*7,'output_sha256':sha(work/(mode+'.bin'))})
    record['passed']=True;(work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps({'passed':True,'rows':len(rows),'exact_words_per_mode':len(rows)*7,'record':str(work/'receipt.json')}))

if __name__=='__main__':main()
