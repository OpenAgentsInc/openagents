// Test-only I/O/allocation/assertions. Product carriage stays in generated Coax.
#include "bundle.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CAP 1048576
#define SPACE 2097152
static coax_string text(const char *s) { return (coax_string){s,(int64_t)strlen(s)}; }
static coax_v_u8 span(uint8_t *p,int64_t n) { return (coax_v_u8){p,n}; }
static int same(coax_string a,const char *s) {return a.count==(int64_t)strlen(s)&&memcmp(a.data,s,(size_t)a.count)==0;}
static void aliases(coax_string s,uint8_t *p,int64_t n) {uintptr_t a=(uintptr_t)s.data,b=(uintptr_t)p;assert(s.count>0&&a>=b&&a-b<=(uintptr_t)n&&(uint64_t)s.count<=(uint64_t)n-(a-b));}
static void path(char *out,const char *dir,const char *name) {int n=snprintf(out,4096,"%s/%s",dir,name);assert(n>0&&n<4096);}
static uint8_t *read_file(const char *dir,const char *name,int64_t *n) {char p[4096];path(p,dir,name);FILE*f=fopen(p,"rb");assert(f);assert(fseek(f,0,SEEK_END)==0);long z=ftell(f);assert(z>=0&&z<=SPACE+16);rewind(f);uint8_t*b=malloc((size_t)z+1);assert(b);assert(fread(b,1,(size_t)z,f)==(size_t)z);fclose(f);*n=z;return b;}
static void save(const char *dir,const char *name,uint8_t *p,int64_t n) {char file[4096];path(file,dir,name);FILE*f=fopen(file,"wb");assert(f&&n>=0);assert(fwrite(p,1,(size_t)n,f)==(size_t)n);assert(fclose(f)==0);}
int main(int argc,char **argv) {
 assert(argc==2);const char*dir=argv[1];uint8_t*a=malloc(SPACE+16),*b=malloc(SPACE+16);assert(a&&b);coax_string events[225],views[224];for(int i=0;i<225;i++)events[i]=text("e");
 int64_t n=0;uint8_t*original=read_file(dir,"originals.pb",&n);coax_t_MbpView r=mbp_read(span(original,n),(coax_v_string){views,224});
 assert(r.ok&&r.code==0&&r.event_count==2&&same(r.policy," {\"b\":2, \"a\":1} "));aliases(r.policy,original,n);aliases(r.aggregate,original,n);for(int i=0;i<2;i++)aliases(views[i],original,n);
 int64_t k=mbp_write(r.policy,r.aggregate,(coax_v_string){views,2},span(a,SPACE),span(b,SPACE));assert(k==n&&memcmp(a,original,(size_t)n)==0);save(dir,"coax.pb",a,k);free(original);
 for(int count=0;count<4;count++){n=mbp_write(text("p"),text("a"),(coax_v_string){events,count},span(a,SPACE),span(b,SPACE));assert(n>0);r=mbp_read(span(a,n),(coax_v_string){views,224});assert(r.ok&&r.event_count==count&&same(r.policy,"p")&&same(r.aggregate,"a"));for(int i=0;i<count;i++)assert(same(views[i],"e"));}
 n=mbp_write(text("p"),text("a"),(coax_v_string){events,0},span(a,SPACE),span(b,SPACE));assert(mbp_write(text("p"),text("a"),(coax_v_string){events,0},span(a,n),span(b,n))==n);assert(mbp_write(text("p"),text("a"),(coax_v_string){events,0},span(a,n-1),span(b,n))==-1);assert(mbp_write(text("p"),text("a"),(coax_v_string){events,0},span(a,n),span(b,n-1))==-1);
 for(int which=0;which<8;which++){a[0]='p';b[0]='a';coax_string p=text("p"),g=text("a");events[0]=text("e");coax_v_u8 out=span(a,SPACE),work=span(b,SPACE);
  if(which==0)work=out;if(which==1)work=span(a+1,SPACE-1);if(which==2)p=(coax_string){(char*)a,1};if(which==3)p=(coax_string){(char*)b,1};if(which==4)g=(coax_string){(char*)a,1};if(which==5)g=(coax_string){(char*)b,1};if(which==6)events[0]=(coax_string){(char*)a,1};if(which==7)events[0]=(coax_string){(char*)b,1};
  assert(mbp_write(p,g,(coax_v_string){events,1},out,work)==-1&&a[0]=='p'&&b[0]=='a');}
 char*policy=malloc(32769),*aggregate=malloc(16385),*event=malloc(16385);assert(policy&&aggregate&&event);memset(policy,'p',32769);memset(aggregate,'a',16385);memset(event,'e',16385);for(int i=0;i<225;i++)events[i]=text("e");
 n=mbp_write((coax_string){policy,32768},(coax_string){aggregate,16384},(coax_v_string){events,224},span(a,SPACE),span(b,SPACE));assert(n>0);r=mbp_read(span(a,n),(coax_v_string){views,224});assert(r.ok&&r.policy.count==32768&&r.aggregate.count==16384&&r.event_count==224);for(int i=0;i<224;i++){assert(same(views[i],"e"));aliases(views[i],a,n);}views[0]=text("sentinel");r=mbp_read(span(a,n),(coax_v_string){views,223});assert(!r.ok&&r.code==1&&same(views[0],"sentinel"));
 assert(mbp_write((coax_string){policy,32769},text("a"),(coax_v_string){events,0},span(a,SPACE),span(b,SPACE))==-1);assert(mbp_write(text("p"),(coax_string){aggregate,16385},(coax_v_string){events,0},span(a,SPACE),span(b,SPACE))==-1);events[0]=(coax_string){event,16385};assert(mbp_write(text("p"),text("a"),(coax_v_string){events,1},span(a,SPACE),span(b,SPACE))==-1);events[0]=text("e");assert(mbp_write(text("p"),text("a"),(coax_v_string){events,225},span(a,SPACE),span(b,SPACE))==-1);
 for(int i=0;i<64;i++)events[i]=(coax_string){event,16384};n=mbp_write(text("p"),text("a"),(coax_v_string){events,63},span(a,SPACE),span(b,SPACE));assert(n>0);int64_t last=CAP-n-3;assert(last>=128&&last<16384);events[63].count=last;
 n=mbp_write(text("p"),text("a"),(coax_v_string){events,64},span(a,SPACE),span(b,SPACE));assert(n==CAP);r=mbp_read(span(a,n),(coax_v_string){views,224});assert(r.ok&&r.event_count==64&&views[63].count==last);aliases(views[63],a,n);save(dir,"exact.pb",a,n);
 a[n]=40;a[n+1]=0;r=mbp_read(span(a,n+2),(coax_v_string){views,224});assert(!r.ok&&r.code==1);events[63].count=last+1;assert(mbp_write(text("p"),text("a"),(coax_v_string){events,64},span(a,SPACE),span(b,SPACE))==-1);
 events[63].count=last-1;assert(mbp_write(text("p"),text("a"),(coax_v_string){events,64},span(a,CAP-1),span(b,CAP-1))==CAP-1);assert(!mbp_read(span(a,SPACE),(coax_v_string){views,224}).ok);assert(!mbp_read(span(a,SPACE+1),(coax_v_string){views,224}).ok);
 uint8_t*oversize=read_file(dir,"schema-valid-over-codec-limit.pb",&n);assert(n>CAP&&n<=SPACE);r=mbp_read(span(oversize,n),(coax_v_string){views,224});assert(!r.ok&&r.code==1);free(oversize);
 uint8_t bad=255;assert(mbp_write((coax_string){(char*)&bad,1},text("a"),(coax_v_string){events,0},span(a,SPACE),span(b,SPACE))==-1);
 free(policy);free(aggregate);free(event);free(a);free(b);puts("PASS public bundle: original borrowed views, native aliases, field/cohort bounds, exact1MiB, near capacity and over-limit hard refusals");return 0;
}
