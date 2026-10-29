/* Public-fixture binary transport and span/transaction sanitizer observations. */
#include "envelope.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define LIMIT 524288
static uint8_t input[LIMIT+32],schema[LIMIT+32],expected[LIMIT+32],out[LIMIT+32],scratch[LIMIT+32],snapshot[LIMIT+32];
static uint32_t le32(const uint8_t*p){return(uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24;}
static int64_t le64(const uint8_t*p){uint64_t v=0;for(int i=0;i<8;i++)v|=(uint64_t)p[i]<<(8*i);return(int64_t)v;}
static coax_string text(uint8_t*p,int64_t n){return(coax_string){p,n};}
static coax_v_u8 view(uint8_t*p,int64_t n){return(coax_v_u8){p,n};}
static void build_failed(coax_t_ArtifactBuild r,uint32_t error){assert(r.error==error&&!r.written&&!r.canonical_size);for(unsigned i=0;i<sizeof out;i++)assert(out[i]==165);}
static void info_failed(coax_t_ArtifactInfo r,uint32_t error){assert(r.error==error&&!r.inline_from&&!r.inline_to&&!r.canonical_size&&!r.issued_at&&!r.retain_until&&!r.wire_bytes);}
int main(void){
 uint8_t header[48];unsigned records=0,builds=0,checks=0;
 while(fread(header,1,sizeof header,stdin)==sizeof header){
  uint32_t op=le32(header),n=le32(header+4),sn=le32(header+8),m=le32(header+12),error=le32(header+16),size=le32(header+20);
  int64_t issued=le64(header+24),retain=le64(header+32),wire=le64(header+40);assert(n<=LIMIT+1&&sn<=65537&&m<=LIMIT);
  assert(fread(input,1,n,stdin)==n&&fread(schema,1,sn,stdin)==sn&&fread(expected,1,m,stdin)==m);memcpy(snapshot,input,n);memset(out,165,sizeof out);memset(scratch,90,sizeof scratch);
  if(!op){
   coax_t_ArtifactBuild r=envelope_build(text(input,n),text(schema,sn),issued,retain,view(scratch,LIMIT),view(out,error?LIMIT:m));
   if(error)build_failed(r,error);else{
    assert(r.error==0&&r.written==m&&r.canonical_size==size&&memcmp(out,expected,m)==0);for(unsigned i=m;i<m+8;i++)assert(out[i]==165);for(unsigned i=0;i<size;i++)assert(!scratch[i]);
    /* Build -> consume closure is checked with the actual emitted body. */
    coax_t_ArtifactInfo q=envelope_check_observed(text(out,m),text(schema,sn),1048576,view(scratch,LIMIT));assert(!q.error&&q.canonical_size==size&&q.issued_at==issued&&q.retain_until==retain&&q.wire_bytes==1048576);
    memset(out,165,sizeof out);r=envelope_build(text(input,n),text(schema,sn),issued,retain,view(scratch,LIMIT),view(out,m-1));build_failed(r,10);
   }builds++;
  }else{
   coax_t_ArtifactInfo r=wire==-1?envelope_check_body(text(input,n),text(schema,sn),view(scratch,LIMIT)):envelope_check_observed(text(input,n),text(schema,sn),wire,view(scratch,LIMIT));
   if(error){info_failed(r,error);if(error==7){assert(!scratch[0]&&!scratch[1]);}}else{assert(!r.error&&r.canonical_size==size&&r.issued_at==issued&&r.retain_until==retain&&r.wire_bytes==(wire==-1?0:wire));assert(r.inline_to-r.inline_from==m&&memcmp(input+r.inline_from,expected,m)==0);for(unsigned i=0;i<size;i++)assert(!scratch[i]);}checks++;
  }
  assert(memcmp(input,snapshot,n)==0);records++;
 }
 memcpy(input,"{}",2);memcpy(schema,"x",1);memcpy(snapshot,input,32);memset(out,165,sizeof out);
 build_failed(envelope_build(text(input,2),text(schema,1),0,1,view(scratch,LIMIT),view(input,LIMIT)),11);
 build_failed(envelope_build(text(input,2),text(schema,1),0,1,view(input,LIMIT),view(out,LIMIT)),11);
 build_failed(envelope_build(text(input,2),text(schema,1),0,1,view(schema,LIMIT),view(out,LIMIT)),11);
 build_failed(envelope_build(text(input,2),text(schema,1),0,1,view(scratch,LIMIT),view(scratch+1,LIMIT)),11);
 assert(memcmp(input,snapshot,32)==0);
 build_failed(envelope_build(text(input,-1),text(schema,1),0,1,view(scratch,LIMIT),view(out,LIMIT)),1);
 build_failed(envelope_build(text(input,2),text(schema,1),0,1,view(scratch,-1),view(out,LIMIT)),10);
 build_failed(envelope_build(text(input,2),text(schema,1),0,1,view(scratch,LIMIT),view(out,LIMIT+1)),10);
 build_failed(envelope_build(text(input,2),text(schema,1),0,1,view(scratch,1),view(out,LIMIT)),10);
 info_failed(envelope_check_body(text(input,2),text(schema,1),view(input,LIMIT)),11);
 info_failed(envelope_check_body(text(input,2),text(schema,1),view(schema,LIMIT)),11);
 coax_t_ArtifactBuild good=envelope_build(text(input,2),text(schema,1),0,1,view(scratch,LIMIT),view(out,LIMIT));assert(!good.error);
 info_failed(envelope_check_body(text(out,good.written),text(schema,1),view(scratch,1)),10);
 printf("%u records (%u builds with closure and short output, %u checks), span/alias transactions passed\n",records,builds,checks);return 0;
}
