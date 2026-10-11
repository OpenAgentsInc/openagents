"""Read original checkpoint weights through bounded pread, one layer at a time."""
import json
import math
import os
from pathlib import Path
import struct
import numpy as np
from gguf_import import BASE,require,sha_bytes,stat
HF=BASE/'artifacts/clef-bf16'
CLEAN=BASE/'reference27b/cpu-head-stage1/results'


class StreamingBf16:
    def __init__(self,path=HF):
        self.path=Path(path)
        reference=json.loads((CLEAN/'reference.json').read_text())
        index_path=self.path/'model.safetensors.index.json'
        index_bytes=index_path.read_bytes()
        require(sha_bytes(index_bytes)==reference['index_sha256'],'checkpoint tensor index changed')
        self.index=json.loads(index_bytes)['weight_map']
        self.index_state=stat(index_path)
        self.states={};self.headers={};self.tensors={};self.payload_read_bytes=0
        for shard in sorted(set(self.index.values())):
            require(Path(shard).name==shard and shard in reference['shards'],'unexpected shard path')
            p=self.path/shard;self.states[shard]=stat(p)
            previous=reference["artifacts"][shard]
            expected=[previous[k] for k in ("bytes","inode","mtime_ns","ctime_ns")]
            require(self.states[shard]==expected,"checkpoint shard changed since clean verified reference: "+shard)
            with p.open('rb') as f:
                count=struct.unpack('<Q',f.read(8))[0]
                require(0<count<=16*1024**2,'checkpoint header too large')
                raw=f.read(count);require(len(raw)==count,'truncated header')
            header=json.loads(raw)
            self.headers[shard]={'first':8+count,'sha256':sha_bytes(raw)}
            for name,item in header.items():
                if name=='__metadata__':continue
                require(self.index.get(name)==shard,'tensor shard binding differs')
                require(item['dtype']=='BF16','expected original BF16 tensors: '+name)
                first,end=item['data_offsets'];size=math.prod(item['shape'])*2
                require(0<=first<end and end-first==size and end<=self.states[shard][0]-(8+count),'invalid tensor extent')
                self.tensors[name]={**item,'shard':shard}
        require(set(self.tensors)==set(self.index),'tensor header/index mismatch')
        self.state={'index':self.index_state,'shards':self.states,'header_sha256':{k:v['sha256'] for k,v in self.headers.items()},
                    'matched_prior_verified_size_inode_mtime_ctime':True,
                    'prior_shard_sha256':{s:reference['artifacts'][s]['sha256'] for s in self.states},
                    'hash_rechecked_this_run':False,'historical_reference_sha256':sha_bytes((CLEAN/'reference.json').read_bytes())}

    def check_state(self):
        require(stat(self.path/'model.safetensors.index.json')==self.index_state,'checkpoint index changed')
        for shard,state in self.states.items():require(stat(self.path/shard)==state,'checkpoint shard changed')

    def flat(self,name,first=0,count=None):
        item=self.tensors[name];total=math.prod(item['shape']);count=total-first if count is None else count
        require(0<=first and 0<count<=total-first and count*4<=512*1024**2,'decoded tensor slice exceeds bound')
        result=np.empty(count,dtype=np.float32)
        fd=os.open(self.path/item['shard'],os.O_RDONLY)
        try:
            for start in range(0,count,1024**2):
                amount=min(1024**2,count-start)
                offset=self.headers[item['shard']]['first']+item['data_offsets'][0]+2*(first+start)
                raw=os.pread(fd,amount*2,offset);self.payload_read_bytes+=len(raw)
                require(len(raw)==2*amount,'short checkpoint read')
                values=(np.frombuffer(raw,dtype='<u2').astype(np.uint32)<<16).view(np.float32)
                require(np.isfinite(values).all(),'nonfinite checkpoint tensor')
                result[start:start+amount]=values
        finally:os.close(fd)
        return result

    def full(self,name):return self.flat(name).reshape(self.tensors[name]['shape'])

    def rows(self,name,rows):
        shape=self.tensors[name]['shape']
        require(len(shape)==2 and 0<len(rows)<=2048 and all(0<=r<shape[0] for r in rows),'row gather outside bound')
        return np.stack([self.flat(name,r*shape[1],shape[1]) for r in rows])

    def layer_arrays(self,index):
        prefix=f'model.language_model.layers.{index}.'
        result={name[len(prefix):]:self.full(name) for name in sorted(self.tensors) if name.startswith(prefix)}
        require(result and sum(v.nbytes for v in result.values())<2*1024**3,'layer resident weights exceed2GiB')
        return result

    def close(self):pass

    def plan(self,tokens):
        layers=[]
        for index in range(64):
            prefix=f'model.language_model.layers.{index}.'
            weights=[item for name,item in self.tensors.items() if name.startswith(prefix)]
            layers.append({'layer':index,'weight_f32_bytes':sum(math.prod(w['shape'])*4 for w in weights),'dense_gemm_flops':2*tokens*sum(math.prod(w['shape']) for w in weights if len(w['shape'])==2)})
        return {'layers':layers,'maximum_layer_f32_bytes':max(l['weight_f32_bytes'] for l in layers),
                'total_dense_gemm_flops':sum(l['dense_gemm_flops'] for l in layers),'input_tokens':tokens,
                'largest_payload_read_bytes':2*1024**2,'maximum_single_tensor_f32_bytes':512*1024**2,
                'owned_group_cap_gib':12,'start_available_gib':24,'available_floor_gib':12,'threads':2,
                'numerical_scope':'Original BF16 checkpoint values cast exactly to f32; unmodified official CPU operators, original A_log and norm weights.',
                'runtime_estimate':'GGUF same-case pass took141s. BF16 uses ~52GB sequential reads instead of17GB quantized reads, with no quantization decode; expected a few minutes, deadline1800s.'}
