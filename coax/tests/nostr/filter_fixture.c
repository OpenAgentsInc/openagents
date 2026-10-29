/* Independent harness for the bounded local filter; generated code included
 * only here so private Coax functions are callable without exporting an ABI. */
#include "generated.c"
#include <stdint.h>
#include <string.h>
#ifndef FILTER_EVENT
#error FILTER_EVENT must contain the independent event JSON fixture
#endif
static const char text[]=FILTER_EVENT;
int filter_case(int mask){
 uint8_t out[1024];coax_t_NostrEvent e=nostr_event_read((coax_string){text,sizeof(text)-1},(coax_v_u8){out,sizeof(out)},0);
 if(e.error)return -1;
 coax_string ids[]={{"53d4f42ea78a74ab5ab47515040729f9ae9348f4ed05b59d07c7d24fb90116dc",64},{"0000000000000000000000000000000000000000000000000000000000000000",64}};
 coax_string authors[]={{"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",64},{"0000000000000000000000000000000000000000000000000000000000000000",64}};
 int64_t kinds[]={1,2};coax_string values[]={{"\xc3\xa9",2},{"no",2}};
 coax_t_NostrTagFilter tag={'x',{values+((mask&128)?0:1),1}};
 coax_t_NostrFilter f={0};
 f.has_ids=(mask&1)!=0;f.ids=(coax_v_string){ids+((mask&2)?0:1),(mask&2048)?0:1};if(!f.has_ids)f.ids.count=0;
 f.has_authors=(mask&4)!=0;f.authors=(coax_v_string){authors+((mask&8)?0:1),f.has_authors?1:0};
 f.has_kinds=(mask&16)!=0;f.kinds=(coax_v_s64){kinds+((mask&32)?0:1),f.has_kinds?1:0};
 f.tags=(coax_v_t_NostrTagFilter){&tag,(mask&64)?1:0};f.has_since=(mask&256)!=0;f.since=1;
 f.has_until=(mask&512)!=0;f.until=-1;f.has_limit=(mask&1024)!=0;f.limit=0;
 return coax_p_nf_match(f,e);
}
int filter_two_tags(void){
 uint8_t out[1024];coax_t_NostrEvent e=nostr_event_read((coax_string){text,sizeof(text)-1},(coax_v_u8){out,sizeof(out)},0);
 if(e.error)return -1;
 coax_string value[]={ {"\xc3\xa9",2} };coax_t_NostrTagFilter tags[]={{'x',{value,1}},{'y',{value,1}}};
 coax_t_NostrFilter f={0};f.tags=(coax_v_t_NostrTagFilter){tags,2};
 return coax_p_nf_match(f,e);
}
int filter_serialize(uint8_t *out,int64_t cap){
 coax_string ids[]={{"53d4f42ea78a74ab5ab47515040729f9ae9348f4ed05b59d07c7d24fb90116dc",64}};
 int64_t kinds[]={1,42};coax_string values[]={{"a\"b\n\xc3\xa9",6}};
 coax_t_NostrTagFilter tag={'x',{values,1}};coax_t_NostrFilter f={0};
 f.has_ids=1;f.ids=(coax_v_string){ids,1};f.has_kinds=1;f.kinds=(coax_v_s64){kinds,2};f.tags=(coax_v_t_NostrTagFilter){&tag,1};f.has_since=1;f.since=-1;f.has_limit=1;f.limit=0;
 return (int)coax_p_nf_write(f,(coax_v_u8){out,cap},0);
}
#ifdef FILTER_WASM
__attribute__((export_name("filter_case"))) int wasm_filter_case(int mask){return filter_case(mask);}
__attribute__((export_name("filter_two_tags"))) int wasm_filter_two_tags(void){return filter_two_tags();}
__attribute__((export_name("filter_serialize"))) int wasm_filter_serialize(uint32_t out,int cap){return filter_serialize((uint8_t *)(uintptr_t)out,cap);}
#endif
#ifdef FILTER_STANDALONE
#include <assert.h>
int main(void){
 for(int m=0;m<4096;m++){
  int expected=(!(m&1)||(!(m&2048)&&(m&2)))&&(!(m&4)||(m&8))&&(!(m&16)||(m&32))&&(!(m&64)||(m&128))&&!(m&256)&&!(m&512);
  assert(filter_case(m)==expected);
 }
 return 0;
}
#endif
