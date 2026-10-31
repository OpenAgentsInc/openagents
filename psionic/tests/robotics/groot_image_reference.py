#!/usr/bin/env python3
"""Firm reason2: independent existing OpenCV/NumPy preprocessing oracle.

Synthetic images qualify image arithmetic; no learned/robotic outcome is implied.
NVIDIA and OpenCV source was read only; no repository scripts are executed.
"""
import argparse
import ctypes as ct
import hashlib
import json
import math
from pathlib import Path
import cv2
import numpy as np

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/"psionic/lib/robotics/groot-image-check-fixtures.coax"

def raw_pattern(kind):
    i=np.arange(196608,dtype=np.int64)
    if kind==0: a=(i*17+(i//768)*37)%256
    elif kind==1: a=np.zeros_like(i)
    elif kind==2: a=np.full_like(i,255)
    elif kind==3: a=((i//3+i//768)%2)*255
    elif kind in (4,5):
        state=113 if kind==4 else 114;values=[]
        for _ in range(196608):
            state=(1664525*state+1013904223)&0xffffffff;values.append(state>>24)
        a=np.asarray(values,dtype=np.uint8)
    elif kind==6: a=(i//3)%256
    else: a=(i//768)%256
    return np.asarray(a,dtype=np.uint8).reshape(256,256,3)

def original(raw):
    flipped=raw[::-1,::-1]
    cropped=flipped[6:249,6:249]
    resized=cv2.resize(cropped,(256,256),interpolation=cv2.INTER_AREA)
    centered=resized.astype(np.float32)-np.float32(127.5)
    normalized=centered/np.float32(127.5)
    return np.ascontiguousarray(normalized.transpose(2,0,1)),resized

def axes():
    inv=256.0/243.0;scale=1.0/inv
    result=[]
    for d in range(256):
        lo=math.floor(d*scale)
        frac=np.float32((d+1)-(lo+1)*inv)
        frac=np.float32(0) if frac<=0 else np.float32(frac-math.floor(frac))
        a=int(np.rint(np.float32((np.float32(1)-frac)*np.float32(2048))))
        b=int(np.rint(np.float32(frac*np.float32(2048))))
        if lo>=242:a,b=2048,0
        result.append([lo,min(lo+1,242),a,b])
    return result

def author_fixtures():
    axis=axes()
    norm=((np.arange(256,dtype=np.float32)-np.float32(127.5))/np.float32(127.5)).view(np.uint32)
    expected,_=original(raw_pattern(0));flat=expected.ravel().view(np.uint32)
    samples=[]
    points=[0,1,7,8,13,17,63,127,128,241,254,255]
    for c in range(3):
        for y in points:
            for x in points:
                index=c*65536+y*256+x;samples.append([index,int(flat[index])])
    text="// Independent OpenCV4.11.0 / NumPy1.26.4 before implementation.\n"
    text+="GI_AXIS_GOLD :: [256][4]int.["+",".join("[4]int.["+",".join(map(str,a))+"]" for a in axis)+"];\n"
    text+="GI_NORM_GOLD :: [256]u32.["+",".join(map(str,norm.tolist()))+"];\n"
    text+="GI_PIXEL_GOLD :: ["+str(len(samples))+"][2]u32.["+",".join("[2]u32.["+",".join(map(str,s))+"]"for s in samples)+"];\n"
    FIXTURE.write_text(text)

class U8(ct.Structure):_fields_=[("data",ct.POINTER(ct.c_uint8)),("count",ct.c_int64)]
class F32(ct.Structure):_fields_=[("data",ct.POINTER(ct.c_float)),("count",ct.c_int64)]
class Read(ct.Structure):_fields_=[("status",ct.c_uint8),("written",ct.c_int64)]
def u8(x):return U8(x.ctypes.data_as(ct.POINTER(ct.c_uint8)),x.size)
def f32(x):return F32(x.ctypes.data_as(ct.POINTER(ct.c_float)),x.size)

def main():
    p=argparse.ArgumentParser();p.add_argument("--write-fixtures",action="store_true");p.add_argument("--library");p.add_argument("--write-metadata",type=Path);args=p.parse_args()
    assert cv2.__version__=="4.11.0" and np.__version__=="1.26.4"
    if args.write_fixtures:author_fixtures()
    if args.library:
        lib=ct.CDLL(str(Path(args.library).resolve()))
        lib.gi_preprocess.argtypes=[ct.c_int64,ct.c_int64,U8,F32,F32,ct.c_bool];lib.gi_preprocess.restype=Read
        words=0;records=[]
        for kind in range(8):
            raw=raw_pattern(kind);expected,resized=original(raw)
            out=np.full(196609,7,dtype=np.float32);work=np.full(196609,9,dtype=np.float32)
            result=lib.gi_preprocess(0,256,u8(raw),f32(out),f32(work),False)
            assert result.status==0 and result.written==196608 and out[-1]==7 and work[-1]==9
            a=out[:196608].view(np.uint32);e=expected.ravel().view(np.uint32)
            bad=np.flatnonzero(a!=e)
            if bad.size:
                i=int(bad[0]);raise AssertionError(f"pattern{kind} word{i}: {int(a[i]):08x} != {int(e[i]):08x}")
            words+=a.size
            records.append({"kind":kind,"raw_sha256":hashlib.sha256(raw.tobytes()).hexdigest(),"resized_u8_sha256":hashlib.sha256(resized.tobytes()).hexdigest(),"normalized_nchw_f32_le_sha256":hashlib.sha256(expected.astype('<f4').tobytes()).hexdigest()})
        metadata={"schema":"psionic-groot-image-reference-v1","date":"2026-10-31","opencv":cv2.__version__,"numpy":np.__version__,"images":8,"exact_f32_words":words,"recipe":"256x256RGBuint8; flipbothspatialaxes; crop[6:249,6:249]; resize256x256INTER_AREA; F32(pixel−127.5)/127.5; NCHW","source":"https://github.com/NVIDIA/Isaac-GR00T/blob/c56f54128d92f743889b7e4cecfc1ceafd88739e/gr00t/model/gr00t_n1d7/image_augmentations.py","resize_source":"https://github.com/opencv/opencv/blob/4.11.0/modules/imgproc/src/resize.cpp","scope":"Synthetic pixel qualification only; no learned model, simulator loop or robotics outcome","records":records}
        if args.write_metadata:args.write_metadata.write_text(json.dumps(metadata,indent=2)+"\n")
        print(f"GR00T image OpenCV{cv2.__version__}:8images,{words}exact F32words passed")

if __name__=="__main__":main()
