# 77. Learn a tokenizer

A byte alphabet can represent every input byte, but common sequences deserve
shorter encodings. This chapter trains a bounded BPE vocabulary entirely in
Coax, serializes it, reopens it, and uses chapter 5's ranked BPE core to encode
held-out text. The profile is `byte-bpe-nanochat2-u17-v1`.

The starting IDs are the bytes 0–255. Each selected pair creates ID 256+rank.
Pairs never cross pretokenized pieces or document boundaries. Count overlapping
adjacent pairs, select the highest count, and break ties by the lowest
`(left ID, right ID)`. Replace disjoint occurrences from left to right, then
recount. For `aaaaa`, the pair `aa` has count four but only two disjoint
replacements. Stop when the requested number of merges is reached or no pair
has count at least two. The model records both counts for auditing.

## Pin the splitting rule

[nanochat's tokenizer source](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/nanochat/tokenizer.py)
uses two-digit number groups. We preserve that exact regex's alternatives,
with Unicode 17 property DATA already pinned by chapter 5. Letters exclude
combining marks, unlike chapter 5's Qwen splitting profile. There is no NFC or
other normalization: decomposed accents round-trip with their original bytes.
Contractions, optional word prefixes, punctuation/newline runs and trailing
whitespace retain the pinned pattern's behavior. Special-token spellings are
ordinary text; this trainer does not create or recognize control IDs.

<!-- source: psionic/lib/bpe-train.coax split -->
```jai
bt_piece :: (table:[]u8,t:PtLayout,s:string,at:int)->int {
 end:=pt_contraction(s,at);if end>at {return end;}r:=pt_rune(s,at);
 if pt_has(table,t,r.cp,4) {end=r.next;if end==s.count {return end;}next:=pt_rune(s,end);if pt_has(table,t,next.cp,4) {end=next.next;}return end;}
 pos:=at;if r.cp!=13 && r.cp!=10 && !pt_has(table,t,r.cp,5) {pos=r.next;}end=pt_run(table,t,s,pos,1,true);if end>pos {return end;}
 pos=at;if r.cp==32 {pos=r.next;}end=pt_run(table,t,s,pos,13,false);
 if end>pos {while true {if end==s.count {return end;}c:=pt_rune(s,end);if c.cp!=13 && c.cp!=10 {return end;}end=c.next;}return end;}
 space:=at;last_newline:=at;last_space:=at;while true {if space==s.count {break;}c:=pt_rune(s,space);if !pt_has(table,t,c.cp,8) {break;}last_space=space;space=c.next;if c.cp==13 || c.cp==10 {last_newline=space;}}
 if last_newline>at {return last_newline;}if space==s.count {return space;}if last_space>at {return last_space;}return space;
}
```

The exact regex, revision, source hash and source license are retained in
[reference.json](../fixtures/bpe-train/reference.json), compressed original
Python DATA and [LICENSE](../fixtures/bpe-train/LICENSE). The capture tool
extracts the string with an AST literal reader. It never imports or executes
downloaded source. Installed `regex 2026.9.29` supplies independent Unicode
splitting expectations; the normal gate needs only Python's standard library.

## Train the pairs

The training call accepts 1–16 documents totaling at most 4,096 UTF-8 bytes and
0–64 requested merges. Empty documents are allowed. Invalid UTF-8 refuses.
Every merged token must fit 64 bytes; an overgrown winning pair refuses the
whole model rather than silently choosing another pair. The maximum vocabulary
is 320 IDs. The returned value owns all rules and reports source bytes,
pretokenized pieces and remaining training symbols.

<!-- source: psionic/lib/bpe-train.coax train -->
```jai
bt_train :: (table:[]u8,documents:[]string,requested:int)->BtModel #export_c {
 if !bt_docs(documents.count) || !bt_merges(requested) {return BtModel.{status=PtStatus.size};}t:=pt_layout(table);if t.status!=PtStatus.ok {return BtModel.{status=t.status};}total:=0;for text:documents {if !bt_bytes(text.count) || !bt_bytes(total+text.count) {return BtModel.{status=PtStatus.size};}total+=text.count;status:=pt_utf8_validate(text);if status!=PtStatus.ok {return BtModel.{status=status};}}
 symbols:[4096]int;groups:[4096]int;n:=0;pieces:=0;for text:documents {at:=0;while at<text.count {end:=bt_piece(table,t,text,at);for i:at..<end {symbols[n]=cast(int)text[i];groups[n]=pieces;n+=1;}pieces+=1;at=end;}}
 counts:[102400]int;touched:[4096]int;model:=BtModel.{source_bytes=total,pieces=pieces};
 for rank:0..<requested {used:=bt_histogram(symbols[..],groups[..],n,counts[..],touched[..]);
  winner:=bt_best(counts[..],touched[0..<used]);for i:0..<used {counts[touched[i]]=0;}if winner.frequency<2 {break;}left:=winner.key/320;right:=winner.key%320;candidate:=model;candidate.rules[rank]=BtRule.{left=left,right=right};candidate.merges=rank+1;v:=bt_vocabulary(candidate);if v.status!=PtStatus.ok {return BtModel.{status=v.status};}
  write:=0;i:=0;replaced:=0;while i<n {group:=groups[i];value:=symbols[i];if i+1<n && group==groups[i+1] && value==left && symbols[i+1]==right {value=256+rank;i+=2;replaced+=1;}else {i+=1;}symbols[write]=value;groups[write]=group;write+=1;}
  n=write;model.rules[rank]=BtRule.{left=left,right=right,frequency=winner.frequency,replaced=replaced};model.merges+=1;
 }
 model.remaining=n;return model;
}
```

The implementation uses an integer pair histogram with a list of touched keys.
It clears those counters between rounds, and returns a winner under the written
tie rule. The independent oracle instead reconstructs a Counter of every pair
on every round and builds fresh token lists. Twenty additional owned corpus
traces cover repeated symbols, embedded NUL, corpus permutations, boundaries,
empty work, descending byte pairs and exact maximum token growth.

The complete capture trains 64 merges on five CC0 synthetic training documents
from chapter 73 and one owned numeric fixture. Validation and test documents
are used only as encoding/round-trip inputs. It starts from 1,258 source bytes
in 288 pieces and ends with 720 symbols. This is a tiny deterministic tokenizer
experiment, not a claim that 64 merges are an effective production vocabulary.
Our merge tie rule is explicit; only the pretokenization pattern is claimed
compatible with the pinned nanochat source.

## Serialize and use the learned vocabulary

`bt_vocabulary` expands each rule from earlier IDs, rejecting forward or
negative references, duplicate pairs and pieces over 64 bytes. `bt_pack` writes
a chapter-5 **PST5** table: byte-ID mapping, every raw vocabulary span, pair
records sorted by `(left,right)` while retaining original merge ranks, the
Unicode property section, and the raw byte pool. It writes no normalization,
composition or added-token sections. Required size is computed before writes;
public and scratch capacities and observed aliases are checked first. Only a
completed table is copied to public output, preserving its unused tail.
The [C adapter](../platform/bpe-train-memory.c) observes spans only.

`bt_encode` selects this chapter's splitting profile, then calls chapter 5's
unchanged `pt_layout` and `pt_bpe`. `bt_decode` uses its `pt_span` reader and
concatenates raw bytes. Training and inference therefore share the same
pinned pretokenization. Chapter 5's existing Qwen entry point keeps its original
single-digit and normalization behavior; a PST5 header alone does not select
the nanochat profile. The profile identity must travel with an artifact.

Inputs to encoding and split inspection are at most 4,096 valid UTF-8 bytes.
Decoding accepts at most 4,096 IDs and 4,096 decoded bytes, including arbitrary
non-UTF-8 bytes from the base vocabulary. Value results own their storage and
return zero payload on refusal. Caller-supplied class tables and trained table
bytes must be admitted against their recorded identity once, kept immutable
and alive during calls; structural headers do not authenticate their contents.
C callers retain valid typed allocations and synchronous exclusive writes.

The native gate compares every learned rule, overlapping count and replacement
count, then compares the entire packed file against the independent oracle.
It writes Coax's table to disk, reopens it, and checks 147 independent regex
span/ID/byte round trips, including Unicode numbers, marks, emoji, contractions,
every ASCII control and held-out documents. Actual aliases refuse. Four native
builds cover debug/release, plain and ASan/UBSan.

## Run the small example

[The example](../examples/bpe-train.coax) uses the chapter-5 owned class table
and a small corpus. The native qualification uses the full pinned Unicode-17
property table from the original Qwen artifact.

<!-- source: psionic/examples/bpe-train.coax example -->
```jai
main :: () {
    classes:=PT_TOY;documents:=[1]string.["abab abab 12 12 é"];
    model:=bt_train(classes[..],documents[..],8);#assert model.status==PtStatus.ok;
    print("trained: % merges, % source bytes, % pieces, % remaining symbols\n",model.merges,model.source_bytes,model.pieces,model.remaining);
    print("first merge: % + %, overlapping count %, disjoint replacements %\n",model.rules[0].left,model.rules[0].right,model.rules[0].frequency,model.rules[0].replaced);
    packed:[65536]u8;scratch:[65536]u8;written:=bt_pack(model,classes[..],packed[..],scratch[..],false);#assert written.status==PtStatus.ok;
    text:="abab 1212 é";encoded:=bt_encode(packed[0..<written.written],text);#assert encoded.status==PtStatus.ok;
    decoded:=bt_decode(packed[0..<written.written],encoded.ids[0..<encoded.written]);#assert decoded.status==PtStatus.ok && cast(string)decoded.bytes[0..<decoded.written]==text;
    print("PST5: % bytes; encoded % IDs; exact UTF-8 byte round trip\n",written.written,encoded.written);
    digits:=bt_split(classes[..],"12345");print("two-digit spans end at % % %\n",digits.ends[0],digits.ends[1],digits.ends[2]);
}
```

```shell
coax/bin/coax run psionic/examples/bpe-train.coax
psionic/check.sh --only bpe-train --jobs 8
```

The [transcript](../fixtures/bpe-train.out) is exact in debug and release.
[The verification record](../../docs/verification/psionic-ch77-2026-10-30.md)
names the passing checks and fixture identities. No training, encoding,
persistence or verification step opens or queries the GPU.
