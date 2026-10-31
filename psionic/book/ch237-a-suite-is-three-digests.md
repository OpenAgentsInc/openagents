# 237. A suite is three digests

[Chapter 16](ch016-are-the-probabilities-any-good.md) scored Elephant on a held-out
suite of twelve items. A score like that means something only if a reader can
tell which suite it was. Questions get reworded, items get edited, gates get
retuned; a later run of "the same suite" is often not the same suite.

This chapter gives a suite three digests:
- the **items digest** covers every item field, each item's task and
  partition, and which partitions are locked;
- the **question digest** covers only the question wording;
- the **gate digest** covers every setting of the gate that judges a result,
  from [chapter 239](ch239-gates-that-can-say-unverifiable.md).

Results go into an append-only store. Each receipt names the suite's three
digests and the digest of the receipt before it, and a locked partition can be
read once. Two results compare only over the same items, gate and partition.

```shell
coax/bin/coax run psionic/examples/suite-digests.coax
psionic/check.sh --only suite
```

```jai
chapter 16's suite: items 7be5373b5066785a, questions 122febdeccba8aea, gate 11de5c6dc83dbd89
question 7 reworded: items 7be5373b5066785a, questions 934d5dd7629dbb37, gate 11de5c6dc83dbd89
item 7's state edited: items fa5040f8a407816b, questions 122febdeccba8aea, gate 11de5c6dc83dbd89
2000 replicates: items 7be5373b5066785a, questions 122febdeccba8aea, gate bfc097d32bfc4e0b
Elephant on the original suite, partition 0 (9 items): accuracy 1.0, receipt 0 5ae6f45398dfa09c
Elephant on the original suite, partition 1 (3 items): accuracy 1.0, receipt 1 aa6dee4d3c4e8ad5
Elephant on the original suite, partition 1 (3 items): refused, locked
first option on the original suite, partition 0 (9 items): accuracy 0.3333333333333333, receipt 2 6158224b6f82fc71
first option on the reworded suite, partition 0 (9 items): accuracy 0.3333333333333333, receipt 3 de52191f638cc841
first option on the edited suite, partition 0 (9 items): accuracy 0.3333333333333333, receipt 4 fba5dcd7fbe46c51
first option on the 2000 replicates suite, partition 0 (9 items): accuracy 0.3333333333333333, receipt 5 cba5169cff07786b
compare receipts 0 and 2: ok, same questions
compare receipts 2 and 3: ok, questions differ
compare receipts 2 and 4: items
compare receipts 2 and 5: gate
compare receipts 0 and 1: partition
the chain of 6 receipts: ok
receipt 1's accuracy changed to 0.5: broken, at receipt 1
and its digest recomputed: broken, at receipt 2
```

The [example](../examples/suite-digests.coax) embeds chapter 16's suite and
runs instantly. Its digests are shown to 16 hex digits.

## The encoding

Every digest is SHA-256, from Coax's own `sha256.coax`, over a fixed byte
encoding:

<!-- source: psionic/lib/suite.coax encode -->
```jai
su_zero :: ()->[32]u8 {z:[32]u8;return z;}
su_same :: (a:[32]u8,b:[32]u8)->bool {for i:0..<32 {if a[i]!=b[i] {return false;}}return true;}
// Little-endian u64s, and texts as a u64 length then their bytes.
su_u64 :: (buf:[]u8,at:int,x:u64)->int {for i:0..<8 {buf[at+i]=cast(u8)((x>>(i*8))&255);}return at+8;}
su_text :: (buf:[]u8,at:int,s:string)->int {p:=su_u64(buf,at,cast(u64)s.count);for i:0..<s.count {buf[p+i]=s[i];}return p+s.count;}
su_flag :: (buf:[]u8,at:int,b:bool)->int {buf[at]=ifx b then 1 else 0;return at+1;}
su_hash :: (bytes:[]u8)->[32]u8 {out:[32]u8;status:=sha256_hash_checked(bytes,out[..],false);#assert status==SHA_OK;return out;}
```

