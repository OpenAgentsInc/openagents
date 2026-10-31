/* Effects/ABI witness only; all force arithmetic is emitted from Coax. */
#include "panda-fluid.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
static size_t checks,boundaries,aliases;
#define NEED(v) do{++checks;if(!(v)){fprintf(stderr,"Panda fluid native failure line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PidOutput)==72 && sizeof(coax_t_PflOutput)==216 && _Alignof(coax_t_PflOutput)==8 && offsetof(coax_t_PflOutput,joint_passive)==72 && offsetof(coax_t_PflOutput,passive)==144,"three9word force fields");
static coax_t_PfkStatus(*const typed)(coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_t_PflOutput)=pfl_forward;
static double real(uint64_t u){double v;memcpy(&v,&u,8);return v;}
static void get(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static coax_t_PflOutput poison(int b){coax_t_PflOutput p;for(int i=0;i<7;i++){p.fluid.hinge.e[i]=b+i;p.joint_passive.hinge.e[i]=b+9+i;p.passive.hinge.e[i]=b+18+i;}for(int i=0;i<2;i++){p.fluid.slide.e[i]=b+7+i;p.joint_passive.slide.e[i]=b+16+i;p.passive.slide.e[i]=b+25+i;}return p;}
static void finite_output(const coax_t_PflOutput*p){for(int i=0;i<7;i++){NEED(isfinite(p->fluid.hinge.e[i]));NEED(isfinite(p->joint_passive.hinge.e[i]));NEED(isfinite(p->passive.hinge.e[i]));}for(int i=0;i<2;i++){NEED(isfinite(p->fluid.slide.e[i]));NEED(isfinite(p->joint_passive.slide.e[i]));NEED(isfinite(p->passive.slide.e[i]));}}
static void refused(double*q,double*v,const int64_t n[5],coax_t_PfkStatus expected){
 double oq[10],ov[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);coax_t_PflOutput out[2]={poison(1001),poison(2001)},before[2];memcpy(before,out,sizeof out);
 NEED(typed((coax_v_f64){q,n[0]},(coax_v_f64){q+7,n[1]},(coax_v_f64){v,n[2]},(coax_v_f64){v+7,n[3]},(coax_v_t_PflOutput){out,n[4]})==expected);NEED(!memcmp(before,out,sizeof out));NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov));
}
static void refused_raw(double*q,double*v,const int64_t n[3],coax_t_PfkStatus expected){
 double oq[10],ov[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);coax_t_PflOutput out[2]={poison(1001),poison(2001)},before[2];memcpy(before,out,sizeof out);
 NEED(pfl_forward_raw((coax_v_f64){q,n[0]},(coax_v_f64){v,n[1]},(coax_v_t_PflOutput){out,n[2]})==expected);NEED(!memcmp(before,out,sizeof out));NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov));
}
static void refused_both(double*q,double*v,coax_t_PfkStatus expected){const int64_t t[]={7,2,7,2,1},r[]={9,9,1};refused(q,v,t,expected);refused_raw(q,v,r,expected);}
static void observed(double*q,double*v,coax_t_PflOutput*result){
 struct Box{unsigned char prefix[32];coax_t_PflOutput out[2];unsigned char tail[32];}box,before;memset(&box,0xa5,sizeof box);box.out[0]=poison(1001);box.out[1]=poison(2001);memcpy(&before,&box,sizeof box);double oq[10],ov[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);coax_t_PflOutput raw=poison(1001);
 coax_t_PfkStatus status=typed((coax_v_f64){q,7},(coax_v_f64){q+7,2},(coax_v_f64){v,7},(coax_v_f64){v+7,2},(coax_v_t_PflOutput){box.out,1});coax_t_PfkStatus rs=pfl_forward_raw((coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_t_PflOutput){&raw,1});NEED(status==COAX_PfkStatus_ok&&rs==status);NEED(!memcmp(&box.out[0],&raw,sizeof raw));NEED(!memcmp(&box.out[1],&before.out[1],sizeof raw));NEED(!memcmp(box.prefix,before.prefix,sizeof box.prefix)&&!memcmp(box.tail,before.tail,sizeof box.tail));NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov));finite_output(&box.out[0]);memcpy(result,&box.out[0],sizeof *result);
}
static void faults(void){
 double q[10]={0},v[10]={0};int64_t t[]={7,2,7,2,1},r[]={9,9,1};q[0]=real(UINT64_C(0x7ff8000000000001));v[0]=21;
 for(int field=0;field<5;field++){int64_t good=t[field];for(int64_t n=-1;n<=good+1;n++)if(n!=good){t[field]=n;refused(q,v,t,COAX_PfkStatus_shape);}t[field]=INT64_MIN;refused(q,v,t,COAX_PfkStatus_shape);t[field]=INT64_MAX;refused(q,v,t,COAX_PfkStatus_shape);t[field]=good;}
 for(int field=0;field<3;field++){int64_t good=r[field];for(int64_t n=-1;n<=good+1;n++)if(n!=good){r[field]=n;refused_raw(q,v,r,COAX_PfkStatus_shape);}r[field]=INT64_MIN;refused_raw(q,v,r,COAX_PfkStatus_shape);r[field]=INT64_MAX;refused_raw(q,v,r,COAX_PfkStatus_shape);r[field]=good;}
 NEED(typed((coax_v_f64){NULL,6},(coax_v_f64){NULL,2},(coax_v_f64){NULL,7},(coax_v_f64){NULL,2},(coax_v_t_PflOutput){NULL,0})==COAX_PfkStatus_shape);NEED(pfl_forward_raw((coax_v_f64){NULL,8},(coax_v_f64){NULL,9},(coax_v_t_PflOutput){NULL,1})==COAX_PfkStatus_shape);
 memset(q,0,sizeof q);memset(v,0,sizeof v);const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int role=0;role<2;role++)for(int axis=0;axis<9;axis++){double*values=role?v:q;for(int i=0;i<5;i++){values[axis]=real(bad[i]);refused_both(q,v,COAX_PfkStatus_number);}values[axis]=0;double limit=role?(axis<7?20.:2.):(axis<7?100.:1.);for(int neg=0;neg<2;neg++){double edge=neg?-limit:limit;values[axis]=edge;coax_t_PflOutput output;observed(q,v,&output);++boundaries;values[axis]=nextafter(edge,neg?-INFINITY:INFINITY);refused_both(q,v,COAX_PfkStatus_range);}values[axis]=0;}
 q[0]=101;v[0]=real(bad[3]);refused_both(q,v,COAX_PfkStatus_range);q[0]=real(bad[3]);v[0]=21;refused_both(q,v,COAX_PfkStatus_number);q[0]=0;q[7]=2;refused_both(q,v,COAX_PfkStatus_range);q[7]=real(bad[3]);refused_both(q,v,COAX_PfkStatus_number);q[7]=0;v[0]=21;v[7]=real(bad[3]);refused_both(q,v,COAX_PfkStatus_range);v[0]=real(bad[3]);v[7]=3;refused_both(q,v,COAX_PfkStatus_number);
}
static void overlap(void){
 struct At{double prefix[6];coax_t_PflOutput out;double tail[7];};union Store{double words[40];struct At at;}storage;_Static_assert(offsetof(struct At,out)==48 && sizeof(struct At)==320,"overlap backing");double q[10]={0},v[10]={0};for(int i=0;i<9;i++)q[i]=v[i]=.125;coax_t_PflOutput want;observed(q,v,&want);
 for(int mode=0;mode<2;mode++)for(int invalid=0;invalid<2;invalid++){for(int i=0;i<40;i++)storage.words[i]=5001+i;for(int i=0;i<12;i++)storage.words[i]=.125;if(invalid)storage.words[0]=real(UINT64_C(0x7ff8000000000001));unsigned char before[sizeof storage];memcpy(before,&storage,sizeof storage);
  coax_t_PfkStatus status=mode?pfl_forward_raw((coax_v_f64){storage.words,9},(coax_v_f64){storage.words+3,9},(coax_v_t_PflOutput){&storage.at.out,1}):typed((coax_v_f64){storage.words,7},(coax_v_f64){storage.words+7,2},(coax_v_f64){storage.words+3,7},(coax_v_f64){storage.words+10,2},(coax_v_t_PflOutput){&storage.at.out,1});
  NEED(status==(invalid?COAX_PfkStatus_number:COAX_PfkStatus_ok));if(invalid)NEED(!memcmp(before,&storage,sizeof storage));else{NEED(!memcmp(&storage.at.out,&want,sizeof want));NEED(!memcmp(before,&storage,48));NEED(!memcmp(before+264,(unsigned char*)&storage+264,sizeof storage-264));}++aliases;
 }
}
int main(int argc,char**argv){
 NEED(argc==4);uint64_t fpcr;__asm__ volatile("mrs %0, fpcr":"=r"(fpcr));NEED(fpcr==0);FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wb"),*profile=fopen(argv[3],"wb");NEED(input&&output&&profile);uint32_t head[2];get(input,head,8);NEED(head[0]==UINT32_C(0x314c4650)&&head[1]==42);
 for(int k=0;k<42;k++){double q[10]={0},v[10]={0};get(input,q,72);get(input,v,72);coax_t_PflOutput fresh;observed(q,v,&fresh);int64_t status=COAX_PfkStatus_ok;put(output,&status,8);put(output,&fresh,sizeof fresh);}NEED(fgetc(input)==EOF);NEED(!fclose(input)&&!fclose(output));
 coax_t_PmProfile p=pm_profile();for(int i=0;i<16;i++){double row[11];row[0]=p.bodies.e[i].mass;for(int j=0;j<3;j++){row[1+j]=p.bodies.e[i].com.e[j];row[8+j]=p.bodies.e[i].principal.e[j];}for(int j=0;j<4;j++)row[4+j]=p.bodies.e[i].inertial.e[j];int64_t mask=p.bodies.e[i].ancestors;put(profile,row,sizeof row);put(profile,&mask,8);}NEED(!fclose(profile));faults();overlap();printf("{\"passed\":true,\"assertions\":%zu,\"cases\":42,\"record_bytes\":224,\"position_rate_boundaries\":%zu,\"alias_cases\":%zu,\"nonfinite_poisons\":90,\"fpcr\":0}\n",checks,boundaries,aliases);return 0;
}
