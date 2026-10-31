/* Firm reason2: effects-only original learned-output adapter replay. */
#include "groot-embodiment.h"
#include <stdio.h>
#include <stdlib.h>
static void need(int ok){if(!ok){fputs("GR00T decode effect failed\n",stderr);exit(1);}}
int main(int argc,char **argv){
 need(argc==3);FILE *f=fopen(argv[1],"rb");need(f!=NULL);need(!fseek(f,0,SEEK_END));long bytes=ftell(f);need(bytes>=0&&bytes%4==0);rewind(f);float *model=malloc((size_t)bytes),out[112];need(model!=NULL);need(fread(model,1,(size_t)bytes,f)==(size_t)bytes&&!fclose(f));
 coax_t_GeRead r=ge_decode((coax_v_f32){model,bytes/4},(coax_v_f32){out,112},0);need(r.status==COAX_GeStatus_ok&&r.written==112);
 f=fopen(argv[2],"wbx");need(f!=NULL);need(fwrite(out,4,112,f)==112&&!fclose(f));free(model);printf("Coax original action adapter: 112 values, %lld clipped\n",(long long)r.clipped);return 0;
}
