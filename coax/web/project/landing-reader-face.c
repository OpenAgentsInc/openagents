/* Scalar effects ABI for the read-only build tool. No Nostr policy here. */
#include "landing-nostr.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
#define INPUT_CAP 1048576
#define OUTPUT_CAP 4194304
unsigned char nl_inputs[3][INPUT_CAP], nl_output_bytes[OUTPUT_CAP];
int64_t nl_output_count;
static coax_t_LrQuery query;
static coax_v_t_LrQuery q(void) {return (coax_v_t_LrQuery){&query,1};}
static coax_string in(int i,int n) {return (coax_string){(const char *)nl_inputs[i],n>=0 && n<=INPUT_CAP?n:0};}
static coax_v_u8 out(void) {return (coax_v_u8){nl_output_bytes,OUTPUT_CAP};}
EXPORT("nl_input") uint32_t input(uint32_t i) {return i<3?(uint32_t)(uintptr_t)nl_inputs[i]:0;}
EXPORT("nl_input_cap") uint32_t input_cap(void) {return INPUT_CAP;}
EXPORT("nl_output") uint32_t output(void) {return (uint32_t)(uintptr_t)nl_output_bytes;}
EXPORT("nl_output_len") uint32_t output_len(void) {return nl_output_count>0?(uint32_t)nl_output_count:0;}
EXPORT("nl_begin") int32_t begin(int32_t n) {return lr_begin(q(),in(0,n));}
EXPORT("nl_request") int32_t request(void) {nl_output_count=lr_request(q(),out());return nl_output_count>=0;}
EXPORT("nl_close") int32_t close_query(void) {nl_output_count=lr_close(q(),out());return nl_output_count>=0;}
EXPORT("nl_frame") int32_t frame(int32_t n) {int64_t r=lr_frame(q(),in(0,n),out());nl_output_count=r>=10?r-10:0;return r>=10?1:(int32_t)r;}
EXPORT("nl_state") int32_t state(void) {return (int32_t)query.state;}
EXPORT("nl_verify") int32_t verify(int32_t n) {return lr_verify_public(in(0,n));}
EXPORT("nl_npub") int32_t npub(int32_t n) {nl_output_count=lr_npub(in(0,n),out());return nl_output_count>=0;}
EXPORT("nl_beacon") int32_t beacon(int32_t n,int64_t now) {nl_output_count=lr_beacon(in(0,n),now,out());return nl_output_count>=0;}
EXPORT("nl_newer") int32_t newer(int64_t g,int64_t t,int64_t hg,int64_t ht) {return lr_newer(g,t,hg,ht);}
EXPORT("nl_budget") int32_t budget(int64_t events,int64_t bytes) {return lr_capture_budget(events,bytes);}
EXPORT("nl_filter") int32_t filter(int32_t mode,int32_t key_n,int32_t value_n) {nl_output_count=lr_filter(mode,in(0,key_n),in(1,value_n),out());return nl_output_count>=0;}
EXPORT("nl_stats") int32_t stats(int32_t work_n,int32_t requests_n,int32_t key_n,int64_t now) {nl_output_count=lr_stats(in(0,work_n),in(1,requests_n),in(2,key_n),now,out());return nl_output_count>=0;}
