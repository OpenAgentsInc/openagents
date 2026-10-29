#!/usr/bin/env python3
"""Whole-policy revalidation of the scalar source audit (#0af2).

Generated well-typed programs over the admitted scalar grammar and its nearest
refused neighbours go through the real audit tool. Every accepted program must
then satisfy two oracles that do not share code with the Coax policy:

1. Its emitted checked and release C, for the entry and every helper it
   reaches, contains no runtime guard, branch, memory access or helper whose
   behavior depends on a non-literal operand (the shapes of the earlier
   checked-arithmetic and checked-cast false acceptances).
2. Compiled checked and optimized, it returns the independently evaluated
   result for boundary and random secret inputs without trapping.

Seeded refused programs must fail oracle 1, so a vacuous oracle cannot pass.
This is source and emitted-C evidence for one compiler, not object timing,
optimizer, JIT or production-secret approval.
"""
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import sys
import tempfile
import unittest
from check import Audit, ROOT, COMPILER, run, metadata_sha

M64 = (1 << 64) - 1
M32 = (1 << 32) - 1
SEED = int(os.environ.get('COAX_SECRET_REVALIDATE_SEED', '20261029'))
COUNT = int(os.environ.get('COAX_SECRET_REVALIDATE_COUNT', '400'))
INPUTS = [(0, 0, False), (M64, M32, True), (1, 1, True), (1 << 63, 1 << 31, False), (M64 - 1, M32 - 1, True)]

