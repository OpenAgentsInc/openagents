#!/usr/bin/env python3
"""macOS-only scratch mutual XPC peer checks; ad-hoc IDs are spoofable."""
from pathlib import Path
import plistlib,re,subprocess,tempfile,time,uuid
SERVICE=r'''#include <xpc/xpc.h>
#include <stdio.h>
static void handle(xpc_connection_t c){
#if REQUIRE_CLIENT
 if(xpc_connection_set_peer_code_signing_requirement(c,"identifier \"CLIENT_ID\"")){xpc_connection_cancel(c);return;}
#endif
 xpc_connection_set_event_handler(c,^(xpc_object_t e){if(e==XPC_ERROR_PEER_CODE_SIGNING_REQUIREMENT){FILE *f=fopen(FI_ERROR_PATH,"w");if(f){fputs("signing",f);fclose(f);}}if(xpc_get_type(e)==XPC_TYPE_DICTIONARY){xpc_object_t r=xpc_dictionary_create_reply(e);if(r){xpc_dictionary_set_string(r,"answer","ok");xpc_connection_send_message(c,r);xpc_release(r);}}});xpc_connection_resume(c);
}
int main(void){xpc_main(handle);return 0;}
'''
CLIENT=r'''#include <xpc/xpc.h>
#include <dispatch/dispatch.h>
#include <signal.h>
#include <string.h>
int main(int argc,char **argv){
 if(argc!=2)return 5;signal(SIGCHLD,SIG_IGN);xpc_connection_t c=xpc_connection_create("SERVICE_ID",NULL);if(!c)return 6;
 if(xpc_connection_set_peer_code_signing_requirement(c,argv[1]))return 7;
 xpc_connection_set_event_handler(c,^(xpc_object_t e){(void)e;});xpc_connection_resume(c);
 dispatch_semaphore_t sem=dispatch_semaphore_create(0);__block int accepted=0,rejected=0;
 xpc_object_t m=xpc_dictionary_create(NULL,NULL,0);
 xpc_connection_send_message_with_reply(c,m,dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT,0),^(xpc_object_t reply){
  if(reply==XPC_ERROR_PEER_CODE_SIGNING_REQUIREMENT)rejected=1;if(xpc_get_type(reply)==XPC_TYPE_DICTIONARY){const char *v=xpc_dictionary_get_string(reply,"answer");if(v && !strcmp(v,"ok"))accepted=1;}dispatch_semaphore_signal(sem);
 });
 long timeout=dispatch_semaphore_wait(sem,dispatch_time(DISPATCH_TIME_NOW,1500000000LL));xpc_connection_cancel(c);
 return !timeout && accepted?0:(!timeout && rejected?8:3);
}
'''
with tempfile.TemporaryDirectory(prefix='fi-xpc-mutual-') as td:
 root=Path(td);base='com.openagents.fi-mutual-'+uuid.uuid4().hex;good_id=base+'-good'
 def build(label,client_id,required):
  sid=base+'-'+label;app=root/(label+'.app')/'Contents';service=app/'XPCServices'/(sid+'.xpc')/'Contents'
  (app/'MacOS').mkdir(parents=True);(service/'MacOS').mkdir(parents=True)
  (app/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':client_id,'CFBundleExecutable':'Probe','CFBundlePackageType':'APPL','CFBundleVersion':'1'}))
  (service/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':sid,'CFBundleExecutable':'Service','CFBundlePackageType':'XPC!','CFBundleVersion':'1','XPCService':{'ServiceType':'Application','RunLoopType':'dispatch_main'}}))
  service_src=root/(label+'-service.c');client_src=root/(label+'-client.c');service_src.write_text(SERVICE.replace('CLIENT_ID',good_id));client_src.write_text(CLIENT.replace('SERVICE_ID',sid))
  error_path=root/(label+'-error');subprocess.run(['cc','-std=c11','-fblocks',f'-DREQUIRE_CLIENT={int(required)}','-DFI_ERROR_PATH="'+str(error_path)+'"',str(service_src),'-o',str(service/'MacOS/Service')],check=True)
  subprocess.run(['cc','-std=c11','-fblocks',str(client_src),'-o',str(app/'MacOS/Probe')],check=True)
  subprocess.run(['codesign','--force','--sign','-',str(service.parent)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  subprocess.run(['codesign','--force','--sign','-',str(app.parent)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  client_meta=subprocess.run(['codesign','-dv',str(app.parent)],check=True,capture_output=True,text=True)
  assert 'Identifier='+client_id in client_meta.stdout+client_meta.stderr
  client_req=subprocess.run(['codesign','-d','-r-',str(app.parent)],check=True,capture_output=True,text=True)
  client_hash=re.search(r'cdhash H"([0-9a-f]+)"',client_req.stdout+client_req.stderr);assert client_hash
  meta=subprocess.run(['codesign','-d','-r-',str(service.parent)],check=True,capture_output=True,text=True)
  match=re.search(r'cdhash H"([0-9a-f]+)"',meta.stdout+meta.stderr);assert match
  return app/'MacOS/Probe','cdhash H"'+match.group(1)+'"',client_hash.group(1),error_path
 good,good_req,good_hash,_=build('good',good_id,True)
 denied,denied_req,_,denied_error=build('denied',base+'-bad',True)
 control,control_req,_,_=build('control',base+'-bad',False)
 spoof,spoof_req,spoof_hash,_=build('spoof',good_id,True);assert spoof_hash!=good_hash
 assert subprocess.run([str(good),good_req],timeout=5).returncode==0
 assert subprocess.run([str(denied),denied_req],timeout=5).returncode==3
 deadline=time.monotonic()+1;error=''
 while time.monotonic()<deadline:
  try:error=denied_error.read_text()
  except FileNotFoundError:pass
  if error=='signing':break
  time.sleep(.005)
 assert error=='signing','service did not report the signing error'
 assert subprocess.run([str(control),control_req],timeout=5).returncode==0
 # Ad-hoc identifiers are not authenticated identity: a different executable
 # with the same identifier is also accepted by this deliberately weak rule.
 assert subprocess.run([str(spoof),spoof_req],timeout=5).returncode==0
 assert subprocess.run([str(good),'cdhash H"'+'0'*40+'"'],timeout=5).returncode==8
 print('macOS scratch XPC: client/server mismatch refused; a distinct ad-hoc executable spoofed the allowed client identifier')
