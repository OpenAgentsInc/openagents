#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent text-to-text generation for chapter 126 (psionic/lib/nanochat/engine.coax).

Tooling only; needs a Python whose unicodedata is 17.0.0. It composes three
other oracles, none of which shares code with Coax: the conversation renderer
and tokenizer of nanochat_chat_oracle.py, the binary64 nanochat model of
nanochat_oracle.py (the checkpoint case, with the export's F32 and
round-to-nearest BF16 weights), and the sampler rules of
next_token_oracle.py. Generation follows the pinned nanochat/engine.py
(92d63d4e), read and never executed: render_for_completion drops the final
assistant message and appends <|assistant_start|>; the last position's logits
pick the next token; <|assistant_end|> or <|bos|> ends the reply and is not
part of it. Every position runs through the oracle's own one-token-at-a-time
path, never its full pass, so the Coax prefill-then-decode is compared with a
different evaluation order. The logits reach the sampler as binary32, and
each step draws one counter of chapter 53's splitmix64 generator (torch's
multinomial stream is not reproduced).
  python3 -I psionic/tests/nanochat_engine_oracle.py           # check the fixture
  python3 -I psionic/tests/nanochat_engine_oracle.py --write   # regenerate it
"""
import argparse
import importlib.util
import math
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'psionic/fixtures/nanochat/engine-goldens.coax'


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'psionic/tests' / f'{name}.py')
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gpt = load('nanochat_oracle')
chat = load('nanochat_chat_oracle')
nxt = load('next_token_oracle')

BOS, USER_START, USER_END, ASSISTANT_START, ASSISTANT_END, PYTHON_START, PYTHON_END = range(7)


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def pick(p, u):
    """Chapter 53's rule: the first ID whose running mass exceeds u, else the last positive one."""
    run = 0.0
    for i, v in enumerate(p):
        run += v
        if u < run:
            return i
    return max(i for i, v in enumerate(p) if v > 0)


class Session:
    """Engine.generate for one row: one token at a time through every layer."""

    def __init__(self, model):
        self.m = model
        c = model.c
        self.kc = [[] for _ in range(c['n_layer'])]
        self.vc = [[] for _ in range(c['n_layer'])]
        self.prev = None
        self.pos = 0

    def step(self, tok):
        m, c, p = self.m, self.m.c, self.m.p
        xs, raw = m.embed([tok], self.prev)
        self.prev = raw[0]
        x, xb = xs[0], None
        x0 = list(x)
        resid, x0l = p['resid_lambdas'][1], p['x0_lambdas'][1]
        for i in range(c['n_layer']):
            x = [resid[i] * a + x0l[i] * b for a, b in zip(x, x0)]
            q, k, v = m.project(i, gpt.rms(x), tok, self.pos)
            self.kc[i].append(k)
            self.vc[i].append(v)
            keys, values, w = self.kc[i], self.vc[i], m.win[i]
            if w < len(keys):
                keys, values = keys[len(keys) - (w + 1):], values[len(keys) - (w + 1):]
            x = m.finish(i, x, m.attend(q, keys, values))
            if i == c['n_layer'] // 2:
                xb = list(x)
        self.pos += 1
        return m.head(x, xb)


def generate(model, base, prompt, temperature, top_k, top_p, seed, limit, context):
    s = Session(model)
    for t in prompt:
        z = s.step(t)
    last = list(z)
    out, counter, python = [], 0, False
    while True:
        logits = [f32(v) for v in z]
        dist = nxt.distribution(logits, None, temperature, top_k, top_p)
        tok = pick(dist, nxt.uniform(seed, counter))
        counter += 1
        if tok == base + ASSISTANT_END:
            return last, out, 'assistant_end'
        if tok == base + BOS:
            return last, out, 'bos'
        out.append(tok)
        if tok == base + PYTHON_START:
            python = True
        elif tok == base + PYTHON_END and python:
            return last, out, 'tool'
        if len(out) == limit:
            return last, out, 'limit'
        if s.pos == context:
            return last, out, 'context'
        z = s.step(tok)


def text(vocab, base, ids):
    b = b''
    for t in ids:
        b += vocab[t] if t < base else chat.SPECIALS[t - base].encode()
    return b


# (name, conversation, temperature, top_k, top_p, seed, limit). Every
# conversation ends with the assistant message that render_for_completion drops.
CASES = [
    ('greedy reply to one user turn', [dict(role='user', content='Hello, world!'), dict(role='assistant', content='')],
     0.0, 0, 1.0, 0, 12),
    ('seeded reply, top-k 40', [dict(role='user', content='Hello, world!'), dict(role='assistant', content='')],
     1.0, 40, 1.0, 126, 12),
    ('system turn merged, temperature 0.8, top-p 0.9',
     [dict(role='system', content='Be brief.'), dict(role='user', content='12 + 345 = ?'), dict(role='assistant', content='357')],
     0.8, 0, 0.9, 53, 12),
    ('a second exchange fills the context',
     [dict(role='user', content='Name 12 animals.'), dict(role='assistant', content='Cats, dogs.'),
      dict(role='user', content='More?'), dict(role='assistant', content='')],
     1.5, 0, 1.0, 9, 40),
]
# A hot sampler ends a reply by itself: the first seed from 1 whose reply stops
# on <|assistant_end|> or <|bos|> within 24 tokens.
STOP_CASE = ('a sampled stop token ends the reply', [dict(role='user', content='Hi'), dict(role='assistant', content='')],
             4.0, 0, 1.0, 24)
