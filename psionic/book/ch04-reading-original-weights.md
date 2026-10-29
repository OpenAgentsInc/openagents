# 4. Reading original weights

[Chapter 3](ch03-numbers-and-matrices.md) gave us numerical operations. Now we
will feed them numbers stored in someone else's file. The reader must establish
what those bytes mean before they become shapes, offsets or floating-point
values that the rest of Psionic trusts.

By the end we can inspect an original Safetensors file, admit a complete pinned
single-file or sharded bundle, decode a bounded BF16/F32 window and multiply
an imported matrix using our existing numerical core. We preserve original
bytes and tensor names; no converted private checkpoint is required.

This is the first artifact slice toward [Elephant and Mammoth](../../docs/psionic/model-targets.md).
It does not execute those architectures yet. Their config interpretation,
tokenizers, larger execution budgets, attention and reference parity remain
separate chapters. The fixtures here are tiny originals authored in this
repository, with no downloaded model weights.

## Run the whole slice

From the repository root, after building Coax once:

```shell
coax/build.sh
coax/bin/coax run psionic/examples/read-weights.coax
psionic/check.sh
```

The [example](../examples/read-weights.coax) prints:

```jai
original bundle: 3 tensors, 22 payload bytes; identities match
BF16 [2,3]: first = 1.0, last = 6.0
imported matvec [1,-1,2]: [5.0, 11.0]
unaligned F32 bias: [0.5, -0.5]
imported scalar: 3 * 2.0 = 6.0
refused: missing tensor; float destination and scratch unchanged
```

The expected matrix is `[1,2,3;4,5,6]`. Multiplying it by `[1,-1,2]` gives
`[1 - 2 + 6, 4 - 5 + 12] = [5,11]`. The bias is `[0.5,-0.5]` and the scalar is
`2`, so the last prediction is `6`. These expectations are chosen before
running the reader; they are not numbers produced by a second copy of its logic.

The command-line example embeds the exact original bytes from
[the owned fixture directory](../fixtures/models/ch04/). The tooling checks
those constants against the actual committed files. The native harness opens
and reads those files and calls the same Coax library through its C interface.
File acquisition is a platform effect; there is no Python runtime dependency
and no model-loading implementation hidden in C.

## Read a byte contract first

