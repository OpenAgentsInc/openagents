#include "block_project.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static uint8_t in[1048576], out[1048576];
int main(int argc, char **argv) {
  if (argc!=2) return 2;
  size_t n=fread(in,1,sizeof in,stdin);if (!feof(stdin) || ferror(stdin)) return 3;
  coax_v_u8 input={in,(int64_t)n};
  if (!strcmp(argv[1],"copy")) {
    int64_t count=mp_copy(input,(coax_v_u8){out,sizeof out});
    if (count<0) return 4;
    return fwrite(out,1,(size_t)count,stdout)==(size_t)count?0:5;
  }
  char *end=NULL;long op=strtol(argv[1],&end,10);if (!end || *end) return 6;
  printf("%lld\n",(long long)mp_read(input,op));return 0;
}
