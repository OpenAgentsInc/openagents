/* Test TU: calls below delegate to the real OS, except explicit fault cases.
   The adapter TU alone gets CXFR_OPEN=fr_test_open and corresponding substitutions. */
#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include "read-file-fixture.h"
#include "read-file-native.h"
static char events[128];static size_t event_count;
static int opens,reads,closes,last_fd,read_fault,close_fault,stat_fault,short_read;
static void event(char c){assert(event_count+1<sizeof events);events[event_count++]=c;events[event_count]=0;}
void fr_test_event(int64_t n){assert(n>=1&&n<=3);event(n==3?'F':(char)('0'+n));}
int fr_test_open(const char *path,int flags,...){event('O');opens++;assert((flags&O_CLOEXEC)!=0);last_fd=open(path,flags);return last_fd;}
int fr_test_fstat(int fd,struct stat *out){event('S');if(stat_fault){errno=EIO;return -1;}return fstat(fd,out);}
ssize_t fr_test_read(int fd,void *out,size_t count){event('R');reads++;if(read_fault==1){errno=EIO;return -1;}if(read_fault==2)return (ssize_t)count+1;if(read_fault==3){errno=EINTR;return -1;}if(short_read&&count>2)count=2;return read(fd,out,count);}
int fr_test_close(int fd){event('C');closes++;assert(fd==last_fd);if(close_fault==1){errno=EINTR;return -1;}int r=close(fd);if(close_fault==2){assert(r==0);errno=EIO;return -1;}return r;}
static void reset(void){memset(events,0,sizeof events);event_count=0;opens=reads=closes=0;last_fd=-1;read_fault=close_fault=stat_fault=short_read=0;}
static int alive(void){struct stat st;return last_fd>=0&&fstat(last_fd,&st)==0;}
static void no_descriptor(void){assert(last_fd==-1);assert(closes==0);}
static void retired(int live){assert(closes==1);assert(alive()==live);if(live){/* Harness-only cleanup, never an adapter retry. */assert(close(last_fd)==0);assert(!alive());}}
struct Guard {uint8_t before[16],data[32],after[16];};
static void init(struct Guard *g){memset(g,0xa5,sizeof *g);}
static void guard(const struct Guard *g,int cap){for(int i=0;i<16;i++)assert(g->before[i]==0xa5&&g->after[i]==0xa5);for(int i=cap;i<32;i++)assert(g->data[i]==0xa5);}
static int64_t run(const char *path,struct Guard *g,int cap,int variant,int64_t s[16]){return fr_fixture_run((coax_v_u8){(uint8_t*)path,(int64_t)strlen(path)},(coax_v_u8){g->data,cap},(coax_v_s64){s,16},variant);}
static void primary(const int64_t s[16],int kind,int domain,int code,int op){assert(s[0]==kind&&s[1]==domain&&s[2]==code&&s[3]==op);}
static void release(const int64_t s[16],int attempted,int confirmed,int secondary){assert(s[4]==attempted&&s[5]==confirmed&&s[6]==secondary);}
int main(void){
 char directory[]="/tmp/coax-read-file-XXXXXX";assert(mkdtemp(directory));char path[512],empty[512],missing[512];
 assert(snprintf(path,sizeof path,"%s/full",directory)>0);assert(snprintf(empty,sizeof empty,"%s/empty",directory)>0);assert(snprintf(missing,sizeof missing,"%s/missing",directory)>0);
 int fd=open(path,O_CREAT|O_EXCL|O_WRONLY,0600);assert(fd>=0&&write(fd,"hello world",11)==11&&close(fd)==0);fd=open(empty,O_CREAT|O_EXCL|O_WRONLY,0600);assert(fd>=0&&close(fd)==0);
 struct Guard g;int64_t s[16];int64_t n;int cases=0;
 reset();init(&g);n=run(path,&g,12,0,s);assert(n==11&&memcmp(g.data,"hello world",11)==0);release(s,1,1,0);assert(opens==1&&reads==2&&strcmp(events,"OSRRC")==0);guard(&g,12);retired(0);cases++;
 reset();init(&g);n=run(path,&g,11,0,s);assert(n==11&&memcmp(g.data,"hello world",11)==0);release(s,1,1,0);assert(strcmp(events,"OSRRC")==0);guard(&g,11);retired(0);cases++;
 reset();init(&g);n=run(path,&g,10,0,s);assert(n==-1);primary(s,3,2,3,3);release(s,1,1,0);assert(strcmp(events,"OSRRC")==0);guard(&g,10);retired(0);cases++;
 reset();init(&g);n=run(empty,&g,0,0,s);assert(n==0);release(s,1,1,0);assert(strcmp(events,"OSRC")==0);guard(&g,0);retired(0);cases++;
 reset();init(&g);n=run(path,&g,0,0,s);assert(n==-1);primary(s,3,2,3,3);release(s,1,1,0);assert(strcmp(events,"OSRC")==0);guard(&g,0);retired(0);cases++;
 reset();init(&g);n=run(missing,&g,12,0,s);assert(n==-1);primary(s,5,1,-ENOENT,1);release(s,0,0,0);assert(strcmp(events,"O")==0);guard(&g,12);no_descriptor();cases++;
 reset();init(&g);n=run("",&g,12,0,s);assert(n==-1);primary(s,1,2,1,1);release(s,0,0,0);assert(events[0]==0);guard(&g,12);no_descriptor();cases++;
 reset();init(&g);uint8_t nul[3]={'a',0,'b'};n=fr_fixture_run((coax_v_u8){nul,3},(coax_v_u8){g.data,12},(coax_v_s64){s,16},0);assert(n==-1);primary(s,1,2,1,1);assert(events[0]==0);guard(&g,12);no_descriptor();cases++;
 reset();init(&g);n=run(directory,&g,12,0,s);assert(n==-1);primary(s,2,2,2,2);release(s,1,1,0);assert(strcmp(events,"OSC")==0);guard(&g,12);retired(0);cases++;
 reset();read_fault=1;init(&g);n=run(path,&g,12,0,s);assert(n==-1);primary(s,5,1,-EIO,3);release(s,1,1,0);assert(strcmp(events,"OSRC")==0);guard(&g,12);retired(0);cases++;
 reset();stat_fault=1;init(&g);n=run(path,&g,12,0,s);assert(n==-1);primary(s,5,1,-EIO,2);release(s,1,1,0);assert(strcmp(events,"OSC")==0);guard(&g,12);retired(0);cases++;
 for(int mode=1;mode<=2;mode++){reset();close_fault=mode;init(&g);n=run(path,&g,12,0,s);assert(n==-1);primary(s,5,1,mode==1?-EINTR:-EIO,4);release(s,1,0,0);assert(strcmp(events,"OSRRC")==0);guard(&g,12);retired(mode==1);cases++;}
 for(int mode=1;mode<=2;mode++){reset();read_fault=1;close_fault=mode;init(&g);n=run(path,&g,12,0,s);assert(n==-1);primary(s,5,1,-EIO,3);release(s,1,0,1);assert(s[7]==5&&s[8]==1&&s[9]==(mode==1?-EINTR:-EIO)&&s[10]==4);assert(strcmp(events,"OSRC")==0);guard(&g,12);retired(mode==1);cases++;}
 reset();read_fault=1;init(&g);n=run(path,&g,12,1,s);assert(n==-777&&s[11]==1&&s[12]==1&&s[13]==1);assert(strcmp(events,"OSRCF")==0);guard(&g,12);retired(0);cases++;
 reset();init(&g);n=run(path,&g,12,1,s);assert(n==11&&s[13]==0);assert(strcmp(events,"OSRRC")==0);guard(&g,12);retired(0);cases++;
 reset();read_fault=1;init(&g);n=run(path,&g,12,2,s);assert(n==-1);primary(s,5,1,-EIO,3);assert(strcmp(events,"OSRC21")==0);guard(&g,12);retired(0);cases++;
 reset();short_read=1;init(&g);n=run(path,&g,12,0,s);assert(n==11&&memcmp(g.data,"hello world",11)==0);assert(reads==7&&strcmp(events,"OSRRRRRRRC")==0);guard(&g,12);retired(0);cases++;
 reset();init(&g);n=run(path,&g,-1,0,s);assert(n==-1);primary(s,4,2,4,1);release(s,0,0,0);assert(events[0]==0);guard(&g,0);no_descriptor();cases++;
 reset();read_fault=2;init(&g);n=run(path,&g,12,0,s);assert(n==-1);primary(s,4,2,4,3);release(s,1,1,0);assert(reads==1&&strcmp(events,"OSRC")==0);guard(&g,12);retired(0);cases++;
 reset();read_fault=3;init(&g);n=run(path,&g,12,0,s);assert(n==-1);primary(s,5,1,-EINTR,3);release(s,1,1,0);assert(reads==1&&strcmp(events,"OSRC")==0);guard(&g,12);retired(0);cases++;
 reset();init(&g);n=run(path,&g,12,3,s);assert(n==-1);primary(s,4,2,4,1);release(s,0,0,0);assert(events[0]==0);guard(&g,12);no_descriptor();cases++;
 reset();init(&g);uint8_t long_path[4097];memset(long_path,'a',sizeof long_path);n=fr_fixture_run((coax_v_u8){long_path,4097},(coax_v_u8){g.data,12},(coax_v_s64){s,16},0);assert(n==-1);primary(s,1,2,1,1);release(s,0,0,0);assert(events[0]==0);guard(&g,12);no_descriptor();cases++;
 assert(unlink(path)==0&&unlink(empty)==0&&rmdir(directory)==0);printf("%d read-file cases passed (real OS plus explicitly injected faults)\n",cases);return 0;
}
