#!/usr/bin/env python3
"""Tooling only: a real NumPy DLPack producer/consumer and independent weights reader.
No installed safetensors or training framework is required by Psionic at runtime.
"""
import ctypes as C
import gc
import json
from pathlib import Path
import struct
import sys
import weakref
import numpy as np

class Tensor(C.Structure):
    _fields_ = [('version',C.c_uint32),('dtype',C.c_uint32),('rank',C.c_int32),('flags',C.c_uint32),('dims',C.c_int64*4),('data',C.c_void_p),('bytes',C.c_uint64)]
class Info(C.Structure):
    _fields_ = [('name',C.c_char*128),('name_bytes',C.c_uint32),('dtype',C.c_uint32),('rank',C.c_int32),('reserved',C.c_uint32),('dims',C.c_int64*4),('offset',C.c_uint64),('bytes',C.c_uint64)]
class Parameter(C.Structure):
    _fields_ = [('name',C.c_char_p),('name_bytes',C.c_uint64),('tensor',Tensor)]
class Result(C.Structure):
    _fields_ = [('status',C.c_uint32),('reserved',C.c_uint32),('written',C.c_uint64)]
class Fit(C.Structure):
    _fields_ = [('status',C.c_uint32),('weight',C.c_float),('loss',C.c_float)]
assert C.sizeof(Tensor)==64 and C.sizeof(Info)==192 and C.sizeof(Result)==16
lib=C.PyDLL(sys.argv[1]) # NumPy producer deleters require the GIL.
def bind(name,ret,*args):
    f=getattr(lib,name); f.restype=ret; f.argtypes=args; return f
inspect=bind('psionic_weights_inspect_v1',Result,C.c_void_p,C.c_uint64,C.POINTER(Info),C.c_uint64)
write=bind('psionic_weights_write_v1',Result,C.POINTER(Parameter),C.c_uint64,C.c_void_p,C.c_uint64)
fit=bind('ic_continue',Fit,C.c_float,C.c_int64) # private Coax example, outside stable ABI
consume=bind('psionic_dlpack_import_v1',C.c_uint32,C.c_void_p,C.c_uint32,C.POINTER(C.c_void_p))
view=bind('psionic_tensor_view_v1',C.c_uint32,C.c_void_p,C.POINTER(Tensor))
copy=bind('psionic_tensor_copy_v1',C.c_uint32,C.POINTER(Tensor),C.POINTER(C.c_void_p))
release=bind('psionic_tensor_release_v1',C.c_uint32,C.POINTER(C.c_void_p))
export=bind('psionic_tensor_dlpack_v1',C.c_uint32,C.c_void_p,C.POINTER(C.c_void_p))
dispose=bind('psionic_dlpack_dispose_v1',None,C.c_void_p)
live=bind('psionic_tensor_live_v1',C.c_uint64)
api=C.pythonapi
get=api.PyCapsule_GetPointer;get.argtypes=[C.c_void_p,C.c_char_p];get.restype=C.c_void_p
rename=api.PyCapsule_SetName;rename.argtypes=[C.c_void_p,C.c_char_p];rename.restype=C.c_int
valid=api.PyCapsule_IsValid;valid.argtypes=[C.c_void_p,C.c_char_p];valid.restype=C.c_int
new=api.PyCapsule_New;new.argtypes=[C.c_void_p,C.c_char_p,C.c_void_p];new.restype=C.py_object
# Global byte strings live longer than every capsule. Destructors receive a raw
# PyObject address; converting it back to py_object would alter its refcount.
name=b'dltensor_versioned';used=b'used_dltensor_versioned'
@C.CFUNCTYPE(None,C.c_void_p)
def destructor(capsule):
    if valid(capsule,name): dispose(get(capsule,name))

def descriptor(a):
    return Tensor(1,0,a.ndim,0,(C.c_int64*4)(*a.shape),a.ctypes.data,a.nbytes)