class Trap(Exception):pass
def checked(op, p, q):
    # the checked development profile: overflow and division by zero trap
    if op in '/%' and q == 0:raise Trap(op)
    v = {'+': p + q, '-': p - q, '*': p * q, '/': p // max(q, 1), '%': p % max(q, 1)}[op]
    if v < 0 or v > M64:raise Trap(op)
    return v
def narrow(v):
    if v > M32:raise Trap('cast')
    return v

# ---- generator: expressions are (text, type, evaluator) over x:u64, y:u32, p:bool
class Gen:
    def __init__(self, rng):self.rng = rng
    def pick(self, *options):
        return self.rng.choice(options)
    def u64(self, d):
        r = self.rng
        if d <= 0 or r.random() < 0.18:
            return self.pick(('x', lambda e: e['x']), ('K', lambda e: 9), (f'(cast(u64){(c := r.choice([0, 1, 7, 255, M64]))})', lambda e, c=c: c))
        k = r.randrange(20)
        a = self.u64(d - 1)
        if k <= 2:
            b = self.u64(d - 1);op = '&|^'[k]
            f = {'&': lambda p, q: p & q, '|': lambda p, q: p | q, '^': lambda p, q: p ^ q}[op]
            return (f'({a[0]}{op}{b[0]})', lambda e, a=a, b=b, f=f: f(a[1](e), b[1](e)))
        if k <= 4:
            n = r.randrange(64);op = '<<' if k == 3 else '>>'
            f = (lambda v, n=n: (v << n) & M64) if k == 3 else (lambda v, n=n: v >> n)
            return (f'({a[0]}{op}{n})', lambda e, a=a, f=f: f(a[1](e)))
        if k == 5:  # a secret shift count (refused)
            b = self.u64(d - 1);return (f'({a[0]}<<({b[0]}^x))', lambda e: 0)
        if k <= 7:
            b = self.u64(d - 1);name = 'add_wrap' if k == 6 else 'mul_wrap'
            f = (lambda p, q: (p + q) & M64) if k == 6 else (lambda p, q: (p * q) & M64)
            return (f'{name}({a[0]},{b[0]})', lambda e, a=a, b=b, f=f: f(a[1](e), b[1](e)))
        if k <= 9:
            if r.random() < 0.7:n = r.randrange(130);b = (str(n), lambda e, n=n: n)
            else:b = self.u64(d - 1)
            name = 'rotl' if k == 8 else 'rotr'
            def rot(v, c, left=(k == 8)):
                c %= 64
                return ((v << c) | (v >> ((64 - c) % 64))) & M64 if left else ((v >> c) | (v << ((64 - c) % 64))) & M64
            return (f'{name}({a[0]},{b[0]})', lambda e, a=a, b=b, rot=rot: rot(a[1](e), b[1](e)))
        if k == 10:return (f'value_barrier({a[0]})', a[1])
        if k == 11:
            c = self.boolean(d - 1);b = self.u64(d - 1)
            return (f'(ifx {c[0]} then {a[0]} else {b[0]})', lambda e, a=a, b=b, c=c: a[1](e) if c[1](e) else b[1](e))
        if k == 12:
            v = self.u32(d - 1);return (f'(cast,trunc(u64){v[0]})', v[1])
        if k == 13:
            v = self.u32(d - 1);return (f'(cast(u64){v[0]})', v[1])
        if k == 14:
            b = self.u64(d - 1);op = self.pick('+', '-', '*', '/', '%')
            return (f'({a[0]}{op}{b[0]})', lambda e, a=a, b=b, op=op: checked(op, a[1](e), b[1](e)))
        if k == 15:
            b = self.u64(d - 1);return (f'mix({a[0]},{b[0]})', lambda e, a=a, b=b: (a[1](e) ^ ((b[1](e) << 1) & M64)))
        if k == 16:return (f'pick_bit({a[0]})', lambda e, a=a: a[1](e) & 1)
        if k == 17:return (f'(cast(u64){a[0]})', a[1])
        if k == 18:
            b = self.u64(d - 1);return (f'(cast(u64)(cast,trunc(u32){a[0]})^{b[0]})', lambda e, a=a, b=b: (a[1](e) & M32) ^ b[1](e))
        b = self.u64(d - 1);return (f'({a[0]}^{b[0]})', lambda e, a=a, b=b: a[1](e) ^ b[1](e))
    def u32(self, d):
        r = self.rng
        if d <= 0 or r.random() < 0.3:
            return self.pick(('y', lambda e: e['y']), (f'(cast(u32){(c := r.choice([0, 3, M32]))})', lambda e, c=c: c))
        k = r.randrange(4)
        if k == 0:
            a = self.u64(d - 1);return (f'(cast,trunc(u32){a[0]})', lambda e, a=a: a[1](e) & M32)
        if k == 1:
            a = self.u64(d - 1);return (f'(cast(u32){a[0]})', lambda e, a=a: narrow(a[1](e)))
        a = self.u32(d - 1);b = self.u32(d - 1);op = '&' if k == 2 else '^'
        return (f'({a[0]}{op}{b[0]})', lambda e, a=a, b=b, op=op: (a[1](e) & b[1](e)) if op == '&' else (a[1](e) ^ b[1](e)))
    def boolean(self, d):
        r = self.rng
        if d <= 0 or r.random() < 0.25:
            return self.pick(('p', lambda e: e['p']), ('true', lambda e: True), ('false', lambda e: False))
        k = r.randrange(6)
        if k == 0:
            a = self.boolean(d - 1);return (f'(!{a[0]})', lambda e, a=a: not a[1](e))
        if k <= 2:
            a = self.boolean(d - 1);b = self.boolean(d - 1);op = '&&' if k == 1 else '||'
            return (f'({a[0]}{op}{b[0]})', lambda e, a=a, b=b, k=k: (a[1](e) and b[1](e)) if k == 1 else (a[1](e) or b[1](e)))
        a = self.u64(d - 1);b = self.u64(d - 1);op = self.pick('==', '!=', '<', '>=')
        f = {'==': lambda p, q: p == q, '!=': lambda p, q: p != q, '<': lambda p, q: p < q, '>=': lambda p, q: p >= q}[op]
        return (f'({a[0]}{op}{b[0]})', lambda e, a=a, b=b, f=f: f(a[1](e), b[1](e)))

PRELUDE = ('K :: cast(u64)9;\n'
           'mix :: (a:u64,b:u64)->u64 {t:=b<<1;t^=a;return t;}\n'
           'pick_bit :: (a:u64)->u64 {return value_barrier(a)&1;}\n')
def program(expr, name='candidate', export=True):
    return f'{name} :: (x:u64,y:u32,p:bool)->u64{" #export_c" if export else ""} {{v:={expr};return v;}}\n'

# ---- oracle 1: emitted C of reached procedures carries no operand-dependent guard
CALL = re.compile(r'\b(coax_[A-Za-z0-9_]+)\s*\(')
SAFE = re.compile(r'^coax_(value_barrier|add_wrap|mul_wrap|rotl|rotr)_u(32|64)$')
LITERAL_SHIFT = re.compile(r'coax_sh[lr]_u(32|64)\(')
def bodies(c, names):
    out = {}
    for name in names:
        m = re.search(r'^[A-Za-z_][A-Za-z0-9_ ]*\b' + re.escape(name) + r'\([^;{]*\)\s*\{\n(.*?)^\}', c, re.S | re.M)
        if m:out[name] = m.group(1)
    return out
def shift_count_literal(body, start, width):
    # the count is the second argument: (int64_t)(UINT64_C(n)) with 0 <= n < width
    depth, i, comma = 0, start, None
    while i < len(body):
        ch = body[i]
        if ch == '(':depth += 1
        elif ch == ')':
            depth -= 1
            if depth == 0:break
        elif ch == ',' and depth == 1 and comma is None:comma = i
        i += 1
    if comma is None:return False
    rest = body[comma + 1:i]
    m = re.match(r'\s*\(int64_t\)\(UINT64_C\((\d+)\)\)\s*,\s*-?\d+\s*$', rest)
    return bool(m) and int(m.group(1)) < width
def left_operand(body, at):
    # the condition of a ternary or the left side of && / ||: back to the unmatched '('
    depth, i = 0, at - 1
    while i >= 0:
        ch = body[i]
        if ch == ')':depth += 1
        elif ch == '(':
            if depth == 0:break
            depth -= 1
        elif depth == 0 and (ch in ',;?' or body[i - 1:i + 1] in ('&&', '||')):break
        elif depth == 0 and ch == '=' and body[i - 1] not in '=!<>' and body[i + 1] != '=':break
        i -= 1
    return body[i + 1:at]
def call_arguments(body, open_at):
    depth, i = 0, open_at
    while i < len(body):
        if body[i] == '(':depth += 1
        elif body[i] == ')':
            depth -= 1
            if depth == 0:return body[open_at + 1:i]
        i += 1
    return body[open_at + 1:]
def secret_temporaries(body):
    # an emitter temporary is secret if any assignment to it reads a local,
    # parameter or secret temporary (fixpoint over every assignment)
    rhs = {}
    for m in re.finditer(r'\b(coax_t\d+)\s*=(?!=)', body):
        depth, i = 0, m.end()
        while i < len(body):
            ch = body[i]
            if ch == '(':depth += 1
            elif ch == ')':
                if depth == 0:break
                depth -= 1
            elif ch in ',;' and depth == 0:break
            i += 1
        rhs.setdefault(m.group(1), []).append(body[m.end():i])
    secret = set()
    while True:
        grown = {t for t, values in rhs.items() if t not in secret and any(re.search(r'\bl_[A-Za-z0-9_]+', v) or any(re.search(r'\b' + s + r'\b', v) for s in secret) for v in values)}
        if not grown:return secret
        secret |= grown
SECRET_TEMPS = set()
def public_operand(text):
    # literals, constants and helper calls on public values: no local, parameter or secret temporary
    if re.search(r'\bl_[A-Za-z0-9_]+', text):return False
    return not any(t in SECRET_TEMPS for t in re.findall(r'\bcoax_t\d+', text))
C_WORDS = {'if', 'while', 'for', 'switch', 'return', 'sizeof', 'uint64_t', 'uint32_t', 'int64_t', 'int32_t', 'UINT64_C', 'UINT32_C', 'INT64_C', 'INT32_C', '_Bool', 'bool'}
def guard_findings(c, entry='candidate'):
    # the entry and every helper it reaches through emitted direct calls
    found = bodies(c, [entry]);problems = []
    if entry not in found:return ['entry body not found']
    pending = [entry]
    while pending:
        calls = set(re.findall(r'\b([A-Za-z_][A-Za-z0-9_]*)\s*\(', found[pending.pop()]))
        for helper in sorted(n for n in calls if (n.startswith('coax_p_') or not n.startswith('coax_')) and n not in C_WORDS):
            if helper in found:continue
            more = bodies(c, [helper])
            if helper not in more:return [f'{helper}: reached helper body not found']
            found.update(more);pending.append(helper)
    global SECRET_TEMPS
    for name, body in found.items():
        SECRET_TEMPS = secret_temporaries(body)
        for word in ['if', 'while', 'for', 'switch', 'goto']:
            if re.search(r'\b' + word + r'\b', body):problems.append(f'{name}: statement {word}')
        if '[' in body:problems.append(f'{name}: memory index')
        for m in re.finditer(r'\?|&&|\|\|', body):
            if not public_operand(left_operand(body, m.start())):problems.append(f'{name}: operand-dependent {m.group(0)}')
        for m in CALL.finditer(body):
            fn = m.group(1)
            if fn.startswith('coax_p_') or fn.startswith('coax_t') or SAFE.match(fn):continue
            s = LITERAL_SHIFT.match(body, m.start())
            if s and shift_count_literal(body, m.end() - 1, int(s.group(1))):continue
            if public_operand(call_arguments(body, m.end() - 1)):continue  # a public guard may trap publicly
            problems.append(f'{name}: helper {fn}')
    return problems

class ScalarRevalidation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='coax-secret-revalidate-');cls.directory = Path(cls.temp.name);cls.tool = Audit()
        cls.record = {'schema': 'coax.secret-source-revalidation.v1', 'profile': 'unsigned-scalar-source-v1', 'seed': SEED, 'count': COUNT,
                      'compiler': cls.tool.compiler_version, 'compiler_sha256': cls.tool.compiler_fingerprint,
                      'policy_sha256': metadata_sha(ROOT / 'coax/lib/crypto/secret-flow.coax'),
                      'driver_sha256': metadata_sha(__file__), 'audit_artifact_sha256': metadata_sha(cls.tool.artifact),
                      'scope': 'source policy plus emitted checked/release C of reached procedures; not object timing, optimizer, JIT or production-secret approval'}
    @classmethod
    def tearDownClass(cls):
        Path(os.environ.get('COAX_SECRET_REVALIDATE_REPORT', '/tmp/coax-secret-revalidation.json')).write_text(json.dumps(cls.record, indent=2) + '\n')
        cls.tool.close();cls.temp.cleanup()
    def emitted(self, path, release):
        flags = ['--release'] if release else []
        return run(COMPILER, 'c', path, '--library', *flags).stdout
    def test_oracle_flags_every_known_false_acceptance_shape(self):
        # Sources the policy must refuse; their emitted C must also fail the
        # independent oracle, or the oracle could pass vacuously.
        controls = {
          'checked_add': 'candidate :: (x:u64)->u64 #export_c {return x+1;}',
          'checked_sub': 'candidate :: (x:u64)->u64 #export_c {return x-1;}',
          'checked_mul': 'candidate :: (x:u64)->u64 #export_c {return x*3;}',
          'division': 'candidate :: (x:u64)->u64 #export_c {return 7/x;}',
          'compound_add': 'candidate :: (x:u64)->u64 #export_c {a:=x;a+=x;return a;}',
          'narrow_cast': 'candidate :: (x:u64)->u32 #export_c {return cast(u32)x;}',
          'widening_checked_cast': 'candidate :: (x:u32)->u64 #export_c {return cast(u64)x;}',
          'secret_shift_count': 'candidate :: (x:u64,y:u64)->u64 #export_c {return x<<y;}',
          'secret_ifx': 'candidate :: (x:u64)->u64 #export_c {return ifx x==0 then 1 else 2;}',
          'secret_short_circuit': 'candidate :: (p:bool,q:bool)->bool #export_c {return p && q;}',
          'secret_index': 'candidate :: (a:[]u64,i:int)->u64 #export_c {return a[i];}',
          'secret_if': 'candidate :: (x:u64)->u64 #export_c {if x==0 {return 1;}return 2;}',
        }
        seen = {}
        for name, text in controls.items():
            path = self.directory / f'control-{name}.coax';path.write_text(text)
            report = self.tool.audit(path, 'candidate');self.assertFalse(report['accepted'], (name, report))
            for release in [False, True]:
                if name in ('checked_add', 'checked_sub', 'checked_mul', 'compound_add') and release:continue  # release wraps by definition
                problems = guard_findings(self.emitted(path, release))
                self.assertTrue(problems, (name, release, 'oracle missed a refused guard shape'))
            seen[name] = report['error_name']
        accepted = self.directory / 'control-accepted.coax'
        accepted.write_text('candidate :: (m:u64,a:u64,b:u64)->u64 #export_c {return b^((a^b)&value_barrier(m));}')
        self.assertTrue(self.tool.audit(accepted, 'candidate')['accepted'])
        for release in [False, True]:self.assertEqual(guard_findings(self.emitted(accepted, release)), [])
        self.record['oracle_controls'] = seen
    def test_actual_byte_helpers_under_the_corrected_policy(self):
        # cb_mask multiplies a secret by CB_MAX64 with checked multiplication.
        # The arithmetic correction refuses that conservatively; its emitted
        # checked C calls the overflow-checking multiply on the secret.
        c = run(COMPILER, 'c', ROOT / 'coax/lib/crypto/bytes.coax', '--library').stdout
        out = {}
        mask = self.tool.audit(ROOT / 'coax/lib/crypto/bytes.coax', 'cb_mask')
        self.assertFalse(mask['accepted'], mask);self.assertEqual(mask['error_name'], 'unsupported');line = 1 + [i for i, text in enumerate((ROOT / 'coax/lib/crypto/bytes.coax').read_text().splitlines()) if text.startswith('cb_mask ::')][0]
        self.assertEqual(mask['line'], line)
        self.assertIn('cb_mask: helper coax_mul_u64', guard_findings(c, 'cb_mask'))
        out['cb_mask'] = {'accepted': False, 'error': 'unsupported', 'line': line, 'emitted_checked_guard': 'coax_mul_u64 on a secret operand'}
        for entry in ['cb_ct_select32', 'cb_ct_select64']:
            report = self.tool.audit(ROOT / 'coax/lib/crypto/bytes.coax', entry);self.assertTrue(report['accepted'], report)
            for release in [False, True]:
                emitted = run(COMPILER, 'c', ROOT / 'coax/lib/crypto/bytes.coax', '--library', *(['--release'] if release else [])).stdout
                self.assertEqual(guard_findings(emitted, entry), [], (entry, release))
            out[entry] = {'accepted': True, 'emitted_checked_and_release_guards': []}
        self.record['actual_helpers'] = out
    def test_generated_programs(self):
        rng = random.Random(SEED);gen = Gen(rng);programs = [];reasons = {}
        for i in range(COUNT):
            expr = gen.u64(rng.randrange(1, 5))
            path = self.directory / f'g{i}.coax';path.write_text(PRELUDE + program(expr[0]))
            report = self.tool.audit(path, 'candidate')
            self.assertNotEqual(report['error'], 1, ('generator produced an ill-typed program', expr[0]))
            reasons[report['error_name']] = reasons.get(report['error_name'], 0) + 1
            if not report['accepted']:continue
            for release in [False, True]:
                problems = guard_findings(self.emitted(path, release))
                self.assertEqual(problems, [], ('accepted program has an operand-dependent guard', release, expr[0]))
            programs.append((i, expr))
        # non-vacuous in both directions
        self.assertGreaterEqual(len(programs), COUNT // 8, reasons);self.assertGreaterEqual(COUNT - len(programs), COUNT // 8, reasons)
        # oracle 2: execute every accepted program checked and optimized
        source = PRELUDE + ''.join(program(e[0], f'candidate_{i}') for i, e in programs)
        lib_path = self.directory / 'accepted.coax';lib_path.write_text(source)
        rng2 = random.Random(SEED + 1);inputs = INPUTS + [(rng2.getrandbits(64), rng2.getrandbits(32), bool(rng2.getrandbits(1))) for _ in range(27)]
        cases = [];public_traps = 0;running = []
        for i, e in programs:
            try:rows = [[i, x, y, p, e[1]({'x': x, 'y': y, 'p': p}) & M64] for x, y, p in inputs]
            except Trap:
                # an accepted program may trap only on public values, i.e. for every input alike
                public_traps += 1;continue
            cases.extend(rows);running.append((i, e))
        self.assertGreaterEqual(len(running), COUNT // 8)
        accepted_count = len(programs);programs = running
        cases_path = self.directory / 'cases.json';cases_path.write_text(json.dumps(cases))
        for mode, flags in [('checked', []), ('release', ['--release'])]:
            c = self.directory / f'accepted-{mode}.c';c.write_text(run(COMPILER, 'c', lib_path, '--library', *flags).stdout)
            so = self.directory / f'accepted-{mode}.so';run('cc', '-std=c11', '-O2', '-shared', '-fPIC', c, '-lm', '-o', so)
            driver = ('import ctypes as C,json,sys\nlib=C.CDLL(sys.argv[1]);bad=0\n'
                      'for i,x,y,p,want in json.load(open(sys.argv[2])):\n'
                      ' f=getattr(lib,"candidate_%d"%i);f.argtypes=[C.c_uint64,C.c_uint32,C.c_bool];f.restype=C.c_uint64\n'
                      ' if f(x,y,p)!=want:print("mismatch",i,x,y,p,f(x,y,p),want);bad+=1\n'
                      'sys.exit(1 if bad else 0)\n')
            result = subprocess.run([sys.executable, '-I', '-c', driver, str(so), str(cases_path)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, (mode, 'trap or mismatch', result.stdout[-2000:], result.stderr[-2000:]))
        self.record['generated'] = {'programs': COUNT, 'accepted': accepted_count, 'executed_programs': len(programs), 'refusals': reasons, 'executed_cases_per_mode': len(cases), 'accepted_public_trap_programs_not_executed': public_traps,
                                    'modes': ['checked -O2', 'release -O2'], 'oracle': 'Python big-integer evaluation; emitted-C guard scan',
                                    'accepted_sources_sha256': hashlib.sha256(source.encode()).hexdigest()}

if __name__ == '__main__':unittest.main()
