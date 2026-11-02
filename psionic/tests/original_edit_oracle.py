#!/usr/bin/env python3
# Firm reason: 2 (independent original-logit parity, BF16 rounding, window identity and evaluation oracle).
"""Audit chapter255's inert original-model artifacts without model execution."""
import ast
import gzip
import hashlib
import json
import math
import re
import importlib.util
import bisect
from pathlib import Path
import struct
import sys
import zipfile
import csv

ROOT = Path(__file__).resolve().parents[2]
F = ROOT / 'psionic/fixtures/original-edit'


def raw(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(path.suffix+'.gz').read_bytes())


def floats(path):
    data = raw(path); return struct.unpack('<'+'f'*(len(data)//4), data)


def npy(data):
    assert data[:6] == b'\x93NUMPY' and data[6] == 1
    size = struct.unpack_from('<H', data, 8)[0]; head = ast.literal_eval(data[10:10+size].decode())
    assert head['descr'] == '<f4' and not head['fortran_order']
    result = struct.unpack('<'+'f'*math.prod(head['shape']), data[10+size:]); return result


def parity():
    with zipfile.ZipFile(ROOT / 'psionic/fixtures/gptoss/original/20b/qualification/reference.npz') as archive:
        for i in range(2):
            ref = npy(archive.read(f'{i}_logits.npy')); got = floats(F / f'parity-{i}.f32')
            assert len(got) == len(ref) == 201088 and all(math.isfinite(x) for x in got)
            err = max(abs(x-y)/(1+abs(y)) for x, y in zip(got, ref))
            assert err < .0005 and max(range(len(got)), key=got.__getitem__) == max(range(len(ref)), key=ref.__getitem__)
            print(f'original prefix {i}: {len(got)} logits, max scaled error {err:.9g}, exact argmax {max(range(len(got)), key=got.__getitem__)}')


def bf16(data):
    return [struct.unpack('<f', b'\0\0'+data[i:i+2])[0] for i in range(0, len(data), 2)]


def round_bf(v):
    bits = struct.unpack('<I', struct.pack('<f', v))[0]
    return struct.pack('<H', (bits+32767+((bits >> 16) & 1)) >> 16)


def identity(meta, data):
    b = bytearray()
    def word(n): b.extend(struct.pack('<Q', n))
    def text(s): word(len(s)); b.extend(s.encode())
    text('original-window-v1'); b.extend(bytes.fromhex(meta['model'])); b.extend(bytes.fromhex(meta['shard'])); text(meta['name'])
    for key in ['rows', 'cols', 'first', 'count', 'dtype']: word(meta[key])
    b.extend(data); return hashlib.sha256(b).hexdigest()


def tasks():
    text = (ROOT / 'psionic/lib/original-study.coax').read_text()
    rows = re.findall(r'OsCase\.\{prompt="([^"]+)",answer="([^"]+)"([^}]*)\}', text)
    assert len(rows) == 54
    for prompt, answer, _ in rows:
        expected = None
        m = re.fullmatch(r'What is (\d+) ([*+/-]) (\d+)\? Reply with the integer only\.', prompt)
        if m:
            a, op, b = m.groups(); a, b = int(a), int(b)
            expected = a*b if op == '*' else a+b if op == '+' else a-b if op == '-' else a//b
        m = re.fullmatch(r'What is (\d+) to the power (\d+)\? Reply with the integer only\.', prompt)
        if m: expected = int(m[1])**int(m[2])
        if '37 squared' in prompt: expected = 37**2
        if 'sum of integers 1 through 20' in prompt: expected = sum(range(1,21))
        if expected is not None: assert str(expected) == answer, (prompt, answer, expected)
    return rows


def match(text, answer, version=2):
    if version == 2: text = text.translate(str.maketrans('₀₁₂₃₄₅₆₇₈₉', '0123456789'))
    return re.sub('[^a-z0-9-]', '', text.lower()) == answer


def repetitions(ids):
    return max([0]+[n for n in range(3,17) for i in range(len(ids)-3*n+1) if ids[i:i+n] == ids[i+n:i+2*n] == ids[i+2*n:i+3*n]])


def rng(seed, counter):
    mask = 2**64-1; x = (seed+(counter+1)*0x9e3779b97f4a7c15) & mask
    x = ((x^(x>>30))*0xbf58476d1ce4e5b9) & mask; x = ((x^(x>>27))*0x94d049bb133111eb) & mask
    return x^(x>>31)


def direction():
    d = [0.]*2880
    for i in range(36,42):
        h = floats(F / f'channel/{i}.f32'); norm = math.sqrt(sum(x*x for x in h))
        for k, x in enumerate(h): d[k] += x/norm/6
    den = 16*math.sqrt(sum(x*x for x in d)); return [x/den for x in d]


def number(s): return struct.unpack('>d', bytes.fromhex(s))[0]


def parent_identity():
    source = (ROOT / 'psionic/fixtures/gptoss/original/targets.coax').read_text().split('G120_ARTIFACTS')[0]
    pins = re.findall(r'GArtifact\.\{name="([^"]+)",bytes=(\d+),sha="([a-f0-9]+)"', source)
    assert len(pins) == 3
    table_sha = re.search(r'HS_TABLE_SHA :: "([a-f0-9]+)"', (ROOT / 'psionic/fixtures/harmony/pins.coax').read_text())[1]
    assert hashlib.sha256(raw(ROOT / 'psionic/fixtures/harmony/gptoss.hst')).hexdigest() == table_sha
    b = bytearray()
    def text(s): b.extend(struct.pack('<Q', len(s))); b.extend(s.encode())
    for s in ['original-edit-model-v1', 'openai/gpt-oss-20b@6cee5e81ee83917806bbde320786a8fb61efebee', 'gpt-oss-metal-f32-ff-v1', table_sha]: text(s)
    for name, size, sha in pins: text(name); b.extend(struct.pack('<Q', int(size))); text(sha)
    return hashlib.sha256(b).hexdigest(), pins[2][2]


def candidates(rows, d):
    result = {}; model, shard = parent_identity()
    for path in sorted((F / 'candidates').glob('*.json')):
        if path.name.endswith('.carry.json'): continue
        meta = json.loads(path.read_text()); id = meta['id']
        assert (meta['model'], meta['shard']) == (model, shard)
        if not meta['valid']:
            assert id >= 13; result[id] = None; continue
        alpha = number(meta['alpha_hex']); target = []
        for r in range(2):
            encoded = b''.join(round_bf(meta['scales'][r]*v+(-alpha if r == 0 else alpha)*d[k]) for k, v in enumerate(rows[r]))
            if 9 <= id <= 11:
                values = [encoded[k:k+2] for k in range(0,len(encoded),2)]; groups = {}
                source = raw(F / f'original-row-{r}.bf16')
                for k in range(2880): groups.setdefault(source[2*k:2*k+2], []).append(k)
                for k in range(2880):
                    group = groups[source[2*k:2*k+2]]; at = bisect.bisect_left(group, k); j = group[at+rng(255+id-9,k) % (len(group)-at)]
                    values[k], values[j] = values[j], values[k]
                encoded = b''.join(values)
            got = raw(F / f'candidates/{id}-{r}.bf16'); assert got == encoded, id
            w = dict(meta['windows'][r], model=meta['model'], shard=meta['shard'])
            source = raw(F / f'original-row-{r}.bf16')
            assert identity(w, source) == w['base'] and identity(w, encoded) == w['digest']
            assert w['changed'] == sum(source[k:k+2] != encoded[k:k+2] for k in range(0,len(source),2))
            target.append(bf16(encoded))
        data = struct.pack('<Q', len('original-overlay-v1'))+b'original-overlay-v1'+bytes.fromhex(meta['model'])+b''.join(bytes.fromhex(w['digest']) for w in meta['windows'])
        assert hashlib.sha256(data).hexdigest() == meta['digest']
        delta = [x-y for r in range(2) for x,y in zip(target[r], rows[r])]
        assert sum(w['changed'] for w in meta['windows']) == meta['changed']
        assert abs(sum(x*x for x in delta)-number(meta['norm2_hex'])) < 1e-13
        result[id] = (meta, target, delta)
    if len(result) > 11:
        for id in [9,10,11]:
            assert result[id][0]['changed'] == result[8][0]['changed']
            assert sorted(x*x for x in result[id][2]) == sorted(x*x for x in result[8][2])
    print(f'candidate artifacts: {len(result)} records, independently reconstructed encoded weights and identities')
    return result


def tokenizer():
    spec = importlib.util.spec_from_file_location('harmony_independent', ROOT / 'psionic/tests/harmony_oracle.py')
    mod = importlib.util.module_from_spec(spec); spec.loader.exec_module(mod); oracle=mod.original(); oracle.helper=mod; return oracle


def audit_runs(data, oracle):
    reverse = {v:k for k,v in oracle.ranks.items()}
    special = ['<|startoftext|>','<|endoftext|>','<|reserved_200000|>','<|reserved_200001|>','<|return|>','<|constrain|>','<|reserved_200004|>','<|channel|>','<|start|>','<|end|>','<|message|>','<|reserved_200009|>','<|reserved_200010|>','<|reserved_200011|>','<|call|>']
    def decode(ids): return b''.join(reverse[x] if x<199998 else (special[x-199998] if x<=200012 else f'<|reserved_{x}|>').encode() for x in ids).decode()
    result = {}; total = 0; emitted = 0; prefix_tokens = 0
    helper=oracle.helper
    prefixes=[helper.render(oracle,[helper.message(0,helper.system(0)),helper.message(2,prompt)],True) for prompt,_,_ in data]
    for path in (F / 'runs').glob('*.json'):
        r = json.loads(path.read_text()); id, case = r['variant'], r['case']; ids = r['generated']
        assert int(path.stem)==id*100+case and type(r['milliseconds']) is int and r['milliseconds']>=0 and r['prefix_tokens']>0
        if 'candidate_digest' in r:
            assert r['candidate_digest']==json.loads((F / f'candidates/{id}.json').read_text())['digest']
        assert 1 <= len(ids) <= (128 if id == 19 else 64) and all(type(x) is int and 0<=x<201088 for x in ids)
        try: text = decode(ids)
        except UnicodeDecodeError: assert r['raw_status'] != 0; text = ''
        if r['raw_status'] == 0: assert text == r['raw']
        assert repetitions(ids) == r['repeat_span']
        prefix = decode(r['prompt_ids']) if 'prompt_ids' in r else '<|start|>assistant'
        if 'prompt_ids' in r:
            assert len(r['prompt_ids']) == r['prefix_tokens'] and f'<|start|>user<|message|>{data[case][0]}<|end|>' in prefix
            assert r['prompt_ids']==prefixes[case]+([200005,17196,200008] if id==2 else [])
        else: assert r['prefix_tokens']==len(prefixes[case])
        full = prefix+text
        finals = re.findall(r'<\|start\|>assistant<\|channel\|>final<\|message\|>(.*?)<\|return\|>', full, re.S)
        complete = bool(finals) and ids[-1] == 200002 and r['parse'] == 0
        assert complete == r['complete']
        if complete: assert finals[-1] == r['final']
        else: assert r['final'] == ''
        correct = complete and match(r['final'],data[case][1])
        if correct and data[case][1].isdigit():
            # Punctuation stripping is a frozen worksheet scorer, not a
            # general numeric parser. Reject observed decimal/fraction
            # collisions rather than silently counting them as arithmetic.
            integer = r['final'].strip(' \t\r\n*`_.')
            assert re.fullmatch(r'[-+]?\d+|[-+]?\d{1,3}(,\d{3})+',integer), (id,case,r['final'])
            assert int(integer.replace(',',''))==int(data[case][1])
        assert r['correct'] == (complete and match(r['final'],data[case][1],2 if r.get('scorer') else 1))
        result[id,case] = dict(r, rescored=correct); total += 1; emitted += len(ids); prefix_tokens += r['prefix_tokens']
    for case in range(54):
        if (3,case) in result: assert result[3,case]['generated'] == result[0,case]['generated']
    if (16,36) in result: assert result[16,36]['generated'] == result[0,36]['generated']
    for case in range(54):
        if (19,case) in result:
            base = result[0,case]['generated']; longer = result[19,case]['generated']
            assert longer[:len(base)]==base
            if result[0,case]['complete']: assert longer==base
    for id in [6,7,9,10,11]:
        if all((id,case) in result and (2,case) in result for case in range(54)):
            assert all(result[id,case]['generated'][3:]==result[2,case]['generated'] for case in range(54))
    print(f'generations: {total} saved sequences, {emitted} emitted tokens, {prefix_tokens} prompt tokens; framing, exact scoring and repetition agree')
    return result


def reference(case, data):
    return {6:'Paris',8:'Tokyo',10:'CAT',11:'H2O',24:'Rome',25:'Berlin',26:'Madrid',27:'Ottawa',32:'DOG',33:'BLUE',34:'Au'}.get(case, data[case][1])


def group(runs, id, held, primary, data):
    selected = [runs[id,i] for i in range(54) if ('held=true' in data[i][2]) == held and (i>=36) == primary]
    return dict(total=len(selected), complete=sum(r['complete'] for r in selected), correct=sum(r['rescored'] for r in selected),
                tokens=sum(len(r['generated']) for r in selected),
                limits=sum(not r['complete'] and len(r['generated'])==r.get('budget',64) and r['generated'][-1] not in [200002,200012] for r in selected),
                repetitions=sum(r['repeat_span']>0 for r in selected), parse=sum(r['parse']!=0 for r in selected))


def carry_and_selection(cs, runs, data):
    for id in [13,14,15]:
        receipt = json.loads((F / f'candidates/{id}.carry.json').read_text()); source = receipt['source']
        assert source == 0 and receipt['contract'] == 'same-weights-greedy-request-v1'
        assert receipt['digest'] == cs[id][0]['digest'] == cs[source][0]['digest']
        assert not any(k[0]==id for k in runs)
        for case in range(54): runs[id,case] = runs[source,case]
    selection = json.loads((F / 'selection.json').read_text())
    eligible = [id for id in range(3,16) if cs[id] is not None and cs[id][0]['changed']]
    def key(id):
        p, r = group(runs,id,False,True,data), group(runs,id,False,False,data)
        return -p['correct'], -r['correct'], p['tokens']+r['tokens'], id
    winner = min(eligible,key=key); p, r = group(runs,winner,False,True,data), group(runs,winner,False,False,data)
    assert selection == dict(winner=winner,digest=cs[winner][0]['digest'],primary_correct=p['correct'],retained_correct=r['correct'],tokens=p['tokens']+r['tokens'])
    quality = json.loads((F / 'quality.json').read_text())
    p, r = group(runs,winner,True,True,data), group(runs,winner,True,False,data)
    bp, br = group(runs,0,True,True,data), group(runs,0,True,False,data)
    assert quality == dict(candidate=winner,finite_goal=p['correct']>=bp['correct']+2 and r['correct']>=br['correct'],verdict='unverifiable',reason='no_floor',baseline_blocks=0)
    return winner


def fit(d):
    terms = []
    for case in range(36,42):
        h, logits = floats(F / f'channel/{case}.f32'), floats(F / f'channel/{case}.logits.f32')
        terms.append((sum(x*y for x,y in zip(h,d)), logits[35644]-logits[17196]))
    alpha = m = v = 0.
    for step in range(1,201):
        grad = sum(-2*t/(1+math.exp(-(z-2*alpha*t)))/6 for t,z in terms)
        m = .9*m+.1*grad; v = .99*v+.01*grad*grad
        alpha = max(-4,min(4,alpha-.05*(m/(1-.9**step))/(math.sqrt(v/(1-.99**step))+1e-8)))
    saved = struct.unpack('<d', raw(F / 'fit-alpha.f64'))[0]
    assert abs(alpha-saved)<1e-12
    print(f'channel-only AdamW: independent 200-step fit agrees, alpha {saved:.16g}')


def tables(cs, runs, data, oracle, rows):
    # Cache the independent fixed-prefix loss once per distinct overlay/case.
    memo = {}; vectors = {}
    for case in range(54): vectors[case] = floats(F / f'answer/{case}.logits.f32'), floats(F / f'answer/{case}.f32'), oracle.encode(reference(case,data))[0]
    def loss(id,case):
        digest = cs[id][0]['digest']; key = digest,case
        if key not in memo:
            original, h, answer = vectors[case]; logits = list(original)
            for r,token in enumerate([35644,17196]): logits[token] += sum((x-y)*z for x,y,z in zip(cs[id][1][r],rows[r],h))
            top = max(range(201088),key=logits.__getitem__); den = math.fsum(math.exp(x-logits[top]) for x in logits)
            memo[key] = logits[top]+math.log(den)-logits[answer], int(top!=answer and 1/den>.9)
        return memo[key]
    for table in ['matrix','supplement']:
        records = list(csv.DictReader((F / f'{table}.csv').open()))
        assert len(records) == (64 if table=='matrix' else 16)
        for row in records:
            id,held,primary = int(row['id']),row['held']=='true',row['target']=='true'
            expected = group(runs,id,held,primary,data)
            for k,v in expected.items(): assert int(row[k]) == v, (id,held,primary,k,row[k],v)
            if table=='matrix':
                assert row['valid']=='true' and int(row['changed'])==cs[id][0]['changed']
                cases = [i for i in range(54) if ('held=true' in data[i][2])==held and (i>=36)==primary]
                values = [loss(id,i) for i in cases]
                assert abs(float(row['forced_final_first_token_nll'])-math.fsum(x[0] for x in values)/len(values))<1e-9
                assert int(row['confident_wrong_first'])==sum(x[1] for x in values)
    print('matrix/supplement CSV: all generation metrics and full-vocabulary first-token NLL/confident-wrong proxies agree independently')


def usage():
    totals = dict(input_tokens=0,cache_creation_input_tokens=0,cache_read_input_tokens=0,output_tokens=0)
    cost = ms = thinking = 0
    for id in [13,14,15]:
        response = json.loads((F / f'llm/{id}.response.json').read_text()); request = json.loads((F / f'llm/{id}.request.json').read_text()); timing = json.loads((F / f'llm/{id}.timing.json').read_text())
        assert response['subtype']=='success' and not response['is_error'] and response['result']=='[1,1,0]'
        assert request['tools']==[] and request['timeout_ms']==100000 and request['max_budget_usd']==1
        assert request['input']==(F / f'llm/{id}.prompt.txt').read_text()
        assert response['subagent_stats']['spawned']==0 and response['num_turns']==1
        assert response['usage']['server_tool_use']==dict(web_search_requests=0,web_fetch_requests=0)
        assert timing['status']==0 and not timing['cut_or_timeout']
        for key in totals: totals[key]+=response['usage'][key]
        thinking += response['usage']['output_tokens_details']['thinking_tokens']; cost+=response['total_cost_usd']; ms+=timing['milliseconds']
    print(f'proposer usage: {totals}, including {thinking} reported thinking output tokens; list USD {cost:.6f}, client ms {ms}')


def ledger(runs):
    assert len(runs)==865 and set(k[0] for k in runs)==set(range(13))|{16,17,18,19}
    emitted = sum(len(r['generated']) for r in runs.values()); prompts = sum(r['prefix_tokens'] for r in runs.values())
    extra = 3+13*runs[0,36]['prefix_tokens']+sum(2*runs[0,i]['prefix_tokens']+5 for i in range(54))+sum(runs[0,i]['prefix_tokens'] for i in list(range(12))+list(range(36,42)))
    expected = dict(sequences=865,emitted_tokens=emitted,prompt_tokens=prompts,generation_milliseconds=sum(r['milliseconds'] for r in runs.values()),carried_comparisons=162,
                    extra_prefill_calls=141,extra_prefill_tokens=extra,extra_single_steps=85,all_prefill_calls=1006,all_single_steps=emitted-865+85)
    got = {r['key']:int(r['value']) for r in csv.DictReader((F / 'ledger.csv').open())}
    assert expected==got
    print(f'logical driver calls: {1006} prefills ({prompts+extra} token positions), {emitted-865+85} single steps; per-generation ms {expected["generation_milliseconds"]}')


def source_fixtures(cs,runs,data):
    text = (F / 'rows.coax').read_text()
    for name,key in [('OE_MODEL','model'),('OE_SHARD','shard')]:
        values = bytes(map(int,re.search(name+r' :: \[32\]u8\.\[([0-9,]+)\]',text)[1].split(',')))
        assert values.hex()==cs[0][0][key]
    rows = re.findall(r'\[5760\]u8\.\[([0-9,]+)\]',text); assert len(rows)==2
    for r,values in enumerate(rows): assert bytes(map(int,values.split(',')))==raw(F / f'original-row-{r}.bf16')
    text = (F / 'results.coax').read_text(); vectors = re.findall(r'\[12\]int\.\[([0-9,]+)\]',text); assert len(vectors)==16
    for id,values in enumerate(vectors):
        expected=[]
        for held,primary in [(False,True),(False,False),(True,True),(True,False)]:
            g=group(runs,id,held,primary,data); expected.extend(g[k] for k in ['total','correct','tokens'])
        assert list(map(int,values.split(',')))==expected
    assert 'OE_QUALITY :: "unverifiable (no_floor)";' in text
    print('public Coax fixture rows and recorded result constants match the raw artifact ledger')


def snapshots():
    for variant,manifest in [('baseline','matrix'),('held','held')]:
        text = (F / f'builds/{manifest}.sha256').read_text()
        for source in ['run','data']:
            digest = re.search(r'([a-f0-9]{64})  psionic/tools/original-edit-'+source+r'\.coax',text)[1]
            assert hashlib.sha256((F / f'builds/{variant}-{source}.coax.txt').read_bytes()).hexdigest()==digest
    print('historical baseline and held-out model-driver snapshots match their observed build manifests')
    for line in (F / 'checks/authority.sha256').read_text().splitlines():
        digest, path = line.split('  ')
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest()==digest
    for name,n in [('window',123),('proposal',45)]:
        assert f'{n} mutants, {n} killed, 0 survived' in (F / f'checks/{name}-final-mutation.log').read_text()


def verify():
    parity()
    rows = [bf16(raw(F / f'original-row-{r}.bf16')) for r in range(2)]
    assert all(len(row) == 2880 and all(math.isfinite(v) for v in row) for row in rows)
    data = tasks()
    if (F / 'channel/36.f32').exists():
        d = direction(); cs = candidates(rows, d); runs = audit_runs(data, tokenizer())
        for path in (F / 'equivalence').glob('*.json'):
            id = int(path.stem); item = json.loads(path.read_text()); assert item['candidate'] == cs[id][0]['digest'] and item['same_hidden']
            h = floats(F / 'channel/36.f32'); base = floats(F / 'channel/36.logits.f32')
            for r, token in enumerate([35644,17196]):
                expected = base[token]+sum((x-y)*z for x,y,z in zip(cs[id][1][r], rows[r], h))
                got = number(item['rows'][r]['got_hex']); assert abs(expected-got)/(1+abs(expected)) < .0005
                assert abs(expected-number(item['rows'][r]['expected_hex'])) < 1e-10
        print('permanent GPU edits and independent activation/output interventions agree under the frozen bound')
        fit(d); usage(); snapshots()
        if (F / 'quality.json').exists():
            ledger(runs)
            winner = carry_and_selection(cs,runs,data)
            source_fixtures(cs,runs,data)
            if (F / 'matrix.csv').exists(): tables(cs,runs,data,tokenizer(),rows)
            print(f'frozen selection {winner}, explicit no-op carries and failed finite goal/independent qualification verified')


if __name__ == '__main__':
    if '--window-vector' in sys.argv:
        m = dict(model='00'*32, shard='00'*32, name='head', rows=8, cols=2, first=4, count=4, dtype=0)
        print(identity(m, bytes([128,63,0,64,0,0,0,128])))
    else: verify()
