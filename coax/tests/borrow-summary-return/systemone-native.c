/* The actual Coax transport functions, with offline process-effect fixtures. */
#include "systemone.h"
#undef NDEBUG
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

_Static_assert(sizeof(void*)==8 && sizeof(coax_string)==16,"native LP64 string ABI");
_Static_assert(offsetof(coax_string,data)==0 && offsetof(coax_string,count)==8,"borrowed descriptor fields");
_Static_assert(sizeof(coax_v_u8)==16 && offsetof(coax_v_u8,count)==8,"caller-buffer view ABI");

/* Every process effect except the two deliberately exercised reads aborts. */
#define UNUSED0(N) int64_t N(void){abort();}
#define UNUSED1(N) int64_t N(int64_t a){(void)a;abort();}
#define UNUSED2(N) int64_t N(int64_t a,int64_t b){(void)a;(void)b;abort();}
#define UNUSED3(N) int64_t N(int64_t a,int64_t b,int64_t c){(void)a;(void)b;(void)c;abort();}
UNUSED0(cxproc_new) UNUSED3(cxproc_push) UNUSED2(cxproc_end) UNUSED1(cxproc_spawn)
UNUSED2(cxproc_read_line) UNUSED2(cxproc_line_byte) UNUSED2(cxproc_write)
UNUSED1(cxproc_flush) UNUSED1(cxproc_close_stdin) UNUSED2(cxproc_kill) UNUSED2(cxproc_wait)
UNUSED1(cxproc_pid) UNUSED1(cxproc_group_alive) UNUSED1(cxproc_release) UNUSED0(cxproc_now_ms)
UNUSED1(cxproc_getenv_push) UNUSED0(cxproc_getenv) UNUSED1(cxproc_getenv_byte)

static const uint8_t stderr_fixture[7]={'E',0,'R',255,'!',128,'\n'};
static int64_t stderr_length,expected_handle;
static unsigned count_reads,byte_reads,digit_cases,stderr_cases;
int64_t cxproc_stderr_count(int64_t h){assert(h==expected_handle);count_reads++;return stderr_length;}
int64_t cxproc_stderr_byte(int64_t h,int64_t i){assert(h==expected_handle && i==(int64_t)byte_reads && i>=0 && i<stderr_length);byte_reads++;return stderr_fixture[i];}

struct Buffer {uint8_t before[8],bytes[64],after[8];};
static void prepare(struct Buffer *b){memset(b->before,91,sizeof(b->before));memset(b->bytes,0xa7,sizeof(b->bytes));memset(b->after,92,sizeof(b->after));}
static void guards(const struct Buffer *b,const struct Buffer *old,size_t offset,size_t capacity){
 assert(!memcmp(b->before,old->before,sizeof(b->before)));assert(!memcmp(b->after,old->after,sizeof(b->after)));
 assert(!memcmp(b->bytes,old->bytes,offset));assert(!memcmp(b->bytes+offset+capacity,old->bytes+offset+capacity,sizeof(b->bytes)-offset-capacity));
}
__attribute__((noinline)) static void clobber_stack(void){volatile uint8_t scratch[8192];for(size_t i=0;i<sizeof(scratch);i++)scratch[i]=0x69;assert(scratch[8191]==0x69);}
static void digits(void){
 static const struct {int64_t value;const char *text;} cases[]={
  {0,"0"},{1,"1"},{-1,"-1"},{10,"10"},{-10,"-10"},{INT64_C(2147483647),"2147483647"},
  {INT64_C(-2147483648),"-2147483648"},{INT64_MAX,"9223372036854775807"},{INT64_MIN+1,"-9223372036854775807"}
 };
 static const size_t offsets[]={0,3,11};
 for(size_t c=0;c<sizeof(cases)/sizeof(cases[0]);c++)for(size_t o=0;o<3;o++){
  size_t length=strlen(cases[c].text),offset=offsets[o],caps[]={0,length-1,length,length+1,64-offset};
  for(size_t k=0;k<5;k++){
   int duplicate=0;for(size_t j=0;j<k;j++)if(caps[j]==caps[k])duplicate=1;if(duplicate)continue;
   size_t capacity=caps[k];struct Buffer b,old;prepare(&b);memcpy(&old,&b,sizeof(b));
   coax_string r=borrow_so_digits((coax_v_u8){b.bytes+offset,(int64_t)capacity},cases[c].value);clobber_stack();digit_cases++;
   guards(&b,&old,offset,capacity);
   if(capacity<length){assert(r.count==0);continue;} /* Partial writes inside a refused view are not claimed atomic. */
   assert(r.data==(const char*)b.bytes+offset && r.count==(int64_t)length);
   assert(!memcmp(r.data,cases[c].text,length));assert(!memcmp(b.bytes+offset+length,old.bytes+offset+length,capacity-length));
   uint8_t first=b.bytes[offset];b.bytes[offset]='Q';assert(r.data[0]=='Q');b.bytes[offset]=first;
  }
 }
}
static void stderr_copy(void){
 static const int64_t lengths[]={0,1,7};static const size_t offsets[]={0,3,11},caps[]={0,1,6,7,9};
 for(size_t n=0;n<3;n++)for(size_t o=0;o<3;o++)for(size_t c=0;c<5;c++){
  size_t offset=offsets[o],capacity=caps[c],written=(size_t)lengths[n];if(written>capacity)written=capacity;
  struct Buffer b,old;prepare(&b);memcpy(&old,&b,sizeof(b));stderr_length=lengths[n];expected_handle=INT64_C(0x2468);count_reads=byte_reads=0;
  coax_string r=borrow_so_stderr(expected_handle,(coax_v_u8){b.bytes+offset,(int64_t)capacity});clobber_stack();stderr_cases++;
  assert(count_reads==1 && byte_reads==written);assert(r.data==(const char*)b.bytes+offset && r.count==(int64_t)written);
  guards(&b,&old,offset,capacity);assert(!memcmp(r.data,stderr_fixture,written));assert(!memcmp(b.bytes+offset+written,old.bytes+offset+written,capacity-written));
  if(written){uint8_t first=b.bytes[offset];b.bytes[offset]='S';assert(r.data[0]=='S');b.bytes[offset]=first;}
 }
}
int main(void){digits();stderr_copy();printf("{\"digits_cases\":%u,\"stderr_cases\":%u,\"scope\":\"actual caller-owned returned buffers, no request lifetime claim\"}\n",digit_cases,stderr_cases);return 0;}
