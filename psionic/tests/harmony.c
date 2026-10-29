#include "harmony-memory.h"
#include "harmony-goldens.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_v_u8 bv(void *p,int64_t n){return (coax_v_u8){p,n};}
static coax_v_s64 iv(void *p,int64_t n){return (coax_v_s64){p,n};}
static coax_v_t_HmRecord rv(void *p,int64_t n){return (coax_v_t_HmRecord){p,n};}
static coax_v_t_HmMessage mv(void *p,int64_t n){return (coax_v_t_HmMessage){p,n};}
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static uint8_t *read_file(const char *path,size_t *n){FILE *f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long length=ftell(f);assert(length>=0);rewind(f);uint8_t *p=malloc((size_t)length+1);assert(p);assert(fread(p,1,(size_t)length,f)==(size_t)length);assert(!fclose(f));*n=(size_t)length;return p;}
static int64_t output[16385],stage[16385],symbols[1024];
static uint8_t decoded[65537],work[65537];
static coax_t_HmRecord records[65],draft[65];
static void equal(coax_string s,int64_t begin,int64_t end){assert(end>=begin && s.count==end-begin && !memcmp(decoded+begin,s.data,(size_t)s.count));}
static void match_records(coax_t_HmMessage *messages,int n){
    for(int i=0;i<n;i++){coax_t_HmMessage m=messages[i];coax_t_HmRecord r=records[i];assert(r.role==m.role && r.channel==m.channel && r.ending==m.ending);equal(m.name,r.name_begin,r.name_end);equal(m.recipient,r.recipient_begin,r.recipient_end);equal(m.body,r.body_begin,r.body_end);}
}
static void references(coax_v_u8 table){
    for(size_t j=0;j<sizeof(ht_cases)/sizeof(*ht_cases);j++){
        memset(output,0xff,sizeof output);coax_t_PtResult r=psionic_harmony_encode(table,ht_cases[j].text,iv(output,16385),iv(stage,16385),iv(symbols,1024));
        if(r.status!=0 || r.written!=ht_cases[j].count || r.work!=ht_cases[j].work || memcmp(output,ht_cases[j].ids,(size_t)ht_cases[j].count*8)){fprintf(stderr,"Harmony encode case %zu: status %u, count %lld/%d\n",j,r.status,(long long)r.written,ht_cases[j].count);abort();}
        assert(output[r.written]==-1);memset(decoded,0xa5,sizeof decoded);
        r=psionic_harmony_decode(table,iv(output,r.written),bv(decoded,65537),bv(work,65537));
        assert(r.status==0 && r.written==ht_cases[j].text.count && !memcmp(decoded,ht_cases[j].text.data,(size_t)r.written) && decoded[r.written]==0xa5);
    }
    for(int e=0;e<3;e++){
        coax_t_PtResult r=psionic_harmony_system(e,bv(decoded,65537),bv(work,65537));coax_string expected=hr_cases[e+1].messages[0].body;
        assert(r.status==0 && r.written==expected.count && !memcmp(decoded,expected.data,(size_t)expected.count));
    }
    for(size_t j=0;j<sizeof(hr_cases)/sizeof(*hr_cases);j++){
        coax_t_HmResult r=psionic_harmony_render(table,mv(hr_cases[j].messages,hr_cases[j].count),hr_cases[j].completion,iv(output,16385),iv(stage,16385),iv(symbols,1024));
        assert(r.status==0 && r.written==hr_cases[j].tokens && !memcmp(output,hr_cases[j].ids,(size_t)r.written*8));
        int64_t n=r.written-(hr_cases[j].completion?2:0); /* original assistant role is one token */
        decoded[65536]=0xa5;records[64].body_end=-7;
        r=psionic_harmony_parse(table,iv(output,n),false,rv(records,65),rv(draft,65),bv(decoded,65537),bv(work,65537));
        assert(r.status==0 && r.written==hr_cases[j].count && r.consumed==n && decoded[65536]==0xa5 && records[64].body_end==-7);
        match_records(hr_cases[j].messages,hr_cases[j].count);
    }
    for(size_t j=0;j<sizeof(hp_cases)/sizeof(*hp_cases);j++){
        memcpy(output,hp_cases[j].ids,(size_t)hp_cases[j].tokens*8);
        coax_t_HmResult r=psionic_harmony_parse(table,iv(output,hp_cases[j].tokens),true,rv(records,65),rv(draft,65),bv(decoded,65537),bv(work,65537));
        assert(r.status==0 && r.written==hp_cases[j].count && r.consumed==hp_cases[j].consumed && r.stop==hp_cases[j].stop);match_records(hp_cases[j].messages,hp_cases[j].count);
    }
    for(int64_t id=199998;id<201088;id++){
        coax_t_PtResult r=psionic_harmony_decode(table,iv(&id,1),bv(decoded,65537),bv(work,65537));assert(r.status==0);
        if(id>=200013){char expected[64];int length=snprintf(expected,sizeof expected,"<|reserved_%lld|>",(long long)id);assert(r.written==length && !memcmp(decoded,expected,(size_t)length));}
    }
    puts("Harmony: 123 original-rank text vectors, 7 renders, 4 completion streams and all 1,090 special IDs pass");
}
static void refusals(coax_v_u8 table){
    coax_t_HmMessage m={COAX_HmRole_assistant,COAX_HmChannel_final,COAX_HmEnd_turn,{"",0},{"",0},{"ok",2}};
    coax_t_HmResult rendered=psionic_harmony_render(table,mv(&m,1),false,iv(output,16385),iv(stage,16385),iv(symbols,1024));assert(rendered.status==0);int64_t n=rendered.written;
    uint8_t saved_bytes[65537];coax_t_HmRecord saved_records[65];memset(decoded,0xa5,sizeof decoded);memset(records,0x55,sizeof records);memcpy(saved_bytes,decoded,sizeof decoded);memcpy(saved_records,records,sizeof records);
    /* Every proper prefix is incomplete; no metadata or decoded bytes publish. */
    for(int64_t cut=1;cut<n;cut++){
        coax_t_HmResult r=psionic_harmony_parse(table,iv(output,cut),false,rv(records,65),rv(draft,65),bv(decoded,65537),bv(work,65537));
        assert(r.status==COAX_HmStatus_incomplete && r.written==0 && r.bytes==0 && r.consumed==0 && !memcmp(saved_bytes,decoded,sizeof decoded) && !memcmp(saved_records,records,sizeof records));
    }
    int64_t old=output[n-1];int64_t invalid[]={-1,201088,200003,200006,200005};
    for(size_t i=0;i<sizeof invalid/sizeof *invalid;i++){
        output[n-1]=invalid[i];coax_t_HmResult r=psionic_harmony_parse(table,iv(output,n),false,rv(records,65),rv(draft,65),bv(decoded,65537),bv(work,65537));
        assert(r.status!=0 && !memcmp(saved_bytes,decoded,sizeof decoded) && !memcmp(saved_records,records,sizeof records));
    }output[n-1]=old;
    assert(psionic_harmony_parse(table,iv(output,n),false,rv(records,0),rv(draft,65),bv(decoded,65537),bv(work,65537)).status==COAX_HmStatus_capacity);
    assert(psionic_harmony_parse(table,iv(output,n),false,rv(records,65),rv(draft,0),bv(decoded,65537),bv(work,65537)).status==COAX_HmStatus_capacity);
    assert(psionic_harmony_parse(table,iv(output,n),false,rv(records,65),rv(draft,65),bv(decoded,1),bv(work,65537)).status==COAX_HmStatus_capacity);
    assert(psionic_harmony_parse(table,iv(output,n),false,rv(records,65),rv(draft,65),bv(decoded,65537),bv(work,1)).status!=0);
    assert(!memcmp(saved_bytes,decoded,sizeof decoded) && !memcmp(saved_records,records,sizeof records));
    assert(psionic_harmony_parse(table,iv(output,n),false,rv(records,65),rv(records,65),bv(decoded,65537),bv(work,65537)).status==COAX_HmStatus_alias);
    assert(psionic_harmony_parse(table,iv(output,n),false,rv(records,65),rv(draft,65),bv(decoded,65537),bv(decoded,65537)).status==COAX_HmStatus_alias);
    assert(psionic_harmony_parse(table,iv(output,n),false,rv(records,1),rv(draft,65),bv(records,sizeof *records),bv(work,65537)).status==COAX_HmStatus_alias);
    assert(psionic_harmony_parse(table,iv(output,n),false,rv(records,65),rv(draft,65),bv(output,n*8),bv(work,65537)).status==COAX_HmStatus_alias);
    assert(psionic_harmony_encode(table,str("hello"),iv(output,8),iv(output,8),iv(symbols,1024)).status==COAX_PtStatus_alias);
    assert(psionic_harmony_encode(table,(coax_string){(char *)output,3},iv(output,8),iv(stage,8),iv(symbols,1024)).status==COAX_PtStatus_alias);
    assert(psionic_harmony_decode(table,iv(output,n),bv(decoded,65537),bv(decoded,65537)).status==COAX_PtStatus_alias);
    m.body=(coax_string){(char *)output,3};assert(psionic_harmony_render(table,mv(&m,1),false,iv(output,16385),iv(stage,16385),iv(symbols,1024)).status==COAX_HmStatus_alias);m.body=str("ok");
    m.name=(coax_string){(char *)stage,1};assert(psionic_harmony_render(table,mv(&m,1),false,iv(output,16385),iv(stage,16385),iv(symbols,1024)).status==COAX_HmStatus_alias);m.name=str("");
    m.recipient=(coax_string){(char *)symbols,1};assert(psionic_harmony_render(table,mv(&m,1),false,iv(output,16385),iv(stage,16385),iv(symbols,1024)).status==COAX_HmStatus_alias);m.recipient=str("");
    int64_t before=output[0];assert(psionic_harmony_render(table,mv(&m,1),false,iv(output,0),iv(stage,16385),iv(symbols,1024)).status==COAX_HmStatus_capacity && output[0]==before);
    uint8_t malformed[]={0xc0,0x80};assert(psionic_harmony_encode(table,(coax_string){(char *)malformed,2},iv(output,8),iv(stage,8),iv(symbols,1024)).status==COAX_PtStatus_utf8 && output[0]==before);
    int64_t bad_tokens[]={200008,188,200002}; /* rank 188 is byte zero, valid UTF8; find a lone continuation byte below */
    int64_t continuation=0;
    for(int64_t id=0;id<256;id++){coax_t_PtResult r=psionic_harmony_decode(table,iv(&id,1),bv(work,65537),bv(decoded,65537));if(r.status==0 && r.written==1 && work[0]==0x80){continuation=id;break;}}
    bad_tokens[1]=continuation;memset(decoded,0xa5,sizeof decoded);
    assert(psionic_harmony_parse(table,iv(bad_tokens,3),true,rv(records,65),rv(draft,65),bv(decoded,65537),bv(work,65537)).cause==COAX_PtStatus_utf8 && decoded[0]==0xa5);
    uint8_t word[1025];memset(word,'x',sizeof word);assert(psionic_harmony_encode(table,(coax_string){(char *)word,1025},iv(output,16385),iv(stage,16385),iv(symbols,1024)).status==COAX_PtStatus_budget);
    uint8_t *large=malloc(65537);assert(large);memset(large,'x',65537);assert(psionic_harmony_encode(table,(coax_string){(char *)large,65537},iv(output,16385),iv(stage,16385),iv(symbols,1024)).status==COAX_PtStatus_size);free(large);
    puts("Harmony: incomplete/malformed streams, capacities, nested aliases and byte limits preserve public outputs");
}
static void boundaries(coax_v_u8 table){
    uint8_t body[1020];for(size_t i=0;i<sizeof body;i++)body[i]=(i%64==63)?'a':' ';
    coax_t_HmMessage messages[65];for(int i=0;i<65;i++)messages[i]=(coax_t_HmMessage){COAX_HmRole_user,0,0,{"",0},{"",0},{(const char *)body,1020}};
    coax_t_HmResult r=psionic_harmony_render(table,mv(messages,64),false,iv(output,16385),iv(stage,16385),iv(symbols,1024));
    assert(r.status==0 && r.bytes==65536 && r.written<=16384);int64_t count=r.written;
    decoded[65536]=0xa5;records[64].body_end=-7;
    r=psionic_harmony_parse(table,iv(output,count),false,rv(records,65),rv(draft,65),bv(decoded,65536),bv(work,65536));
    assert(r.status==0 && r.written==64 && r.bytes==65536 && decoded[65536]==0xa5 && records[64].body_end==-7);
    output[0]=-7;
    assert(psionic_harmony_render(table,mv(messages,65),false,iv(output,16385),iv(stage,16385),iv(symbols,1024)).status==COAX_HmStatus_size && output[0]==-7);
    assert(psionic_harmony_render(table,mv(messages,64),true,iv(output,16385),iv(stage,16385),iv(symbols,1024)).status==COAX_HmStatus_size && output[0]==-7);
    memset(body,' ',sizeof body);
    r=psionic_harmony_render(table,mv(messages,64),false,iv(output,16385),iv(stage,16385),iv(symbols,1024));
    assert(r.status==COAX_HmStatus_tokenizer && r.cause==COAX_PtStatus_budget && output[0]==-7);
    puts("Harmony: 64 messages and exactly 65,536 decoded bytes pass; count, prefix and aggregate work overruns refuse");
}
int main(void){
    size_t n;uint8_t *data=read_file("psionic/fixtures/harmony/gptoss.hst",&n);coax_v_u8 table=bv(data,(int64_t)n);
    assert(hs_admit(table));data[n-1]^=1;assert(!hs_admit(table));data[n-1]^=1;assert(!hs_admit(bv(data,(int64_t)n-1)));
    references(table);refusals(table);boundaries(table);free(data);puts("Harmony native gate passes");
}
