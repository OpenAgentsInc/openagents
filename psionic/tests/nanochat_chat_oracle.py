#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent nanochat conversation rendering and byte accounting.

Reads only committed data: the bpe-train source record (rules, vectors) and
the learned PST5 table. Its split uses Python's re with Unicode 17 letter and
number classes built from unicodedata, and tiktoken-style lowest-rank merging,
so it shares no code with psionic/lib/bpe-train.coax. It first reproduces all
retained regex/ID vectors, then writes psionic/fixtures/nanochat/*.coax.
The render rules are read from the pinned tokenizer.py (render_conversation,
SPECIAL_TOKENS); upstream code is never imported or run.
  python3 -I psionic/tests/nanochat_chat_oracle.py           # check
  python3 -I psionic/tests/nanochat_chat_oracle.py --write   # regenerate
"""
import argparse, hashlib, json, math, re, sys, unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BPE = ROOT / 'psionic/fixtures/bpe-train'
OUT = ROOT / 'psionic/fixtures/nanochat'
SPECIALS = ['<|bos|>', '<|user_start|>', '<|user_end|>', '<|assistant_start|>', '<|assistant_end|>',
            '<|python_start|>', '<|python_end|>', '<|output_start|>', '<|output_end|>']
assert unicodedata.unidata_version == '17.0.0'


def ranges(pred):
    out, start = [], None
    for cp in range(0x110000):
        ok = pred(cp)
        if ok and start is None:
            start = cp
        if not ok and start is not None:
            out.append((start, cp - 1)); start = None
    if start is not None:
        out.append((start, 0x10ffff))
    esc = lambda c: '\\U%08x' % c
    return ''.join(esc(a) if a == b else esc(a) + '-' + esc(b) for a, b in out)


L = ranges(lambda c: unicodedata.category(chr(c)).startswith('L'))
N = ranges(lambda c: unicodedata.category(chr(c)).startswith('N'))
# Unicode White_Space (PropList.txt), not str.isspace: U+001C..U+001F are not spaces.
WS = '\\t\\n\\x0b\\x0c\\r \\x85\\xa0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000'
PATTERN = re.compile(
    "'(?i:[sdmt]|ll|ve|re)"
    f"|[^\\r\\n{L}{N}]?+[{L}]+"
    f"|[{N}]{{1,2}}"
    f"| ?[^{WS}{L}{N}]++[\\r\\n]*"
    f"|[{WS}]*[\\r\\n]"
    f"|[{WS}]+(?![^{WS}])"
    f"|[{WS}]+")


def vocabulary(rules):
    vocab = [bytes([i]) for i in range(256)]
    for r in rules:
        vocab.append(vocab[r['left']] + vocab[r['right']])
    return vocab


def encode(text, ranks):
    ids = []
    for piece in PATTERN.findall(text):
        parts = [bytes([b]) for b in piece.encode()]
        while len(parts) > 1:
            best = None
            for i in range(len(parts) - 1):
                r = ranks.get(parts[i] + parts[i + 1])
                if r is not None and (best is None or r < best[0]):
                    best = (r, i)
            if best is None:
                break
            i = best[1]
            parts[i:i + 2] = [parts[i] + parts[i + 1]]
        ids += [ranks[p] for p in parts]
    return ids


def render(conv, enc, base, max_tokens):
    sp = {name: base + i for i, name in enumerate(SPECIALS)}
    msgs = [dict(m) for m in conv]
    if msgs[0]['role'] == 'system':
        assert msgs[1]['role'] == 'user'
        msgs[1]['content'] = msgs[0]['content'] + '\n\n' + msgs[1]['content']
        msgs = msgs[1:]
    ids, mask = [], []
    def add(xs, m):
        ids.extend(xs); mask.extend([m] * len(xs))
    add([sp['<|bos|>']], 0)
    for i, m in enumerate(msgs):
        assert m['role'] == ('user' if i % 2 == 0 else 'assistant')
        c = m['content']
        if m['role'] == 'user':
            add([sp['<|user_start|>']], 0); add(enc(c), 0); add([sp['<|user_end|>']], 0)
        else:
            add([sp['<|assistant_start|>']], 0)
            if isinstance(c, str):
                add(enc(c), 1)
            else:
                for part in c:
                    v = enc(part['text'])
                    if part['type'] == 'text':
                        add(v, 1)
                    elif part['type'] == 'python':
                        add([sp['<|python_start|>']], 1); add(v, 1); add([sp['<|python_end|>']], 1)
                    else:
                        add([sp['<|output_start|>']], 0); add(v, 0); add([sp['<|output_end|>']], 0)
            add([sp['<|assistant_end|>']], 1)
    return ids[:max_tokens], mask[:max_tokens], len(ids)


# (name, conversation, max_tokens): tool boundaries, system merge, Unicode,
# two-digit numbers, special-looking text that stays ordinary, truncation.
CASES = [
    ('one user turn', [dict(role='user', content='Hello, world!')], 2048),
    ('system merges into the first user turn', [dict(role='system', content='Be brief.'), dict(role='user', content='12 + 345 = ?'),
                                                 dict(role='assistant', content='357')], 2048),
    ('assistant text, python call and unsupervised output', [dict(role='user', content='What is 6789 * 2?'),
        dict(role='assistant', content=[dict(type='text', text='Let me compute.'), dict(type='python', text='6789*2'),
                                        dict(type='python_output', text='13578'), dict(type='text', text=' It is 13578.')])], 2048),
    ('two exchanges with Unicode', [dict(role='user', content='é é ́a 😀'), dict(role='assistant', content='क ख १२३४५'),
                                    dict(role='user', content="We're I'LL"), dict(role='assistant', content='')], 2048),
    ('special-token text is ordinary bytes', [dict(role='user', content='[MASK] <|bos|> <|user_end|>'),
                                              dict(role='assistant', content=[])], 2048),
    ('truncation keeps the first max_tokens', [dict(role='user', content='The quick brown fox 123456'),
                                               dict(role='assistant', content='jumps over 78 lazy dogs.')], 12),
]
DOCUMENTS = ['', 'Hello, world!', '١٢٣٤٥ ⅣⅤⅥ 99', '\r\n \t\n x']


def coax_str(s):
    esc = {'\\': '\\\\', '"': '\\"', '\n': '\\n', '\t': '\\t', '\r': '\\r'}
    assert all(ord(c) >= 32 or c in esc for c in s)
    return '"' + ''.join(esc.get(c, c) for c in s) + '"'


def build():
    ref = json.loads((BPE / 'reference.json').read_text())
    rules = ref['model']['rules']
    vocab = vocabulary(rules)
    ranks = {b: i for i, b in enumerate(vocab)}
    for v in ref['vectors']:
        got = encode(v['text'], ranks)
        if got != v['ids']:
            sys.exit(f'nanochat oracle: split/merge disagrees with the retained vector {v["text"]!r}: {got} != {v["ids"]}')
    base = len(vocab)
    enc = lambda t: encode(t, ranks)
    table = (BPE / 'learned.pst').read_bytes()
    assert hashlib.sha256(table).hexdigest() == ref['packed_sha256']
    out = ['// Generated by psionic/tests/nanochat_chat_oracle.py from the bpe-train record; do not edit.',
           f'NC_BASE :: {base};']
    rows = []
    for n, (name, conv, limit) in enumerate(CASES):
        ids, mask, total = render(conv, enc, base, limit)
        msgs, parts = [], []
        for m in conv:
            c = m['content']
            if isinstance(c, str):
                msgs.append(f'NcMessage.{{role=NcRole.{m["role"]},text={coax_str(c)}}}')
            else:
                first = len(parts)
                for p in c:
                    kind = {'text': 'text', 'python': 'python', 'python_output': 'output'}[p['type']]
                    parts.append(f'NcPart.{{kind=NcPartKind.{kind},text={coax_str(p["text"])}}}')
                msgs.append(f'NcMessage.{{role=NcRole.{m["role"]},list=true,first={first},parts={len(c)}}}')
        rows.append(dict(name=name, msgs=msgs, parts=parts, limit=limit, ids=ids, mask=mask, total=total))
    width = max(len(r['ids']) for r in rows)
    out.append(f'NC_CASES :: {len(rows)};')
    for n, r in enumerate(rows):
        parts = r['parts'] or ['NcPart.{}']
        out.append(f'#test "independent nanochat render {n}: {r["name"]}" {{')
        out.append(f'    messages:=[{len(r["msgs"])}]NcMessage.[{",".join(r["msgs"])}];parts:=[{len(parts)}]NcPart.[{",".join(parts)}];')
        out.append(f'    want:=[{max(1, len(r["ids"]))}]int.[{",".join(map(str, r["ids"])) or "0"}];mask:=[{max(1, len(r["mask"]))}]u8.[{",".join(map(str, r["mask"])) or "0"}];')
        out.append(f'    ids:[2048]int;got:[2048]u8;table:=NC_LEARNED;specials:[9]int;for i:0..<9 {{specials[i]=NC_BASE+i;}}')
        out.append(f'    r:=nc_render(table[..],specials[..],32768,messages[..],parts[..],{r["limit"]},ids[..],got[..],false);')
        out.append(f'    #assert r.status==NcStatus.ok && r.written=={len(r["ids"])} && r.total=={r["total"]};')
        out.append(f'    for i:0..<r.written {{#assert ids[i]==want[i] && got[i]==mask[i];}}')
        out.append('}')
    for n, text in enumerate(DOCUMENTS):
        ids = [base] + enc(text)
        out.append(f'#test "independent nanochat document {n} begins with BOS" {{')
        out.append(f'    table:=NC_LEARNED;want:=[{len(ids)}]int.[{",".join(map(str, ids))}];ids:[64]int;')
        out.append(f'    r:=nc_document(table[..],{coax_str(text)},ids[..],false);#assert r.status==NcStatus.ok && r.written=={len(ids)};')
        out.append(f'    for i:0..<r.written {{#assert ids[i]==want[i];}}')
        out.append('}')
    # Byte accounting: nanochat's evaluate_bpb over targets, ignore index -1, specials count 0 bytes.
    targets = [base, 72, -1, 300, base + 4, 101, 256, base + 8]
    losses = [0.5, 1.25, 9.0, 2.0, 0.75, 0.125, 3.5, 4.0]
    nb = [0 if t < 0 or t >= base else len(vocab[t]) for t in targets]
    nats = sum(l for l, b in zip(losses, nb) if b > 0); total = sum(nb)
    out.append('#test "independent bits per byte: ignored targets and special tokens carry no bytes" {')
    out.append(f'    table:=NC_LEARNED;targets:=[{len(targets)}]int.[{",".join(map(str, targets))}];losses:=[{len(losses)}]f64.[{",".join(repr(x) for x in losses)}];')
    out.append(f'    r:=nc_bpb(table[..],targets[..],losses[..]);#assert r.status==NcStatus.ok && r.bytes=={total} && r.nats=={nats!r};')
    out.append(f'    #assert abs(r.bpb-{nats / (math.log(2) * total)!r})<0.000000000001;')
    for t, b in zip(targets, nb):
        if t >= 0:
            out.append(f'    #assert nc_token_bytes(table[..],{t})=={b};')
    out.append('}')
    goldens = '\n'.join(out) + '\n'
    learned = '// Generated by psionic/tests/nanochat_chat_oracle.py: psionic/fixtures/bpe-train/learned.pst as data.\n' \
              f'NC_LEARNED :: [{len(table)}]u8.[' + ','.join(map(str, table)) + '];\n'
    return goldens, learned, len(ref['vectors']), len(CASES), len(DOCUMENTS)


def main():
    p = argparse.ArgumentParser(); p.add_argument('--write', action='store_true'); a = p.parse_args()
    goldens, learned, vectors, cases, docs = build()
    files = {OUT / 'chat-goldens.coax': goldens, OUT / 'learned.coax': learned}
    for path, text in files.items():
        if a.write:
            path.write_text(text)
        elif not path.is_file() or path.read_text() != text:
            sys.exit(f'nanochat oracle: {path.relative_to(ROOT)} differs; regenerate with --write')
    print(f'nanochat oracle: {vectors} retained split/ID vectors reproduced by an independent re/unicodedata split; '
          f'{cases} conversations, {docs} BOS documents and bits-per-byte accounting {"written" if a.write else "verify"}')


main()
