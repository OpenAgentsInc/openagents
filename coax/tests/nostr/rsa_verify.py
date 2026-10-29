#!/usr/bin/env python3
"""Independent integer oracle and pinned Wycheproof verification fixtures.

Public-only primitive evidence. This does not claim FI/JOSE authorization.
"""
import ctypes as c
import hashlib
import json
import os
import random
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'coax/lib/crypto/rsa_verify.coax'
DATA = ROOT / 'coax/tests/nostr/data/rsa'
COMPILER = Path(os.environ.get('COAX_COMPILER', ROOT / 'coax/bin/coax'))
EXPECTED = {
 '2048': '94a917b01ff50fb874cfc05bf29b4af44868d944a6558201cf18380da93fb393',
 '3072': '0f5f18cabfaad3e2792e82f7e9882f8999049b456714de924b8a5e202f61ca43',
 '4096': '957aca128e30bd02c8982f8ca482d6521d16683f4bbc3f193cdfd4c8f04bc667',
 '8192': '86df4075b4ce5954726abfa00cc19ebe8b608d6d99094d5e123d6a3a86568bd9',
}
PREFIX = bytes.fromhex('3031300d060960864801650304020105000420')

class View(c.Structure):
 _fields_ = [('data', c.POINTER(c.c_uint8)), ('count', c.c_int64)]

def view(data):
 buf = (c.c_uint8 * max(1, len(data)))()
 buf[:len(data)] = data
 return View(buf, len(data))

def oracle(n, signature, message):
 if not 256 <= len(n) <= 512 or len(signature) != len(n): return False
 if not n[0] or (len(n) == 256 and n[0] < 128) or not (n[-1] & 1): return False
 value = int.from_bytes(signature, 'big')
 modulus = int.from_bytes(n, 'big')
 if value >= modulus: return False
 em = pow(value, 65537, modulus).to_bytes(len(n), 'big')
 encoded = b'\x00\x01' + b'\xff' * (len(n) - 54) + b'\x00' + PREFIX + hashlib.sha256(message).digest()
 return em == encoded

def main():
 fixtures = []
 for size, expected in EXPECTED.items():
  path = DATA / f'rsa_signature_{size}_sha256_test.json'
  assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, path
  payload = json.loads(path.read_text())
  for group in payload['testGroups']:
   if int(group['publicKey']['publicExponent'], 16) != 65537: continue
   n = bytes.fromhex(group['publicKey']['modulus']).lstrip(b'\0')
   for case in group['tests']:
    sig, msg = bytes.fromhex(case['sig']), bytes.fromhex(case['msg'])
    expected_result = oracle(n, sig, msg)
    if case['result'] == 'valid' and size != '8192': assert expected_result, (size, case['tcId'])
    if size == '8192': assert not expected_result, (size, case['tcId'])
    if case['result'] == 'invalid': assert not expected_result, (size, case['tcId'])
    fixtures.append((size, case['tcId'], n, sig, msg, expected_result))
 with tempfile.TemporaryDirectory(prefix='coax-rsa-') as tmp:
  vectors = Path(tmp) / 'vectors.json'
  vectors.write_text(json.dumps([{'size': size, 'id': tcid, 'n': n.hex(), 's': sig.hex(), 'm': msg.hex(), 'expected': expected} for size, tcid, n, sig, msg, expected in fixtures]))
  for mode in ('checked', 'release'):
   generated, lib = Path(tmp) / (mode + '.c'), Path(tmp) / (mode + '.dylib')
   cmd = [str(COMPILER), 'c', str(SOURCE), '--library', '--header', str(Path(tmp) / 'rsa.h'), '-o', str(generated)]
   if mode == 'release': cmd.append('--release')
   subprocess.run(cmd, check=True)
   wrapper = Path(tmp) / (mode + '_wrapper.c')
   wrapper.write_text('#include "' + str(generated) + '"\n'
    'void mont_probe(const uint32_t *ap,const uint32_t *bp,const uint32_t *np,int64_t count,uint32_t *out) {'
    'coax_t_RsaWords a={0},b={0},n={0};'
    'for(int i=0;i<count;i++){a.words.e[i]=ap[i];b.words.e[i]=bp[i];n.words.e[i]=np[i];}'
    'uint32_t inv=coax_p_rsa_mont_inverse(n.words.e[0]);'
    'coax_t_RsaWords r=coax_p_rsa_mont_mul(a,b,n,count,inv);'
    'for(int i=0;i<count;i++)out[i]=r.words.e[i];}\n')
   subprocess.run(['cc', '-std=c11', '-O2', '-shared', '-fPIC', str(wrapper), '-lm', '-o', str(lib)], check=True)
   library = c.CDLL(str(lib))
   probe = library.mont_probe
   ptr = c.POINTER(c.c_uint32)
   probe.argtypes = [ptr, ptr, ptr, c.c_int64, ptr]
   rng = random.Random(20261028)
   for count in (1, 2, 3, 63, 64, 65, 95, 96, 127, 128):
    def words(value): return (c.c_uint32 * count)(*((value >> (32*i)) & 0xffffffff for i in range(count)))
    for _ in range(30):
     n = (rng.getrandbits(count*32) | 1 | (1 << (count*32-1)))
     a, b = rng.randrange(n), rng.randrange(n)
     out = (c.c_uint32 * count)()
     probe(words(a), words(b), words(n), count, out)
     result = sum(out[i] << (32*i) for i in range(count))
     assert result == (a*b*pow(1 << (32*count), -1, n)) % n, (mode, count)
   verify = library.rsa_verify_sha256
   verify.argtypes = [View, View, View]
   verify.restype = c.c_bool
   for size, tcid, n, sig, msg, expected_result in fixtures:
    result = verify(view(n), view(sig), view(msg))
    assert result == expected_result, (mode, size, tcid, result, expected_result)
   print(f'{mode}: {len(fixtures)} Wycheproof cases and 300 seeded Montgomery products match independent integer oracle', flush=True)
   print(f'{mode} generated C sha256 {hashlib.sha256(generated.read_bytes()).hexdigest()}; native library sha256 {hashlib.sha256(lib.read_bytes()).hexdigest()}', flush=True)
   wasm = Path(tmp) / (mode + '.wasm')
   subprocess.run(['/opt/homebrew/opt/llvm/bin/clang', '--target=wasm32-wasip1',
    '--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot',
    '-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes', '-std=c11', '-O2',
    '-I' + tmp, str(generated), str(ROOT / 'coax/tests/nostr/rsa_verify_wasm.c'),
    '-mexec-model=reactor', '-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base',
    '-lm', '-o', str(wasm)], check=True)
   print(f'{mode} Wasm sha256 {hashlib.sha256(wasm.read_bytes()).hexdigest()}', flush=True)
   subprocess.run(['node', str(ROOT / 'coax/tests/nostr/rsa_verify_wasm.cjs'), str(wasm), str(vectors)], check=True)

if __name__ == '__main__': main()
