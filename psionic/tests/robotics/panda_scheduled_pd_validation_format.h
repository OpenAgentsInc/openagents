#ifndef PSIONIC_PANDA_SCHEDULED_PD_VALIDATION_FORMAT_H
#define PSIONIC_PANDA_SCHEDULED_PD_VALIDATION_FORMAT_H
#include "panda_joint_pd_tracking_format.h"
enum{CD_CONTEXTS=8,CD_STEPS=750};
#define CD_INPUT_MAGIC UINT32_C(0x31494443)
#define CD_SDK_MAGIC UINT32_C(0x31534443)
#define CD_OWN_MAGIC UINT32_C(0x314f4443)
typedef struct{PntRecord fresh;coax_t_PsdSchedule schedule;}CdRecord;
_Static_assert(sizeof(CdRecord)==1072&&offsetof(CdRecord,schedule)==1040,"C/D record ABI");
_Static_assert(sizeof(coax_t_PsvReference)==184&&offsetof(coax_t_PsvReference,reference_tick)==168&&offsetof(coax_t_PsvReference,reference_steps)==176,"C/D reference ABI");
_Static_assert(sizeof(coax_t_PsvVerdict)==16&&offsetof(coax_t_PsvVerdict,valid)==0&&offsetof(coax_t_PsvVerdict,feasible)==8,"C/D verdict ABI");
static void cd_header(FILE*f,uint32_t magic,uint32_t bytes){uint32_t words[]={magic,2,4,750,bytes,1};PNT_NEED(fwrite(words,1,24,f)==24&&!fflush(f));}
static void cd_read_header(FILE*f,uint32_t magic,uint32_t bytes){uint32_t w[6];PNT_NEED(fread(w,1,24,f)==24&&w[0]==magic&&w[1]==2&&w[2]==4&&w[3]==750&&w[4]==bytes&&w[5]==1);}
static void cd_input(const char*path,coax_t_PntInitial out[8]){FILE*f=fopen(path,"rb");PNT_NEED(f);cd_read_header(f,CD_INPUT_MAGIC,144);PNT_NEED(fread(out,1,1152,f)==1152&&fgetc(f)==EOF&&!fclose(f));for(int i=0;i<8;i++)pnt_state(out[i].q.e,out[i].v.e);}
static void cd_initial(int context,coax_t_PntInitial*out){struct{unsigned char a[32];coax_t_PntInitial out[2];unsigned char z[32];}box,old;memset(&box,0xa5,sizeof box);memcpy(&old,&box,sizeof box);coax_t_PntStatus status=psv_initial(context,(coax_v_t_PntInitial){box.out,1});PNT_NEED(status.stage==0&&status.code==0&&!memcmp(box.a,old.a,32)&&!memcmp(box.z,old.z,32)&&!memcmp(&box.out[1],&old.out[1],144));pnt_state(box.out[0].q.e,box.out[0].v.e);memcpy(out,&box.out[0],144);}
static void cd_schedule(const coax_t_PsdSchedule*s){double f[3];memcpy(f,s,24);pnt_finite(f,3);PNT_NEED(s->absolute_error>=0&&s->absolute_error<=40&&s->damping>=4&&s->damping<=20&&s->regime>=0&&s->regime<=2);}
static void cd_command_fields(const coax_t_PsdCommand*c){pnt_command_fields(&c->base);cd_schedule(&c->schedule);}
static coax_t_PntStatus cd_command(int context,int64_t tick,double*q,double*v,coax_t_PsdCommand*out){struct{unsigned char a[32];coax_t_PsdCommand out[2];unsigned char z[32];}box,old;memset(&box,0xa5,sizeof box);memcpy(&old,&box,sizeof box);double before[18];memcpy(before,q,72);memcpy(before+9,v,72);coax_t_PntStatus status=psv_command(context,tick,(coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_t_PsdCommand){box.out,1});PNT_NEED(!memcmp(q,before,72)&&!memcmp(v,before+9,72));if(status.stage||status.code)PNT_NEED(!memcmp(&box,&old,sizeof box));else{cd_command_fields(&box.out[0]);PNT_NEED(!memcmp(box.a,old.a,32)&&!memcmp(box.z,old.z,32)&&!memcmp(&box.out[1],&old.out[1],744));memcpy(out,&box.out[0],744);}return status;}
static coax_t_PntStatus cd_step(int context,int64_t tick,double*q,double*v,coax_t_PsdStep*out){struct{unsigned char a[32];coax_t_PsdStep out[2];unsigned char z[32];}box,old;memset(&box,0xa5,sizeof box);memcpy(&old,&box,sizeof box);double before[18];memcpy(before,q,72);memcpy(before+9,v,72);coax_t_PntStatus status=psv_step(context,tick,(coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_t_PsdStep){box.out,1});PNT_NEED(!memcmp(q,before,72)&&!memcmp(v,before+9,72));if(status.stage||status.code)PNT_NEED(!memcmp(&box,&old,sizeof box));else{cd_command_fields(&box.out[0].command);PNT_NEED(!memcmp(box.a,old.a,32)&&!memcmp(box.z,old.z,32)&&!memcmp(&box.out[1],&old.out[1],976));memcpy(out,&box.out[0],976);}return status;}
static void cd_pack(CdRecord*r,int context,int tick,coax_t_PntStatus status,const double*q,const double*v,const coax_t_PsdCommand*c,const double*force,const double*postq,const double*postv,double time){pnt_pack(&r->fresh,context%4,tick,status,q,v,&c->base,force,postq,postv,time);r->schedule=c->schedule;cd_schedule(&r->schedule);}
static void cd_index_start(FILE*f,int64_t ix[8][6]){for(int c=0;c<8;c++){ix[c][0]=c/4;ix[c][1]=c%4;ix[c][2]= -1;ix[c][3]=0;ix[c][4]= -1;ix[c][5]= -1;}PNT_NEED(fwrite(ix,1,384,f)==384&&!fflush(f));}
static void cd_index_write(FILE*f,int context,int64_t ix[8][6]){PNT_NEED(!fseek(f,context*48,SEEK_SET)&&fwrite(ix[context],1,48,f)==48&&!fflush(f));}
static void cd_event(FILE*f,int context,int tick,const char*event,coax_t_PntStatus status){PNT_NEED(fprintf(f,"{\"context\":%d,\"pose\":%d,\"scenario\":%d,\"tick\":%d,\"event\":\"%s\",\"stage\":%lld,\"code\":%lld}\n",context,context<0?-1:context/4,context<0?-1:context%4,tick,event,(long long)status.stage,(long long)status.code)>0&&!fflush(f));}
#endif
