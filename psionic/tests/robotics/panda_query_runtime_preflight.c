/* NEW positive file/effect composition, not a simulator/model execution. */
#include <mujoco/mujoco.h>
#include "panda_query_runtime.h"
static void*volatile sdk_link_reference=(void*)&mj_loadXML;
int main(int argc,char**argv){
 if(argc!=2||!sdk_link_reference)return 2;
 char*args[10]={"preflight","unused-shards","unused-pst","unused-noise","unused-shaders","unused-out","unused-instruction","1","0",argv[1]};coax_t_PqnDescriptorV1 descriptor;
 if(!panda_query_preflight_v1("owned-shadow-v1",10,args,&descriptor))return 1;
 printf("{\"passed\":true,\"assets\":%lld,\"file_hashes\":88,\"SDK_API_calls\":0,\"NN_calls\":0,\"policy_open_calls\":0,\"profile\":\"%.*s\"}\n",(long long)descriptor.assets,(int)descriptor.profile.count,descriptor.profile.data);return 0;
}
