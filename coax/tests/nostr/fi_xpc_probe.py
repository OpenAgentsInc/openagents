#!/usr/bin/env python3
"""Headless lifecycle/peer-requirement probe; not an FI broker or provider."""
from pathlib import Path
import plistlib
import os
import time
import re
import subprocess
import tempfile
import uuid

SERVICE='''#include <xpc/xpc.h>
#include <unistd.h>
static void handle(xpc_connection_t c) {
 xpc_connection_set_event_handler(c, ^(xpc_object_t e) {
  if(xpc_get_type(e)==XPC_TYPE_DICTIONARY){if(xpc_dictionary_get_bool(e,"hang")){sleep(2);return;}
   xpc_object_t r=xpc_dictionary_create_reply(e);
   if(r){xpc_dictionary_set_string(r,"answer","ok");xpc_dictionary_set_int64(r,"pid",getpid());xpc_connection_send_message(c,r);xpc_release(r);}}
 });xpc_connection_resume(c);
}
int main(void){xpc_main(handle);return 0;}
'''
CLIENT='''#include <xpc/xpc.h>
#include <signal.h>
#include <string.h>
#include <stdio.h>
#include <unistd.h>
#include <dispatch/dispatch.h>
int main(int argc,char **argv) {
 if(argc!=3)return 5;signal(SIGCHLD,SIG_IGN);
 xpc_connection_t c=xpc_connection_create("SERVICE_ID",NULL);if(!c)return 2;
 if(xpc_connection_set_peer_code_signing_requirement(c,argv[1]))return 6;
 xpc_connection_set_event_handler(c,^(xpc_object_t e){(void)e;});xpc_connection_resume(c);
 xpc_object_t m=xpc_dictionary_create(NULL,NULL,0),r=xpc_connection_send_message_with_reply_sync(c,m);
 if(!r || xpc_get_type(r)!=XPC_TYPE_DICTIONARY)return 3;
 const char *v=xpc_dictionary_get_string(r,"answer");if(!v || strcmp(v,"ok"))return 4;
 if(!strcmp(argv[2],"hang") || !strcmp(argv[2],"stay")){
  printf("%lld\\n",(long long)xpc_dictionary_get_int64(r,"pid"));fflush(stdout);
  dispatch_semaphore_t sem=dispatch_semaphore_create(0);xpc_object_t h=xpc_dictionary_create(NULL,NULL,0);xpc_dictionary_set_bool(h,"hang",true);
  xpc_connection_send_message_with_reply(c,h,dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT,0),^(xpc_object_t ignored){(void)ignored;dispatch_semaphore_signal(sem);});
  long timed=dispatch_semaphore_wait(sem,dispatch_time(DISPATCH_TIME_NOW,100000000));xpc_connection_cancel(c);if(!strcmp(argv[2],"stay"))sleep(3);return timed?7:8;
 }
 return 0;
}
'''
with tempfile.TemporaryDirectory(prefix='fi-xpc-probe-') as td:
 root=Path(td);sid='com.openagents.fi-probe-'+uuid.uuid4().hex
 app=root/'Probe.app/Contents';service=app/'XPCServices'/(sid+'.xpc')/'Contents'
 (app/'MacOS').mkdir(parents=True);(service/'MacOS').mkdir(parents=True)
 (app/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':sid+'-client','CFBundleExecutable':'Probe','CFBundlePackageType':'APPL','CFBundleVersion':'1'}))
 (service/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':sid,'CFBundleExecutable':'Service','CFBundlePackageType':'XPC!','CFBundleVersion':'1','XPCService':{'ServiceType':'Application','RunLoopType':'dispatch_main'}}))
 service_src=root/'service.c';client_src=root/'client.c';service_src.write_text(SERVICE);client_src.write_text(CLIENT.replace('SERVICE_ID',sid))
 subprocess.run(['cc','-std=c11','-fblocks',str(service_src),'-o',str(service/'MacOS/Service')],check=True)
 subprocess.run(['cc','-std=c11','-fblocks',str(client_src),'-o',str(app/'MacOS/Probe')],check=True)
 subprocess.run(['codesign','--force','--sign','-',str(service.parent)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 subprocess.run(['codesign','--force','--sign','-',str(app.parent)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 result=subprocess.run(['codesign','-d','-r-',str(service.parent)],check=True,capture_output=True,text=True)
 match=re.search(r'cdhash H"([0-9a-f]+)"',result.stdout+result.stderr);assert match
 good='cdhash H"'+match.group(1)+'"';bad='cdhash H"'+'0'*len(match.group(1))+'"'
 assert subprocess.run([str(app/'MacOS/Probe'),good,'ok'],timeout=5).returncode==0
 assert subprocess.run([str(app/'MacOS/Probe'),bad,'ok'],timeout=5).returncode==3
 hang=subprocess.run([str(app/'MacOS/Probe'),good,'hang'],timeout=5,capture_output=True,text=True);assert hang.returncode==7,hang
 pid=int(hang.stdout.strip());until=time.monotonic()+3
 while time.monotonic()<until:
  try:os.kill(pid,0)
  except ProcessLookupError:break
  time.sleep(.01)
 else:raise AssertionError('XPC service remained alive after client exit')
 staying=subprocess.Popen([str(app/'MacOS/Probe'),good,'stay'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
 pid=int(staying.stdout.readline().strip());time.sleep(.4)
 try:os.kill(pid,0);alive_after_cancel=True
 except ProcessLookupError:alive_after_cancel=False
 _,err=staying.communicate(timeout=5);assert staying.returncode==7,(staying.returncode,err)
 print('headless XPC echo: pinned hash accepted, wrong hash denied, request timed out, service exited with client; service alive 0.4s after cancellation while client stayed alive:',alive_after_cancel)