The [Safetensors specification](https://github.com/safetensors/safetensors#format)
describes a file as an eight-byte unsigned little-endian header length, that
many bytes of UTF-8 JSON, and a payload. The header begins with `{`; trailing
whitespace can pad it. A tensor record contains `dtype`, `shape` and
`data_offsets`. The offsets are relative to the payload, with an exclusive end.
The payload must be completely indexed, without holes or overlapping tensors.
A special `__metadata__` object maps strings to strings. Duplicate keys are
invalid. These details matter more than the order of the JSON fields.

```jai
[8-byte header length][JSON header][payload bytes]
                                   ^ data base
absolute_begin = data_base + data_offsets[0]
absolute_end   = data_base + data_offsets[1]
```

We never cast an arbitrary byte address to a float pointer. A tensor can begin
at an unaligned byte offset. Our original fixture deliberately puts the F32
bias fourteen bytes into its payload, after six BF16 matrix entries and one
BF16 scalar. JSON key order is different from payload order.

## Name our supported profile

`original-weights-v1` is a bounded subset of Safetensors. The full format
also permits empty tensors and more dtypes. We explicitly refuse those here.
A format refusal is not evidence that an external file is corrupt.

| Resource or convention | This profile |
| --- | --- |
| Raw weight file | 10 bytes through 64 GiB, inclusive; prefix/header/payload all included. |
| JSON header / shard index | At most 1 MiB each. |
| Weight bundle | 1–16 shards; at most 128 GiB of raw weight files combined. |
| Inventory | 1–4,096 tensors across the bundle; globally unique names. |
| Shape | Rank 0–4, positive active axes, canonical zero inactive axes; scalar `[]` contains one value. |
| Dtypes | Original little-endian BF16 and F32 only. |
| Names / header metadata | At most 128 bytes per string, printable unescaped ASCII; tensor names and metadata keys nonempty; at most 64 header metadata pairs. |
| Shard filenames | Nonempty flat basenames ending `.safetensors`; no leading dot, slash, backslash or colon. |
| Identity sidecars | Config 1 byte–1 MiB; tokenizer 1 byte–64 MiB; hashed as opaque original bytes. |
| Decode | A nonempty window of at most 1,048,576 values, staged as finite F32. |

The artifact limits are separate from chapter 2's execution limit of one
million F32 elements. Inventory can describe a large original tensor without
allocating its expanded F32 representation. The host may map immutable files
and decode selected windows. This chapter does not provide an asynchronous
streaming filesystem API, a memory planner for a full model, or a faster backend.
The checked SHA-256 path processes byte views in chunks of at most 1 MiB.

## Admit the prefix before slicing

The [format unit](../lib/model-io/format.coax) checks the total file count before
reading eight bytes. It accumulates the length into `u64`, validates it against
the header and file budgets, then narrows it. An attacker cannot turn an
unsigned length into a negative signed offset or cause an unchecked addition.

<!-- source: psionic/lib/model-io/format.coax header -->
```jai
pw_header :: (file:[]u8)->PwHeader #export_c {
    if !pw_file_size(file.count) {return PwHeader.{status=PwStatus.size};}
    length:u64=0;
    for i:0..<8 {length |= cast(u64)file[i] << (i*8);}
    if !pw_header_size(length,file.count) {return PwHeader.{status=PwStatus.size};}
    if file[8]!='{' {return PwHeader.{status=PwStatus.schema};}
    return PwHeader.{data=8+cast(int)length};
}
```

The shape routine similarly checks rank and canonical axes before multiplying.
Each product must fit a byte budget using division first. Relative intervals
must have the exact dtype/shape byte length and fit the payload before we add
the header base. Pairwise interval checks are scoped to each shard; name
uniqueness is global. Once disjoint intervals cover exactly the payload byte
count, there is no room for an uncovered gap.

[The reader](../lib/model-io/reader.coax) uses Coax's existing
[RFC 8259 tokenizer](../../coax/lib/nostr/json.coax). It recognizes each required
field exactly once, accepts arbitrary field order, rejects unknown tensor
fields, validates metadata values, and checks the final JSON end token.
Escaped/non-ASCII identifiers are refused rather than compared under two
different spellings. The tokenizer's bounds come from the already admitted
header view; the profile's string limits are checked separately.

## Publish an inventory only when it is complete

An entry contains the original name, shape, dtype, element count, absolute
byte interval and shard slot. It does not own the bytes. Names borrow slices
of the original header; keep all original files alive and immutable while
using an inventory. Mutable metadata drafts are caller-provided workspaces.

<!-- source: psionic/lib/model-io/reader.coax inspect -->
```jai
pw_inspect_checked :: (file:[]u8,output:[]PwTensor,draft:[]PwTensor,overlap:bool)->PwResult #export_c {
    if !pw_file_size(file.count) {return PwResult.{status=PwStatus.size};}
    if output.count<1 || draft.count<1 {return PwResult.{status=PwStatus.capacity};}
    if overlap {return PwResult.{status=PwStatus.alias};}
    r:=pw_scan(file,draft,0,0);if r.status!=PwStatus.ok {return r;}
    if output.count<r.written {return PwResult.{status=PwStatus.capacity};}
    for i:0..<r.written {output[i]=draft[i];}
    return r;
}
```

The scanner can dirty `draft` while discovering a later problem. The public
`output` inventory is copied only after the entire file is valid and output
capacity is sufficient. Every refusal returns zero written entries and zero
payload bytes. This is synchronous publication into exclusively owned buffers,
not a concurrent atomic transaction.

Calling the raw inspector establishes structural validity. It does not
establish the identity or provenance of the artifact. For a model import, we
need the bundle gate next.

## Bind the complete bundle to expected identities

The [bundle unit](../lib/model-io/bundle.coax) accepts typed caller-provided
pins. They are a compact runtime form of an owner-reviewed manifest, not
another untrusted JSON file the loader silently accepts as its own authority.
The fixture's [manifest](../fixtures/models/ch04/manifest.json) records the
original artifact lengths, SHA-256 hashes, shape/dtype/name inventory and
fixture provenance. The example explicitly constructs its pins from those
owned fixtures; a production host must obtain expected pins through its
trusted model selection path.

<!-- source: psionic/lib/model-io/bundle.coax manifest -->
```jai
PwIdentity :: struct { origin:string; revision:string; config_sha:string; tokenizer_sha:string; index_sha:string; }
PwShard :: struct { name:string; data:[]u8; size:int; sha:string; }
PwSpec :: struct { name:string; shape:PsionicShape; dtype:PwDtype; shard:int; }
```

`origin` and `revision` are required bounded labels. A label is not a signature.
Expected hashes bind exact config, tokenizer, index and shard bytes; this does
not authenticate whoever supplied the hashes, establish redistribution rights,
or interpret the config/tokenizer. Actual model qualification must pin the
upstream repository revision, license and original files separately.

The gate hashes every sidecar and every shard using the existing
[checked Coax SHA-256 implementation](../../coax/lib/crypto/sha256.coax), checks
exact shard sizes, and scans all shards. It then matches the complete expected
tensor inventory by name, dtype, canonical shape and shard. Header order is
irrelevant. Publication uses the caller's manifest order. Missing, extra or
duplicate tensors refuse the entire bundle.

For sharded artifacts the
[Hugging Face index](https://huggingface.co/docs/transformers/main/big_models#shard-metadata)
provides `weight_map`, mapping tensor names to shard filenames. Our index
profile accepts `weight_map` and optional `metadata` containing exactly
`total_size`; when present that size must equal the sum of tensor payload bytes,
excluding headers. Every inventory entry must appear exactly once in the map
and point to its admitted shard. An empty index is allowed only for one file
and an empty expected index hash. Index strings never trigger filesystem opens:
the host supplies the exact expected shard views.

A successful bundle check leaves names borrowing the original headers. The
host must hold these exact bytes immutable between identity admission and
use. Changing a file after checking its hash invalidates that identity claim.
The synchronous API has no cache that makes mutated buffers trusted.

## BF16 is a bit placement operation

BF16 has the same exponent width and bias as F32, with seven explicit fraction
bits instead of twenty-three. Widening to F32 places its sixteen bits in the
high half of a 32-bit word and fills the low half with zeros. Finite BF16
values widen exactly, including signed zero and subnormals.

```jai
BF16 1.0:     0x3f80
F32  1.0:     0x3f800000
F32 bits = u32(BF16 word) << 16
```

<!-- source: psionic/lib/model-io/format.coax bf16 -->
```jai
pw_bf16_bits :: (word:u16)->u32 #export_c {return cast(u32)word << 16;}
pw_word :: (data:[]u8,at:int,width:int)->PwWord #export_c {
    if width!=2 && width!=4 {return PwWord.{status=PwStatus.unsupported};}
    if at<0 || width>data.count || at>data.count-width {return PwWord.{status=PwStatus.offsets};}
    bits:u32=0;
    for i:0..<width {bits |= cast(u32)data[at+i] << (i*8);}
    return PwWord.{bits=bits};
}
```

`pw_word` loads little-endian bytes without an alignment assumption. F32 uses
its original 32-bit encoding directly. The format itself allows infinities
and NaNs; the inspector can inventory them. Our execution profile refuses a
decode window containing either. It checks every selected encoding before
writing either float buffer, so a bad last value cannot leave partial floats.

## Decode a window, then use the existing kernels

[The decoding unit](../lib/model-io/weights.coax) reparses the original file to
find a tensor by name. It does not accept a caller-edited descriptor as proof
that an offset is safe. It validates `first`, `count` and both float capacities
with subtraction before computing the first byte address.

<!-- source: psionic/lib/model-io/weights.coax decode -->
```jai
// Metadata workspaces may change on refusal. Float output AND scratch do not.
pw_decode_checked :: (file:[]u8,name:string,first:int,count:int,output:[]f32,scratch:[]f32,inventory:[]PwTensor,draft:[]PwTensor,overlap:bool)->PwResult #export_c {
    if overlap {return PwResult.{status=PwStatus.alias};}
    r:=pw_inspect_checked(file,inventory,draft,false);if r.status!=PwStatus.ok {return r;}
    at:=pw_find(inventory[0..<r.written],name);if at<0 {return PwResult.{status=PwStatus.manifest};}
    tensor:=inventory[at];
    if !pw_window(tensor.elements,first,count,output.count,scratch.count) {return PwResult.{status=PwStatus.range};}
    width:=pw_width(tensor.dtype);begin:=tensor.begin+first*width;
    for i:0..<count {
        if !psionic_tensor_finite_bits(pw_float_bits(file,begin+i*width,tensor.dtype)) {return PwResult.{status=PwStatus.number};}
    }
    for i:0..<count {scratch[i]=f32_from_bits(pw_float_bits(file,begin+i*width,tensor.dtype));}
    for i:0..<count {output[i]=scratch[i];}
    return PwResult.{written=count,bytes=count*width};
}
```

The two metadata workspaces can change on refusal. The float destination and
float scratch both remain unchanged on every refusal. On success only the
selected prefix changes; unused writable capacity is preserved. The second
pass converts finite encodings, and the final pass publishes them. This is a
stronger scratch guarantee than chapter 3's arithmetic-overflow case.

Here is the whole runnable program. All read-only and writable locals have
separate storage, so the Coax call supplies `overlap = false`. Native callers
use the [memory wrapper](../platform/weights-memory.h), which observes every
writable span against all other spans, including nested shard bytes, names,
hashes, manifest strings and descriptor arrays. Its C code contains no parsing,
hashing, numeric conversion or model admission policy. As in earlier chapters,
truthful allocation lengths, pointer validity, alignment, exclusive access and
lifetime are caller obligations; an address observation cannot prove them.

<!-- source: psionic/examples/read-weights.coax example -->
```jai
main :: () {
    // These byte-exact owned originals are checked against the files on disk.
    file:=CH04_MODEL_SAFETENSORS;config:=CH04_CONFIG_JSON;tokenizer:=CH04_TOKENIZER_JSON;
    id:=PwIdentity.{origin="openagents-owned-ch04-fixture",revision="owned-v1",
        config_sha=CH04_CONFIG_JSON_SHA,tokenizer_sha=CH04_TOKENIZER_JSON_SHA};
    shards:=[1]PwShard.[PwShard.{name="model.safetensors",data=file[..],size=file.count,sha=CH04_MODEL_SAFETENSORS_SHA}];
    specs:=[3]PwSpec.[PwSpec.{name="scale"},
        PwSpec.{name="linear.weight",shape=PsionicShape.{rank=2,dims=[4]int.[2,3,0,0]}},
        PwSpec.{name="linear.bias",dtype=PwDtype.f32,shape=PsionicShape.{rank=1,dims=[4]int.[2,0,0,0]}}];
    inventory:[3]PwTensor;draft:[3]PwTensor;
    admitted:=pw_bundle_checked(id,config[..],tokenizer[..],file[0..<0],shards[..],specs[..],inventory[..],draft[..],false);
    #assert admitted.status==PwStatus.ok && admitted.written==3 && admitted.bytes==22;
    print("original bundle: % tensors, % payload bytes; identities match\n",admitted.written,admitted.bytes);

    weights:[6]f32;stage:[6]f32;
    decoded:=pw_decode_checked(file[..],"linear.weight",0,6,weights[..],stage[..],inventory[..],draft[..],false);
    #assert decoded.status==PwStatus.ok && decoded.written==6 && weights[0]==1 && weights[5]==6;
    print("BF16 [2,3]: first = %, last = %\n",weights[0],weights[5]);
    x:=[3]f32.[1,-1,2];out:[2]f32;scratch:[2]f32;ids:[1]int;mask:[1]u8;
    product:=psionic_ops_checked(PsionicRequest.{op=PsionicOp.matmul,rows=2,width=3,columns=1},
        weights[..],x[..],ids[0..<0],mask[0..<0],out[..],scratch[..],false);
    #assert product.status==PsionicOpStatus.ok && out[0]==5 && out[1]==11;
    print("imported matvec [1,-1,2]: [%, %]\n",out[0],out[1]);
    bias:=pw_decode_checked(file[..],"linear.bias",0,2,out[..],scratch[..],inventory[..],draft[..],false);
    #assert bias.status==PwStatus.ok && out[0]==0.5 && out[1]==-0.5;
    print("unaligned F32 bias: [%, %]\n",out[0],out[1]);
    scale:=pw_decode_checked(file[..],"scale",0,1,out[..],scratch[..],inventory[..],draft[..],false);
    #assert scale.status==PwStatus.ok && out[0]*3==6;
    print("imported scalar: 3 * % = %\n",out[0],out[0]*3);
    refused:=pw_decode_checked(file[..],"missing",0,1,out[..],scratch[..],inventory[..],draft[..],false);
    #assert refused.status==PwStatus.manifest && refused.written==0 && out[0]==2 && scratch[0]==2;
    print("refused: missing tensor; float destination and scratch unchanged\n");
}
```

The scalar `2` in this file is authored fixture data. It is not presented as
a saved training run from chapter 1. Checkpointing an optimizer and resuming
training remains a later chapter.

## Checks that can disagree with the implementation

The owned fixture generator uses Python's standard `json`, `struct` and
`hashlib` only as tooling. Its default verifies committed bytes, embedded Coax
constants and manifest identities; it does not overwrite changed fixtures.
The F32 bias is intentionally unaligned, fields are reordered, and the same
inventory appears in both a single original file and two shards plus an index.

```shell
python3 -I psionic/tests/weights_fixtures.py
coax/bin/coax test psionic/lib/model-io/weights.coax
coax/bin/coax test psionic/lib/model-io/weights.coax --release
coax/bin/coax mutate psionic/lib/model-io/weights.coax
psionic/check.sh
```

Checks precede implementation in the authority units. Their imported closure
also runs the shared JSON and checked crypto checks. Cases cover unsigned
prefixes, rank/shape/byte limits, malformed JSON, duplicate keys, overlapping
intervals, gaps, capacities, shard/index coverage, identity mismatches and
preserved publication buffers. Budget endpoints are checked without allocating
multi-gigabyte files. The native harness exercises actual files, every
truncation of the single-file fixture, real short allocations and aliases,
a SHA input crossing the one-MiB chunk boundary, and all 65,536 BF16 encodings.
Finite values are compared with an independent `ldexp` arithmetic oracle;
non-finite encodings must refuse. F32 checks include signed zero, subnormals,
maximum finite values, infinities and NaNs at the first and last positions.

The same generated library runs in debug and release, plain and under
ASan/UBSan. The book gate verifies every source listing, local link, example
transcript and exit status. The [verification record](../../docs/verification/psionic-ch04-2026-10-29.md)
retains exact results and scope. These bounded tests do not qualify arbitrary
Safetensors files, full Elephant/Mammoth checkpoints, another host's float
behavior, or a full model's memory/performance requirements.

## Try a change

Request the last two matrix entries with `first = 4`, `count = 2`, and derive
`[5,6]` before running. Try `first = 5`, `count = 2` and verify that both float
buffers remain unchanged. Then alter one original byte while keeping its
expected hash and verify that the bundle refuses before publishing inventory.

Next: tokenizers and prompt bytes. The original tokenizer identity we bind
here becomes an interpreted, checked input in chapter 5; original weight
execution will reuse this artifact boundary as attention and model-specific
architecture arrive.
