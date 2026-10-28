#!/usr/bin/env python3
"""Explicit RFC/Python/WebCrypto and worker checks, never a gate-format cutover."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import subprocess
import tempfile
import time
import unittest

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
PLATFORM=ROOT/'coax/lib/crypto/platform'
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
def run(*args,**kwargs):
    result=subprocess.run([str(x) for x in args],capture_output=True,text=True,**kwargs)
    if result.returncode:raise RuntimeError(f'{args[0]} failed ({result.returncode}): {result.stderr or result.stdout}')
    return result
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Progress(C.Structure):_fields_=[('error',C.c_uint32),('phase',C.c_uint32),('completed',C.c_int64),('total',C.c_int64)]
def array(data):return(C.c_uint8*len(data))(*data)
def view(data,at=0,count=None):return View(C.cast(C.byref(data,at),C.POINTER(C.c_uint8)),len(data)-at if count is None else count)
def vectors():
    text=(ROOT/'coax/tests/vectors/upstream/rfc/rfc7914.txt').read_text()
    start=text.index('\n11.  Test Vectors for PBKDF2 with HMAC-SHA-256\n')
    pattern=r'PBKDF2-HMAC-SHA-256 \(P="([^"]*)", S="([^"]*)",\s+c=(\d+), dkLen=(\d+)\) =\s*((?:[0-9a-f]{2}\s+){63}[0-9a-f]{2})'
    rows=re.findall(pattern,text[start:]);assert len(rows)==2
    return[(p.encode(),s.encode(),int(c),int(n),bytes.fromhex(want)) for p,s,c,n,want in rows]

class Pbkdf2(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='coax-pbkdf2-');cls.directory=Path(cls.temp.name);cls.libs=[];cls.browser=[]
        for mode,flags in [('checked',[]),('release',['--release'])]:
            c=cls.directory/(mode+'.c');c.write_text(run(COMPILER,'c',ROOT/'coax/lib/crypto/pbkdf2-worker.coax','--library','--header',cls.directory/'zeroize-policy.h',*flags).stdout)
            artifact=cls.directory/(mode+'.dylib')
            run('cc','-std=c11','-O2','-DCOAX_PBKDF2_TEST_OBSERVE','-I'+str(cls.directory),'-I'+str(PLATFORM),'-shared','-fPIC',c,PLATFORM/'zeroize.c',PLATFORM/'pbkdf2-worker.c','-lm','-o',artifact)
            lib=C.CDLL(str(artifact))
            for name,args,result in [('pbkdf2_init',[View,View,View,C.c_int64,C.c_int64],C.c_uint32),('pbkdf2_gate_init',[View,View,View],C.c_uint32),('pbkdf2_info',[View],Progress),('pbkdf2_step',[View,C.c_int64],Progress),('pbkdf2_result',[View,View],C.c_uint32),('pbkdf2_cancel',[View],C.c_uint32),('coax_secure_zero',[C.c_void_p,C.c_int64,C.c_int64,C.c_int64],C.c_uint32)]:
                fn=getattr(lib,name);fn.argtypes=args;fn.restype=result
            for name in ['pb_worker_password','pb_worker_salt','pb_worker_output']:getattr(lib,name).restype=C.c_size_t
            lib.pb_worker_preflight.argtypes=[C.c_int32]*6;lib.pb_worker_preflight.restype=C.c_uint32
            lib.pb_worker_begin.argtypes=[C.c_int32]*5;lib.pb_worker_begin.restype=C.c_uint32
            lib.pb_worker_advance.argtypes=[C.c_int32];lib.pb_worker_advance.restype=C.c_uint32
            cls.libs.append(lib)
    @classmethod
    def tearDownClass(cls):
        paths=[ROOT/'coax/lib/crypto/pbkdf2.coax',ROOT/'coax/lib/crypto/pbkdf2-memory.coax',ROOT/'coax/lib/crypto/pbkdf2-worker.coax',ROOT/'coax/tests/nostr/pbkdf2_rfc.coax',PLATFORM/'pbkdf2-worker.c',PLATFORM/'pbkdf2-worker.js',PLATFORM/'zeroize.c',PLATFORM/'zeroize.h',ROOT/'coax/lib/crypto/zeroize.coax',ROOT/'coax/lib/crypto/hmac.coax',ROOT/'coax/lib/crypto/sha256.coax',ROOT/'coax/lib/crypto/bytes.coax',ROOT/'coax/lib/proto/fingerprint.coax',ROOT/'coax/web/unlock/gate.ts',ROOT/'coax/tests/vectors/upstream/rfc/rfc7914.txt',Path(__file__),HERE/'browser.html',HERE/'browser.ts',HERE/'rejection-worker.js']
        report={'schema':'coax.pbkdf2-conformance.v1','compiler':run(COMPILER,'--version').stdout.strip(),'compiler_sha256':sha(COMPILER),'host':platform.platform(),'native_compiler':run('cc','--version').stdout.splitlines()[0],'wasm_compiler':run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],'flags':['-std=c11','-O2','checked and --release','COAX_PBKDF2_TEST_OBSERVE (fixture only)'],'legacy_profile':{'KDF':'PBKDF2-HMAC-SHA256','rounds':210000,'salt_bytes':16,'key_bytes':32,'phrase':'already normalized lowercase ASCII alphanumeric','phrase_bytes':[4,40],'envelope':'existing AES-GCM 256 / 12-byte IV unchanged'},'source_sha256':{str(p.relative_to(ROOT)):sha(p) for p in paths},'native_artifacts':{mode:sha(cls.directory/(mode+'.dylib')) for mode in ['checked','release']},'browser':cls.browser,'scope':'primitive and dedicated-worker fixture; legacy gate/envelope unchanged; identified caller-buffer cleanup only'}
        Path(os.environ.get('COAX_PBKDF2_REPORT','/tmp/coax-pbkdf2-conformance.json')).write_text(json.dumps(report,indent=2)+'\n');cls.temp.cleanup()
    def derive(self,lib,password,salt,rounds,length,budget=256):
        p=array(password);s=array(salt);state=array([165]*832);out=array([165]*(length+8))
        self.assertEqual(lib.pbkdf2_init(view(state),view(p),view(s),rounds,length),0)
        previous=0
        while True:
            progress=lib.pbkdf2_step(view(state),budget);self.assertEqual(progress.error,0)
            self.assertGreater(progress.completed,previous);self.assertLessEqual(progress.completed-previous,budget);previous=progress.completed
            if progress.phase==2:break
        self.assertEqual(progress.completed,rounds*((length+31)//32))
        self.assertEqual(lib.pbkdf2_result(view(state),view(out)),0);self.assertEqual(bytes(out)[length:],bytes([165])*8)
        result=bytes(out)[:length]
        self.assertEqual(lib.coax_secure_zero(state,len(state),0,len(state)),0);self.assertEqual(bytes(state),bytes(832))
        return result
    def test_actual_rfc7914_sha256_and_independent_random_boundaries(self):
        for password,salt,rounds,length,want in vectors():
            self.assertEqual(hashlib.pbkdf2_hmac('sha256',password,salt,rounds,length),want)
            for lib in self.libs:self.assertEqual(self.derive(lib,password,salt,rounds,length),want)
        rng=random.Random(7914);sizes=[0,1,55,60,63,64,65,1024];lengths=[1,31,32,33,64,255,256]
        for index in range(40):
            password=rng.randbytes(sizes[index%8]);salt=rng.randbytes(sizes[(index//5)%8]);rounds=[1,2,7][index%3];length=lengths[index%7]
            want=hashlib.pbkdf2_hmac('sha256',password,salt,rounds,length)
            for lib in self.libs:self.assertEqual(self.derive(lib,password,salt,rounds,length,budget=[1,3,1024][index%3]),want)
    def test_aliases_invalid_metadata_cancellation_and_explicit_disposal(self):
        for lib in self.libs:
            state=array([165]*833);password=array(b'password');salt=array(b'salt');out=array([165]*41)
            for operation in [lambda:lib.pbkdf2_init(view(state),view(password),view(salt),2,33),lambda:lib.pbkdf2_init(view(state,count=832),view(password,count=1025),view(salt),2,33),lambda:lib.pbkdf2_init(view(state,count=832),view(state,831,1),view(salt),2,33),lambda:lib.pbkdf2_init(view(state,count=832),view(password),view(salt),2**63-1,256)]:
                self.assertNotEqual(operation(),0);self.assertEqual(bytes(state),bytes([165])*833)
            self.assertEqual(lib.pbkdf2_init(view(state,count=832),view(password),view(salt),2,33),0)
            before=bytes(state)
            for budget in [0,-1,1025,2**63-1]:self.assertEqual(lib.pbkdf2_step(view(state,count=832),budget).error,7);self.assertEqual(bytes(state),before)
            self.assertEqual(lib.pbkdf2_result(view(state,count=832),view(out)),4);self.assertEqual(bytes(out),bytes([165])*41)
            self.assertEqual(lib.pbkdf2_step(view(state,count=832),1).completed,1)
            self.assertEqual(lib.pbkdf2_cancel(view(state,count=832)),0);self.assertEqual(lib.pbkdf2_step(view(state,count=832),1).error,8)
            self.assertEqual(lib.pbkdf2_result(view(state,count=832),view(out)),8);self.assertEqual(bytes(out),bytes([165])*41)
            self.assertEqual(lib.coax_secure_zero(state,832,0,832),0);self.assertEqual(bytes(state),bytes(832)+bytes([165]))
            self.assertEqual(lib.pbkdf2_init(view(state,count=832),view(password),view(salt),1,33),0)
            self.assertEqual(lib.pbkdf2_step(view(state,count=832),1024).phase,2);before=bytes(state)
            self.assertEqual(lib.pbkdf2_result(view(state,count=832),view(state,576,33)),5);self.assertEqual(bytes(state),before)
            self.assertEqual(lib.pbkdf2_result(view(state,count=832),view(out,count=32)),3)
            self.assertEqual(lib.pbkdf2_result(view(state,count=832),view(out)),0)
    def test_worker_cleanup_after_bad_input_and_cancellation(self):
        for lib in self.libs:
            self.assertEqual(lib.pb_worker_dispose(),0)
            self.assertEqual(lib.pb_worker_preflight(1025,16,1,32,256,0),2)
            self.assertEqual(lib.pb_worker_preflight(4,16,210000,64,256,1),2)
            C.memmove(lib.pb_worker_password(),b'ABCD',4);C.memmove(lib.pb_worker_salt(),bytes(range(16)),16)
            self.assertEqual(lib.pb_worker_begin(4,16,210000,32,1),2);self.assertEqual(lib.pb_worker_clean(),1);self.assertNotEqual(lib.pb_worker_finish(),0)
            C.memmove(lib.pb_worker_password(),b'openagents',10);C.memmove(lib.pb_worker_salt(),bytes(range(16)),16)
            self.assertEqual(lib.pb_worker_begin(10,16,210000,32,1),0)
            self.assertEqual(C.string_at(lib.pb_worker_password(),1024),bytes(1024));self.assertEqual(C.string_at(lib.pb_worker_salt(),1024),bytes(1024))
            self.assertEqual(lib.pb_worker_advance(256),0);self.assertEqual(lib.pb_worker_completed(),256)
            self.assertEqual(lib.pb_worker_cancel(),0);self.assertEqual(lib.pb_worker_clean(),1);self.assertNotEqual(lib.pb_worker_finish(),0)
    def test_fixture_pins_and_legacy_parameter_source(self):
        base=ROOT/'coax/tests/vectors';manifest=json.loads((base/'manifest.json').read_text());row=next(x for x in manifest['files'] if x['path']=='upstream/rfc/rfc7914.txt')
        self.assertEqual(sha(base/row['path']),row['sha256'])
        gate=(ROOT/'coax/web/unlock/gate.ts').read_text()
        for expected in ['export const ROUNDS = 210_000','const MIN = 4, MAX = 40','new Uint8Array(16)','new Uint8Array(12)','hash: "SHA-256"','{ name: "AES-GCM", length: 256 }','phrase.toLowerCase().replace(/[^a-z0-9]/g, "")']:
            self.assertIn(expected,gate)
    def test_native_ASan_UBSan_state_output_and_cancel(self):
        host=self.directory/'sanitized.c';host.write_text('''#include "zeroize.h"
#include <assert.h>
int main(void){uint8_t state[833],p[65],salt[1024],out[264];for(int i=0;i<833;i++)state[i]=165;for(int i=0;i<65;i++)p[i]=i;for(int i=0;i<1024;i++)salt[i]=i;for(int i=0;i<264;i++)out[i]=165;
coax_v_u8 s={state,832},password={p,65},n={salt,1024},o={out,264};
assert(pbkdf2_init(s,password,n,7,256)==0);coax_t_PbProgress progress;do{progress=pbkdf2_step(s,3);assert(!progress.error);}while(progress.phase==1);
assert(progress.completed==56);assert(pbkdf2_result(s,o)==0);for(int i=256;i<264;i++)assert(out[i]==165);assert(state[832]==165);
assert(pbkdf2_init(s,password,n,210000,32)==0);assert(pbkdf2_step(s,1).completed==1);assert(pbkdf2_cancel(s)==0);assert(pbkdf2_result(s,o)==8);
assert(coax_secure_zero(state,832,0,832)==0);for(int i=0;i<832;i++)assert(!state[i]);assert(state[832]==165);
assert(pbkdf2_init((coax_v_u8){state,-1},password,n,1,32)!=0);assert(pbkdf2_init(s,(coax_v_u8){state+1,1},n,1,32)==5);return 0;}''')
        output=self.directory/'sanitized'
        run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(self.directory),'-I'+str(PLATFORM),self.directory/'checked.c',PLATFORM/'zeroize.c',host,'-lm','-o',output);run(output)
    def test_real_wasm_worker_webcrypto_and_main_thread_frames(self):
        clang=Path('/opt/homebrew/opt/llvm/bin/clang');sysroot=Path('/opt/homebrew/opt/wasi-libc/share/wasi-sysroot');resources=Path('/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes')
        for mode in ['checked','release']:
            module=self.directory/(mode+'.wasm')
            run(clang,'--target=wasm32-wasip1','--sysroot='+str(sysroot),'-resource-dir='+str(resources),'-std=c11','-O2','-DCOAX_PBKDF2_TEST_OBSERVE','-I'+str(self.directory),'-I'+str(PLATFORM),'-mexec-model=reactor',self.directory/(mode+'.c'),PLATFORM/'zeroize.c',PLATFORM/'pbkdf2-worker.c','-Wl,--export-memory,--export=__heap_base,-z,stack-size=1048576,--max-memory=67108864,--gc-sections','-lm','-o',module)
            report=self.directory/(mode+'.json');run('bun',HERE/'browser.ts',module,report)
            result=json.loads(report.read_text());self.assertTrue(result['abiOverflowRefused'])
            self.assertEqual(len(result['rejectionCleanup']['cases']),8);self.assertTrue(result['rejectionCleanup']['receivedInputsZeroBeforeResponse']);self.assertTrue(result['rejectionCleanup']['noKeyReleased'])
            result['mode']=mode;result['artifact_sha256']=sha(module);self.browser.append(result)

if __name__=='__main__':unittest.main()
