#include "decision-json-memory.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
int main(void){
    /* All 255 choice slots: names "o0".."o254", one dominant option. */
    enum{N=255};coax_t_PtOption *options=calloc(N,sizeof(*options));char (*names)[8]=calloc(N,8);double p[N];
    unsigned char *out=malloc(1<<16);assert(options&&names&&out);
    for(int i=0;i<N;i++){snprintf(names[i],8,"o%d",i);options[i].name=str(names[i]);p[i]=0.5/(N-1);}
    p[200]=0.5;
    coax_t_PjResult r=psionic_answer_json(COAX_QrKind_choice,0,(coax_v_f64){p,N},(coax_v_t_PtOption){options,N},(coax_v_u8){out,1<<16});
    assert(r.status==COAX_PjStatus_ok && r.written>0 && r.written<(1<<16));
    assert(!memcmp(out,"{\"type\":\"choice\",\"choice\":\"o200\",\"probabilities\":{\"o0\":0.001969,",63));
    const char *tail="\"o254\":0.001969},\"confidence\":0.498031}";assert(!memcmp(out+r.written-strlen(tail),tail,strlen(tail)));
    /* Refusals before writing: NaN, infinity, capacity and observed overlap. */
    memset(out,7,64);double bad[2]={NAN,0.5};
    coax_t_PtOption two[2]={{str("a"),{COAX_PtEntryKind_none,str("")}},{str("b"),{COAX_PtEntryKind_none,str("")}}};
    assert(psionic_answer_json(COAX_QrKind_choice,0,(coax_v_f64){bad,2},(coax_v_t_PtOption){two,2},(coax_v_u8){out,64}).status==COAX_PjStatus_number);
    bad[0]=INFINITY;assert(psionic_answer_json(COAX_QrKind_noul,0,(coax_v_f64){bad,2},(coax_v_t_PtOption){two,0},(coax_v_u8){out,64}).status==COAX_PjStatus_number);
    double half[2]={0.5,0.5};
    assert(psionic_answer_json(COAX_QrKind_choice,0,(coax_v_f64){half,2},(coax_v_t_PtOption){two,2},(coax_v_u8){out,8}).status==COAX_PjStatus_capacity);
    assert(psionic_answer_json(COAX_QrKind_choice,(uint8_t)2,(coax_v_f64){half,2},(coax_v_t_PtOption){two,2},(coax_v_u8){out,64}).status==COAX_PjStatus_shape);
    assert(psionic_answer_json((coax_t_QrKind)3,0,(coax_v_f64){half,2},(coax_v_t_PtOption){two,2},(coax_v_u8){out,64}).status==COAX_PjStatus_shape);
    assert(psionic_answer_json(COAX_QrKind_choice,0,(coax_v_f64){half,2},(coax_v_t_PtOption){two,2},(coax_v_u8){(unsigned char *)half,16}).status==COAX_PjStatus_alias);
    two[1].name=(coax_string){(char *)out+60,1};
    assert(psionic_answer_json(COAX_QrKind_choice,0,(coax_v_f64){half,2},(coax_v_t_PtOption){two,2},(coax_v_u8){out,64}).status==COAX_PjStatus_alias);
    assert(psionic_answer_json(COAX_QrKind_choice,0,(coax_v_f64){half,2},(coax_v_t_PtOption){two,2},(coax_v_u8){(unsigned char *)two,16}).status==COAX_PjStatus_alias);
    for(int i=0;i<64;i++)assert(out[i]==7);
    two[1].name=str("b");
    r=psionic_answer_json(COAX_QrKind_choice,1,(coax_v_f64){half,2},(coax_v_t_PtOption){two,2},(coax_v_u8){out,64});
    assert(r.status==COAX_PjStatus_ok && r.written==64 && !memcmp(out,"{\"type\":\"choice\",\"choice\":\"a\",\"probabilities\":{\"a\":0.5,\"b\":0.5}}",64));
    free(options);free(names);free(out);puts("answer JSON native: 255 options, number refusals, capacity and observed overlap pass");
}
