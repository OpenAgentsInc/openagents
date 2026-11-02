/* Independent caller-pointer/extent/content oracle for the actual so_post.
 * Scripted process effects only; no real child, network or lifetime inference. */
#include "b2-systemone-runtime.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static FILE *diagnostics(void){return stderr;}
#undef stderr
#define REQUIRE(x) do{if(!(x)){fprintf(diagnostics(),"failed %d: %s\n",__LINE__,#x);return 1;}}while(0)
enum{SUCCESS,EMPTY_SUCCESS,EXIT28,HTTP503,SPAWN_ERROR};
static int scenario,live,releases,effects,line_index;static const char*line;
static const char response[]="{\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}";
static const char trouble[]="independent stderr bytes";
typedef struct{uint8_t before[16],data[192],after[16];} Guard;
static void fill(Guard*g){memset(g,0xa5,sizeof*g);}
static coax_string text(const char*s){return(coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 view(Guard*g,int offset,int count){return(coax_v_u8){g->data+offset,count};}
static int content(coax_string s,const char*t,size_t n){return s.count==(int64_t)n&&(!n||!memcmp(s.data,t,n));}
static int untouched(const Guard*g,const Guard*old,int offset,int capacity){
 return !memcmp(g->before,old->before,sizeof g->before)&&!memcmp(g->after,old->after,sizeof g->after)&&!memcmp(g->data,old->data,(size_t)offset)&&!memcmp(g->data+offset+capacity,old->data+offset+capacity,sizeof g->data-offset-capacity);
}
static void disturb(void){volatile uint8_t scratch[16384];for(size_t i=0;i<sizeof scratch;i++)scratch[i]=(uint8_t)i;}
int64_t cxproc_new(void){effects++;if(live)return -1;live=1;return 0;}
int64_t cxproc_push(int64_t h,int64_t field,int64_t byte){(void)field;(void)byte;effects++;return live&&h==0?0:-1;}
int64_t cxproc_end(int64_t h,int64_t field){(void)field;effects++;return live&&h==0?0:-1;}
int64_t cxproc_spawn(int64_t h){effects++;return live&&h==0?(scenario==SPAWN_ERROR?2:0):-1;}
int64_t cxproc_write(int64_t h,int64_t byte){(void)byte;effects++;return live&&h==0?0:-1;}
int64_t cxproc_flush(int64_t h){effects++;return live&&h==0?0:-1;}
int64_t cxproc_close_stdin(int64_t h){effects++;return live&&h==0?0:-1;}
int64_t cxproc_read_line(int64_t h,int64_t wait){(void)wait;effects++;if(!live||h!=0)return -9;if(scenario==EXIT28)return -1;if(!line_index++){line=scenario==EMPTY_SUCCESS?"":response;return(int64_t)strlen(line);}if(line_index==2){line=scenario==HTTP503?"503":"200";return 3;}return -1;}
int64_t cxproc_line_byte(int64_t h,int64_t i){effects++;return live&&h==0?(uint8_t)line[i]:-1;}
int64_t cxproc_wait(int64_t h,int64_t timeout){(void)timeout;effects++;return live&&h==0?(scenario==EXIT28?28:0):-1;}
int64_t cxproc_stderr_count(int64_t h){effects++;return live&&h==0?(int64_t)strlen(trouble):0;}
int64_t cxproc_stderr_byte(int64_t h,int64_t i){effects++;return live&&h==0?(uint8_t)trouble[i]:-1;}
int64_t cxproc_release(int64_t h){effects++;if(!live||h!=0)return -1;live=0;releases++;return 0;}
int64_t cxproc_getenv_push(int64_t byte){(void)byte;effects++;return 0;}
int64_t cxproc_getenv(void){effects++;return 0;}
static coax_t_SoClient client(void){coax_t_SoClient c;coax_default_t_SoClient(&c);c.key_env=text("");c.url=text("http://offline.invalid");c.curl=text("scripted-only");return c;}
static int check(int which,int offset,int err_capacity){
 Guard out,err;fill(&out);fill(&err);Guard oldout=out,olderr=err;scenario=which;live=releases=effects=line_index=0;
 coax_t_SoResult r=b2_post(client(),text("{}"),view(&out,offset,128),view(&err,offset,err_capacity));
 REQUIRE(!live&&releases==1);disturb();
 if(which==SUCCESS||which==EMPTY_SUCCESS||which==HTTP503){
  const char*wanted=which==EMPTY_SUCCESS?"":response;REQUIRE(r.body.data==(char*)out.data+offset&&content(r.body,wanted,strlen(wanted)));
  REQUIRE(r.stderr.data==NULL&&r.stderr.count==0);REQUIRE(r.status==(which==HTTP503?503:200));
  if(which==HTTP503)REQUIRE(content(r.problem,"the API refused the request",27));
  else{REQUIRE(r.problem.count==0);if(which==SUCCESS)REQUIRE(r.usage.ok&&r.usage.input_tokens==7&&r.usage.output_tokens==2);}
  if(r.body.count){uint8_t first=out.data[offset];out.data[offset]='X';REQUIRE(r.body.data[0]=='X');out.data[offset]=first;}
 }else{
  REQUIRE(r.body.data==NULL&&r.body.count==0&&r.status==0);
  if(which==EXIT28){size_t n=strlen(trouble);if(n>(size_t)err_capacity)n=(size_t)err_capacity;REQUIRE(r.stderr.data==(char*)err.data+offset&&content(r.stderr,trouble,n));REQUIRE(content(r.problem,"the request timed out",21));}
  else REQUIRE(r.stderr.data==NULL&&r.stderr.count==0&&content(r.problem,"curl not found",14));
 }
 REQUIRE(untouched(&out,&oldout,offset,128)&&untouched(&err,&olderr,offset,err_capacity));return 0;
}
int main(void){int cases=0;for(int which=SUCCESS;which<=SPAWN_ERROR;which++)for(int off=0;off<3;off++)for(int cap=0;cap<3;cap++){if(check(which,off==0?0:off==1?3:11,cap==0?0:cap==1?7:64))return 1;cases++;}
 Guard out,err;fill(&out);fill(&err);Guard oldout=out,olderr=err;effects=live=releases=0;coax_t_SoResult empty=b2_post(client(),text(""),view(&out,3,128),view(&err,3,64));REQUIRE(!effects&&!live&&!releases&&empty.body.data==NULL&&empty.body.count==0&&empty.stderr.data==NULL&&empty.stderr.count==0&&content(empty.problem,"no request body",15));REQUIRE(!memcmp(&out,&oldout,sizeof out)&&!memcmp(&err,&olderr,sizeof err));
 printf("{\"buffer_cases\":%d,\"default_empty_setup_cases\":1,\"scope\":\"actual so_post caller buffers; scripted effects; no asynchronous owner lifetime qualification\"}\n",cases);return 0;}
