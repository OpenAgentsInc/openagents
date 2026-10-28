#!/usr/bin/env python3
"""Pinned BIP173/NIP19 and independent library oracle against emitted Coax C.

Bun dependencies are test-only, pinned by coax/web/project/bun.lock.
"""
import json
import os
from pathlib import Path
import random
import re
import struct
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
COAX = ROOT / "coax/bin/coax"
TOOLS = ROOT / "coax/web/project"
PREFIXES = ["", "npub", "nsec", "note", "nprofile", "nevent", "naddr"]
DRIVER = r'''
#include "bech32.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define MAX 6008
static uint8_t input[MAX],prefix[MAX],expected[MAX],expected_prefix[MAX],out[MAX],hrp[MAX];
static int u32(FILE *f,uint32_t *v){uint8_t b[4];if(fread(b,1,4,f)!=4)return 0;*v=(uint32_t)b[0]|((uint32_t)b[1]<<8)|((uint32_t)b[2]<<16)|((uint32_t)b[3]<<24);return 1;}
static int spans(void){
  uint8_t b[256],saved[256];coax_v_u8 zero={NULL,0};
  for(int shift=0;shift<4;shift++){
    memset(b,77,sizeof b);memcpy(b+32,"a12uel5l",8);memcpy(saved,b,sizeof b);
    coax_t_B32_Result r=bech32_decode((coax_string){(char*)b+32,8},(coax_v_u8){b+32+shift,8},zero);
    if(r.error!=4||r.written||r.hrp_written||memcmp(b,saved,sizeof b))return 11;
  }
  memset(b,77,sizeof b);memcpy(b+32,"a",1);b[64]=0;memcpy(saved,b,sizeof b);
  coax_t_NC_Result e=bech32_encode((coax_string){(char*)b+32,1},(coax_v_u8){b+64,1},(coax_v_u8){b+32,32});
  if(e.error!=4||e.written||memcmp(b,saved,sizeof b))return 12;
  e=bech32_encode((coax_string){"a",1},(coax_v_u8){b+64,1},(coax_v_u8){b+64,32});
  if(e.error!=4||e.written||memcmp(b,saved,sizeof b))return 20;
  const char *generic="abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw";
  coax_t_B32_Result pair=bech32_decode((coax_string){generic,(int64_t)strlen(generic)},(coax_v_u8){b+32,83},(coax_v_u8){b+40,32});
  if(pair.error!=4||pair.written||pair.hrp_written||memcmp(b,saved,sizeof b))return 21;
  memset(b,77,sizeof b);memcpy(b+16,generic,strlen(generic));memcpy(saved,b,sizeof b);
  pair=bech32_decode((coax_string){(char*)b+16,(int64_t)strlen(generic)},(coax_v_u8){b+128,83},(coax_v_u8){b+17,32});
  if(pair.error!=4||pair.written||pair.hrp_written||memcmp(b,saved,sizeof b))return 22;
  e=bech32_encode((coax_string){"a",1},(coax_v_u8){b,-1},(coax_v_u8){b+128,64});if(e.error!=3||e.written)return 13;
  coax_t_B32_Result d=bech32_decode((coax_string){(char*)b,91},zero,zero);if(d.error!=1||d.written||d.hrp_written)return 14;
  coax_t_N19_Result n=nip19_decode((coax_string){(char*)b,-1},(coax_v_u8){b+128,64});if(n.error!=1||n.written)return 15;
  n=nip19_decode((coax_string){(char*)b,5001},(coax_v_u8){b+128,64});if(n.error!=1||n.written)return 16;
  e=nip19_encode(2,(coax_v_u8){b+32,32},(coax_v_u8){b+33,128});if(e.error!=4||e.written)return 17;
  e=nip19_encode(2,(coax_v_u8){b,3121},(coax_v_u8){b+128,64});if(e.error!=1||e.written)return 18;
  const char *key="npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg";
  memset(b,77,sizeof b);memcpy(b+16,key,strlen(key));memcpy(saved,b,sizeof b);
  n=nip19_decode((coax_string){(char*)b+16,(int64_t)strlen(key)},(coax_v_u8){b+17,32});if(n.error!=4||n.written||memcmp(b,saved,sizeof b))return 19;
  return 0;
}
int main(int argc,char **argv){
  if(argc!=2)return 2;FILE*f=fopen(argv[1],"rb");if(!f)return 3;uint32_t a[18];size_t rows=0;
  while(u32(f,&a[0])){
    for(int i=1;i<18;i++)if(!u32(f,&a[i]))return 4;
    uint32_t op=a[0],type=a[1],hn=a[2],n=a[3],cap=a[4],hcap=a[5],error=a[6],size=a[7],hsize=a[8];
    if(op>3||hn>MAX||n>MAX||cap>MAX||hcap>MAX||size>MAX||hsize>MAX)return 5;
    if(fread(prefix,1,hn,f)!=hn||fread(input,1,n,f)!=n||fread(expected,1,size,f)!=size||fread(expected_prefix,1,hsize,f)!=hsize)return 6;
    memset(out,77,sizeof out);memset(hrp,77,sizeof hrp);
    uint32_t actual=0;int64_t written=0,hwritten=0;coax_t_N19_Result meta={0};
    if(op==0){coax_t_NC_Result r=bech32_encode((coax_string){(char*)prefix,hn},(coax_v_u8){input,n},(coax_v_u8){out,cap});actual=r.error;written=r.written;}
    else if(op==1){coax_t_B32_Result r=bech32_decode((coax_string){(char*)input,n},(coax_v_u8){hrp,hcap},(coax_v_u8){out,cap});actual=r.error;written=r.written;hwritten=r.hrp_written;}
    else if(op==2){coax_t_NC_Result r=nip19_encode(type,(coax_v_u8){input,n},(coax_v_u8){out,cap});actual=r.error;written=r.written;}
    else{meta=nip19_decode((coax_string){(char*)input,n},(coax_v_u8){out,cap});actual=meta.error;written=meta.written;}
    if(actual!=error||written!=(error?0:size)||hwritten!=(error?0:hsize)){fprintf(stderr,"row%zu op%u got %u/%lld/%lld expected%u/%u/%u\n",rows,op,actual,(long long)written,(long long)hwritten,error,size,hsize);return 7;}
    if(!error&&(memcmp(out,expected,size)||memcmp(hrp,expected_prefix,hsize))){fprintf(stderr,"row%zu byte mismatch\n",rows);return 8;}
    if(op==3&&!error){uint32_t got[]={meta.type,(uint32_t)meta.special_at,(uint32_t)meta.special_length,meta.has_special,(uint32_t)meta.author_at,meta.has_author,meta.kind,meta.has_kind,(uint32_t)meta.relays};for(int i=0;i<9;i++)if(got[i]!=a[9+i]){fprintf(stderr,"row%zu metadata%d got%u expected%u\n",rows,i,got[i],a[9+i]);return 9;}}
    for(size_t i=error?0:size;i<MAX;i++)if(out[i]!=77){fprintf(stderr,"row%zu unexpected output write\n",rows);return 10;}
    for(size_t i=error?0:hsize;i<MAX;i++)if(hrp[i]!=77){fprintf(stderr,"row%zu unexpected HRP write\n",rows);return 11;}
    rows++;
  }
  fclose(f);int e=spans();if(e){fprintf(stderr,"span case%d\n",e);return e;}
  printf("%zu vectors and native spans passed\n",rows);return 0;
}
'''


