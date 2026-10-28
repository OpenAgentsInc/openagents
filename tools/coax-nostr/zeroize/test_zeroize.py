#!/usr/bin/env python3
"""Explicit identified-buffer erasure checks; never a hidden-copy guarantee."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT/'coax/bin/coax'))
SOURCE = ROOT/'coax/lib/crypto/zeroize.coax'
PLATFORM = ROOT/'coax/lib/crypto/platform'
OBJDUMP = Path('/opt/homebrew/opt/llvm/bin/llvm-objdump')
WASM_DIS = Path('/opt/homebrew/opt/binaryen/bin/wasm-dis')
CLANG = Path(os.environ.get('COAXIAL_CLANG', '/opt/homebrew/opt/llvm/bin/clang'))
SYSROOT = Path(os.environ.get('COAXIAL_SYSROOT', '/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'))
RESOURCES = Path(os.environ.get('COAXIAL_RESOURCES', '/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'))

def run(*args, **kwargs):
    result = subprocess.run([str(x) for x in args], capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f'{args[0]} failed ({result.returncode}): {result.stderr or result.stdout}')
    return result

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

class Record(C.Structure):
    _fields_ = [('id', C.c_uint64), ('capacity', C.c_int64), ('secret', C.c_int64)]

class Policy(C.Structure):
    _fields_ = [('records', Record*8), ('count', C.c_int64), ('phase', C.c_uint32),
                ('pending', C.c_uint32), ('error', C.c_uint32), ('reason', C.c_uint32)]

class Scope(C.Structure):
    _fields_ = [('policy', Policy), ('buffers', C.c_void_p*8)]

def bind(path):
    library = C.CDLL(str(path))
    library.coax_secure_zero.argtypes = [C.c_void_p, C.c_int64, C.c_int64, C.c_int64]
    library.coax_secure_zero.restype = C.c_uint32
    library.coax_secret_register.argtypes = [C.POINTER(Scope), C.c_uint64, C.c_void_p, C.c_int64, C.c_int64]
    library.coax_secret_copy.argtypes = [C.POINTER(Scope), C.c_uint64, C.c_uint64, C.c_int64]
    library.coax_secret_close.argtypes = [C.POINTER(Scope), C.c_uint32]
    library.coax_secret_reset.argtypes = [C.POINTER(Scope)]
    for name in ['coax_secret_register', 'coax_secret_copy', 'coax_secret_close', 'coax_secret_reset']:
        getattr(library, name).restype = C.c_uint32
    library.zero_scope_complete.argtypes = [Policy]
    library.zero_scope_complete.restype = C.c_bool
    return library

def native_function(assembly, name):
    match = re.search(r'<_' + name + r'>:\n(.*?)(?=\n[0-9a-f]+ <|\Z)', assembly, re.S)
    if not match:
        raise AssertionError(f'missing native function {name}')
    return match[1]

def audit_native(assembly, lto):
    probe = native_function(assembly, 'zero_probe')
    if lto:
        stores = len(re.findall(r'\bstrb\s+wzr,', probe))
        if stores != 64:
            raise AssertionError(f'unobserved native probe has {stores}/64 byte-zero stores')
    else:
        effect = native_function(assembly, 'coax_secure_zero')
        stores = len(re.findall(r'\bstrb\s+wzr,', effect))
        if stores < 1 or '<_coax_secure_zero>' not in probe:
            raise AssertionError('native probe does not reach retained volatile byte stores')
    return stores

def audit_wasm(wat, lto):
    # Count only byte stores of a literal zero, excluding stack/global metadata
    # writes and the seed initialization. LTO fully expands the 64-byte probe.
    stores = 0
    for match in re.finditer(r'\(i32\.store8\b', wat):
        depth = 1
        end = match.end()
        while depth:
            if wat[end] == '(': depth += 1
            elif wat[end] == ')': depth -= 1
            end += 1
        if re.search(r'\(i32\.const 0\)\s*\)$', wat[match.start():end]): stores += 1
    if (lto and stores != 64) or (not lto and stores < 1):
        raise AssertionError(f'unobserved Wasm probe lacks required byte-zero stores: {stores}')
    return stores

class Zeroize(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='coax-zeroize-')
        cls.directory = Path(cls.temp.name)
        cls.observations = []
        cls.libraries = []
        cls.platform_text = (PLATFORM/'zeroize.c').read_text()
        for mode, flags in [('checked', []), ('release', ['--release'])]:
            generated = cls.directory/(mode+'.c')
            generated.write_text(run(COMPILER, 'c', SOURCE, '--library', '--header', cls.directory/'zeroize-policy.h', *flags).stdout)
            for lto in [False, True]:
                output = cls.directory/(mode+('-lto' if lto else '')+'.dylib')
                run('cc', '-std=c11', '-O3', *(['-flto'] if lto else []), '-I'+str(cls.directory), '-I'+str(PLATFORM), '-shared', '-fPIC', generated, PLATFORM/'zeroize.c', '-lm', '-o', output)
                cls.libraries.append(bind(output))

    @classmethod
    def tearDownClass(cls):
        paths = [SOURCE, PLATFORM/'zeroize.c', PLATFORM/'zeroize.h', HERE/'probe.c', HERE/'functional.c', HERE/'wasm.cjs', Path(__file__)]
        report = {'schema':'coax.zeroize-artifact-observations.v1', 'scope':'identified registered caller buffers only',
                  'compiler':run(COMPILER, '--version').stdout.strip(), 'compiler_sha256':digest(COMPILER),
                  'host':platform.platform(), 'native_compiler':run('cc', '--version').stdout.splitlines()[0],
                  'wasm_compiler':run(CLANG, '--version').stdout.splitlines()[0],
                  'wasm_disassembler':run(WASM_DIS, '--version').stdout.strip(),
                  'source_sha256':{str(p.relative_to(ROOT)):digest(p) for p in paths}, 'artifacts':cls.observations,
                  'native_functional_libraries':{Path(lib._name).stem:digest(lib._name) for lib in cls.libraries},
                  'excluded':['registers','unregistered/hidden compiler and ABI copies','OS/swap/snapshots','physical device certification']}
        Path(os.environ.get('COAX_ZEROIZE_REPORT', '/tmp/coax-zeroize-artifacts.json')).write_text(json.dumps(report, indent=2)+'\n')
        cls.temp.cleanup()

    def fixture(self, library):
        source = (C.c_uint8*10)(*([77]*10))
        scratch = (C.c_uint8*10)(*([88]*10))
        scope = Scope()
        self.assertEqual(library.coax_secret_register(C.byref(scope), 1, source, 8, 4), 0)
        self.assertEqual(library.coax_secret_register(C.byref(scope), 2, scratch, 8, 0), 0)
        return scope, source, scratch

    def assert_clean(self, library, reason=0):
        scope, source, scratch = self.fixture(library)
        self.assertEqual(library.coax_secret_copy(C.byref(scope), 1, 2, 4), 0)
        self.assertEqual(library.coax_secret_close(C.byref(scope), reason), 0)
        self.assertTrue(library.zero_scope_complete(scope.policy))
        self.assertEqual(bytes(source), bytes(4)+bytes([77])*6)
        self.assertEqual(bytes(scratch), bytes(8)+bytes([88])*2)
        self.assertEqual(library.coax_secret_reset(C.byref(scope)), 0)
        self.assertEqual(scope.policy.count, 0)

    def fault_library(self, name, text):
        source = self.directory/(name+'.c'); source.write_text(text)
        output = self.directory/(name+'.dylib')
        run('cc', '-std=c11', '-O3', '-flto', '-I'+str(self.directory), '-I'+str(PLATFORM), '-shared', '-fPIC', self.directory/'checked.c', source, '-lm', '-o', output)
        return bind(output)

    def wasm(self, mode, lto, source, extra, output):
        run(CLANG, '--target=wasm32-wasip1', '--sysroot='+str(SYSROOT), '-resource-dir='+str(RESOURCES), '-std=c11', '-O3', *(['-flto'] if lto else []),
            '-I'+str(self.directory), '-I'+str(PLATFORM), '-mexec-model=reactor', self.directory/(mode+'.c'), source, extra,
            '-Wl,--export-memory,--export=__heap_base,-z,stack-size=1048576,--max-memory=67108864,--gc-sections', '-lm', '-o', output)

    def test_native_observable_ranges_aliases_and_cleanup_paths(self):
        for library in self.libraries:
            for capacity in range(17):
                for at in range(capacity+1):
                    data = (C.c_uint8*(capacity+2))(*([165]*(capacity+2)))
                    length = capacity-at
                    self.assertEqual(library.coax_secure_zero(C.byref(data, 1), capacity, at, length), 0)
                    self.assertEqual(bytes(data), bytes([165])*(at+1)+bytes(length)+bytes([165]))
            self.assertEqual(library.coax_secure_zero(None, 0, 0, 0), 0)
            self.assertEqual(library.coax_secure_zero(None, 1, 0, 1), 3)
            data = (C.c_uint8*1048576)(*([165]*1048576))
            self.assertEqual(library.coax_secure_zero(data, len(data), 0, len(data)), 0)
            self.assertEqual(bytes(data), bytes(len(data)))
            guarded = (C.c_uint8*16)(*([77]*16))
            for capacity, at, length in [(-1,0,0),(1048577,0,0),(4,-1,1),(4,0,5),(4,2**63-1,1),(4,0,-2**63)]:
                self.assertNotEqual(library.coax_secure_zero(guarded, capacity, at, length), 0)
                self.assertEqual(bytes(guarded), bytes([77])*16)
            for reason in range(3):
                self.assert_clean(library, reason)
            scope, source, scratch = self.fixture(library)
            saved = bytes(source), bytes(scratch)
            self.assertEqual(library.coax_secret_register(C.byref(scope), 3, C.byref(source, 1), 8, 0), 4)
            self.assertEqual(library.coax_secret_copy(C.byref(scope), 1, 99, 4), 7)
            self.assertEqual(library.coax_secret_copy(C.byref(scope), 1, 1, 1), 4)
            self.assertEqual(library.coax_secret_reset(C.byref(scope)), 5)
            self.assertEqual((bytes(source),bytes(scratch)), saved)
            self.assertEqual(library.coax_secret_close(C.byref(scope), 2), 0)

    def test_omitted_wipe_early_return_and_copy_escape_are_rejected(self):
        omitted = re.sub(r'static uint32_t erase_range\([^}]+\}', 'static uint32_t erase_range(volatile uint8_t *buffer,size_t count){(void)buffer;(void)count;return 0;}', self.platform_text, count=1)
        self.assertNotEqual(omitted, self.platform_text)
        with self.assertRaises(AssertionError): self.assert_clean(self.fault_library('omitted', omitted))
        early = self.platform_text.replace('coax_t_ZeroChange change=zero_scope_begin(scope->policy,reason);', 'return 0;\n    coax_t_ZeroChange change=zero_scope_begin(scope->policy,reason);')
        with self.assertRaises(AssertionError): self.assert_clean(self.fault_library('early', early))
        start = self.platform_text.index('uint32_t coax_secret_copy(')
        end = self.platform_text.index('uint32_t coax_secret_close(')
        body = self.platform_text[start:end]; self.assertEqual(body.count('scope->policy=change.scope;'), 1)
        escaped = self.platform_text[:start]+body.replace('scope->policy=change.scope;', '')+self.platform_text[end:]
        with self.assertRaises(AssertionError): self.assert_clean(self.fault_library('escaped', escaped))

    def test_failed_and_partial_effects_remain_tracked_and_retry(self):
        failed = re.sub(r'static uint32_t erase_range\([^}]+\}', 'static uint32_t erase_range(volatile uint8_t *buffer,size_t count){(void)buffer;(void)count;return 6;}', self.platform_text, count=1)
        bad = self.fault_library('effect-failed', failed)
        scope, source, scratch = self.fixture(bad)
        self.assertEqual(bad.coax_secret_copy(C.byref(scope), 1, 2, 4), 0)
        self.assertEqual(bad.coax_secret_close(C.byref(scope), 1), 6)
        self.assertFalse(bad.zero_scope_complete(scope.policy)); self.assertEqual(scope.policy.pending, 3)
        self.assertEqual(self.libraries[0].coax_secret_close(C.byref(scope), 1), 0)
        self.assertEqual(bytes(source)[:4], bytes(4)); self.assertEqual(bytes(scratch)[:8], bytes(8))
        partial = self.platform_text.replace('if(length)memcpy(scope->buffers[change.slot],scope->buffers[from],(size_t)length);', 'if(length){memcpy(scope->buffers[change.slot],scope->buffers[from],1);return 6;}')
        self.assertNotEqual(partial, self.platform_text)
        bad = self.fault_library('partial-copy', partial)
        scope, source, scratch = self.fixture(bad)
        self.assertEqual(bad.coax_secret_copy(C.byref(scope), 1, 2, 4), 6)
        self.assertEqual(scope.policy.records[1].secret, 8)
        self.assertEqual(bad.coax_secret_close(C.byref(scope), 2), 0)
        self.assertEqual(bytes(source)[:4], bytes(4)); self.assertEqual(bytes(scratch)[:8], bytes(8))

    def test_native_unobserved_O3_and_LTO_artifacts(self):
        self.assertEqual(platform.machine(), 'arm64', 'native instruction audit is explicitly Apple arm64 only')
        for mode in ['checked', 'release']:
            for lto in [False, True]:
                output = self.directory/f'probe-{mode}-{lto}'
                run('cc', '-std=c11', '-O3', *(['-flto'] if lto else []), '-I'+str(self.directory), '-I'+str(PLATFORM), self.directory/(mode+'.c'), PLATFORM/'zeroize.c', HERE/'probe.c', '-lm', '-o', output)
                assembly = run(OBJDUMP, '--no-show-raw-insn', '-d', output).stdout
                stores = audit_native(assembly, lto); run(output)
                self.observations.append({'target':'Apple arm64', 'mode':mode, 'O3':True, 'LTO':lto, 'probe_buffer_post_wipe_observation':False, 'zero_byte_store_instructions':stores, 'artifact_sha256':digest(output), 'disassembly_sha256':hashlib.sha256(assembly.encode()).hexdigest()})
        omitted = self.directory/'omitted-artifact.c'
        omitted.write_text(re.sub(r'static uint32_t erase_range\([^}]+\}', 'static uint32_t erase_range(volatile uint8_t *p,size_t n){(void)p;(void)n;return 0;}', self.platform_text, count=1))
        output = self.directory/'bad-probe'
        run('cc', '-std=c11', '-O3', '-flto', '-I'+str(self.directory), '-I'+str(PLATFORM), self.directory/'checked.c', omitted, HERE/'probe.c', '-lm', '-o', output)
        with self.assertRaises(AssertionError): audit_native(run(OBJDUMP, '--no-show-raw-insn', '-d', output).stdout, True)

    def test_actual_wasm_observable_and_unobserved_optimized_artifacts(self):
        for tool in [CLANG, SYSROOT, RESOURCES, WASM_DIS]: self.assertTrue(tool.exists(), f'explicit tool missing: {tool}')
        for mode in ['checked', 'release']:
            for lto in [False, True]:
                functional = self.directory/f'functional-{mode}-{lto}.wasm'
                self.wasm(mode, lto, PLATFORM/'zeroize.c', HERE/'functional.c', functional)
                result = json.loads(run('node', HERE/'wasm.cjs', functional).stdout)
                self.assertEqual((result['ranges'],result['scopes']), (2145,3))
                output = self.directory/f'probe-{mode}-{lto}.wasm'
                self.wasm(mode, lto, PLATFORM/'zeroize.c', HERE/'probe.c', output)
                wat = run(WASM_DIS, output).stdout; stores = audit_wasm(wat, lto)
                self.assertTrue(json.loads(run('node', HERE/'wasm.cjs', output).stdout)['probe'])
                self.observations.append({'target':'wasm32-wasip1 Node/V8/WASI', 'mode':mode, 'O3':True, 'LTO':lto, 'probe_buffer_post_wipe_observation':False, 'zero_byte_store_instructions':stores, 'functional':result, 'functional_artifact_sha256':digest(functional), 'artifact_sha256':digest(output), 'disassembly_sha256':hashlib.sha256(wat.encode()).hexdigest()})
        omitted = self.directory/'omitted-artifact.c'
        omitted.write_text(re.sub(r'static uint32_t erase_range\([^}]+\}', 'static uint32_t erase_range(volatile uint8_t *p,size_t n){(void)p;(void)n;return 0;}', self.platform_text, count=1))
        output = self.directory/'bad-probe.wasm'; self.wasm('checked', True, omitted, HERE/'probe.c', output)
        with self.assertRaises(AssertionError): audit_wasm(run(WASM_DIS, output).stdout, True)

    def test_native_ASan_UBSan_real_adapter_and_malformed_scope(self):
        host = self.directory/'sanitized.c'
        host.write_text('''#include "zeroize.h"
#include <assert.h>
int main(void){uint8_t a[10],b[10];for(int i=0;i<10;i++){a[i]=77;b[i]=88;}
coax_secret_scope s={0};assert(coax_secret_register(&s,1,a,8,4)==0);assert(coax_secret_register(&s,2,b,8,0)==0);
assert(coax_secret_copy(&s,1,2,4)==0);assert(coax_secret_close(&s,2)==0);assert(zero_scope_complete(s.policy));
for(int i=0;i<4;i++)assert(a[i]==0);for(int i=0;i<8;i++)assert(b[i]==0);assert(a[8]==77&&b[8]==88);
s.policy.count=9;assert(coax_secret_register(&s,3,a,8,4)==5);assert(coax_secret_copy(&s,1,2,4)==5);assert(coax_secret_close(&s,2)==5);
assert(coax_secure_zero(0,0,0,0)==0);assert(coax_secure_zero(0,1,0,1)==3);assert(coax_secret_close(0,0)==5);
assert(coax_secure_zero(a,8,-9223372036854775807LL-1,0)!=0);assert(coax_secure_zero(a,8,0,9223372036854775807LL)!=0);return 0;}''')
        output = self.directory/'sanitized'
        run('cc', '-std=c11', '-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer', '-I'+str(self.directory), '-I'+str(PLATFORM), self.directory/'checked.c', PLATFORM/'zeroize.c', host, '-lm', '-o', output)
        run(output)

if __name__ == '__main__': unittest.main()
