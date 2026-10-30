# 62. Memory by lookup

Engram remembers short token patterns by looking up rows, rather than scanning
a learned table. The pinned V4.1 Flash configuration uses two tables, at layers
1 and 14. Each table occupies about 101 GB, but one token asks for just 24 rows
from each. The table stays in a file; only those rows enter the decode buffer.

This chapter implements `deepseek-engram-lookup-v1`: the original normalized
token map, exact hash constants and bucket layout, streaming history, file
range plans and FP8-to-BF16 row decoding. It qualifies bounded `mmap` and
`pread` effects with owned rows at the original files' sizes and offsets.
Original table payloads, learned lookup projections and gated residual
injection remain part of chapter 68's model binding. No complete original
Engram forward pass is claimed here.

The reference [Engram source](https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/inference/engram.py),
configuration and tokenizer are pinned in
[engram-source.json](../fixtures/deepseek/engram-source.json), with lossless
original archives. Upstream code is read, never executed.

## Normalize once, admit a map

The reference decodes each of the tokenizer's 129,280 IDs separately, with
special tokens retained. NFKC then NFD, accent stripping and lowercasing merge
spelling variants. Accent stripping removes all Unicode mark categories;
removing only nonspacing marks produces the wrong vocabulary. Only ASCII
space, tab, carriage return and newline collapse to one space. A private-use
sentinel preserves an exact single space through trimming. Empty normalized
strings fall back to their decoded text. Tokens that decode with U+FFFD use
their raw token spelling instead, preserving partial UTF-8 byte tokens.

Assign compressed IDs in raw-ID order, reusing the first ID for each equal
normalized key. This yields exactly **99,092 classes**. Raw pad ID 2 maps to
compressed pad ID 2. The compressed size also determines the hash multipliers;
using the raw vocabulary size changes every row choice.

The offline owned oracle reconstructs byte decoding and normalization with
Python's Unicode 15.1 database. Every decoded/normalized key was independently
compared with Rust `tokenizers` 0.22.2, with zero differences. The resulting
517,120-byte little-endian u32 map has SHA-256
`c60a86322ec17b4142bfef3c57a8d81fb428550cdf487f88a4320cb59fe46481`.
`eg_v41_map` authenticates those complete bytes and checks every mapped ID.
The returned descriptor borrows immutable bytes. Serving needs neither Python,
Rust tokenizers nor a Unicode normalization engine.

## Hash the current token and three predecessors

There are eight heads for each of the 2-, 3- and 4-grams, making 24 bucket
ranges per layer. Starting at 16,000,000, allocate the next unused prime for
each range, sharing the used-prime set across both layers. The resulting
48 primes and cumulative offsets give these row counts:

| Layer | Rows | FP8 codes | UE8M0 scales | Total tensor bytes |
| --- | --- | --- | --- | --- |
| 1 | 384,006,168 | 98,305,579,008 | 3,072,049,344 | 101,377,628,352 |
| 14 | 384,016,682 | 98,308,270,592 | 3,072,133,456 | 101,380,404,048 |

Each lookback has a fixed odd multiplier. The reference seeds NumPy's
`default_rng` with `10007 * layer_id`, draws four integers below
`floor(floor((2^63-1)/99092)/2)`, then doubles and adds one. The retained
constants match NumPy 2.4.4 exactly. Products fit positive signed int64.
XOR the products through the requested n-gram length, reduce modulo that
head's prime and add its bucket offset.

`EgHistory` carries the position and three compressed IDs, including dead
markers. Missing prefix positions and masked tokens are dead. When a dead
position is encountered while looking backward, it and every older position
use pad ID 2. Keep the actual last three IDs across a dead marker: subsequent
tokens still need the exact original history. A masked current token hashes
four pads, then enters history as dead. The caller also suppresses that
position's learned Engram gate when integrating the model.

Hash output is token-major, then layer, n-gram length and head. The complete
chunk is staged before any public ID is written. Failure returns no next
history; successful calls return the exact continuation. Whole, single-token
and irregular chunks give identical IDs and history.

<!-- source: psionic/lib/deepseek/engram-hash.coax hash -->
```jai
eg_hash :: (m:EgMap,state:EgHistory,tokens:[]int,mask:[]u8,out:[]int,stage:[]int,overlap:bool)->EgHash #export_c {
    if !eg_hash_size(tokens.count) {return EgHash.{status=EgStatus.shape};}
    if !eg_raw_size(m.raw) || m.data.count!=m.raw*4 || m.compressed!=99092 || m.pad!=2 {return EgHash.{status=EgStatus.identity};}
    if !eg_history_valid(state) || !eg_advance(state.next,tokens.count) {return EgHash.{status=EgStatus.state};}
    n:=tokens.count*48;if mask.count!=tokens.count || out.count<n || stage.count<n {return EgHash.{status=EgStatus.capacity};}if overlap {return EgHash.{status=EgStatus.alias};}
    next:=state;for t:0..<tokens.count {if tokens[t]<0 || tokens[t]>=m.raw {return EgHash.{status=EgStatus.token};}if mask[t]>1 {return EgHash.{status=EgStatus.format};}
        current:=eg_u32(m.data,4*tokens[t]);if current>=m.compressed {return EgHash.{status=EgStatus.token};}if mask[t]==0 {current=-1;}
        look:=eg_look(current,next);
        for layer:0..<2 {rolling:=u64(look[0])*EG_MULT[layer*4];for back:1..<4 {rolling^=u64(look[back])*EG_MULT[layer*4+back];
            for head:0..<8 {i:=layer*24+(back-1)*8+head;stage[t*48+i]=cast(int)(rolling%u64(EG_PRIMES[i]))+EG_OFFSETS[i];}}}
        next.last=[3]int.[current,next.last[0],next.last[1]];next.next+=1;
    }
    for i:0..<n {out[i]=stage[i];}return EgHash.{written=n,next=next};
}
```

