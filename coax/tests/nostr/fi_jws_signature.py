#!/usr/bin/env python3
"""Original-byte signature tests with OpenSSL and an independent header oracle."""
import base64
import ctypes as c
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from fi_protected import reference as header_oracle
ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/nostr/block/fi-jws-signature.coax'
DATA=ROOT/'coax/tests/nostr/data/fi-jws'
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
WRAPPER='''#include "generated.c"
int verify(const uint8_t *token,int64_t tn,const uint8_t *modulus,int64_t mn) {
 coax_t_FiSignatureCheck r=fi_jws_signature((coax_v_u8){(uint8_t*)token,tn},(coax_v_u8){(uint8_t*)modulus,mn});return r.verified?1:0;
}
int rsa(const uint8_t *n,int64_t nn,const uint8_t *s,int64_t sn,const uint8_t *m,int64_t mn) {
 return rsa_verify_sha256((coax_v_u8){(uint8_t*)n,nn},(coax_v_u8){(uint8_t*)s,sn},(coax_v_u8){(uint8_t*)m,mn})?1:0;
}
'''
enc=lambda data:base64.urlsafe_b64encode(data).rstrip(b'=')
def decode(data):
 raw=base64.b64decode(data+b'='*(-len(data)%4),altchars=b'-_',validate=True)
 if enc(raw)!=data:raise ValueError('noncanonical')
 return raw

def main():
 for name,want in {'token.txt':'b923626e39cdf11a8f6dd2a799c487e21bd49cd196947be5394031e47b100eb0',
                   'modulus.hex':'d543077d28c8d9ea1cdef8920399335e0fdd4f9352d6690baccedc099e09a148',
                   'public.pem':'acd5948b81c890ad7a2bf04f9ac63a333e2bfb9435341acc37e4c75573bdfdb8'}.items():
  assert hashlib.sha256((DATA/name).read_bytes()).hexdigest()==want, name
 original=(DATA/'token.txt').read_bytes().strip();modulus=bytes.fromhex((DATA/'modulus.hex').read_text());header,payload,sig=original.split(b'.')
 rfc_path=DATA/'rfc7515-a2.json';assert hashlib.sha256(rfc_path.read_bytes()).hexdigest()=='aa767928f06159eb7fe6050ebee2d59f3d87492323303d9eec82ce1d3b155892'
 rfc=json.loads(rfc_path.read_text());rfc_token=rfc['compact'].encode();rh,rp,rs=rfc_token.split(b'.');rfc_n=decode(rfc['modulus_base64url'].encode());rfc_sig=decode(rs);rfc_message=rh+b'.'+rp
 assert rfc['exponent_base64url']=='AQAB' and len(rfc_n)==256 and len(rfc_sig)==256
 digest=hashlib.sha256(rfc_message).digest();expected=b'\x00\x01'+b'\xff'*(256-54)+b'\x00'+bytes.fromhex('3031300d060960864801650304020105000420')+digest
 assert pow(int.from_bytes(rfc_sig,'big'),65537,int.from_bytes(rfc_n,'big')).to_bytes(256,'big')==expected
 cases_rfc={'token':base64.b64encode(rfc_token).decode(),'modulus':rfc_n.hex(),'signature':rfc_sig.hex(),'message':base64.b64encode(rfc_message).decode()}
 rawsig=decode(sig);changed=bytes([rawsig[0]^1])+rawsig[1:]
 cases=[original,header+b'.'+payload+b'.'+enc(changed),header+b'.'+enc(b'{"sub":"fixture", "iat":1700000000}')+b'.'+sig,
  enc(b'{"alg":"HS256","typ":"nip-fi+jwt","kid":"test-key"}')+b'.'+payload+b'.'+sig,
  header+b'.'+payload+b'.'+sig+b'=',header+b'..'+sig,header+b'.'+payload+b'.'+sig+b'.x',b'',header+b'.'+payload+b'.'+sig[:-1]]
 with tempfile.TemporaryDirectory(prefix='fi-jws-') as tmp:
  tmp=Path(tmp);rows=[]
  for token in cases:
   try:
    h,p,s=token.split(b'.');hdr=decode(h);decode(p);signature=decode(s)
    if not header_oracle(hdr) or not 256<=len(signature)<=512: raise ValueError('header or width')
    (tmp/'signed').write_bytes(h+b'.'+p);(tmp/'sig').write_bytes(signature)
    valid=subprocess.run(['openssl','dgst','-sha256','-verify',str(DATA/'public.pem'),'-signature',str(tmp/'sig'),str(tmp/'signed')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0
   except (ValueError,TypeError):valid=False
   rows.append({'token':base64.b64encode(token).decode(),'valid':valid})
  assert rows[0]['valid'] and not any(v['valid'] for v in rows[1:])
  fixtures=tmp/'vectors.json';fixtures.write_text(json.dumps({'modulus':modulus.hex(),'cases':rows,'rfc':cases_rfc}))
  for mode in ('checked','release'):
   generated=tmp/'generated.c';wrapper=tmp/'wrapper.c';lib=tmp/(mode+'.dylib');wasm=tmp/(mode+'.wasm')
   command=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(generated)]
   if mode=='release':command.append('--release')
   subprocess.run(command,check=True);wrapper.write_text(WRAPPER)
   subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(wrapper),'-lm','-o',str(lib)],check=True)
   library=c.CDLL(str(lib));verify=library.verify;ptr=c.POINTER(c.c_uint8);verify.argtypes=[ptr,c.c_int64,ptr,c.c_int64];verify.restype=c.c_int
   n=(c.c_uint8*len(modulus)).from_buffer_copy(modulus)
   for token,row in zip(cases,rows):
    t=(c.c_uint8*max(1,len(token)))();t[:len(token)]=token
    assert bool(verify(t,len(token),n,len(modulus)))==row['valid'],(mode,token)
   def buf(data): return (c.c_uint8*len(data)).from_buffer_copy(data)
   raw_rsa=library.rsa;raw_rsa.argtypes=[ptr,c.c_int64,ptr,c.c_int64,ptr,c.c_int64];raw_rsa.restype=c.c_int
   assert raw_rsa(buf(rfc_n),len(rfc_n),buf(rfc_sig),len(rfc_sig),buf(rfc_message),len(rfc_message))==1
   assert verify(buf(rfc_token),len(rfc_token),buf(rfc_n),len(rfc_n))==0 # FI profile requires typ and kid.
   subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(tmp),str(ROOT/'coax/tests/nostr/fi_jws_signature_wasm.c'),'-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
   print(f'{mode}: {len(rows)} JWS cases match OpenSSL; generated_c_sha256={hashlib.sha256(generated.read_bytes()).hexdigest()} native_sha256={hashlib.sha256(lib.read_bytes()).hexdigest()} wasm_sha256={hashlib.sha256(wasm.read_bytes()).hexdigest()}',flush=True)
   subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_jws_signature_wasm.cjs'),str(wasm),str(fixtures)],check=True)
if __name__=='__main__':main()
