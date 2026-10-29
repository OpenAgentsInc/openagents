#!/usr/bin/env python3
"""Exercise real HTTPS transport and conservative refusals; no provider authority."""
import ctypes as c
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
SOURCE=ROOT/'coax/lib/nostr/block/fi-jwks.coax'
ADAPTER=ROOT/'coax/tests/nostr/fixtures/fi_transport/fi_fetch.c'

with tempfile.TemporaryDirectory(prefix='fi-fetch-') as d:
 d=Path(d);gen=d/'fi_transport_generated.c';header=d/'fi_transport_generated.h';lib=d/'fetch.dylib'
 cmd=[str(COMPILER),'c',str(SOURCE),'--library','--header',str(header),'-o',str(gen)]
 if os.environ.get('FI_POLICY_RELEASE')=='1':cmd.append('--release')
 subprocess.run(cmd,check=True)
 fixture=ROOT/'coax/tests/nostr/fi_fetch_resolver_fixture.c'
 helper=ROOT/'coax/tests/nostr/fixtures/fi_transport/fi_dns_worker.c';helper_bin=d/'fi-dns-worker'
 helper_obj=d/'helper.o'
 subprocess.run(['cc','-std=c11','-O2','-Dgetaddrinfo=fi_test_getaddrinfo','-Dfreeaddrinfo=fi_test_freeaddrinfo','-c',str(helper),'-o',str(helper_obj)],check=True)
 subprocess.run(['cc','-DFI_TEST_PID_FILE="'+str(d/'resolver.pid')+'"',str(helper_obj),str(fixture),'-o',str(helper_bin)],check=True)
 subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),'-DFI_DNS_HELPER_PATH="'+str(helper_bin)+'"','-DFI_FETCH_TEST_UNSAFE_REAPER=1',str(gen),str(ADAPTER),'-lcurl','-lpthread','-lm','-o',str(lib)],check=True)
 unit=ROOT/'coax/tests/nostr/fi_fetch_sockaddr_fixture.c';executable=d/'sockaddr-sanitized'
 subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),'-DFI_DNS_HELPER_PATH="'+str(helper_bin)+'"',str(gen),str(unit),'-lcurl','-lpthread','-lm','-o',str(executable)],check=True)
 subprocess.run([str(executable)],check=True)
 asan=ROOT/'coax/tests/nostr/fi_fetch_sanitized.c';asan_exe=d/'fetch-sanitized'
 subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),'-DFI_DNS_HELPER_PATH="'+str(helper_bin)+'"','-DFI_FETCH_TEST_UNSAFE_REAPER=1',str(gen),str(ADAPTER),str(asan),'-lcurl','-lpthread','-lm','-o',str(asan_exe)],check=True)
 subprocess.run([str(asan_exe)],check=True,timeout=20)
 os.environ['HTTPS_PROXY']='http://127.0.0.1:1';os.environ['https_proxy']='http://127.0.0.1:1';os.environ['ALL_PROXY']='http://127.0.0.1:1';os.environ['NO_PROXY']='';os.environ['no_proxy']=''
 api=c.CDLL(str(lib));api.fi_fetch_start.argtypes=[c.c_char_p,c.c_int64,c.c_int64,c.c_int64];api.fi_fetch_start.restype=c.c_void_p
 api.fi_fetch_poll.argtypes=[c.c_void_p,c.POINTER(c.c_uint8),c.c_int64,c.POINTER(c.c_int64)];api.fi_fetch_poll.restype=c.c_int
 api.fi_fetch_free.argtypes=[c.c_void_p]
 def fetch(uri,transport=1,timeout=9999,retry_start=False):
  b=uri.encode();handle=api.fi_fetch_start(b,len(b),transport,timeout)
  if retry_start:
   until=time.monotonic()+1
   while not handle and time.monotonic()<until:
    time.sleep(.005);handle=api.fi_fetch_start(b,len(b),transport,timeout)
  if not handle:return (2,b'')
  buf=(c.c_uint8*524288)();n=c.c_int64()
  try:
   until=time.monotonic()+12
   while time.monotonic()<until:
    status=api.fi_fetch_poll(handle,buf,len(buf),c.byref(n))
    if status:return status,bytes(buf[:n.value])
    time.sleep(.005)
   raise AssertionError('poll did not finish before bounded wait')
  finally:api.fi_fetch_free(handle)
 for uri in ['http://example.com/','https://127.0.0.1/','https://user@example.com/','https://example.com/#fragment','not-a-uri']:
  assert fetch(uri)[0]==2,uri
 assert fetch('https://example.com/',transport=2)[0]==2
 assert fetch('https://example.com/',timeout=0)[0]==2
 assert fetch('https://example.com/',timeout=10000)[0]==2
 for host in ['mixed.fixture','oversize.fixture','family.fixture','mapped.fixture','loopback.fixture','short.fixture','wrongfam.fixture']:
  assert fetch('https://'+host+'/')[0]==2,host
 before=time.monotonic();assert fetch('https://stall.fixture/',timeout=50)[0]==2;assert time.monotonic()-before<.2
 # The helper sleeps 30 seconds: it must be killed and reaped promptly.
 pid=int((d/'resolver.pid').read_text());until=time.monotonic()+1
 while time.monotonic()<until:
  try: os.kill(pid,0)
  except ProcessLookupError: break
  time.sleep(.005)
 else: raise AssertionError('resolver child survived or was not reaped')
 status,body=fetch('https://ipv4.icanhazip.com/',retry_start=True);assert status==1 and body
 before=time.monotonic();assert fetch('https://example.com/',timeout=1)[0]==2;assert time.monotonic()-before<2
 # A timed-out worker can remain in the resolver; retry only once it exits.
 time.sleep(.1)
 status,body=fetch('https://ipv4.icanhazip.com/')
 assert status==1 and b'.' in body,(status,body[:100])
 # A too-short output never receives partial data; the same completed result can be copied later.
 b=b'https://ipv4.icanhazip.com/';handle=api.fi_fetch_start(b,len(b),1,9999);assert handle
 try:
  small=(c.c_uint8*1)(165);n=c.c_int64(123);until=time.monotonic()+10
  while time.monotonic()<until:
   status=api.fi_fetch_poll(handle,small,1,c.byref(n))
   if status:break
   time.sleep(.005)
  assert status==2 and n.value==0 and small[0]==165
  buf=(c.c_uint8*524288)();assert api.fi_fetch_poll(handle,buf,len(buf),c.byref(n))==1 and n.value>1
 finally:api.fi_fetch_free(handle)
 # These live endpoints exercise actual certificate, redirect, body and deadline effects.
 for uri in ['https://wrong.host.badssl.com/','https://expired.badssl.com/']:
  # Connectivity control: the same live endpoint responds if certificate checking is disabled.
  subprocess.run(['curl','-q','--noproxy','*','--insecure','--max-time','5','--silent','--show-error','--output',os.devnull,uri],check=True)
  assert fetch(uri)[0]==2,uri
 assert fetch('https://httpbin.org/redirect/1')[0]==2
 status,body=fetch('https://httpbin.org/stream-bytes/524288?chunk_size=1024');assert status==1 and len(body)==524288,(status,len(body))
 assert fetch('https://httpbin.org/stream-bytes/524289?chunk_size=1024')[0]==2
 before=time.monotonic();assert fetch('https://httpbin.org/delay/3',timeout=200)[0]==2;assert time.monotonic()-before<1.5
 # Resolver/transfer cancellation can occupy the single worker briefly after poll.
 time.sleep(.4)
 status,_=fetch('https://example.com/',transport=0);assert status==2
 print('native DNS fault cases, actual TLS/redirect/stream/deadline and proxy refusals passed')