def tlv(tag, value):
    assert len(value) <= 255
    return bytes([tag, len(value)]) + value


def metadata(kind, payload):
    if kind <= 3:
        return [kind, 0, 32, 1, 0, 0, 0, 0, 0]
    m = [kind, 0, 0, 0, 0, 0, 0, 0, 0]
    at = 0
    while at < len(payload):
        tag, n = payload[at:at+2]
        value = at + 2
        if tag == 0:
            m[1:4] = [value, n, 1]
        elif tag == 1:
            m[8] += 1
        elif tag == 2 and kind != 4:
            m[4:6] = [value, 1]
        elif tag == 3 and kind != 4:
            m[6:8] = [int.from_bytes(payload[value:value+n], "big"), 1]
        at = value + n
    return m


def oracle(requests):
    script = r'''
import {bech32,bech32m} from "@scure/base";
import * as nip19 from "nostr-effect/nip19";
const req=JSON.parse(await Bun.stdin.text()), out=[];
for(const r of req){
  if(r.mode==="decode"){
    try{const d=bech32.decode(r.text,90);out.push({hrp:d.prefix,words:d.words})}catch{out.push({error:true})}
  }else{
    const raw=Uint8Array.from(Buffer.from(r.payload,"hex"));
    const words=r.words||bech32.toWords(raw),enc=(r.m?bech32m:bech32).encode(r.hrp,words,r.limit||5000);
    if(r.entity){
      let other;const e=r.entity;
      if(r.hrp==="npub")other=nip19.npubEncode(e.key);
      if(r.hrp==="nsec")other=nip19.nsecEncode(raw);
      if(r.hrp==="note")other=nip19.noteEncode(e.key);
      if(r.hrp==="nprofile")other=nip19.nprofileEncode(e);
      if(r.hrp==="nevent")other=nip19.neventEncode(e);
      if(r.hrp==="naddr")other=nip19.naddrEncode(e);
      if(other!==enc)throw new Error("independent NIP19 encoder disagrees");
      if(nip19.decode(enc).type!==r.hrp)throw new Error("independent NIP19 decoder disagrees");
    }
    out.push({text:enc});
  }
}
console.log(JSON.stringify(out));
'''
    p = subprocess.run(["bun", "-e", script], cwd=TOOLS, input=json.dumps(requests), text=True, capture_output=True, timeout=120)
    if p.returncode:
        raise RuntimeError(p.stderr)
    return json.loads(p.stdout)


