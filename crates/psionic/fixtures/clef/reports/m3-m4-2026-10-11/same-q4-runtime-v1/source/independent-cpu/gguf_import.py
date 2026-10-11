"""Bounded GGUF import for an independent, streamed CPU reference."""
import ast
import hashlib
import json
import math
import os
from pathlib import Path
import struct

import numpy as np

BASE = Path("@@CLEF_SCRATCH@@")
GGUF = BASE / "artifacts/Clef-Q4_K_M.gguf"
UPSTREAM = BASE / "q4k-weight-diagnostic/upstream_quants.py"
QUANT = {0:(1,4,None), 1:(1,2,None), 8:(32,34,"Q8_0"), 12:(256,144,"Q4_K"), 14:(256,210,"Q6_K")}


def require(value, message):
    if not value:
        raise ValueError(message)


def sha_bytes(value):
    return hashlib.sha256(value).hexdigest()


def stat(path):
    value = path.stat()
    return [value.st_size, value.st_ino, value.st_mtime_ns, value.st_ctime_ns]


def decoder_classes():
    source = UPSTREAM.read_bytes()
    expected = json.loads(UPSTREAM.with_name("source.json").read_text())
    require(sha_bytes(source) == expected["sha256"], "independent GGUF decoder changed")
    selected = []
    for node in ast.parse(source).body:
        if isinstance(node, ast.ClassDef) and node.name in {"Q4_K", "Q6_K", "Q8_0"}:
            node.bases, node.keywords = [], []
            selected.append(node)
    require(len(selected) == 3, "independent decoder classes missing")
    scope = {"np":np,"QK_K":256}
    exec(compile(ast.fix_missing_locations(ast.Module(body=selected,type_ignores=[])),str(UPSTREAM),"exec"),scope)
    return {name:scope[name] for name in ("Q4_K","Q6_K","Q8_0")}, expected


class StreamingGguf:
    def __init__(self, path=GGUF):
        self.path = Path(path)
        self.state = stat(self.path)
        require(self.state[0] == 19232219200, "GGUF artifact size differs")
        self.decoders, self.decoder_identity = decoder_classes()
        self.tensors, self.metadata = {}, {}
        self.payload_read_bytes = 0
        with self.path.open("rb") as source:
            def number(fmt):
                return struct.unpack("<"+fmt,source.read(struct.calcsize(fmt)))[0]
            def string():
                size=number("Q");require(size <= 16*1024**2,"metadata string too large")
                return source.read(size).decode("utf8")
            formats={0:"B",1:"b",2:"H",3:"h",4:"I",5:"i",6:"f",7:"?",10:"Q",11:"q",12:"d"}
            def value(kind):
                if kind in formats:return number(formats[kind])
                if kind==8:return string()
                require(kind==9,"unsupported GGUF metadata")
                subtype,count=number("I"),number("Q")
                require(count<=1000000,"metadata array too large")
                if subtype in formats:source.seek(count*struct.calcsize(formats[subtype]),1)
                else:
                    for _ in range(count):value(subtype)
                return None
            require(source.read(4)==b"GGUF" and number("I")==3,"GGUF version differs")
            count, metadata=number("Q"),number("Q")
            require(count<10000 and metadata<1000,"GGUF counts exceed bounds")
            for _ in range(metadata):
                key=string();self.metadata[key]=value(number("I"))
                require(source.tell()<64*1024**2,"GGUF metadata exceeds bound")
            for _ in range(count):
                name, dims=string(),number("I")
                require(1<=dims<=4,"invalid tensor rank")
                shape=[number("Q") for _ in range(dims)]
                self.tensors[name]={"shape":shape,"type":number("I"),"offset":number("Q")}
            alignment=self.metadata.get("general.alignment",32)
            self.data_start=(source.tell()+alignment-1)//alignment*alignment
        require(self.metadata.get("qwen35.ssm.v_head_reordered",True) is True,"unsupported value-head layout")
        self.fd=os.open(self.path,os.O_RDONLY)

    def close(self):
        if self.fd is not None:os.close(self.fd);self.fd=None

    def check_state(self):
        require(stat(self.path)==self.state,"GGUF changed during diagnostic")

    def flat(self,name,first=0,count=None):
        item=self.tensors[name]
        require(item["type"] in QUANT,"unsupported tensor dtype: "+name)
        block,byte_count,decoder=QUANT[item["type"]]
        total=math.prod(item["shape"])
        count=total-first if count is None else count
        require(0<=first<total and 0<count<=total-first and first%block==0 and count%block==0,"unaligned tensor slice")
        require(count*4<=512*1024**2,"single decoded tensor exceeds512MiB")
        result=np.empty(count,dtype=np.float32)
        for start in range(0,count,block*2048):
            amount=min(block*2048,count-start)
            offset=self.data_start+item["offset"]+(first+start)//block*byte_count
            size=amount//block*byte_count
            raw=os.pread(self.fd,size,offset)
            self.payload_read_bytes+=len(raw)
            require(len(raw)==size,"short GGUF tensor read")
            if item["type"]==0:values=np.frombuffer(raw,dtype="<f4")
            elif item["type"]==1:values=np.frombuffer(raw,dtype="<f2").astype(np.float32)
            else:values=self.decoders[decoder].dequantize_blocks(np.frombuffer(raw,dtype=np.uint8).reshape(-1,byte_count)).reshape(-1)
            require(np.isfinite(values).all(),"nonfinite GGUF weight")
            result[start:start+amount]=values
        return result

    def full(self,name):
        return self.flat(name).reshape(tuple(reversed(self.tensors[name]["shape"])))

    def rows(self,name,rows):
        shape=self.tensors[name]["shape"]
        require(len(shape)==2 and 0<len(rows)<=2048 and all(0<=r<shape[1] for r in rows),"row gather outside bound")
        return np.stack([self.flat(name,row*shape[0],shape[0]) for row in rows])


