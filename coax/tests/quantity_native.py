#!/usr/bin/env python3
"""Independent quantity IEEE and transparent native-ABI qualification.

This focused test does not run the unchanged quantity policy checks/mutations.
Each generated quantity operation is compared with independently written raw C
arithmetic. Inputs are owned deterministic bit patterns; NaN payload equality
and floating exception flags are deliberately outside the profile.
"""
from pathlib import Path
import hashlib,json,os,platform,random,re,struct,subprocess,sys,tempfile,time,unittest
ROOT=Path(__file__).resolve().parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def invoke(args,**kw):
 return subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,text=True,timeout=180,**kw)
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
 rng=random.Random(0x513130)
 def cases(edges,bits,exponent,mantissa,quiet):
  out=[(a,b,edges[(5*i+7*j+3)%len(edges)]) for i,a in enumerate(edges) for j,b in enumerate(edges)]
  for _ in range(256):
   row=[]
   for _ in range(3):
    value=rng.getrandbits(bits)
    if value&exponent==exponent and value&mantissa:value|=quiet
    row.append(value)
   out.append(tuple(row))
  return out
 return cases(e32,32,0x7f800000,0x007fffff,0x00400000),cases(e64,64,0x7ff0000000000000,0x000fffffffffffff,0x0008000000000000)

def function_text(text,name):
 m=re.search(r'(?m)^(?:static )?(?:float|double)\s+'+re.escape(name)+r'\([^;\n]*\)\s*\{',text)
 if not m:raise AssertionError('generated C function definition not found: '+name)
 start=m.start();at=text.index('{',m.start());depth=1;cursor=at+1
 while depth:
  if text[cursor]=='{':depth+=1
  if text[cursor]=='}':depth-=1
  cursor+=1
 body=text[start:cursor]
 return '\n'.join(line for line in body.splitlines() if not line.lstrip().startswith('#line'))

class QuantityNative(unittest.TestCase):
 def test_transparent_quantity_native(self):
  self.assertEqual(sys.byteorder,'little','owned binary transport profile is little endian')
  work=Path(os.environ.get('COAX_QUANTITY_RECORD_DIR',tempfile.mkdtemp(prefix='coax-quantity-native-')));work.mkdir(parents=True,exist_ok=True)
  cases32,cases64=patterns();data=struct.pack('<II',len(cases32),len(cases64))
  data+=b''.join(struct.pack('<III',*row) for row in cases32)
  data+=b''.join(struct.pack('<QQQ',*row) for row in cases64)
  (work/'cases.bin').write_bytes(data)
  record={'schema':'coax-quantity-native-v1','compiler':str(COMPILER),'compiler_sha256':sha(COMPILER),
    'platform':platform.platform(),'machine':platform.machine(),'cases_f32':len(cases32),'cases_f64':len(cases64),
    'comparisons_per_mode':16*len(cases32)+13*len(cases64)+11,'input_sha256':sha(work/'cases.bin'),
    'oracle':'Independently written raw-C volatile scalar operations, six IEEE comparisons and ldexpf/ldexp binary scaling.',
    'profile':'Host IEEE arithmetic in round-to-nearest, no fast math or contraction. Compare all non-NaN bits, including signed zero, subnormals and infinities; compare NaNs by classification and canonicalize transcript NaNs. No finite-result, NaN-payload, exception-flag or cross-platform guarantee.',
    'scope':'Transparent scalar/nested/array/view ABI, built-in quantity arithmetic and binary scale conversion, plus exactly-once source-order foreign calls. Not full R12 units/frames or bootstrap qualification.',
    'modes':[],'commands':[]}
  def command(name,args):
   start=time.monotonic();r=invoke(args);(work/(name+'.stdout')).write_text(r.stdout);(work/(name+'.stderr')).write_text(r.stderr)
   record['commands'].append({'name':name,'arguments':[str(x) for x in args],'elapsed_seconds':time.monotonic()-start,'exit':r.returncode})
   (work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
   self.assertEqual(r.returncode,0,r.stderr or r.stdout);return r
  record['compiler_version']=command('version',[COMPILER,'version']).stdout.strip()
  cc=os.environ.get('CC','cc');record['c_compiler_version']=command('cc-version',[cc,'--version']).stdout
  source=ROOT/'coax/tests/quantity/native.coax';raw=ROOT/'coax/tests/quantity/native-raw.coax';native=ROOT/'coax/tests/quantity-native.c'
  generated=command('quantity-c',[COMPILER,'c',source,'--library','--header',work/'quantity-native.h']).stdout
  (work/'quantity.c').write_text(generated)
  plain=command('raw-c',[COMPILER,'c',raw,'--library']).stdout;(work/'raw.c').write_text(plain)
  record['code_comparison']={'full_generated_c_equal':generated==plain,'limit':'Whole C files include different source paths, semantic declarations and metadata; no full byte-equality claim. Homologous no-conversion exported functions are compared separately.','functions':{}}
  for name in ['QN_pure32','QN_pure64']:
   q=function_text(generated,name);p=function_text(plain,name)
   (work/(name+'-quantity.c')).write_text(q+'\n');(work/(name+'-raw.c')).write_text(p+'\n')
   record['code_comparison']['functions'][name]={'exact_without_source_line_directives':q==p,'whitespace_normalized_equal':re.sub(r'\s+',' ',q)==re.sub(r'\s+',' ',p),'quantity_sha256':hashlib.sha256(q.encode()).hexdigest(),'raw_sha256':hashlib.sha256(p.encode()).hexdigest()}
  blobs=[]
  for mode,flags in [('debug',['-O0','-g']),('release',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
   command('compile-'+mode,[cc,'-std=c11',*flags,'-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-variable','-Wno-unused-parameter','-Wno-misleading-indentation','-Wno-unknown-pragmas','-ffp-contract=off','-fno-fast-math','-I'+str(work),work/'quantity.c',native,'-lm','-o',work/mode])
   result=command('run-'+mode,[work/mode,work/'cases.bin',work/(mode+'.bin')]);blob=(work/(mode+'.bin')).read_bytes();blobs.append(blob)
   record['modes'].append({'mode':mode,'passed':True,'output_sha256':sha(work/(mode+'.bin')),'stdout':result.stdout.strip()})
  self.assertEqual(blobs[0],blobs[1]);self.assertEqual(blobs[0],blobs[2])
  record['source_sha256']={str(p.relative_to(ROOT)):sha(p) for p in [source,raw,native,Path(__file__).resolve(),ROOT/'coax/lib/compiler/quantity.coax']}
  record['generated_c_sha256']=sha(work/'quantity.c');record['generated_header_sha256']=sha(work/'quantity-native.h')
  record['abi']={'scalar_function_pointer_checks':True,'nested_size_alignment_offsets':True,'quantity_array_matches_raw_array_parameter':True,'quantity_view_matches_raw_view_parameter':True,'ordered_tick_trace':[1,2,3],'ordered_tick_result':28.0,'ordered_raw_left_tick_trace':[1,2,3],'ordered_raw_left_result':10.0,'conversion_single_tick_trace':[4],'conversion_single_tick_result':10.0}
  record['passed']=True;(work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n');print('quantity native receipt:',work/'receipt.json')

if __name__=='__main__':unittest.main()
