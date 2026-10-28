#!/usr/bin/env python3
"""Independent RFC4648/oracle and native span tests for emitted Coax codecs.

Maintenance-only stdlib tooling. No compiler bootstrap or app build dependency.
"""
import base64
import binascii
import json
import os
from pathlib import Path
import random
import struct
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
COAX = ROOT / "coax/bin/coax"
DRIVER = r'''
#include "codecs.h"
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#define MAX (2097152 + 16)
static uint8_t input[MAX], expected[MAX], output[MAX];
static int read_u32(FILE *f, uint32_t *out) {
  uint8_t b[4]; if (fread(b,1,4,f)!=4) return 0;
  *out=(uint32_t)b[0]|((uint32_t)b[1]<<8)|((uint32_t)b[2]<<16)|((uint32_t)b[3]<<24); return 1;
}
static coax_t_NC_Result run(uint32_t op, uint8_t *in, int64_t n, uint8_t *out, int64_t cap) {
  coax_v_u8 a={in,n}, b={out,cap}; coax_string s={(char *)in,n};
  if(op==0) return hex_encode(a,b);
  if(op==1) return hex_decode(s,b);
  if(op==2) return base64_encode(a,b);
  return base64_decode(s,b);
}
static int spans(void) {
  for(uint32_t op=0;op<4;op++) {
    const char *texts[]={"fo","666f","fo","Zm8="};
    const char *codes[]={"666f","fo","Zm8=","fo"};
    size_t n=strlen(texts[op]), need=strlen(codes[op]);
    for(int offset=-1;offset<=(int)n;offset++) {
      uint8_t b[128], saved[128]; memset(b,77,sizeof b); memcpy(b+32,texts[op],n); memcpy(saved,b,sizeof b);
      coax_t_NC_Result r=run(op,b+32,(int64_t)n,b+32+offset,(int64_t)need);
      int overlap=(offset<(int)n && offset+(int)need>0);
      if(overlap) { if(r.error!=4 || r.written || memcmp(b,saved,sizeof b)) return 10+(int)op; }
      else { if(r.error || r.written!=(int64_t)need || memcmp(b+32+offset,codes[op],need)) return 20+(int)op; }
    }
    uint8_t b[16]; memset(b,77,sizeof b);
    coax_t_NC_Result r=run(op,b,-1,b+8,4);
    if(r.written || (r.error!=1 && r.error!=3)) return 30+(int)op;
    int64_t excessive=op==1?2097154:op==3?1398108:1048577;
    r=run(op,b,excessive,b+8,4);
    if(r.written || (r.error!=1 && r.error!=3)) return 40+(int)op;
    for(int i=0;i<16;i++) if(b[i]!=77) return 50+(int)op;
    r=run(op,(uint8_t *)texts[op],(int64_t)n,b,-1);
    if(r.error!=2 || r.written) return 60+(int)op;
    r=run(op,NULL,0,NULL,0);
    if(r.error || r.written) return 70+(int)op;
  }
  return 0;
}
int main(int argc,char **argv) {
  if(argc!=2) return 2;
  FILE *f=fopen(argv[1],"rb"); if(!f) return 3;
  uint32_t op,n,cap,error,size; size_t rows=0;
  while(read_u32(f,&op)) {
    if(!read_u32(f,&n)||!read_u32(f,&cap)||!read_u32(f,&error)||!read_u32(f,&size)) return 4;
    if(op>3 || n>MAX || cap>MAX || size>MAX) return 5;
    if(fread(input,1,n,f)!=n || fread(expected,1,size,f)!=size) return 6;
    memset(output,77,sizeof output);
    coax_t_NC_Result r=run(op,input,n,output,cap);
    if(r.error!=error || r.written!=(error?0:size)) {fprintf(stderr,"row %zu op %u result %u/%lld expected %u/%u\n",rows,op,r.error,(long long)r.written,error,size);return 7;}
    if(!error && memcmp(output,expected,size)) {fprintf(stderr,"row %zu byte mismatch\n",rows);return 8;}
    for(size_t i=error?0:size;i<sizeof output;i++) if(output[i]!=77) {fprintf(stderr,"row %zu unexpected write at %zu\n",rows,i);return 9;}
    rows++;
  }
  fclose(f); int e=spans(); if(e) {fprintf(stderr,"span case %d\n",e);return e;}
  printf("%zu vectors and native span cases passed\n",rows); return 0;
}
'''


def canonical64(data):
    try:
        decoded = base64.b64decode(data, validate=True)
    except (ValueError, binascii.Error):
        return None
    return decoded if base64.b64encode(decoded) == data else None


