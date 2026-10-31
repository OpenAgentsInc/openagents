/* Fresh owned controller sample: ABI, caller storage and admission checks.
   Numerical expected fields are independently captured SDK facts, read by Python. */
#include "panda-owned-sample.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
static size_t assertions, boundaries, aliases;
#define NEED(x) do { ++assertions; if (!(x)) { fprintf(stderr,"owned sample native failure line %d\n",__LINE__); exit(1); } } while (0)
_Static_assert(sizeof(coax_t_PcSample)==144*8 && _Alignof(coax_t_PcSample)==8,"PcSample remains 144 F64 words");
#define FIELD(name,word) _Static_assert(offsetof(coax_t_PcSample,name)==(word)*8,"PcSample " #name " offset")
FIELD(position,0); FIELD(rotation,3); FIELD(q,12); FIELD(qd,19); FIELD(mass,26);
FIELD(jp,75); FIELD(jr,96); FIELD(velocity,117); FIELD(bias,123); FIELD(lower,130); FIELD(upper,137);
static coax_t_PcStatus (*const typed)(coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_t_PcSample)=pcs_forward;
static double real(uint64_t bits) { double value; memcpy(&value,&bits,8); return value; }
static void read_bytes(FILE *f,void *p,size_t n) { NEED(fread(p,1,n,f)==n); }
static void write_bytes(FILE *f,const void *p,size_t n) { NEED(fwrite(p,1,n,f)==n); }
static coax_t_PcSample admitted(double q[10],double v[10]) {
    struct { unsigned char before[32]; coax_t_PcSample samples[2]; unsigned char after[32]; } box, prior;
    double oldq[10],oldv[10],words[144]; memcpy(oldq,q,sizeof oldq); memcpy(oldv,v,sizeof oldv);
    memset(&box,0xa5,sizeof box); memcpy(&prior,&box,sizeof box);
    NEED(typed((coax_v_f64){q,7},(coax_v_f64){q+7,2},(coax_v_f64){v,7},(coax_v_f64){v+7,2},(coax_v_t_PcSample){box.samples,1})==COAX_PcStatus_ok);
    coax_t_PcSample raw;
    NEED(pcs_forward_raw((coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_t_PcSample){&raw,1})==COAX_PcStatus_ok);
    NEED(!memcmp(&box.samples[0],&raw,sizeof raw));
    NEED(!memcmp(&box.samples[1],&prior.samples[1],sizeof raw));
    NEED(!memcmp(box.before,prior.before,sizeof box.before) && !memcmp(box.after,prior.after,sizeof box.after));
    NEED(!memcmp(q,oldq,sizeof oldq) && !memcmp(v,oldv,sizeof oldv));
    memcpy(words,&raw,sizeof words); for (int i=0;i<144;i++) NEED(isfinite(words[i]));
    for (int i=0;i<7;i++) { NEED(raw.q.e[i]==q[i]); NEED(raw.qd.e[i]==v[i]); }
    return raw;
}
static void refused(double q[10],double v[10],const int64_t lens[5],coax_t_PcStatus expected) {
    coax_t_PcSample out[2],old[2]; double oq[10],ov[10];
    memset(out,0xa5,sizeof out); memcpy(old,out,sizeof old); memcpy(oq,q,sizeof oq); memcpy(ov,v,sizeof ov);
    NEED(typed((coax_v_f64){q,lens[0]},(coax_v_f64){q+7,lens[1]},(coax_v_f64){v,lens[2]},(coax_v_f64){v+7,lens[3]},(coax_v_t_PcSample){out,lens[4]})==expected);
    NEED(!memcmp(out,old,sizeof old)); NEED(!memcmp(q,oq,sizeof oq) && !memcmp(v,ov,sizeof ov));
}
static void refused_raw(double q[10],double v[10],const int64_t lens[3],coax_t_PcStatus expected) {
    coax_t_PcSample out[2],old[2]; double oq[10],ov[10];
    memset(out,0xa5,sizeof out); memcpy(old,out,sizeof old); memcpy(oq,q,sizeof oq); memcpy(ov,v,sizeof ov);
    NEED(pcs_forward_raw((coax_v_f64){q,lens[0]},(coax_v_f64){v,lens[1]},(coax_v_t_PcSample){out,lens[2]})==expected);
    NEED(!memcmp(out,old,sizeof old)); NEED(!memcmp(q,oq,sizeof oq) && !memcmp(v,ov,sizeof ov));
}
static void refused_both(double q[10],double v[10],coax_t_PcStatus status) {
    const int64_t t[]={7,2,7,2,1},r[]={9,9,1}; refused(q,v,t,status); refused_raw(q,v,r,status);
}
static void overlap(double q[10],double v[10]) {
    /* An entire output overlaps both inputs. On success those bytes are output;
       the complete pre-call input must have been read before publication. */
    coax_t_PcSample want=admitted(q,v);
    union Store { double words[160]; coax_t_PcSample sample; } storage;
    unsigned char old[sizeof storage];
    for (int mode=0;mode<2;mode++) {
        memset(&storage,0xa5,sizeof storage); memcpy(storage.words+4,q,9*8); memcpy(storage.words+24,v,9*8); memcpy(old,&storage,sizeof old);
        coax_t_PcStatus s=mode==0
          ?typed((coax_v_f64){storage.words+4,7},(coax_v_f64){storage.words+11,2},(coax_v_f64){storage.words+24,7},(coax_v_f64){storage.words+31,2},(coax_v_t_PcSample){&storage.sample,1})
          :pcs_forward_raw((coax_v_f64){storage.words+4,9},(coax_v_f64){storage.words+24,9},(coax_v_t_PcSample){&storage.sample,1});
        NEED(s==COAX_PcStatus_ok); NEED(!memcmp(&storage.sample,&want,sizeof want));
        NEED(!memcmp((unsigned char*)&storage+sizeof want,old+sizeof want,sizeof storage-sizeof want)); ++aliases;
        /* Refusal with the same overlap leaves every byte, including inputs. */
        memset(&storage,0xa5,sizeof storage); memcpy(storage.words+4,q,9*8); memcpy(storage.words+24,v,9*8); storage.words[4]=real(UINT64_C(0x7ff8000000000001)); memcpy(old,&storage,sizeof old);
        s=mode==0
          ?typed((coax_v_f64){storage.words+4,7},(coax_v_f64){storage.words+11,2},(coax_v_f64){storage.words+24,7},(coax_v_f64){storage.words+31,2},(coax_v_t_PcSample){&storage.sample,1})
          :pcs_forward_raw((coax_v_f64){storage.words+4,9},(coax_v_f64){storage.words+24,9},(coax_v_t_PcSample){&storage.sample,1});
        NEED(s==COAX_PcStatus_number); NEED(!memcmp(&storage,old,sizeof storage)); ++aliases;
    }
}
static void faults(void) {
    double q[10]={0},v[10]={0}; int64_t t[]={7,2,7,2,1},r[]={9,9,1};
    /* Poison values demonstrate that every malformed shape wins first. */
    q[0]=real(UINT64_C(0x7ff8000000000001)); v[0]=21;
    for (int field=0;field<5;field++) { int64_t good=t[field]; for(int64_t n=-1;n<=good+1;n++) if(n!=good) { t[field]=n; refused(q,v,t,COAX_PcStatus_shape); } t[field]=good; }
    for (int field=0;field<3;field++) { int64_t good=r[field]; for(int64_t n=-1;n<=good+1;n++) if(n!=good) { r[field]=n; refused_raw(q,v,r,COAX_PcStatus_shape); } r[field]=good; }
    NEED(typed((coax_v_f64){NULL,6},(coax_v_f64){NULL,2},(coax_v_f64){NULL,7},(coax_v_f64){NULL,2},(coax_v_t_PcSample){NULL,0})==COAX_PcStatus_shape);
    NEED(pcs_forward_raw((coax_v_f64){NULL,8},(coax_v_f64){NULL,9},(coax_v_t_PcSample){NULL,1})==COAX_PcStatus_shape);
    q[0]=v[0]=0;
    const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
    for (int role=0;role<2;role++) for(int axis=0;axis<9;axis++) {
        double *values=role==0?q:v; double limit=role==0?(axis<7?100:1):(axis<7?20:2);
        for(size_t k=0;k<sizeof bad/sizeof bad[0];k++) { values[axis]=real(bad[k]); refused_both(q,v,COAX_PcStatus_number); }
        for(int sign=-1;sign<=1;sign+=2) {
            values[axis]=sign*nextafter(limit,INFINITY); refused_both(q,v,COAX_PcStatus_range);
            values[axis]=sign*limit; (void)admitted(q,v); ++boundaries;
        }
        values[axis]=0;
    }
    q[0]=101; q[1]=real(UINT64_C(0x7ff8000000000001)); refused_both(q,v,COAX_PcStatus_range);
    q[0]=real(UINT64_C(0x7ff8000000000001)); q[1]=101; refused_both(q,v,COAX_PcStatus_number); q[0]=q[1]=0;
    q[8]=2; v[0]=real(UINT64_C(0x7ff8000000000001)); refused_both(q,v,COAX_PcStatus_range); q[8]=0;
    refused_both(q,v,COAX_PcStatus_number); v[0]=21; v[1]=real(UINT64_C(0x7ff8000000000001)); refused_both(q,v,COAX_PcStatus_range);
    v[0]=real(UINT64_C(0x7ff8000000000001)); v[1]=21; refused_both(q,v,COAX_PcStatus_number); v[0]=v[1]=0;
    v[6]=21; v[7]=real(UINT64_C(0x7ff8000000000001)); refused_both(q,v,COAX_PcStatus_range); v[6]=0;
    refused_both(q,v,COAX_PcStatus_number); v[7]=0;
}
int main(int argc,char **argv) {
    if(argc!=3) return 2;
    FILE *input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx"); NEED(input&&output);
    uint32_t header[2]; read_bytes(input,header,sizeof header); NEED(header[0]==UINT32_C(0x31534350)&&header[1]==15);
    for(unsigned n=0;n<15;n++) {
        double values[18],q[10]={0},v[10]={0}; read_bytes(input,values,sizeof values);
        for(int i=0;i<18;i++) NEED(isfinite(values[i])); memcpy(q,values,9*8); memcpy(v,values+9,9*8);
        coax_t_PcSample sample=admitted(q,v); write_bytes(output,&sample,sizeof sample);
        if(n==3) overlap(q,v);
    }
    NEED(fgetc(input)==EOF); faults(); NEED(!fclose(input)&&!fclose(output));
    printf("{\"schema\":\"psionic.panda-owned-sample.native.v1\",\"passed\":true,\"cases\":15,\"words_per_case\":144,\"inclusive_boundaries\":%zu,\"alias_cases\":%zu,\"assertions\":%zu}\n",boundaries,aliases,assertions);
    return 0;
}
