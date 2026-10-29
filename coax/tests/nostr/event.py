#!/usr/bin/env python3
"""Explicit independent event tests. No runtime Python/signing dependency.

Test signatures use only deterministic throwaway keys via the pinned public
libsecp oracle artifact. Production code under test is the actual Coax module.
"""
import copy,ctypes as C,hashlib,json,os,platform,random,re,tempfile,time,unittest
from pathlib import Path
import secp_field as base
from oracle import NativeOracle,DEFAULT_LIBRARY,schnorr_verify
from make_event_fixtures import canonical,fixture,encoded,generate
ROOT=base.ROOT;SOURCE=ROOT/'coax/lib/nostr/event.coax'
OK,JSON,SHAPE,LIMIT,HEX,OUTPUT,ALIAS,ID,SIGNATURE=range(9)
class Event(C.Structure):
    _fields_=[('error',C.c_int64),('admission',C.c_int64),('original',base.View),('id',C.c_uint8*32),('public_key',C.c_uint8*32),('signature',C.c_uint8*64),('computed_id',C.c_uint8*32)]+[(n,C.c_int64) for n in ['created_at','kind','tags_from','tags_to','tags','content_from','content_to','written']]+[('verified',C.c_bool)]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()

class Events(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='coax-nostr-event-');cls.dir=Path(cls.temp.name);cls.oracle=NativeOracle(DEFAULT_LIBRARY);cls.libs=[];cls.cases=[];cls.start=time.monotonic()
        cls.report={'schema':'coax.nostr-event.v1','scope':'bounded public NIP-01 read/hash/verify correctness; no author/scope/freshness admission, signing, production-key or physical-iOS claim','compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':sha(base.COMPILER),'native_compiler':base.run('cc','--version').stdout.splitlines()[0],'native_target':base.run('cc','-dumpmachine').stdout.strip(),'sources':{},'native':[]}
        deps=json.loads(base.run(base.COMPILER,'deps',SOURCE).stdout)
        cls.report['dependency_graph']=json.loads(json.dumps(deps).replace(str(ROOT)+'/', ''))
        for name in deps['sources']:
            p=Path(name);cls.report['sources'][str(p.relative_to(ROOT))]=sha(p)
        for p in [SOURCE,*Path(__file__).parent.glob('event*'),Path(__file__).parent/'make_event_fixtures.py']:
            if p.is_file():cls.report['sources'][str(p.relative_to(ROOT))]=sha(p)
        variants=[('checked',SOURCE,[]),('release',SOURCE,['--release'])]
        previous=os.environ.get('COAX_EVENT_PARITY_SOURCE')
        if previous:
            old=Path(previous).resolve();before=json.loads((ROOT/'coax/tests/nostr/data/event-initial-mutation-2026-10-28.json').read_text())
            cls.assert_snapshot=before['focused']['sources']['coax/lib/nostr/event.coax']
            if sha(old)!=cls.assert_snapshot:raise AssertionError('before-refactor source does not match retained initial snapshot')
            variants.extend([('before_checked',old,[]),('before_release',old,['--release'])])
            old_deps=json.loads(base.run(base.COMPILER,'deps',old).stdout);old_root=old.parents[3]
            cls.report['before_after_parity']={'before_snapshot':'f5581bacd14ca501ab667e6bb25322e7c31bf837','before_source_sha256':sha(old),'before_sources':{str(Path(p).relative_to(old_root)):sha(p) for p in old_deps['sources']},'contract':'equal errors/admission/verified/written; successful parsed fields, original spans, canonical bytes and computed IDs; refusal partial metadata is not a contract','cases':0}
        for mode,entry,args in variants:
            source=cls.dir/(mode+'.c');source.write_text(base.run(base.COMPILER,'c',entry,'--library','--header',cls.dir/'event.h',*args).stdout)
            target=cls.dir/(mode+('.dylib' if platform.system()=='Darwin' else '.so'));base.run('cc','-std=c11','-O2','-shared','-fPIC',source,'-lm','-o',target)
            lib=C.CDLL(str(target))
            for name in ['nostr_event_read','nostr_event_verify_vartime']:
                f=getattr(lib,name);f.argtypes=[base.View,base.View,C.c_bool];f.restype=Event
            cls.libs.append((mode,lib));cls.report['native'].append({'mode':mode,'generated_c_sha256':sha(source),'artifact_sha256':sha(target)})
    @classmethod
    def tearDownClass(cls):
        cls.oracle.close();cls.report['elapsed_seconds']=time.monotonic()-cls.start;cls.report['temporary_bytes']=sum(p.stat().st_size for p in cls.dir.rglob('*') if p.is_file())
        if cls.report['temporary_bytes']>33554432:raise AssertionError('32 MiB temporary-artifact budget exceeded')
        for name,want in cls.report['sources'].items():
            if sha(ROOT/name)!=want:raise AssertionError('changed test/runtime input:'+name)
        if os.environ.get('COAX_EVENT_REPORT'):Path(os.environ['COAX_EVENT_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
        cls.temp.cleanup()
    @classmethod
    def signed(cls,e,secret=3):
        e=copy.deepcopy(e);key=secret.to_bytes(32,'big');public=cls.oracle.point(key)
        if public is None:raise AssertionError('test key')
        e['pubkey']=public[1:].hex();digest=hashlib.sha256(canonical(e)).digest();e['id']=digest.hex();sig=cls.oracle.sign(digest,key,bytes(32))
        if sig is None:raise AssertionError('test signature')
        e['sig']=sig.hex();return e
    def call(self,text,error=OK,*,verify=False,expected=None,capacity=262144,overlap=False,counts=None,wasm=True):
        if isinstance(text,str):text=text.encode()
        reference=None
        for mode,lib in self.libs:
            a=base.array(b'\xa5'*8+text+b'\xa5'*8);storage=base.array(b'\xa5'*(262144+16));iv=base.View(C.cast(C.byref(a,8),C.POINTER(C.c_uint8)),len(text));ov=base.View(C.cast(C.byref(storage,8),C.POINTER(C.c_uint8)),capacity)
            if counts is not None:iv.count=counts
            e=getattr(lib,'nostr_event_verify_vartime' if verify else 'nostr_event_read')(iv,ov,overlap)
            self.assertEqual(e.error,error,(mode,text[:100]));self.assertEqual(bool(e.verified),verify and error==OK);self.assertEqual(bytes(a),b'\xa5'*8+text+b'\xa5'*8)
            limit=capacity if 0<=capacity<=262144 else 262144
            self.assertEqual(bytes(storage[:8]),b'\xa5'*8);self.assertEqual(bytes(storage[8+limit:]),b'\xa5'*(262152-limit))
            if error!=OK:self.assertEqual(e.written,0)
            observable=(e.error,e.admission,e.written,bool(e.verified))
            if error==OK:
                observable+=(bytes(e.id),bytes(e.public_key),bytes(e.signature),bytes(e.computed_id),e.created_at,e.kind,e.tags_from,e.tags_to,e.tags,e.content_from,e.content_to,bytes(storage[8:8+e.written]),C.string_at(e.original.data,e.original.count))
            if reference is None:reference=observable
            else:self.assertEqual(observable,reference,('before/after public parity',mode))
            if expected is not None:
                pre=canonical(expected);self.assertEqual(bytes(storage[8:8+e.written]),pre);self.assertEqual(e.written,len(pre));self.assertEqual(bytes(e.computed_id),hashlib.sha256(pre).digest());self.assertEqual(e.tags,len(expected['tags']));self.assertEqual(e.kind,expected['kind']);self.assertEqual(e.created_at,expected['created_at']);self.assertEqual(C.string_at(e.original.data,e.original.count),text)
                raw=text[e.content_from:e.content_to];self.assertEqual(json.loads(b'"'+raw+b'"'),expected['content'])
        if wasm:
            row={'name':str(len(self.cases)),'text':text.hex(),'error':error,'verify':verify,'verified':verify and error==OK,'capacity':capacity,'overlap':overlap}
            if counts is not None:row['input_count']=counts
            if expected is not None:row.update(preimage=canonical(expected).hex(),id=expected['id'])
            self.cases.append(row)
        if 'before_after_parity' in self.report:self.report['before_after_parity']['cases']+=1
        return e
    def test_canonical_oracles_and_signed_public_events(self):
        rng=random.Random(2026102901);events=[]
        contents=['','"\\/\b\f\n\r\t\x00\x1f','é€😀𝄞\u2028\u2029',''.join(map(chr,range(32))),'a'*1024]
        for i in range(96):
            e=fixture();e.update(created_at=[0,1,1761750000,2**53-1][i%4],kind=[0,1,65535][i%3],content=rng.choice(contents),tags=[[],['x','repeat'],['x',str(i),'é😀','']]);e=self.signed(e,i+1);events.append(e)
        path=self.dir/'reference.json';path.write_text(json.dumps(events,ensure_ascii=False))
        js=json.loads(base.run('node',ROOT/'coax/tests/nostr/event_reference.cjs',path).stdout)
        for i,(e,ref) in enumerate(zip(events,js)):
            self.assertEqual(ref['preimage'],canonical(e).hex());self.assertEqual(ref['id'],e['id'])
            if i<8:self.assertTrue(schnorr_verify(bytes.fromhex(e['id']),bytes.fromhex(e['pubkey']),bytes.fromhex(e['sig'])))
            # Wire order, whitespace, raw UTF-8, slash escapes and escaped keys
            # may change; the original is retained and the ID remains exact.
            text=json.dumps(dict(reversed(list(e.items()))),ensure_ascii=bool(i%2),indent=1 if i%3==0 else None)
            if i%4==0:text=text.replace('"pubkey"','"\\u0070ubkey"').replace('/','\\/')
            self.call(text,verify=True,expected=e)
        self.report['independent_preimages']=len(events);self.report['independent_bigint_signature_checks']=8
    def test_tampering_shapes_duplicates_and_unknown_preservation(self):
        e=self.signed(fixture());self.call(encoded(e),expected=e)
        for field in ['id','sig','pubkey','content','created_at','kind','tags']:
            v=copy.deepcopy(e)
            if field in ['id','sig','pubkey']:v[field]=('0' if v[field][0]!='0' else '1')+v[field][1:]
            elif field=='tags':v[field].append(['changed'])
            elif field=='content':v[field]+='changed'
            else:v[field]+=1
            self.call(encoded(v),SIGNATURE if field=='sig' else ID,verify=True)
        for field in ['pubkey','sig']:
            v=copy.deepcopy(e);v[field]='ff'*(32 if field=='pubkey' else 64);v['id']=hashlib.sha256(canonical(v)).hexdigest();self.call(encoded(v),SIGNATURE,verify=True)
        for field in ['id','pubkey','created_at','kind','tags','content','sig']:
            v=copy.deepcopy(e);del v[field];self.call(encoded(v),SHAPE,verify=True)
        for field in ['id','pubkey','sig']:
            for value in [e[field].upper(),e[field][:-1],'g'+e[field][1:]]:
                v=copy.deepcopy(e);v[field]=value;self.call(encoded(v),HEX,verify=True)
        for suffix in [',"x":1,"\\u0078":2}',',"unknown":{"a":1,"a":2}}',',"unknown":"\\ud800"}',',"unknown":NaN}']:
            self.call(encoded(e)[:-1]+suffix,JSON,verify=True)
        for field,value,error in [('created_at',-1,SHAPE),('created_at',2**53,JSON),('created_at',1.0,JSON),('kind',65536,SHAPE),('kind','1',JSON),('content',None,SHAPE),('tags',{},SHAPE),('tags',[[False]],SHAPE)]:
            v=copy.deepcopy(e);v[field]=value;self.call(encoded(v),error,verify=True)
        v=copy.deepcopy(e);v['unknown']={'raw':[1e20,False,None,'keep']};self.call(json.dumps(v,ensure_ascii=False,indent=1),verify=True,expected=v)
        self.call('[]',SHAPE);self.call('{',JSON);self.call(encoded(e)+' false',JSON)
    def test_limits_guards_and_view_refusals(self):
        e=self.signed(fixture());text=encoded(e);n=len(canonical(e))
        self.call(text,expected=e,capacity=n);self.call(text,OUTPUT,capacity=n-1);self.call(text,OUTPUT,capacity=0)
        self.call(text,ALIAS,overlap=True);self.call(text,OUTPUT,capacity=-1);self.call(text,OUTPUT,capacity=262145)
        self.call(text,LIMIT,counts=-1);self.call(text,LIMIT,counts=262145)
        for count in [2000,2001]:
            v=copy.deepcopy(e);v['tags']=[[]]*count;v=self.signed(v);self.call(encoded(v),OK if count==2000 else SHAPE,expected=v if count==2000 else None)
        for count in [16,17]:
            v=copy.deepcopy(e);v['tags']=[['x']*count];v=self.signed(v);self.call(encoded(v),OK if count==16 else SHAPE,expected=v if count==16 else None)
        for count in [65536,65537]:
            v=copy.deepcopy(e);v['content']='a'*count;v=self.signed(v);self.call(encoded(v),OK if count==65536 else JSON,expected=v if count==65536 else None)
        for count in [10922,10923]:
            v=copy.deepcopy(e);v['content']='a'*count;v=self.signed(v);text=encoded(v).replace('"content":"'+'a'*count+'"','"content":"'+'\\u0061'*count+'"');self.call(text,OK if count==10922 else JSON,expected=v if count==10922 else None)
        text=encoded(e);self.call(text+' '*(262144-len(text)),expected=e);self.call(text+' '*(262145-len(text)),LIMIT)
        self.report['profile']={'input':262144,'output':262144,'raw_string':65536,'decoded_content':65536,'tags':2000,'values_per_tag':16,'safe_integer':2**53-1}
    def test_fixture_reproducibility_and_no_signing_dependency(self):
        self.assertEqual((ROOT/'coax/tests/nostr/event_fixtures.coax').read_text(),generate())
        graph=json.dumps(self.report['dependency_graph']).lower()
        for forbidden in ['secp_mult_base','schnorr_sign','nostr/keys','nip42','key_custody']:self.assertNotIn(forbidden,graph)
    def test_seeded_emitted_faults_are_detected(self):
        source=(self.dir/'checked.c').read_text();valid=self.signed(fixture());wrong=copy.deepcopy(valid);wrong['id']='0'*64;unsigned=copy.deepcopy(valid);unsigned['sig']='ff'*64
        faults=[('missing-id','coax_p_ne_same_id','return 1;',wrong,False,ID),('signature-bypass','coax_p_ne_verified','l_event.verified=1;l_event.error=0;return l_event;',unsigned,True,SIGNATURE),('lost-content-field','coax_p_ne_field','return 0;',valid,True,OK),('lost-tags','coax_p_ne_write_tags','return coax_p_json_raw(l_out,l_at,(coax_string){"[]",2});',valid,True,OK)]
        reports=[]
        for label,name,body,event,verify,want in faults:
            match=re.search(r'\b'+name+r'\([^;{}]*\)\s*\{',source);self.assertIsNotNone(match)
            start=match.end();end=source.index('\n}',start);mutant=source[:start]+'\n'+body+source[end:]
            path=self.dir/(label+'.c');path.write_text(mutant);target=self.dir/(label+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',path,'-lm','-o',target)
            lib=C.CDLL(str(target));fn=getattr(lib,'nostr_event_verify_vartime' if verify else 'nostr_event_read');fn.argtypes=[base.View,base.View,C.c_bool];fn.restype=Event
            raw=base.array(encoded(event).encode());out=(C.c_uint8*262144)();result=fn(base.view(raw),base.view(out),False)
            self.assertNotEqual(result.error,want,label);reports.append({'fault':label,'detected':True,'observed_error':result.error,'correct_error':want,'mutated_c_sha256':sha(path),'artifact_sha256':sha(target)})
        self.report['seeded_emitted_faults']=reports
    def test_z_real_wasm_and_sanitized_callers(self):
        fixtures=self.dir/'wasm.json';fixtures.write_text(json.dumps(self.cases));reports=[]
        for mode,_ in self.libs:
            target=self.dir/(mode+'.wasm');base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(mode+'.c'),ROOT/'coax/tests/nostr/event_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',target)
            reports.append({'mode':mode,'artifact_sha256':sha(target),**json.loads(base.run('node',ROOT/'coax/tests/nostr/event_wasm.cjs',target,fixtures).stdout)})
        self.report['wasm']=reports;self.report['cases_per_native_profile']=len(self.cases);self.report['wasm_stack_bytes']=1048576
        e=self.signed(fixture());text=encoded(e).encode();probe=self.dir/'sanitized.c'
        probe.write_text('#include "checked.c"\n#include <assert.h>\n#include <string.h>\nint main(void){uint8_t out[262146];memset(out,165,sizeof out);char text[]='+json.dumps(text.decode())+';coax_string s={text,sizeof(text)-1};coax_t_NostrEvent e=nostr_event_verify_vartime(s,(coax_v_u8){out+1,262144},0);assert(e.error==0&&e.verified);assert(out[0]==165&&out[262145]==165);e=nostr_event_read(s,(coax_v_u8){out+1,1},0);assert(e.error==5&&!e.verified&&e.written==0);return 0;}\n')
        target=self.dir/'sanitized';base.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',probe,'-lm','-o',target);base.run(target);self.report['asan_ubsan']=True
if __name__=='__main__':unittest.main()
