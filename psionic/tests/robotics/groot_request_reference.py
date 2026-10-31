#!/usr/bin/env python3
"""Firm reason2: original-tokenizer development oracle; data packing is tooling.

Original gated bytes and the derived PST stay PRIVATE. No NVIDIA/Transformers
source is executed. Existing installed tokenizers is the independent BPE oracle.
Only ASCII language is admitted by the current Coax request profile.
"""
import argparse
import ctypes as ct
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import struct
import numpy as np
import tokenizers

ROOT = Path(__file__).resolve().parents[3]
REV = "9ce19a195e423419c349abfc86fd07178b230561"
TOKEN_SHA = "a5d85b6dcc535e6b93115a9ef287e6132fdbf30270da6218194ba742261173c7"
UNICODE_SHA = "09eed4956d3394e254e92d9d2cc4cf83d31ac2cc92f5d9986090a1bff9b15223"
original = Path.home()/f".cache/huggingface/hub/models--nvidia--Cosmos-Reason2-2B/snapshots/{REV}/tokenizer.json"
private_pst = Path.home()/".cache/psionic/groot-libero/2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21/cosmos-tokenizer.pst"

def digest(b):
    return hashlib.sha256(b).hexdigest()

def pack_original():
    raw = original.read_bytes()
    assert digest(raw) == TOKEN_SHA
    j = json.loads(raw)
    m = j["model"]
    assert j["normalizer"] == {"type":"NFC"} and j["padding"] is None and j["truncation"] is None
    assert m["type"] == "BPE" and not m["dropout"] and not m["byte_fallback"] and not m.get("ignore_merges",False)
    assert all(not a[k] for a in j["added_tokens"] for k in ["normalized","single_word","lstrip","rstrip"])
    # Reuse owned serializer and public Unicode17 DATA; never the Qwen3.5 vocabulary.
    spec = importlib.util.spec_from_file_location("owned_text_packer",ROOT/"psionic/tests/text_fixtures.py")
    owned = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owned)
    public = (ROOT/"psionic/fixtures/text/ch05/qwen35.pst").read_bytes()
    assert digest(public) == UNICODE_SHA
    counts = struct.unpack_from("<7I",public,4)
    widths = [8,16,16,24,12,4,1]
    starts = []
    at = 1056
    for n,width in zip(counts,widths):
        starts.append(at)
        at += n*width
    def section(k):
        return [struct.unpack_from("<"+"I"*(widths[k]//4),public,starts[k]+i*widths[k]) for i in range(counts[k])]
    # Cosmos has \p{L}+, whereas Qwen3.5 also includes combining marks. CCC data
    # remains unchanged for NFC. Current public API admits only ASCII language.
    props = [(lo,hi,flags&~2,ccc) for lo,hi,flags,ccc in section(2)]
    inverse = owned.byte_alphabet()
    base = m["vocab"]
    vocab = [None]*(len(base)+len(j["added_tokens"]))
    for text,index in base.items():
        vocab[index] = bytes(inverse[c] for c in text)
    for a in j["added_tokens"]:
        vocab[a["id"]] = a["content"].encode()
    assert len(vocab)==151669 and all(vocab)
    pairs = [pair.split(" ") if isinstance(pair,str) else pair for pair in m["merges"]]
    assert all(len(pair)==2 for pair in pairs)
    merges = [(base[a],base[b],rank,base[a+b]) for rank,(a,b) in enumerate(pairs)]
    assert len(merges)==151387 and len({(a,b) for a,b,_,_ in merges})==len(merges)
    byte_ids = [base[next(c for c,b in inverse.items() if b==i)] for i in range(256)]
    packed = owned.pack(vocab,merges,[a["id"] for a in j["added_tokens"]],props,section(3),section(4),byte_ids)
    private_pst.parent.mkdir(parents=True,exist_ok=True)
    if private_pst.exists():
        assert private_pst.read_bytes()==packed
    else:
        private_pst.write_bytes(packed)
    return packed

class U8(ct.Structure):
    _fields_=[("data",ct.POINTER(ct.c_uint8)),("count",ct.c_int64)]
class I64(ct.Structure):
    _fields_=[("data",ct.POINTER(ct.c_int64)),("count",ct.c_int64)]
class String(ct.Structure):
    _fields_=[("data",ct.c_char_p),("count",ct.c_int64)]
class Read(ct.Structure):
    _fields_=[("status",ct.c_uint8),("written",ct.c_int64),("content",ct.c_int64),("count",ct.c_int64),("rope_delta",ct.c_int64)]
class PtRead(ct.Structure):
    _fields_=[("status",ct.c_uint8),("written",ct.c_int64),("work",ct.c_int64)]

def u8(x):
    return U8(x.ctypes.data_as(ct.POINTER(ct.c_uint8)),x.size)
def i64(x):
    return I64(x.ctypes.data_as(ct.POINTER(ct.c_int64)),x.size)
def string(b):
    return String(b,len(b))

def canonical(instruction):
    formal = re.sub(r"[^\w\s]","",instruction.lower())
    return ("<|im_start|>user\n"+("<|vision_start|>"+"<|image_pad|>"*64+"<|vision_end|>")*2+formal+"<|im_end|>\n").encode()

def reference_plan(ids,pad_to):
    # General source get_rope_index construction, independent of Coax's closed
    # form for fixed2cameras: append text spans and 3D image grids in token order.
    chunks=[]
    cursor=0
    for camera in range(2):
        end=ids.index(151655,cursor)
        start=int(chunks[-1].max())+1 if chunks else 0
        length=end-cursor
        chunks.append(np.tile(np.arange(length,dtype=np.int64),(3,1))+start)
        chunks.append(np.indices((1,8,8),dtype=np.int64).reshape(3,64)+length+start)
        cursor=end+64
    if cursor<len(ids):
        chunks.append(np.tile(np.arange(len(ids)-cursor,dtype=np.int64),(3,1))+int(chunks[-1].max())+1)
    position=np.concatenate(chunks,axis=1)
    count=pad_to or len(ids)
    left=count-len(ids)
    result=np.zeros((6,count),dtype=np.int64)
    result[0,:left]=151643
    result[0,left:]=ids
    result[1,left:]=1
    result[2,left:]=np.asarray(ids)==151655
    result[3:,:left]=1
    result[3:,left:]=position
    return result,int(position.max())+1-count

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--library")
    p.add_argument("--write-metadata",type=Path)
    args=p.parse_args()
    packed=pack_original()
    metadata={"schema":"psionic-groot-request-contract-v1","cosmos_revision":REV,"original_tokenizer_sha256":TOKEN_SHA,"private_packed_table":{"bytes":len(packed),"sha256":digest(packed),"private_cache_suffix":str(private_pst.relative_to(Path.home())),"derivation":"Original Cosmos BPE vocab/merges/added tokens and byte alphabet; owned PST5 serializer; public pinned Unicode17 tables with combining-mark wordflag cleared to match Cosmos regex. Current request formalization is ASCII only.","public_unicode_data_sha256":UNICODE_SHA},"selected_profile":{"language":"ASCII; Python lower then remove nonword/nonwhitespace; unsupported for nonASCII, never silently rewritten","cameras":["image","wrist_image"],"grid_thw":[[1,16,16],[1,16,16]],"image_tokens_each":64,"minimum_tokens":137,"maximum_tokens":512,"limit_origin":"Owned bounded admission profile; NVIDIA does not truncate or pad every request to512.","padding":"pad_to0 retains actual length; otherwise explicit leftpadding to supplied batchlength<=512","arrays":["input_ids","attention_mask","image_mask","position_t","position_h","position_w"]},"sample":{"instruction":"open the drawer","rendered_bytes":1767,"tokens":140,"prefix_ids":[151644,872,198,151652],"image_spans_half_open":[[4,68],[70,134]],"separator_ids":{"68":151653,"69":151652,"134":151653},"instruction_ids":[2508,279,26482],"suffix_ids":[151645,198],"rope_delta_unpadded":-112},"scope":"Request preparation only; no learned VLA or simulator execution. Gated original/derived tokenizer payloads remain private."}
    if args.library:
        lib=ct.CDLL(str(Path(args.library).resolve()))
        lib.gr_render.argtypes=[String,U8,U8,ct.c_bool];lib.gr_render.restype=Read
        lib.gr_plan.argtypes=[I64,ct.c_int64,I64,ct.c_bool];lib.gr_plan.restype=Read
        lib.pt_encode_checked.argtypes=[U8,String,I64,I64,U8,I64,I64,ct.c_bool];lib.pt_encode_checked.restype=PtRead
        table=np.frombuffer(packed,dtype=np.uint8)
        oracle=tokenizers.Tokenizer.from_file(str(original))
        samples=["","Open the drawer!","PICK up the RED cube, please.","   Keep__underscores 123\t\n", "a"*1024,"1"*375,"1"*376]
        samples += [chr(i) for i in range(128)]
        samples += [" ".join(["pick","up","the",str(i),"drawer"]*((i%7)+1)) for i in range(32)]
        words=0;plans=0;traces=[]
        for instruction in samples:
            raw=instruction.encode()
            out=np.full(8192,7,dtype=np.uint8);work=np.zeros(8192,dtype=np.uint8)
            r=lib.gr_render(string(raw),u8(out),u8(work),False)
            expected=canonical(instruction)
            assert r.status==0 and bytes(out[:r.written])==expected and np.all(out[r.written:]==7)
            words += r.written
            ids=oracle.encode(expected.decode(),add_special_tokens=False).ids
            encoded=np.full(2048,-7,dtype=np.int64);stage=np.zeros(2048,dtype=np.int64)
            normal=np.zeros(8192,dtype=np.uint8);scalars=np.zeros(8192,dtype=np.int64);symbols=np.zeros(2048,dtype=np.int64)
            e=lib.pt_encode_checked(u8(table),string(expected),i64(encoded),i64(stage),u8(normal),i64(scalars),i64(symbols),False)
            assert e.status==0 and encoded[:e.written].tolist()==ids and np.all(encoded[e.written:]==-7)
            words += e.written
            tokens=np.asarray(ids,dtype=np.int64)
            output=np.full(3073,-7,dtype=np.int64)
            if len(ids)>512:
                refused=lib.gr_plan(i64(tokens),0,i64(output),False)
                assert refused.status==1 and refused.written==0 and np.all(output==-7)
                continue
            for pad in sorted({0,len(ids),min(512,len(ids)+1),512}):
                output.fill(-7)
                result=lib.gr_plan(i64(tokens),pad,i64(output),False)
                wanted,delta=reference_plan(ids,pad)
                assert (result.status,result.written,result.content,result.count,result.rope_delta)==(0,wanted.size,len(ids),wanted.shape[1],delta)
                assert np.array_equal(output[:wanted.size],wanted.ravel()) and np.all(output[wanted.size:]==-7)
                words += wanted.size;plans += 1
            if instruction=="Open the drawer!":
                assert ids[:4]==metadata['sample']['prefix_ids'] and ids[135:138]==metadata['sample']['instruction_ids']
            traces.append({"instruction_sha256":digest(raw),"rendered_sha256":digest(expected),"tokens":len(ids),"token_ids_le_i64_sha256":digest(np.asarray(ids,dtype='<i8').tobytes())})
        metadata['qualification']={"tokenizers_version":tokenizers.__version__,"requests":len(samples),"plans":plans,"exact_bytes_or_integer_words":words,"traces":traces}
        print(f"GR00T request original tokenizers {tokenizers.__version__}: {len(samples)}requests,{plans}plans,{words}exact bytes/integerwords passed")
    if args.write_metadata:
        args.write_metadata.write_text(json.dumps(metadata,indent=2)+"\n")
    print(f"Private Cosmos PST5: {len(packed)}bytes SHA256{digest(packed)}")

if __name__=="__main__":
    main()
