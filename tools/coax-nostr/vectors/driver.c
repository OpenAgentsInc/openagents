/* Test I/O only: length+message+expected digest records from a verified corpus.
 * Hashing is always the Coax adapter. No native hash or expected-value oracle. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "vector.h"
#define VECTOR_MAX 65536
static uint8_t input[VECTOR_MAX], expected[32], output[32];
int main(void) {
    unsigned row=0;
    for (;;) {
        uint8_t len[4]; size_t got=fread(len,1,4,stdin);
        if (got==0) { if(ferror(stdin))return 3; break; }
        if (got!=4) {fprintf(stderr,"truncated record length\n");return 3;}
        uint32_t n=((uint32_t)len[0]<<24)|((uint32_t)len[1]<<16)|((uint32_t)len[2]<<8)|len[3];
        if(n>VECTOR_MAX || fread(input,1,n,stdin)!=n || fread(expected,1,32,stdin)!=32){fprintf(stderr,"invalid record\n");return 3;}
        memset(output,0xa5,sizeof(output));
        coax_v_u8 in={input,n},out={output,32};
        if(!vector_digest(in,out)){fprintf(stderr,"row %u: adapter refused\n",row);return 4;}
        if(memcmp(output,expected,32)){fprintf(stderr,"row %u: digest mismatch\n",row);return 5;}
        printf("ok %u\n",row++);
    }
    if(!row){fprintf(stderr,"empty vector run\n");return 6;}
    printf("passed %u\n",row);
    return 0;
}
