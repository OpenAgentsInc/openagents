#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python's existing KV v1 encoder/replayer compares native bounded reads of real full, damaged and zero-padded log files)
"""Bounded file loading agrees with the original independent wire model."""
import argparse
import importlib.util
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location("arena_wire",Path(__file__).with_name("kvskip-store.py"))
wire=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire);old=wire.old
DRIVER=r'''
#include "bridge.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void word(uint64_t v,int n){for(int i=0;i<n;i++)putchar((int)((v>>(8*i))&255));}
int main(int argc,char **argv){
 if(argc!=2)return 2;uint8_t *table=malloc(8388608),*out=malloc(8388608),*frame=malloc(2097152),*scratch=malloc(16384);uint64_t fields[7]={0};
 if(!table||!out||!frame||!scratch)return 3;
 int64_t code=kvs_file_c((coax_v_u8){(uint8_t*)argv[1],strlen(argv[1])},(coax_v_u8){table,8388608},(coax_v_u8){frame,2097152},(coax_v_u8){scratch,16384},(coax_v_u64){fields,7},(coax_v_u8){out,8388608});
 word((uint64_t)code,4);word(fields[3],8);word(fields[5],1);word(fields[1],8);if(!code)fwrite(out,1,fields[6],stdout);
 free(table);free(out);free(frame);free(scratch);return 0;
}
'''


def run(compiler,mode):
    with tempfile.TemporaryDirectory(prefix="coax-kvstream-") as temporary:
        d=Path(temporary);source=d/"stream.c";header=d/"bridge.h";driver=d/"driver.c";binary=d/"check"
        p=subprocess.run([str(compiler),"c","coax/tests/bitcoin/kvstream-native.coax","--library","--header",str(header),*(["--release"] if mode=="release" else [])],cwd=ROOT,capture_output=True,text=True,timeout=840)
        assert p.returncode==0,p.stderr;source.write_text(p.stdout);driver.write_text(DRIVER)
        flags=["-O1","-g","-fsanitize=address,undefined","-fno-omit-frame-pointer"] if mode=="sanitized" else ["-O2"]
        p=subprocess.run(["cc","-std=c11","-D_DARWIN_C_SOURCE",*flags,"-I"+str(d),"-I"+str(ROOT/"coax/platform"),str(source),str(driver),str(ROOT/"bitcoin/platform/explorer-files-native.c"),str(ROOT/"coax/platform/store-file-native.c"),"-lm","-o",str(binary)],capture_output=True,text=True,timeout=840)
        assert p.returncode==0,p.stderr
        batches,_=wire.fixture();cases=0
        for mask in (0,0x0123456789abcdef):
            frames=[old.frame(i,ops,mask) for i,ops in enumerate(batches,1)]
            journal=old.header(mask)+b"".join(frames);base=journal[:-len(frames[-1])]
            variants=[journal,*[base+frames[-1][:i] for i in range(len(frames[-1]))]]
            variants += [base+frames[-1][:i]+bytes(16390) for i in range(0,len(frames[-1]),13)]
            variants += [journal[:i] for i in range(20)]
            high=old.header(mask)+old.frame(2**64-1,[],mask)
            variants += [high,high+old.frame(1,[],mask),high+old.frame(2,[],mask)]
            for i in range(0,len(journal),max(1,len(journal)//40)):
                damaged=bytearray(journal);damaged[i]^=1;variants.append(bytes(damaged)+b"x")
            for data in variants:
                code,state,seq,keep,cut=old.replay(data);expected=struct.pack("<IQBQ",code,keep,cut,seq)
                if code==0:
                    expected+=old.header(mask)
                    if seq:expected+=old.frame(seq,[("put",k,v) for k,v in sorted(state.items())],mask)
                path=d/"journal";path.write_bytes(data);p=subprocess.run([str(binary),str(path)],capture_output=True,timeout=30)
                assert p.returncode==0,(p.returncode,p.stderr.decode());assert p.stdout==expected,(cases,code,seq,keep,cut,"bounded file replay differs")
                assert b"runtime error:" not in p.stderr and b"ERROR: AddressSanitizer" not in p.stderr,p.stderr
                assert path.read_bytes()==data,"a reader repaired the writer file";cases+=1
        print(f"bounded KV replay {mode}: {cases} independent full, torn, corrupt and zero-padded files; exact canonical snapshots and no reader mutation pass",flush=True)


if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("--coax",type=Path,required=True);p.add_argument("--mode",choices=("debug","release","sanitized"),default="debug");a=p.parse_args();run(a.coax.resolve(),a.mode)
