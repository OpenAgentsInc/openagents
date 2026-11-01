/* NEW fixed experiment transport, not a public product protocol. */
#ifndef PSIONIC_PANDA_JOINT_PD_TRACKING_FORMAT_H
#define PSIONIC_PANDA_JOINT_PD_TRACKING_FORMAT_H
#include "panda-joint-pd-tracking.h"
#include <float.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
enum{PNT_CASES=4,PNT_STEPS=750,PNT_VALUES=120,PNT_META=10};
static size_t pnt_checks;
#define PNT_NEED(x) do{pnt_checks++;if(!(x)){fprintf(stderr,"Tracking line%d: %s\n",__LINE__,#x);exit(2);}}while(0)
typedef struct{double values[PNT_VALUES];int64_t meta[PNT_META];}PntRecord;
typedef struct{PntRecord fresh;double shadow_q[9],shadow_v[9];int64_t control_clipped,force_clipped,shadow_control_clipped,shadow_force_clipped;}PntOwnRecord;
_Static_assert(sizeof(double)==8&&DBL_MANT_DIG==53&&FLT_RADIX==2,"binary64");
_Static_assert(sizeof(coax_t_PntInitial)==144&&offsetof(coax_t_PntInitial,q)==0&&offsetof(coax_t_PntInitial,v)==72,"initial ABI");
_Static_assert(sizeof(coax_t_PntStatus)==16&&offsetof(coax_t_PntStatus,stage)==0&&offsetof(coax_t_PntStatus,code)==8,"status ABI");
_Static_assert(sizeof(coax_t_PntCommand)==712&&offsetof(coax_t_PntCommand,reference)==0&&offsetof(coax_t_PntCommand,controller)==168&&offsetof(coax_t_PntCommand,controls)==456&&offsetof(coax_t_PntCommand,external)==528&&offsetof(coax_t_PntCommand,reference_tick)==600&&offsetof(coax_t_PntCommand,reference_steps)==608&&offsetof(coax_t_PntCommand,pulse)==616&&offsetof(coax_t_PntCommand,actuator_control_clipped)==624&&offsetof(coax_t_PntCommand,actuator_force_clipped)==632&&offsetof(coax_t_PntCommand,actuator)==640,"command ABI");
_Static_assert(sizeof(coax_t_PntStep)==944&&offsetof(coax_t_PntStep,command)==0&&offsetof(coax_t_PntStep,plant)==712,"step ABI");
_Static_assert(sizeof(PntRecord)==1040&&sizeof(PntOwnRecord)==1216&&offsetof(PntOwnRecord,shadow_q)==1040&&offsetof(PntOwnRecord,control_clipped)==1184,"trace ABI");
static void pnt_finite(const double*x,int n){for(int i=0;i<n;i++)PNT_NEED(isfinite(x[i]));}
static void pnt_state(const double*q,const double*v){pnt_finite(q,9);pnt_finite(v,9);for(int i=0;i<9;i++)PNT_NEED(fabs(q[i])<=(i<7?100:1)&&fabs(v[i])<=(i<7?20:2));}
static void pnt_header(FILE*f,uint32_t magic,uint32_t bytes){uint32_t h[]={magic,PNT_CASES,PNT_STEPS,bytes};PNT_NEED(fwrite(h,1,16,f)==16);}
static void pnt_read_header(FILE*f,uint32_t magic,uint32_t bytes){uint32_t h[4];PNT_NEED(fread(h,1,16,f)==16&&h[0]==magic&&h[1]==PNT_CASES&&h[2]==PNT_STEPS&&h[3]==bytes);}
static void pnt_input(const char*path,coax_t_PntInitial result[PNT_CASES]){FILE*f=fopen(path,"rb");PNT_NEED(f);pnt_read_header(f,UINT32_C(0x31495450),1);PNT_NEED(fread(result,1,PNT_CASES*144,f)==PNT_CASES*144&&fgetc(f)==EOF&&!fclose(f));for(int i=0;i<PNT_CASES;i++)pnt_state(result[i].q.e,result[i].v.e);}
static void pnt_initial_observed(int id,coax_t_PntInitial*out){struct{unsigned char prefix[32];coax_t_PntInitial value[2];unsigned char suffix[32];}box,old;memset(&box,0xa5,sizeof box);memcpy(&old,&box,sizeof box);coax_t_PntStatus status=pnt_initial(id,(coax_v_t_PntInitial){box.value,1});PNT_NEED(status.stage==0&&status.code==0);pnt_state(box.value[0].q.e,box.value[0].v.e);PNT_NEED(!memcmp(box.prefix,old.prefix,32)&&!memcmp(box.suffix,old.suffix,32)&&!memcmp(&box.value[1],&old.value[1],144));memcpy(out,&box.value[0],144);}
static void pnt_command_fields(const coax_t_PntCommand*c){double pd[35],actuator[9];memcpy(pd,&c->controller,280);memcpy(actuator,&c->actuator,72);pnt_finite(c->reference.angle.e,7);pnt_finite(c->reference.rate.e,7);pnt_finite(c->reference.acceleration.e,7);pnt_finite(pd,35);pnt_finite(c->controls.e,9);pnt_finite(c->external.e,9);pnt_finite(actuator,9);PNT_NEED(c->controller.clipped>=0&&c->controller.clipped<=7&&c->reference_steps>=0&&c->reference_steps<=1000000&&c->reference_tick>=0&&c->reference_tick<=c->reference_steps&&(c->pulse==0||c->pulse==1)&&c->actuator_control_clipped>=0&&c->actuator_control_clipped<=9&&c->actuator_force_clipped>=0&&c->actuator_force_clipped<=2);}
static coax_t_PntStatus pnt_control_observed(int id,int tick,double*q,double*v,coax_t_PntCommand*out){struct{unsigned char prefix[32];coax_t_PntCommand value[2];unsigned char suffix[32];}box,old;double input[18];memcpy(input,q,72);memcpy(input+9,v,72);memset(&box,0xa5,sizeof box);memcpy(&old,&box,sizeof box);coax_t_PntStatus status=pnt_control(id,tick,(coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_t_PntCommand){box.value,1});PNT_NEED(!memcmp(input,q,72)&&!memcmp(input+9,v,72));if(status.stage||status.code)PNT_NEED(!memcmp(&box,&old,sizeof box));else{pnt_command_fields(&box.value[0]);PNT_NEED(!memcmp(box.prefix,old.prefix,32)&&!memcmp(box.suffix,old.suffix,32)&&!memcmp(&box.value[1],&old.value[1],712));memcpy(out,&box.value[0],712);}return status;}
static coax_t_PntStatus pnt_step_observed(int id,int tick,double*q,double*v,coax_t_PntStep*out){struct{unsigned char prefix[32];coax_t_PntStep value[2];unsigned char suffix[32];}box,old;double input[18];memcpy(input,q,72);memcpy(input+9,v,72);memset(&box,0xa5,sizeof box);memcpy(&old,&box,sizeof box);coax_t_PntStatus status=pnt_step(id,tick,(coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_t_PntStep){box.value,1});PNT_NEED(!memcmp(input,q,72)&&!memcmp(input+9,v,72));if(status.stage||status.code)PNT_NEED(!memcmp(&box,&old,sizeof box));else{pnt_command_fields(&box.value[0].command);PNT_NEED(!memcmp(box.prefix,old.prefix,32)&&!memcmp(box.suffix,old.suffix,32)&&!memcmp(&box.value[1],&old.value[1],944));memcpy(out,&box.value[0],944);}return status;}
static void pnt_unpack_plant(const coax_t_PeuOutput*p,double*q,double*v){double all[27];memcpy(all,p,216);pnt_finite(all,27);for(int j=0;j<7;j++){q[j]=p->angles.e[j];v[j]=p->hinge_rates.e[j];}for(int j=0;j<2;j++){q[7+j]=p->slides.e[j];v[7+j]=p->slide_rates.e[j];}pnt_state(q,v);PNT_NEED(p->control_clipped>=0&&p->control_clipped<=9&&p->force_clipped>=0&&p->force_clipped<=2);}
static void pnt_pack(PntRecord*out,int id,int tick,coax_t_PntStatus status,const double*preq,const double*prev,const coax_t_PntCommand*c,const double*actuator,const double*postq,const double*postv,double time){memcpy(out->values,preq,72);memcpy(out->values+9,prev,72);memcpy(out->values+18,&c->reference,168);memcpy(out->values+39,&c->controller,280);memcpy(out->values+74,c->controls.e,72);memcpy(out->values+83,c->external.e,72);memcpy(out->values+92,actuator,72);memcpy(out->values+101,postq,72);memcpy(out->values+110,postv,72);out->values[119]=time;int64_t meta[]={id,tick,status.stage,status.code,c->controller.clipped,c->reference_tick,c->reference_steps,c->pulse,c->actuator_control_clipped,c->actuator_force_clipped};memcpy(out->meta,meta,80);pnt_finite(out->values,120);}
#endif
