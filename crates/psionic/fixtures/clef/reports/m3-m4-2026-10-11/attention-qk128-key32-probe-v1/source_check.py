"""Check the exact preserved update body without compiling or running kernels."""
from pathlib import Path
import ast,hashlib,json,re
D=Path(__file__).resolve().parent
W=Path('<WORKTREE>')
a=(D/'baseline.metal').read_text();b=(D/'candidate.metal').read_text();m=json.loads((D/'manifest.json').read_text())
assert hashlib.sha256(a.encode()).hexdigest()==m['production_sha256']
assert (W/m['production_source']).read_bytes()==a.encode()
def body(s):
 start=s.index('// Eight adjacent lanes')
 end=s.index('threadgroup_barrier(mem_flags::mem_threadgroup);',s.index('value_op.run(probability_tile, values, output_tile);',start))
 return re.sub(r'\s+','',s[start:end])
assert body(a)==body(b.replace('scores[row * kScoreKeys + score_part + key]','scores[row * kKeys + key]'))
for part in ['kRowLanes = kThreads / kQueries','kKeys = 32','kQueries = 16','kThreads = 128','kQueries, kDim, kKeys, false, false, false, matmul2d_descriptor::mode::multiply_accumulate']:
 assert part in a and part in b
assert 'kScoreKeys = 128' in b
assert '16 * keys * 4 + 16 * 32 * 2 + 16 * dim * 4' in (D/'probe.swift').read_text()
assert (D/'score_diagnostics.swift').read_text() in (D/'probe.swift').read_text()
for p in D.glob('*.py'):ast.parse(p.read_text())
assert m['expected_check_commands']==68+3*4 and m['expected_bench_commands']==128+3*4
assert m['expected_check_comparisons']==144+3*4 and m['expected_bench_comparisons']==204+3*4
assert m['declared_threadgroup_bytes']==[16*32*4+16*32*2+16*256*4,16*128*4+16*32*2+16*256*4]
print('Source check passed: exact32-key update expressions/PV descriptor, original baseline, counts, scratch sizes, embedded diagnostics and Python AST.')