def import_numpy(versioned):
    a=np.arange(6,dtype=np.float32).reshape(2,3).copy()
    ptr=a.ctypes.data;ref=weakref.ref(a)
    cap=a.__dlpack__(max_version=(1,0)) if versioned else a.__dlpack__()
    n=name if versioned else b'dltensor';u=used if versioned else b'used_dltensor'
    carrier=get(id(cap),n);assert carrier
    assert rename(id(cap),u)==0
    h=C.c_void_p();assert consume(carrier,int(versioned),C.byref(h))==0
    d=Tensor();assert view(h,C.byref(d))==0 and d.data==ptr and d.bytes==24
    del a,cap;gc.collect();assert ref() is not None
    assert list((C.c_float*6).from_address(d.data))==[0,1,2,3,4,5]
    assert release(C.byref(h))==0 and not h.value
    gc.collect();assert ref() is None and live()==0

class Exported:
    def __init__(self,ptr): self.ptr=ptr
    def __dlpack_device__(self):return (1,0)
    def __dlpack__(self,stream=None,max_version=None,dl_device=None,copy=None):
        assert self.ptr and stream is None and copy is not True
        p=self.ptr;self.ptr=None;return new(p,name,destructor)
    def __del__(self):
        if self.ptr: dispose(self.ptr)

for v in (False,True):import_numpy(v)
a=np.array([[1,2,3],[4,5,6]],dtype=np.float32)
h=C.c_void_p();assert copy(C.byref(descriptor(a)),C.byref(h))==0
owned=Tensor();assert view(h,C.byref(owned))==0
p=C.c_void_p();assert export(h,C.byref(p))==0
assert release(C.byref(h))==0 and live()==1
b=np.from_dlpack(Exported(p.value));assert b.ctypes.data==owned.data and not b.flags.writeable
np.testing.assert_array_equal(a,b)
del b;gc.collect();assert live()==0
# An unconsumed capsule also releases its export lease.
assert copy(C.byref(descriptor(a)),C.byref(h))==0
p=C.c_void_p();assert export(h,C.byref(p))==0;assert release(C.byref(h))==0
cap=new(p.value,name,destructor);del cap;gc.collect();assert live()==0

# Independent external producer: architecture is y=w*x; the artifact carries
# only a named weight. No object deserialization or original framework runs.
w=np.array([0.5],dtype='<f4')
header=json.dumps({'scale.weight':{'dtype':'F32','shape':[1],'data_offsets':[0,4]}},separators=(',',':')).encode()
header+=b' '*((-len(header))%8)
original=struct.pack('<Q',len(header))+header+w.tobytes()
file=C.create_string_buffer(original);info=(Info*16)()
r=inspect(file,len(original),info,16);assert r.status==0 and r.written==1
assert info[0].name==b'scale.weight' and info[0].bytes==4 and info[0].rank==1 and info[0].dims[0]==1 and info[0].dtype==0
initial=struct.unpack_from('<f',original,info[0].offset)[0]
trained=fit(initial,16);assert not trained.status and abs(trained.weight-2)<1e-6 and trained.loss<1e-11
learned=np.array([trained.weight],dtype=np.float32)
param=Parameter(b'scale.weight',12,descriptor(learned));out=C.create_string_buffer(1024)
r=write(C.byref(param),1,out,len(out));assert not r.status
portable=out.raw[:r.written];n=struct.unpack_from('<Q',portable)[0];meta=json.loads(portable[8:8+n]);t=meta['scale.weight']
assert t['dtype']=='F32' and t['shape']==[1] and t['data_offsets']==[0,4]
reloaded=np.frombuffer(portable,dtype='<f4',count=1,offset=8+n)
np.testing.assert_allclose(reloaded*np.array([-3,0,3],dtype=np.float32),[-6,0,6],rtol=0,atol=1e-6)
Path(sys.argv[2]).write_bytes(portable)
assert live()==0
print(f'NumPy {np.__version__}: legacy/versioned DLPack zero-copy and lifetime checks pass')
print(f'external weights -> Coax continuation -> portable weights -> NumPy: weight={trained.weight:.9g}, loss={trained.loss:.9g}')
