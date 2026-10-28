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

if __name__=='__main__':unittest.main()