## Read 6,336 bytes per layer

Each row holds 256 E4M3 code bytes and eight UE8M0 scale bytes. Scales belong
to one row and each group of 32 values, rather than a dense 32-by-32 tile.
`EgTable` describes independent code and scale tensor offsets inside one file,
its row count and exact expected file length. Admission checks both tensor
extents and their separation with 64-bit arithmetic; either tensor order is
valid. `eg_reads` stages two ranges per requested row and arranges their
private destinations as consecutive 264-byte packets.

The original two shards have 664- and 672-byte header prefixes and total
lengths 101,535,150,936 and 101,537,926,640 bytes. Their retained HTTP range
headers confirm the dtype, shape and offsets. These prefixes do **not**
authenticate the complete weight files. Model binding must separately admit
the complete immutable originals and their tensor identity.

For 24 rows, the range plan has 48 reads and requests 6,336 payload bytes.
`psionic_eg_lookup` either maps the immutable file read-only and lazily, or
uses `pread` on those ranges. Coax checks dimensions, work capacity and
range plans before any file effect. C observes addresses and file length,
performs the I/O and reports effect failures. It contains no hash or numeric
policy. The mapping is virtual address space, not a copy of the whole table.
OS page faults, readahead and page-cache residency can exceed the selected
payload; no physical disk-bandwidth claim follows from its byte count.

The decoder uses chapter 66's exact E4M3/UE8M0 scalar reconstruction and
nearest-even BF16 publication. Invalid code/scale bytes or overflow refuse.
All decoded words stage before publication; a late bad row or failed file
operation preserves the complete output and spare tail. Packet and decode
work are private and may change on failure. One layer requires 6,336 packet
bytes and 12,288 decode-work bytes, plus its BF16 output; the adapter uses
48 planned and 48 draft range descriptors on its stack.

The caller owns valid typed allocations/extents, immutable admitted map and
file lifetimes, matching model/table/history identity and exclusive disjoint
writes. Keep a file unchanged through `fstat`, mapping or reads; truncation of
a live mapping violates that contract. File-length observation is a shape
check, not authentication. The library does not retain mappings, allocate
whole tables or choose cache residency.

## Run and check it

The [example](../examples/deepseek-engram.coax) hashes an owned small map,
checks all 912 IDs, plans one layer's rows and decodes independently scaled
groups. The native gate additionally admits the complete original tokenizer
map and checks an original-ID prefix.

```shell
coax/bin/coax run psionic/examples/deepseek-engram.coax
coax/bin/coax run psionic/examples/deepseek-engram.coax --release
psionic/tests/check_deepseek_engram.sh
psionic/check.sh --only deepseek/engram --jobs 8
```

<!-- source: psionic/examples/deepseek-engram.coax example -->
```jai
main :: () {
    // An owned map keeps the example small. The native gate admits the complete
    // original 129,280-ID map, then hashes the same original prefix independently.
    bytes:=EGG_MAP;map:=eg_admit_map(bytes[..],8,99092,2,EGG_MAP_SHA);#assert map.status==EgStatus.ok;
    tokens:=EGG_TOKENS;mask:=EGG_MASK;ids:[912]int;work:[912]int;
    r:=eg_hash(map.value,eg_start(),tokens[..],mask[..],ids[..],work[..],false);#assert r.status==EgStatus.ok;
    for i:0..<912 {#assert ids[i]==EGG_HASHES[i];}
    print("Engram: % tokens, % row IDs, % retained history tokens\n",r.next.next,r.written,3);
    for layer:0..<2 {print("table %: % rows, % code and scale bytes\n",layer,EG_ROWS[layer],EG_ROWS[layer]*264);}
    table:=EgTable.{rows=EG_ROWS[0],data=4096,scales=4096+EG_ROWS[0]*256,file_bytes=4096+EG_ROWS[0]*264};
    reads:[48]EgRead;draft:[48]EgRead;p:=eg_reads(table,ids[0..<24],reads[..],draft[..],false);#assert p.status==EgStatus.ok;
    print("one layer lookup: % reads, % bytes, % BF16 values\n",p.written,p.bytes,24*256);
    packet:[264]u8;for i:0..<256 {packet[i]=0x38;}for i:0..<8 {packet[256+i]=u8(127+i);}
    values:[256]u16;stage:[256]u16;decoded:=eg_decode(1,packet[..],values[..],stage[..],false);#assert decoded.status==EgStatus.ok && values[0]==0x3f80 && values[255]==0x4300;
    print("row decode: % values, independent scales per 32\n",decoded.written);
}
```

The owned prefix oracle retains full history instead of the implementation's
three-slot continuation. A separate prime construction and NumPy multiplier
comparison pin the layout. Independent scalar row equations supply 12,288
expected BF16 words, with different scales in every group.

Native checks create sparse owned files with the original shard lengths and
tensor offsets: 203,073,077,576 logical bytes with about 0.4 MB physically
allocated in this run. Both mapped and direct reads fetch the same requested
payload and decode exactly. They exercise original hashed row IDs plus the
last row of each table, real aliases, exact capacities, short/bad files and
late malformed rows. Debug/release and plain/ASan/UBSan modes all qualify the
same boundaries. These sparse files contain owned test rows, not original
model embeddings. Exact evidence and scope are in the
[verification record](../../docs/verification/psionic-ch62-2026-10-29.md).
