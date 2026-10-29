#include "decision-prepare-memory.h"
#include "prepare-golden.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
int main(int argc,char **argv){
    const char *path=argc==2?argv[1]:"psionic/fixtures/text/ch05/toy.pst";FILE *f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>0);rewind(f);
    unsigned char *table=malloc((size_t)n);assert(table && fread(table,1,(size_t)n,f)==(size_t)n);fclose(f);
    unsigned char prompt[1024],render_work[1024],normal[2048];int64_t candidate[512],full[512],stage[512],scalars[2048],symbols[1024],out[512];
    for(int i=0;i<512;i++)out[i]=-7;
    coax_t_PrWorkspace work={{prompt,1024},{render_work,1024},{candidate,512},{full,512},{stage,512},{normal,2048},{scalars,2048},{symbols,1024}};
    coax_t_PtEntry state={COAX_PtEntryKind_text,str("ready")};coax_t_PtQuestion q={COAX_PtQuestionKind_choice,{COAX_PtEntryKind_text,str("Pick.")}};
    coax_t_PtOption opts[2]={{str("yes"),{COAX_PtEntryKind_none,str("")}},{str("no"),{COAX_PtEntryKind_none,str("")}}};
    coax_t_PrResult r=psionic_prepare((coax_v_u8){table,n},state,q,(coax_v_t_PtOption){opts,2},(coax_v_s64){out,512},work);
    int64_t count=(int64_t)(sizeof(prepare_ids)/sizeof(*prepare_ids));
    assert(r.status==COAX_PrStatus_ok && r.written==count && r.prefix==prepare_prefix && r.slots==2 && out[count]==-7);
    assert(!memcmp(out,prepare_ids,(size_t)count*sizeof(int64_t)));out[0]=-7;
    opts[0].name=(coax_string){(char *)out,1};
    r=psionic_prepare((coax_v_u8){table,n},state,q,(coax_v_t_PtOption){opts,2},(coax_v_s64){out,512},work);
    assert(r.status==COAX_PrStatus_alias && out[0]==-7);
    free(table);puts("typed decision preparation: owned tokenizer oracle and nested alias checks pass");
}
