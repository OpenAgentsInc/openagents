/* NEW actuator/joint-passive ABI/effect harness. Numerical authority is
   the separate retained-SDK/Gaussian oracle, never arithmetic in this file. */
#include "panda-actuation.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
static size_t checks,position_rate_boundaries,control_boundaries,aliases;
#define NEED(v) do{++checks;if(!(v)){fprintf(stderr,"Panda actuation native failure line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PaOutput)==160&&_Alignof(coax_t_PaOutput)==8&&offsetof(coax_t_PaOutput,joint_passive)==72&&offsetof(coax_t_PaOutput,control_clipped)==144&&offsetof(coax_t_PaOutput,force_clipped)==152,"18force words plus two clipping counts");
static coax_t_PfkStatus(*const typed)(coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_t_PaOutput)=pa_forward;
static double real(uint64_t bits){double value;memcpy(&value,&bits,8);return value;}
static void get(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static coax_t_PaOutput poison(int base){coax_t_PaOutput out;for(int i=0;i<7;i++){out.actuator.hinge.e[i]=base+i;out.joint_passive.hinge.e[i]=base+9+i;}for(int i=0;i<2;i++){out.actuator.slide.e[i]=base+7+i;out.joint_passive.slide.e[i]=base+16+i;}out.control_clipped=base+18;out.force_clipped=base+19;return out;}
static void finite_output(const coax_t_PaOutput*p){for(int i=0;i<7;i++){NEED(isfinite(p->actuator.hinge.e[i]));NEED(isfinite(p->joint_passive.hinge.e[i]));}for(int i=0;i<2;i++){NEED(isfinite(p->actuator.slide.e[i]));NEED(isfinite(p->joint_passive.slide.e[i]));}NEED(p->control_clipped>=0&&p->control_clipped<=9&&p->force_clipped>=0&&p->force_clipped<=2);}
static void refused(double*q,double*v,double*f,const int64_t n[7],coax_t_PfkStatus expected){
 double oq[10],ov[10],of[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);memcpy(of,f,sizeof of);coax_t_PaOutput out[2]={poison(1001),poison(2001)},before[2];memcpy(before,out,sizeof out);
 NEED(typed((coax_v_f64){q,n[0]},(coax_v_f64){q+7,n[1]},(coax_v_f64){v,n[2]},(coax_v_f64){v+7,n[3]},(coax_v_f64){f,n[4]},(coax_v_f64){f+7,n[5]},(coax_v_t_PaOutput){out,n[6]})==expected);
 NEED(!memcmp(before,out,sizeof out));NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov)&&!memcmp(f,of,sizeof of));
}
static void refused_raw(double*q,double*v,double*f,const int64_t n[4],coax_t_PfkStatus expected){
 double oq[10],ov[10],of[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);memcpy(of,f,sizeof of);coax_t_PaOutput out[2]={poison(1001),poison(2001)},before[2];memcpy(before,out,sizeof out);
 NEED(pa_forward_raw((coax_v_f64){q,n[0]},(coax_v_f64){v,n[1]},(coax_v_f64){f,n[2]},(coax_v_t_PaOutput){out,n[3]})==expected);
 NEED(!memcmp(before,out,sizeof out));NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov)&&!memcmp(f,of,sizeof of));
}
static void refused_both(double*q,double*v,double*f,coax_t_PfkStatus expected){const int64_t t[]={7,2,7,2,7,2,1},r[]={9,9,9,1};refused(q,v,f,t,expected);refused_raw(q,v,f,r,expected);}
static coax_t_PfkStatus observed(double*q,double*v,double*f,coax_t_PaOutput*result){
 struct Box{unsigned char prefix[32];coax_t_PaOutput out[2];unsigned char tail[32];}box,before;memset(&box,0xa5,sizeof box);box.out[0]=poison(1001);box.out[1]=poison(2001);memcpy(&before,&box,sizeof box);
 double oq[10],ov[10],of[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);memcpy(of,f,sizeof of);coax_t_PaOutput raw=poison(1001);
 coax_t_PfkStatus status=typed((coax_v_f64){q,7},(coax_v_f64){q+7,2},(coax_v_f64){v,7},(coax_v_f64){v+7,2},(coax_v_f64){f,7},(coax_v_f64){f+7,2},(coax_v_t_PaOutput){box.out,1});
 coax_t_PfkStatus raw_status=pa_forward_raw((coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_f64){f,9},(coax_v_t_PaOutput){&raw,1});
 NEED(status==COAX_PfkStatus_ok);NEED(status==raw_status);NEED(!memcmp(&box.out[0],&raw,sizeof raw));
 NEED(!memcmp(&box.out[1],&before.out[1],sizeof raw));NEED(!memcmp(box.prefix,before.prefix,sizeof box.prefix)&&!memcmp(box.tail,before.tail,sizeof box.tail));
 NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov)&&!memcmp(f,of,sizeof of));
 if(status!=COAX_PfkStatus_ok)NEED(!memcmp(&box,&before,sizeof box));finite_output(&box.out[0]);memcpy(result,&box.out[0],sizeof *result);return status;
}
static void overlap(void){
 /* Output spans all three deliberately overlapping input views. A named union
    member provides the output object; published bytes replace input storage. */
 struct At{double prefix[6];coax_t_PaOutput out;double tail[14];};union Store{double words[40];struct At at;}storage;
 _Static_assert(offsetof(struct At,out)==6*8&&sizeof(struct At)==40*8,"overlap offsets");
 double q[10]={0},v[10]={0},f[10]={0};for(int i=0;i<9;i++)q[i]=v[i]=f[i]=.125;coax_t_PaOutput want;NEED(observed(q,v,f,&want)==COAX_PfkStatus_ok);
 for(int mode=0;mode<2;mode++){
  for(int i=0;i<40;i++)storage.words[i]=5001+i;for(int i=0;i<15;i++)storage.words[i]=.125;unsigned char old[sizeof storage];memcpy(old,&storage,sizeof old);
  coax_t_PfkStatus status=mode==0?typed((coax_v_f64){storage.words,7},(coax_v_f64){storage.words+7,2},(coax_v_f64){storage.words+3,7},(coax_v_f64){storage.words+10,2},(coax_v_f64){storage.words+6,7},(coax_v_f64){storage.words+13,2},(coax_v_t_PaOutput){&storage.at.out,1}):pa_forward_raw((coax_v_f64){storage.words,9},(coax_v_f64){storage.words+3,9},(coax_v_f64){storage.words+6,9},(coax_v_t_PaOutput){&storage.at.out,1});
  NEED(status==COAX_PfkStatus_ok);NEED(!memcmp(&storage.at.out,&want,sizeof want));NEED(!memcmp(&storage,old,6*8));NEED(!memcmp((unsigned char*)&storage+26*8,old+26*8,sizeof storage-26*8));++aliases;
  for(int i=0;i<40;i++)storage.words[i]=5001+i;for(int i=0;i<15;i++)storage.words[i]=.125;storage.words[0]=real(UINT64_C(0x7ff8000000000001));memcpy(old,&storage,sizeof old);
  status=mode==0?typed((coax_v_f64){storage.words,7},(coax_v_f64){storage.words+7,2},(coax_v_f64){storage.words+3,7},(coax_v_f64){storage.words+10,2},(coax_v_f64){storage.words+6,7},(coax_v_f64){storage.words+13,2},(coax_v_t_PaOutput){&storage.at.out,1}):pa_forward_raw((coax_v_f64){storage.words,9},(coax_v_f64){storage.words+3,9},(coax_v_f64){storage.words+6,9},(coax_v_t_PaOutput){&storage.at.out,1});
  NEED(status==COAX_PfkStatus_number);NEED(!memcmp(&storage,old,sizeof storage));++aliases;
 }
}
static void faults(void){
 double q[10]={0},v[10]={0},f[10]={0};int64_t t[]={7,2,7,2,7,2,1},r[]={9,9,9,1};q[0]=real(UINT64_C(0x7ff8000000000001));v[0]=21;f[0]=10001;
 for(int field=0;field<7;field++){int64_t good=t[field];for(int64_t n=-1;n<=good+1;n++)if(n!=good){t[field]=n;refused(q,v,f,t,COAX_PfkStatus_shape);}t[field]=INT64_MIN;refused(q,v,f,t,COAX_PfkStatus_shape);t[field]=INT64_MAX;refused(q,v,f,t,COAX_PfkStatus_shape);t[field]=good;}
 for(int field=0;field<4;field++){int64_t good=r[field];for(int64_t n=-1;n<=good+1;n++)if(n!=good){r[field]=n;refused_raw(q,v,f,r,COAX_PfkStatus_shape);}r[field]=INT64_MIN;refused_raw(q,v,f,r,COAX_PfkStatus_shape);r[field]=INT64_MAX;refused_raw(q,v,f,r,COAX_PfkStatus_shape);r[field]=good;}
 NEED(typed((coax_v_f64){NULL,6},(coax_v_f64){NULL,2},(coax_v_f64){NULL,7},(coax_v_f64){NULL,2},(coax_v_f64){NULL,7},(coax_v_f64){NULL,2},(coax_v_t_PaOutput){NULL,0})==COAX_PfkStatus_shape);
 NEED(pa_forward_raw((coax_v_f64){NULL,8},(coax_v_f64){NULL,9},(coax_v_f64){NULL,9},(coax_v_t_PaOutput){NULL,1})==COAX_PfkStatus_shape);
 memset(q,0,sizeof q);memset(v,0,sizeof v);memset(f,0,sizeof f);const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int role=0;role<3;role++){double*values=role==0?q:role==1?v:f;for(int axis=0;axis<9;axis++){
  double limit=role==0?(axis<7?100:1):role==1?(axis<7?20:2):(axis<7?10000:1);
  for(size_t k=0;k<5;k++){values[axis]=real(bad[k]);refused_both(q,v,f,COAX_PfkStatus_number);}
  for(int sign=-1;sign<=1;sign+=2){values[axis]=sign*nextafter(limit,INFINITY);refused_both(q,v,f,COAX_PfkStatus_range);values[axis]=sign*limit;coax_t_PaOutput result;coax_t_PfkStatus status=observed(q,v,f,&result);if(role<2){NEED(status==COAX_PfkStatus_ok);++position_rate_boundaries;}else {NEED(status==COAX_PfkStatus_ok);++control_boundaries;}}
  values[axis]=0;
 }}
 /* Every independent precedence test starts from clean inputs. */
 for(int role=0;role<3;role++){double*values=role==0?q:role==1?v:f;values[0]=role==0?101:role==1?21:10001;values[1]=real(bad[3]);refused_both(q,v,f,COAX_PfkStatus_range);values[0]=real(bad[3]);values[1]=role==0?101:role==1?21:10001;refused_both(q,v,f,COAX_PfkStatus_number);memset(q,0,sizeof q);memset(v,0,sizeof v);memset(f,0,sizeof f);}
 q[8]=2;v[0]=real(bad[3]);refused_both(q,v,f,COAX_PfkStatus_range);memset(q,0,sizeof q);memset(v,0,sizeof v);
 q[6]=101;q[7]=real(bad[3]);refused_both(q,v,f,COAX_PfkStatus_range);q[6]=real(bad[3]);q[7]=2;refused_both(q,v,f,COAX_PfkStatus_number);memset(q,0,sizeof q);
 v[6]=21;v[7]=real(bad[3]);refused_both(q,v,f,COAX_PfkStatus_range);v[6]=real(bad[3]);v[7]=3;refused_both(q,v,f,COAX_PfkStatus_number);memset(v,0,sizeof v);
 v[8]=3;f[0]=real(bad[3]);refused_both(q,v,f,COAX_PfkStatus_range);memset(v,0,sizeof v);memset(f,0,sizeof f);
 f[6]=10001;f[7]=real(bad[3]);refused_both(q,v,f,COAX_PfkStatus_range);f[6]=real(bad[3]);f[7]=2;refused_both(q,v,f,COAX_PfkStatus_number);
}
int main(int argc,char**argv){
 if(argc!=3)return 2;uint16_t endian=1;NEED(*(unsigned char*)&endian==1);FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx");NEED(input&&output);uint32_t magic,n;get(input,&magic,4);get(input,&n,4);NEED(magic==UINT32_C(0x31464150)&&n==18);
 for(uint32_t row=0;row<n;row++){double values[27],q[10]={0},v[10]={0},f[10]={0};get(input,values,sizeof values);for(int i=0;i<27;i++)NEED(isfinite(values[i]));memcpy(q,values,9*8);memcpy(v,values+9,9*8);memcpy(f,values+18,9*8);coax_t_PaOutput result;NEED(observed(q,v,f,&result)==COAX_PfkStatus_ok);put(output,&result,sizeof result);}
 NEED(fgetc(input)==EOF);overlap();faults();NEED(!fclose(input)&&!fclose(output));NEED(position_rate_boundaries==36&&control_boundaries==18&&aliases==4);
 printf("{\"schema\":\"psionic.panda-actuation.native.v1\",\"passed\":true,\"cases\":18,\"record_bytes\":160,\"position_rate_boundaries\":%zu,\"control_boundaries\":%zu,\"alias_cases\":%zu,\"assertions\":%zu}\n",position_rate_boundaries,control_boundaries,aliases,checks);return 0;
}