CONTEXT = 64


def coax_case(n, name, conv, prompt, gen, stop, body, opts, last):
    t, k, p, seed, limit = opts
    msgs = ','.join(f'NcMessage.{{role=NcRole.{m["role"]},text={chat.coax_str(m["content"])}}}' for m in conv)
    out = [f'#test "independent generation {n}: {name}" {{',
           f'    messages:=[{len(conv)}]NcMessage.[{msgs}];',
           f'    prompt:=[{len(prompt)}]int.[{",".join(map(str, prompt))}];',
           f'    want:=[{max(1, len(gen))}]int.[{",".join(map(str, gen)) or "0"}];',
           f'    bytes:=[{max(1, len(body))}]u8.[{",".join(map(str, body)) or "0"}];',
           f'    o:=NxOptions.{{temperature={t!r},top_k={k},top_p={p!r}}};',
           f'    #assert ne_golden(messages[..],prompt[..],want[0..<{len(gen)}],bytes[0..<{len(body)}],o,{seed},{limit},NeStop.{stop});']
    if last is not None:
        # 13 decimals (|logit| <= 15): a last-bit libm difference between hosts
        # does not change the file, and the check's tolerance is 1e-12.
        out.append(f'    last:=[{len(last)}]f64.[{",".join(f"{v:.13f}" for v in last)}];#assert ne_golden_logits(prompt[..],last[..]);')
    out.append('}')
    return out


def build():
    import json
    ref = json.loads((ROOT / 'psionic/fixtures/bpe-train/reference.json').read_text())
    vocab = chat.vocabulary(ref['model']['rules'])
    ranks = {b: i for i, b in enumerate(vocab)}
    base = len(vocab)
    enc = lambda s: chat.encode(s, ranks)
    model = gpt.Model(gpt.CASES['checkpoint'])
    assert model.c['vocab_size'] == base + 9 and model.c['sequence_len'] == CONTEXT
    lines = ['// Generated by psionic/tests/nanochat_engine_oracle.py; do not edit.']
    # Every 97th weight in state_dict order, as the export stores it (F32, BF16 round-to-nearest-even).
    flat = [v for name, _, _ in gpt.inventory(model.c) for v in model.p[name][1]]
    samples = flat[::97]
    lines.append(f'NE_WEIGHT_COUNT :: {len(flat)};')
    lines.append(f'NE_WEIGHT_SAMPLES :: [{len(samples)}]f64.[{",".join(repr(v) for v in samples)}];')
    summary = []
    for n, (name, conv, t, k, p, seed, limit) in enumerate(CASES):
        assert conv[-1]['role'] == 'assistant'
        ids, _, total = chat.render(conv[:-1], enc, base, 2048)
        assert len(ids) == total
        prompt = ids + [base + ASSISTANT_START]
        last, gen, stop = generate(model, base, prompt, t, k, p, seed, limit, CONTEXT)
        body = text(vocab, base, gen)
        lines += coax_case(n, name, conv, prompt, gen, stop, body, (t, k, p, seed, limit), last if n == 0 else None)
        summary.append(f'{n}: {len(prompt)} prompt + {len(gen)} generated, {stop}')
    name, conv, t, k, p, limit = STOP_CASE
    prompt = chat.render(conv[:-1], enc, base, 2048)[0] + [base + ASSISTANT_START]
    for seed in range(1, 400):
        last, gen, stop = generate(model, base, prompt, t, k, p, seed, limit, CONTEXT)
        if stop in ('assistant_end', 'bos'):
            break
    else:
        sys.exit('nanochat engine oracle: no seed below 400 stops by itself')
    n = len(CASES)
    lines += coax_case(n, name, conv, prompt, gen, stop, text(vocab, base, gen), (t, k, p, seed, limit), None)
    summary.append(f'{n}: seed {seed}, {len(prompt)} prompt + {len(gen)} generated, {stop}')
    return '\n'.join(lines) + '\n', summary


def main():
    a = argparse.ArgumentParser()
    a.add_argument('--write', action='store_true')
    o = a.parse_args()
    text_out, summary = build()
    if o.write:
        OUT.write_text(text_out)
    elif not OUT.is_file() or OUT.read_text() != text_out:
        sys.exit(f'nanochat engine oracle: {OUT.relative_to(ROOT)} differs; regenerate with --write')
    print('nanochat engine oracle: ' + '; '.join(summary) + (' (written)' if o.write else ' (verify)'))


if __name__ == '__main__':
    main()