Numbers are little-endian u64s; floats are their bit patterns. A text is its
length and then its bytes, so `"ab","c"` and `"a","bc"` can't encode the same.
Each digest starts with its own tag, such as `psionic.suite.items.v1`, so an
items digest can never equal a question digest of other bytes.

## A suite

<!-- source: psionic/lib/suite.coax suite -->
```jai
// The items digest covers every item field, its task and partition, and the
// set of locked partitions; the question digest covers only the questions;
// the gate digest covers every gate setting.
su_suite :: (items:[]SuItem,questions:[]string,gate:SuGate,locked:u64)->SuSuite #export_c {
    n:=items.count;
    if n<1 || n>SU_ITEMS_MAX || questions.count!=n {return SuSuite.{status=SuStatus.shape};}
    for item:items {if !su_item_ok(item) {return SuSuite.{status=SuStatus.shape};}}
    for q:questions {if !su_text_ok(q,0,SU_QUESTION_MAX) {return SuSuite.{status=SuStatus.shape};}}
    g:=su_gate_ok(gate);if g!=SuStatus.ok {return SuSuite.{status=g};}
    s:=SuSuite.{mask=locked};buf:[SU_BYTES]u8;
    at:=su_text(buf[..],0,"psionic.suite.items.v1");at=su_u64(buf[..],at,locked);at=su_u64(buf[..],at,cast(u64)n);
    for item:items {
        at=su_text(buf[..],at,item.id);buf[at]=item.kind;at+=1;
        at=su_u64(buf[..],at,cast(u64)item.task);at=su_u64(buf[..],at,cast(u64)item.partition);
        at=su_text(buf[..],at,item.state);at=su_u64(buf[..],at,cast(u64)item.options);
        for k:0..<item.options {
            o:=item.option[k];at=su_flag(buf[..],at,o.named);at=su_text(buf[..],at,o.name);
            at=su_flag(buf[..],at,o.described);at=su_text(buf[..],at,o.description);
        }
        at=su_u64(buf[..],at,cast(u64)item.gold);at=su_u64(buf[..],at,cast(u64)(item.abstain+1));
    }
    s.items=su_hash(buf[0..<at]);
    at=su_text(buf[..],0,"psionic.suite.questions.v1");at=su_u64(buf[..],at,cast(u64)n);
    for q:questions {at=su_text(buf[..],at,q);}
    s.questions=su_hash(buf[0..<at]);
    at=su_text(buf[..],0,"psionic.suite.gate.v1");buf[at]=gate.kind;at+=1;
    at=su_u64(buf[..],at,cast(u64)gate.clear);at=su_u64(buf[..],at,cast(u64)gate.min_tasks);
    at=su_u64(buf[..],at,cast(u64)gate.replicates);at=su_u64(buf[..],at,gate.seed);
    at=su_u64(buf[..],at,f64_bits(gate.cost_limit));at=su_u64(buf[..],at,f64_bits(gate.seconds_limit));
    s.gate=su_hash(buf[0..<at]);
    return s;
}
```

The suite is checked before it is hashed:
- one to 64 items, with one question each;
- ids of 1 to 64 bytes, states up to 512, options up to 64, questions up to
  256;
- tasks below 1,024 and partitions below 64;
- a yes-or-no item has no options and a gold label of 0 or 1; the others have
  2 to 8 options, and the gold label and the "I don't know" option, if any,
  must name one of them;
- a gate with sensible counts, and limits from 0 to 10³⁰⁰.

Anything else is refused with `shape` or `number`, and no digest is written.

Each edit in the example moves exactly one digest:

| Edit | Items | Questions | Gate |
| --- | --- | --- | --- |
| none | 7be5373b… | 122febde… | 11de5c6d… |
| question 7 reworded | same | 934d5dd7… | same |
| item 7's state edited | fa5040f8… | same | same |
| 2,000 replicates instead of 1,000 | same | same | bfc097d3… |

