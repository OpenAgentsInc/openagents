/* Private V1 opt-in. The included generated session must be
   panda-query-runtime.coax. Every callback here performs effects/transport;
   Coax owns admission, query construction and clock/controller decisions. */
#ifndef PSIONIC_PANDA_QUERY_RUNTIME_V1_H
#define PSIONIC_PANDA_QUERY_RUNTIME_V1_H
#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "panda-env.h"
#include "panda-session.h"
typedef struct {coax_t_PqnDescriptorV1 descriptor;} PandaLoopQueryV1;
static int panda_query_read_v1(const char*name,unsigned char**bytes,size_t*count){
 *bytes=NULL;*count=0;FILE*f=fopen(name,"rb");if(!f)return 0;if(fseek(f,0,SEEK_END)){fclose(f);return 0;}long n=ftell(f);if(n<0){fclose(f);return 0;}rewind(f);if(pqn_file_request((int64_t)n,32,0)){fclose(f);return 0;}unsigned char*p=malloc((size_t)n+1);if(!p){fclose(f);return 0;}int ok=fread(p,1,(size_t)n,f)==(size_t)n;int closed=!fclose(f);if(!ok||!closed){free(p);return 0;}p[n]=0;*bytes=p;*count=(size_t)n;return 1;
}
static int panda_query_hash_v1(const unsigned char*p,size_t count,unsigned char out[32]){return !pqn_file_hash((coax_v_u8){(unsigned char*)p,(int64_t)count},(coax_v_u8){out,32},0);}
static int panda_query_file_digest_v1(const char*name,unsigned char out[32]){unsigned char*p=NULL;size_t n=0;int ok=panda_query_read_v1(name,&p,&n);if(ok)ok=panda_query_hash_v1(p,n,out);free(p);return ok;}
/* No SDK API is invoked: inspect the actual loaded implementation's path,
   rather than accepting a caller's pathname for an unrelated approved file. */
static const char*panda_query_sdk_path_v1(void){Dl_info info;void*symbol=dlsym(RTLD_DEFAULT,"mj_loadXML");if(!symbol||!dladdr(symbol,&info))return NULL;return info.dli_fname;}
static int panda_query_preflight_v1(const char*choice,int argc,char**argv,coax_t_PqnDescriptorV1*out){
 coax_string text={choice?choice:"",choice?(int64_t)strlen(choice):0};coax_t_PqnDescriptorV1 descriptor=pqn_descriptor(text);if(descriptor.status)return 0;
 const char*sdk_path=panda_query_sdk_path_v1();unsigned char sdk[32]={0};int sdk_ok=sdk_path&&panda_query_file_digest_v1(sdk_path,sdk);if(argc<10)return 0;
 for(int c=9;c<argc;c++){
  char xml_path[4096],init_path[4096];if(snprintf(xml_path,sizeof xml_path,"%s/scene.xml",argv[c])>=(int)sizeof xml_path||snprintf(init_path,sizeof init_path,"%s/init.f64",argv[c])>=(int)sizeof init_path)return 0;
  unsigned char*xml=NULL;size_t n=0;unsigned char xh[32]={0},ih[32]={0},assets[2720]={0};int xml_ok=panda_query_read_v1(xml_path,&xml,&n);int ok=sdk_ok&&xml_ok;if(xml_ok)ok=panda_query_hash_v1(xml,n,xh)&&ok;ok=panda_query_file_digest_v1(init_path,ih)&&ok;
  /* Authenticate this exact model BEFORE extracting any paths from it. */
  coax_t_PqnStatus model=pqn_model((coax_v_u8){xh,32},(coax_v_u8){ih,32},(coax_v_u8){sdk,32},ok?0:1);if(model){free(xml);return 0;}
  for(int i=0;i<descriptor.assets;i++){
   coax_string borrowed=pqn_asset_path((coax_v_u8){xml,(int64_t)n},i);char name[4096];int read=borrowed.count>0&&borrowed.count<(int64_t)sizeof name;if(read){memcpy(name,borrowed.data,(size_t)borrowed.count);name[borrowed.count]=0;read=panda_query_file_digest_v1(name,assets+32*i);}if(!read)ok=0;
  }
  coax_t_PqnStatus admitted=pqn_admit((coax_v_u8){xh,32},(coax_v_u8){ih,32},(coax_v_u8){sdk,32},(coax_v_u8){assets,2720},ok?0:1);free(xml);if(admitted)return 0;
 }
 *out=descriptor;return 1;
}
/* Trusted internal helper: the entrypoint already admitted its fixed scene.
   Clock state adopts every result; sample output publishes only after ALL
   stages succeed. Caller telemetry is an effect buffer and can change. */
static int panda_query_current_v1(PandaEnv*env,const PandaTelemetry*t,int sampled,coax_t_PcSample*out){
 PandaRobotStateV1 robot={0};int read=panda_env_robot_state(env,&robot);uint64_t words[173],q[9],v[9];memcpy(words,t,sizeof words);memcpy(q,robot.q,sizeof q);memcpy(v,robot.qd,sizeof v);
 coax_t_PqsResult chosen=pqs_select(1,(coax_v_u64){words,173},(coax_v_u64){q,9},(coax_v_u64){v,9},sampled?0:1,read?0:1,(coax_v_t_PcSample){out,1});return !chosen.code;
}
static int panda_query_control_v1(PandaEnv*env,PandaTelemetry*t,coax_t_PkState*clock,int scenario,int substep,coax_t_PcSample*out){
 coax_t_PkResult stage=pk_forward(*clock,panda_env_forward(env)?0:1);*clock=stage.state;if(stage.status)return 0;
 *t=(PandaTelemetry){0};int sampled=panda_env_sample(env,t);coax_t_PcSample selected;int chosen=panda_query_current_v1(env,t,sampled,&selected);
 stage=pk_sample(*clock,chosen?0:1);*clock=stage.state;if(stage.status)return 0;stage=pk_control(*clock,substep);*clock=stage.state;if(stage.status)return 0;
 *out=ps_sample(scenario,clock->supervisor.tick,substep,selected);return 1;
}
#endif
