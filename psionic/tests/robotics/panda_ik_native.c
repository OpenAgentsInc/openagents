/* Test-only effects/ABI witness. Coax owns FK, DLS and convergence policy. */
#include "panda-ik.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#ifndef PIK_TARGET_VIEW
#error PIK_TARGET_VIEW must name the actual target view in the emitted header
#endif
typedef __typeof__(((coax_t_PikOutput*)0)->position) PikPoint;
static size_t checks,events,trace_updates,aliases,limit_boundaries,target_boundaries;
#define NEED(v) do{++checks;if(!(v)){fprintf(stderr,"Panda IK native failure line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PikOutput)==104 && _Alignof(coax_t_PikOutput)==8 && offsetof(coax_t_PikOutput,position)==56 && offsetof(coax_t_PikOutput,error_metres)==80 && offsetof(coax_t_PikOutput,updates)==88 && offsetof(coax_t_PikOutput,projected_coordinates)==96,"eleven F64/twoI64 output");
_Static_assert(sizeof(PikPoint)==24 && offsetof(PikPoint,x)==0 && offsetof(PikPoint,y)==8 && offsetof(PikPoint,z)==16,"source world point layout");
static coax_t_PikStatus(*const solve_typed)(coax_v_f64,PIK_TARGET_VIEW,int64_t,coax_v_t_PikOutput)=pik_solve;
static coax_t_PikStatus(*const step_typed)(coax_v_f64,PIK_TARGET_VIEW,coax_v_t_PikOutput)=pik_step;
static double home[7],lower[7],upper[7],alias_target[3];
static void get(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static double real(uint64_t u){double v;memcpy(&v,&u,8);return v;}
static coax_t_PikOutput poison(void){coax_t_PikOutput p;for(int i=0;i<7;i++)p.angles.e[i]=1001+i;p.position.x=1008;p.position.y=1009;p.position.z=1010;p.error_metres=1011;p.updates=1012;p.projected_coordinates=1013;return p;}
static void finite_output(const coax_t_PikOutput*p){for(int i=0;i<7;i++){NEED(isfinite(p->angles.e[i]));NEED(p->angles.e[i]>=lower[i]&&p->angles.e[i]<=upper[i]);}NEED(isfinite(p->position.x)&&isfinite(p->position.y)&&isfinite(p->position.z)&&isfinite(p->error_metres)&&p->error_metres>=0);NEED(p->updates>=0&&p->updates<=96&&p->projected_coordinates>=0&&p->projected_coordinates<=7*96);}
static coax_t_PikStatus direct(int solve,int raw,double*q,double*t,int64_t budget,const int64_t n[3],coax_t_PikOutput*out){
 PikPoint point={t[0],t[1],t[2]},before_point;memcpy(&before_point,&point,sizeof point);
 if(raw)return solve?pik_solve_raw((coax_v_f64){q,n[0]},(coax_v_f64){t,n[1]},budget,(coax_v_t_PikOutput){out,n[2]}):pik_step_raw((coax_v_f64){q,n[0]},(coax_v_f64){t,n[1]},(coax_v_t_PikOutput){out,n[2]});
 coax_t_PikStatus status=solve?solve_typed((coax_v_f64){q,n[0]},(PIK_TARGET_VIEW){&point,n[1]},budget,(coax_v_t_PikOutput){out,n[2]}):step_typed((coax_v_f64){q,n[0]},(PIK_TARGET_VIEW){&point,n[1]},(coax_v_t_PikOutput){out,n[2]});NEED(!memcmp(&point,&before_point,sizeof point));return status;
}
static coax_t_PikStatus observed(int solve,double*q,double*t,int64_t budget,coax_t_PikOutput*result){
 struct Box{unsigned char prefix[32];coax_t_PikOutput out[2];unsigned char tail[32];}box,before;memset(&box,0xa5,sizeof box);box.out[0]=box.out[1]=poison();memcpy(&before,&box,sizeof box);double oldq[7],oldt[3];memcpy(oldq,q,sizeof oldq);memcpy(oldt,t,sizeof oldt);coax_t_PikOutput raw=poison();int64_t nt[]={7,1,1},nr[]={7,3,1};
 coax_t_PikStatus status=direct(solve,0,q,t,budget,nt,box.out),other=direct(solve,1,q,t,budget,nr,&raw);NEED(status==other);NEED(status==COAX_PikStatus_ok||status==COAX_PikStatus_not_converged);NEED(!memcmp(&raw,&box.out[0],sizeof raw));NEED(!memcmp(&box.out[1],&before.out[1],sizeof raw));NEED(!memcmp(box.prefix,before.prefix,sizeof box.prefix)&&!memcmp(box.tail,before.tail,sizeof box.tail));NEED(!memcmp(q,oldq,sizeof oldq)&&!memcmp(t,oldt,sizeof oldt));if(status==COAX_PikStatus_ok){finite_output(&box.out[0]);if(!solve){NEED(box.out[0].updates==1);for(int i=0;i<7;i++)NEED(fabs(box.out[0].angles.e[i]-oldq[i])<=.1+2e-15);}}else NEED(!memcmp(&box,&before,sizeof box));memcpy(result,&box.out[0],sizeof *result);return status;
}
static void emit(FILE*f,int64_t c,int64_t kind,int64_t stage,int64_t status,const coax_t_PikOutput*p){int64_t h[]={c,kind,stage,status};put(f,h,sizeof h);put(f,p,sizeof *p);++events;}
static void refused(int solve,int raw,double*q,double*t,int64_t budget,const int64_t n[3],coax_t_PikStatus expected){double oldq[7],oldt[3];memcpy(oldq,q,sizeof oldq);memcpy(oldt,t,sizeof oldt);coax_t_PikOutput out[2]={poison(),poison()},before[2];memcpy(before,out,sizeof out);NEED(direct(solve,raw,q,t,budget,n,out)==expected);NEED(!memcmp(before,out,sizeof out));NEED(!memcmp(q,oldq,sizeof oldq)&&!memcmp(t,oldt,sizeof oldt));}
static void refused_all(double*q,double*t,int64_t budget,coax_t_PikStatus expected){for(int solve=0;solve<2;solve++)for(int raw=0;raw<2;raw++){int64_t n[]={7,raw?3:1,1};refused(solve,raw,q,t,budget,n,expected);}}
static void faults(void){
 double q[7],t[3]={0};memcpy(q,home,sizeof q);q[0]=real(UINT64_C(0x7ff8000000000001));
 for(int solve=0;solve<2;solve++)for(int raw=0;raw<2;raw++){int64_t n[]={7,raw?3:1,1};for(int field=0;field<3;field++){int64_t good=n[field];for(int64_t v=-1;v<=good+1;v++)if(v!=good){n[field]=v;refused(solve,raw,q,t,-1,n,COAX_PikStatus_shape);}n[field]=INT64_MIN;refused(solve,raw,q,t,-1,n,COAX_PikStatus_shape);n[field]=INT64_MAX;refused(solve,raw,q,t,-1,n,COAX_PikStatus_shape);n[field]=good;}}
 for(int raw=0;raw<2;raw++){int64_t n[]={7,raw?3:1,1};for(int k=0;k<4;k++){int64_t budget[]={-1,97,INT64_MIN,INT64_MAX};refused(1,raw,q,t,budget[k],n,COAX_PikStatus_range);}}
 memcpy(q,home,sizeof q);const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int axis=0;axis<10;axis++)for(int b=0;b<5;b++){memcpy(q,home,sizeof q);memset(t,0,sizeof t);if(axis<7)q[axis]=real(bad[b]);else t[axis-7]=real(bad[b]);refused_all(q,t,1,COAX_PikStatus_number);}
 for(int axis=0;axis<7;axis++)for(int side=0;side<2;side++){memcpy(q,home,sizeof q);memset(t,0,sizeof t);q[axis]=side?upper[axis]:lower[axis];coax_t_PikOutput p;NEED(observed(0,q,t,1,&p)==COAX_PikStatus_ok);++limit_boundaries;q[axis]=nextafter(q[axis],side?INFINITY:-INFINITY);refused_all(q,t,1,COAX_PikStatus_limits);}
 for(int axis=0;axis<3;axis++)for(int side=0;side<2;side++){memcpy(q,home,sizeof q);memset(t,0,sizeof t);t[axis]=side?10:-10;coax_t_PikOutput p;NEED(observed(0,q,t,1,&p)==COAX_PikStatus_ok);++target_boundaries;t[axis]=nextafter(t[axis],side?INFINITY:-INFINITY);refused_all(q,t,1,COAX_PikStatus_range);}
 memcpy(q,home,sizeof q);memset(t,0,sizeof t);q[0]=upper[0]+1;t[0]=real(bad[3]);refused_all(q,t,1,COAX_PikStatus_limits);q[0]=real(bad[3]);t[0]=11;refused_all(q,t,1,COAX_PikStatus_number);memcpy(q,home,sizeof q);t[0]=11;t[2]=real(bad[3]);refused_all(q,t,1,COAX_PikStatus_range);t[0]=real(bad[3]);t[2]=11;refused_all(q,t,1,COAX_PikStatus_number);
}
static void overlaps(void){
 struct At{double prefix[3];coax_t_PikOutput out;double tail[8];};struct Inputs{double q[7];PikPoint target;double tail[14];};union Store{double words[24];struct At at;struct Inputs inputs;}s;
 _Static_assert(sizeof s==192&&offsetof(struct At,out)==24&&offsetof(struct Inputs,target)==56,"output overlaps both input objects");
 for(int solve=0;solve<2;solve++){double q[7],t[3];memcpy(q,home,sizeof q);memcpy(t,alias_target,sizeof t);coax_t_PikOutput expected;NEED(observed(solve,q,t,96,&expected)==COAX_PikStatus_ok);
  for(int raw=0;raw<2;raw++)for(int invalid=0;invalid<2;invalid++){for(int i=0;i<24;i++)s.words[i]=5001+i;memcpy(s.inputs.q,home,sizeof home);s.inputs.target=(PikPoint){t[0],t[1],t[2]};if(invalid)s.inputs.q[0]=real(UINT64_C(0x7ff8000000000001));unsigned char old[sizeof s];memcpy(old,&s,sizeof s);coax_t_PikStatus status;
   if(raw)status=solve?pik_solve_raw((coax_v_f64){s.words,7},(coax_v_f64){s.words+7,3},96,(coax_v_t_PikOutput){&s.at.out,1}):pik_step_raw((coax_v_f64){s.words,7},(coax_v_f64){s.words+7,3},(coax_v_t_PikOutput){&s.at.out,1});else status=solve?solve_typed((coax_v_f64){s.inputs.q,7},(PIK_TARGET_VIEW){&s.inputs.target,1},96,(coax_v_t_PikOutput){&s.at.out,1}):step_typed((coax_v_f64){s.inputs.q,7},(PIK_TARGET_VIEW){&s.inputs.target,1},(coax_v_t_PikOutput){&s.at.out,1});
   NEED(status==(invalid?COAX_PikStatus_number:COAX_PikStatus_ok));if(invalid)NEED(!memcmp(old,&s,sizeof s));else{NEED(!memcmp(&s.at.out,&expected,sizeof expected));NEED(!memcmp(old,&s,24));NEED(!memcmp(old+128,(unsigned char*)&s+128,sizeof s-128));}++aliases;
  }
 }
}
struct Case{double q[7],target[3];int64_t budget;};
int main(int argc,char**argv){
 NEED(argc==3);uint64_t fpcr;__asm__ volatile("mrs %0, fpcr":"=r"(fpcr));NEED(fpcr==0);FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wb");NEED(input&&output);uint32_t h[2];get(input,h,8);NEED(h[0]==UINT32_C(0x314b4950)&&h[1]==50);_Static_assert(sizeof(struct Case)==88,"ten doubles and budget");struct Case cases[50];get(input,cases,sizeof cases);get(input,lower,sizeof lower);get(input,upper,sizeof upper);get(input,home,sizeof home);get(input,alias_target,sizeof alias_target);NEED(fgetc(input)==EOF);NEED(!fclose(input));
 for(int c=0;c<50;c++){struct Case*item=&cases[c];coax_t_PikOutput result;coax_t_PikStatus status=observed(1,item->q,item->target,item->budget,&result);emit(output,c,0,0,status,&result);if((c>=1&&c<=34)||c==49){status=observed(0,item->q,item->target,1,&result);NEED(status==COAX_PikStatus_ok);emit(output,c,1,1,status,&result);}
  double current[7];memcpy(current,item->q,sizeof current);
  for(int64_t stage=0;;stage++){status=observed(1,current,item->target,0,&result);if(status==COAX_PikStatus_ok||stage==item->budget){emit(output,c,3,stage,status,&result);break;}NEED(stage<96&&status==COAX_PikStatus_not_converged);status=observed(0,current,item->target,1,&result);NEED(status==COAX_PikStatus_ok);emit(output,c,2,stage+1,status,&result);memcpy(current,result.angles.e,sizeof current);++trace_updates;}
 }
 NEED(!fclose(output));faults();overlaps();printf("{\"passed\":true,\"assertions\":%zu,\"cases\":50,\"record_bytes\":136,\"events\":%zu,\"trajectory_updates\":%zu,\"limit_boundaries\":%zu,\"target_boundaries\":%zu,\"nonfinite_poisons\":50,\"alias_cases\":%zu,\"fpcr\":0}\n",checks,events,trace_updates,limit_boundaries,target_boundaries,aliases);return 0;
}