def corpus(path):
    rng = random.Random(0xC0A4648)
    rows = []

    def record(op, data, expected, cap=None, invalid=False):
        if cap is None:
            cap = len(expected) + 3
        error = 1 if invalid else 2 if cap < len(expected) else 0
        rows.append((op, data, cap, error, expected if not error else b""))

    lengths = list(range(97)) + [127, 128, 129, 255, 256, 257, 1023, 1024, 1025, 4096, 8192]
    lengths += [rng.randrange(2049) for _ in range(800)]
    for n in lengths:
        raw = rng.randbytes(n)
        hx, b64 = raw.hex().encode(), base64.b64encode(raw)
        for op, data, expected in [(0, raw, hx), (1, hx, raw), (1, hx.upper(), raw), (2, raw, b64), (3, b64, raw)]:
            record(op, data, expected)
            record(op, data, expected, max(0, len(expected) - 1))
            record(op, data, expected, len(expected))
    for raw, text in [(b"", b""), (b"f", b"Zg=="), (b"fo", b"Zm8="), (b"foo", b"Zm9v"), (b"foob", b"Zm9vYg=="), (b"fooba", b"Zm9vYmE="), (b"foobar", b"Zm9vYmFy")]:
        record(2, raw, text); record(3, text, raw)
    invalid = [b"Zg", b"Zg=", b"Zg===", b"=AAA", b"Z=g=", b"Zg==\n", b"Z g=", b"-_==", b"Zh==", b"Zm9=", b"A===", b"====", b"AAA!", b"AA=A", b"\xffAAA"]
    for data in invalid:
        assert canonical64(data) is None
        record(3, data, b"", invalid=True)
    # Exhaust every nonzero unused pad-bit combination for one-/two-byte quanta.
    alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    for n in [1, 2]:
        for _ in range(256):
            data = bytearray(base64.b64encode(rng.randbytes(n)))
            at, mask = (1, 15) if n == 1 else (2, 3)
            v = alphabet.index(data[at])
            for extra in range(1, mask + 1):
                data[at] = alphabet[v | extra]
                assert canonical64(data) is None
                record(3, bytes(data), b"", invalid=True)
    for data in [b"f", b"fg", b"00 ", b"0x00", b"\xff0", b" 00", b"00\n"]:
        record(1, data, b"", invalid=True)
    # The encoded maximum has room for two pad bytes; without them decoded bytes exceed the ceiling.
    record(3, b"A" * 1398104, b"", invalid=True)
    raw = rng.randbytes(1048576)
    record(0, raw, raw.hex().encode())
    record(2, raw, base64.b64encode(raw))
    record(3, base64.b64encode(raw), raw)
    with path.open("wb") as f:
        for op, data, cap, error, expected in rows:
            f.write(struct.pack("<5I", op, len(data), cap, error, len(expected)))
            f.write(data); f.write(expected)
    return len(rows)


class Codecs(unittest.TestCase):
    def test_independent_oracle_and_real_native_spans(self):
        with tempfile.TemporaryDirectory(prefix="coax-codecs-") as tmp:
            d = Path(tmp)
            rows = corpus(d / "vectors.bin")
            (d / "driver.c").write_text(DRIVER)
            for name, flags in [("checked", []), ("release", ["--release"])]:
                generated = subprocess.check_output([str(COAX), "c", str(ROOT / "coax/lib/nostr/codecs.coax"), "--library", "--header", str(d / "codecs.h"), *flags])
                (d / "codecs.c").write_bytes(generated)
                exe = d / name
                subprocess.run(["clang", "-std=c11", "-O2", "-g", "-fsanitize=address,undefined", "-fno-sanitize-recover=all", "-Wno-unused-function", str(d / "codecs.c"), str(d / "driver.c"), "-lm", "-o", str(exe)], check=True)
                result = subprocess.run([str(exe), str(d / "vectors.bin")], capture_output=True, text=True, env={**os.environ, "ASAN_OPTIONS": "detect_leaks=0"}, timeout=120)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, f"{rows} vectors and native span cases passed\n")
                print(f"{name}: {result.stdout.strip()}")


    def test_pinned_nip44_payload_base64_layer(self):
        source = ROOT / "coax/tests/vectors/upstream/nip44/nip44.vectors.json"
        data = json.loads(source.read_text())["v2"]
        cases = data["valid"]["encrypt_decrypt"] + data["invalid"]["decrypt"]
        rows = []
        malformed = 0
        for case in cases:
            encoded = case["payload"].encode("utf-8")
            decoded = canonical64(encoded)
            if decoded is None:
                malformed += 1
            rows.append((3, encoded, len(decoded or b"") + 3, 1 if decoded is None else 0, decoded or b""))
        self.assertGreater(malformed, 0)
        with tempfile.TemporaryDirectory(prefix="coax-nip44-base64-") as tmp:
            d = Path(tmp)
            with (d / "vectors.bin").open("wb") as f:
                for op, encoded, cap, error, decoded in rows:
                    f.write(struct.pack("<5I", op, len(encoded), cap, error, len(decoded)))
                    f.write(encoded); f.write(decoded)
            (d / "driver.c").write_text(DRIVER)
            for name, flags in [("checked", []), ("release", ["--release"])]:
                generated = subprocess.check_output([str(COAX), "c", str(ROOT / "coax/lib/nostr/codecs.coax"), "--library", "--header", str(d / "codecs.h"), *flags])
                (d / "codecs.c").write_bytes(generated)
                exe = d / name
                subprocess.run(["clang", "-std=c11", "-O2", "-g", "-fsanitize=address,undefined", "-fno-sanitize-recover=all", "-Wno-unused-function", str(d / "codecs.c"), str(d / "driver.c"), "-lm", "-o", str(exe)], check=True)
                result = subprocess.run([str(exe), str(d / "vectors.bin")], capture_output=True, text=True, env={**os.environ, "ASAN_OPTIONS": "detect_leaks=0"}, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, f"{len(rows)} vectors and native span cases passed\n")
                print(f"NIP44 base64 {name}: {len(rows)} source payloads ({malformed} malformed base64) passed")


if __name__ == "__main__":
    unittest.main()
