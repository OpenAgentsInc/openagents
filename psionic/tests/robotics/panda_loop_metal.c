/* Native Coax NN provider; SDK/control effects are shared with the opt-in reference. */
#define _POSIX_C_SOURCE 200809L
#include "panda_loop_effects.h"
static int native_forward(void*p,const uint8_t*a,const uint8_t*w,const float*s,const float*n,float*out,PandaPolicyTiming*t){return panda_policy_forward(p,a,w,s,n,out,t);}
static const char*native_error(const void*p){return panda_policy_error(p);}
int main(int argc,char **argv){
  REQUIRE(argc>=10);/* SHARDS PST NOISE SHADERS OUT INSTRUCTION MAX_TICKS FAULT CASE... */
  const char *shaders=argv[4],*output=argv[5];int max_ticks=atoi(argv[7]),scenario=atoi(argv[8]);REQUIRE(max_ticks>0&&max_ticks<=720&&scenario>=0&&scenario<=4);
  mkdir(output,0700);size_t noise_bytes;unsigned char *noise_raw=read_file(argv[3],&noise_bytes);REQUIRE(noise_bytes==5280*4);float noise[5280];memcpy(noise,noise_raw,sizeof(noise));
#ifdef PANDA_OWNED_QUERY_V1
  coax_t_PqnDescriptorV1 query;REQUIRE(panda_query_preflight_v1(getenv("PANDA_QUERY_SOURCE"),argc,argv,&query));
#endif
  PandaPolicyConfig cfg={.shard_directory=argv[1],.pst_path=argv[2],.instruction=argv[6]};const char *modules[6]={"tile.metal","device.metal","vision.metal","wide.metal","text.metal","action.metal"};char module_paths[6][4096];for(int i=0;i<6;i++){path(module_paths[i],shaders,modules[i]);cfg.metal_paths[i]=module_paths[i];}
  char error[2048]={0};PandaPolicy *policy=NULL;double opened=now();REQUIRE(panda_policy_open(&cfg,&policy,error,sizeof(error)));double open_seconds=now()-opened;
#ifdef PANDA_OWNED_QUERY_V1
  char profile[256],schema[256];REQUIRE(query.profile.count>0&&query.profile.count<256&&query.schema.count>0&&query.schema.count<256);memcpy(profile,query.profile.data,query.profile.count);profile[query.profile.count]=0;memcpy(schema,query.schema.data,query.schema.count);schema[query.schema.count]=0;
  PandaLoopProvider provider={policy,native_forward,native_error,profile,schema,1};
#else
  coax_string name=ps_profile();char profile[256];REQUIRE(name.count>0&&name.count<256);memcpy(profile,name.data,name.count);profile[name.count]=0;
  PandaLoopProvider provider={policy,native_forward,native_error,profile,"psionic-panda-closed-loop-v1",1};
#endif
  int result=panda_loop_cases(argc,argv,&provider,noise_raw,noise_bytes,open_seconds);panda_policy_close(policy);free(noise_raw);return result;
}
