#!/usr/bin/env python3
"""Optional differential test tooling for the real JSON decoder/writer."""
import hashlib, json, random, subprocess, tempfile, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
REFERENCE='a165c9bc26a5aeb95a371a137583425ec6948398'
REFERENCE_SHA256='28887ed82a25ff92dd7ee6965ee18417f62421725dd79df23c2664a723c8226b'
OBSERVER=r'''
JsonObservation :: struct {hash:u64;tokens:int;valid:bool;scalars:bool;error_at:int;}
NAME_probe :: (text:string)->JsonObservation #export_c {
  j:[1]Json;json_start(j[..],text);h:=cast(u64)1;n:=0;scalars:=true;buf:[65536]u8;t:=json_next(j[..]);
  while true {
    values:=[8]int.[t,j[0].at,j[0].depth,j[0].want,j[0].from,j[0].to,j[0].error_at,ifx j[0].key then 1 else 0];
    for v:values {h=add_wrap(mul_wrap(h,cast(u64)6364136223846793005),cast(u64)(v+1));}
    n+=1;
    if t==JSON_STRING {length:=json_unescape(json_text(j[..]),buf[..]);
      if length<0 {scalars=false;} else {for b:buf[0..<length] {h=add_wrap(mul_wrap(h,cast(u64)6364136223846793005),cast(u64)b);}}
    }
    if t==JSON_END || t==JSON_ERROR {break;}t=json_next(j[..]);
  }
  return JsonObservation.{hash=h,tokens=n,valid=t==JSON_END,scalars=scalars,error_at=j[0].error_at};
}
NAME_write :: (text:string,out:[]u8)->int #export_c {return json_write_string(out,0,text);}
NAME_integer :: (text:string)->JsonInt #export_c {return json_int(text);}
'''
DRIVER=r'''
#include "cur.h"
#include <stdio.h>
#include <string.h>
#include <stdint.h>
extern coax_t_JsonObservation ref_probe(coax_string);
extern int64_t ref_write(coax_string,coax_v_u8);
extern coax_t_JsonInt ref_integer(coax_string);
static char hex[2100001];static uint8_t data[1048576];
int main(void) {
  size_t n,index=0;
  while(scanf("%zu %2100000s",&n,hex)==2) {
    if(n>sizeof data || (n && strlen(hex)!=n*2))return 2;
    for(size_t i=0;i<n;i++){unsigned v;if(sscanf(hex+i*2,"%2x",&v)!=1)return 3;data[i]=(uint8_t)v;}
    coax_string s={(char*)data,(int64_t)n};coax_t_JsonObservation a=ref_probe(s),b=cur_probe(s);
    if(a.hash!=b.hash || a.tokens!=b.tokens || a.valid!=b.valid || a.scalars!=b.scalars || a.error_at!=b.error_at){fprintf(stderr,"token mismatch at case %zu\n",index);return 4;}
    coax_t_JsonInt x=ref_integer(s),y=cur_integer(s);
    if(x.ok!=y.ok || x.value!=y.value){fprintf(stderr,"integer mismatch at case %zu\n",index);return 5;}
    if(n<=128)for(int cap=0;cap<=20;cap++) {
      uint8_t left[64],right[64];memset(left,165,64);memset(right,165,64);
      int64_t u=ref_write(s,(coax_v_u8){left,cap}),v=cur_write(s,(coax_v_u8){right,cap});
      if(u!=v || memcmp(left,right,64)){fprintf(stderr,"writer mismatch at case %zu cap %d\n",index,cap);return 6;}
    }
    index++;
  }
  printf("%zu exact token/range/decode/integer cases; bounded writer parity\n",index);return 0;
}
'''

def corpus():
    data=[b'',b'null',b'true',b'false',b'0',b'-0',b'9007199254740992',b'-9007199254740992',b'9007199254740993',b'1e1',b'1e',b'{"x":]',b'"\\u000',b'"\\uDC00\\uDC00"']
    rng=random.Random(20261028)
    for _ in range(15000):
        obj={rng.choice(['x','content','é','😀','a\x00b']):rng.choice([None,True,False,rng.randint(-2**53,2**53),rng.random(),[1,2],{'a':'é😀\n'}])}
        b=json.dumps(obj,ensure_ascii=bool(rng.randrange(2)),separators=(',',':')).encode();data.append(b)
        at=rng.randrange(len(b));data.append(b[:at]+bytes([rng.randrange(256)])+b[at+1:])
    for _ in range(10000):data.append(bytes(rng.randrange(256) for _ in range(rng.randrange(1,80))))
    for cp in [0,127,128,2047,2048,0xD7FF,0xE000,0xFFFF,0x10000,0x10FFFF]:
        data.append(('"'+chr(cp)+'"').encode());data.append(('"'+chr(cp)+'"').encode()[:-1])
    for lead in range(256):
        for tail in [0x7f,0x80,0x8f,0x90,0x9f,0xa0,0xbf,0xc0]:data.append(b'"'+bytes([lead,tail,0x80,0x80])+b'"')
    data += (ROOT/'coax/lib/nostr/fixtures/coax_relay.jsonl').read_bytes().splitlines()
    data += [b'"'+b'a'*65536+b'"',b'["'+b'a'*65536+b'"]',b'"'+b'a'*65537+b'"']
    return data

class DecoderParity(unittest.TestCase):
    def test_native_token_ranges_decoding_and_writer_match_frozen_reference(self):
        cases=corpus()
        original=subprocess.check_output(['git','show',REFERENCE+':coax/lib/nostr/json.coax'],cwd=ROOT).decode().split('// ---- checks')[0]
        self.assertEqual(hashlib.sha256(original.encode()).hexdigest(),REFERENCE_SHA256)
        with tempfile.TemporaryDirectory() as t:
            d=Path(t)
            for name in ['ref','cur']:
                pre=original if name=='ref' else '#load '+json.dumps(str(ROOT/'coax/lib/nostr/json.coax'))+';\n'
                (d/(name+'.coax')).write_text(pre+'\n'+OBSERVER.replace('NAME',name))
            for mode in ['checked','release']:
                objects=[]
                for name in ['ref','cur']:
                    cmd=[str(ROOT/'coax/bin/coax'),'c',str(d/(name+'.coax')),'--library','--header',str(d/(name+'.h'))]
                    if mode=='release':cmd+=['--release']
                    (d/(name+'.c')).write_bytes(subprocess.check_output(cmd,timeout=90))
                    obj=d/(name+'.o');objects.append(str(obj))
                    subprocess.run(['clang','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-c',str(d/(name+'.c')),'-o',str(obj)],check=True,timeout=60)
                (d/'driver.c').write_text(DRIVER)
                subprocess.run(['clang','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all',*objects,str(d/'driver.c'),'-lm','-o',str(d/'parity')],check=True,timeout=60)
                payload=''.join(f'{len(b)} {b.hex() if b else "-"}\n' for b in cases)
                result=subprocess.run([str(d/'parity')],input=payload,text=True,capture_output=True,timeout=90)
                self.assertEqual(result.returncode,0,(mode,result.stderr));print(mode,result.stdout.strip())
        self.assertTrue(original)

if __name__=='__main__':unittest.main()
