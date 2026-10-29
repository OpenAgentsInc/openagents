/* Independent retained token/byte oracles and actual storage. Tooling only. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "text-memory.h"
#include "text-goldens.h"
static coax_v_u8 bytes(void *p,int64_t n){return (coax_v_u8){p,n};}
static coax_v_s64 ints(void *p,int64_t n){return (coax_v_s64){p,n};}
static coax_string text(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static void *read_file(const char *name,size_t *n) {
    FILE *f=fopen(name,"rb");assert(f);assert(fseek(f,0,SEEK_END)==0);long count=ftell(f);assert(count>=0);rewind(f);
    void *p=malloc((size_t)count+1);assert(p);assert(fread(p,1,(size_t)count,f)==(size_t)count);assert(fclose(f)==0);*n=(size_t)count;return p;
}
static uint32_t u32(const uint8_t *p){return (uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24;}
static void failed(coax_t_PtResult r,int status){assert(r.status==status && r.written==0 && r.work==0);}
static int64_t output[65536],stage[65536],scalars[262144],symbols[1024];
static uint8_t normal[262144],prompt[65536],scratch[65536];
static coax_t_PtResult encode(coax_v_u8 table,coax_string input) {
    return psionic_text_encode(table,input,ints(output,65536),ints(stage,65536),bytes(normal,sizeof normal),ints(scalars,262144),ints(symbols,1024));
}
static void token_vectors(coax_v_u8 table) {
    for(size_t c=0;c<sizeof text_goldens/sizeof *text_goldens;c++) {
        TextGolden g=text_goldens[c];for(size_t i=0;i<65536;i++) output[i]=-7;
        coax_t_PtResult r=encode(table,g.text);
        if(r.status!=0 || r.written!=g.count){fprintf(stderr,"text case %zu: status %u, count %lld, expected %lld\n",c,r.status,(long long)r.written,(long long)g.count);abort();}
        assert(r.work==g.work && memcmp(output,g.ids,(size_t)g.count*8)==0 && output[g.count]==-7);
    }
    for(int i=0;i<256;i++) {
        uint8_t label[2];coax_t_PtResult r=pt_label_checked(i,bytes(label,2));assert(r.status==0);
        r=encode(table,(coax_string){(const char *)label,r.written});assert(r.status==0 && r.written==1 && output[0]==golden_slot_ids[i]);
    }
    for(size_t i=0;i<sizeof added_goldens/sizeof *added_goldens;i++) {
        AddedGolden g=added_goldens[i];coax_t_PtResult r=encode(table,g.text);
        assert(r.status==0 && r.written==1 && output[0]==g.id);
        char wrapped[128];assert(g.text.count>=1 && g.text.count<=(int64_t)sizeof wrapped-2);
        wrapped[0]='x';memcpy(wrapped+1,g.text.data,(size_t)g.text.count);wrapped[g.text.count+1]='y';
        r=encode(table,(coax_string){wrapped,g.text.count+2});
        assert(r.status==0 && r.written==3 && output[0]==golden_x_id && output[1]==g.id && output[2]==golden_y_id);
    }
    printf("text: 177 original-Qwen token vectors, %zu added literals in context and 256 single-token slots pass\n",sizeof added_goldens/sizeof *added_goldens);
}
static void request_vectors(coax_v_u8 table) {
    int64_t full[32768],ends[3],candidate[16384];int64_t accumulated=0;coax_t_PtBranch branches[3];
    for(size_t c=0;c<sizeof request_goldens/sizeof *request_goldens;c++) {
        RequestGolden g=request_goldens[c];memset(prompt,0xa5,sizeof prompt);
        coax_t_PtPrompt r=psionic_text_render(g.state,g.question,(coax_v_t_PtOption){(coax_t_PtOption *)g.options,g.n},bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));
        if(r.status!=0 || r.written!=g.prompt.count || r.prefix_bytes!=g.prefix || memcmp(prompt,g.prompt.data,(size_t)g.prompt.count)!=0){fprintf(stderr,"render case %zu: status %u, bytes %lld/%lld, prefix %lld/%lld\n",c,r.status,(long long)r.written,(long long)g.prompt.count,(long long)r.prefix_bytes,(long long)g.prefix);abort();}
        assert(prompt[r.written]==0xa5);
        coax_t_PtResult encoded=encode(table,(coax_string){(const char *)prompt,r.written});assert(encoded.status==0 && encoded.written==g.count && memcmp(output,g.ids,(size_t)g.count*8)==0);
        /* First three requests share the same state and exercise actual suffix forks. */
        if(c<3){memcpy(full+accumulated,output,(size_t)g.count*8);accumulated+=g.count;ends[c]=accumulated;}
        encoded=encode(table,(coax_string){(const char *)prompt,r.prefix_bytes});assert(encoded.status==0 && encoded.written==g.candidate_count && memcmp(output,g.candidate,(size_t)g.candidate_count*8)==0);
        if(c==0) memcpy(candidate,output,(size_t)encoded.written*8);
        uint8_t before[65536];memcpy(before,prompt,sizeof prompt);
        coax_t_PtPrompt bad=psionic_text_render(g.state,g.question,(coax_v_t_PtOption){(coax_t_PtOption *)g.options,g.n},bytes(prompt,r.written-1),bytes(scratch,sizeof scratch));
        assert(bad.status==COAX_PtStatus_capacity && bad.written==0 && bad.prefix_bytes==0 && bad.slots==0 && memcmp(before,prompt,sizeof prompt)==0);
    }
    coax_t_PtSplit s=psionic_text_split(ints(candidate,request_goldens[0].candidate_count),ints(full,accumulated),ints(ends,3),(coax_v_t_PtBranch){branches,3});assert(s.status==0 && s.written==3);
    int64_t expected=request_goldens[0].candidate_count;
    for(int i=0,start=0;i<3;i++) {
        int64_t same=0;while(same<expected && same<ends[i]-start && candidate[same]==full[start+same]) same++;expected=same;start=(int)ends[i];
    }
    assert(s.prefix==expected);
    for(int i=0,start=0;i<3;i++){assert(branches[i].begin==start+expected && branches[i].end==ends[i] && branches[i].answer==ends[i]-start-1);start=(int)ends[i];}
    printf("text: %zu exact rendered/tokenized requests, 1/26/27/255 options and complete shared-prefix forks pass\n",sizeof request_goldens/sizeof *request_goldens);
}
static void normalization(coax_v_u8 table) {
    size_t n;uint8_t *data=read_file("psionic/fixtures/text/ch05/normalization.bin",&n);size_t at=0;int count=0;
    while(at<n){assert(n-at>=8);uint32_t a=u32(data+at),b=u32(data+at+4);at+=8;assert(a<=n-at && b<=n-at-a);
        coax_t_PtResult r=psionic_text_normalize(table,(coax_string){(const char *)data+at,a},bytes(normal,sizeof normal),ints(scalars,262144));
        if(r.status!=0 || r.written!=b || memcmp(normal,data+at+a,b)!=0){fprintf(stderr,"normalization case %d failed: %u, %lld/%u\n",count,r.status,(long long)r.written,b);abort();}
        at+=a+b;count++;
    }assert(at==n && count==26506);free(data);printf("text: %d independent Unicode-17 NFC vectors, including all Hangul decompositions, pass\n",count);
}
static void failures(coax_v_u8 table) {
    for(size_t i=0;i<65536;i++) output[i]=-7;
    uint8_t invalid[][4]={{0xc0,0x80,0,0},{0xed,0xa0,0x80,0},{0xf4,0x90,0x80,0x80},{0x80,0,0,0}};
    for(size_t i=0;i<4;i++){failed(encode(table,(coax_string){(const char *)invalid[i],4}),COAX_PtStatus_utf8);assert(output[0]==-7);}
    int64_t tiny=-9;
    failed(psionic_text_encode(table,text("hello world"),ints(&tiny,1),ints(stage,65536),bytes(normal,sizeof normal),ints(scalars,262144),ints(symbols,1024)),COAX_PtStatus_capacity);assert(tiny==-9);
    failed(psionic_text_encode(table,text("hi"),ints(output,8),ints(output+1,8),bytes(normal,sizeof normal),ints(scalars,262144),ints(symbols,1024)),COAX_PtStatus_alias);
    failed(psionic_text_encode(table,(coax_string){(const char *)output,3},ints(output,8),ints(stage,8),bytes(normal,sizeof normal),ints(scalars,262144),ints(symbols,1024)),COAX_PtStatus_alias);
    failed(psionic_text_encode(table,text("hi"),ints(output,8),ints(stage,8),bytes(output+7,8),ints(scalars,262144),ints(symbols,1024)),COAX_PtStatus_alias);
    uint8_t longword[1025];memset(longword,'x',sizeof longword);failed(encode(table,(coax_string){(const char *)longword,sizeof longword}),COAX_PtStatus_budget);
    uint8_t *large=malloc(65537);assert(large);memset(large,'x',65537);failed(encode(table,(coax_string){(const char *)large,65537}),COAX_PtStatus_size);free(large);
    coax_t_PtEntry state={1,{(const char *)prompt,1}};coax_t_PtQuestion q={COAX_PtQuestionKind_noul,{0,{"",0}}};coax_v_t_PtOption no={NULL,0};
    coax_t_PtPrompt p=psionic_text_render(state,q,no,bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));assert(p.status==COAX_PtStatus_alias && p.written==0);
    state=(coax_t_PtEntry){1,{"ready",5}};coax_t_PtOption opts[2]={0};opts[0].name=(coax_string){(const char *)prompt,1};opts[1].name=text("second");q.kind=COAX_PtQuestionKind_choice;
    p=psionic_text_render(state,q,(coax_v_t_PtOption){opts,2},bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));assert(p.status==COAX_PtStatus_alias);
    opts[0].name=text("first");opts[0].description=(coax_t_PtEntry){1,{(const char *)scratch,1}};
    p=psionic_text_render(state,q,(coax_v_t_PtOption){opts,2},bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));assert(p.status==COAX_PtStatus_alias);
    char long_name[65537];memset(long_name,'x',sizeof long_name);
    opts[0].name=(coax_string){long_name,sizeof long_name};opts[0].description=(coax_t_PtEntry){0,{NULL,0}};
    memset(prompt,0xa5,sizeof prompt);
    p=psionic_text_render(state,q,(coax_v_t_PtOption){opts,1},bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));
    assert(p.status==COAX_PtStatus_size && p.written==0);
    for(size_t k=0;k<sizeof prompt;k++) assert(prompt[k]==0xa5);
    /* JSON accepts escaped code units syntactically; isolated surrogates must
       refuse before any published byte changes. A valid pair becomes UTF-8. */
    const char *bad_json[]={"{\"x\":\"\\ud800\"}","{\"\\udc00\":1}"};
    q.kind=COAX_PtQuestionKind_noul;
    for(size_t i=0;i<sizeof bad_json/sizeof *bad_json;i++) {
        state=(coax_t_PtEntry){COAX_PtEntryKind_json,text(bad_json[i])};
        memset(prompt,0xa5,sizeof prompt);
        p=psionic_text_render(state,q,no,bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));
        assert(p.status==COAX_PtStatus_json && p.written==0 && p.prefix_bytes==0 && p.slots==0);
        for(size_t k=0;k<sizeof prompt;k++) assert(prompt[k]==0xa5);
    }
    state=(coax_t_PtEntry){COAX_PtEntryKind_json,text("{\"x\":\"\\ud83d\\ude00\"}")};
    p=psionic_text_render(state,q,no,bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));
    assert(p.status==0);int found=0;
    for(int64_t i=0;i+4<=p.written;i++) if(!memcmp(prompt+i,"\xf0\x9f\x98\x80",4)) found=1;
    assert(found);
    char number[4310];memcpy(number,"{\"n\":",5);memset(number+5,'9',4300);
    number[4305]='}';number[4306]=0;
    state=(coax_t_PtEntry){COAX_PtEntryKind_json,text(number)};
    p=psionic_text_render(state,q,no,bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));assert(p.status==0);
    number[4305]='9';number[4306]='}';number[4307]=0;
    state.value=text(number);memset(prompt,0xa5,sizeof prompt);
    p=psionic_text_render(state,q,no,bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));
    assert(p.status==COAX_PtStatus_size && p.written==0 && p.prefix_bytes==0 && p.slots==0);
    for(size_t k=0;k<sizeof prompt;k++) assert(prompt[k]==0xa5);
    state=(coax_t_PtEntry){COAX_PtEntryKind_text,{"",0}};
    p=psionic_text_render(state,q,no,bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));
    assert(p.status==0 && p.written<2048);uint8_t empty_prompt[2048];memcpy(empty_prompt,prompt,(size_t)p.written);
    int64_t empty_length=p.written,empty_prefix=p.prefix_bytes;
    state.value=(coax_string){NULL,0};
    p=psionic_text_render(state,q,no,bytes(prompt,sizeof prompt),bytes(scratch,sizeof scratch));
    assert(p.status==0 && p.written==empty_length && p.prefix_bytes==empty_prefix && memcmp(prompt,empty_prompt,(size_t)empty_length)==0);
    /* Admission is exact, then immutable. A changed pack cannot inherit it. */
    uint8_t *copy=malloc((size_t)table.count);assert(copy);memcpy(copy,table.data,(size_t)table.count);copy[table.count-1]^=1;assert(!pt_admit(bytes(copy,table.count)));free(copy);
    puts("text: UTF8, actual capacities/aliases, piece/text budgets and exact table identity refusals pass");
}
int main(void){size_t n;void *data=read_file("psionic/fixtures/text/ch05/qwen35.pst",&n);coax_v_u8 table=bytes(data,(int64_t)n);assert(pt_admit(table));token_vectors(table);request_vectors(table);normalization(table);failures(table);free(data);}
