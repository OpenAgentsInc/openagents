#!/usr/bin/env python3
"""New derived-unit IEEE/ABI corpus. Never executes the old M1 native corpus."""
from pathlib import Path
import hashlib,json,os,platform,random,re,struct,subprocess,tempfile,time
ROOT=Path(__file__).resolve().parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
SOURCE=ROOT/'coax/tests/quantity/derived-native.coax'
RAW=ROOT/'coax/tests/quantity/derived-native-raw.coax'
NATIVE=ROOT/'coax/tests/quantity-derived-native.c'
SEED=0x51444d323031

def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def patterns():
    e32=[0,0x80000000,1,0x80000001,0x007fffff,0x807fffff,0x00800000,0x80800000,
         0x3f800000,0xbf800000,0x3f000000,0xbf000000,0x40000000,0xc0000000,
         0x3f800001,0x3f7fffff,0x7f7fffff,0xff7fffff,0x7f800000,0xff800000,
         0x7fc00001,0xffc12345,0x33800000,0xb3800000]
    e64=[0,0x8000000000000000,1,0x8000000000000001,0x000fffffffffffff,0x800fffffffffffff,
         0x0010000000000000,0x8010000000000000,0x3ff0000000000000,0xbff0000000000000,
         0x3fe0000000000000,0xbfe0000000000000,0x4000000000000000,0xc000000000000000,
         0x3ff0000000000001,0x3fefffffffffffff,0x7fefffffffffffff,0xffefffffffffffff,
         0x7ff0000000000000,0xfff0000000000000,0x7ff8000000000001,0xfff8123456789abc,
         0x3ca0000000000000,0xbca0000000000000]
    rng=random.Random(SEED)
    def make(edges,width,exponent,mantissa,quiet):
        rows=[(a,b) for a in edges for b in edges]
        for _ in range(512):
            pair=[]
            for _ in range(2):
                x=rng.getrandbits(width)
                if x&exponent==exponent and x&mantissa:x|=quiet
                pair.append(x)
            rows.append(tuple(pair))
        return rows
    return make(e32,32,0x7f800000,0x007fffff,0x00400000),make(e64,64,0x7ff0000000000000,0x000fffffffffffff,0x0008000000000000)

def function_text(text,name):
    m=re.search(r'(?m)^(?:static )?(?:float|double)\s+'+re.escape(name)+r'\([^;\n]*\)\s*\{',text)
    assert m,('generated C function not found',name)
    start=m.start();at=text.index('{',m.start());depth=1;cursor=at+1
    while depth:
        if text[cursor]=='{':depth+=1
        if text[cursor]=='}':depth-=1
        cursor+=1
    return '\n'.join(line for line in text[start:cursor].splitlines() if not line.lstrip().startswith('#line'))