Rewording is the case to watch. Chapter 16's question 7 asked whether the shop
is open at 20:00; asking "8 pm" instead tests the same fact. A run on the
reworded suite is still comparable, and the comparison says the questions
differ. That is how wording sensitivity gets measured, instead of being hidden
inside a "new version" of the suite.

## The store

<!-- source: psionic/lib/suite.coax store -->
```jai
su_receipt_digest :: (r:SuReceipt)->[32]u8 #export_c {
    buf:[512]u8;at:=su_text(buf[..],0,"psionic.suite.receipt.v1");at=su_u64(buf[..],at,cast(u64)r.sequence);
    for i:0..<32 {buf[at+i]=r.previous[i];buf[at+32+i]=r.suite.items[i];buf[at+64+i]=r.suite.questions[i];buf[at+96+i]=r.suite.gate[i];}
    at+=128;at=su_u64(buf[..],at,r.suite.mask);at=su_u64(buf[..],at,cast(u64)r.partition);
    at=su_u64(buf[..],at,r.system);at=su_u64(buf[..],at,cast(u64)r.read);at=su_u64(buf[..],at,f64_bits(r.score));
    for i:0..<32 {buf[at+i]=r.values[i];}
    return su_hash(buf[0..<at+32]);
}
// Append at count: the receipt takes the next sequence number and the previous
// receipt's digest. A locked partition of a suite's items can be read once.
su_append :: (store:[]SuReceipt,count:int,r:SuReceipt)->SuAppend #export_c {
    if count<0 || count>store.count || r.suite.status!=SuStatus.ok || r.read<1 {return SuAppend.{status=SuStatus.shape};}
    if r.partition<0 || r.partition>=SU_PARTITIONS {return SuAppend.{status=SuStatus.shape};}
    if !(r.score>=-1.0e300 && r.score<=1.0e300) {return SuAppend.{status=SuStatus.number};}
    if count==store.count {return SuAppend.{status=SuStatus.capacity};}
    if (r.suite.mask>>r.partition)&1==1 {
        for old:store[0..<count] {
            if su_same(old.suite.items,r.suite.items) && old.partition==r.partition {return SuAppend.{status=SuStatus.locked};}
        }
    }
    e:=r;e.sequence=count;e.previous=ifx count==0 then su_zero() else store[count-1].digest;
    e.digest=su_receipt_digest(e);store[count]=e;
    return SuAppend.{count=count+1};
}
// The first receipt whose sequence, link or digest does not hold.
su_verify :: (store:[]SuReceipt,count:int)->SuCheck #export_c {
    if count<0 || count>store.count {return SuCheck.{status=SuStatus.shape};}
    for r,i:store[0..<count] {
        previous:=ifx i==0 then su_zero() else store[i-1].digest;
        if r.sequence!=i || !su_same(r.previous,previous) || !su_same(r.digest,su_receipt_digest(r)) {return SuCheck.{status=SuStatus.broken,index=i};}
    }
    return SuCheck.{};
}
```

A receipt records:
- its sequence number and the previous receipt's digest;
- the suite's three digests and its locked partitions;
- the partition read, the system, the number of items read and the score;
- a digest of the per-item values, so a score can be checked against them.

Append fills in the sequence and the link, so a caller can't choose them.
Verify walks the chain and names the first receipt whose sequence, link or
digest doesn't hold.

In the example, chapter 16's last item of each kind forms the locked partition
1. Elephant reads it once and scores 3 of 3. A second read of the same items'
partition 1 is refused with `locked`, even by the same system with the same
answers: a held-out set that can be read again is a training set.

Then the example changes receipt 1's accuracy to 0.5. Verify breaks at receipt
1, because its digest no longer matches. Recomputing that digest moves the
break to receipt 2, which still names receipt 1's old digest. To hide one edit,
every later receipt would have to be rewritten. A reader who kept any later
digest would see it.