def make_corpus(path):
    rng = random.Random(0xC0A19)
    requests, descriptions = [], []
    # Every published base-checksum BIP173 vector, excluding Segwit-only policy.
    source = (ROOT / "coax/tests/vectors/upstream/bip173/bip-0173.mediawiki").read_text()
    section = source.split("The following strings are valid Bech32:", 1)[1].split("The following list gives valid segwit", 1)[0]
    good, bad = section.split("The following string are not valid Bech32", 1)
    valid = re.findall(r"^\* <tt>(.*?)</tt>", good, re.M)
    invalid = re.findall(r"^\* <tt>(.*?)</tt>", bad, re.M)
    invalid += [" 1nwldj5", "\x7f1axkwrx", "\x801eym55h", "de1lg7wt\xff", "a12UEL5L", "A1LQFN3A"]
    for text in valid + invalid:
        requests.append(dict(mode="decode", text=text)); descriptions.append(("generic", text))
    for _ in range(10000):
        kind = rng.randrange(1, 7)
        key, author = rng.randbytes(32), rng.randbytes(32)
        relays = [f"wss://r{rng.randrange(10)}.example" for _ in range(rng.randrange(3))]
        if kind <= 3:
            payload = key; entity = dict(key=key.hex())
        else:
            ident = rng.choice(["", "code", "space + slash/", "earth-🌍"])
            payload = tlv(0, ident.encode() if kind == 6 else key)
            payload += b"".join(tlv(1, r.encode()) for r in relays)
            entity = dict(relays=relays)
            if kind == 4:
                entity["pubkey"] = key.hex()
            if kind == 5:
                entity["id"] = key.hex()
            if kind == 6 or rng.randrange(2):
                if kind != 4:
                    payload += tlv(2, author)
                    entity["pubkey" if kind == 6 else "author"] = author.hex()
            if kind == 6 or rng.randrange(2):
                if kind != 4:
                    k = rng.choice([0, 1, 65535, 4294967295, rng.randrange(2**32)])
                    payload += tlv(3, k.to_bytes(4, "big")); entity["kind"] = k
            if kind == 6:
                entity["identifier"] = ident
        requests.append(dict(hrp=PREFIXES[kind], payload=payload.hex(), entity=entity))
        descriptions.append(("nip", kind, payload, True))
    # Unknown TLVs, relay repeats/order and optional field presence.
    for kind in [4, 5, 6]:
        for _ in range(100):
            fields = [tlv(0, b"" if kind == 6 else rng.randbytes(32)), tlv(99, rng.randbytes(9)), tlv(1, b"same"), tlv(1, b"same")]
            if kind == 6:
                fields += [tlv(2, rng.randbytes(32)), tlv(3, b"\0\0\0\0")]
            if kind == 4:
                fields += [tlv(2, b"\xff"), tlv(3, b"\xff")]
            rng.shuffle(fields); payload = b"".join(fields)
            requests.append(dict(hrp=PREFIXES[kind], payload=payload.hex())); descriptions.append(("nip", kind, payload, True))
    # All 0..255-byte identifier sizes, including empty, are valid raw byte fields.
    for n in range(256):
        payload = tlv(0, rng.randbytes(n)) + tlv(2, rng.randbytes(32)) + tlv(3, b"\xff\xff\xff\xff")
        requests.append(dict(hrp="naddr", payload=payload.hex())); descriptions.append(("nip", 6, payload, True))
    invalid_payloads = [(4, b""), (4, tlv(0,b"x")), (5,tlv(0,rng.randbytes(32))+tlv(2,b"x")), (5,tlv(0,rng.randbytes(32))+tlv(3,b"x")), (6,tlv(0,b"")), (6,tlv(0,b"")+tlv(2,rng.randbytes(32))), (6,tlv(0,b"")+tlv(3,b"\0"*4))]
    for kind in [4,5,6]:
        base=tlv(0,b"" if kind==6 else rng.randbytes(32))
        if kind==6:base+=tlv(2,rng.randbytes(32))+tlv(3,b"\0"*4)
        invalid_payloads += [(kind,base+tlv(0,b"" if kind==6 else rng.randbytes(32))), (kind,base+b"\x63"), (kind,base+b"\x63\x02\x00"), (kind,base+tlv(1,b"\x80"))]
    for kind,payload in invalid_payloads:
        requests.append(dict(hrp=PREFIXES[kind],payload=payload.hex()));descriptions.append(("nip",kind,payload,False))
    # Correct Bech32 checksum with invalid NIP19 padding or unsupported HRP.
    for words in [[0], [0]*51+[1], [0]*53, []]:
        requests.append(dict(hrp="npub",payload="",words=words));descriptions.append(("nip_text_bad",))
    requests.append(dict(hrp="nrelay",payload="00"));descriptions.append(("nip_text_bad",))
    requests.append(dict(hrp="npub",payload="00"*32,m=True));descriptions.append(("nip_text_bad",))
    # Long metadata stays within the raised NIP19 limit and exceeds generic BIP173.
    for count in [1, 8, 11]:
        payload=tlv(0,rng.randbytes(32))+b"".join(tlv(1,b"r"*255) for _ in range(count))
        requests.append(dict(hrp="nprofile",payload=payload.hex()));descriptions.append(("nip",4,payload,True))
    payload=tlv(0,b"")+tlv(2,rng.randbytes(32))+tlv(3,b"\0"*4)+b"".join(tlv(99,b"x"*255) for _ in range(11))+tlv(99,b"x"*246)
    assert len(payload)==3117
    requests.append(dict(hrp="naddr",payload=payload.hex()));descriptions.append(("nip",6,payload,True))
    payload=tlv(0,rng.randbytes(32))+b"".join(tlv(1,b"r"*255) for _ in range(12))
    requests.append(dict(hrp="nprofile",payload=payload.hex(),limit=9000));descriptions.append(("nip_oversize",4,payload))
    result=oracle(requests); rows=[]
    def record(op,kind,prefix,data,expected=b"",expected_prefix=b"",meta=None,error=0,cap=None,hcap=83):
        if cap is None:cap=len(expected)+2
        if not error and (cap<len(expected) or hcap<len(expected_prefix)):error=2
        rows.append(([op,kind,len(prefix),len(data),cap,hcap,error,0 if error else len(expected),0 if error else len(expected_prefix),*(meta or [0]*9)],prefix,data,b"" if error else expected,b"" if error else expected_prefix))
    for desc,res in zip(descriptions,result):
        if desc[0]=="generic":
            encoded=desc[1].encode("utf-8")
            if res.get("error"):record(1,0,b"",encoded,error=1)
            else:
                words=bytes(res["words"]);hrp=res["hrp"].encode()
                record(1,0,b"",encoded,words,hrp)
                record(0,0,hrp,words,desc[1].lower().encode())
        elif desc[0]=="nip":
            _,kind,payload,valid=desc;encoded=res["text"].encode()
            if valid:
                meta=metadata(kind,payload)
                if len(encoded)>90:record(1,0,b"",encoded,error=1)
                record(2,kind,b"",payload,encoded)
                record(3,0,b"",encoded,payload,meta=meta)
                record(3,0,b"",encoded.upper(),payload,meta=meta)
                record(3,0,b"",encoded,payload,meta=meta,cap=max(0,len(payload)-1))
                if len(payload)>0:record(2,kind,b"",payload,encoded,cap=len(encoded)-1)
            else:
                record(2,kind,b"",payload,error=1);record(3,0,b"",encoded,error=1)
        elif desc[0]=="nip_oversize":
            record(2,desc[1],b"",desc[2],error=3)
            record(3,0,b"",res["text"].encode(),error=1)
        else:record(3,0,b"",res["text"].encode(),error=1)
    with path.open("wb") as f:
        for header,prefix,data,out,hrp in rows:
            f.write(struct.pack("<18I",*header));f.write(prefix);f.write(data);f.write(out);f.write(hrp)
    return len(rows)


