/* Firm reason2: opt-in reference FFI. Every preparation/decoder operator and
   dimension comes from the already qualified Coax helpers/descriptors. */
#define _POSIX_C_SOURCE 200809L
#include "panda-policy-generated.h"
#include "panda_reference_bridge.h"
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
struct PandaReference {
 PandaReferenceInfer infer;void*user;char error[512];int complete;int64_t tokens;
 unsigned char*pst;size_t pst_bytes;
 uint8_t rendered[8192],renderwork[8192],normalwork[8192];
 int64_t ids[2048],stage[2048],scalars[8192],symbols[2048],request[3072];
 float image[196608],imagework[196608],pixels[786432],state[132],noise[5280];
 uint32_t words[5280],returned_words[5280];float returned[5280],normal[5280],decoded[112];
};
static double seconds(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+1e-9*t.tv_nsec;}
static int refused(PandaReference*p,const char*format,...){va_list ap;va_start(ap,format);vsnprintf(p->error,sizeof p->error,format,ap);va_end(ap);return 0;}
PandaReference*panda_reference_open(const char*pst,const char*instruction,PandaReferenceInfer infer,void*user,char*error,size_t cap){
 PandaReference*p=calloc(1,sizeof*p);if(!p)return NULL;p->infer=infer;p->user=user;if(!pst||!instruction||!infer){refused(p,"reference arguments");goto fail;}
 FILE*f=fopen(pst,"rb");if(!f){refused(p,"PST open");goto fail;}if(fseek(f,0,SEEK_END)){fclose(f);refused(p,"PST seek");goto fail;}long size=ftell(f);if(size<0||fseek(f,0,SEEK_SET)){fclose(f);refused(p,"PST extent");goto fail;}p->pst=malloc((size_t)size+1);if(!p->pst){fclose(f);refused(p,"PST allocation");goto fail;}p->pst_bytes=(size_t)size;int read_ok=fread(p->pst,1,p->pst_bytes,f)==p->pst_bytes;read_ok=fclose(f)==0&&read_ok;if(!read_ok){refused(p,"PST read");goto fail;}
 coax_t_GrRead rendered=gr_render((coax_string){instruction,(int64_t)strlen(instruction)},(coax_v_u8){p->rendered,8192},(coax_v_u8){p->renderwork,8192},0);if(rendered.status){refused(p,"Coax render%d",rendered.status);goto fail;}
 coax_t_PtResult ids=pt_encode_checked((coax_v_u8){p->pst,(int64_t)p->pst_bytes},(coax_string){(char*)p->rendered,rendered.written},(coax_v_s64){p->ids,2048},(coax_v_s64){p->stage,2048},(coax_v_u8){p->normalwork,8192},(coax_v_s64){p->scalars,8192},(coax_v_s64){p->symbols,2048},0);if(ids.status||ids.written!=152){refused(p,"Coax source task IDs%d/count%lld",ids.status,(long long)ids.written);goto fail;}p->tokens=ids.written;
 coax_t_GrRead plan=gr_plan((coax_v_s64){p->ids,p->tokens},0,(coax_v_s64){p->request,3072},0);if(plan.status||plan.written!=6*p->tokens){refused(p,"Coax source request plan%d",plan.status);goto fail;}return p;
fail:if(error&&cap)snprintf(error,cap,"%s",p->error);panda_reference_close(p);return NULL;
}
int panda_reference_forward(PandaReference*p,const uint8_t*a,const uint8_t*w,const float*state,const float*noise,float*out,PandaPolicyTiming*timing){
 if(!p)return 0;p->complete=0;p->error[0]=0;if(!a||!w||!state||!noise||!out)return refused(p,"reference frame arguments");double begin=seconds();coax_t_PvnCursor c=pvn_begin(p->tokens);coax_v_t_PvnCursor cursor={&c,1};
 for(int at=0;at<6;at++){coax_t_PvnPrep d=pvn_prep(at);int status=0;int64_t written=0;
  if(d.kind==0){coax_t_GiRead r=gi_preprocess(d.first,d.rows,(coax_v_u8){(uint8_t*)(d.camera?w:a),d.input_count},(coax_v_f32){p->image,d.output_count},(coax_v_f32){p->imagework,d.output_count},0);status=r.status;written=r.written;}
  else if(d.kind==1){coax_t_GvRead r=gv_pack(d.frames,d.height,d.width,(coax_v_f32){p->image,d.input_count},(coax_v_f32){p->pixels+d.offset,d.output_count},0);status=r.status;written=r.written;}
  else if(d.kind==2){coax_t_GeRead r=ge_normalize((coax_v_f32){(float*)state,d.input_count},(coax_v_f32){p->state,d.output_count},0);status=r.status;written=r.written;}
  else if(d.kind==3){memcpy(p->words,noise,(size_t)d.input_count*4);coax_t_GvpRead r=pvn_noise((coax_v_u32){p->words,d.input_count},(coax_v_f32){p->noise,d.output_count},0);status=r.status;written=r.written;}
  else return refused(p,"Coax reference preparation operation");
  if(pvn_prep_ack(cursor,at,status,written))return refused(p,"Coax reference preparation%d refused%d",at,status);
 }
 double prepared=seconds();for(int i=0;i<5280;i++)p->returned_words[i]=0x7fc00001;memcpy(p->returned,p->returned_words,sizeof p->returned);
 if(!p->infer(p->user,p->pixels,p->ids,p->tokens,p->state,p->noise,p->returned))return refused(p,"MPS reference callback refused");
 memcpy(p->returned_words,p->returned,sizeof p->returned_words);coax_t_GvpRead finite=pvn_noise((coax_v_u32){p->returned_words,5280},(coax_v_f32){p->normal,5280},0);if(finite.status)return refused(p,"Reference result incomplete/nonfinite%d",finite.status);
 coax_t_GeRead decoded=ge_decode((coax_v_f32){p->normal,5280},(coax_v_f32){p->decoded,112},0);if(decoded.status||decoded.written!=112)return refused(p,"Coax reference decoder%d",decoded.status);
 memcpy(out,p->decoded,sizeof p->decoded);if(timing){memset(timing,0,sizeof*timing);timing->tokens=p->tokens;timing->preparation_seconds=prepared-begin;timing->total_seconds=seconds()-begin;}p->complete=1;return 1;
}
const char*panda_reference_error(const PandaReference*p){return p?p->error:"no reference context";}
int panda_reference_copy(PandaReference*p,const char*name,void*out,size_t bytes){if(!p||!p->complete||!name||!out)return 0;const void*source=NULL;size_t count=0;
 if(!strcmp(name,"pixels")){source=p->pixels;count=sizeof p->pixels;}else if(!strcmp(name,"state")){source=p->state;count=sizeof p->state;}else if(!strcmp(name,"noise")){source=p->noise;count=sizeof p->noise;}else if(!strcmp(name,"ids")){source=p->ids;count=(size_t)p->tokens*8;}else if(!strcmp(name,"normalized-actions")){source=p->normal;count=sizeof p->normal;}else if(!strcmp(name,"decoded-actions")){source=p->decoded;count=sizeof p->decoded;}else return 0;if(bytes!=count)return 0;memcpy(out,source,count);return 1;}
void panda_reference_close(PandaReference*p){if(p){free(p->pst);free(p);}}
