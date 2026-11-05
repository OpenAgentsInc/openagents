// Firm reason: 2 (independent XInput2/XTEST SDK experiment: an isolated virtual keyboard targets the game without changing the physical keyboard's focus).
#define _POSIX_C_SOURCE 200809L
#include <X11/Xlib.h>
#include <X11/extensions/XInput2.h>
#include <X11/extensions/XTest.h>
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
static void pause_ms(int ms){struct timespec t={0,ms*1000000L};nanosleep(&t,NULL);}
int main(int argc,char **argv){
  assert(argc==3);Window target=strtoul(argv[1],NULL,10);long expected=strtol(argv[2],NULL,10);
  Display *d=XOpenDisplay(":1");assert(d);char *title=NULL;assert(XFetchName(d,target,&title) && title && strcmp(title,"World of Warcraft")==0);XFree(title);
  Atom a=XInternAtom(d,"_NET_WM_PID",True),actual;int format;unsigned long n,left;unsigned char *data=NULL;
  assert(XGetWindowProperty(d,target,a,0,1,False,AnyPropertyType,&actual,&format,&n,&left,&data)==Success && format==32 && n==1 && *(long*)data==expected);XFree(data);
  int major=2,minor=0;assert(XIQueryVersion(d,&major,&minor)==Success);
  Window before,after;int revert;XGetInputFocus(d,&before,&revert);
  int original=2;XIGetClientPointer(d,target,&original);assert(original>=2);
  char name[80];snprintf(name,sizeof name,"WoW-Agent-%ld",(long)getpid());
  XIAnyHierarchyChangeInfo change={0};change.add.type=XIAddMaster;change.add.name=name;change.add.send_core=True;change.add.enable=True;
  assert(XIChangeHierarchy(d,&change,1)==Success);XSync(d,False);
  int count,pointer=0,keyboard=0,slave=0;XIDeviceInfo *devices=XIQueryDevice(d,XIAllDevices,&count);assert(devices);
  char wanted[100];snprintf(wanted,sizeof wanted,"%s pointer",name);
  for(int i=0;i<count;i++)if(strcmp(devices[i].name,wanted)==0){pointer=devices[i].deviceid;keyboard=devices[i].attachment;}
  snprintf(wanted,sizeof wanted,"%s XTEST keyboard",name);
  for(int i=0;i<count;i++)if(strcmp(devices[i].name,wanted)==0)slave=devices[i].deviceid;
  XIFreeDeviceInfo(devices);assert(pointer && keyboard && slave);
  assert(XISetClientPointer(d,target,pointer)==Success && XISetClientPointer(d,None,pointer)==Success);
  assert(XISetFocus(d,keyboard,target,CurrentTime)==Success);XSync(d,False);pause_ms(500);
  XDevice *device=XOpenDevice(d,slave);assert(device);
  KeyCode key=XKeysymToKeycode(d,XStringToKeysym("l"));assert(key);
  int down=XTestFakeDeviceKeyEvent(d,device,key,True,NULL,0,CurrentTime);XSync(d,False);pause_ms(20);
  int up=XTestFakeDeviceKeyEvent(d,device,key,False,NULL,0,CurrentTime);XSync(d,False);pause_ms(500);XCloseDevice(d,device);
  assert(XISetClientPointer(d,target,original)==Success && XISetClientPointer(d,None,2)==Success);
  change.remove.type=XIRemoveMaster;change.remove.deviceid=pointer;change.remove.return_mode=XIAttachToMaster;change.remove.return_pointer=2;change.remove.return_keyboard=3;
  assert(XIChangeHierarchy(d,&change,1)==Success);XSync(d,False);XGetInputFocus(d,&after,&revert);
  printf("{\"target\":%lu,\"pid\":%ld,\"virtual_pointer\":%d,\"virtual_keyboard\":%d,\"key\":\"l\",\"down_sent\":%d,\"up_sent\":%d,\"core_focus_before\":%lu,\"core_focus_after\":%lu,\"same_core_focus\":%s,\"virtual_pair_removed\":true}\n",target,expected,pointer,keyboard,down,up,before,after,before==after?"true":"false");
  XCloseDisplay(d);return down && up && before==after?0:1;
}
