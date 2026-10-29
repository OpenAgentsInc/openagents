#!/usr/bin/env python3
"""macOS ad-hoc exact-code peer pinning experiment, not deployed identity."""
from pathlib import Path
import plistlib,re,shutil,subprocess,tempfile,time,uuid
CLIENT=r'''#include <xpc/xpc.h>
#include <dispatch/dispatch.h>
#include <signal.h>
#include <string.h>
volatile int variant=VARIANT;
int main(int argc,char **argv){
 if(argc!=2 || variant==999)return 5;signal(SIGCHLD,SIG_IGN);xpc_connection_t c=xpc_connection_create("SERVICE_ID",NULL);if(!c)return 6;
 if(xpc_connection_set_peer_code_signing_requirement(c,argv[1]))return 7;
 xpc_connection_set_event_handler(c,^(xpc_object_t e){(void)e;});xpc_connection_resume(c);
 dispatch_semaphore_t sem=dispatch_semaphore_create(0);__block int accepted=0;
 xpc_object_t m=xpc_dictionary_create(NULL,NULL,0);
 xpc_connection_send_message_with_reply(c,m,dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT,0),^(xpc_object_t reply){
  if(xpc_get_type(reply)==XPC_TYPE_DICTIONARY){const char *v=xpc_dictionary_get_string(reply,"answer");if(v && !strcmp(v,"ok"))accepted=1;}dispatch_semaphore_signal(sem);
 });
 long timed=dispatch_semaphore_wait(sem,dispatch_time(DISPATCH_TIME_NOW,1500000000LL));xpc_connection_cancel(c);return !timed && accepted?0:3;
}
'''
SERVICE=r'''#include <xpc/xpc.h>
#include <stdio.h>
static void handle(xpc_connection_t c){
 if(xpc_connection_set_peer_code_signing_requirement(c,"cdhash H\"CLIENT_HASH\"")){xpc_connection_cancel(c);return;}
 xpc_connection_set_event_handler(c,^(xpc_object_t e){if(e==XPC_ERROR_PEER_CODE_SIGNING_REQUIREMENT){FILE *f=fopen(FI_ERROR_PATH,"w");if(f){fputs("signing",f);fclose(f);}}
 if(xpc_get_type(e)==XPC_TYPE_DICTIONARY){xpc_object_t r=xpc_dictionary_create_reply(e);if(r){xpc_dictionary_set_string(r,"answer","ok");xpc_connection_send_message(c,r);xpc_release(r);}}});xpc_connection_resume(c);
}
int main(void){xpc_main(handle);return 0;}
'''
def cdhash(path):
 r=subprocess.run(['codesign','-d','-r-',str(path)],check=True,capture_output=True,text=True);m=re.search(r'cdhash H"([0-9a-f]+)"',r.stdout+r.stderr);assert m;return m.group(1)
with tempfile.TemporaryDirectory(prefix='fi-xpc-codehash-') as td:
 root=Path(td);sid='com.openagents.fi-codehash-'+uuid.uuid4().hex;client_id=sid+'-client'
 app=root/'Good.app/Contents';service=app/'XPCServices'/(sid+'.xpc')/'Contents';(app/'MacOS').mkdir(parents=True);(service/'MacOS').mkdir(parents=True)
 (app/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':client_id,'CFBundleExecutable':'Probe','CFBundlePackageType':'APPL','CFBundleVersion':'1'}))
 (service/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':sid,'CFBundleExecutable':'Service','CFBundlePackageType':'XPC!','CFBundleVersion':'1','XPCService':{'ServiceType':'Application','RunLoopType':'dispatch_main'}}))
 def build_client(path,variant):
  source=root/('client-'+str(variant)+'.c');binary=root/('client-'+str(variant));source.write_text(CLIENT.replace('SERVICE_ID',sid).replace('VARIANT',str(variant)))
  subprocess.run(['cc','-std=c11','-fblocks',str(source),'-o',str(binary)],check=True)
  subprocess.run(['codesign','--force','--sign','-','--identifier',client_id,str(binary)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  meta=subprocess.run(['codesign','-dv',str(binary)],check=True,capture_output=True,text=True);assert 'Identifier='+client_id in meta.stdout+meta.stderr
  result=cdhash(binary);shutil.copy2(binary,path);return result
 good=app/'MacOS/Probe';good_hash=build_client(good,1)
 error=root/'peer-error';source=root/'service.c';source.write_text(SERVICE.replace('CLIENT_HASH',good_hash))
 subprocess.run(['cc','-std=c11','-fblocks','-DFI_ERROR_PATH="'+str(error)+'"',str(source),'-o',str(service/'MacOS/Service')],check=True)
 subprocess.run(['codesign','--force','--sign','-',str(service.parent)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 server_req='cdhash H"'+cdhash(service.parent)+'"'
 assert subprocess.run([str(good),server_req],timeout=5).returncode==0
 clone_app=root/'Clone.app';shutil.copytree(app.parent,clone_app);clone=clone_app/'Contents/MacOS/Probe';assert cdhash(clone)==good_hash
 assert subprocess.run([str(clone),server_req],timeout=5).returncode==0
 rogue_app=root/'Rogue.app';shutil.copytree(app.parent,rogue_app);rogue=rogue_app/'Contents/MacOS/Probe';rogue_hash=build_client(rogue,2);assert rogue_hash!=good_hash
 assert not error.exists(),'a prior connection already recorded a signing error'
 assert subprocess.run([str(rogue),server_req],timeout=5).returncode==3
 deadline=time.monotonic()+1;contents=''
 while time.monotonic()<deadline:
  try:contents=error.read_text()
  except FileNotFoundError:pass
  if contents=='signing':break
  time.sleep(.005)
 assert contents=='signing','service did not observe peer signing failure'
 print('macOS ad-hoc XPC scratch: pinned client and copied binary accepted; distinct same-identifier binary produced peer signing failure')
