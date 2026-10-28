/* No read, checksum or comparison of the buffer after erasure. This is the
 * artifact-retention probe, separate from observable functional tests. */
#include "zeroize.h"
#include <string.h>
#if defined(__wasm__)
#define PROBE __attribute__((used,noinline,export_name("zero_probe")))
#else
#define PROBE __attribute__((used,noinline))
#endif
PROBE uint32_t zero_probe(uint32_t seed){
    uint8_t buffer[64];
    memset(buffer,(int)seed,sizeof(buffer));
    return coax_secure_zero(buffer,64,0,64);
}
#ifndef __wasm__
int main(void){return (int)zero_probe(17);}
#endif