def main():
    assert os.sys.byteorder=='little','owned binary case transport is little endian'
    work=Path(os.environ.get('COAX_DERIVED_QUANTITY_RECORD_DIR',tempfile.mkdtemp(prefix='coax-derived-quantity-')));work.mkdir(parents=True,exist_ok=True)
    rows32,rows64=patterns();assert len(rows32)==len(rows64)==1088
    data=struct.pack('<II',len(rows32),len(rows64))+b''.join(struct.pack('<II',*r) for r in rows32)+b''.join(struct.pack('<QQ',*r) for r in rows64)
    (work/'cases.bin').write_bytes(data)
    record={'schema':'coax-derived-quantity-native-v1','seed':SEED,'compiler_sha256':sha(COMPILER),'platform':platform.platform(),'machine':platform.machine(),
            'cases_f32':len(rows32),'cases_f64':len(rows64),'observations_per_mode':8*len(rows32)+8*len(rows64)+9,
            'oracle':'Independent volatile raw-C multiply/divide and staged (a/b)*.5 and (a/b)*b; not another quantity implementation.',
            'profile':'Round-to-nearest host IEEE binary32/binary64, no fast math/contraction, AArch64 FZ disabled; compare exact non-NaN bits (signed zeros, subnormals, infinities), NaNs by classification with canonicalized transcript. NaN payloads and floating exception flags are outside scope.',
            'scope':'Only NEW derived products/quotients/formal cancellation/explicit named and generated conversion targets, scalar/nested ABI and ordered one-time operand effects. No old M1 native rerun, device ABI, frames or complete R12 claim.',
            'input_sha256':sha(work/'cases.bin'),'source_sha256':{str(p.relative_to(ROOT)):sha(p) for p in [SOURCE,RAW,NATIVE,Path(__file__).resolve()]},'commands':[],'modes':[],'body_comparison':{}}
    def command(name,args):
        start=time.monotonic();p=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,timeout=180)
        (work/(name+'.stdout')).write_bytes(p.stdout);(work/(name+'.stderr')).write_bytes(p.stderr)
        record['commands'].append({'name':name,'arguments':[str(x) for x in args],'elapsed_seconds':time.monotonic()-start,'exit':p.returncode})
        (work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
        assert p.returncode==0,(name,p.stderr.decode(errors='replace'),p.stdout.decode(errors='replace'))
        if name.startswith('run-'):assert not p.stderr,(name,p.stderr.decode(errors='replace'))
        return p
    record['compiler_version']=command('compiler-version',[COMPILER,'version']).stdout.decode().strip()
    cc=os.environ.get('CC','cc');record['c_compiler_version']=command('cc-version',[cc,'--version']).stdout.decode()
    command('coax-checks',[COMPILER,'test',SOURCE])
    generated=command('quantity-c',[COMPILER,'c',SOURCE,'--library','--header',work/'derived-native.h']).stdout.decode();(work/'quantity.c').write_text(generated)
    raw=command('raw-c',[COMPILER,'c',RAW,'--library']).stdout.decode();(work/'raw.c').write_text(raw)
    for width in [32,64]:
        for stem in ['product','quotient','one','to_named','from_named','half_form','half_one','cancel']:
            name=f'QD_{stem}{width}';a=function_text(generated,name);b=function_text(raw,name)
            (work/(name+'-quantity.c')).write_text(a+'\n');(work/(name+'-raw.c')).write_text(b+'\n')
            equal=a==b
            record['body_comparison'][name]={'exact_without_source_line_directives':equal,'whitespace_normalized_equal':re.sub(r'\s+',' ',a)==re.sub(r'\s+',' ',b),'quantity_sha256':hashlib.sha256(a.encode()).hexdigest(),'raw_sha256':hashlib.sha256(b.encode()).hexdigest()}
            if stem in ['product','quotient','one','half_form','cancel']:assert equal,('no-conversion C body differs',name,a,b)
    outputs=[]
    for mode,flags in [('debug',['-O0','-g']),('release',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
        command('compile-'+mode,[cc,'-std=c11',*flags,'-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-variable','-Wno-unused-parameter','-Wno-misleading-indentation','-ffp-contract=off','-fno-fast-math','-I'+str(work),work/'quantity.c',NATIVE,'-lm','-o',work/mode])
        result=command('run-'+mode,[work/mode,work/'cases.bin',work/(mode+'.bin')]);blob=(work/(mode+'.bin')).read_bytes()
        expected_bytes=8*len(rows32)*4+8*len(rows64)*8+40
        assert len(blob)==expected_bytes,(mode,len(blob),expected_bytes)
        outputs.append(blob);record['modes'].append({'mode':mode,'passed':True,'observations':record['observations_per_mode'],'output_bytes':len(blob),'output_sha256':sha(work/(mode+'.bin')),'stderr_bytes':0,'stdout':result.stdout.decode().strip()})
    assert outputs[0]==outputs[1]==outputs[2]
    record['abi']={'scalar_function_pointers':True,'nested_size_alignment_offsets':True,'nested_roundtrip':True,'ordered_product':[1,2],'ordered_quotient':[3,4],'ordered_cancel':[5,6,7],'ordered_explicit_half':[8,9],'generated_target_conversion_once':[10],'ordered_reverse_product':[11,12]}
    record['generated_c_sha256']=sha(work/'quantity.c');record['generated_header_sha256']=sha(work/'derived-native.h');record['raw_generated_c_sha256']=sha(work/'raw.c')
    record['passed']=True;(work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'passed':True,'observations_per_mode':record['observations_per_mode'],'record':str(work/'receipt.json')}))
if __name__=='__main__':main()
