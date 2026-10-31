/* Firm reason: thin wrapper (5), file bytes and process status only.
   Coax owns input bounds, tokenizer identity, requests, rendering and counts. */
#include "token-capacity.h"
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char **argv) {
  if(argc!=2)return 2;
  FILE *f=fopen(argv[1],"rb");if(!f)return 2;
  if(fseek(f,0,SEEK_END)){fclose(f);return 2;}
  long n=ftell(f);if(!guard_token_input_size(n)){fclose(f);return 2;}rewind(f);
  unsigned char *p=malloc((size_t)n);if(!p){fclose(f);return 2;}
  if(fread(p,1,(size_t)n,f)!=(size_t)n){free(p);fclose(f);return 2;}fclose(f);
  int result=(int)guard_token_probe((coax_v_u8){p,n});free(p);return result;
}