## Comparing two results

<!-- source: psionic/lib/suite.coax compare -->
```jai
// Two results compare only over the same items, gate and partition; the
// questions may differ, which is what rewording measures.
su_compare :: (a:SuReceipt,b:SuReceipt)->SuComparison #export_c {
    if a.suite.status!=SuStatus.ok || b.suite.status!=SuStatus.ok {return SuComparison.{status=SuStatus.shape};}
    if !su_same(a.suite.items,b.suite.items) || a.suite.mask!=b.suite.mask {return SuComparison.{status=SuStatus.items};}
    if !su_same(a.suite.gate,b.suite.gate) {return SuComparison.{status=SuStatus.gate};}
    if a.partition!=b.partition {return SuComparison.{status=SuStatus.partition};}
    return SuComparison.{questions_same=su_same(a.suite.questions,b.suite.questions)};
}
```

| Receipts | Result | Why |
| --- | --- | --- |
| 0 and 2 | ok, same questions | Elephant and the first-option baseline on the same suite |
| 2 and 3 | ok, questions differ | the reworded suite: same items and gate |
| 2 and 4 | `items` | item 7's state was edited |
| 2 and 5 | `gate` | the gate draws 2,000 replicates |
| 0 and 1 | `partition` | the open and the locked partition |

Items are checked first, then the gate, then the partition. The first-option
baseline is right on 3 of 9 open items; Elephant on all 9, and chapter 239's
gate still calls that unverifiable for lack of headroom.

## Supported profile

`sha256-suite-v1`:
- SHA-256 over the encoding above, with one tag per digest;
- up to 64 items, 64 partitions, 1,024 tasks and a mask of locked partitions;
- items of kind choice (0), yes-or-no (1) and score (2), as in chapter 16;
- a store of receipts held by the caller, appended at a count.

`su_append` writes one store and reads only values, so the native adapter only
names a native array as a store.

## How it is checked

- **Coax checks:** 46 checks in debug and release, counting the SHA-256 checks it loads. They cover:
  - every digest of a small suite and a receipt, pinned to Python's hashlib;
  - that each edit moves only its own digest, including a flipped option
    flag, an abstain option and the locked mask;
  - every bound on items, texts, tasks, partitions, options and the gate;
  - appends: sequence and link, capacity, a locked partition read once, an
    open partition read twice, and NaN and infinite scores;
  - verify on tampered sequence, link, digest and score;
  - every comparison status, in order.
- **An independent oracle,** [suite_oracle.py](../tests/suite_oracle.py),
  writes the encoding again with Python's hashlib. It reads chapter 16's suite
  from [heldout-v1.json](../fixtures/quality/heldout-v1.json), not from the
  example's copy.
- **The native harness** [suite.c](../tests/suite.c) builds the suite from the
  oracle's copy, through the adapter, plain and under ASan/UBSan, in debug and
  release. Every digest, receipt, score, refusal and comparison must be equal;
  NaN and infinity are refused in scores and limits.
- **Mutation testing** of `suite.coax` kills all 1,076 mutants. The first run
  left 27 survivors, each fixed in the checks: a pinned receipt digest,
  mismatch cases for the digest helpers, bounds at each limit, a locked first
  receipt, and an open partition above the locked one.

The [verification record](../../docs/verification/psionic-ch178-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **No signatures.** The chain shows that a receipt changed, not who wrote
  it. Signing the last digest is a later step.
- **One store, held by the caller.** Nothing here stops a caller from keeping
  two stores and showing the better one; publishing digests does.
- **Locked means once per items digest.** Editing one item makes a new suite
  whose locked partition can be read again. A held-out set should be replaced,
  not edited.
- **No verdicts in the receipt.** Chapter 239's gates read scores; storing
  their verdicts next to the receipt is left to the harness.
