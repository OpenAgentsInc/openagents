#!/usr/bin/env python3
"""Test-only NIP-98 tag extraction; parsed tags confer no authority."""
import copy
import ctypes as C
import json
import tempfile
import unittest
from pathlib import Path
import api_event_parity as prior
from secp_field import ROOT, COMPILER, View, array, view
from event import Event

class Tags(C.Structure):
    _fields_ = [('valid', C.c_bool)] + [(n, C.c_int64) for n in ('uc','mc','pc','un','mn','pn')]

def encoded(event):
    return json.dumps(event, ensure_ascii=True, separators=(',', ':')).encode()

class TagExtraction(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix='coax-nip98-tags-')
        d = Path(cls.tmp.name); cls.libs = []
        for mode, options in [('checked', []), ('release', ['--release'])]:
            tag_source=d/(mode+'-tags.c'); event_source=d/(mode+'-event.c')
            tag_source.write_text(prior.run(COMPILER, 'c', ROOT/'coax/lib/nostr/nip98-tags.coax', '--library', '--header', d/(mode+'-tags.h'), *options))
            event_source.write_text(prior.run(COMPILER, 'c', ROOT/'coax/lib/nostr/event.coax', '--library', '--header', d/(mode+'-event.h'), *options))
            prior.run('cc', '-std=c11', '-O2', '-shared', '-fPIC', tag_source, '-lm', '-o', d/(mode+'-tags.so'))
            prior.run('cc', '-std=c11', '-O2', '-shared', '-fPIC', event_source, '-lm', '-o', d/(mode+'-event.so'))
            tags=C.CDLL(str(d/(mode+'-tags.so'))); events=C.CDLL(str(d/(mode+'-event.so')))
            tags.n98_source_tags.argtypes=[View,View,View,View,C.c_bool];tags.n98_source_tags.restype=Tags
            events.nostr_event_verify_vartime.argtypes=[View,View,C.c_bool];events.nostr_event_verify_vartime.restype=Event
            cls.libs.append((mode,tags,events))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def call_tags(self, lib, raw, expected, overlap=False):
        source=array(raw); bufs=[array(b'\xa5'*(n+16)) for n in (1024,16,64)]
        views=[View(C.cast(C.byref(buf,8),C.POINTER(C.c_uint8)),len(buf)-16) for buf in bufs]
        r=lib.n98_source_tags(view(source),*views,overlap)
        self.assertEqual(bytes(source),raw)
        for buf in bufs:
            self.assertEqual(bytes(buf[:8]),b'\xa5'*8);self.assertEqual(bytes(buf[-8:]),b'\xa5'*8)
        self.assertEqual(bool(r.valid),expected[0])
        if r.valid:
            self.assertEqual((r.uc,r.mc,r.pc),expected[1])
            self.assertEqual(tuple(bytes(buf[8:8+n]).decode() for buf,n in zip(bufs,(r.un,r.mn,r.pn))),expected[2])
        if overlap:
            for buf in bufs:self.assertEqual(bytes(buf),b'\xa5'*len(buf))
        return r

    def test_original_signed_event_tag_spans(self):
        fixtures=json.loads((ROOT/'coax/tests/nostr/data/api-public-events.json').read_text())
        extra=json.loads((ROOT/'coax/tests/nostr/data/api-request-observation-public.json').read_text())
        cases=[('basic',fixtures['basic'],(True,(1,1,0),('https://example.test/api','POST',''))),
               ('payload',fixtures['payload'],(True,(1,1,1),('https://example.test/api','POST',__import__('hashlib').sha256(b'hello').hexdigest()))),
               ('duplicate',fixtures['url_last_correct'],(True,(2,1,0),('https://example.test/api','POST',''))),
               ('empty',extra['empty_payload'],(True,(1,1,1),('https://example.test/api','POST',''))),
               ('extra_field',extra['extra_url_field'],(False,None,None))]
        for name,event,expected in cases:
            original=encoded(event)
            for mode,tags,events in self.libs:
                with self.subTest(case=name,mode=mode):
                    source=array(original);scratch=array(bytes(262144))
                    parsed=events.nostr_event_verify_vartime(view(source),view(scratch),False)
                    self.assertEqual(parsed.error,0);self.assertTrue(parsed.verified)
                    returned=C.string_at(parsed.original.data,parsed.original.count)
                    self.assertEqual(returned,original);self.assertEqual(bytes(source),original)
                    # The exact span comes from the public verifier's original,
                    # not JSON reserialization. A production bridge must do
                    # this inside Coax while preserving live/immutable views.
                    self.call_tags(tags,returned[parsed.tags_from:parsed.tags_to],expected)

    def test_untrusted_arrays_and_unicode_scope(self):
        fixtures=[(b'[]',(True,(0,0,0),('','',''))),
                  (b'[["unknown","\\ud800"]]',(True,(0,0,0),('','',''))),
                  (b'[["\\ud800","x"]]',(True,(0,0,0),('','',''))),
                  (b'[["u","\\ud800"]]',(False,None,None)),
                  (b'[["u","value","extra"]]',(False,None,None)),
                  (b'[["u"]]',(False,None,None)),
                  (b'[["unknown",2]]',(False,None,None)),
                  (b'[] true',(False,None,None))]
        for mode,tags,events in self.libs:
            for raw,expected in fixtures:
                with self.subTest(mode=mode,raw=raw):self.call_tags(tags,raw,expected)
            self.call_tags(tags,b'[]',(False,None,None),overlap=True)
            event=json.loads((ROOT/'coax/tests/nostr/data/api-public-events.json').read_text())['basic']
            event=copy.deepcopy(event);event['tags'].append(['unknown','\ud800'])
            raw=encoded(event);source=array(raw);scratch=array(bytes(262144))
            rejected=events.nostr_event_verify_vartime(view(source),view(scratch),False)
            self.assertEqual(rejected.error,1);self.assertFalse(rejected.verified)

class SanitizedTags(unittest.TestCase):
    def test_checked_and_release_tag_parser_sanitized(self):
        with tempfile.TemporaryDirectory(prefix='coax-nip98-tags-sanitized-') as tmp:
            d=Path(tmp)
            for mode,options in [('checked',[]),('release',['--release'])]:
                source=d/(mode+'.c');header=d/'tags.h';target=d/mode
                source.write_text(prior.run(COMPILER,'c',ROOT/'coax/lib/nostr/nip98-tags.coax','--library','--header',header,*options))
                prior.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),source,
                    ROOT/'coax/tests/nostr/nip98_tags_sanitized.c','-lm','-o',target)
                prior.run(target)

class WasmTags(unittest.TestCase):
    def test_checked_and_release_tag_parser(self):
        with tempfile.TemporaryDirectory(prefix='coax-nip98-tags-wasm-') as tmp:
            d=Path(tmp)
            for mode,options in [('checked',[]),('release',['--release'])]:
                source=d/(mode+'.c');header=d/'tags.h';target=d/(mode+'.wasm')
                source.write_text(prior.run(COMPILER,'c',ROOT/'coax/lib/nostr/nip98-tags.coax','--library','--header',header,*options))
                prior.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1',
                    '--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot',
                    '-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes',
                    '-std=c11','-O2','-I'+str(d),source,ROOT/'coax/tests/nostr/nip98_tags_wasm.c',
                    '-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=8388608,--export=__heap_base,-z,stack-size=1048576',
                    '-lm','-o',target)
                report=json.loads(prior.run('node',ROOT/'coax/tests/nostr/nip98_tags_wasm.cjs',target))
                self.assertEqual(report['cases'],15);self.assertEqual(report['failures'],0)

if __name__=='__main__':unittest.main()
