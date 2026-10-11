"""Replace only the three FFN matrices with original BF16 values cast to f32."""
import re
from gguf_import import StreamingGguf,require
from bf16_import import StreamingBf16


class HybridGguf:
    def __init__(self):
        self.quantized=StreamingGguf()
        self.original=StreamingBf16()
        self.tensors=self.quantized.tensors
        self.decoder_identity=self.quantized.decoder_identity
        self.state={'gguf':self.quantized.state,'original':self.original.state}
        self.matrix_sources=[]
        for layer in range(64):
            for operation in ('gate','up','down'):
                gg=f'blk.{layer}.ffn_{operation}.weight'
                hf=f'model.language_model.layers.{layer}.mlp.{operation}_proj.weight'
                require(tuple(reversed(self.quantized.tensors[gg]['shape']))==tuple(self.original.tensors[hf]['shape']),'replacement matrix shape differs')
                self.matrix_sources.append({'layer':layer,'operation':operation,'gguf_name':gg,'replaced_gguf_type':self.quantized.tensors[gg]['type'],
                    'source':'original_checkpoint','source_tensor':hf,'source_shard':self.original.tensors[hf]['shard'],'source_dtype':'BF16','execution_dtype':'F32',
                    'shape':self.original.tensors[hf]['shape']})
        self.replaced=[]

    def full(self,name):
        match=re.fullmatch(r'blk\.(\d+)\.ffn_(gate|up|down)\.weight',name)
        if match:
            layer=int(match[1]);require(0<=layer<64,'invalid FFN layer')
            self.replaced.append(name)
            return self.original.full(f'model.language_model.layers.{layer}.mlp.{match[2]}_proj.weight')
        return self.quantized.full(name)

    def rows(self,name,rows):return self.quantized.rows(name,rows)
    def check_state(self):self.quantized.check_state();self.original.check_state()
    def close(self):self.quantized.close();self.original.close()
    @property
    def payload_read_bytes(self):return self.quantized.payload_read_bytes+self.original.payload_read_bytes
