// Browser SDK scalar projection of the checked native/Wasm transport vectors.
#include "transport-cost-parity.h"
static int ready;
static coax_a64_s64 words;
__attribute__((export_name("transport_cost_word"))) int64_t transport_cost_word(int64_t field){
  if(!ready){words=transport_cost_words();ready=1;}
  return field>=0&&field<64?words.e[field]:-1;
}
