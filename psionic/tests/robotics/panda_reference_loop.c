/* Firm reason2: reference-provider effects entrypoint, called by Python. */
#define _POSIX_C_SOURCE 200809L
#include "panda_loop_effects.h"
#include "panda_reference_bridge.h"
static int reference_forward(void*p,const uint8_t*a,const uint8_t*w,const float*s,const float*n,float*out,PandaPolicyTiming*t){return panda_reference_forward(p,a,w,s,n,out,t);}
static const char*reference_error(const void*p){return panda_reference_error(p);}
int panda_reference_run(PandaReference*reference,int argc,char**argv,double opened){
 REQUIRE(reference&&argc>=10);size_t bytes;unsigned char*noise=read_file(argv[3],&bytes);
 PandaLoopProvider provider={reference,reference_forward,reference_error,"panda-libero-mujoco331-transformers-independenthead-mps-f32-fixed-noise-v1","psionic-panda-reference-closed-loop-v1",0};int result=panda_loop_cases(argc,argv,&provider,noise,bytes,opened);free(noise);return result;
}