class Bech32(unittest.TestCase):
    def test_pinned_vectors_and_ten_thousand_independent_entities(self):
        with tempfile.TemporaryDirectory(prefix="coax-bech32-") as tmp:
            d=Path(tmp);rows=make_corpus(d/"vectors.bin");(d/"driver.c").write_text(DRIVER)
            for name,flags in [("checked",[]),("release",["--release"])]:
                (d/"bech32.c").write_bytes(subprocess.check_output([str(COAX),"c",str(ROOT/"coax/lib/nostr/bech32.coax"),"--library","--header",str(d/"bech32.h"),*flags]))
                exe=d/name
                subprocess.run(["clang","-std=c11","-O2","-g","-fsanitize=address,undefined","-fno-sanitize-recover=all","-Wno-unused-function",str(d/"bech32.c"),str(d/"driver.c"),"-lm","-o",str(exe)],check=True)
                run=subprocess.run([str(exe),str(d/"vectors.bin")],capture_output=True,text=True,env={**os.environ,"ASAN_OPTIONS":"detect_leaks=0"},timeout=120)
                self.assertEqual(run.returncode,0,run.stderr)
                self.assertEqual(run.stdout,f"{rows} vectors and native spans passed\n")
                print(f"{name}: {run.stdout.strip()}")


if __name__=="__main__":unittest.main()
