/* Text fixtures only; no model weights, network, services or GPU calls. */
#include "deepseek-front-v41-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "front-v41-goldens.h"
#include "front-goldens.h"
#define LEN(x) ((int64_t)(sizeof(x)/sizeof((x)[0])))
#define V(x) (coax_v_s64){x,LEN(x)}
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static void texts(coax_v_u8 table){
    int64_t out[4096],stage[4096],symbols[4096];coax_t_PtLayout layout=pt_layout(table);assert(layout.status==COAX_PtStatus_ok);
    for(size_t c=0;c<sizeof(dvf_texts)/sizeof(dvf_texts[0]);c++){
        coax_string text={(const char *)dvf_texts[c],dvf_lengths[c]};for(size_t i=0;i<4096;i++)out[i]=-77;
        coax_t_PtResult r=psionic_dvf_text(table,text,V(out),V(stage),V(symbols));
        if(r.status!=COAX_PtStatus_ok || r.written!=dvf_ids_counts[c] || memcmp(out,dvf_ids[c],(size_t)r.written*8))fprintf(stderr,"text case %zu: status %u, got %lld expected %lld\n",c,r.status,(long long)r.written,(long long)dvf_ids_counts[c]);
        assert(r.status==COAX_PtStatus_ok && r.written==dvf_ids_counts[c]);assert(!memcmp(out,dvf_ids[c],(size_t)r.written*8));assert(out[r.written]==-77);
        int64_t at=0,count=0;
        while(at<text.count){int64_t partition=dvf_partition(table,layout,text,at);coax_string part={text.data+at,partition-at};int64_t pos=0;
            while(pos<part.count){pos=dvf_piece(table,layout,part,pos);assert(count<dvf_ends_counts[c]);if(pos+at!=dvf_ends[c][count])fprintf(stderr,"span case %zu at %lld got %lld expected %lld\n",c,(long long)count,(long long)(pos+at),(long long)dvf_ends[c][count]);assert(pos+at==dvf_ends[c][count++]);}at=partition;
        }assert(count==dvf_ends_counts[c]);
    }
    puts("V4.1: every retained original text ID and three-stage split endpoint agrees");
}
static void vocabulary(coax_v_u8 table){
    uint8_t *bytes=malloc(DVF_DECODE_BYTES);assert(bytes);size_t at=0;int64_t out[4096],stage[4096];
    for(int64_t id=3;id<128000;id++){
        coax_t_PtResult r=psionic_dvf_units(table,(coax_v_s64){&id,1},V(out),V(stage));assert(r.status==COAX_PtStatus_ok && r.written>0 && r.written<=4096 && at+4+(size_t)r.written<=DVF_DECODE_BYTES);
        for(int i=0;i<4;i++){bytes[at++]=(uint8_t)((uint64_t)r.written>>(8*i));}
        for(int64_t i=0;i<r.written;i++){assert(out[i]>=0 && out[i]<256);bytes[at++]=(uint8_t)out[i];}
    }
    assert(at==DVF_DECODE_BYTES && pw_digest((coax_v_u8){bytes,(int64_t)at},str(DVF_DECODE_SHA)));free(bytes);
    for(int64_t id=2;id<129280;id++){
        if(id>=3 && id<128000)continue;
        _Bool control=id==128799 || id==128803 || id==128804 || id==128821 || id==128822 || id==128825;
        out[0]=-77;coax_t_PtResult r=psionic_dvf_units(table,(coax_v_s64){&id,1},V(out),V(stage));assert(r.status==(control?COAX_PtStatus_ok:COAX_PtStatus_unsupported));if(!control)assert(out[0]==-77);
    }
    puts("V4.1: all 127997 ordinary vocabulary byte strings match the original-data digest; every added ID is mapped or refused explicitly");
}
static void unicode_properties(coax_v_u8 table){
    uint8_t *flags=malloc(0x110000);assert(flags);coax_t_PtLayout layout=pt_layout(table);assert(layout.status==COAX_PtStatus_ok);
    for(int64_t cp=0;cp<0x110000;cp++){int64_t value=dvf_probe_flags(table,layout,cp);assert(value>=0 && value<=255);flags[cp]=(uint8_t)value;}
    assert(pw_digest((coax_v_u8){flags,0x110000},str(DVF_UNICODE_SHA)));free(flags);
    puts("V4.1: Coax property lookup at all 1114112 code points matches the independently pinned Unicode 16 data digest");
}
static void boundaries(coax_v_u8 table){
    int64_t out[4096],stage[4096],temp[4096],symbols[4096],before[4096];uint8_t raw[8192];
    const char *literal="<think>｜DSML｜<｜User｜>e\xcc\x81";coax_t_DfSegment segments[3]={{.text=str(literal)},{.special=1,.control=COAX_DfSpecial_user},{.text=str("你好")}};
    coax_t_PtResult r=psionic_dvf_encode(table,(coax_v_t_DfSegment){segments,3},V(out),V(stage),V(temp),(coax_v_u8){raw,sizeof(raw)},V(symbols));assert(r.status==COAX_PtStatus_ok);
    int64_t special=-1;for(int64_t i=0;i<r.written;i++){if(out[i]>=128000){assert(out[i]==128803 && special==-1);special=i;}else assert(out[i]>=3);}assert(special>0);
    /* All UTF-8 byte splits rejoin before validation/BPE, even inside scalars. */
    int64_t reference[4096];coax_t_PtResult ordinary=psionic_dvf_text(table,str(literal),V(reference),V(stage),V(symbols));assert(ordinary.status==COAX_PtStatus_ok && ordinary.written==special);
    for(int64_t split=0;split<=(int64_t)strlen(literal);split++){segments[0]=(coax_t_DfSegment){.text={literal,split}};segments[1]=(coax_t_DfSegment){.text={literal+split,(int64_t)strlen(literal)-split}};
        r=psionic_dvf_encode(table,(coax_v_t_DfSegment){segments,2},V(out),V(stage),V(temp),(coax_v_u8){raw,sizeof(raw)},V(symbols));assert(r.status==COAX_PtStatus_ok && r.written==ordinary.written && !memcmp(out,reference,(size_t)r.written*8));}
    assert(psionic_dvf_text(table,str("a"),V(out),V(out),V(symbols)).status==COAX_PtStatus_alias);
    segments[0]=(coax_t_DfSegment){.text={(const char *)out,1}};assert(psionic_dvf_encode(table,(coax_v_t_DfSegment){segments,1},V(out),V(stage),V(temp),(coax_v_u8){raw,sizeof(raw)},V(symbols)).status==COAX_PtStatus_alias);
    uint8_t invalid[]={'x',255};for(size_t i=0;i<4096;i++)out[i]=-1000-(int64_t)i;memcpy(before,out,sizeof(out));
    segments[0]=(coax_t_DfSegment){.text=str("ab")};segments[1]=(coax_t_DfSegment){.special=1,.control=COAX_DfSpecial_eos};segments[2]=(coax_t_DfSegment){.text={(const char *)invalid,2}};
    r=psionic_dvf_encode(table,(coax_v_t_DfSegment){segments,3},V(out),V(stage),V(temp),(coax_v_u8){raw,sizeof(raw)},V(symbols));assert(r.status==COAX_PtStatus_utf8 && !memcmp(out,before,sizeof(out)));
    segments[2].text=str("c");r=psionic_dvf_encode(table,(coax_v_t_DfSegment){segments,3},(coax_v_s64){out,1},V(stage),V(temp),(coax_v_u8){raw,sizeof(raw)},V(symbols));assert(r.status==COAX_PtStatus_capacity && !memcmp(out,before,sizeof(out)));
    int64_t ids[2]={128822,128000};r=psionic_dvf_units(table,V(ids),V(out),V(stage));assert(r.status==COAX_PtStatus_unsupported && !memcmp(out,before,sizeof(out)));
    assert(psionic_dvf_units(table,V(ids),V(out),V(out)).status==COAX_PtStatus_alias);
    puts("V4.1: marker spellings remain ordinary, every UTF-8 segment split agrees, aliases and late failures preserve output");
}
static void recipe_tokens(coax_v_u8 table){
    const int64_t counts[]={1,4,3,4};
    const int roles[4][4]={{1},{0,1,2,1},{1,3,1},{0,1,2,3}};
    const char *content[4][4]={{"<think>｜DSML｜你好"},{"rules","Hi","answer","next"},{"a","b","c"},{"rules","<think>","answer","｜DSML｜"}};
    for(int c=0;c<4;c++)for(int thinking=0;thinking<2;thinking++){
        coax_t_DfMessage messages[4];for(int64_t i=0;i<counts[c];i++)messages[i]=(coax_t_DfMessage){.role=(coax_t_DfRole)roles[c][i],.content=str(content[c][i])};
        coax_t_DfSegment segments[128],work[128];coax_t_DfRecipe recipe=df_recipe((coax_v_t_DfMessage){messages,counts[c]},thinking,(coax_v_t_DfSegment){segments,128},(coax_v_t_DfSegment){work,128},0);assert(recipe.status==COAX_DfStatus_ok);
        int64_t out[4096],stage[4096],temp[4096],symbols[4096];uint8_t raw[8192];coax_t_PtResult r=psionic_dvf_encode(table,(coax_v_t_DfSegment){segments,recipe.written},V(out),V(stage),V(temp),(coax_v_u8){raw,sizeof(raw)},V(symbols));assert(r.status==COAX_PtStatus_ok && r.written==dvf_recipe_counts[2*c+thinking] && !memcmp(out,dvf_recipes[2*c+thinking],(size_t)r.written*8));
    }
    puts("V4.1: eight bounded chat/thinking recipes match independent original-tokenizer IDs");
}
static void stream(coax_v_u8 table){
    /* Encode the owned independent protocol oracle into actual original IDs. */
    coax_t_DfSegment segments[256];uint8_t raw[1024];int64_t n=0,bytes=0,start=0;
    for(int64_t i=0;i<LEN(DFF_UNITS);i++){int64_t unit=DFF_UNITS[i];if(unit<256)raw[bytes++]=(uint8_t)unit;else{if(bytes>start)segments[n++]=(coax_t_DfSegment){.text={(const char *)raw+start,bytes-start}};segments[n++]=(coax_t_DfSegment){.special=1,.control=(coax_t_DfSpecial)(unit-256)};start=bytes;}}
    if(bytes>start)segments[n++]=(coax_t_DfSegment){.text={(const char *)raw+start,bytes-start}};
    int64_t ids[4096],stage[4096],temp[4096],symbols[4096],units[4096],unit_stage[4096];uint8_t text[4096];
    coax_t_PtResult encoded=psionic_dvf_encode(table,(coax_v_t_DfSegment){segments,n},V(ids),V(stage),V(temp),(coax_v_u8){text,sizeof(text)},V(symbols));assert(encoded.status==COAX_PtStatus_ok);
    int64_t splits=0;
    for(int64_t a=0;a<=encoded.written;a++)for(int64_t b=a;b<=encoded.written;b++){
        coax_t_DfState state={0};coax_t_DfEvent out[4096],event_stage[4096];int64_t seen=0,edges[]={0,a,b,encoded.written};
        for(int c=1;c<4;c++){coax_t_PtResult decoded=psionic_dvf_units(table,(coax_v_s64){ids+edges[c-1],edges[c]-edges[c-1]},V(units),V(unit_stage));assert(decoded.status==COAX_PtStatus_ok);
            coax_t_DfResult feed=df_feed(state,(coax_v_s64){units,decoded.written},(coax_v_t_DfEvent){out,4096},(coax_v_t_DfEvent){event_stage,4096},0);assert(feed.status==COAX_DfStatus_ok);
            for(int64_t j=0;j<feed.written;j++,seen++){assert(seen<LEN(DFF_BYTES));assert(out[j].kind==DFF_KINDS[seen] && out[j].call==DFF_CALLS[seen] && out[j].byte==DFF_BYTES[seen]);}state=feed.state;assert(df_end(state,0).status==COAX_DfStatus_incomplete);
        }assert(seen==LEN(DFF_BYTES) && df_end(state,1).status==COAX_DfStatus_ok);splits++;
    }
    for(int64_t i=0;i<encoded.written;i++){
        coax_t_PtResult d=psionic_dvf_units(table,(coax_v_s64){ids,i},V(units),V(unit_stage));assert(d.status==COAX_PtStatus_ok);coax_t_DfEvent out[4096],work[4096];coax_t_DfResult f=df_feed((coax_t_DfState){0},(coax_v_s64){units,d.written},(coax_v_t_DfEvent){out,4096},(coax_v_t_DfEvent){work,4096},0);assert(f.status==COAX_DfStatus_ok && df_end(f.state,1).status==COAX_DfStatus_incomplete);
    }
    printf("V4.1: %lld actual token IDs, %lld token split pairs and every truncation match all %lld independent events; finish remains explicit\n",(long long)encoded.written,(long long)splits,(long long)LEN(DFF_BYTES));
}
int main(int argc,char **argv){assert(argc==2);FILE *f=fopen(argv[1],"rb");assert(f && !fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0 && !fseek(f,0,SEEK_SET));uint8_t *data=malloc((size_t)size);assert(data && fread(data,1,(size_t)size,f)==(size_t)size && !fclose(f));coax_v_u8 table={data,size};assert(pw_digest(table,str(DVF_SHA)));data[size-1]^=1;assert(!pw_digest(table,str(DVF_SHA)));data[size-1]^=1;texts(table);vocabulary(table);unicode_properties(table);boundaries(table);recipe_tokens(table);stream(table);free(data);return 0;}
