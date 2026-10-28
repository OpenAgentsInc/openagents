"""Independent variable-time test references. Never use these with production keys."""
import ctypes as C
import hashlib
import json
import platform
from pathlib import Path

P=2**256-2**32-977
N=0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G=(0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
   0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)
HERE=Path(__file__).resolve().parent
DEFAULT_LIBRARY=HERE/'.cache/build'/('libsecp_oracle.dylib' if platform.system()=='Darwin' else 'libsecp_oracle.so')

def arithmetic(modulus,op,a,b=0):
    if not (type(a) is int and type(b) is int and 0<=a<2**256 and 0<=b<2**256): raise ValueError('expected unsigned 256-bit integers')
    if op=='normalize': return a%modulus
    if op=='add': return (a+b)%modulus
    if op=='sub': return (a-b)%modulus
    if op=='mul': return (a*b)%modulus
    if op=='negate': return -a%modulus
    if op=='inverse':
        if a%modulus==0: raise ValueError('zero has no multiplicative inverse')
        return pow(a,-1,modulus)
    raise ValueError('unsupported arithmetic operation')

def field(op,a,b=0): return arithmetic(P,op,a,b)
def scalar(op,a,b=0): return arithmetic(N,op,a,b)

def lift_x(x):
    if not 0<=x<P: return None
    y=pow((pow(x,3,P)+7)%P,(P+1)//4,P)
    if y*y%P!=(x*x*x+7)%P:return None
    return (x,y if y%2==0 else P-y)

def point_add(a,b):
    if a is None:return b
    if b is None:return a
    x,y=a;u,v=b
    if x==u and (y+v)%P==0:return None
    slope=((3*x*x)*pow(2*y,-1,P) if a==b else (v-y)*pow(u-x,-1,P))%P
    r=(slope*slope-x-u)%P
    return r,(slope*(x-r)-y)%P

def point_mul(k,point=G):
    if type(k) is not int or k<0:raise ValueError('nonnegative scalar required')
    result=None
    while k:
        if k&1:result=point_add(result,point)
        point=point_add(point,point);k>>=1
    return result

def encode_point(point):
    return None if point is None else bytes([2+(point[1]&1)])+point[0].to_bytes(32,'big')

def decode_point(data):
    if len(data)!=33 or data[0] not in (2,3):return None
    point=lift_x(int.from_bytes(data[1:],'big'))
    if point is None:return None
    x,y=point
    return (x,y if (y&1)==(data[0]&1) else P-y)

def raw_ecdh(secret,peer):
    if len(secret)!=32 or len(peer)!=32:return None
    k=int.from_bytes(secret,'big');point=lift_x(int.from_bytes(peer,'big'))
    if not 1<=k<N or point is None:return None
    result=point_mul(k,point)
    return None if result is None else result[0].to_bytes(32,'big')

def tagged(tag,message):
    digest=hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(digest+digest+message).digest()

def schnorr_verify(message,public,signature):
    if len(public)!=32 or len(signature)!=64:return False
    point=lift_x(int.from_bytes(public,'big'))
    r=int.from_bytes(signature[:32],'big');s=int.from_bytes(signature[32:],'big')
    if point is None or r>=P or s>=N:return False
    e=int.from_bytes(tagged('BIP0340/challenge',signature[:32]+public+message),'big')%N
    result=point_add(point_mul(s,G),point_mul((N-e)%N,point))
    return result is not None and result[1]%2==0 and result[0]==r

class NativeOracle:
    def __init__(self,path):
        self.path=Path(path).resolve()
        report=json.loads((self.path.parent/'build.json').read_text())
        pin=json.loads((HERE/'UPSTREAM.json').read_text())
        if (report['artifact_sha256']!=hashlib.sha256(self.path.read_bytes()).hexdigest()
            or report['source_commit']!=pin['commit'] or report['archive_sha256']!=pin['archive_sha256']
            or report['adapter_sha256']!=hashlib.sha256((HERE/'oracle.c').read_bytes()).hexdigest()):
            raise ValueError('native test-oracle artifact/provenance drift; rebuild explicitly after review')
        self.lib=C.CDLL(str(self.path))
        self.lib.oa_context.restype=C.c_void_p
        self.lib.oa_destroy.argtypes=[C.c_void_p]
        self.lib.oa_ecdh.argtypes=[C.c_void_p,C.c_void_p,C.c_void_p,C.c_void_p,C.c_int]
        self.lib.oa_point.argtypes=[C.c_void_p,C.c_void_p,C.c_void_p,C.c_size_t,C.c_void_p]
        self.lib.oa_verify.argtypes=[C.c_void_p,C.c_void_p,C.c_size_t,C.c_void_p,C.c_void_p]
        self.lib.oa_sign.argtypes=[C.c_void_p,C.c_void_p,C.c_size_t,C.c_void_p,C.c_void_p,C.c_void_p]
        self.context=self.lib.oa_context()
        if not self.context:raise RuntimeError('test context creation failed')
    def close(self):
        if self.context:self.lib.oa_destroy(self.context);self.context=None
    def verify(self,message,public,signature):
        if len(public)!=32 or len(signature)!=64 or len(message)>1048576:return False
        return bool(self.lib.oa_verify(self.context,C.c_char_p(message),len(message),C.c_char_p(public),C.c_char_p(signature)))
    def sign(self,message,secret,aux):
        if len(secret)!=32 or len(aux)!=32 or len(message)>1048576:return None
        out=C.create_string_buffer(64)
        return out.raw if self.lib.oa_sign(self.context,C.c_char_p(message),len(message),C.c_char_p(secret),C.c_char_p(aux),out) else None
    def point(self,secret,peer=b''):
        if len(secret)!=32 or len(peer) not in (0,33):return None
        out=C.create_string_buffer(33)
        return out.raw if self.lib.oa_point(self.context,C.c_char_p(secret),C.c_char_p(peer),len(peer),out) else None
    def ecdh(self,secret,peer,default_hash=False):
        if len(secret)!=32 or len(peer)!=32:return None
        out=C.create_string_buffer(32)
        return out.raw if self.lib.oa_ecdh(self.context,C.c_char_p(secret),C.c_char_p(peer),out,int(default_hash)) else None
