/* Firm reason2: changed FFI/preparation/publication boundary oracle; no model. */
#include "panda_reference_bridge.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int mode,calls;static unsigned long checks;
#define CHECK(x) do{assert(x);checks++;}while(0)
static int infer(void *ctx,const float *pixels,const int64_t *ids,int64_t n,const float *state,const float *noise,float *out){
 CHECK(ctx==&mode&&n==152);int images=0;for(int i=0;i<n;i++){CHECK(ids[i]>=0&&ids[i]<151936);images+=ids[i]==151655;}CHECK(images==128);
 for(int i=0;i<786432;i++)CHECK(pixels[i]==-1);for(int i=8;i<132;i++)CHECK(state[i]==0);for(int i=0;i<5280;i++)CHECK(noise[i]==0);
 calls++;if(mode==1)return 0;if(mode!=4)for(int i=0;i<5280;i++)out[i]=0;out[0]=1;out[5279]=-1;if(mode==2)out[0]=NAN;if(mode==3)out[5279]=NAN;return 1;
}
int main(int argc,char **argv){CHECK(argc==2);char error[512]={0};const char*instruction="pick up the book and place it in the back compartment of the caddy";
 CHECK(panda_reference_open(argv[1],instruction,NULL,&mode,error,sizeof error)==NULL);
 PandaReference*p=panda_reference_open(argv[1],instruction,infer,&mode,error,sizeof error);if(!p)fprintf(stderr,"%s\n",error);CHECK(p!=NULL);
 uint8_t *raw=calloc(196608,1);float state[8]={0},noise[5280]={0},out[113],normal[5280];CHECK(raw!=NULL);
 for(mode=0;mode<5;mode++){for(int i=0;i<113;i++)out[i]=777;PandaPolicyTiming timing;int ok=panda_reference_forward(p,raw,raw,state,noise,out,&timing);CHECK(ok==(mode==0));CHECK(out[112]==777);if(ok){CHECK(timing.tokens==152&&timing.total_seconds>=timing.preparation_seconds);CHECK(panda_reference_copy(p,"normalized-actions",normal,sizeof normal));CHECK(normal[0]==1&&normal[5279]==-1);CHECK(!panda_reference_copy(p,"normalized-actions",normal,sizeof normal-1));for(int i=0;i<112;i++)CHECK(isfinite(out[i])&&out[i]!=777);}else {CHECK(strlen(panda_reference_error(p))>0);CHECK(!panda_reference_copy(p,"normalized-actions",normal,sizeof normal));for(int i=0;i<113;i++)CHECK(out[i]==777);}}
 CHECK(calls==5);CHECK(!panda_reference_forward(p,NULL,raw,state,noise,out,NULL));panda_reference_close(p);free(raw);printf("reference provider native: %lu checks; finite/full callback publication and atomic refusal\n",checks);return 0;
}
