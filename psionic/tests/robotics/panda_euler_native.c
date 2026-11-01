/* NEW owned Euler ABI/refusal consumer. Expected q/v/effective accelerations
   come from independent cached algebra and original SDK steps, never C physics. */
#include "panda-euler.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
static size_t assertions,aliases,coordinate_rate_endpoints,control_endpoints,external_endpoints,boundary_ranges,nonfinite_lane_cases,late_position_cases,late_rate_cases;
#define NEED(x) do{assertions++;if(!(x)){fprintf(stderr,"Euler native line%d: %s\n",__LINE__,#x);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PeuOutput)==232&&_Alignof(coax_t_PeuOutput)==8&&offsetof(coax_t_PeuOutput,slides)==56&&offsetof(coax_t_PeuOutput,hinge_rates)==72&&offsetof(coax_t_PeuOutput,slide_rates)==128&&offsetof(coax_t_PeuOutput,effective)==144&&offsetof(coax_t_PeuOutput,control_clipped)==216&&offsetof(coax_t_PeuOutput,force_clipped)==224,"q9/v9/aeff9 and two clipping counts");
typedef struct {double q[10],v[10],control[10],external[10];} Inputs;
static coax_t_PmsStatus(*const typed)(coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_t_PeuOutput)=peu_step;
static double real(uint64_t bits){double value;memcpy(&value,&bits,8);return value;}
static coax_t_PeuOutput poison(int base){coax_t_PeuOutput x;for(int i=0;i<7;i++){x.angles.e[i]=base+i;x.hinge_rates.e[i]=base+9+i;x.effective.hinge.e[i]=base+18+i;}for(int i=0;i<2;i++){x.slides.e[i]=base+7+i;x.slide_rates.e[i]=base+16+i;x.effective.slide.e[i]=base+25+i;}x.control_clipped=base+27;x.force_clipped=base+28;return x;}
static void finite_output(const coax_t_PeuOutput*x,int ok){for(int i=0;i<7;i++){NEED(isfinite(x->angles.e[i]));NEED(isfinite(x->hinge_rates.e[i]));NEED(isfinite(x->effective.hinge.e[i]));}for(int i=0;i<2;i++){NEED(isfinite(x->slides.e[i]));NEED(isfinite(x->slide_rates.e[i]));NEED(isfinite(x->effective.slide.e[i]));}if(ok)NEED(x->control_clipped>=0&&x->control_clipped<=9&&x->force_clipped>=0&&x->force_clipped<=2);}
static coax_t_PmsStatus call(Inputs*x,const int64_t n[9],coax_t_PeuOutput*out){return typed((coax_v_f64){x->q,n[0]},(coax_v_f64){x->q+7,n[1]},(coax_v_f64){x->v,n[2]},(coax_v_f64){x->v+7,n[3]},(coax_v_f64){x->control,n[4]},(coax_v_f64){x->control+7,n[5]},(coax_v_f64){x->external,n[6]},(coax_v_f64){x->external+7,n[7]},(coax_v_t_PeuOutput){out,n[8]});}
static void refused(Inputs*x,int raw,const int64_t*n,coax_t_PmsStatus expected){Inputs old;memcpy(&old,x,sizeof old);coax_t_PeuOutput out[2]={poison(1001),poison(2001)},before[2];memcpy(before,out,sizeof out);coax_t_PmsStatus status=raw?peu_step_raw((coax_v_f64){x->q,n[0]},(coax_v_f64){x->v,n[1]},(coax_v_f64){x->control,n[2]},(coax_v_f64){x->external,n[3]},(coax_v_t_PeuOutput){out,n[4]}):call(x,n,out);NEED(status==expected);NEED(!memcmp(out,before,sizeof out)&&!memcmp(x,&old,sizeof old));}
static void refused_both(Inputs*x,coax_t_PmsStatus status){const int64_t t[]={7,2,7,2,7,2,7,2,1},r[]={9,9,9,9,1};refused(x,0,t,status);refused(x,1,r,status);}
static coax_t_PmsStatus observed(Inputs*x,coax_t_PeuOutput*result){
 struct Box{unsigned char prefix[32];coax_t_PeuOutput out[2];unsigned char tail[32];}box,before;memset(&box,0xa5,sizeof box);box.out[0]=poison(1001);box.out[1]=poison(2001);memcpy(&before,&box,sizeof box);Inputs old;memcpy(&old,x,sizeof old);const int64_t n[]={7,2,7,2,7,2,7,2,1};coax_t_PeuOutput raw=poison(1001);
 coax_t_PmsStatus status=call(x,n,box.out),other=peu_step_raw((coax_v_f64){x->q,9},(coax_v_f64){x->v,9},(coax_v_f64){x->control,9},(coax_v_f64){x->external,9},(coax_v_t_PeuOutput){&raw,1});
 NEED(status==COAX_PmsStatus_ok||status==COAX_PmsStatus_range);NEED(status==other&&!memcmp(&box.out[0],&raw,sizeof raw));NEED(!memcmp(&box.out[1],&before.out[1],sizeof raw));NEED(!memcmp(box.prefix,before.prefix,sizeof box.prefix)&&!memcmp(box.tail,before.tail,sizeof box.tail));NEED(!memcmp(x,&old,sizeof old));if(status!=COAX_PmsStatus_ok)NEED(!memcmp(&box,&before,sizeof box));finite_output(&box.out[0],status==COAX_PmsStatus_ok);memcpy(result,&box.out[0],sizeof *result);return status;
}
static void overlap(void){
 struct At{double prefix[6];coax_t_PeuOutput out;double tail[14];};union Store{double words[49];struct At at;}storage;_Static_assert(sizeof(struct At)==392&&offsetof(struct At,out)==48,"all four input views overlap output");Inputs x={0};for(int i=0;i<9;i++)x.q[i]=x.v[i]=x.control[i]=x.external[i]=.125;coax_t_PeuOutput wanted;NEED(observed(&x,&wanted)==COAX_PmsStatus_ok);
 for(int raw=0;raw<2;raw++)for(int fault=0;fault<2;fault++){
  for(int i=0;i<49;i++)storage.words[i]=5001+i;for(int i=0;i<18;i++)storage.words[i]=.125;if(fault)storage.words[17]=real(UINT64_C(0x7ff8000000000001));unsigned char old[sizeof storage];memcpy(old,&storage,sizeof old);
  coax_t_PmsStatus status=raw?peu_step_raw((coax_v_f64){storage.words,9},(coax_v_f64){storage.words+3,9},(coax_v_f64){storage.words+6,9},(coax_v_f64){storage.words+9,9},(coax_v_t_PeuOutput){&storage.at.out,1}):typed((coax_v_f64){storage.words,7},(coax_v_f64){storage.words+7,2},(coax_v_f64){storage.words+3,7},(coax_v_f64){storage.words+10,2},(coax_v_f64){storage.words+6,7},(coax_v_f64){storage.words+13,2},(coax_v_f64){storage.words+9,7},(coax_v_f64){storage.words+16,2},(coax_v_t_PeuOutput){&storage.at.out,1});
  NEED(status==(fault?COAX_PmsStatus_number:COAX_PmsStatus_ok));if(fault)NEED(!memcmp(&storage,old,sizeof storage));else {NEED(!memcmp(&storage.at.out,&wanted,sizeof wanted));NEED(!memcmp(&storage,old,48)&&!memcmp((unsigned char*)&storage+280,old+280,sizeof storage-280));}aliases++;
 }
}
static double*role(Inputs*x,int which){return which==0?x->q:which==1?x->v:which==2?x->control:x->external;}
static void faults(void){
 Inputs x={0};int64_t t[]={7,2,7,2,7,2,7,2,1},r[]={9,9,9,9,1};x.q[0]=real(UINT64_C(0x7ff8000000000001));x.external[0]=10001;
 for(int raw=0;raw<2;raw++){int fields=raw?5:9;int64_t*n=raw?r:t;for(int field=0;field<fields;field++){int64_t good=n[field];for(int64_t z=-1;z<=good+1;z++)if(z!=good){n[field]=z;refused(&x,raw,n,COAX_PmsStatus_shape);}n[field]=INT64_MIN;refused(&x,raw,n,COAX_PmsStatus_shape);n[field]=INT64_MAX;refused(&x,raw,n,COAX_PmsStatus_shape);n[field]=good;}}
 NEED(typed((coax_v_f64){NULL,6},(coax_v_f64){NULL,2},(coax_v_f64){NULL,7},(coax_v_f64){NULL,2},(coax_v_f64){NULL,7},(coax_v_f64){NULL,2},(coax_v_f64){NULL,7},(coax_v_f64){NULL,2},(coax_v_t_PeuOutput){NULL,0})==COAX_PmsStatus_shape);NEED(peu_step_raw((coax_v_f64){NULL,8},(coax_v_f64){NULL,9},(coax_v_f64){NULL,9},(coax_v_f64){NULL,9},(coax_v_t_PeuOutput){NULL,1})==COAX_PmsStatus_shape);
 memset(&x,0,sizeof x);const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int which=0;which<4;which++)for(int axis=0;axis<9;axis++){
  double*values=role(&x,which);double bound=which==0?(axis<7?100:1):which==1?(axis<7?20:2):which==2?(axis<7?10000:1):10000;
  for(int b=0;b<5;b++){values[axis]=real(bad[b]);refused_both(&x,COAX_PmsStatus_number);nonfinite_lane_cases++;}
  for(int sign=-1;sign<=1;sign+=2){values[axis]=sign*nextafter(bound,INFINITY);refused_both(&x,COAX_PmsStatus_range);values[axis]=sign*bound;coax_t_PeuOutput result;coax_t_PmsStatus status=observed(&x,&result);if(status==COAX_PmsStatus_range)boundary_ranges++;if(which<2)coordinate_rate_endpoints++;else if(which==2)control_endpoints++;else external_endpoints++;}
  values[axis]=0;
 }
 /* External admission precedes q/rates/control; within each role index order
    decides whether an earlier range or a later NaN is observed first. */
 for(int which=0;which<4;which++){double*values=role(&x,which);values[0]=which==0?101:which==1?21:10001;values[1]=real(bad[3]);refused_both(&x,COAX_PmsStatus_range);values[0]=real(bad[3]);values[1]=which==0?101:which==1?21:10001;refused_both(&x,COAX_PmsStatus_number);memset(&x,0,sizeof x);}
 x.q[0]=real(bad[3]);x.external[0]=10001;refused_both(&x,COAX_PmsStatus_range);x.q[0]=101;x.external[0]=real(bad[3]);refused_both(&x,COAX_PmsStatus_number);memset(&x,0,sizeof x);
 x.q[8]=2;x.v[0]=real(bad[3]);refused_both(&x,COAX_PmsStatus_range);memset(&x,0,sizeof x);x.v[8]=3;x.control[0]=real(bad[3]);refused_both(&x,COAX_PmsStatus_range);memset(&x,0,sizeof x);
 x.control[7]=.04;x.external[7]=10000;refused_both(&x,COAX_PmsStatus_range);memset(&x,0,sizeof x);size_t net_refused=0;
 for(int sign=-1;sign<=1;sign+=2){x.external[1]=sign*10000;coax_t_PeuOutput result;coax_t_PmsStatus status=observed(&x,&result);if(status==COAX_PmsStatus_range){refused_both(&x,COAX_PmsStatus_range);net_refused++;}else finite_output(&result,1);}NEED(net_refused==1);
 memset(&x,0,sizeof x);for(int axis=0;axis<9;axis++)for(int sign=-1;sign<=1;sign+=2){x.q[axis]=sign*(axis<7?100:1);x.v[axis]=sign;coax_t_PsfOutput instantaneous;NEED(psf_forward_raw((coax_v_f64){x.q,9},(coax_v_f64){x.v,9},(coax_v_f64){x.control,9},(coax_v_f64){x.external,9},(coax_v_t_PsfOutput){&instantaneous,1})==COAX_PmsStatus_ok);refused_both(&x,COAX_PmsStatus_range);x.q[axis]=0;x.v[axis]=0;late_position_cases++;}

 memset(&x,0,sizeof x);for(int sign=-1;sign<=1;sign+=2){x.v[0]=sign*20;x.external[0]=sign*10000;coax_t_PsfOutput instantaneous;NEED(psf_forward_raw((coax_v_f64){x.q,9},(coax_v_f64){x.v,9},(coax_v_f64){x.control,9},(coax_v_f64){x.external,9},(coax_v_t_PsfOutput){&instantaneous,1})==COAX_PmsStatus_ok);refused_both(&x,COAX_PmsStatus_range);late_rate_cases++;}

}
int main(int argc,char**argv){
 NEED(argc==3);uint16_t little=1;NEED(*(unsigned char*)&little==1);FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx");NEED(input&&output);uint32_t header[2];NEED(fread(header,1,8,input)==8&&header[0]==UINT32_C(0x31554550)&&header[1]==6);
 for(int row=0;row<6;row++){double words[36];NEED(fread(words,1,sizeof words,input)==sizeof words);for(int i=0;i<36;i++)NEED(isfinite(words[i]));Inputs x={0};memcpy(x.q,words,72);memcpy(x.v,words+9,72);memcpy(x.control,words+18,72);memcpy(x.external,words+27,72);coax_t_PeuOutput result;int64_t status=observed(&x,&result);NEED(status==COAX_PmsStatus_ok);NEED(fwrite(&status,1,8,output)==8&&fwrite(&result,1,232,output)==232);}
 NEED(fgetc(input)==EOF);overlap();faults();NEED(coordinate_rate_endpoints==36&&control_endpoints==18&&external_endpoints==18&&aliases==4&&nonfinite_lane_cases==180&&late_position_cases==18&&late_rate_cases==2);NEED(!fclose(input)&&!fclose(output));printf("{\"schema\":\"psionic.panda-euler.native.v1\",\"passed\":true,\"cases\":6,\"record_bytes\":240,\"coordinate_rate_endpoints\":%zu,\"control_endpoints\":%zu,\"external_endpoints\":%zu,\"boundary_downstream_ranges\":%zu,\"alias_cases\":%zu,\"nonfinite_lane_cases\":%zu,\"late_position_cases\":%zu,\"late_rate_cases\":%zu,\"assertions\":%zu}\n",coordinate_rate_endpoints,control_endpoints,external_endpoints,boundary_ranges,aliases,nonfinite_lane_cases,late_position_cases,late_rate_cases,assertions);return 0;
}
