// Firm reason: 2 (independent Xlib SDK oracle for real compositor geometry, cursor pixels and WM_DELETE_WINDOW routing).
#define _POSIX_C_SOURCE 200809L
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/cursorfont.h>
#include <assert.h>
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
static pid_t fixture_pid;
static void failed(int signal_number) { if(fixture_pid>0)kill(-fixture_pid,SIGTERM); _exit(128+signal_number); }
static void pause_ms(int ms) { struct timespec t={ms/1000,(ms%1000)*1000000L}; nanosleep(&t,NULL); }
static void command(int fd,const char *s) { size_t n=strlen(s); assert(write(fd,s,n)==(ssize_t)n); }
static Window window(Display *d,const char *title,int w,int h,unsigned long color) {
  Window x=XCreateSimpleWindow(d,DefaultRootWindow(d),10,10,w,h,0,0,color);
  XClassHint c={.res_name="steam_app_battlenet",.res_class="steam_app_battlenet"};
  XSetClassHint(d,x,&c); XStoreName(d,x,title); XSelectInput(d,x,StructureNotifyMask|ExposureMask);
  Cursor cursor=XCreateFontCursor(d,XC_left_ptr);XDefineCursor(d,x,cursor);XFreeCursor(d,cursor);
  Atom a=XInternAtom(d,"WM_DELETE_WINDOW",False); XSetWMProtocols(d,x,&a,1);
  XMapWindow(d,x); XFlush(d); return x;
}
static XWindowAttributes geometry(Display *d,Window x,int w,int h) {
  XWindowAttributes a={0};
  for(int i=0;i<100;i++) { XSync(d,False); assert(XGetWindowAttributes(d,x,&a)); if(a.width==w && a.height==h)return a; pause_ms(50); }
  fprintf(stderr,"geometry got %dx%d at %d,%d; expected %dx%d\n",a.width,a.height,a.x,a.y,w,h); abort();
}
static unsigned char *pixels(const char *path,int *w,int *h) {
  FILE *f=NULL; for(int i=0;i<100 && !f;i++){f=fopen(path,"rb");if(!f)pause_ms(20);} assert(f);
  int max=0; assert(fscanf(f,"P6 %d %d %d",w,h,&max)==3 && max==255); assert(fgetc(f)!=EOF);
  size_t n=(size_t)*w**h*3; unsigned char *p=malloc(n); assert(p && fread(p,1,n,f)==n); fclose(f); return p;
}
int main(int argc,char **argv) {
  signal(SIGABRT,failed);
  assert(argc==4); assert(mkdir(argv[3],0700)==0);
  char runtime[4096],wayland[4096],socket_path[108],log[4096];
  snprintf(runtime,sizeof runtime,"%s/runtime",argv[3]); assert(mkdir(runtime,0700)==0);
  assert(snprintf(socket_path,sizeof socket_path,"%s/desk.sock",argv[3])<(int)sizeof socket_path);
  assert(snprintf(wayland,sizeof wayland,"%s/wayland",runtime)<(int)sizeof wayland); assert(snprintf(log,sizeof log,"%s/compositor.log",argv[3])<(int)sizeof log);
  int in[2]; assert(pipe(in)==0); pid_t child=fork(); assert(child>=0);
  if(!child) {
    assert(setpgid(0,0)==0);
    close(in[1]); assert(dup2(in[0],0)==0); close(in[0]);
    assert(freopen(log,"w",stdout) && dup2(1,2)==2); assert(setenv("XDG_RUNTIME_DIR",runtime,1)==0);
    execl(argv[1],argv[1],wayland,argv[2],runtime,"2548","1428","--xwayland","--desk-socket",socket_path,(char *)NULL); _exit(127);
  }
  fixture_pid=child;
  close(in[0]); char reply[16384]={0};
  for(int i=0;i<100;i++) {
    int s=socket(AF_UNIX,SOCK_STREAM,0); assert(s>=0); struct sockaddr_un a={.sun_family=AF_UNIX}; strcpy(a.sun_path,socket_path);
    if(connect(s,(struct sockaddr *)&a,sizeof a)==0) { command(s,"{\"generation\":2,\"type\":\"info\"}\n"); ssize_t n=read(s,reply,sizeof reply-1); assert(n>0); reply[n]=0;close(s);break; }
    close(s); pause_ms(50);
  }
  char *p=strstr(reply,"\"display\":\""); assert(p);p+=11;char display[32];size_t n=strcspn(p,"\"");assert(n<sizeof display);memcpy(display,p,n);display[n]=0;
  Display *d=NULL;for(int i=0;i<100 && !d;i++){d=XOpenDisplay(display);if(!d)pause_ms(50);}assert(d);
  Window helper=window(d,"",160,20,0x102030);pause_ms(250);
  Window main=window(d,"Battle.net",1240,900,0x405060);geometry(d,main,2548,1428);
  Window dialog=window(d,"WoW: Forever Installation",920,560,0x708090);
  XWindowAttributes a={0};int centered=0;
  for(int i=0;i<100;i++) {
    XSync(d,False);assert(XGetWindowAttributes(d,dialog,&a));
    centered=a.width>=920 && a.width<=924 && a.height>=560 && a.height<=564 && abs(a.x-(2548-a.width)/2)<=1 && abs(a.y-(1428-a.height)/2)<=1;
    if(centered)break;pause_ms(50);
  }
  assert(centered);
  XWindowAttributes b;assert(XGetWindowAttributes(d,helper,&b) && b.width<=320 && b.height<=160);
  geometry(d,main,2548,1428);
  char off[4096],on[4096],cmd[8192];snprintf(off,sizeof off,"%s/cursor-off.ppm",argv[3]);snprintf(on,sizeof on,"%s/cursor-on.ppm",argv[3]);
  command(in[1],"pointer off\nmove 100 100\n");pause_ms(100);snprintf(cmd,sizeof cmd,"shot %s\n",off);command(in[1],cmd);pause_ms(200);
  command(in[1],"pointer on\n");snprintf(cmd,sizeof cmd,"shot %s\n",on);command(in[1],cmd);pause_ms(200);
  int w,h,w2,h2;unsigned char *before=pixels(off,&w,&h),*after=pixels(on,&w2,&h2);assert(w==2548 && h==1428 && w==w2 && h==h2);
  int changed=0;for(int y=0;y<h;y++)for(int x=0;x<w;x++){size_t k=((size_t)y*w+x)*3;if(memcmp(before+k,after+k,3)){assert(x>=4 && x<196 && y>=4 && y<196);changed++;}}assert(changed>0);free(before);free(after);
  snprintf(cmd,sizeof cmd,"move 1000 600\nclose\n");command(in[1],cmd);
  Atom wm_delete=XInternAtom(d,"WM_DELETE_WINDOW",False);int received=0;
  for(int i=0;i<100 && !received;i++){while(XPending(d)){XEvent e;XNextEvent(d,&e);if(e.type==ClientMessage && e.xclient.window==dialog && (Atom)e.xclient.data.l[0]==wm_delete)received++;}if(!received)pause_ms(50);}
  assert(received==1 && kill(child,0)==0); assert(XGetWindowAttributes(d,main,&a) && XGetWindowAttributes(d,helper,&b));
  XDestroyWindow(d,dialog);XDestroyWindow(d,main);XDestroyWindow(d,helper);XCloseDisplay(d);
  command(in[1],"quit\n");close(in[1]);int status;assert(waitpid(child,&status,0)==child && WIFEXITED(status) && WEXITSTATUS(status)==0);
  puts("ok Xlib: main fills desk; helper does not tile; installer centered at requested 920x560 plus rounding padding; cursor pixels present; close reaches only dialog via WM_DELETE_WINDOW");return 0;
}
