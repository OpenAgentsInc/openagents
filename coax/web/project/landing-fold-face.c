/* Own translation unit: coax0's generated types and flat names stay apart.
   The public Wasm entry takes only original bytes, never a verified flag. */
#include "landing-fold.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
#define INPUT_CAP 1048576
#define OUTPUT_CAP 4194304
extern unsigned char nl_inputs[3][INPUT_CAP],nl_output_bytes[OUTPUT_CAP];
extern int64_t nl_output_count;
extern _Bool lr_verify_public(coax_string text);
static coax_t_LfState project;
static coax_v_t_LfState p(void) {return (coax_v_t_LfState){&project,1};}
static coax_string in(int i,int n) {return (coax_string){(const char *)nl_inputs[i],n>=0 && n<=INPUT_CAP?n:0};}
EXPORT("nl_fold_begin") int32_t fold_begin(int32_t key_n,int32_t slug_n,int32_t known_n) {return lf_begin(p(),in(0,key_n),in(1,slug_n),in(2,known_n));}
EXPORT("nl_fold_feed") int32_t fold_feed(int32_t n) {coax_string original=in(0,n);_Bool verified=lr_verify_public(original);return lf_feed(p(),original,verified);}
EXPORT("nl_fold_result") int32_t fold_result(void) {nl_output_count=lf_result(p(),(coax_v_u8){nl_output_bytes,OUTPUT_CAP});return nl_output_count>=0;}
int64_t home_fold_ids(int64_t offset) {nl_output_count=lf_home_ids(p(),offset,(coax_v_u8){nl_output_bytes,OUTPUT_CAP});return nl_output_count;}
int64_t home_fold_known(void) {nl_output_count=lf_home_known(p(),(coax_v_u8){nl_output_bytes,OUTPUT_CAP});return nl_output_count;}
