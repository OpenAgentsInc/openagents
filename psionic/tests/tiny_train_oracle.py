#!/usr/bin/env python3
"""Independent reference for chapter 115's data and baseline.

Reads the sentences from psionic/fixtures/tiny-train/corpus.coax and, written
separately from psionic/lib/transformer/train.coax, folds them to 15 symbols,
groups rows by chapter 120's rules (shared task or session, or bigram Jaccard
of at least 1/2), splits by time, builds the BOS-led streams and scores the
add-one bigram on the held-out stream. Prints the values the Coax checks pin.
Tooling only; nothing here runs in the library.
"""
import math
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
LETTERS = "etaoinshrdlcu"
BOS = 15
TRAIN_END, VALID_END = 37, 46


def sentences():
    text = (ROOT / "psionic/fixtures/tiny-train/corpus.coax").read_text()
    return re.findall(r'^    "([^"]*)"', text, re.M)


def fold(s):
    out, prev = [], 0
    for ch in s.lower():
        t = (LETTERS.index(ch) + 1 if ch in LETTERS else 14) if "a" <= ch <= "z" else 0
        if t != 0 or prev != 0:
            out.append(t)
        prev = t
    return out


def bigrams(doc):
    pairs, prev = set(), BOS
    for t in doc:
        pairs.add((prev, t))
        prev = t
    return pairs


def main():
    docs = [fold(s) for s in sentences()]
    n = len(docs)
    label = list(range(n))
    for i in range(n):
        for j in range(i + 1, n):
            x, y = bigrams(docs[i]), bigrams(docs[j])
            if 2 * len(x & y) >= len(x | y) and label[i] != label[j]:
                keep, gone = min(label[i], label[j]), max(label[i], label[j])
                label = [keep if l == gone else l for l in label]
    latest = {}
    for i in range(n):
        latest[label[i]] = max(latest.get(label[i], -1), i)
    split = ["train" if latest[label[i]] <= TRAIN_END else "validation" if latest[label[i]] <= VALID_END else "test" for i in range(n)]
    stream = {k: [] for k in ("train", "validation", "test")}
    for d, k in zip(docs, split):
        stream[k] += [BOS] + d
    train, held = stream["train"], stream["validation"]
    counts = [[0] * 16 for _ in range(16)]
    for a, b in zip(train, train[1:]):
        counts[a][b] += 1
    loss = -sum(math.log((counts[a][b] + 1) / (sum(counts[a]) + 16)) for a, b in zip(held, held[1:])) / (len(held) - 1)
    print(f"rows {n}, symbols {sum(map(len, docs))}, groups {len(set(label))}")
    joined = {g: [i for i in range(n) if label[i] == g] for g in sorted(set(label))}
    print("groups of more than one row:", [rows for rows in joined.values() if len(rows) > 1])
    print(f"splits train {split.count('train')} validation {split.count('validation')} test {split.count('test')}")
    print(f"streams train {len(train)} held-out {len(held)}")
    print(f"first row {docs[0][:12]} count {len(docs[0])}, last row count {len(docs[-1])}")
    print(f"counts t->h {counts[2][8]} of row t {sum(counts[2])}; h->e {counts[8][1]} of row h {sum(counts[8])}")
    print(f"TT_ORACLE_BIGRAM :: {loss!r};")


if __name__ == "__main__":
    main()
