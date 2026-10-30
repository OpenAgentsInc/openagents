/* Process I/O only. All target selection, pins and serialization are Coax. */
#include "model-info.h"
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc != 2) { fputs("usage: psionic-model-info elephant-1\n", stderr); return 64; }
    unsigned char out[8192];
    int64_t n = pmd_manifest((coax_string){argv[1],(int64_t)strlen(argv[1])},(coax_v_u8){out,sizeof out});
    if (n < 0) { fputs("model-info: unsupported model or manifest does not fit\n",stderr);return 2; }
    if (fwrite(out,1,(size_t)n,stdout)!=(size_t)n || fputc('\n',stdout)==EOF) return 1;
    return 0;
}
