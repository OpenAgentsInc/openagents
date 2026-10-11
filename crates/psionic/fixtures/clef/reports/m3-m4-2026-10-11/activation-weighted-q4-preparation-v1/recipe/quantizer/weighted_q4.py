"""Bounded adapter to the exact previously built upstream weighted-Q4 quantizer."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import sys
import numpy as np

BASE=Path(__file__).resolve().parents[2]
MANIFEST=BASE/'q4-down-q6-regeneration/helper-build-2/build/bridge.json'
REVISION='62a6f74bab748b1755f4cd8dfc0871caf865c6c8'


def require(value,message):
    if not value:raise ValueError(message)


def sha(raw):return hashlib.sha256(raw).hexdigest()


def symbol_image(function):
    class DlInfo(ctypes.Structure):
        _fields_=[('path',ctypes.c_char_p),('base',ctypes.c_void_p),('symbol',ctypes.c_char_p),('address',ctypes.c_void_p)]
    call=ctypes.CDLL(None).dladdr;call.argtypes=[ctypes.c_void_p,ctypes.POINTER(DlInfo)];call.restype=ctypes.c_int
    result=DlInfo();require(call(ctypes.cast(function,ctypes.c_void_p),ctypes.byref(result))!=0 and result.path,'cannot locate upstream symbol')
    path=Path(os.fsdecode(result.path)).resolve()
    return {'path':str(path),'sha256':sha(path.read_bytes())}


class WeightedQ4:
    def __init__(self):
        require(os.environ.get('CLEF_ACTIVATION_GUARDED')=='1','requires bounded owned-process guard')
        require(not any(k.startswith('DYLD_') or k in {'LD_PRELOAD','LD_LIBRARY_PATH'} for k in os.environ),'library overrides remain')
        self.manifest=json.loads(MANIFEST.read_text())
        require(self.manifest['status']=='built' and self.manifest['upstream_revision']==REVISION,'upstream build identity differs')
        path=Path(self.manifest['bridge']);require(sha(path.read_bytes())==self.manifest['bridge_sha256'],'bridge file changed')
        self.library=ctypes.CDLL(str(path.resolve()))
        self.function=self.library.ggml_quantize_chunk
        self.function.argtypes=[ctypes.c_int,ctypes.c_void_p,ctypes.c_void_p,ctypes.c_int64,ctypes.c_int64,ctypes.c_int64,ctypes.c_void_p]
        self.function.restype=ctypes.c_size_t
        self.decode=self.library.dequantize_row_q4_K
        self.decode.argtypes=[ctypes.c_void_p,ctypes.c_void_p,ctypes.c_int64];self.decode.restype=None
        self.valid=self.library.ggml_validate_row_data
        self.valid.argtypes=[ctypes.c_int,ctypes.c_void_p,ctypes.c_size_t];self.valid.restype=ctypes.c_bool
        expected={str(Path(p).resolve()):v for p,v in self.manifest['runtime_libraries'].items()}
        self.images={name:symbol_image(fn) for name,fn in [('quantize',self.function),('decode',self.decode),('validate',self.valid)]}
        for value in self.images.values():require(expected.get(value['path'])==value['sha256'],'loaded upstream image differs')
        self.identity={'revision':REVISION,'manifest_sha256':sha(MANIFEST.read_bytes()),'images':self.images,'adapter_sha256':sha(Path(__file__).read_bytes())}

    def quantize(self,source,importance):
        require(isinstance(source,np.ndarray) and source.dtype==np.float32 and source.ndim==2,'source must be a f32 matrix')
        rows,width=source.shape
        require(1<=rows<=64 and width in (5120,17408),'source shape outside bounded real FFN layouts')
        require(isinstance(importance,np.ndarray) and importance.dtype==np.float32 and importance.shape==(width,),'importance must have one f32 value per input column')
        require(np.isfinite(source).all() and np.isfinite(importance).all() and np.all(importance>=0),'nonfinite or negative quantization input')
        source=np.ascontiguousarray(source);importance=np.ascontiguousarray(importance)
        source_before=source.tobytes();importance_before=importance.tobytes()
        size=rows*(width//256)*144;storage=np.full(size+128,0xa5,dtype=np.uint8);packed=storage[64:-64]
        require(self.function(12,source.ctypes.data,packed.ctypes.data,0,rows,width,importance.ctypes.data)==size,'upstream Q4 quantizer returned wrong size')
        require(source.tobytes()==source_before and importance.tobytes()==importance_before,'upstream changed input storage')
        require(np.all(storage[:64]==0xa5) and np.all(storage[-64:]==0xa5),'upstream changed output guard')
        require(self.valid(12,packed.ctypes.data,size),'invalid packed Q4 row data')
        return packed.copy()

    def decode_packed(self,packed,width):
        require(packed.dtype==np.uint8 and packed.ndim==1 and width in (5120,17408),'invalid packed data shape')
        size=(width//256)*144;require(len(packed)%size==0 and 1<=len(packed)//size<=64,'packed rows exceed bound')
        values=len(packed)//144*256
        storage=np.full(values+32,0x7fc00555,dtype=np.uint32);result=storage[16:-16].view(np.float32)
        before=packed.tobytes();self.decode(packed.ctypes.data,result.ctypes.data,values)
        require(packed.tobytes()==before and np.all(storage[:16]==0x7fc00555) and np.all(storage[-16:]==0x7fc00555),'decoder changed input or guard')
        require(np.isfinite(result).all(),'decoder produced nonfinite values')
        return result.copy()
