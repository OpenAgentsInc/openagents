#!/usr/bin/env python3
"""Independent trace oracle for the untrusted DNS callback observation gate."""
import ctypes as c
import itertools
import os
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3]
compiler=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
source=ROOT/'coax/lib/nostr/block/fi-dns-observation.coax'
class State(c.Structure): _fields_=[('status',c.c_int64),('count',c.c_int64)]
class Bytes(c.Structure): _fields_=[('data',c.POINTER(c.c_uint8)),('count',c.c_int64)]
addresses={'public4':bytes([8,8,8,8]),'public6':bytes.fromhex('26064700000000000000000000000001'),
 'private':bytes([127,0,0,1]),'mapped':bytes.fromhex('00000000000000000000ffff08080808'),'short':bytes([8,8,8])}
events=tuple(addresses)+('cancel','fail')
def model(state,event):
 status,count=state
 if event=='cancel': return state if status in (1,2) else (2,0)
 if event=='fail': return state if status==2 else (1,0)
 if status==2:return state
 if status==1:return (1,0)
 if count>=16 or event in ('private','mapped','short'):return (1,0)
 return (0,count+1)
with tempfile.TemporaryDirectory(prefix='fi-dns-observation-') as td:
 d=Path(td)
 for mode in ('checked','release'):
  cmd=[str(compiler),'c',str(source),'--library','-o',str(d/'generated.c')]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True)
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(d/'generated.c'),'-lm','-o',str(lib)],check=True)
  api=c.CDLL(str(lib));api.fi_dns_observation_start.restype=State
  for fn in ('fi_dns_observation_cancel','fi_dns_observation_fail'):
   getattr(api,fn).argtypes=[State];getattr(api,fn).restype=State
  api.fi_dns_observation_add.argtypes=[State,Bytes];api.fi_dns_observation_add.restype=State
  for trace in itertools.product(events,repeat=4):
   s=api.fi_dns_observation_start();expected=(0,0)
   for event in trace:
    expected=model(expected,event)
    if event in addresses:
     raw=addresses[event];buf=(c.c_uint8*len(raw)).from_buffer_copy(raw)
     s=api.fi_dns_observation_add(s,Bytes(buf,len(raw)))
    else:s=getattr(api,'fi_dns_observation_'+event)(s)
    assert (s.status,s.count)==expected,(mode,trace,event,(s.status,s.count),expected)
  s=api.fi_dns_observation_start();buf=(c.c_uint8*4)(8,8,8,8)
  for n in range(17):
   s=api.fi_dns_observation_add(s,Bytes(buf,4));assert (s.status,s.count)==((0,n+1) if n<16 else (1,0))
  print(mode, f'{len(events)**4} independently modeled four-event traces and 17-event bound passed')
