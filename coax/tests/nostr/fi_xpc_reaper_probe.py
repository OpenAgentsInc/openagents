#!/usr/bin/env python3
"""macOS-only host-reaper vs XPC service ownership experiment.
Not an authenticated FI provider, broker deployment, or production service.
"""
from pathlib import Path
import os
import plistlib
import re
import subprocess
import tempfile
import time
import uuid

ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
SERVICE='''#define _POSIX_C_SOURCE 200809L
#include <xpc/xpc.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "fi_fetch.h"
static void handle(xpc_connection_t c){
 xpc_connection_set_event_handler(c,^(xpc_object_t e){
  if(xpc_get_type(e)!=XPC_TYPE_DICTIONARY)return;
  xpc_object_t reply=xpc_dictionary_create_reply(e);if(!reply)return;
  const char *uri=xpc_dictionary_get_string(e,"uri");int64_t deadline=xpc_dictionary_get_int64(e,"deadline");
  int state=2;int64_t written=0;uint8_t *body=malloc(524288);
  FiNativeFetch *f=uri && body?fi_fetch_start(uri,strlen(uri),1,deadline):NULL;
  if(f){struct timespec pause={0,5000000};for(int i=0;i<2200;i++){
    state=fi_fetch_poll(f,body,524288,&written);if(state)break;nanosleep(&pause,NULL);
   }fi_fetch_free(f);}
  if(!state)state=2;xpc_dictionary_set_int64(reply,"status",state);
  if(state==1)xpc_dictionary_set_data(reply,"body",body,(size_t)written);
  xpc_connection_send_message(c,reply);xpc_release(reply);free(body);
 });xpc_connection_resume(c);
}
int main(void){xpc_main(handle);return 0;}
'''
CLIENT='''#include <xpc/xpc.h>
#include <signal.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <sys/wait.h>
#include <sys/file.h>
#include <fcntl.h>
#include <errno.h>
#include <pthread.h>
#include <stdatomic.h>
static atomic_int done, reaped, spawn_error, inflight, resolver_started, overlapping_reaps;
static int resolver_lock_held(void){int fd=open(FI_REAPER_PID_FILE,O_RDONLY);if(fd<0)return 0;int r=flock(fd,LOCK_EX|LOCK_NB);int held=r<0 && errno==EWOULDBLOCK;if(r==0)flock(fd,LOCK_UN);close(fd);return held;}
static void *host_reaper(void *unused){(void)unused;while(!atomic_load(&done)){while(waitpid(-1,NULL,WNOHANG)>0){atomic_fetch_add(&reaped,1);if(atomic_load(&inflight) && atomic_load(&resolver_started) && resolver_lock_held())atomic_fetch_add(&overlapping_reaps,1);}usleep(1000);}return NULL;}
static void *host_children(void *unused){(void)unused;int i=0;while(i++<900 && !resolver_lock_held())usleep(1000);if(!resolver_lock_held()){atomic_store(&spawn_error,1);return NULL;}atomic_store(&resolver_started,1);for(i=0;i<32;i++){pid_t pid=fork();if(pid<0){atomic_store(&spawn_error,1);return NULL;}if(pid==0)_exit(0);usleep(10000);}return NULL;}
static int64_t now_ms(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (int64_t)t.tv_sec*1000+t.tv_nsec/1000000;}
static int request(xpc_connection_t c,const char *uri,int deadline,int want){
 xpc_object_t m=xpc_dictionary_create(NULL,NULL,0);xpc_dictionary_set_string(m,"uri",uri);xpc_dictionary_set_int64(m,"deadline",deadline);
 xpc_object_t r=xpc_connection_send_message_with_reply_sync(c,m);if(!r || xpc_get_type(r)!=XPC_TYPE_DICTIONARY)return 0;
 int got=(int)xpc_dictionary_get_int64(r,"status");size_t n=0;const void *p=xpc_dictionary_get_data(r,"body",&n);
 return got==want && (want!=1 || (p && n>0));
}
int main(int argc,char **argv){
 if(argc!=2 || signal(SIGCHLD,SIG_DFL)==SIG_ERR)return 4;pthread_t thread;
 if(pthread_create(&thread,NULL,host_reaper,NULL))return 11;
 for(int i=0;i<32;i++){pid_t pid=fork();if(pid<0)return 12;if(pid==0)_exit(0);}
 int64_t limit=now_ms()+3000;while(atomic_load(&reaped)<32 && now_ms()<limit)usleep(1000);
 if(atomic_load(&reaped)!=32)return 13;
 xpc_connection_t c=xpc_connection_create("SERVICE_ID",NULL);if(!c)return 5;
 if(xpc_connection_set_peer_code_signing_requirement(c,argv[1]))return 6;
 xpc_connection_set_event_handler(c,^(xpc_object_t e){(void)e;});xpc_connection_resume(c);
 if(!request(c,"invalid",1000,2))return 7;
 pthread_t producer;atomic_store(&inflight,1);if(pthread_create(&producer,NULL,host_children,NULL))return 15;
 int64_t start=now_ms();if(!request(c,"https://stall.fixture/",1000,2))return 8;atomic_store(&inflight,0);
 int64_t elapsed=now_ms()-start;if(elapsed<600 || elapsed>2500)return 10;
 pthread_join(producer,NULL);if(atomic_load(&spawn_error))return 16;
 limit=now_ms()+3000;while(atomic_load(&reaped)<64 && now_ms()<limit)usleep(1000);
 usleep(400000);
 if(atomic_load(&reaped)!=64 || atomic_load(&overlapping_reaps)<1)return 14;atomic_store(&done,1);pthread_join(thread,NULL);
 return 0;
}
'''
with tempfile.TemporaryDirectory(prefix='fi-xpc-fetch-') as td:
 d=Path(td);sid='com.openagents.fi-fetch-probe-'+uuid.uuid4().hex
 app=d/'Probe.app/Contents';service=app/'XPCServices'/(sid+'.xpc')/'Contents'
 (app/'MacOS').mkdir(parents=True);(service/'MacOS').mkdir(parents=True)
 (app/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':sid+'-client','CFBundleExecutable':'Probe','CFBundlePackageType':'APPL','CFBundleVersion':'1'}))
 (service/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':sid,'CFBundleExecutable':'Service','CFBundlePackageType':'XPC!','CFBundleVersion':'1','XPCService':{'ServiceType':'Application','RunLoopType':'dispatch_main'}}))
 gen=d/'fi_transport_generated.c';header=d/'fi_transport_generated.h'
 subprocess.run([str(COMPILER),'c',str(ROOT/'coax/lib/nostr/block/fi-jwks.coax'),'--library','--header',str(header),'-o',str(gen)],check=True)
 helper=d/'resolver';obj=d/'resolver.o';fixture=d/'locked-resolver-fixture.c'
 original=(ROOT/'coax/tests/nostr/fi_fetch_resolver_fixture.c').read_text()
 old='FILE *file=fopen(FI_TEST_PID_FILE,"w");if(file){fprintf(file,"%ld",(long)getpid());fclose(file);}'
 new='FILE *file=fopen(FI_TEST_PID_FILE,"w");if(!file || flock(fileno(file),LOCK_EX))return EAI_FAIL;fprintf(file,"%ld",(long)getpid());fflush(file);'
 assert original.count(old)==1
 fixture.write_text(original.replace('#include <arpa/inet.h>','#include <sys/file.h>\n#include <arpa/inet.h>').replace(old,new).replace('nanosleep(&t,0);return EAI_NONAME;','nanosleep(&t,0);fclose(file);return EAI_NONAME;'))
 subprocess.run(['cc','-std=c11','-O2','-Dgetaddrinfo=fi_test_getaddrinfo','-Dfreeaddrinfo=fi_test_freeaddrinfo','-c',str(ROOT/'coax/tests/nostr/fixtures/fi_transport/fi_dns_worker.c'),'-o',str(obj)],check=True)
 subprocess.run(['cc','-D_DARWIN_C_SOURCE','-DFI_TEST_PID_FILE="'+str(d/'resolver.pid')+'"',str(obj),str(fixture),'-o',str(helper)],check=True)
 ss=d/'service.c';cs=d/'client.c';ss.write_text(SERVICE);cs.write_text(CLIENT.replace('SERVICE_ID',sid))
 platform=ROOT/'coax/tests/nostr/fixtures/fi_transport'
 subprocess.run(['cc','-std=c11','-fblocks','-I'+str(d),'-I'+str(platform),'-DFI_FETCH_TEST_UNSAFE_REAPER=1','-DFI_DNS_HELPER_PATH="'+str(helper)+'"',str(gen),str(platform/'fi_fetch.c'),str(ss),'-lcurl','-lpthread','-lm','-o',str(service/'MacOS/Service')],check=True)
 subprocess.run(['cc','-std=c11','-fblocks','-pthread','-DFI_REAPER_PID_FILE="'+str(d/'resolver.pid')+'"',str(cs),'-o',str(app/'MacOS/Probe')],check=True)
 subprocess.run(['codesign','--force','--sign','-',str(service.parent)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 subprocess.run(['codesign','--force','--sign','-',str(app.parent)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 r=subprocess.run(['codesign','-d','-r-',str(service.parent)],check=True,capture_output=True,text=True)
 m=re.search(r'cdhash H"([0-9a-f]+)"',r.stdout+r.stderr);assert m
 requirement='cdhash H"'+m.group(1)+'"'
 result=subprocess.run([str(app/'MacOS/Probe'),requirement],timeout=30)
 assert result.returncode==0,result.returncode
 pid=int((d/'resolver.pid').read_text());until=time.monotonic()+1
 while time.monotonic()<until:
  try:os.kill(pid,0)
  except ProcessLookupError:break
  time.sleep(.005)
 else:raise AssertionError('resolver child survived or was not reaped')
 print('macOS XPC scratch service: actively reaping host and stalled service-owned resolver cancellation passed')
