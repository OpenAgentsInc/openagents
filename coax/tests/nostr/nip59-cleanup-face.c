/* Independent host projection of the actual Coax cleanup observations. */
#include "nip59-cleanup.h"
#include <stdint.h>
#if defined(__wasm__)
__attribute__((export_name("crypto_cleanup"))) int64_t cleanup(void) {return nip59_cleanup_check();}
#else
#include <stdio.h>
int main(void) {if(nip59_cleanup_check()!=1)return 1;puts("NIP-59 runtime cleanup passed");return 0;}
#endif
