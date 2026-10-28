#!/usr/bin/env python3
"""Optional independent JSON admission oracle + native ASan/UBSan driver."""
import json, os, random, re, subprocess, tempfile, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/nostr/adapters/json-admission.coax'
class Num(str): pass
class Obj(list): pass

def expected(text,domain):
    def constant(_): raise ValueError('non-JSON constant')
    try:
        if len(text.encode('utf-8'))>[262144,65536,524288][domain]: return False
        value=json.loads(text,object_pairs_hook=Obj,parse_int=Num,parse_float=Num,parse_constant=constant)
        def visit(v,depth=0):
            if isinstance(v,Num): return
            if isinstance(v,str):
                encoded=v.encode('utf-8') # Lone escaped surrogates cannot become signed text.
                if len(encoded)>65536: raise ValueError('decoded string')
                return
            if isinstance(v,(Obj,list)):
                if depth>=16: raise ValueError('depth')
                if isinstance(v,Obj):
                    seen=set()
                    for k,x in v:
                        k.encode('utf-8')
                        if k in seen: raise ValueError('duplicate')
                        seen.add(k)
                        number=k in ({'exp','iat','nbf'} if domain==1 else {'kind','created_at'})
                        if number and (not isinstance(x,Num) or not re.fullmatch(r'-?(0|[1-9][0-9]*)',x) or abs(int(x))>2**53-1):
                            raise ValueError('integer')
                        visit(x,depth+1)
                else:
                    for x in v: visit(x,depth+1)
        visit(value)
        return True
    except (ValueError,UnicodeError,RecursionError): return False

DRIVER=r'''
#include "admit.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static char hex[2100001]; static unsigned char text[1048576];
int main(void) {
  int domain; long long frame,proto; size_t n;
  while(scanf("%d %lld %lld %zu %2100000s",&domain,&frame,&proto,&n,hex)==5) {
    if(n>sizeof text || strlen(hex)!=n*2) return 2;
    for(size_t i=0;i<n;i++) {unsigned v;if(sscanf(hex+2*i,"%2x",&v)!=1)return 3;text[i]=(unsigned char)v;}
    unsigned char before[1048576];memcpy(before,text,n);
    int64_t result=ja_admit((coax_string){(char*)text,(int64_t)n},ja_policy(domain),frame,proto);
    if(memcmp(before,text,n)) return 4;
    printf("%lld\n",(long long)result);
  }
  return 0;
}
'''
class AdmissionTests(unittest.TestCase):
    def test_independent_oracle_native_sanitized(self):
        cases=[]
        def add(s,d):
            b=s.encode('utf-8');cases.append((b,d,max(1,len(b)) if d==0 else 0,0,expected(s,d)))
        basic=['{}','[]','1e999','9007199254740992','"\\ud800"','"\\ud83d\\ude00"','{"a":1,"\\u0061":2}',
               '{"x":{"a":1},"a":2}','{"é":1,"\\u00e9":2}','{"a\\u0000":1,"a\\u0000":2}',
               '[1,]','{"a":}','NaN','Infinity','{"x":1} false','{"x":"\\udc00"}',
               '{"x":{"a":1,"a":2}}','{"a":1,"ab":2}','['*16+']'*16,'['*17+']'*17]
        for d in range(3):
            for s in basic:add(s,d)
            for key in ['kind','created_at','exp','iat','nbf','unknown']:
                for v in ['0','-0','9007199254740991','-9007199254740991','9007199254740992','-9007199254740992','9007199254740993','1e0','1.0','"1"','null','[]','{}','true']:
                    add('{'+json.dumps(key)+':'+v+'}',d)
            for key in ['é','x','😀','a\x00b','𝄞','/','\\']:
                a=json.dumps(key,ensure_ascii=False);b=json.dumps(key,ensure_ascii=True)
                add('{'+a+':1,'+b+':2}',d)
                add('[{'+a+':1},{'+b+':2}]',d)
        for d in range(3):
            add('"'+'a'*65536+'"',d)
            add('"'+'a'*65537+'"',d)
        # Raw escapes have their own bound, independent of decoded byte count.
        for d in range(3):
            b=('"'+'\\u0061'*10923+'"').encode()
            cases.append((b,d,len(b) if d==0 else 0,0,False))
        rng=random.Random(20261028)
        for _ in range(3000):
            d=rng.randrange(3);pairs=[]
            for k in range(rng.randrange(1,9)):
                key=rng.choice(['a','ab','é','😀','kind','created_at','exp','nbf','iat','u'+str(k)])
                val=rng.choice(['1','1e999','-0','9007199254740992','null','"\\ud800"','"x"','[1,2]','{"x":1}'])
                pairs.append(json.dumps(key,ensure_ascii=bool(rng.randrange(2)))+':'+val)
            add('{'+','.join(pairs)+'}',d)
        with tempfile.TemporaryDirectory() as t:
            p=Path(t)
            for mode in ['checked','release']:
                cmd=[str(ROOT/'coax/bin/coax'),'c',str(SOURCE),'--library','--header',str(p/'admit.h')]
                if mode=='release':cmd+=['--release']
                (p/'admit.c').write_bytes(subprocess.check_output(cmd,timeout=90))
                (p/'driver.c').write_text(DRIVER)
                subprocess.run(['clang','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all',str(p/'admit.c'),str(p/'driver.c'),'-lm','-o',str(p/'admit')],check=True,timeout=60)
                payload=''.join(f'{d} {f} {pr} {len(b)} {b.hex()}\n' for b,d,f,pr,_ in cases)
                run=subprocess.run([str(p/'admit')],input=payload,text=True,capture_output=True,timeout=60,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'})
                self.assertEqual(run.returncode,0,run.stderr)
                results=run.stdout.splitlines();self.assertEqual(len(results),len(cases))
                for case,result in zip(cases,results):self.assertEqual(int(result)==0,case[-1],(mode,case[0][:120],len(case[0]),case[1:4],int(result)))
        print(f'{len(cases)} independent JSON cases per mode; original bytes preserved')

    def test_reference_snapshot_identity(self):
        # json_parser.py exercises current emitted code against this immutable
        # baseline, including token/range/decode/number and partial-writer parity.
        from json_parser import REFERENCE,REFERENCE_SHA256
        import hashlib
        original=subprocess.check_output(['git','show',REFERENCE+':coax/lib/nostr/json.coax'],cwd=ROOT)
        self.assertEqual(hashlib.sha256(original.split(b'// ---- checks')[0]).hexdigest(),REFERENCE_SHA256)

if __name__=='__main__':unittest.main()
