# 33. Read a GGUF file

A checkpoint starts with a description of its tensors. Before mapping any
weight bytes we need to know whether that description fits in the file, and
whether names, counts, shapes, blocks and offsets can be trusted as addresses.
This chapter supplies the bounded `gguf-v3-le-v1` structural reader in
[gguf.coax](../lib/gguf.coax). It runs entirely on the CPU.

The input is an immutable header prefix and the observed full file length.
Metadata keys and tensor names borrow that prefix: retain it while using the
result. The caller supplies separate public and scratch descriptor arrays.
[The C adapter](../platform/gguf-memory.c) observes their address overlap;
Coax decides admission and publishes descriptors only after the entire header
passes. Failure preserves every public descriptor, including unused tails.
Scratch may change. `PgMeta` preserves a value's wire type and byte span;
`pg_atom` reads scalar bits or an array's element type and count without
normalizing strings or floating values.

## A named format profile

We pin the [GGUF specification](https://github.com/ggml-org/ggml/blob/facdc44a67da17df4433adcc468b424f48e9ded2/docs/gguf.md)
and [llama.cpp layout constants](https://github.com/ggml-org/llama.cpp/blob/e0f142d5ddc1461ce721644d92af33db23545105/gguf-py/gguf/constants.py).
This version reads **little-endian GGUF v3**. Big endian and older versions
return `unsupported`. It recognizes all 13 metadata tags and the 36 tensor
layouts present in the pinned table; unknown or retired layout IDs refuse.
This does not decode their quantized arithmetic.

Limits are explicit: a file up to 1 TiB, a supplied prefix up to 64 MiB,
4,096 metadata records and 4,096 tensors, printable ASCII metadata keys of
1–256 bytes, UTF-8 tensor names of 1–64 bytes, and strings up to 1 MiB.
Nested arrays have at most eight array levels and 1,048,576 visited values
in total. Boolean bytes must be zero or one. Tensor rank is 1–4, dimensions
are positive, the element product is at most 2^44, and the first dimension
must contain complete quantization blocks. Every product is checked before
multiplication. Metadata and tensor names must be unique within their kind.

The default alignment is 32. `general.alignment` must be U32, a power of two
from 8 through 1,048,576, following the pinned implementation's narrower
profile. Offsets are relative to the aligned data base. Payload ranges must
fit the observed file and not overlap; gaps and descriptor order are allowed.
Header padding must be zero. Vocabulary-only upstream files omit final
alignment padding: with zero tensors the reader permits zero padding from the
metadata end through its aligned boundary and returns `data = 0`.
It does not authenticate payload contents, infer architecture, allocate
weights or open a device. File length is an observation, not proof that a
subsequently modified file still has these contents.

## Inspect before publishing

<!-- source: psionic/lib/gguf.coax inspect -->
```jai
pg_read :: (header:[]u8,file_bytes:int,meta:[]PgMeta,meta_work:[]PgMeta,tensors:[]PgTensor,tensor_work:[]PgTensor,overlap:bool)->PgResult #export_c {
    if overlap {return PgResult.{status=PgStatus.alias};}if !pg_file(file_bytes) {return PgResult.{status=PgStatus.size};}if header.count<24 {return PgResult.{status=PgStatus.truncated};}if !pg_prefix(header.count,file_bytes) {return PgResult.{status=PgStatus.size};}
    c:=pg_uint(PgCursor.{data=header},4);if c.word!=0x46554747 {return PgResult.{status=PgStatus.format};}c=pg_uint(c,4);if c.word!=3 {return PgResult.{status=PgStatus.unsupported};}
    c=pg_count(c,4096);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}nt:=cast(int)c.word;c=pg_count(c,4096);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}nm:=cast(int)c.word;
    if meta.count<nm || meta_work.count<nm || tensors.count<nt || tensor_work.count<nt {return PgResult.{status=PgStatus.capacity};}alignment:=32;
    for i:0..<nm {c=pg_string(c,256);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}key:=c.text;if !pg_key(key) {return PgResult.{status=PgStatus.format};}for j:0..<i {if meta_work[j].key==key {return PgResult.{status=PgStatus.duplicate};}}
        c=pg_uint(c,4);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}kind:=cast(int)c.word;begin:=c.at;c=pg_skip(c,kind);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}
        meta_work[i]=PgMeta.{key=key,kind=kind,begin=begin,end=c.at};
        if key=="general.alignment" {if kind!=4 {return PgResult.{status=PgStatus.format};}a:=pg_uint(PgCursor.{data=header,at=begin},4);alignment=cast(int)a.word;if !pg_alignment(alignment) {return PgResult.{status=PgStatus.offsets};}}
    }
    for i:0..<nt {c=pg_name(c);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}name:=c.text;if name.count==0 {return PgResult.{status=PgStatus.format};}for j:0..<i {if tensor_work[j].name==name {return PgResult.{status=PgStatus.duplicate};}}
        c=pg_uint(c,4);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}if c.word<1 || c.word>4 {return PgResult.{status=PgStatus.shape};}rank:=cast(int)c.word;dims:[4]int;
        for axis:0..<rank {c=pg_dimension(c);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}dims[axis]=cast(int)c.word;}for axis:rank..<4 {dims[axis]=1;}
        c=pg_uint(c,4);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}kind:=cast(int)c.word;size:=pg_size(rank,dims,kind);if size.status!=PgStatus.ok {return PgResult.{status=size.status};}
        c=pg_count(c,PG_FILE_MAX);if c.status!=PgStatus.ok {return PgResult.{status=c.status};}offset:=cast(int)c.word;if offset%alignment!=0 {return PgResult.{status=PgStatus.offsets};}
        tensor_work[i]=PgTensor.{name=name,rank=rank,dims=dims,kind=kind,elements=size.elements,offset=offset,bytes=size.bytes};
    }
    data:=pg_align(c.at,alignment);padding_end:=data;
    if nt==0 {if file_bytes>data {return PgResult.{status=PgStatus.offsets};}padding_end=file_bytes;}
    for i:0..<nt {t:=tensor_work[i];if !pg_range(file_bytes,data,t.offset,t.bytes) {return PgResult.{status=PgStatus.offsets};}t.begin=data+t.offset;t.end=t.begin+t.bytes;
        for j:0..<i {other:=tensor_work[j];if pg_intersects(t.begin,t.end,other.begin,other.end) {return PgResult.{status=PgStatus.offsets};}}tensor_work[i]=t;
    }
    if padding_end>header.count {return PgResult.{status=PgStatus.truncated};}for i:c.at..<padding_end {if header[i]!=0 {return PgResult.{status=PgStatus.padding};}}
    for i:0..<nm {meta[i]=meta_work[i];}for i:0..<nt {tensors[i]=tensor_work[i];}return PgResult.{metadata=nm,tensors=nt,data=ifx nt==0 then 0 else data,alignment=alignment};
}
```

The borrowed value spans allow a model-specific reader to interpret metadata
later. This separates structural admission from an architecture's required
keys, tensor names and quantization support.

## Run it

[The example](../examples/gguf.coax) uses an owned fixture with every metadata
wire tag and two tensor layouts. It corrupts late padding after a successful
read to demonstrate atomic publication.

<!-- source: psionic/examples/gguf.coax example -->
```jai
main :: () {
    bytes:=PG_OWNED;metadata:[15]PgMeta;meta_work:[15]PgMeta;tensors:[2]PgTensor;tensor_work:[2]PgTensor;
    r:=pg_read(bytes[..],bytes.count,metadata[..],meta_work[..],tensors[..],tensor_work[..],false);
    #assert r.status==PgStatus.ok;
    print("GGUF v3: % metadata, % tensors, data at %\n",r.metadata,r.tensors,r.data);
    for i:0..<r.tensors {t:=tensors[i];print("%: type %, % elements, bytes [% , %)\n",t.name,t.kind,t.elements,t.begin,t.end);}
    bytes[575]=1;
    refused:=pg_read(bytes[..],bytes.count,metadata[..],meta_work[..],tensors[..],tensor_work[..],false);
    #assert refused.status==PgStatus.padding && tensors[0].begin==576;
    print("nonzero padding: refused; published descriptors preserved\n");
}
```

```shell
coax/bin/coax run psionic/examples/gguf.coax
psionic/check.sh --only gguf --jobs 8
```

The [recorded transcript](../fixtures/gguf.out) is exact in debug and release.
The [stdlib struct oracle](../tests/gguf_fixtures.py) constructs independent
headers, extracts layout DATA from inert pinned source, and checks all fixture
identities. The native harness compares every metadata span and tensor shape,
type and byte extent against its expected results for an original 260K-model
GGUF and an original GPT-2 vocabulary GGUF, plus the owned all-layout fixture.
A pinned big-endian checkpoint explicitly refuses. Their revisions, URLs,
full-file hashes and compressed artifacts are in [source.json](../fixtures/gguf/source.json).
No downloaded scripts are executed and no model decision runs.

Malformed checks cover every shorter owned header prefix, 61 independently
patched wire fields, duplicates, maximum budgets, aliasing, nested arrays,
and exact boundary products. The [verification record](../../docs/verification/psionic-ch33-2026-10-30.md)
reports the focused debug/release, native, sanitizer and mutation results.