def head_permutation(heads=48,keys=16):
    require(heads%keys==0,"invalid grouped heads")
    # Row g in GGUF was moved from HF row (g % keys) * groups + g // keys.
    gg_to_hf=np.array([(g%keys)*(heads//keys)+g//keys for g in range(heads)])
    return gg_to_hf,np.argsort(gg_to_hf)


def value_rows_to_hf(array,width=128,offset=0):
    _,hf_to_gg=head_permutation()
    selected=(offset+hf_to_gg[:,None]*width+np.arange(width)).reshape(-1)
    result=array.copy()
    result[offset:offset+48*width]=array[selected]
    return result


def value_columns_to_hf(array):
    _,hf_to_gg=head_permutation()
    selected=(hf_to_gg[:,None]*128+np.arange(128)).reshape(-1)
    return array[:,selected].copy()


def ordinary_norm_to_hf(array):
    parameter=array-np.float32(1)
    require(np.array_equal((parameter+np.float32(1)).view(np.uint32),array.view(np.uint32)),"norm +1 is not exactly reversible")
    return parameter


def layer_arrays(reader,index):
    prefix=f"blk.{index}."
    get=lambda name:reader.full(prefix+name)
    result={"input_layernorm.weight":ordinary_norm_to_hf(get("attn_norm.weight")),
            "post_attention_layernorm.weight":ordinary_norm_to_hf(get("post_attention_norm.weight"))}
    for operation in ("gate","up","down"):
        result[f"mlp.{operation}_proj.weight"]=get(f"ffn_{operation}.weight")
    negative_a=None
    if index%4==3:
        for destination,source in (("q_proj","attn_q"),("k_proj","attn_k"),("v_proj","attn_v"),("o_proj","attn_output")):
            result[f"self_attn.{destination}.weight"]=get(source+".weight")
        for name in ("q","k"):
            result[f"self_attn.{name}_norm.weight"]=ordinary_norm_to_hf(get(f"attn_{name}_norm.weight"))
    else:
        result["linear_attn.in_proj_qkv.weight"]=value_rows_to_hf(get("attn_qkv.weight"),offset=4096)
        result["linear_attn.in_proj_z.weight"]=value_rows_to_hf(get("attn_gate.weight"))
        for destination,source in (("a","alpha"),("b","beta")):
            result[f"linear_attn.in_proj_{destination}.weight"]=value_rows_to_hf(get(f"ssm_{source}.weight"),width=1)
        conv=value_rows_to_hf(get("ssm_conv1d.weight"),offset=4096)
        result["linear_attn.conv1d.weight"]=conv[:,None,:]
        result["linear_attn.dt_bias"]=value_rows_to_hf(get("ssm_dt.bias"),width=1)
        result["linear_attn.norm.weight"]=get("ssm_norm.weight")
        result["linear_attn.out_proj.weight"]=value_columns_to_hf(get("ssm_out.weight"))
        negative_a=value_rows_to_hf(get("ssm_a"),width=1)
        require(np.all(negative_a<0),"stored A must be negative")
        # The forward adapter uses stored A directly. The unused parameter stays
        # present so loading still verifies the original module's complete keys.
        result["linear_attn.A_log"]=np.zeros(48,dtype=np.float32)
    require(sum(v.nbytes for v in result.values())<2*1024**3,"layer resident weights exceed2GiB")
    return result,negative_a


def fixture_checks():
    gg_to_hf,hf_to_gg=head_permutation()
    original=np.arange(48*128*3,dtype=np.float32).reshape(48*128,3)
    gg=original.reshape(48,128,3)[gg_to_hf].reshape(48*128,3)
    require(np.array_equal(value_rows_to_hf(gg),original),"value-row inverse failed")
    qk=np.arange(4096*3,dtype=np.float32).reshape(4096,3)
    combined=np.concatenate([qk,gg])
    require(np.array_equal(value_rows_to_hf(combined,offset=4096),np.concatenate([qk,original])),"QKV inverse affected Q/K")
    original_columns=original.T.copy()
    gg_columns=original_columns.reshape(3,48,128)[:,gg_to_hf].reshape(3,48*128)
    require(np.array_equal(value_columns_to_hf(gg_columns),original_columns),"output-column inverse failed")
    require(np.array_equal(value_rows_to_hf(gg_to_hf.astype(np.float32),width=1),np.arange(48,dtype=np.float32)),"scalar-head inverse failed")
    # HF repeats each key head three times; GGUF cycles over16 keys three times.
    require(np.array_equal((hf_to_gg%16),np.repeat(np.arange(16),3)),"grouped-head association differs")
    exact=np.array([0.25,0.5,0.75,1,1.5,2,4],dtype=np.float32)
    ordinary_norm_to_hf(exact)
    rejected=False
    try:ordinary_norm_to_hf(np.array([2**-25],dtype=np.float32))
    except ValueError:rejected=True
    require(rejected,"nonreversible norm was accepted")
    return {"value_rows":True,"QK_rows_unchanged":True,"output_columns":True,"scalar_heads":True,
            "grouped_key_association":True,"norm_exact_reconstruction":True,"nonreversible_norm_refused":True}
