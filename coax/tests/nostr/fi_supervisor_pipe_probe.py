#!/usr/bin/env python3
"""Test-only reaper-independent framed resolver supervisor. No provider authority."""
import os,select,signal,subprocess,tempfile,threading,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
source=ROOT/'coax/tests/nostr/fixtures/fi_transport/fi_dns_supervisor_experiment.c'
SHIM=r'''#define _POSIX_C_SOURCE 200809L
#include <arpa/inet.h>
#include <netdb.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
static struct addrinfo rows[2];static struct sockaddr_in v4[2];
int fi_test_getaddrinfo(const char *host,const char *port,const struct addrinfo *hints,struct addrinfo **out){
 (void)port;(void)hints;
 if(!strcmp(host,"stall.fixture")){FILE *f=fopen(FI_MARKER,"w");if(f){fprintf(f,"%ld",(long)getpid());fclose(f);}struct timespec t={30,0};nanosleep(&t,0);return EAI_NONAME;}
 if(!strcmp(host,"forged.fixture")){unsigned char fake[28]={'F','D','R','1',1,4,1,187,8,8,8,8};write(1,fake,sizeof(fake));_exit(2);}
 if(!strcmp(host,"partial.fixture")){write(1,"FDR1",4);_exit(0);}
 if(strcmp(host,"good.fixture") && strcmp(host,"mixed.fixture"))return EAI_NONAME;
 struct timespec pause={0,100000000};nanosleep(&pause,0);int count=!strcmp(host,"mixed.fixture")?2:1;
 memset(rows,0,sizeof(rows));memset(v4,0,sizeof(v4));
 for(int i=0;i<count;i++){v4[i].sin_family=AF_INET;v4[i].sin_port=htons(443);inet_pton(AF_INET,i?"127.0.0.1":"8.8.8.8",&v4[i].sin_addr);rows[i].ai_family=AF_INET;rows[i].ai_socktype=SOCK_STREAM;rows[i].ai_protocol=IPPROTO_TCP;rows[i].ai_addr=(void *)&v4[i];rows[i].ai_addrlen=sizeof(v4[i]);if(i+1<count)rows[i].ai_next=&rows[i+1];}
 *out=rows;return 0;
}
void fi_test_freeaddrinfo(struct addrinfo *a){(void)a;}
'''
with tempfile.TemporaryDirectory(prefix='fi-supervisor-pipe-') as td:
 d=Path(td);shim=d/'shim.c';shim.write_text(SHIM)
 for mode in ('native','asan'):
  exe=d/('resolver-'+mode);marker=d/('marker-'+mode);obj=d/('supervisor-'+mode+'.o')
  flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'] if mode=='asan' else ['-O2']
  subprocess.run(['cc','-std=c11',*flags,'-Dgetaddrinfo=fi_test_getaddrinfo','-Dfreeaddrinfo=fi_test_freeaddrinfo','-c',str(source),'-o',str(obj)],check=True)
  subprocess.run(['cc','-std=c11',*flags,'-DFI_MARKER="'+str(marker)+'"',str(shim),str(obj),'-o',str(exe)],check=True)
  # Test-only source injection makes the supervisor exit itself after the
  # resolver starts. It sends no signal to a potentially recycled PID.
  crash_marker=d/('crash-marker-'+mode);crash_src=d/('crash-supervisor-'+mode+'.c');crash_obj=d/('crash-supervisor-'+mode+'.o');crash_exe=d/('crash-resolver-'+mode)
  original=source.read_text();needle='pid_t result=waitpid(pid,&status,WNOHANG);';assert original.count(needle)==1
  crash_src.write_text(original.replace(needle,'if(access(FI_CRASH_MARKER,F_OK)==0)_exit(99);'+needle))
  crash_shim=d/('crash-shim-'+mode+'.c');crash_shim.write_text(SHIM.replace('struct timespec t={30,0};','struct timespec t={2,0};'))
  subprocess.run(['cc','-std=c11',*flags,'-DFI_CRASH_MARKER="'+str(crash_marker)+'"','-Dgetaddrinfo=fi_test_getaddrinfo','-Dfreeaddrinfo=fi_test_freeaddrinfo','-c',str(crash_src),'-o',str(crash_obj)],check=True)
  subprocess.run(['cc','-std=c11',*flags,'-DFI_MARKER="'+str(crash_marker)+'"',str(crash_shim),str(crash_obj),'-o',str(crash_exe)],check=True)
  signal.signal(signal.SIGCHLD,signal.SIG_DFL);stopped=threading.Event();reaped=[];guard=threading.Lock()
  def host_reaper():
   while not stopped.is_set():
    try:
     while True:
      pid,status=os.waitpid(-1,os.WNOHANG)
      if not pid:break
      with guard:reaped.append(pid)
    except ChildProcessError:pass
    time.sleep(.001)
  thread=threading.Thread(target=host_reaper);thread.start()
  def has_reaped(pid):
   with guard:return pid in reaped
  def reap_observed(pid):
   until=time.monotonic()+2
   while time.monotonic()<until:
    if has_reaped(pid):return
    time.sleep(.002)
   raise AssertionError(('host reaper did not observe supervisor',pid))
  def begin(host,seconds=3,executable=exe):
   r,w=os.pipe();control_r,control_w=os.pipe();deadline=time.monotonic_ns()+int(seconds*1_000_000_000)
   assert min(r,w,control_r,control_w)>2
   actions=[(os.POSIX_SPAWN_DUP2,w,1),(os.POSIX_SPAWN_DUP2,control_r,0),
            (os.POSIX_SPAWN_CLOSE,r),(os.POSIX_SPAWN_CLOSE,w),
            (os.POSIX_SPAWN_CLOSE,control_r),(os.POSIX_SPAWN_CLOSE,control_w)]
   pid=os.posix_spawn(str(executable),[str(executable),host,'443',str(deadline)],{},file_actions=actions)
   os.close(w);os.close(control_r);os.set_blocking(r,False);return r,control_w,pid
  def receive(r):
   result=b'';until=time.monotonic()+4
   while time.monotonic()<until:
    ready,_,_=select.select([r],[],[],.1)
    if not ready:continue
    b=os.read(r,512)
    if not b:os.close(r);return result
    result+=b
   os.close(r);raise AssertionError('supervisor response stalled')
  try:
   # The host reaper, not this consumer, reaps these supervisor children.
   r,control,pid=begin('good.fixture');data=receive(r);os.close(control);reap_observed(pid)
   assert len(data)==28 and data[:5]==b'FDR1\x01' and data[5]==4 and data[8:12]==b'\x08\x08\x08\x08',data
   r,control,pid=begin('mixed.fixture');data=receive(r);os.close(control);reap_observed(pid)
   assert len(data)==51 and data[:5]==b'FDR1\x02' and data[31:35]==b'\x7f\x00\x00\x01',data
   # A complete-looking frame from a child that fails, and a partial frame
   # from a child that exits successfully, must yield only the failure terminal.
   for host in ('forged.fixture','partial.fixture','missing.fixture'):
    r,control,pid=begin(host);assert receive(r)==b'FDE1',host;os.close(control);reap_observed(pid)
   r,control,pid=begin('stall.fixture',5);until=time.monotonic()+2
   while not marker.exists() and time.monotonic()<until:time.sleep(.002)
   assert marker.exists(),'stalled resolver did not start';child=int(marker.read_text());start=time.monotonic();os.close(control)
   assert receive(r)==b'FDE1','cancelled resolver did not produce only the cleanup terminal'
   reap_observed(pid)
   until=time.monotonic()+2
   while time.monotonic()<until:
    try:os.kill(child,0)
    except ProcessLookupError:break
    time.sleep(.002)
   else:raise AssertionError('stalled resolver still exists')
   assert time.monotonic()-start<2
   # Counterexample: EOF without a terminal can occur while the resolver is
   # still alive. A receiver must quarantine its slot rather than reuse it.
   r,control,pid=begin('stall.fixture',5,crash_exe);data=receive(r);os.close(control);reap_observed(pid)
   assert data==b'' and crash_marker.exists(),'crashed supervisor produced cleanup evidence'
   orphan=int(crash_marker.read_text());os.kill(orphan,0)
   until=time.monotonic()+4
   while time.monotonic()<until:
    try:os.kill(orphan,0)
    except ProcessLookupError:break
    time.sleep(.002)
   else:raise AssertionError('test orphan did not exit on its own')
   print(mode,'host reaper consumed supervisors; framed success, failed child, partial output and cancellation/EOF passed',flush=True)
  finally:
   stopped.set();thread.join()
