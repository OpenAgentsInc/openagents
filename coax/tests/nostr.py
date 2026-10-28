#!/usr/bin/env python3
"""Compile the Coax event projection consumer and feed canonical protobuf bytes."""
import ctypes as C
from pathlib import Path
import subprocess
import tempfile
import unittest
ROOT=Path(__file__).resolve().parents[2]

def varint(n):
    out=bytearray()
    while n>=128:out.append((n&127)|128);n>>=7
    out.append(n)
    return bytes(out)

class CoaxNostrTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory();d=Path(cls.temp.name)
        source=subprocess.check_output([str(ROOT/'coax/bin/coax'),'c',str(ROOT/'coax/examples/nostr-wire.coax'),'--library','--header',str(d/'wire.h')])
        (d/'wire.c').write_bytes(source)
        (d/'host.c').write_text('#include "wire.h"\n#include <stddef.h>\nint64_t run(const unsigned char*p,size_t n){coax_t_ProtoBuffer b={0};b.bytes.data=(uint8_t*)p;b.bytes.count=(int64_t)n;return nostr_projection_kind(b);}')
        subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(d/'wire.c'),str(d/'host.c'),'-lm','-o',str(d/'wire.dylib')],check=True)
        cls.lib=C.CDLL(str(d/'wire.dylib'));cls.lib.run.argtypes=[C.c_void_p,C.c_size_t];cls.lib.run.restype=C.c_int64
    @classmethod
    def tearDownClass(cls):cls.temp.cleanup()
    def kind(self,data):
        before=data[:]
        result=self.lib.run(C.c_char_p(data),len(data))
        self.assertEqual(data,before)
        return result
    def test_canonical_source_event_fixture(self):
        data=(ROOT/'proto/nostr/fixtures/owner-attested.event.pb').read_bytes()
        self.assertEqual(self.kind(data),1)
        self.assertEqual(self.kind(data+b'\xf8\x07\x00'),1) # Unknown protobuf field retained by caller.
        self.assertEqual(self.kind(data[:-1]),-1)
    def test_kind_zero_presence_and_all_block_kinds(self):
        for kind in [0,22242,24200,30174,30175,30178,44200,45010,39000,39001,39002,39005,39006,39007,9033,9035,9036,13535,65535]:
            self.assertEqual(self.kind(b'\x20'+varint(kind)),kind)
        self.assertEqual(self.kind(b''),-1)
        self.assertEqual(self.kind(b'\x20'+varint(65536)),-1)
    def test_refuse_duplicate_truncated_and_oversized_wire(self):
        for data in [b'\x20\x00\x20\x01',b'\x20\x80',b'\x22\x00',b'\x00',b'\x0a\xff\xff\xff\xff\x0f',b'\x20\x01\x19',b'\x20\x01\x23',b'\x20\x01\x08'+b'\xff'*9+b'\x02',b'\x20\x00'+b'\x00'*1048576]:
            with self.subTest(size=len(data)):self.assertEqual(self.kind(data),-1)
    def test_skip_full_width_timestamp_and_opaque_content(self):
        wire=b'\x18'+varint(2**64-1)+b'\x20'+varint(24200)+b'\x32\x04YQ=='
        self.assertEqual(self.kind(wire),24200)


TRAP_ALLOC = r"""
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "check.h"
/* The allocator traps: any call from the parser aborts the process. */
void *malloc(size_t n) { (void)n; abort(); }
void *calloc(size_t a, size_t b) { (void)a; (void)b; abort(); }
void *realloc(void *p, size_t n) { (void)p; (void)n; abort(); }
void free(void *p) { if (p) abort(); }
static char line[1 << 20];
int main(int argc, char **argv) {
  int fd_lines = 0, ok = 0;
  FILE *f = fopen(argv[1], "rb");   /* stdio's buffer comes from libc's own allocator, before any parse */
  if (!f) return 2;
  static char all[1 << 22]; size_t n = fread(all, 1, sizeof all, f);
  size_t at = 0;
  while (at < n) {
    size_t e = at; while (e < n && all[e] != '\n') e++;
    coax_string s = { all + at, (int64_t)(e - at) };
    fd_lines++; ok += json_check(s) ? 1 : 0;
    at = e + 1;
  }
  coax_string bad = { "[1,]", 4 };
  if (json_check(bad)) return 3;
  (void)line; (void)argc;
  char out[64]; snprintf(out, sizeof out, "%d %d\n", fd_lines, ok);
  return write(1, out, strlen(out)) > 0 ? 0 : 4;
}
"""

class CoaxJsonTests(unittest.TestCase):
    def test_json_parses_the_relay_fixture_without_allocating(self):
        """coax/lib/nostr/json.coax built natively with a trapping allocator parses every captured frame."""
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            (d/'check.coax').write_text(f'#load "{ROOT}/coax/lib/nostr/json.coax";\n'
                'json_check :: (text: string) -> bool #export_c { out: [65536] u8; j: [1] Json; json_start(j[..], text);\n'
                '  while true { t := json_next(j[..]); if t == JSON_END { return true; } if t == JSON_ERROR { return false; }\n'
                '    if t == JSON_STRING && json_unescape(json_text(j[..]), out[..]) < 0 { return false; } } return false; }\n')
            c = subprocess.check_output([str(ROOT/'coax/bin/coax'), 'c', str(d/'check.coax'), '--library', '--header', str(d/'check.h')])
            (d/'check.c').write_bytes(c)
            (d/'main.c').write_text(TRAP_ALLOC.replace('#include <string.h>', '#include <string.h>\n#include <unistd.h>'))
            subprocess.run(['/usr/bin/clang', '-std=c11', '-O1', '-w', str(d/'check.c'), str(d/'main.c'), '-lm', '-o', str(d/'check')], check=True)
            frames = (ROOT/'coax/lib/nostr/fixtures/coax_relay.jsonl').read_text().count('\n')
            out = subprocess.run([str(d/'check'), str(ROOT/'coax/lib/nostr/fixtures/coax_relay.jsonl')], capture_output=True, text=True)
            self.assertEqual(out.returncode, 0, out.stderr)
            self.assertEqual(out.stdout.split(), [str(frames), str(frames)])

if __name__=='__main__':unittest.main()
