#!/usr/bin/env python3
"""Untrusted lifecycle fixture; no authenticated provider or FI authority."""
import ast
import os
from pathlib import Path
import subprocess
import tempfile
import time
ROOT=Path(__file__).resolve().parents[3]
supervisor=ROOT/'coax/tests/nostr/fixtures/fi_transport/fi_dns_supervisor_experiment.c'
client=ROOT/'coax/tests/nostr/fixtures/fi_transport/fi_pipe_client_experiment.c'
fixture=ROOT/'coax/tests/nostr/fi_pipe_client_probe.c'
module=ast.parse((ROOT/'coax/tests/nostr/fi_supervisor_pipe_probe.py').read_text())
shim_text=next(ast.literal_eval(n.value) for n in module.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='SHIM' for t in n.targets))
fake_helper=r'''#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
int main(int argc,char **argv){
 if(argc!=4)return 2;const char *path=!strcmp(argv[1],"first.fixture")?FI_MARKER:FI_MARKER2;
 pid_t p=fork();if(p<0)return 3;if(p==0){close(0);close(1);close(64);FILE *f=fopen(path,"w");if(!f)_exit(4);fprintf(f,"%ld\n",(long)getpid());fclose(f);struct timespec t={3,0};nanosleep(&t,NULL);_exit(0);}
 struct stat st;struct timespec tick={0,1000000};for(int i=0;i<2000;i++){if(!stat(path,&st) && st.st_size>0){write(1,"FDE1",4);return 0;}nanosleep(&tick,NULL);}return 5;
}'''
with tempfile.TemporaryDirectory(prefix='fi-pipe-client-') as td:
 d=Path(td)
 lockprobe=d/'lockprobe';subprocess.run(['cc','-std=c11','-O2',str(ROOT/'coax/tests/nostr/fi_flock_restart_probe.c'),'-o',str(lockprobe)],check=True)
 for mode in ('native','asan'):
  flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'] if mode=='asan' else ['-O2']
  for kind in ('disabled','normal','crash','fake','client-exit'):
   marker=d/(mode+'-'+kind+'.pid');marker2=d/(mode+'-'+kind+'-second.pid');lock=d/(mode+'-'+kind+'.lock');helper=d/(mode+'-'+kind+'-helper');obj=d/(mode+'-'+kind+'.o');src=d/(mode+'-'+kind+'.c');shim=d/(mode+'-'+kind+'-shim.c')
   s=supervisor.read_text()
   if kind=='normal':
    needle="const uint8_t terminal[4]={'F','D','E','1'};";assert s.count(needle)==1
    s=s.replace(needle,'struct timespec delay={0,500000000};nanosleep(&delay,NULL);'+needle)
   if kind=='crash':
    needle='pid_t result=waitpid(pid,&status,WNOHANG);';assert s.count(needle)==1
    s=s.replace(needle,'if(access(FI_CRASH_MARKER,F_OK)==0)_exit(99);'+needle)
   if kind=='client-exit':
    needle='aborted=1;kill(pid,SIGKILL);';assert s.count(needle)==1
    s=s.replace(needle,'struct timespec delay={1,0};nanosleep(&delay,NULL);'+needle)
   src.write_text(fake_helper if kind=='fake' else s);shim.write_text(shim_text.replace('struct timespec t={30,0};','struct timespec t={2,0};') if kind=='crash' else shim_text)
   if kind=='fake':
    subprocess.run(['cc','-std=c11',*flags,'-DFI_MARKER="'+str(marker)+'"','-DFI_MARKER2="'+str(marker2)+'"',str(src),'-o',str(helper)],check=True)
   else:
    subprocess.run(['cc','-std=c11',*flags,'-DFI_DNS_EXPERIMENT_LOCK_FD=64','-DFI_CRASH_MARKER="'+str(marker)+'"','-Dgetaddrinfo=fi_test_getaddrinfo','-Dfreeaddrinfo=fi_test_freeaddrinfo','-c',str(src),'-o',str(obj)],check=True)
    subprocess.run(['cc','-std=c11',*flags,'-DFI_MARKER="'+str(marker)+'"',str(shim),str(obj),'-o',str(helper)],check=True)
   exe=d/(mode+'-'+kind+'-client');defs=['-DFI_PIPE_TEST_HELPER_PATH="'+str(helper)+'"','-DFI_PIPE_TEST_LOCK_PATH="'+str(lock)+'"','-DFI_TEST_MARKER="'+str(marker)+'"','-DFI_TEST_MARKER2="'+str(marker2)+'"']
   if kind!='disabled':defs+=['-DFI_PIPE_TEST_ONLY=1']
   if kind=='disabled':defs+=['-DFI_TEST_DISABLED=1']
   if kind=='crash':defs+=['-DFI_TEST_CRASH=1']
   if kind=='fake':defs+=['-DFI_TEST_FAKE=1']
   client_src=client
   if kind=='client-exit':
    defs+=['-DFI_TEST_EXIT=1'];needle='spawned=1;close_if(&data[1]);';client_text=client.read_text();assert client_text.count(needle)==1
    client_src=d/(mode+'-exit-client.c');client_src.write_text(client_text.replace(needle,'_exit(97);'+needle))
   subprocess.run(['cc','-std=c11',*flags,*defs,'-I'+str(client.parent),str(client_src),str(fixture),'-pthread','-o',str(exe)],check=True)
   for auto in (0,1):
    if marker.exists():marker.unlink()
    if marker2.exists():marker2.unlink()
    observed=subprocess.run([str(exe),str(auto)],timeout=18)
    assert observed.returncode==(97 if kind=='client-exit' else 0),(kind,observed.returncode)
    if kind=='client-exit':
     assert subprocess.run([str(lockprobe),'probe',str(lock),str(marker)]).returncode==23,'spawned helper lost lock after client exit'
     until=time.monotonic()+4
     while time.monotonic()<until:
      if subprocess.run([str(lockprobe),'probe',str(lock),str(marker)]).returncode==0:break
      time.sleep(.02)
     else:raise AssertionError('helper did not release inherited test lock')
    if kind=='crash':
     assert marker.exists();pid=int(marker.read_text());until=time.monotonic()+5
     assert subprocess.run([str(lockprobe),'probe',str(lock),str(marker)]).returncode==23,'restart did not see inherited lock'
     while time.monotonic()<until:
      try:os.kill(pid,0)
      except ProcessLookupError:break
      time.sleep(.005)
     else:raise AssertionError('test orphan did not exit naturally')
     assert subprocess.run([str(lockprobe),'probe',str(lock),str(marker)]).returncode==0,'lock remained held after orphan exit'
    if kind=='fake':
     assert marker.exists() and marker2.exists();pids=[int(marker.read_text()),int(marker2.read_text())];until=time.monotonic()+6
     for pid in pids:
      while time.monotonic()<until:
       try:os.kill(pid,0)
       except ProcessLookupError:break
       time.sleep(.005)
      else:raise AssertionError('untrusted-helper test child did not exit naturally')
   print(mode,kind,'passed under active reaper and SIGCHLD auto-reap',flush=True)
